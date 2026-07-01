"""Shared pytest configuration and fixtures for Argus Python tests."""

from __future__ import annotations

import pyarrow as pa
import pytest

from argus.schema import SCHEMA_VERSION, canonical_schema


@pytest.fixture
def sample_batch() -> pa.RecordBatch:
    """Create a minimal valid RecordBatch conforming to the canonical schema."""
    return pa.RecordBatch.from_pydict(
        {
            "asset_id": pa.array([1, 2], type=pa.uint32()),
            "timestamp_ns": pa.array(
                [1_700_000_000_000_000_000, 1_700_000_001_000_000_000],
                type=pa.int64(),
            ),
            "price": pa.array([100.0, 200.0], type=pa.float64()),
            "volume": pa.array([1000.0, None], type=pa.float64()),
            "source": pa.array(["yfinance", "ccxt:binance"]).dictionary_encode(),
            "schema_version": pa.array(
                [SCHEMA_VERSION, SCHEMA_VERSION], type=pa.uint16()
            ),
        },
        schema=canonical_schema(),
    )
