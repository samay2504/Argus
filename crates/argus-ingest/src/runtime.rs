//! Shard-per-core runtime and tick pipeline.
//!
//! Handles distribution of ticks to lock-free shard threads via SPSC ring buffers,
//! and evaluates risk limits locally within the shard.

use crate::adapter::DataAdapter;
use crate::arena::TickArena;
use argus_core::covariance::CovarianceManager;
use argus_core::error::ArgusError;
use argus_core::risk_policy::{ExposureSnapshot, RiskPolicy, StaticLimitPolicy};
use argus_core::types::{AssetId, TimestampNs};
use arrow::array::Array;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{storage::Heap, HeapRb, SharedRb};
use std::sync::{Arc, Mutex};
use std::thread;

pub type TickProducer = ringbuf::wrap::caching::Caching<Arc<SharedRb<Heap<TickPayload>>>, true, false>;
pub type TickConsumer = ringbuf::wrap::caching::Caching<Arc<SharedRb<Heap<TickPayload>>>, false, true>;

/// The tick payload sent from the orchestrator to a shard.
#[derive(Debug, Clone, Copy)]
pub struct TickPayload {
    pub asset_id: u32,
    pub timestamp_ns: i64,
    pub price: f64,
    pub volume: Option<f64>,
}

/// A worker shard that processes ticks in a single thread without locks.
pub struct Shard {
    id: usize,
    rx: TickConsumer,
    num_assets: usize,
    snapshot_tx: Arc<Mutex<ExposureSnapshot>>,
}

impl Shard {
    /// Creates a new shard.
    pub fn new(id: usize, rx: TickConsumer, num_assets: usize, snapshot_tx: Arc<Mutex<ExposureSnapshot>>) -> Self {
        Self { id, rx, num_assets, snapshot_tx }
    }

    /// Runs the shard event loop.
    pub fn run(mut self) {
        let mut cov_mgr = CovarianceManager::new(self.num_assets);
        let _policy = StaticLimitPolicy::new(10.0, 0.99, 5.0);
        let mut arena = TickArena::new(1024 * 1024); // 1MB arena
        let mut ticks_since_snapshot = 0;

        loop {
            arena.reset();
            let mut processed = 0;

            while let Some(tick) = self.rx.try_pop() {
                processed += 1;
                let _ = cov_mgr.update_tick(
                    tick.asset_id as usize,
                    TimestampNs(tick.timestamp_ns),
                    tick.price,
                );
                
                let _snapshot = ExposureSnapshot {
                    asset_ids: vec![AssetId(tick.asset_id)],
                    variances: vec![cov_mgr.variance(tick.asset_id as usize).unwrap_or(0.0)],
                    covariance_matrix: vec![vec![0.0]],
                    factor_betas: vec![vec![0.0; 5]],
                };
                let _actions = _policy.evaluate(&_snapshot);
            }
            
            if processed > 0 {
                ticks_since_snapshot += processed;
                // Only update the shared snapshot occasionally to avoid lock contention
                if ticks_since_snapshot >= 1 {
                    let mut asset_ids = Vec::new();
                    let mut variances = Vec::new();
                    // We just export the assets this shard owns
                    for i in 0..self.num_assets {
                        if i % 4 == self.id { // Assume 4 shards for modulo, or just export all
                            if let Ok(v) = cov_mgr.variance(i) {
                                asset_ids.push(AssetId(i as u32));
                                variances.push(v);
                            }
                        }
                    }
                    
                    if let Ok(mut snap) = self.snapshot_tx.try_lock() {
                        snap.asset_ids = asset_ids;
                        snap.variances = variances;
                        ticks_since_snapshot = 0;
                    }
                }
            }
            std::thread::yield_now();
        }
    }
}

/// Orchestrates data ingestion and distributes ticks to shards.
pub struct Orchestrator {
    adapters: Vec<Box<dyn DataAdapter>>,
    txs: Vec<TickProducer>,
    num_assets: usize,
    tick_count: u64,
    snapshots: Vec<Arc<Mutex<ExposureSnapshot>>>,
}

impl Orchestrator {
    /// Creates a new orchestrator, initializing ring buffers but not spawning threads.
    pub fn new(adapters: Vec<Box<dyn DataAdapter>>, num_shards: usize, num_assets: usize) -> Self {
        let mut txs = Vec::with_capacity(num_shards);
        let mut snapshots = Vec::with_capacity(num_shards);

        for i in 0..num_shards {
            let rb = HeapRb::<TickPayload>::new(1024 * 64);
            let (tx, rx) = rb.split();
            txs.push(tx);

            let snap = Arc::new(Mutex::new(ExposureSnapshot {
                asset_ids: vec![],
                variances: vec![],
                covariance_matrix: vec![],
                factor_betas: vec![],
            }));
            snapshots.push(snap.clone());

            let shard = Shard::new(i, rx, num_assets, snap);
            thread::Builder::new()
                .name(format!("argus-shard-{}", i))
                .spawn(move || {
                    shard.run();
                })
                .expect("Failed to spawn shard thread");
        }

        Self {
            adapters,
            txs,
            num_assets,
            tick_count: 0,
            snapshots,
        }
    }

    pub fn snapshot(&self) -> ExposureSnapshot {
        let mut all_ids = Vec::new();
        let mut all_vars = Vec::new();
        
        for snap_mtx in &self.snapshots {
            if let Ok(snap) = snap_mtx.lock() {
                all_ids.extend(&snap.asset_ids);
                all_vars.extend(&snap.variances);
            }
        }
        
        ExposureSnapshot {
            asset_ids: all_ids,
            variances: all_vars,
            covariance_matrix: vec![],
            factor_betas: vec![],
        }
    }

    /// Pushes a record batch directly into the orchestrator.
    pub fn push_batch(&mut self, batch: &arrow::record_batch::RecordBatch) -> Result<(), ArgusError> {
        let asset_ids = batch.column(0).as_any().downcast_ref::<arrow::array::UInt32Array>().unwrap();
        let timestamps = batch.column(1).as_any().downcast_ref::<arrow::array::Int64Array>().unwrap();
        let prices = batch.column(2).as_any().downcast_ref::<arrow::array::Float64Array>().unwrap();
        let volumes = batch.column(3).as_any().downcast_ref::<arrow::array::Float64Array>().unwrap();

        for i in 0..batch.num_rows() {
            let tick = TickPayload {
                asset_id: asset_ids.value(i),
                timestamp_ns: timestamps.value(i),
                price: prices.value(i),
                volume: if volumes.is_null(i) { None } else { Some(volumes.value(i)) },
            };

            let shard_idx = (tick.asset_id as usize) % self.txs.len();
            let _ = self.txs[shard_idx].try_push(tick);
            self.tick_count += 1;
        }
        Ok(())
    }

    /// Processes available data from all internal adapters.
    pub fn poll_all(&mut self) -> Result<(), ArgusError> {
        let mut batches = Vec::new();
        for adapter in &mut self.adapters {
            if let Some(batch) = adapter.poll()? {
                batches.push(batch);
            }
        }
        for batch in batches {
            self.push_batch(&batch)?;
        }
        Ok(())
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn num_adapters(&self) -> usize {
        self.adapters.len()
    }
}
