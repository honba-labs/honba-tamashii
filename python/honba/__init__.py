"""本場 honba — Rust core, Python surface.

Two layers meet in this package:

1. The compiled extension `honba._honba`, built from this repo's own Rust crate
   (`crates/honba-tamashii-py`). It is the authoritative face of the canonical
   types (`honba-core`): exact-decimal `Price`, `Quantity`, `Money`,
   `InstrumentId`, `PriceBand`, the market-data `Bar`/`Tick`/`DepthLevel`, the
   order contracts, and the pre-trade risk gate (`honba-mon`). Everything
   below re-exports those, plus the thin Python `Strategy` base class from
   `honba/strategy.py`.
2. A bulk-imported tree of pure-Python modules carried over from the legacy
   honba-labs package (honba.domain, honba.entities, honba.markets,
   honba.screener, honba.data, honba.research, honba.backtest, honba.strategies,
   honba.adapters, honba.ai, honba.api, honba.client, honba.cli, honba.query,
   honba.utils, honba.wire, honba.analytics, honba.algo_analytics, honba.core,
   plus top-level risk.py / session.py / event_loop.py / _native.py /
   display.py / report.py / incubation.py / log.py / log_main.py). That code
   was written against an older compiled extension; its native calls are
   deliberately lazy (see `honba._native.native_attr`), so plain `import
   honba` and ordinary submodule imports never touch the extension; only a
   call into a function this extension does not have fails at call time with a
   clear ImportError / RuntimeError, or silently falls back to an existing
   pure-Python path where the legacy code already has one (e.g.
   `honba.backtest.simulated.resolve_backend("auto")` and
   `honba.markets.india.settlement.nse_equity_settlement_days`).

Money never crosses the boundary as a float: a price, quantity or amount is
an exact decimal on both sides (`decimal.Decimal` in Python).
"""
from honba._honba import (
    Bar,
    DepthLevel,
    FillReport,
    FnoContract,
    InstrumentId,
    Metrics,
    MinorUnit,
    Money,
    OtrConfig,
    OrderAck,
    OrderIntent,
    OrderRequest,
    OrderSide,
    OrderStatus,
    OrderType,
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
    SMA,
    Tick,
    TimeInForce,
    TradingState,
)
from honba.strategy import Strategy

__all__ = [
    "Bar",
    "DepthLevel",
    "FillReport",
    "FnoContract",
    "InstrumentId",
    "Metrics",
    "MinorUnit",
    "Money",
    "OtrConfig",
    "OrderAck",
    "OrderIntent",
    "OrderRequest",
    "OrderSide",
    "OrderStatus",
    "OrderType",
    "Price",
    "PriceBand",
    "PyContext",
    "Quantity",
    "RiskDecision",
    "RiskEngine",
    "RiskLimits",
    "RiskRefusal",
    "RiskStage",
    "Runner",
    "SMA",
    "Strategy",
    "Tick",
    "TimeInForce",
    "TradingState",
    "arrow",
    "indicators",
]

__version__ = "0.1.0"
