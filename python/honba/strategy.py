"""Base class for honba strategies.

Subclass and override any of:

    setup(self)              — once, before the first bar
    on_start(self)           — once, when the runner starts
    on_tick(self, tick, ctx) — per tick (for tick-level strategies)
    on_bar(self, bar, ctx)   — per bar; call ctx.submit(...) to place orders
    on_order_event(self, event, ctx) — order lifecycle events
    on_position_event(self, event, ctx) — position lifecycle events
    on_stop(self)            — once, after the last bar / market close

The Rust engine calls these in order. A strategy that raises will be caught
by the runner's GIL-disciplined wrapper and the strategy state will be
transitioned to ``Degraded`` with automated protective order cancellations
via ``honba-mon`` rather than crashing the host process.
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any, Optional, TypeVar, Generic, List, Dict

from honba._honba import Bar, InstrumentId, Metrics, PyContext, RiskRefusalError, SMA, Tick

T = TypeVar("T")


class PyMetrics(Generic[T]):
    """Result object returned by ``Runner.run()``.

    Contains performance metrics and the double-entry ledger of statutory
    charges (STT, GST, SEBI, stamp duty).
    """

    def __init__(
        self,
        *,
        total_return: Decimal,
        cagr: Decimal,
        annual_volatility: Decimal,
        sharpe_ratio: Decimal,
        sortino_ratio: Decimal,
        max_drawdown: Decimal,
        dsr: Decimal,  # Daily Shortfall Ratio
        pbo: Decimal,  # Profitability Ratio
        tax_breakdown: dict[str, Decimal],
        final_cash: Decimal,
        orders: int,
        trades: int,
        degraded: bool,
        errors: List[str],
    ):
        self.total_return = total_return
        self.cagr = cagr
        self.annual_volatility = annual_volatility
        self.sharpe_ratio = sharpe_ratio
        self.sortino_ratio = sortino_ratio
        self.max_drawdown = max_drawdown
        self.dsr = dsr
        self.pbo = pbo
        self.tax_breakdown = tax_breakdown
        self.final_cash = final_cash
        self.orders = orders
        self.trades = trades
        self.degraded = degraded
        self.errors = errors

    def __repr__(self) -> str:
        return (
            f"PyMetrics(total_return={self.total_return!r}, "
            f"sharpe={self.sharpe_ratio!r}, max_drawdown={self.max_drawdown!r}, "
            f"degraded={self.degraded})"
        )


class Strategy:
    """Ergonomic base class for honba strategies.

    Provides declarative ``Parameter`` descriptors, property accessors
    backed by the runner's ``PyContext``, and high-level execution
    helpers ``self.buy()`` / ``self.sell()`` / ``self.close_position()``
    / ``self.cancel_all()``.
    """

    def __init__(self) -> None:
        self._parameters: dict[str, Any] = {}
        self._params: dict[str, Any] = {}

    # ------------------------------------------------------------------
    # Lifecycle hooks (override as needed)
    # ------------------------------------------------------------------

    def setup(self) -> None:
        """Called once before the first bar or tick. Override to initialise state."""

    def on_start(self) -> None:
        """Called once when the runner starts. Override for per-strategy init."""

    def on_tick(self, tick: Any, ctx: PyContext) -> None:
        """Called per real-time tick (for tick-level strategies). Default no-op."""

    def on_bar(self, bar: Bar, ctx: PyContext) -> None:
        """Called for every bar close. Override to make trading decisions."""

    def on_order_event(self, event: dict[str, Any], ctx: PyContext) -> None:
        """Called on order lifecycle events (accepted, filled, cancelled, rejected).

        *event* has keys ``"status"``, ``"order_id"``, ``"fill_report"`` when
        available.
        """

    def on_position_event(self, event: dict[str, Any], ctx: PyContext) -> None:
        """Called on position lifecycle events (opened, reduced, closed, reversed)."""

    def on_stop(self) -> None:
        """Called once after the last bar or on market close. Override to log/close out."""

    # ------------------------------------------------------------------
    # Property accessors (populated by the runner before each callback)
    # ------------------------------------------------------------------

    @property
    def portfolio(self) -> dict[str, Any]:
        """Return the current portfolio state as seen by the strategy."""
        return {}

    @property
    def positions(self) -> dict[str, Any]:
        """Return per-instrument position snapshots."""
        return {}

    @property
    def cash(self) -> Decimal:
        """Current available cash balance."""
        return Decimal("0")

    @property
    def indicators(self) -> dict[str, Any]:
        """Return the latest indicator values."""
        return {}

    # ------------------------------------------------------------------
    # Declarative Parameter descriptor (for Genetic / Bayesian optimisation)
    # ------------------------------------------------------------------

    class Parameter:
        """Descriptor exposing a tunable parameter to the strategy body.

        Usage::

            class MyStrategy(Strategy):
                mass = Parameter(default=1.0, min=0.1, max=10.0, step=0.1)

        The ``default`` value is stored in ``self._parameters["mass]`` and
        exposed via ``self.mass`` at runtime.
        """

        def __init__(
            self,
            *,
            default: Any,
            min: Optional[Any] = None,
            max: Optional[Any] = None,
            step: Optional[Any] = None,
            description: str = "",
        ):
            self.default = default
            self.min = min
            self.max = max
            self.step = step
            self.description = description

        def __set_name__(self, owner: type, name: str) -> None:
            self.param_name = name

        def __get__(self, instance: "Strategy", owner: type) -> Any:
            if instance is None:
                return self
            return instance._parameters.get(self.param_name, self.default)

        def __set__(self, instance: "Strategy", value: Any) -> None:
            instance._parameters[self.param_name] = value

    # ------------------------------------------------------------------
    # High-level execution helpers (convenience methods)
    # ------------------------------------------------------------------

    def buy(
        self,
        ctx: PyContext,
        instrument: str,
        quantity: Decimal | str,
        price: Optional[Decimal | str] = None,
        order_type: str = "limit",
    ) -> Any:
        """Submit a buy order.

        Parameters
        ----------
        ctx : PyContext
            The strategy context.
        instrument : str
            Instrument ID, e.g. ``"NSE:RELIANCE"``.
        quantity : Decimal or str
            Order quantity (e.g. ``Decimal("10")`` or ``"10"``).
        price : Decimal or str, optional
            Limit price for a limit order; ``None`` defaults to market.
        order_type : str
            ``"limit"`` | ``"market"`` | ``"stop"`` | ``"stop_limit"``.

        Returns
        -------
        The order ID assigned by the engine.
        """
        qty = Decimal(quantity) if not isinstance(quantity, Decimal) else quantity
        if price is not None:
            price = Decimal(price)
        order = ctx.submit(instrument, "BUY", qty, price=price, order_type=order_type)
        return order

    def sell(
        self,
        ctx: PyContext,
        instrument: str,
        quantity: Decimal | str,
        price: Optional[Decimal | str] = None,
        order_type: str = "limit",
    ) -> Any:
        """Submit a sell order.

        Parameters
        ----------
        ctx : PyContext
            The strategy context.
        instrument : str
            Instrument ID.
        quantity : Decimal or str
            Order quantity.
        price : Decimal or str, optional
            Limit price.
        order_type : str
            ``"limit"`` | ``"market"`` | ``"stop"`` | ``"stop_limit"``.

        Returns
        -------
        The order ID assigned by the engine.
        """
        qty = Decimal(quantity) if not isinstance(quantity, Decimal) else quantity
        if price is not None:
            price = Decimal(price)
        order = ctx.submit(instrument, "SELL", qty, price=price, order_type=order_type)
        return order

    def close_position(
        self,
        ctx: PyContext,
        instrument: str,
        quantity: Optional[Decimal | str] = None,
    ) -> Any:
        """Close (or reduce) a position in *instrument*.

        If *quantity* is omitted the entire position is closed at market.
        """
        qty = Decimal(quantity) if quantity is not None else None
        order = ctx.submit(instrument, "CLOSE", qty or 0)
        return order

    def cancel_all(self, ctx: PyContext) -> list[Any]:
        """Cancel all open orders for this strategy's ``algo_id``."""
        return ctx.cancel_all()

    # ------------------------------------------------------------------
    # Convenient typed accessors — overridden by runner if desired
    # ------------------------------------------------------------------

    def _read_cash(self) -> Decimal:
        return self.cash

    def _read_positions(self) -> dict[str, Any]:
        return self.positions