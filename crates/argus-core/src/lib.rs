//! Argus Core — Online risk & factor exposure math engine.
//!
//! This crate contains the numerical core of the Argus system:
//! - Welford's online variance estimator
//! - Hayashi-Yoshida asynchronous covariance estimator
//! - Recursive Least Squares (RLS) factor loading estimator
//!
//! # Design Invariants
//! - No heap allocation in the per-tick update path after warm-up
//! - Every public function returns `Result<T, ArgusError>`
//! - Every numerical algorithm cites its source paper in a doc comment

pub mod covariance;
pub mod error;
pub mod math;
pub mod risk_policy;
pub mod types;

pub use error::ArgusError;
pub use types::{AssetId, BasisPoints, ForgettingFactor, TimestampNs};
