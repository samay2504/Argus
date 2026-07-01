"""Canonical Arrow schema for normalized tick data.

All data adapters (equities/Rust, crypto/Python, macro/Python) must
normalize into this single versioned Arrow schema before crossing into
argus-core. The schema is defined once here and mirrored in Rust.
"""

from __future__ import annotations

import pyarrow as pa

__all__ = ["SCHEMA_VERSION", "canonical_schema", "validate_batch"]

SCHEMA_VERSION: int = 1


def canonical_schema() -> pa.Schema:
    """Return the canonical Arrow schema for normalized tick data.

    Fields
    ------
    asset_id : uint32
        Resolved via a registry, not a raw string ticker, in the hot path.
    timestamp_ns : int64
        UTC, nanosecond epoch — required for the Hayashi-Yoshida overlap
        estimator to compute meaningful interval overlaps.
    price : float64
        Last trade / close price.
    volume : float64, nullable
        Trade volume. Nullable for macro series (e.g., FRED data).
    source : dictionary<string>
        Data source identifier, e.g., ``"yfinance"``, ``"ccxt:binance"``,
        ``"fred"``.
    schema_version : uint16
        Bumped on any breaking field change. ``argus-ffi`` rejects
        mismatched versions loudly.
    """
    return pa.schema(
        [
            pa.field("asset_id", pa.uint32(), nullable=False),
            pa.field("timestamp_ns", pa.int64(), nullable=False),
            pa.field("price", pa.float64(), nullable=False),
            pa.field("volume", pa.float64(), nullable=True),
            pa.field("source", pa.dictionary(pa.int8(), pa.string()), nullable=False),
            pa.field("schema_version", pa.uint16(), nullable=False),
        ],
        metadata={"argus_schema_version": str(SCHEMA_VERSION)},
    )


def validate_batch(batch: pa.RecordBatch, /) -> None:
    """Validate that a RecordBatch conforms to the canonical schema.

    Parameters
    ----------
    batch : pa.RecordBatch
        The batch to validate (positional-only).

    Raises
    ------
    ValueError
        If the batch schema does not match the canonical schema, if
        ``schema_version`` contains a mismatched value, or if required
        non-nullable fields contain nulls.
    """
    expected = canonical_schema()

    # Check field count
    if batch.num_columns != len(expected):
        msg = (
            f"expected {len(expected)} columns, got {batch.num_columns}: "
            f"expected {expected.names}, got {batch.schema.names}"
        )
        raise ValueError(msg)

    # Check each field
    for i, expected_field in enumerate(expected):
        actual_field = batch.schema.field(i)
        if actual_field.name != expected_field.name:
            msg = f"field {i}: expected name '{expected_field.name}', got '{actual_field.name}'"
            raise ValueError(msg)
        # For dictionary types, check the value type matches
        if pa.types.is_dictionary(expected_field.type):
            if not pa.types.is_dictionary(actual_field.type):
                msg = f"field '{expected_field.name}': expected dictionary type, got {actual_field.type}"
                raise ValueError(msg)
        elif actual_field.type != expected_field.type:
            msg = (
                f"field '{expected_field.name}': expected type {expected_field.type}, "
                f"got {actual_field.type}"
            )
            raise ValueError(msg)

    # Check schema version values
    version_col = batch.column("schema_version")
    for val in version_col:
        if val.as_py() != SCHEMA_VERSION:
            msg = f"schema version mismatch: expected {SCHEMA_VERSION}, got {val.as_py()}"
            raise ValueError(msg)

    # Check non-nullable fields have no nulls
    for field in expected:
        if not field.nullable:
            col = batch.column(field.name)
            if col.null_count > 0:
                msg = f"non-nullable field '{field.name}' contains {col.null_count} null(s)"
                raise ValueError(msg)
