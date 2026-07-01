"""Tests for the canonical Arrow schema definition and validation."""

from __future__ import annotations

import pyarrow as pa
import pytest

from argus.schema import SCHEMA_VERSION, canonical_schema, validate_batch


class TestCanonicalSchema:
    """Tests for canonical_schema()."""

    def test_returns_arrow_schema(self) -> None:
        schema = canonical_schema()
        assert isinstance(schema, pa.Schema)

    def test_has_required_fields(self) -> None:
        schema = canonical_schema()
        expected_names = [
            "asset_id",
            "timestamp_ns",
            "price",
            "volume",
            "source",
            "schema_version",
        ]
        assert schema.names == expected_names

    def test_field_types(self) -> None:
        schema = canonical_schema()
        assert schema.field("asset_id").type == pa.uint32()
        assert schema.field("timestamp_ns").type == pa.int64()
        assert schema.field("price").type == pa.float64()
        assert schema.field("volume").type == pa.float64()
        assert pa.types.is_dictionary(schema.field("source").type)
        assert schema.field("schema_version").type == pa.uint16()

    def test_volume_is_nullable(self) -> None:
        schema = canonical_schema()
        assert schema.field("volume").nullable is True

    def test_required_fields_not_nullable(self) -> None:
        schema = canonical_schema()
        for name in ["asset_id", "timestamp_ns", "price", "schema_version"]:
            assert schema.field(name).nullable is False, f"{name} should not be nullable"

    def test_schema_metadata_contains_version(self) -> None:
        schema = canonical_schema()
        assert schema.metadata is not None
        assert schema.metadata[b"argus_schema_version"] == str(SCHEMA_VERSION).encode()

    def test_schema_version_is_positive(self) -> None:
        assert SCHEMA_VERSION >= 1


class TestValidateBatch:
    """Tests for validate_batch()."""

    def test_valid_batch_passes(self, sample_batch: pa.RecordBatch) -> None:
        # Should not raise
        validate_batch(sample_batch)

    def test_wrong_column_count_raises(self) -> None:
        batch = pa.RecordBatch.from_pydict(
            {"asset_id": pa.array([1], type=pa.uint32())}
        )
        with pytest.raises(ValueError, match="expected 6 columns"):
            validate_batch(batch)

    def test_wrong_schema_version_raises(self) -> None:
        schema = canonical_schema()
        batch = pa.RecordBatch.from_pydict(
            {
                "asset_id": pa.array([1], type=pa.uint32()),
                "timestamp_ns": pa.array([1_700_000_000_000_000_000], type=pa.int64()),
                "price": pa.array([100.0], type=pa.float64()),
                "volume": pa.array([1000.0], type=pa.float64()),
                "source": pa.array(["yfinance"]).dictionary_encode(),
                "schema_version": pa.array([999], type=pa.uint16()),
            },
            schema=schema,
        )
        with pytest.raises(ValueError, match="schema version mismatch"):
            validate_batch(batch)

    def test_wrong_field_name_raises(self) -> None:
        schema = pa.schema(
            [
                pa.field("wrong_name", pa.uint32(), nullable=False),
                pa.field("timestamp_ns", pa.int64(), nullable=False),
                pa.field("price", pa.float64(), nullable=False),
                pa.field("volume", pa.float64(), nullable=True),
                pa.field("source", pa.dictionary(pa.int8(), pa.string()), nullable=False),
                pa.field("schema_version", pa.uint16(), nullable=False),
            ]
        )
        batch = pa.RecordBatch.from_pydict(
            {
                "wrong_name": pa.array([1], type=pa.uint32()),
                "timestamp_ns": pa.array([1_700_000_000_000_000_000], type=pa.int64()),
                "price": pa.array([100.0], type=pa.float64()),
                "volume": pa.array([1000.0], type=pa.float64()),
                "source": pa.array(["yfinance"]).dictionary_encode(),
                "schema_version": pa.array([SCHEMA_VERSION], type=pa.uint16()),
            },
            schema=schema,
        )
        with pytest.raises(ValueError, match="expected name 'asset_id'"):
            validate_batch(batch)
