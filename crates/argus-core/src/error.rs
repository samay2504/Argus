//! Unified error types for argus-core.

/// Unified error type for all argus-core operations.
///
/// All public functions in argus-core return `Result<T, ArgusError>`.
/// This enum is `#[non_exhaustive]` to allow adding variants in minor
/// version bumps without breaking downstream matches.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ArgusError {
    /// The Arrow schema version of an incoming batch does not match
    /// the version expected by this build of argus-core.
    #[error("schema version mismatch: expected {expected}, got {actual}")]
    SchemaVersionMismatch {
        /// The schema version this build expects.
        expected: u16,
        /// The schema version found in the incoming batch.
        actual: u16,
    },

    /// A parameter value is outside its valid domain.
    #[error("invalid parameter `{name}`: {reason}")]
    InvalidParameter {
        /// Name of the invalid parameter.
        name: &'static str,
        /// Human-readable explanation of why the value is invalid.
        reason: String,
    },

    /// An operation requires more observations than are currently available.
    #[error("insufficient data: need at least {needed} observations, have {have}")]
    InsufficientData {
        /// Minimum number of observations required.
        needed: u64,
        /// Number of observations currently available.
        have: u64,
    },

    /// An error propagated from the Apache Arrow library.
    #[error("arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// An error during data ingestion from an external source.
    #[error("data ingestion error from {source_name}: {detail}")]
    DataIngestion {
        /// The data source that produced the error (e.g., "yfinance", "ccxt:binance").
        source_name: String,
        /// Human-readable detail about what went wrong.
        detail: String,
    },

    /// The schema structure does not match the canonical Argus schema.
    #[error("invalid schema: {0}")]
    InvalidSchema(String),
}
