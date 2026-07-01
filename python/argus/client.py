"""High-level orchestration API for the Argus risk engine."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

import pyarrow as pa

from argus.schema import canonical_schema, validate_batch

if TYPE_CHECKING:
    from argus.adapters.protocol import DataAdapter

__all__ = ["ArgusClient"]


@dataclass(slots=True)
class ArgusClient:
    """High-level entrypoint for the Argus risk engine.

    Orchestrates data adapters, feeds batches into the Rust core engine,
    and exposes the current risk exposure snapshot.
    """

    _adapters: list[DataAdapter] = field(default_factory=list)
    _tick_count: int = field(default=0, init=False)
    _engine: object = field(default=None, init=False)  # Will be _argus_core.PyArgusEngine when available

    @classmethod
    def create(
        cls,
        /,
        adapters: list[DataAdapter],
        *,
        num_assets: int,
        num_shards: int | None = None,
        forgetting_factor: float = 0.99,
    ) -> ArgusClient:
        """Factory method. Registers all adapters, builds the engine."""
        client = cls(_adapters=adapters)
        try:
            from argus._argus_core import PyArgusEngine
            shards = num_shards if num_shards is not None else 4
            client._engine = PyArgusEngine(num_assets, num_shards=shards)
        except ImportError:
            pass  # Rust engine not built yet; operate in Python-only mode
        return client

    def poll_and_update(self) -> int:
        """Poll all adapters, feed new data into the engine.

        Returns the number of ticks ingested.
        """
        count = 0
        for adapter in self._adapters:
            if (batch := adapter.poll()) is not None:
                validate_batch(batch)
                count += batch.num_rows
                if self._engine is not None:
                    try:
                        self._engine.process_batch(batch)
                    except Exception:
                        pass  # Log and continue in production
        self._tick_count += count
        return count

    def snapshot(self) -> dict:
        """Return current exposure snapshot."""
        if self._engine is not None:
            try:
                snap = self._engine.snapshot()
                return {
                    "asset_ids": snap.asset_ids,
                    "variances": snap.variances,
                    "covariance_matrix": snap.covariance_matrix,
                    "factor_betas": snap.factor_betas,
                }
            except Exception as e:
                return {"error": str(e)}
        return {
            "tick_count": self._tick_count,
            "num_adapters": len(self._adapters),
        }

    @property
    def tick_count(self) -> int:
        """Total number of ticks ingested."""
        return self._tick_count
