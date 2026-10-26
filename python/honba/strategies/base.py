"""The Strategy interface (ADR 008, E4-S2) and StockSharp Strategy Primitives.

Mirrors the Rust ``Strategy`` trait: strategies receive typed callbacks and act
only through their ``StrategyContext`` (``self.ctx``): read the clock, positions,
cash and instrument metadata, and submit order intents. They never touch
execution directly, so the same strategy runs in backtest, paper and live.

Also provides specialized StockSharp-derived strategy base classes:
- ``QuotingStrategy``: Automated quote management and pegging with tolerance bands.
- ``ArbitrageStrategy``: Dual-leg / cash-futures spread tracking and hedged execution.
- ``SlicingStrategy``: Institutional parent-order slicing (TWAP / iceberg child orders).
- ``RebalancingStrategy``: Target-weight portfolio drift detection and two-phase rebalancing.
"""

from __future__ import annotations

import logging
import warnings
from abc import ABC
from decimal import Decimal
from typing import TYPE_CHECKING, Any, ClassVar

from honba.entities.bar import Bar
from honba.entities.instrument import InstrumentId
from honba.entities.order import OrderIntent
from honba.entities.tick import QuoteTick, TradeTick
from honba.entities.trade import Trade
from honba.strategies.context import LedgerContext, StrategyContext

if TYPE_CHECKING:
    from typing_extensions import Self

    from honba.strategies.manifest import ParameterSpec

__all__ = [
    "LEGACY_ENTRY_POINTS",
    "ArbitrageStrategy",
    "Parameter",
    "QuotingStrategy",
    "RebalancingStrategy",
    "SlicingStrategy",
    "Strategy",
]

LEGACY_ENTRY_POINTS: tuple[str, ...] = (
    "drain_intents",
    "handle_fill",
    "handle_rejected",
)
"""Pre-ADR-008 runner entry points. Overriding one is deprecated (removed in 0.3)."""


class Parameter:
    """One-line parameter declaration for strategy hyperparameters (E4-S2).

    Can be used as a class attribute descriptor:
    ```python
    class MyStrategy(Strategy):
        fast = Parameter(default=20, min=5, max=50, step=5, description="Fast SMA")
    ```
    """

    def __init__(
        self,
        default: Any,
        *,
        min: float | None = None,
        max: float | None = None,
        step: float | None = None,
        range: str | tuple[float, float, float] | list[float] | None = None,
        values: list[Any] | None = None,
        description: str = "",
    ) -> None:
        self.default = default
        self.description = description
        self.name: str = ""
        self.values = values
        self.range_str: str | None = None

        if range is not None:
            if isinstance(range, str):
                parts = [p.strip() for p in range.split(":")]
                if len(parts) != 3:
                    raise ValueError(
                        f"range string must be 'min:max:step', got {range!r}"
                    )
                self.min = float(parts[0]) if "." in parts[0] else int(parts[0])
                self.max = float(parts[1]) if "." in parts[1] else int(parts[1])
                self.step = float(parts[2]) if "." in parts[2] else int(parts[2])
                self.range_str = range
            elif isinstance(range, (tuple, list)) and len(range) == 3:
                self.min, self.max, self.step = range[0], range[1], range[2]
                self.range_str = f"{self.min}:{self.max}:{self.step}"
            else:
                raise ValueError(
                    f"range must be 'min:max:step' or (min, max, step), got {range!r}"
                )
        else:
            self.min = min
            self.max = max
            self.step = step
            if min is not None and max is not None and step is not None:
                self.range_str = f"{min}:{max}:{step}"

        if self.min is not None and self.max is not None and self.min > self.max:
            raise ValueError(f"min ({self.min}) cannot exceed max ({self.max})")
        if self.step is not None and self.step <= 0:
            raise ValueError(f"step ({self.step}) must be > 0")

        # Validate default value against bounds
        if isinstance(self.default, (int, float)):
            if self.min is not None and self.default < self.min:
                raise ValueError(f"default {self.default} is less than min {self.min}")
            if self.max is not None and self.default > self.max:
                raise ValueError(
                    f"default {self.default} is greater than max {self.max}"
                )

    def __set_name__(self, owner: type, name: str) -> None:
        self.name = name

    def __get__(self, instance: Any, owner: type) -> Any:
        if instance is None:
            return self
        params = getattr(instance, "_params", None)
        if params is not None and self.name in params:
            return params[self.name]
        return self.default

    def discrete_values(self) -> list[Any]:
        """Returns all discrete values across the declared range."""
        if self.values is not None:
            return list(self.values)
        if self.min is not None and self.max is not None and self.step is not None:
            is_int = (
                isinstance(self.default, int)
                and (isinstance(self.min, int) or float(self.min).is_integer())
                and (isinstance(self.step, int) or float(self.step).is_integer())
            )
            vals = []
            curr = float(self.min)
            max_val = float(self.max)
            step_val = float(self.step)
            while curr <= max_val + 1e-9:
                vals.append(round(curr) if is_int else round(curr, 6))
                curr += step_val
            return vals
        return [self.default]

    def to_spec(self) -> ParameterSpec:
        from honba.strategies.manifest import ParameterSpec

        return ParameterSpec(
            default=self.default,
            range=self.range_str,
            min=float(self.min) if self.min is not None else None,
            max=float(self.max) if self.max is not None else None,
            step=float(self.step) if self.step is not None else None,
            description=self.description or None,
        )


