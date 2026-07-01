# Argus Roadmap

This document lists features and improvements that are explicitly
**deferred** from v1. These are not oversights — they are deliberate
scoping decisions. Each item was considered during design and explicitly
rejected from the current build scope.

## Deferred to v2+

### Risk Policy — Full RL Hedging
- **What:** A CVaR-constrained, learned risk policy per the SIAM FME
  risk-aware RL framing (Coache & Jaimungal, 2024).
- **Why deferred:** Training an RL hedging policy is a multi-month
  research project. v1 ships the `RiskPolicy` trait and a static
  limit-check implementation only.
- **Interface ready:** Yes — `RiskPolicy` trait defined in `argus-core`.

### Crypto Adapter — Native Rust (ccxt-rust)
- **What:** Migrate the crypto adapter from Python CCXT to a native
  Rust implementation.
- **Why deferred:** As of July 2026, no Rust CCXT port supports >20
  exchanges. `ccxt-rust` exists but covers only ~5 exchanges.
- **Monitor:** Check `ccxt-rust` crate maturity quarterly.

### SIMD / AVX-512 Matrix Operations
- **What:** Vectorized matrix operations for covariance updates.
- **Why deferred:** The current scalar implementation meets performance
  targets. SIMD optimization is premature until profiling shows matrix
  math as the bottleneck.

### Additional Data Adapters
- **Bloomberg Terminal adapter**
- **Refinitiv/LSEG adapter**
- **Databento adapter**
- **Interactive Brokers market data adapter**
- **Why deferred:** These require proprietary licenses/SDKs. The
  `DataAdapter` trait is designed for drop-in replacements.

### GPU-Accelerated Covariance
- **What:** CUDA/ROCm-accelerated large-scale covariance computation.
- **Why deferred:** GPU offload has fixed overhead that only pays off
  at very large universe sizes (>10,000 assets).

### Distributed Shard Runtime
- **What:** Multi-node sharding for very large universes.
- **Why deferred:** Single-node shard-per-core is sufficient for the
  target universe sizes in v1.

### WebSocket Streaming Adapters
- **What:** Real-time tick-by-tick data via exchange WebSocket feeds.
- **Why deferred:** v1 uses poll-based OHLCV fetching. WebSocket
  integration adds connection management complexity.

### Multivariate Realized Kernel (MRK)
- **What:** Noise-robust covariance estimator that handles microstructure
  noise better than raw Hayashi-Yoshida.
- **Why deferred:** The HY estimator is sufficient for the sampling
  frequencies used in v1 (minute-level, not tick-level).

### Pre-Averaging for Microstructure Noise
- **What:** Pre-averaging technique for sub-second tick data.
- **Why deferred:** v1 targets minute-level and daily data, not
  sub-second microstructure.

## Phase Completion Status

- [x] Phase 0 — Workspace skeleton
- [x] Phase 1 — Data adapters & canonical schema
- [x] Phase 2 — Math core (Welford, HY, RLS)
- [x] Phase 3 — Shard runtime, ring buffers
- [x] Phase 4 — Python/Rust FFI boundary
- [x] Phase 5 — Documentation & OSS packaging
