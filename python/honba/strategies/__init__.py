"""Trading strategy interface, runners, and indicator library."""

from typing import Any

from honba.strategies import indicators
from honba.strategies.base import (
    ArbitrageStrategy,
    Parameter,
    QuotingStrategy,
    RebalancingStrategy,
    SlicingStrategy,
    Strategy,
)
from honba.strategies.context import LedgerContext, StrategyContext
from honba.strategies.declarative import (
    DeclarativeStrategy,
    Entry,
    TargetWeightStrategy,
)
from honba.strategies.portfolio import (
    EqualWeight,
    EveryNDays,
    MarketView,
    NamedUniverse,
    PortfolioStrategy,
    RebalanceSchedule,
    ScheduleState,
    SelectAll,
    Selector,
    StaticUniverse,
    Universe,
    WeightingScheme,
)


def __getattr__(name: str) -> Any:
    if name == "StrategyV1":
        from honba.v1.strategies import Strategy as StrategyV1

        return StrategyV1
    if name == "StrategyV2":
        from honba.v2.strategies import Strategy as StrategyV2

        return StrategyV2
    if name == "AdaptiveMomentum":
        from honba.v2.strategies import AdaptiveMomentum

        return AdaptiveMomentum
    if name == "SmaCrossover":
        from honba.v2.strategies import SmaCrossover

        return SmaCrossover
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [
    "AdaptiveMomentum",
    "ArbitrageStrategy",
    "DeclarativeStrategy",
    "Entry",
    "EqualWeight",
    "EveryNDays",
    "LedgerContext",
    "MarketView",
    "NamedUniverse",
    "Parameter",
    "PortfolioStrategy",
    "QuotingStrategy",
    "RebalanceSchedule",
    "RebalancingStrategy",
    "ScheduleState",
    "SelectAll",
    "Selector",
    "SlicingStrategy",
    "SmaCrossover",
    "StaticUniverse",
    "Strategy",
    "StrategyContext",
    "StrategyV1",
    "StrategyV2",
    "TargetWeightStrategy",
    "Universe",
    "WeightingScheme",
    "indicators",
]