class Strategy(ABC):
    """Subclass, set ``name``, override the hooks you need.

    Every hook defaults to a no-op. The runner binds a context before ``on_start``;
    a strategy used on its own (tests, ``honba.strategies.testing.replay``) gets a
    private ``LedgerContext``, so subclasses need not call ``super().__init__()``.\
    """

    name: ClassVar[str]
    __strategy_api_version__: ClassVar[str] = "2.0.0"
    # Driving bars consumed before the first order (``StrategyManifest.warmup_bars``); the
    # runner suppresses orders until then. A runner or config value overrides it.
    warmup_bars: ClassVar[int] = 0
    _declared_parameters: ClassVar[dict[str, Parameter]] = {}
    _ctx: StrategyContext
    logger: logging.Logger
    _params: dict[str, Any]

    def __init_subclass__(cls, **kwargs: Any) -> None:
        super().__init_subclass__(**kwargs)
        # Collect declared Parameter instances across class hierarchy
        declared: dict[str, Parameter] = {}
        for base in reversed(cls.__mro__):
            if hasattr(base, "_declared_parameters"):
                declared.update(base._declared_parameters)
        for k, v in cls.__dict__.items():
            if isinstance(v, Parameter):
                declared[k] = v
        cls._declared_parameters = declared

        overridden = [m for m in LEGACY_ENTRY_POINTS if m in cls.__dict__]
        if overridden:
            warnings.warn(
                f"{cls.__qualname__} overrides {', '.join(overridden)}: these runner entry points "
                "are still called in 0.1/0.2 but not from 0.3, when the runner talks to the "
                "StrategyContext directly; move the logic to on_fill (ADR 008)",
                DeprecationWarning,
                stacklevel=3,  # past ABCMeta.__new__: the class statement of the subclass
            )

    def __new__(cls, *args: Any, **kwargs: Any) -> Self:
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define a class attribute `name`")
        self = super().__new__(cls)
        self._ctx = LedgerContext()
        self.logger = logging.getLogger(f"honba.strategy.{cls.name}")
        self._params = {}
        if args and hasattr(args[0], "params") and isinstance(args[0].params, dict):
            self._params = dict(args[0].params)
        elif "config" in kwargs and hasattr(kwargs["config"], "params"):
            self._params = dict(kwargs["config"].params)
        elif "params" in kwargs and isinstance(kwargs["params"], dict):
            self._params = dict(kwargs["params"])
        return self

    @classmethod
    def hyperparameters(cls) -> dict[str, Parameter]:
        """Declared hyperparameters for this strategy (E4-S2)."""
        return dict(cls._declared_parameters)

    # -- context ---------------------------------------------------------------
    @property
    def ctx(self) -> StrategyContext:
        """The context this strategy reads from and submits to."""
        return self._ctx

    def bind(self, ctx: StrategyContext) -> None:
        """Attach the runner's context. Called by runners before ``on_start``."""
        if not isinstance(ctx, StrategyContext):
            raise TypeError(f"expected a StrategyContext, got {type(ctx).__name__}")
        self._ctx = ctx

    # -- hooks (all default to no-ops) -------------------------------------------
    def on_start(self) -> None: ...

    def on_bar(self, bar: Bar, ctx: Any = None) -> None: ...

    def on_step(self, event: Any, ctx: Any = None) -> None:
        """Called on every market event (MarketEvent).

        Defaults to unpacking Bar events and calling self.on_bar(bar, ctx)
        for backward compatibility.
        """
        bar = getattr(event, "bar", None)
        if bar is not None:
            self.on_bar(bar, ctx)
        elif hasattr(event, "close") and hasattr(event, "open"):
            self.on_bar(event, ctx)

    def on_quote(self, quote: QuoteTick) -> None:
        """Top-of-book update."""

    def on_trade(self, trade: TradeTick) -> None:
        """A market trade print (the strategy's own executions arrive in ``on_fill``)."""

    def on_fill(self, fill: Trade) -> None: ...

    def on_stop(self) -> None: ...

    # -- conveniences (delegate to the context) ---------------------------------
    def position(self, instrument_id: InstrumentId) -> float:
        """Net signed quantity held, updated from fills (``self.ctx.position``)."""
        return self.ctx.position(instrument_id)

    def busy(self, instrument_id: InstrumentId) -> bool:
        """True while an order for this instrument is unfilled (``self.ctx.busy``).

        Fills arrive after the strategy emits an intent, so gate new orders on
        this to avoid duplicate entries or exits.
        """
        return self.ctx.busy(instrument_id)

    def log_event(self, event_type: str, message: str = "", **data: Any) -> None:
        """Emit a structured event log for this strategy.

        Parameters
        ----------
        event_type : str
            Event name/identifier (e.g. 'EVENT_BUY', 'EVENT_MEMBERSHIP_ADD').
        message : str, optional
            Optional human-readable description.
        **data : Any
            Key-value pairs to log as event attributes.
        """
        parts = [event_type]
        if message:
            parts.append(message)
        if data:
            parts.append(" ".join(f"{k}={v}" for k, v in data.items()))
        self.logger.info(
            " ".join(parts), extra={"event_type": event_type, "event_data": data}
        )

    def buy(self, instrument_id: InstrumentId, quantity: float, **data: Any) -> None:
        self.log_event("EVENT_BUY", symbol=instrument_id.symbol, qty=quantity, **data)
        self.submit(OrderIntent.market_buy(instrument_id, quantity))

    def sell(self, instrument_id: InstrumentId, quantity: float, **data: Any) -> None:
        self.log_event("EVENT_SELL", symbol=instrument_id.symbol, qty=quantity, **data)
        self.submit(OrderIntent.market_sell(instrument_id, quantity))

    def submit(self, intent: OrderIntent) -> None:
        self.ctx.submit(intent)

    def cancel(self, intent: OrderIntent) -> None:
        """Ask for the order sent from ``intent`` to be cancelled (idempotent, see the context)."""
        self.ctx.cancel(intent)

    # -- legacy runner entry points (kept for one minor version) -----------------
    def drain_intents(self) -> list[OrderIntent]:
        """Returns and clears pending intents (legacy runner entry point)."""
        return self._ledger().drain_intents()

    def handle_fill(self, fill: Trade) -> None:
        """Legacy runner entry point: books the fill in the context, then calls ``on_fill``."""
        self._ledger().apply_fill(fill)
        self.on_fill(fill)

    def handle_rejected(self, intent: OrderIntent) -> None:
        """Legacy runner entry point: an order was rejected or cancelled unfilled."""
        self._ledger().release(intent)

    def _ledger(self) -> LedgerContext:
        if not isinstance(self.ctx, LedgerContext):
            raise TypeError(
                "drain_intents/handle_fill/handle_rejected need a LedgerContext; "
                f"this strategy is bound to {type(self.ctx).__name__}"
            )
        return self.ctx


