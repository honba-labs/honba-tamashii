"""Honba Strategy API v2 (Latest & Recommended).

Provides the Strategy v2 event-driven contract:
- Explicit version tag: `__strategy_api_version__ = "2.0.0"`
- Unified event hook: `on_step(event: MarketEvent, ctx: PyContextV2)`
- Backward compatibility: automatically unpacks Bar events into `on_bar(bar, ctx)`
"""

from __future__ import annotations

from typing import Any, ClassVar
from decimal import Decimal

from honba.entities.instrument import InstrumentId
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
    """Strategy API v2 base class (Latest & Recommended)."""

    name: ClassVar[str] = "strategy_v2"
    __strategy_api_version__: ClassVar[str] = "2.0.0"

    def on_step(self, event: Any, ctx: Any = None) -> None:
        """Called on every market event (Bar, Tick, etc.).

        Dispatches Bar events to `on_bar(bar, ctx)` if defined.
        """
        bar = getattr(event, "bar", None)
        if bar is not None:
            self.on_bar(bar, ctx)
        elif hasattr(event, "close") and hasattr(event, "open"):
            self.on_bar(event, ctx)


class AdaptiveMomentum(Strategy):
    """Next-generation event-driven Adaptive Momentum strategy (Strategy API v2)."""

    name = "adaptive_momentum"
    warmup_bars = 10
    __strategy_api_version__ = "2.0.0"

    def __init__(
        self,
        lookback: int = 10,
        threshold: float = 0.0,
        quantity: float = 1.0,
        instrument_id: Any = None,
        **kwargs: Any,
    ) -> None:
        super().__init__()
        self.lookback = lookback
        self.threshold = threshold
        self.quantity = float(quantity)
        if instrument_id is None:
            instrument_id = InstrumentId("RELIANCE", "NSE")
        elif isinstance(instrument_id, str):
            parts = instrument_id.split(".")
            sym = parts[0]
            exch = parts[1] if len(parts) > 1 else "NSE"
            instrument_id = InstrumentId(sym, exch)
        self.instrument_id = instrument_id
        self.history: list[float] = []

    def on_step(self, event: Any, ctx: Any = None) -> None:
        close = getattr(event, "close", None)
        if close is not None:
            close_val = float(str(close))
        elif hasattr(event, "bar") and event.bar is not None:
            close_val = float(event.bar.close)
        else:
            return

        self.history.append(close_val)
        if len(self.history) <= self.lookback:
            return

        past_px = self.history[-self.lookback - 1]
        momentum = (close_val - past_px) / past_px if past_px != 0 else 0.0

        iid = self.instrument_id
        if hasattr(event, "bar") and event.bar is not None and hasattr(event.bar, "instrument_id"):
            iid = event.bar.instrument_id

        if ctx is not None and hasattr(ctx, "submit"):
            if momentum > self.threshold:
                ctx.submit(str(iid), "BUY", Decimal(str(self.quantity)))
            elif momentum < -self.threshold:
                ctx.submit(str(iid), "SELL", Decimal(str(self.quantity)))
        else:
            pos = self.position(iid)
            if momentum > self.threshold and pos <= 0:
                self.buy(iid, self.quantity)
            elif momentum < -self.threshold and pos >= 0:
                self.sell(iid, self.quantity)


class SmaCrossover(Strategy):
    """v2 SMA Crossover strategy."""

    name = "sma_crossover_v2"
    warmup_bars = 30
    __strategy_api_version__ = "2.0.0"

    def __init__(
        self,
        instrument_id: Any = None,
        fast: int = 10,
        slow: int = 30,
        quantity: float = 1.0,
        **kwargs: Any,
    ) -> None:
        if isinstance(instrument_id, int):
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

    def on_step(self, event: Any, ctx: Any = None) -> None:
        close = getattr(event, "close", None)
        if close is not None:
            close_val = float(str(close))
        elif hasattr(event, "bar") and event.bar is not None:
            close_val = float(event.bar.close)
        else:
            return

        f = self.fast_sma.update(close_val)
        s = self.slow_sma.update(close_val)
        if f is None or s is None:
            return

        iid = self.instrument_id
        if hasattr(event, "bar") and event.bar is not None and hasattr(event.bar, "instrument_id"):
            iid = event.bar.instrument_id

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


__all__ = [
    "AdaptiveMomentum",
    "ArbitrageStrategy",
    "Parameter",
    "QuotingStrategy",
    "RebalancingStrategy",
    "SlicingStrategy",
    "SmaCrossover",
    "Strategy",
    "indicators",
]
