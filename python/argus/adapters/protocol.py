"""DataAdapter protocol for Argus data sources."""

from __future__ import annotations

from typing import Protocol, runtime_checkable

import pyarrow as pa

__all__ = ["DataAdapter"]


@runtime_checkable
class DataAdapter(Protocol):
    """Protocol for data source adapters.

    All adapters must normalize vendor-specific data into the canonical
    Arrow schema (see argus.schema) before returning it.
    """

    def poll(self) -> pa.RecordBatch | None:
        """Poll for the next batch of normalized tick data.

        Returns a RecordBatch conforming to the canonical Argus schema,
        or None when there is no new data to process.
        """
        ...
