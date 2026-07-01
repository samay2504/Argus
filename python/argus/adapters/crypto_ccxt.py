"""CCXT-based cryptocurrency data adapter.

Uses the CCXT library to fetch OHLCV data from cryptocurrency exchanges.
This adapter stays in Python because CCXT has no Rust port with comparable
exchange coverage (100+ venues) or maintenance velocity.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

import ccxt
import pyarrow as pa

from argus.registry import AssetRegistry
from argus.schema import SCHEMA_VERSION, canonical_schema, validate_batch

if TYPE_CHECKING:
    pass

__all__ = ["CryptoAdapter"]


@dataclass(slots=True)
class CryptoAdapter:
    """Cryptocurrency data adapter using CCXT."""
    
    exchange_id: str = "binanceus"
    symbols: list[str] = field(default_factory=lambda: ["BTC/USDT", "ETH/USDT"])
    timeframe: str = "1m"
    registry: AssetRegistry = field(default_factory=AssetRegistry)
    
    _exchange: ccxt.Exchange | None = field(default=None, init=False, repr=False)

    def _get_exchange(self) -> ccxt.Exchange:
        if self._exchange is None:
            exchange_class = getattr(ccxt, self.exchange_id)
            self._exchange = exchange_class({"enableRateLimit": True})
        return self._exchange

    def poll(self) -> pa.RecordBatch | None:
        """Poll for the next batch of normalized tick data."""
        exchange = self._get_exchange()
        
        asset_ids = []
        timestamp_ns = []
        prices = []
        volumes = []
        
        for symbol in self.symbols:
            try:
                # fetch_ohlcv returns [timestamp, open, high, low, close, volume]
                if (ohlcv := exchange.fetch_ohlcv(symbol, self.timeframe, limit=1)):
                    last_candle = ohlcv[-1]
                    ts_ms = last_candle[0]
                    close = last_candle[4]
                    vol = last_candle[5]
                    
                    asset_ids.append(self.registry.resolve(symbol))
                    timestamp_ns.append(int(ts_ms) * 1_000_000)
                    prices.append(float(close))
                    volumes.append(float(vol))
            except Exception:
                continue
                
        if not asset_ids:
            return None
            
        source_str = f"ccxt:{self.exchange_id}"
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
