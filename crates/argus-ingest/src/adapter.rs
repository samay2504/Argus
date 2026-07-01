//! Data adapter trait for normalizing vendor-specific data into the
//! canonical Argus Arrow schema.

use arrow::record_batch::RecordBatch;

use argus_core::ArgusError;

/// Trait for data source adapters.
///
/// Implementations normalize vendor-specific data into the canonical
/// Arrow schema (see `crate::schema`) before returning it. Each adapter
/// is interchangeable — a bank or hedge fund can swap in a Bloomberg
/// or Refinitiv adapter without touching `argus-core`.
///
/// # Contract
/// - Every returned `RecordBatch` must conform to the canonical schema
///   (correct fields, correct `schema_version`)
/// - `poll()` must never block indefinitely; return `Ok(None)` when idle
/// - Adapters are `Send` so they can be moved to shard threads
pub trait DataAdapter: Send {
    /// Poll for the next batch of normalized tick data.
    ///
    /// Returns `Ok(Some(batch))` when new data is available,
    /// `Ok(None)` when there is no new data to process,
    /// or `Err(ArgusError)` on failure.
    fn poll(&mut self) -> Result<Option<RecordBatch>, ArgusError>;
}
