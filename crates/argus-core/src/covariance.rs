//! Covariance matrix manager for the Argus risk engine.
//!
//! Owns a set of [`WelfordOnline`] instances for per-asset variance
//! (diagonal entries) and [`HayashiYoshida`] instances for pairwise
//! asynchronous covariance (off-diagonal entries).
//!
//! The off-diagonal pairs are stored in a flat `Vec` indexed by the
//! upper-triangular pair index: `i * n - i * (i + 1) / 2 + j - i - 1`
//! for `i < j`.

use crate::error::ArgusError;
use crate::math::{HayashiYoshida, Tick, WelfordOnline};
use crate::types::TimestampNs;

/// Manages a live covariance matrix for `n` assets.
///
/// Diagonal elements are tracked by [`WelfordOnline`] (sample variance)
/// and off-diagonal elements by [`HayashiYoshida`] (asynchronous covariance).
///
/// # Hot-path invariant
///
/// After construction, [`update_tick`] performs no heap allocation — all
/// estimator storage is pre-allocated in `new`.
pub struct CovarianceManager {
    /// Number of assets.
    n: usize,
    /// Diagonal variances — one [`WelfordOnline`] per asset.
    welford: Vec<WelfordOnline>,
    /// Off-diagonal covariances — `n * (n - 1) / 2` [`HayashiYoshida`] pairs.
    hy_pairs: Vec<HayashiYoshida>,
}

impl CovarianceManager {
    /// Creates a new `CovarianceManager` for `n` assets.
    ///
    /// Pre-allocates all estimator state so the hot path is allocation-free.
    pub fn new(n: usize) -> Self {
        let welford = (0..n).map(|_| WelfordOnline::new()).collect();
        let num_pairs = n * (n.saturating_sub(1)) / 2;
        let hy_pairs = (0..num_pairs).map(|_| HayashiYoshida::new()).collect();
        Self {
            n,
            welford,
            hy_pairs,
        }
    }

    /// Returns the flat pair index for the upper-triangular entry `(i, j)` where `i < j`.
    #[inline]
    fn pair_index(&self, i: usize, j: usize) -> usize {
        i * self.n - i * (i + 1) / 2 + j - i - 1
    }

    /// Updates the covariance matrix with a new tick for `asset_idx`.
    ///
    /// Feeds the observation into the diagonal [`WelfordOnline`] and into
    /// every [`HayashiYoshida`] pair that includes `asset_idx`.
    ///
    /// # Errors
    ///
    /// Returns [`ArgusError::InvalidParameter`] if `asset_idx >= n`.
    #[inline]
    pub fn update_tick(
        &mut self,
        asset_idx: usize,
        ts: TimestampNs,
        price: f64,
    ) -> Result<(), ArgusError> {
        if asset_idx >= self.n {
            return Err(ArgusError::InvalidParameter {
                name: "asset_idx",
                reason: format!(
                    "asset index {} out of range for {}-asset universe",
                    asset_idx, self.n
                ),
            });
        }

        // Update diagonal variance estimator.
        self.welford[asset_idx].update(price);

        let tick = Tick { ts, price };

        // Update every HY pair involving this asset.
        for other in 0..self.n {
            if other == asset_idx {
                continue;
            }
            let (i, j) = if asset_idx < other {
                (asset_idx, other)
            } else {
                (other, asset_idx)
            };
            let idx = self.pair_index(i, j);
            if asset_idx < other {
                self.hy_pairs[idx].update_x(tick);
            } else {
                self.hy_pairs[idx].update_y(tick);
            }
        }

        Ok(())
    }

    /// Returns the sample variance for `asset_idx`.
    ///
    /// # Errors
    ///
    /// Returns [`ArgusError::InvalidParameter`] if `asset_idx >= n`,
    /// or [`ArgusError::InsufficientData`] if fewer than 2 observations.
    #[inline]
    pub fn variance(&self, asset_idx: usize) -> Result<f64, ArgusError> {
        if asset_idx >= self.n {
            return Err(ArgusError::InvalidParameter {
                name: "asset_idx",
                reason: format!(
                    "asset index {} out of range for {}-asset universe",
                    asset_idx, self.n
                ),
            });
        }
        self.welford[asset_idx].variance()
    }

    /// Returns the Hayashi-Yoshida covariance estimate for assets `(i, j)`.
    ///
    /// # Errors
    ///
    /// Returns [`ArgusError::InvalidParameter`] if `i == j` or either index is
    /// out of range, or [`ArgusError::InsufficientData`] if no overlapping
    /// intervals have been observed.
    #[inline]
    pub fn covariance(&self, i: usize, j: usize) -> Result<f64, ArgusError> {
        if i >= self.n || j >= self.n {
            return Err(ArgusError::InvalidParameter {
                name: "asset_index",
                reason: format!(
                    "index ({}, {}) out of range for {}-asset universe",
                    i, j, self.n
                ),
            });
        }
        if i == j {
            return self.welford[i].variance();
        }
        let (lo, hi) = if i < j { (i, j) } else { (j, i) };
        let idx = self.pair_index(lo, hi);
        self.hy_pairs[idx].covariance()
    }

    /// Returns the Pearson correlation between assets `(i, j)`.
    ///
    /// Computed as `cov(i,j) / sqrt(var(i) * var(j))`.
    ///
    /// # Errors
    ///
    /// Returns an error if covariance or either variance is unavailable.
    #[inline]
    pub fn correlation(&self, i: usize, j: usize) -> Result<f64, ArgusError> {
        if i == j {
            return Ok(1.0);
        }
        let cov = self.covariance(i, j)?;
        let var_i = self.variance(i)?;
        let var_j = self.variance(j)?;
        let denom = (var_i * var_j).sqrt();
        if denom < f64::EPSILON {
            return Err(ArgusError::InsufficientData {
                needed: 2,
                have: 0,
            });
        }
        Ok(cov / denom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_correct_pair_count() {
        let mgr = CovarianceManager::new(4);
        assert_eq!(mgr.welford.len(), 4);
        assert_eq!(mgr.hy_pairs.len(), 6); // 4*3/2
    }

    #[test]
    fn pair_index_formula() {
        let mgr = CovarianceManager::new(4);
        // (0,1)=0, (0,2)=1, (0,3)=2, (1,2)=3, (1,3)=4, (2,3)=5
        assert_eq!(mgr.pair_index(0, 1), 0);
        assert_eq!(mgr.pair_index(0, 2), 1);
        assert_eq!(mgr.pair_index(0, 3), 2);
        assert_eq!(mgr.pair_index(1, 2), 3);
        assert_eq!(mgr.pair_index(1, 3), 4);
        assert_eq!(mgr.pair_index(2, 3), 5);
    }

    #[test]
    fn out_of_range_returns_error() {
        let mut mgr = CovarianceManager::new(2);
        assert!(mgr.update_tick(2, TimestampNs(0), 100.0).is_err());
        assert!(mgr.variance(5).is_err());
        assert!(mgr.covariance(0, 3).is_err());
    }

    #[test]
    fn self_correlation_is_one() {
        let mgr = CovarianceManager::new(3);
        assert!((mgr.correlation(1, 1).unwrap() - 1.0).abs() < f64::EPSILON);
    }
}
