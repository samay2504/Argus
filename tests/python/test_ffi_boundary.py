"""FFI boundary tests for the Argus engine."""

import pyarrow as pa
import pytest

from argus.schema import canonical_schema, SCHEMA_VERSION


def _make_test_batch(n_rows: int = 10) -> pa.RecordBatch:
    """Create a test batch conforming to the canonical schema."""
    return pa.RecordBatch.from_pydict(
        {
            "asset_id": pa.array(list(range(n_rows)), type=pa.uint32()),
            "timestamp_ns": pa.array(
                [1_000_000_000 * (i + 1) for i in range(n_rows)], type=pa.int64()
            ),
            "price": pa.array(
                [100.0 + i * 0.1 for i in range(n_rows)], type=pa.float64()
            ),
            "volume": pa.array(
                [1000.0 + i for i in range(n_rows)], type=pa.float64()
            ),
            "source": pa.array(
                ["test"] * n_rows,
                type=pa.dictionary(pa.int8(), pa.utf8()),
            ),
            "schema_version": pa.array(
                [SCHEMA_VERSION] * n_rows, type=pa.uint16()
            ),
        },
        schema=canonical_schema(),
    )


class TestFFIBoundary:
    """Tests for the Rust/Python FFI boundary."""

    def test_engine_import(self) -> None:
        """Test that the Rust engine module can be imported."""
        try:
            from argus._argus_core import PyArgusEngine
            engine = PyArgusEngine(0, num_shards=2)
            assert engine is not None
        except ImportError:
            pytest.skip("Rust engine not built (run maturin develop)")

    def test_process_valid_batch(self) -> None:
        """Test processing a valid batch through the FFI."""
        try:
            from argus._argus_core import PyArgusEngine
        except ImportError:
            pytest.skip("Rust engine not built")

        engine = PyArgusEngine(0, num_shards=2)
        batch = _make_test_batch(5)
        engine.process_batch(batch)

    def test_schema_version_mismatch(self) -> None:
        """Test that schema version mismatch raises."""
        try:
            from argus._argus_core import PyArgusEngine
        except ImportError:
            pytest.skip("Rust engine not built")

        engine = PyArgusEngine(0, num_shards=2)
        bad_batch = pa.RecordBatch.from_pydict(
            {
                "asset_id": pa.array([1], type=pa.uint32()),
                "timestamp_ns": pa.array([1_000_000_000], type=pa.int64()),
                "price": pa.array([100.0], type=pa.float64()),
                "volume": pa.array([1000.0], type=pa.float64()),
                "source": pa.array(
                    ["test"],
                    type=pa.dictionary(pa.int8(), pa.utf8()),
                ),
                "schema_version": pa.array([999], type=pa.uint16()),
            },
            schema=canonical_schema().set(
                5,
                canonical_schema().field(5),
            ),
        )
        with pytest.raises(ValueError):
            engine.process_batch(bad_batch)

    def test_empty_batch(self) -> None:
        """Test that an empty batch is handled gracefully."""
        try:
            from argus._argus_core import PyArgusEngine
        except ImportError:
            pytest.skip("Rust engine not built")

        engine = PyArgusEngine(0, num_shards=2)
        empty = _make_test_batch(0)
        engine.process_batch(empty)  # Should not raise
