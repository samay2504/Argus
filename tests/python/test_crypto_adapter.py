"""Tests for CryptoAdapter."""

import pyarrow as pa
import pytest

from argus.adapters.crypto_ccxt import CryptoAdapter
from argus.registry import AssetRegistry

@pytest.fixture
def mock_exchange(mocker):
    exchange = mocker.MagicMock()
    # fetch_ohlcv returns [timestamp_ms, open, high, low, close, volume]
    exchange.fetch_ohlcv.return_value = [
        [1672531200000, 16500.0, 16550.0, 16490.0, 16520.5, 10.5]
    ]
    return exchange

def test_crypto_adapter_unit(mocker, mock_exchange):
    mocker.patch("argus.adapters.crypto_ccxt.ccxt.binanceus", return_value=mock_exchange, create=True)
    registry = AssetRegistry()
    adapter = CryptoAdapter(exchange_id="binanceus", symbols=["BTC/USDT"], registry=registry)
    
    batch = adapter.poll()
    
    assert batch is not None
    assert isinstance(batch, pa.RecordBatch)
    assert batch.num_rows == 1
    
    # Assert values
    assert batch.column("asset_id")[0].as_py() == registry.resolve("BTC/USDT")
    assert batch.column("timestamp_ns")[0].as_py() == 1672531200000000000
    assert batch.column("price")[0].as_py() == 16520.5
    assert batch.column("volume")[0].as_py() == 10.5
    assert batch.column("source")[0].as_py() == "ccxt:binanceus"
    
@pytest.mark.integration
def test_crypto_adapter_integration():
    registry = AssetRegistry()
    # Use standard binanceus to ensure it's accessible publicly
    adapter = CryptoAdapter(exchange_id="binanceus", symbols=["BTC/USDT"], registry=registry)
    batch = adapter.poll()
    
    if batch is not None:
        assert isinstance(batch, pa.RecordBatch)
        assert batch.num_rows > 0
        assert batch.column("price")[0].as_py() > 0
