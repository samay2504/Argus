//! Domain-specific newtypes for the Argus system.
//!
//! These types enforce semantic correctness at the type level,
//! preventing accidental misuse of raw numeric types across
//! function boundaries.

use crate::error::ArgusError;

/// A resolved asset identifier, assigned by the asset registry.
///
/// In the hot path, assets are identified by this integer ID rather
/// than string tickers, avoiding allocation and comparison costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(pub u32);

impl AssetId {
    /// Returns the underlying `u32` value.
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Asset({})", self.0)
    }
}

/// A value expressed in basis points (1 bp = 0.01%).
///
/// Used for risk limits and threshold specifications.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BasisPoints(pub f64);

impl BasisPoints {
    /// Converts basis points to a decimal fraction.
    /// E.g., 100 bp → 0.01.
    #[inline]
    pub fn as_decimal(self) -> f64 {
        self.0 / 10_000.0
    }
}

impl std::fmt::Display for BasisPoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}bp", self.0)
    }
}

/// The exponential forgetting factor λ for Recursive Least Squares.
///
/// Must satisfy λ ∈ (0, 1]. A value of 1.0 recovers ordinary recursive
/// OLS with no forgetting. Smaller values weight recent observations
/// more heavily; the effective window length is approximately 1/(1−λ).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ForgettingFactor(f64);

impl ForgettingFactor {
    /// Creates a new `ForgettingFactor`, validating that λ ∈ (0, 1].
    ///
    /// # Errors
    /// Returns `ArgusError::InvalidParameter` if `lambda` is not in (0, 1].
    pub fn new(lambda: f64) -> Result<Self, ArgusError> {
        if lambda > 0.0 && lambda <= 1.0 {
            Ok(Self(lambda))
        } else {
            Err(ArgusError::InvalidParameter {
                name: "lambda",
                reason: format!("forgetting factor must be in (0, 1], got {lambda}"),
            })
        }
    }

    /// Returns the underlying `f64` value.
    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// Returns the approximate effective window length: 1/(1−λ).
    /// Returns `f64::INFINITY` when λ = 1.0 (no forgetting).
    #[inline]
    pub fn effective_window(self) -> f64 {
        if (self.0 - 1.0).abs() < f64::EPSILON {
            f64::INFINITY
        } else {
            1.0 / (1.0 - self.0)
        }
    }
}

impl std::fmt::Display for ForgettingFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "λ={:.4}", self.0)
    }
}

/// A UTC timestamp in nanoseconds since the Unix epoch.
///
/// Required for the Hayashi-Yoshida overlap estimator to compute
/// meaningful interval overlaps between asynchronously sampled series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimestampNs(pub i64);

impl TimestampNs {
    /// Returns the underlying `i64` nanosecond epoch value.
    #[inline]
    pub const fn as_nanos(self) -> i64 {
        self.0
    }
}

impl std::fmt::Display for TimestampNs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ns", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forgetting_factor_valid_range() {
        assert!(ForgettingFactor::new(0.5).is_ok());
        assert!(ForgettingFactor::new(1.0).is_ok());
        assert!(ForgettingFactor::new(0.99).is_ok());
    }

    #[test]
    fn forgetting_factor_invalid_range() {
        assert!(ForgettingFactor::new(0.0).is_err());
        assert!(ForgettingFactor::new(-0.1).is_err());
        assert!(ForgettingFactor::new(1.1).is_err());
        assert!(ForgettingFactor::new(f64::NAN).is_err());
    }

    #[test]
    fn forgetting_factor_effective_window() {
        let ff = ForgettingFactor::new(0.99).unwrap();
        assert!((ff.effective_window() - 100.0).abs() < 1.0);

        let ff_full = ForgettingFactor::new(1.0).unwrap();
        assert!(ff_full.effective_window().is_infinite());
    }

    #[test]
    fn basis_points_conversion() {
        let bp = BasisPoints(100.0);
        assert!((bp.as_decimal() - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn asset_id_display() {
        let id = AssetId(42);
        assert_eq!(format!("{id}"), "Asset(42)");
    }
}
