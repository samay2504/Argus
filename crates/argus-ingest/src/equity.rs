//! Equity data adapter using yfinance-rs.

use crate::adapter::DataAdapter;
use crate::schema::{canonical_schema, validate_batch, SCHEMA_VERSION};
use argus_core::error::ArgusError;
use argus_core::types::AssetId;
use arrow::array::{DictionaryArray, Float64Array, Int64Array, Int8Array, StringArray, UInt16Array, UInt32Array};
use arrow::datatypes::Int8Type;
use arrow::record_batch::RecordBatch;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use yfinance_rs::{Ticker, YfClient};

pub struct EquityAdapter {
    symbols: Vec<String>,
    registry: HashMap<String, AssetId>,
    client: YfClient,
    rt: Runtime,
}

impl EquityAdapter {
    pub fn new(registry_map: HashMap<String, u32>) -> Result<Self, ArgusError> {
        let mut registry = HashMap::new();
        let mut symbols = Vec::new();
        for (sym, id) in registry_map {
            registry.insert(sym.clone(), AssetId(id));
            symbols.push(sym);
        }
        
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ArgusError::DataIngestion {
                source_name: "yfinance".into(),
                detail: format!("failed to create tokio runtime: {e}"),
            })?;
            
        let client = YfClient::default();
            
        Ok(Self {
            symbols,
            registry,
            client,
            rt,
        })
    }
}

impl DataAdapter for EquityAdapter {
    fn poll(&mut self) -> Result<Option<RecordBatch>, ArgusError> {
        let mut asset_ids = Vec::with_capacity(self.symbols.len());
        let mut timestamp_ns = Vec::with_capacity(self.symbols.len());
        let mut prices = Vec::with_capacity(self.symbols.len());
        let mut volumes = Vec::with_capacity(self.symbols.len());
        
        for sym in &self.symbols {
            let asset_id = *self.registry.get(sym).unwrap();
            let ticker = Ticker::new(&self.client, sym);
            
            let quote_res = self.rt.block_on(async { ticker.quote().await });
            if let Ok(quote) = quote_res {
                let price = quote.price.map(|p| format!("{}", p).parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0);
                let volume = quote.day_volume.map(|v| format!("{}", v).parse::<f64>().unwrap_or(0.0));
                let ts = quote.as_of.map(|ts| ts.timestamp_nanos_opt().unwrap_or(0)).unwrap_or(0);
                
                asset_ids.push(asset_id.as_u32());
                timestamp_ns.push(ts);
                prices.push(price);
                volumes.push(volume);
            }
        }
        
        if asset_ids.is_empty() {
            return Ok(None);
        }
        
        let mut source_keys = Vec::with_capacity(asset_ids.len());
        for _ in 0..asset_ids.len() {
            source_keys.push(0i8);
        }
        let dict_values = Arc::new(StringArray::from(vec!["yfinance"]));
        let source_arr = DictionaryArray::<Int8Type>::new(
            Int8Array::from(source_keys),
            dict_values,
        );
        
        let len = prices.len();
        let batch = RecordBatch::try_new(
            Arc::new(canonical_schema()),
            vec![
                Arc::new(UInt32Array::from(asset_ids)),
                Arc::new(Int64Array::from(timestamp_ns)),
                Arc::new(Float64Array::from(prices)),
                Arc::new(Float64Array::from(volumes)),
                Arc::new(source_arr),
                Arc::new(UInt16Array::from(vec![SCHEMA_VERSION; len])),
            ],
        )?;
        
        validate_batch(&batch)?;
        
        Ok(Some(batch))
    }
}
