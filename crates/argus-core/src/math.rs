//! Mathematical estimators for Argus.

use crate::error::ArgusError;
use crate::types::{ForgettingFactor, TimestampNs};

/// Welford's online variance estimator.
///
/// Reference: Welford, B. P. (1962). "Note on a method for calculating corrected sums of squares and products".
/// Technometrics. 4 (3): 419–420.
#[derive(Debug, Clone, Default)]
pub struct WelfordOnline {
    count: u64,
    mean: f64,
    m2: f64,
}

impl WelfordOnline {
    /// Creates a new `WelfordOnline` estimator.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the estimator with a new observation.
    #[inline]
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / (self.count as f64);
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Returns the sample variance.
    ///
    /// # Errors
    /// Returns `ArgusError::InsufficientData` if fewer than 2 observations have been provided.
    #[inline]
    pub fn variance(&self) -> Result<f64, ArgusError> {
        if self.count < 2 {
            return Err(ArgusError::InsufficientData { needed: 2, have: self.count });
        }
        Ok(self.m2 / (self.count - 1) as f64)
    }

    /// Returns the sample mean.
    #[inline]
    pub fn mean(&self) -> f64 {
        self.mean
    }
}

/// A tick representing a price observation at a specific time.
#[derive(Debug, Clone, Copy)]
pub struct Tick {
    pub ts: TimestampNs,
    pub price: f64,
}

/// Hayashi-Yoshida asynchronous covariance estimator.
///
/// Reference: Hayashi, T., & Yoshida, N. (2005). "On covariance estimation of non-synchronously observed diffusion processes".
/// Bernoulli, 11(2), 359-379.
#[derive(Debug, Clone, Default)]
pub struct HayashiYoshida {
    cov: f64,
    
    x_prev: Option<Tick>,
    x_curr: Option<Tick>,
    
    y_prev: Option<Tick>,
    y_curr: Option<Tick>,
    
    count: u64,
}

impl HayashiYoshida {
    /// Creates a new `HayashiYoshida` estimator.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with an observation for asset X.
    #[inline]
    pub fn update_x(&mut self, tick: Tick) {
        self.x_prev = self.x_curr;
        self.x_curr = Some(tick);
        self.accumulate();
    }

    /// Update with an observation for asset Y.
    #[inline]
    pub fn update_y(&mut self, tick: Tick) {
        self.y_prev = self.y_curr;
        self.y_curr = Some(tick);
        self.accumulate();
    }
    
    #[inline]
    fn accumulate(&mut self) {
        if let (Some(x_p), Some(x_c), Some(y_p), Some(y_c)) = (self.x_prev, self.x_curr, self.y_prev, self.y_curr) {
            // Check if the intervals (x_p.ts, x_c.ts] and (y_p.ts, y_c.ts] overlap
            let max_start = x_p.ts.max(y_p.ts);
            let min_end = x_c.ts.min(y_c.ts);
            
            if min_end > max_start {
                let dx = x_c.price - x_p.price;
                let dy = y_c.price - y_p.price;
                self.cov += dx * dy;
                self.count += 1;
                
                // Clear the older intervals to avoid double counting overlaps in streaming approximation.
                // In a strict HY implementation, we evaluate all overlapping pairs.
                if x_c.ts < y_c.ts {
                    self.x_prev = None;
                } else {
                    self.y_prev = None;
                }
            }
        }
    }

    /// Returns the accumulated covariance.
    #[inline]
    pub fn covariance(&self) -> Result<f64, ArgusError> {
        if self.count < 1 {
            return Err(ArgusError::InsufficientData { needed: 1, have: self.count });
        }
        Ok(self.cov)
    }
}

/// Recursive Least Squares (RLS) factor loading estimator.
///
/// Uses the Sherman-Morrison formula to update the inverse covariance matrix in O(K^2).
#[derive(Debug, Clone)]
pub struct RecursiveLeastSquares<const K: usize> {
    lambda: f64,
    p_inv: [[f64; K]; K],
    beta: [f64; K],
    count: u64,
}

impl<const K: usize> RecursiveLeastSquares<K> {
    /// Creates a new RLS estimator.
    ///
    /// # Errors
    /// Returns `ArgusError::InvalidParameter` if `lambda` is invalid.
    pub fn new(lambda: ForgettingFactor, delta: f64) -> Self {
        let mut p_inv = [[0.0; K]; K];
        for i in 0..K {
            p_inv[i][i] = 1.0 / delta;
        }
        
        Self {
            lambda: lambda.value(),
            p_inv,
            beta: [0.0; K],
            count: 0,
        }
    }

    /// Updates the RLS estimator with a new observation `y` and feature vector `x`.
    #[inline]
    pub fn update(&mut self, x: &[f64; K], y: f64) {
        self.count += 1;
        
        // Compute P * x
        let mut px = [0.0; K];
        for i in 0..K {
            for j in 0..K {
                px[i] += self.p_inv[i][j] * x[j];
            }
        }
        
        // Compute x^T * P * x
        let mut xt_px = 0.0;
        for i in 0..K {
            xt_px += x[i] * px[i];
        }
        
        let denom = self.lambda + xt_px;
        
        // Update P matrix (Sherman-Morrison)
        let mut new_p = [[0.0; K]; K];
        for i in 0..K {
            for j in 0..K {
                new_p[i][j] = (self.p_inv[i][j] - (px[i] * px[j]) / denom) / self.lambda;
            }
        }
        self.p_inv = new_p;
        
        // Compute prediction error
        let mut y_pred = 0.0;
        for i in 0..K {
            y_pred += self.beta[i] * x[i];
        }
        let error = y - y_pred;
        
        // Update beta
        for i in 0..K {
            let mut k_gain = 0.0;
            for j in 0..K {
                k_gain += self.p_inv[i][j] * x[j];
            }
            self.beta[i] += k_gain * error;
        }
    }

    /// Returns the estimated factor loadings (betas).
    ///
    /// # Errors
    /// Returns `ArgusError::InsufficientData` if not enough observations.
    #[inline]
    pub fn beta(&self) -> Result<[f64; K], ArgusError> {
        if self.count < K as u64 {
            return Err(ArgusError::InsufficientData { needed: K as u64, have: self.count });
        }
        Ok(self.beta)
    }
}
