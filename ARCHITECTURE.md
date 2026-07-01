# Argus Architecture

This document describes the technical architecture of Argus, including
design rationale, algorithm choices, and benchmark results.

## System Overview

Argus is a sub-millisecond-class, online (streaming, single-pass) risk and
factor exposure engine. It computes incremental covariance/correlation matrices
and recursive factor betas across equities, crypto, and macro conditioning
variables.

The system enforces a hard separation between:
- **Rust hot path** — all numerical computation, shard runtime, ring buffers
- **Python orchestration** — data adapter I/O, client API, configuration

## Data Flow

```mermaid
graph TD
    subgraph "Python Orchestration Layer"
        YF["yfinance-rs<br/>(Rust, async)"]
        CCXT["CCXT<br/>(Python)"]
        FRED["FRED REST<br/>(Python)"]
        REG["Asset Registry"]
    end

    subgraph "Arrow FFI Boundary"
        RB["Arrow RecordBatch<br/>(zero-copy via PyCapsule)"]
    end

    subgraph "Rust Engine Core"
        VAL["Schema Validator"]
        ROUTE["Shard Router<br/>(asset_id % num_shards)"]

        subgraph "Shard 0 (pinned thread)"
            W0["Welford<br/>Variance"]
            HY0["Hayashi-Yoshida<br/>Covariance"]
            RLS0["RLS<br/>Factor Betas"]
            A0["Tick Arena"]
        end

        subgraph "Shard 1 (pinned thread)"
            W1["Welford"]
            HY1["HY Cov"]
            RLS1["RLS"]
            A1["Arena"]
        end

        RING["SPSC Ring Buffer<br/>(factor broadcast)"]
        SNAP["ExposureSnapshot"]
    end

    YF --> RB
    CCXT --> RB
    FRED --> RB
    RB --> VAL --> ROUTE
    ROUTE --> W0
    ROUTE --> W1
    RING --> RLS0
    RING --> RLS1
    W0 --> SNAP
    HY0 --> SNAP
    RLS0 --> SNAP
    W1 --> SNAP
    HY1 --> SNAP
    RLS1 --> SNAP
```

## Design Principles

### 1. Math in Rust, I/O in Python

No statistical computation is ever performed in Python on the production path.
Python handles:
- Data adapter I/O (network calls to Yahoo Finance, CCXT, FRED)
- Configuration and orchestration
- Result presentation

Rust handles:
- All numerical estimators (Welford, HY, RLS)
- Shard runtime and tick dispatch
- Memory management (arenas, ring buffers)

### 2. Single Arrow Boundary Crossing

Data crosses the Rust/Python FFI boundary exactly once per batch as an
Arrow `RecordBatch`, amortizing marshalling cost. We use `pyo3-arrow`'s
PyCapsule interface for zero-copy transfer — no serialization, no memcpy.

### 3. Shard-Per-Core, Shared-Nothing

Each compute shard:
- Runs on a dedicated OS thread (not a tokio task — we want pinned cores)
- Owns a disjoint partition of the asset universe
- Has its own Welford, HY, and RLS state
- Communicates only via lock-free SPSC ring buffers

**No `Mutex`, `RwLock`, or `Arc<Mutex<T>>` in the hot path.**

This follows the LMAX Disruptor pattern: mechanical sympathy with predictable
latency and linear scaling with core count.

### 4. No Heap Allocation After Warm-up

All buffers are:
- Arena-allocated at shard spawn time (single `Vec<u8>` allocation, bump pointer)
- Fixed-capacity (`ArrayVec`, const-generic arrays)
- Pre-allocated to partition size

The `dhat-rs` integration test proves zero post-warmup allocations.

## Algorithm Choices

### §5.1 Welford's Online Variance

The naive formula `E[X²] - E[X]²` suffers from catastrophic cancellation
when the mean is large relative to the variance. Welford's single-pass
algorithm is numerically stable and O(1) per tick.

**Recurrence:**
$$
\delta = x_t - \mu_{t-1} \\
\mu_t = \mu_{t-1} + \delta / t \\
M2_t = M2_{t-1} + \delta \cdot (x_t - \mu_t) \\
\sigma^2_t = M2_t / (t - 1)
$$

State: 24 bytes (count, mean, M2). Fully stack-allocatable.

**Reference:** B.P. Welford (1962), "Note on a Method for Calculating
Corrected Sums of Squares and Products," *Technometrics*, 4(3), 419–420.

### §5.2 Hayashi-Yoshida Asynchronous Covariance

Plain pairwise covariance assumes synchronous sampling. In a multi-asset,
multi-venue universe (equities close at 4pm ET, crypto trades 24/7,
FRED updates daily), naive covariance is downward-biased (the "Epps effect").

