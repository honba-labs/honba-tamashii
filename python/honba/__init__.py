"""本場 honba — Rust core, Python surface.

Two layers meet in this package:

1. The compiled extension `honba._honba`, built from this repo's own Rust crate
   (`crates/honba-tamashii-py`). It is the authoritative face of the canonical
   types (`honba-core`): exact-decimal `Price`, `Quantity`, `Money`,
   `InstrumentId`, `PriceBand`, the market-data `Bar`/`Tick`/`DepthLevel`, the
   order contracts, the pre-trade risk gate (`honba-mon`), the zero-copy Arrow
   Lakehouse (`honba-me-feed`), NSE Bhavcopy loaders (`honba-me-loaders`), and
   vectorized indicators (`honba-me-indicators`).
2. The Python SDK (`honba.strategy`, `honba.strategies`, `honba.indicators`,
   `honba.data`, `honba.runner`) providing an idiomatic research-to-live API.

Money never crosses the boundary as a float: a price, quantity or amount is
an exact decimal on both sides (`decimal.Decimal` in Python).
"""

from honba import data, indicators
from honba._honba import (
    ArrowStream,
    Bar,
    BhavRecord,
    DepthLevel,
    FillReport,
    FnoContract,
    InstrumentId,
    Lakehouse,
    Metrics,
    MinorUnit,
    Money,
    OrderAck,
    OrderIntent,
    OrderRequest,
    OrderSide,
    OrderStatus,
    OrderType,
    OtrConfig,
    Price,
    PriceBand,
    PyContext,
    Quantity,
    RiskDecision,
    RiskEngine,
    RiskLimits,
    RiskRefusal,
    RiskStage,
    Runner,
    Sma,
    Tick,
    TimeInForce,
    TradingState,
)
from honba.data import arrow
from honba.domain.trade import Trade
from honba.strategies.base import (
    ArbitrageStrategy,
    QuotingStrategy,
    RebalancingStrategy,
    SlicingStrategy,
)
from honba.strategy import Strategy

__all__ = [
    "ArbitrageStrategy",
    "ArrowStream",
    "Bar",
    "BhavRecord",
    "DepthLevel",
    "FillReport",
    "FnoContract",
    "InstrumentId",
    "Lakehouse",
    "Metrics",
    "MinorUnit",
    "Money",
    "OrderAck",
    "OrderIntent",
    "OrderRequest",
    "OrderSide",
    "OrderStatus",
    "OrderType",
    "OtrConfig",
    "Price",
    "PriceBand",
    "PyContext",
    "Quantity",
    "QuotingStrategy",
    "RebalancingStrategy",
    "RiskDecision",
    "RiskEngine",
    "RiskLimits",
    "RiskRefusal",
    "RiskStage",
    "Runner",
    "SlicingStrategy",
    "Sma",
    "Strategy",
    "Tick",
    "TimeInForce",
    "Trade",
    "TradingState",
    "arrow",
    "data",
    "indicators",
]

__version__ = "0.1.0"