# ===========================================================================
# StockSharp Specialized Strategy Base Classes (Phase 5.3)
# ===========================================================================


def _dec(v: Any) -> Decimal:
    if isinstance(v, Decimal):
        return v
    return Decimal(str(v))


class QuotingStrategy(Strategy):
    """StockSharp-style Quoting Strategy base class.

    Maintains a working limit order pegged to a target price and quantity,
    cancelling and replacing the active quote only when price drift exceeds
    ``price_tolerance`` or the desired quantity changes.
    """

    name: ClassVar[str] = "quoting"
    quoting_quantity = Parameter(default=10.0, min=1.0, max=10000.0, step=1.0)
    price_tolerance = Parameter(default=0.05, min=0.0, max=100.0, step=0.05)

    def __init__(
        self,
        *,
        quoting_side: str = "BUY",
        quoting_quantity: Decimal | float | str = Decimal(10),
        price_tolerance: Decimal | float | str = Decimal("0.05"),
        **kwargs: Any,
    ) -> None:
        self.quoting_side = quoting_side.upper()
        self._target_qty = _dec(quoting_quantity)
        self._filled_qty = Decimal(0)
        self._price_tolerance = _dec(price_tolerance)
        self._quoted_price: Decimal | None = None
        self._quoted_qty: Decimal | None = None
        self._active_intent: OrderIntent | None = None

    @property
    def remaining_quantity(self) -> Decimal:
        """Remaining unfilled quote quantity."""
        return max(Decimal(0), self._target_qty - self._filled_qty)

    @property
    def quoted_price(self) -> Decimal | None:
        """Currently working quote limit price, if any."""
        return self._quoted_price

    def need_requote(
        self,
        target_price: Decimal | float | str,
        target_qty: Decimal | float | str | None = None,
    ) -> bool:
        """Return ``True`` if the quote needs to be placed or replaced."""
        tp = _dec(target_price)
        tq = _dec(target_qty) if target_qty is not None else self.remaining_quantity
        if tq <= 0:
            return False
        if self._quoted_price is None or self._quoted_qty is None:
            return True
        if self._quoted_qty != tq:
            return True
        return abs(tp - self._quoted_price) > self._price_tolerance

    def update_quote(
        self,
        instrument_id: InstrumentId,
        target_price: Decimal | float | str,
        target_qty: Decimal | float | str | None = None,
    ) -> OrderIntent | None:
        """Place or replace the working quote when ``need_requote`` is ``True``."""
        tp = _dec(target_price)
        tq = _dec(target_qty) if target_qty is not None else self.remaining_quantity
        if tq <= 0:
            if self._active_intent is not None:
                self.cancel(self._active_intent)
                self._active_intent = None
                self._quoted_price = None
                self._quoted_qty = None
            return None
        if not self.need_requote(tp, tq):
            return self._active_intent
        if self._active_intent is not None:
            self.cancel(self._active_intent)
        if self.quoting_side == "BUY":
            intent = OrderIntent.limit_buy(instrument_id, float(tq), float(tp))
        else:
            intent = OrderIntent.limit_sell(instrument_id, float(tq), float(tp))
        self.submit(intent)
        self._active_intent = intent
        self._quoted_price = tp
        self._quoted_qty = tq
        return intent

    def on_fill(self, fill: Trade) -> None:
        self._filled_qty += _dec(fill.quantity)
        if self.remaining_quantity <= 0:
            self._active_intent = None
            self._quoted_price = None
            self._quoted_qty = None


