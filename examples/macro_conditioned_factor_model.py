"""Macro-conditioned factor model demo.

Shows how FRED macro factors condition the factor model,
demonstrating that betas shift as the macro environment changes.
"""

from argus import ArgusClient
from argus.adapters.macro_fred import MacroAdapter
from argus.registry import AssetRegistry


def main() -> None:
    """Run the macro-conditioned factor model demo."""
    print("Argus — Macro-Conditioned Factor Model Demo")
    print("=" * 50)
    print()
    print("This demo illustrates how macro factors from FRED")
    print("(Fed Funds Rate, 10Y-2Y spread, VIX) condition the")
    print("factor model, causing betas to shift in response")
    print("to macro regime changes.")
    print()

    registry = AssetRegistry()
    macro = MacroAdapter(
        series_ids=("DFF", "T10Y2Y", "VIXCLS"),
        registry=registry,
    )

    client = ArgusClient.create([macro], forgetting_factor=0.95)

    n = client.poll_and_update()
    print(f"Ingested {n} macro data points")
    print(f"Snapshot: {client.snapshot()}")
    print()
    print("In a full setup, you would see factor betas here")
    print("shifting as the macro environment changes.")


if __name__ == "__main__":
    main()
