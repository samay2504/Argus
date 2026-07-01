"""Argus data source adapters."""

from argus.adapters.protocol import DataAdapter
from argus.adapters.crypto_ccxt import CryptoAdapter
from argus.adapters.macro_fred import MacroAdapter

__all__ = ["DataAdapter", "CryptoAdapter", "MacroAdapter"]