class ArbitrageStrategy(Strategy):
    """StockSharp-style dual-leg Arbitrage Strategy base class.

    Tracks the price spread between a primary leg and a hedge leg
    (``spread = primary_price - hedge_ratio * hedge_price``) and dispatches
    synchronized entry and exit orders across both legs.
    """

    name: ClassVar[str] = "arbitrage"
    entry_spread = Parameter(default=5.0, min=0.1, max=500.0, step=0.5)
    exit_spread = Parameter(default=0.5, min=0.0, max=100.0, step=0.1)

    def __init__(
        self,
        *,
        entry_spread: Decimal | float | str = Decimal("5.0"),
        exit_spread: Decimal | float | str = Decimal("0.5"),
        hedge_ratio: Decimal | float | str = Decimal("1.0"),
        leg_quantity: Decimal | float | str = Decimal(1),
        **kwargs: Any,
    ) -> None:
        self._entry_spread = _dec(entry_spread)
        self._exit_spread = _dec(exit_spread)
        self._hedge_ratio = _dec(hedge_ratio)
        self._leg_quantity = _dec(leg_quantity)
        self.spread_position: int = 0  # +1 long spread, -1 short spread, 0 flat
        self.last_spread: Decimal | None = None

    def compute_spread(
        self,
        primary_price: Decimal | float | str,
        hedge_price: Decimal | float | str,
    ) -> Decimal:
        """Compute ``primary_price - hedge_ratio * hedge_price``."""
        sp = _dec(primary_price) - self._hedge_ratio * _dec(hedge_price)
        self.last_spread = sp
        return sp

    def evaluate_legs(
        self,
        primary_instrument: InstrumentId,
        primary_price: Decimal | float | str,
        hedge_instrument: InstrumentId,
        hedge_price: Decimal | float | str,
        quantity: Decimal | float | str | None = None,
    ) -> int:
        """Evaluate the two legs and submit synchronized entry/exit orders when triggered.

        Returns the updated ``spread_position`` (``1``, ``-1``, or ``0``).
        """
        spread = self.compute_spread(primary_price, hedge_price)
        qty = float(_dec(quantity) if quantity is not None else self._leg_quantity)
        hedge_qty = float(_dec(qty) * self._hedge_ratio)

        if self.spread_position == 0:
            if spread >= self._entry_spread:
                # Spread is rich: sell primary, buy hedge
                self.sell(primary_instrument, qty)
                self.buy(hedge_instrument, hedge_qty)
                self.spread_position = -1
            elif spread <= -self._entry_spread:
                # Spread is cheap: buy primary, sell hedge
                self.buy(primary_instrument, qty)
                self.sell(hedge_instrument, hedge_qty)
                self.spread_position = 1
        elif self.spread_position == -1 and spread <= self._exit_spread:
            self.buy(primary_instrument, qty)
            self.sell(hedge_instrument, hedge_qty)
            self.spread_position = 0
        elif self.spread_position == 1 and spread >= -self._exit_spread:
            self.sell(primary_instrument, qty)
            self.buy(hedge_instrument, hedge_qty)
            self.spread_position = 0

        return self.spread_position


