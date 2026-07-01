"""End-to-end tests for the Argus engine."""

import pyarrow as pa
import pytest

from argus import ArgusClient
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


class TestArgusClientE2E:
    """End-to-end tests for ArgusClient."""

    def test_create_client(self) -> None:
        client = ArgusClient.create([], num_assets=0)
        assert client.tick_count == 0

    def test_poll_empty_adapters(self) -> None:
        client = ArgusClient.create([], num_assets=0)
        n = client.poll_and_update()
        assert n == 0

    def test_snapshot_initial(self) -> None:
        client = ArgusClient.create([], num_assets=0)
        snap = client.snapshot()
        if client._engine is not None:
            assert "asset_ids" in snap
        else:
            assert snap["tick_count"] == 0
