# Contributing to Argus

Welcome to the Argus development team. 

Argus is an online risk & factor exposure engine designed to support sub-millisecond per-tick overhead. We achieve this by splitting responsibilities across two languages: Python and Rust.

## Architecture Guidelines

### The Dual-Language Architecture

The system is split into two halves:
1. **Python Orchestration & Ingestion (`argus` package)**:
   - Python is used to fetch data from outside sources (Yahoo Finance, CCXT for crypto, FRED for macro).
   - Python defines the canonical Arrow schema.
   - Python batches ticks into Arrow `RecordBatch`es.
2. **Rust Core Risk Engine (`argus-core`, `argus-ingest`, `argus-ffi`)**:
   - The Rust engine takes over once a `RecordBatch` crosses the FFI boundary.
   - The Rust side is strictly **zero-allocation** in the hot path. All allocations occur ahead of time (via `TickArena`) or during the initialization phase.
   - We use SPSC ring buffers and background Shard threads to process ticks in parallel, lock-free.

### The FFI Boundary

- Data crosses the Rust/Python boundary **exactly once** per batch.
- We use Apache Arrow (`pyo3-arrow`) to pass zero-copy RecordBatches across the boundary.
- **Never** pass data point-by-point.
- **Never** serialize to JSON across the boundary.

### Rust Guidelines

- **Zero Allocation**: Avoid `Vec::new()`, `Box::new()`, `String::new()`, and `Mutex` in the hot path (per-tick processing). Use `TickArena` or static arrays where possible.
- **Errors**: All public functions should return `Result<T, ArgusError>`. Do not `unwrap()` or `panic!()` outside of tests.
- **Types**: Use semantic domain newtypes like `AssetId`, `TimestampNs`, `BasisPoints`, and `ForgettingFactor`.

## Building the Project

1. Activate your Python environment.
2. Build the Rust extension in development mode:
   ```bash
   pip install -e .
   ```
3. Run Python E2E tests:
   ```bash
   pytest tests/python/
   ```
4. Run Rust unit and property tests:
   ```bash
   cargo test --workspace
   ```
5. Run Benchmarks:
   ```bash
   cargo bench
   ```

## Creating a new Adapter

To create a new Data Adapter, subclass the Python `DataAdapter` protocol in `argus.adapters`. Ensure your `poll()` method returns a `pyarrow.RecordBatch` conforming to the `argus.schema.canonical_schema()`.

Thank you for contributing to Argus!