class SlicingStrategy(Strategy):
    """StockSharp-style Order Slicing (TWAP / VWAP / Iceberg) Strategy base class.

    Splits a large parent order of ``total_quantity`` into smaller child slices
    of ``slice_size`` to minimize market impact.
    """

    name: ClassVar[str] = "slicing"
    total_quantity = Parameter(default=100.0, min=1.0, max=1000000.0, step=10.0)
    slice_size = Parameter(default=10.0, min=1.0, max=100000.0, step=1.0)

    def __init__(
        self,
        *,
        total_quantity: Decimal | float | str = Decimal(100),
        slice_size: Decimal | float | str = Decimal(10),
        side: str = "BUY",
        **kwargs: Any,
    ) -> None:
        self._total_quantity = _dec(total_quantity)
        self._slice_size = _dec(slice_size)
        if self._slice_size <= 0:
            raise ValueError("slice_size must be > 0")
        self.side = side.upper()
        self.dispatched_quantity: Decimal = Decimal(0)
        self.filled_quantity: Decimal = Decimal(0)
        self.slices_sent: int = 0

    @property
    def remaining_to_dispatch(self) -> Decimal:
        """Quantity not yet dispatched in child slices."""
        return max(Decimal(0), self._total_quantity - self.dispatched_quantity)

    @property
    def is_complete(self) -> bool:
        """True once all parent quantity has been dispatched."""
        return self.remaining_to_dispatch <= 0

    def next_slice_quantity(self) -> Decimal:
        """Compute the size of the next child slice."""
        return min(self._slice_size, self.remaining_to_dispatch)

    def execute_slice(
        self,
        instrument_id: InstrumentId,
        price: Decimal | float | str | None = None,
    ) -> OrderIntent | None:
        """Dispatch the next child order slice if ``remaining_to_dispatch > 0``."""
        qty = self.next_slice_quantity()
        if qty <= 0:
            return None
        if price is None:
            intent = (
                OrderIntent.market_buy(instrument_id, float(qty))
                if self.side == "BUY"
                else OrderIntent.market_sell(instrument_id, float(qty))
            )
        else:
            px = float(_dec(price))
            intent = (
                OrderIntent.limit_buy(instrument_id, float(qty), px)
                if self.side == "BUY"
                else OrderIntent.limit_sell(instrument_id, float(qty), px)
            )
        self.submit(intent)
        self.dispatched_quantity += qty
        self.slices_sent += 1
        return intent

    def on_fill(self, fill: Trade) -> None:
        self.filled_quantity += _dec(fill.quantity)


