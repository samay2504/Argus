"""Tests for MacroAdapter."""

import os
import pyarrow as pa
import pytest

from argus.adapters.macro_fred import MacroAdapter
from argus.registry import AssetRegistry

@pytest.fixture
def mock_requests_get(mocker):
    mock_resp = mocker.MagicMock()
    mock_resp.json.return_value = {
        "observations": [
            {
                "realtime_start": "2023-01-01",
                "realtime_end": "2023-01-01",
                "date": "2023-01-01",
                "value": "4.33"
            }
        ]
    }
    mock_resp.raise_for_status.return_value = None
    return mocker.patch("argus.adapters.macro_fred.requests.get", return_value=mock_resp)

def test_macro_adapter_unit(mock_requests_get):
    registry = AssetRegistry()
    adapter = MacroAdapter(series_ids=("DFF",), api_key="dummy", registry=registry, poll_interval_s=0.0)
    
    batch = adapter.poll()
    
    assert batch is not None
    assert isinstance(batch, pa.RecordBatch)
    assert batch.num_rows == 1
    
    # Assert values
    assert batch.column("asset_id")[0].as_py() == registry.resolve("DFF")
    assert batch.column("price")[0].as_py() == 4.33
    assert batch.column("volume")[0].as_py() is None
    assert batch.column("source")[0].as_py() == "fred"
    
@pytest.mark.integration
def test_macro_adapter_integration():
    api_key = os.environ.get("FRED_API_KEY", "")
    if not api_key:
        pytest.skip("FRED_API_KEY not set")
        
    registry = AssetRegistry()
    adapter = MacroAdapter(series_ids=("DFF",), api_key=api_key, registry=registry, poll_interval_s=0.0)
    batch = adapter.poll()
    
    if batch is not None:
        assert isinstance(batch, pa.RecordBatch)
        assert batch.num_rows > 0
        assert batch.column("price")[0].as_py() > 0
