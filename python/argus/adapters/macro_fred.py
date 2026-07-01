"""FRED-based macro series data adapter.

Uses direct requests-based REST calls to avoid taking a heavy
or unmaintained Python dependency.
"""

from __future__ import annotations

import datetime
import os
import time
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

import pyarrow as pa
import requests

from argus.registry import AssetRegistry
from argus.schema import SCHEMA_VERSION, canonical_schema, validate_batch

if TYPE_CHECKING:
    pass

__all__ = ["MacroAdapter"]


@dataclass(slots=True, frozen=True)
class MacroAdapter:
    """Macro data adapter using FRED REST API."""
    
    series_ids: tuple[str, ...] = ("DFF", "T10Y2Y", "VIXCLS")
    api_key: str = field(default_factory=lambda: os.environ.get("FRED_API_KEY", ""))
    registry: AssetRegistry = field(default_factory=AssetRegistry)
    poll_interval_s: float = 60.0
    
    _state: _MacroState = field(default_factory=lambda: _MacroState(), repr=False, init=False)

    def poll(self) -> pa.RecordBatch | None:
        """Poll for the next batch of normalized tick data."""
        if not self.api_key:
            return None
            
        now = time.monotonic()
        if now - self._state.last_poll_time < self.poll_interval_s:
            return None
            
        self._state.last_poll_time = now
        
        asset_ids = []
        timestamp_ns = []
        prices = []
        volumes = []
        
        base_url = "https://api.stlouisfed.org/fred/series/observations"
        
        for series_id in self.series_ids:
            try:
                params = {
                    "series_id": series_id,
                    "api_key": self.api_key,
                    "file_type": "json",
                    "sort_order": "desc",
                    "limit": 1,
                }
                resp = requests.get(base_url, params=params, timeout=5)
                resp.raise_for_status()
                data = resp.json()
                
                observations = data.get("observations", [])
                if not observations:
                    continue
                    
                latest = observations[0]
                val_str = latest.get("value", ".")
                if val_str == ".":
                    continue
                    
                date_obj = datetime.datetime.strptime(latest["date"], "%Y-%m-%d")
                ts_ns = int(date_obj.replace(tzinfo=datetime.timezone.utc).timestamp()) * 1_000_000_000
                
                asset_ids.append(self.registry.resolve(series_id))
                timestamp_ns.append(ts_ns)
                prices.append(float(val_str))
                volumes.append(None)
            except Exception:
                continue
                
        if not asset_ids:
            return None
            
        source_str = "fred"
        source_dict = pa.array([source_str], type=pa.string())
        source_indices = pa.array([0] * len(asset_ids), type=pa.int8())
        source_arr = pa.DictionaryArray.from_arrays(source_indices, source_dict)
        
        arrays = [
            pa.array(asset_ids, type=pa.uint32()),
            pa.array(timestamp_ns, type=pa.int64()),
            pa.array(prices, type=pa.float64()),
            pa.array(volumes, type=pa.float64()),
            source_arr,
            pa.array([SCHEMA_VERSION] * len(asset_ids), type=pa.uint16())
        ]
        
        batch = pa.RecordBatch.from_arrays(arrays, schema=canonical_schema())
        validate_batch(batch)
        return batch


class _MacroState:
    def __init__(self) -> None:
        self.last_poll_time: float = 0.0