class RebalancingStrategy(Strategy):
    """StockSharp-style Portfolio Rebalancing Strategy base class.

    Maintains a dictionary of ``target_weights`` (mapping symbol or instrument key
    to target portfolio weight in ``[0, 1]``) and generates sell-first, buy-second
    rebalancing orders whenever an instrument's weight drifts beyond
    ``drift_threshold_bps`` basis points.
    """

    name: ClassVar[str] = "rebalancing"
    drift_threshold_bps = Parameter(default=50.0, min=0.0, max=2000.0, step=5.0)

    def __init__(
        self,
        target_weights: dict[str, Decimal | float | str] | None = None,
        *,
        drift_threshold_bps: Decimal | float | str = Decimal(50),
        **kwargs: Any,
    ) -> None:
        self.target_weights: dict[str, Decimal] = {
            k: _dec(v) for k, v in (target_weights or {}).items()
        }
        self._drift_threshold = _dec(drift_threshold_bps) / Decimal(10000)

    def compute_rebalance_deltas(
        self,
        portfolio_value: Decimal | float | str,
        current_positions: dict[str, Decimal | float | str],
        prices: dict[str, Decimal | float | str],
    ) -> dict[str, Decimal]:
        """Compute integer share deltas needed to restore ``target_weights``."""
        nav = _dec(portfolio_value)
        if nav <= 0:
            return {}
        deltas: dict[str, Decimal] = {}
        all_keys = set(self.target_weights.keys()) | set(current_positions.keys())
        for key in sorted(all_keys):
            px = _dec(prices.get(key, 0))
            if px <= 0:
                continue
            curr_qty = _dec(current_positions.get(key, 0))
            curr_weight = (curr_qty * px) / nav
            target_weight = self.target_weights.get(key, Decimal(0))
            if abs(target_weight - curr_weight) < self._drift_threshold:
                continue
            target_notional = nav * target_weight
            target_qty = target_notional // px
            delta = target_qty - curr_qty
            if delta != 0:
                deltas[key] = delta
        return deltas

    def rebalance(
        self,
        instruments: dict[str, InstrumentId],
        portfolio_value: Decimal | float | str,
        current_positions: dict[str, Decimal | float | str],
        prices: dict[str, Decimal | float | str],
    ) -> list[tuple[str, Decimal]]:
        """Execute two-phase rebalance (sells first to free cash, then buys)."""
        deltas = self.compute_rebalance_deltas(
            portfolio_value, current_positions, prices
        )
        executed: list[tuple[str, Decimal]] = []
        # Phase 1: sells
        for key, delta in deltas.items():
            if delta < 0 and key in instruments:
                self.sell(instruments[key], float(-delta))
                executed.append((key, delta))
        # Phase 2: buys
        for key, delta in deltas.items():
            if delta > 0 and key in instruments:
                self.buy(instruments[key], float(delta))
                executed.append((key, delta))
        return executed
