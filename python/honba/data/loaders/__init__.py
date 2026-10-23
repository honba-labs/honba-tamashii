"""Data loaders for market data."""

from __future__ import annotations

from typing import Any

from honba.data.loaders.nse import (
    NseBhavcopyProvider,
    parse_bhavcopy_csv,
    parse_bhavcopy_records,
)

__all__ = [
    "NseBhavcopyProvider",
    "YFinanceProvider",
    "dataframe_to_bars",
    "from_yfinance_symbol",
    "normalize_timeframe",
    "parse_bhavcopy_csv",
    "parse_bhavcopy_records",
    "to_yfinance_symbol",
]


def __getattr__(name: str) -> Any:
    if name in (
        "YFinanceProvider",
        "dataframe_to_bars",
        "from_yfinance_symbol",
        "normalize_timeframe",
        "to_yfinance_symbol",
    ):
        from honba.data.loaders import yfinance as _yf

        return getattr(_yf, name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
