"""Asset registry for mapping symbol strings to integer AssetIds."""

from __future__ import annotations

__all__ = ["AssetRegistry"]

class AssetRegistry:
    """Registry mapping symbol strings to integer AssetIds."""

    def __init__(self) -> None:
        self._symbol_to_id: dict[str, int] = {}
        self._id_to_symbol: dict[int, str] = {}
        self._next_id: int = 1

    def resolve(self, symbol: str) -> int:
        """Resolve a symbol to an AssetId, creating it if it doesn't exist."""
        if symbol not in self._symbol_to_id:
            asset_id = self._next_id
            self._next_id += 1
            self._symbol_to_id[symbol] = asset_id
            self._id_to_symbol[asset_id] = symbol
        return self._symbol_to_id[symbol]

    def get_symbol(self, asset_id: int) -> str | None:
        """Get the symbol for an AssetId."""
        return self._id_to_symbol.get(asset_id)
