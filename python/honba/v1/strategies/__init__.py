"""Honba Strategy API v1 (Maintained for existing software).

Provides the Strategy v1 contract:
- Explicit version tag: `__strategy_api_version__ = "1.0.0"`
- Canonical hooks: `setup()`, `on_bar(bar, ctx)`, `on_stop()`
"""

from __future__ import annotations

from typing import Any, ClassVar
from decimal import Decimal

from honba.entities.bar import Bar
from honba.entities.instrument import InstrumentId
from honba.entities.order import OrderIntent
from honba import indicators
from honba.strategies.base import (
    ArbitrageStrategy,
    Parameter,
    QuotingStrategy,
    RebalancingStrategy,
    SlicingStrategy,
    Strategy as _BaseStrategy,
)
from honba.strategies.indicators import Sma


class Strategy(_BaseStrategy):
    """Strategy API v1 base class."""

    name: ClassVar[str] = "strategy_v1"
    __strategy_api_version__: ClassVar[str] = "1.0.0"


class SmaCrossover(Strategy):
    """v1 SMA Crossover strategy."""

    name = "sma_crossover"
    warmup_bars = 30
    __strategy_api_version__ = "1.0.0"

    def __init__(
        self,
        instrument_id: Any = None,
        fast: int = 10,
        slow: int = 30,
        quantity: float = 1.0,
        **kwargs: Any,
    ) -> None:
        if isinstance(instrument_id, int):
            # Called as SmaCrossover(10, 30)
            slow = fast
            fast = instrument_id
            instrument_id = None

        if instrument_id is None:
            instrument_id = InstrumentId("RELIANCE", "NSE")
        elif isinstance(instrument_id, str):
            parts = instrument_id.split(".")
            sym = parts[0]
            exch = parts[1] if len(parts) > 1 else "NSE"
            instrument_id = InstrumentId(sym, exch)

        super().__init__()
        if not 0 < fast < slow:
            raise ValueError(f"SMA periods must be positive and fast < slow: got fast={fast}, slow={slow}")
        self.instrument_id = instrument_id
        self.fast_period = fast
        self.slow_period = slow
        self.quantity = float(quantity)
        self.fast_sma = Sma(fast)
        self.slow_sma = Sma(slow)
        self.bars_seen = 0

    def on_bar(self, bar: Any, ctx: Any = None) -> None:
        self.bars_seen += 1
        close_px = getattr(bar, "close", None)
        if close_px is None:
            return
        close_val = float(str(close_px))
        f = self.fast_sma.update(close_val)
        s = self.slow_sma.update(close_val)
        if f is None or s is None:
            return

        iid = getattr(bar, "instrument_id", self.instrument_id)
        if ctx is not None and hasattr(ctx, "submit"):
            if f > s:
                ctx.submit(str(iid), "BUY", Decimal(str(self.quantity)))
            elif f < s:
                ctx.submit(str(iid), "SELL", Decimal(str(self.quantity)))
        else:
            pos = self.position(iid)
            if f > s and pos <= 0:
                self.buy(iid, self.quantity)
            elif f < s and pos >= 0:
                self.sell(iid, self.quantity)


class BuyAndHold(Strategy):
    """v1 Buy and Hold strategy."""

    name = "buy_and_hold"
    __strategy_api_version__ = "1.0.0"

    def __init__(self, instrument_id: Any = None, quantity: float = 1.0, **kwargs: Any) -> None:
        if instrument_id is None:
            instrument_id = InstrumentId("RELIANCE", "NSE")
        elif isinstance(instrument_id, str):
            parts = instrument_id.split(".")
            sym = parts[0]
            exch = parts[1] if len(parts) > 1 else "NSE"
            instrument_id = InstrumentId(sym, exch)

        super().__init__()
        self.instrument_id = instrument_id
        self.quantity = float(quantity)
        self.bought = False

    def on_bar(self, bar: Any, ctx: Any = None) -> None:
        if not self.bought:
            iid = getattr(bar, "instrument_id", self.instrument_id)
            if ctx is not None and hasattr(ctx, "submit"):
                ctx.submit(str(iid), "BUY", Decimal(str(self.quantity)))
            else:
                self.buy(iid, self.quantity)
            self.bought = True


__all__ = [
    "ArbitrageStrategy",
    "BuyAndHold",
    "Parameter",
    "QuotingStrategy",
    "RebalancingStrategy",
    "SlicingStrategy",
    "SmaCrossover",
    "Strategy",
    "indicators",
]
