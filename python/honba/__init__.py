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

from honba import async_bridge, compat, data, indicators, sizing, v1, v2
from honba._honba import (
    ArrowStream,
    Bar,
    BhavRecord,
    DepthLevel,
    FillReport,
    FnoContract,
    InstrumentId,
    Lakehouse,
    MarketEvent,
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
    PyCancellationToken,
    PyContext,
    PyContextV2,
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
from honba.runner import PortfolioResult, PortfolioRunner
from honba.strategies.base import (
    ArbitrageStrategy,
    QuotingStrategy,
    RebalancingStrategy,
    SlicingStrategy,
)
from honba.strategy import Strategy
from honba.v1.strategies import Strategy as StrategyV1
from honba.v2.strategies import Strategy as StrategyV2

CancellationToken = PyCancellationToken


__all__ = [
    "ArbitrageStrategy",
    "ArrowStream",
    "Bar",
    "BhavRecord",
    "CancellationToken",
    "DepthLevel",
    "FillReport",
    "FnoContract",
    "InstrumentId",
    "Lakehouse",
    "MarketEvent",
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
    "PortfolioResult",
    "PortfolioRunner",
    "Price",
    "PriceBand",
    "PyCancellationToken",
    "PyContext",
    "PyContextV2",
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
    "StrategyV1",
    "StrategyV2",
    "Tick",
    "TimeInForce",
    "Trade",
    "TradingState",
    "arrow",
    "async_bridge",
    "compat",
    "data",
    "indicators",
    "sizing",
    "v1",
    "v2",
]

__version__ = "0.1.0"
