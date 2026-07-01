"""Multi-asset universe demo — the definitive Argus example.

Demonstrates:
1. All three adapter types: equities, crypto, macro
2. ArgusClient orchestration
3. Mixed-asset covariance and factor loading estimation
"""

import time

from argus import ArgusClient
from argus.adapters.crypto_ccxt import CryptoAdapter
from argus.adapters.macro_fred import MacroAdapter
from argus.registry import AssetRegistry
from argus._argus_core import PyEquityAdapter
import pyarrow as pa


def main() -> None:
    """Run the multi-asset universe demo."""
    print("Argus — Multi-Asset Universe Demo")
    print("=" * 50)
    print()

    # Build asset registry
    registry = AssetRegistry()

    # Equities
    for sym in ["SPY", "AAPL", "MSFT"]:
        registry.resolve(sym)
    
    # Create EquityAdapter using Rust via PyO3
    raw_equity = PyEquityAdapter(registry._symbol_to_id)
    
    # Wrap it to return pyarrow.RecordBatch via Arrow PyCapsule Interface
    class EquityWrapper:
        def poll(self):
            batch = raw_equity.poll()
            if batch is not None:
                # Use Arrow PyCapsule interface to convert pyo3_arrow output to pyarrow
                return pa.record_batch(batch)
            return None

    equity = EquityWrapper()

    # Crypto
    crypto = CryptoAdapter(
        exchange_id="binanceus",
        symbols=["BTC/USDT", "ETH/USDT"],
        registry=registry,
    )

    # Macro
    macro = MacroAdapter(
        series_ids=("DFF", "T10Y2Y", "VIXCLS"),
        registry=registry,
    )

    # Create client
    client = ArgusClient.create(
        [equity, crypto, macro],
        num_assets=len(registry._symbol_to_id),
        num_shards=2,
        forgetting_factor=0.99,
    )

    print(f"Registered {len(registry._symbol_to_id)} assets across equity/crypto/macro")
    print(f"Asset universe: {list(registry._symbol_to_id.keys())}")
    print()

    # Poll loop
    print("Polling adapters...")
    for i in range(3):
        n = client.poll_and_update()
        print(f"  Iteration {i + 1}: ingested {n} ticks")
        time.sleep(0.5)

    # Snapshot
    snap = client.snapshot()
    print()
    print("Exposure Snapshot:")
    for k, v in snap.items():
        print(f"  {k}: {v}")

    print()
    print("Demo complete.")


if __name__ == "__main__":
    main()
