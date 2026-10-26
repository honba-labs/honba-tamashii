"""Kelly-criterion position sizing.

Sizes from the edge of a strategy: the Kelly fraction ``f* = p - (1 - p) / b``
(win rate ``p``, payoff ratio ``b``), damped by a safety ``fraction`` (default
half-Kelly). The arithmetic runs in the native :class:`honba._honba.KellySizer`
kernel, so the lot rounding and caps are shared with every other sizer.

    f = fraction * max(0, p - (1 - p) / b)
    Q = floor(f * Equity / |P_entry - P_stop| / LotSize) * LotSize
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any

from honba._honba import KellySizer as _NativeKellySizer
from honba._honba import Money, PositionSizingResult, Signal

from honba.sizing.base import BasePositionSizer

__all__ = ["KellySizer"]


class KellySizer(BasePositionSizer):
    """Damped Kelly growth sizing (the default ``fraction`` is half-Kelly)."""

    name = "kelly_criterion"

    def __init__(
        self,
        win_rate: Decimal | str,
        payoff_ratio: Decimal | str,
        *,
        fraction: Decimal | str = Decimal("0.5"),
        lot_size: Any = None,
        max_position_notional: Any = None,
        max_position_qty: Any = None,
        freeze_limit_qty: Any = None,
    ) -> None:
        """Build a Kelly sizer.

        Args:
            win_rate: Probability of a winning trade, in ``[0, 1]``.
            payoff_ratio: Average win divided by average loss (``b``).
            fraction: Safety damping applied to the raw Kelly fraction
                (``0.5`` = half-Kelly).
            lot_size: Exchange lot size used for discretisation (default 1).
            max_position_notional: Optional cash notional cap.
            max_position_qty: Optional quantity cap.
            freeze_limit_qty: Optional scrip freeze limit.
        """
        self._kernel = _NativeKellySizer(
            win_rate,
            payoff_ratio,
            fraction=fraction,
            lot_size=lot_size,
            max_position_notional=max_position_notional,
            max_position_qty=max_position_qty,
            freeze_limit_qty=freeze_limit_qty,
        )

    @property
    def win_rate(self) -> Decimal:
        """The assumed probability of a winning trade."""
        return self._kernel.win_rate

    @property
    def payoff_ratio(self) -> Decimal:
        """Average win divided by average loss."""
        return self._kernel.payoff_ratio

    @property
    def fraction(self) -> Decimal:
        """The damping applied to the raw Kelly fraction."""
        return self._kernel.fraction

    @property
    def kelly_fraction(self) -> Decimal:
        """The raw (undamped) Kelly fraction, ``p - (1 - p) / b`` (never negative)."""
        return self._kernel.kelly_fraction

    def calculate_size(
        self,
        signal: Signal,
        equity: Money | Decimal,
        **kwargs: Any,
    ) -> PositionSizingResult:
        """Size ``signal`` from the damped Kelly fraction against its stop."""
        stop_loss = self._stop_loss(signal, kwargs.pop("stop_loss", None))
        return self._kernel.calculate_size(
            signal.instrument,
            self._entry_price(signal),
            equity,
            side=self._side(signal),
            stop_loss=stop_loss,
        )

    def __repr__(self) -> str:
        return (
            "KellySizer("
            f"win_rate={self.win_rate!r}, payoff_ratio={self.payoff_ratio!r}, "
            f"fraction={self.fraction!r})"
        )
