"""Fixed-fractional position sizing.

Risks a fixed percentage of account equity per trade, divided by the stop-loss
distance — the classic "risk 1% of equity per trade" rule.

    Q = floor(Equity * RiskPct / |P_entry - P_stop| / LotSize) * LotSize

The arithmetic runs in the native :class:`honba._honba.FixedFractionalSizer`
kernel, so the result is byte-for-byte the same as the Rust engine's.
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any

from honba._honba import FixedFractionalSizer as _NativeFixedFractionalSizer
from honba._honba import Money, PositionSizingResult, Signal

from honba.sizing.base import BasePositionSizer

__all__ = ["FixedFractionalSizer"]


class FixedFractionalSizer(BasePositionSizer):
    """Risk a fixed fraction of equity against the stop distance."""

    name = "fixed_fractional"

    def __init__(
        self,
        risk_pct: Decimal | str = Decimal("0.01"),
        *,
        lot_size: Any = None,
        max_position_notional: Any = None,
        max_position_qty: Any = None,
        freeze_limit_qty: Any = None,
    ) -> None:
        """Build a fixed-fractional sizer.

        Args:
            risk_pct: Fraction of equity to risk per trade (``0.01`` = 1%).
            lot_size: Exchange lot size used for discretisation (default 1).
            max_position_notional: Optional cash notional cap.
            max_position_qty: Optional quantity cap.
            freeze_limit_qty: Optional scrip freeze limit.
        """
        self._kernel = _NativeFixedFractionalSizer(
            risk_pct,
            lot_size=lot_size,
            max_position_notional=max_position_notional,
            max_position_qty=max_position_qty,
            freeze_limit_qty=freeze_limit_qty,
        )

    @property
    def risk_pct(self) -> Decimal:
        """The fraction of equity risked per trade."""
        return self._kernel.risk_pct

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
        """Size ``signal`` at ``risk_pct`` of ``equity`` against its stop."""
        stop_loss = self._stop_loss(signal, kwargs.pop("stop_loss", None))
        return self._kernel.calculate_size(
            signal.instrument,
            self._entry_price(signal),
            equity,
            side=self._side(signal),
            stop_loss=stop_loss,
        )

    def __repr__(self) -> str:
        return f"FixedFractionalSizer(risk_pct={self.risk_pct!r})"