The Hayashi-Yoshida estimator sums return products over *overlapping*
intervals only:

$$
\hat{C}_{HY} = \sum_{i,j} \Delta X_i \cdot \Delta Y_j \cdot \mathbb{1}\{I_i \cap I_j \neq \emptyset\}
$$

where $I_i = (t_{i-1}^X, t_i^X]$ and $I_j = (t_{j-1}^Y, t_j^Y]$.

Two intervals overlap iff $\max(\text{start}_i, \text{start}_j) < \min(\text{end}_i, \text{end}_j)$.

**Online adaptation:** Rather than buffering full history, we maintain bounded
pending-interval queues per asset side (default depth 64, using `ArrayVec`).
When a new tick arrives, we scan the other side's queue for overlaps, accumulate
products, and purge expired entries. This is amortized O(1) per tick.

**Reference:** T. Hayashi & N. Yoshida (2005), "On covariance estimation
of non-synchronously observed diffusion processes," *Bernoulli*, 11(2),
359–379.

### §5.3 Recursive Least Squares (RLS)

Batch OLS requires recomputing β from a stored window — O(n) per update,
plus memory for the full window. RLS updates β in O(k²) per tick (where
k = number of factors), with no history buffering.

**Recurrence:**
$$
e_t = r_t - \beta_{t-1}^T f_t \quad \text{(prediction error)} \\
K_t = P_{t-1} f_t / (\lambda + f_t^T P_{t-1} f_t) \quad \text{(gain vector)} \\
\beta_t = \beta_{t-1} + K_t \cdot e_t \quad \text{(coefficient update)} \\
P_t = (P_{t-1} - K_t f_t^T P_{t-1}) / \lambda \quad \text{(inverse cov update)}
$$

λ ∈ (0, 1] is the forgetting factor. λ = 1 recovers ordinary recursive OLS.
The effective window length is ≈ 1/(1-λ) observations.

For K = 5 factors, P matrix is 200 bytes. For K = 10, 800 bytes. All stack-allocated
via const generics.

**Reference:** S. Haykin (2002), *Adaptive Filter Theory*, 4th ed.,
Prentice Hall, Chapter 13.

### §5.4 Risk Policy Seam

v1 ships exactly one `RiskPolicy` implementation: `StaticLimitPolicy`.
This checks hard thresholds on:
- Maximum single-asset variance
- Maximum pairwise correlation
- Maximum factor beta magnitude

The `RiskPolicy` trait is the extensibility seam for v2's CVaR-constrained
RL hedging policy (see ROADMAP.md).

## Canonical Schema

All data crosses the adapter→engine boundary as Arrow `RecordBatch` with
this schema:

| Field | Type | Nullable | Description |
|-------|------|----------|-------------|
| `asset_id` | `uint32` | No | Resolved asset identifier |
| `timestamp_ns` | `int64` | No | UTC nanosecond epoch |
| `price` | `float64` | No | Price/value |
| `volume` | `float64` | Yes | Trading volume (null for macro) |
| `source` | `dictionary<int8, utf8>` | No | Data source identifier |
| `schema_version` | `uint16` | No | Schema version (currently 1) |

Schema-level metadata: `{"argus_schema_version": "1"}`

The schema is defined once in Python (`argus.schema.canonical_schema()`) and
mirrored in Rust (`argus_ingest::schema::canonical_schema()`). Both sides
validate incoming batches and reject mismatches loudly.

## Crate Dependency Graph

```mermaid
graph TD
    A["argus-core<br/>(error, types, math,<br/>covariance, risk_policy)"]
    B["argus-ingest<br/>(adapters, schema,<br/>runtime, arena)"]
    C["argus-ffi<br/>(PyO3, Arrow FFI,<br/>PyArgusEngine)"]
    D["argus-cli<br/>(standalone binary)"]
    E["python/argus<br/>(client, adapters,<br/>schema, registry)"]

    A --> B
    A --> C
    B --> C
    A --> D
    B --> D
    C --> E
```

## Benchmark Results

*Benchmark numbers are recorded after running `cargo bench` on the target machine.*

| Benchmark | ops/sec | Latency (p99) | Hardware |
|---|---|---|---|
| Welford single update | TBD | TBD | TBD |
| HY pair update (32 pending) | TBD | TBD | TBD |
| RLS step (K=5) | TBD | TBD | TBD |
| RLS step (K=10) | TBD | TBD | TBD |
| 1000×1000 correlation matrix update | TBD | TBD | TBD |
| SPSC ring buffer throughput | TBD | TBD | TBD |
| Shard tick processing (100 assets) | TBD | TBD | TBD |
| Arena alloc/reset cycle | TBD | TBD | TBD |

To run benchmarks:
```bash
cargo bench --bench core_benchmarks
```
