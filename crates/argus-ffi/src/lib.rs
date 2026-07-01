//! Argus FFI — PyO3 bindings for the Argus risk engine.
//!
//! This crate provides the Python-facing API for Argus. Data crosses
//! the Rust/Python boundary exactly once per tick batch, via Apache Arrow
//! `RecordBatch` (never via JSON, never via per-field PyO3 getters/setters
//! in a hot loop).

use argus_core::error::ArgusError;
use argus_ingest::runtime::{Orchestrator, TickPayload};
use argus_ingest::schema::validate_batch;
use arrow::array::{Float64Array, Int64Array, UInt32Array};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3_arrow::PyRecordBatch;
use std::collections::HashMap;
use argus_ingest::equity::EquityAdapter;
use argus_ingest::adapter::DataAdapter;

#[pyclass(module = "argus._argus_core")]
pub struct PyExposureSnapshot {
    #[pyo3(get)]
    pub asset_ids: Vec<u32>,
    #[pyo3(get)]
    pub variances: Vec<f64>,
    #[pyo3(get)]
    pub covariance_matrix: Vec<Vec<f64>>,
    #[pyo3(get)]
    pub factor_betas: Vec<Vec<f64>>,
}

/// The central risk engine orchestrator wrapped for Python.
#[pyclass(unsendable, module = "argus._argus_core")]
pub struct PyArgusEngine {
    orchestrator: Orchestrator,
}

#[pymethods]
impl PyArgusEngine {
    /// Create a new Argus Engine.
    #[new]
    #[pyo3(signature = (num_assets, num_shards=4))]
    pub fn new(num_assets: usize, num_shards: usize) -> Self {
        // Initialize the orchestrator with no internal adapters yet, 
        // as data is pushed from Python for this interface.
        let orchestrator = Orchestrator::new(vec![], num_shards, num_assets);
        Self { orchestrator }
    }

    /// Process a batch of normalized tick data.
    ///
    /// The batch must conform to the canonical Argus Arrow schema.
    pub fn process_batch(&mut self, batch: PyRecordBatch) -> PyResult<()> {
        let rb = batch.into_inner();
        
        // Validate schema
        validate_batch(&rb).map_err(|e| PyValueError::new_err(e.to_string()))?;
        
        let num_rows = rb.num_rows();
        if num_rows == 0 {
            return Ok(());
        }
        
        self.orchestrator.push_batch(&rb).map_err(|e| PyValueError::new_err(e.to_string()))?;
        
        Ok(())
    }

    pub fn snapshot(&self) -> PyResult<PyExposureSnapshot> {
        let snap = self.orchestrator.snapshot();
        Ok(PyExposureSnapshot {
            asset_ids: snap.asset_ids.into_iter().map(|id| id.as_u32()).collect(),
            variances: snap.variances,
            covariance_matrix: snap.covariance_matrix,
            factor_betas: snap.factor_betas,
        })
    }
}

/// A Python wrapper around the Rust yfinance-rs adapter.
#[pyclass(unsendable, module = "argus._argus_core")]
pub struct PyEquityAdapter {
    adapter: EquityAdapter,
}

#[pymethods]
impl PyEquityAdapter {
    #[new]
    pub fn new(registry: HashMap<String, u32>) -> PyResult<Self> {
        let adapter = EquityAdapter::new(registry)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self { adapter })
    }

    pub fn poll(&mut self) -> PyResult<Option<PyRecordBatch>> {
        let batch_opt = self.adapter.poll()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(batch_opt.map(PyRecordBatch::new))
    }
}

/// Argus: Online risk & factor exposure engine.
///
/// This module is imported in Python as `argus._argus_core`.
#[pymodule]
fn _argus_core(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyExposureSnapshot>()?;
    m.add_class::<PyArgusEngine>()?;
    m.add_class::<PyEquityAdapter>()?;
    Ok(())
}
