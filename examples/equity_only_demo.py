"""Equity-only demo for Argus risk engine."""

from argus import ArgusClient
from argus.adapters.crypto_ccxt import CryptoAdapter
from argus.adapters.macro_fred import MacroAdapter
from argus.registry import AssetRegistry


def main() -> None:
    """Run the equity-only demo."""
    print("Argus — Equity-Only Demo")
    print("=" * 40)
    print()
    print("Note: This demo requires the Rust engine to be built.")
    print("Run 'maturin develop' first.")
    print()

    registry = AssetRegistry()
    for sym in ["SPY", "AAPL", "MSFT"]:
        registry.register(sym, "equity")

    print(f"Registered {len(registry)} assets")
    print("Assets:", registry.tickers())


if __name__ == "__main__":
    main()
