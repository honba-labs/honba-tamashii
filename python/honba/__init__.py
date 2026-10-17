"""本場 honba — Rust core, Python surface.

The extension module is built from the Rust crate in this repository and
exposed as `honba._honba`. Everything public-facing is re-exported here.
"""
from honba._honba import (
    Bar,
    Metrics,
    OrderIntent,
    PyContext,
    Runner,
    SMA,
)
from honba.strategy import Strategy

__all__ = [
    "Bar",
    "Metrics",
    "OrderIntent",
    "PyContext",
    "Runner",
    "SMA",
    "Strategy",
]

__version__ = "0.1.0"
