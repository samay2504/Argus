//! Argus Ingest — Data ingestion adapters and shard runtime.
//!
//! This crate handles:
//! - Data adapter trait and implementations (equities via `yfinance-rs`)
//! - Canonical Arrow schema definition and validation
//! - Arena allocation for zero-alloc tick processing (Phase 3)
//! - SPSC ring buffers for cross-shard communication (Phase 3)
//! - Shard-per-core runtime (Phase 3)

pub mod adapter;
pub mod arena;
pub mod equity;
pub mod runtime;
pub mod schema;

pub use adapter::DataAdapter;
