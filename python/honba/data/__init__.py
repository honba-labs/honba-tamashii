"""Data management package for Honba: Arrow lakehouse, Bhavcopy loaders, query, and store."""

from __future__ import annotations

import importlib
from typing import Any

from honba.data import arrow
from honba.data.arrow import (
    ArrowStream,
    BhavRecord,
    Lakehouse,
    bars_to_arrow,
    bhavcopy_to_arrow,
    bhavcopy_to_bars,
    export_capsule,
    load_bars,
    load_bhavcopy_day,
    load_ticks,
    parse_bhavcopy_csv,
    protocol_capsule,
    ticks_to_arrow,
    to_polars,
    to_pyarrow,
)

__all__ = [
    "ArrowStream",
    "BhavRecord",
    "Lakehouse",
    "arrow",
    "bars_to_arrow",
    "bhavcopy_to_arrow",
    "bhavcopy_to_bars",
    "export_capsule",
    "load_bars",
    "load_bhavcopy_day",
    "load_ticks",
    "loaders",
    "parse_bhavcopy_csv",
    "protocol_capsule",
    "query",
    "store",
    "ticks_to_arrow",
    "to_polars",
    "to_pyarrow",
]


def __getattr__(name: str) -> Any:
    if name in ("loaders", "query", "store"):
        return importlib.import_module(f"honba.data.{name}")
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
