# Argus

Argus is an online, streaming risk and factor exposure engine designed for sub-millisecond per-tick overhead. It computes incremental covariance matrices and recursive factor betas across equities, crypto, and macro variables.

## Key Features

- **Dual-Language Architecture**: Python for orchestration and data fetching, Rust for the lock-free, zero-allocation math core.
- **Zero-Copy FFI**: Data passes from Python to Rust via Apache Arrow `RecordBatch` arrays over PyO3-Arrow, ensuring minimal serialization overhead.
- **Zero-Allocation Hot Path**: Pre-allocated bump arenas (`TickArena`) and SPSC lock-free ringbuffers enable shards to process ticks asynchronously without heap allocation or mutex contention.
- **Advanced Math Core**: Welford's online variance, Hayashi-Yoshida asynchronous covariance, and Recursive Least Squares factor estimation.

## Research Foundations
Argus implements the following state-of-the-art mathematical estimators for high-frequency streaming environments:
- **Welford (1962)**: *Note on a method for calculating corrected sums of squares and products.* (Robust one-pass variance).
- **Hayashi & Yoshida (2005)**: *On covariance estimation of non-synchronously observed diffusion processes.* (Asynchronous tick correlation).
- **Recursive Least Squares (RLS)**: Real-time adaptive filtering for factor loading estimation with forgetting factors.

## Installation

You need the Rust toolchain and Python 3.10+ installed.

```bash
pip install -e .
```

## Quick Start

The main pipeline demo (`examples/multi_asset_universe.py`) spins up the Rust orchestrator and fetches data across equities (`yfinance-rs`), crypto (`CCXT`), and macro variables (`FRED API`).

### 1. Setup Environment
To fetch macro data, you'll need a FRED API key. Create an `.env` file in the root directory:
```bash
FRED_API_KEY=your_api_key_here
```

### 2. Run the Main Pipeline
Run the multi-asset universe demo to see the engine in action:

**PowerShell:**
```powershell
Get-Content .env | foreach { $n,$v = $_.split('='); [System.Environment]::SetEnvironmentVariable($n,$v) }
python examples/multi_asset_universe.py
```

**Bash / Zsh:**
```bash
set -a; source .env; set +a
python examples/multi_asset_universe.py
```

### Manual Initialization (Code Example)

```python
from argus import ArgusClient
from argus.registry import AssetRegistry
from argus.adapters.crypto_ccxt import CryptoAdapter

registry = AssetRegistry()
registry.resolve("BTC/USDT")

crypto = CryptoAdapter(
    exchange_id="binance",
    symbols=["BTC/USDT"],
    registry=registry,
)

client = ArgusClient.create([crypto], num_assets=len(registry._symbol_to_id))
client.poll_and_update()

snapshot = client.snapshot()
print(snapshot)
```

## Developer Workflow

- **Run E2E Tests**: `pytest tests/python/`
- **Run Rust Core Tests**: `cargo test --workspace`
- **Run Math Benchmarks**: `cargo bench`

Please refer to `CONTRIBUTING.md` for architectural invariants and FFI guidelines.

---
*Built for scale. https://github.com/samay2504/Argus*
