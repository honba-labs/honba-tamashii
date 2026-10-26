"""Fixed-notional position sizing.

Converts a fixed cash notional into a lot-rounded quantity:

    Q = floor(Notional / P_entry / LotSize) * LotSize

The arithmetic runs in the native :class:`honba._honba.FixedNotionalSizer`
kernel, so the lot rounding and caps are shared with every other sizer. Unlike
the risk-based sizers, this does not derive the quantity from a stop-loss.
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any

from honba._honba import FixedNotionalSizer as _NativeFixedNotionalSizer
from honba._honba import Money, PositionSizingResult, Signal

from honba.sizing.base import BasePositionSizer

__all__ = ["FixedNotionalSizer"]


class FixedNotionalSizer(BasePositionSizer):
    """Allocate a fixed cash notional per trade, lot-rounded."""

    name = "fixed_notional"

    def __init__(
        self,
        notional: Decimal | str | Money,
        *,
        lot_size: Any = None,
        max_position_notional: Any = None,
        max_position_qty: Any = None,
        freeze_limit_qty: Any = None,
    ) -> None:
        """Build a fixed-notional sizer.

        Args:
            notional: Cash notional to deploy per trade.
            lot_size: Exchange lot size used for discretisation (default 1).
            max_position_notional: Optional cash notional cap.
            max_position_qty: Optional quantity cap.
            freeze_limit_qty: Optional scrip freeze limit.
        """
        self._kernel = _NativeFixedNotionalSizer(
            notional,
            lot_size=lot_size,
            max_position_notional=max_position_notional,
            max_position_qty=max_position_qty,
            freeze_limit_qty=freeze_limit_qty,
        )
        self._notional = _money_amount(notional)
        self._currency = _money_currency(notional)

    @property
    def notional(self) -> Decimal:
        """The cash notional deployed per trade."""
        return self._notional

    @property
    def currency(self) -> str:
        """The currency of the notional."""
        return self._currency

    @property
    def lot_size(self) -> Any:
        """The exchange lot size used for rounding."""
        return self._kernel.lot_size

    def calculate_size(
        self,
        signal: Signal,
        equity: Money | Decimal,
        **kwargs: Any,
    ) -> PositionSizingResult:
        """Size ``signal`` at the fixed notional (equity and stop are accepted
        for the interface but not used to scale the notional)."""
        return self._kernel.calculate_size(
            signal.instrument,
            self._entry_price(signal),
            side=self._side(signal),
        )

    def __repr__(self) -> str:
        return f"FixedNotionalSizer(notional={self.notional!r}, currency={self.currency!r})"


def _money_amount(value: Decimal | str | Money) -> Decimal:
    if hasattr(value, "to_decimal"):
        return value.to_decimal()
    return Decimal(str(value))


def _money_currency(value: Decimal | str | Money) -> str:
    return getattr(value, "currency", None) or "INR"
