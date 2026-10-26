"""ATR / volatility-band position sizing.

Sizes inversely to the instrument's volatility: a wide ATR band means a
smaller position for the same rupee risk.

    Q = floor(Equity * TargetRisk / (ATR * Multiplier) / LotSize) * LotSize

The ATR is a price distance; the native kernel expresses it as a fraction of
the entry price and applies the canonical ``volatility_target`` model, so the
lot rounding and caps are shared with every other sizer.
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any

from honba._honba import AtrVolatilitySizer as _NativeAtrVolatilitySizer
from honba._honba import Money, PositionSizingResult, Signal

from honba.sizing.base import BasePositionSizer

__all__ = ["AtrVolatilitySizer"]


class AtrVolatilitySizer(BasePositionSizer):
    """Size inversely proportional to the ATR band."""

    name = "atr_volatility"

    def __init__(
        self,
        target_risk: Decimal | str = Decimal("0.01"),
        multiplier: Decimal | str = Decimal("1"),
        *,
        lookback_bars: int = 20,
        lot_size: Any = None,
        max_position_notional: Any = None,
        max_position_qty: Any = None,
        freeze_limit_qty: Any = None,
    ) -> None:
        """Build an ATR-band sizer.

        Args:
            target_risk: Fraction of equity to risk per trade.
            multiplier: Multiple of the ATR used as the stop band (e.g. ``2``
                for a two-ATR stop).
            lookback_bars: ATR lookback recorded on the request.
        """
        self._kernel = _NativeAtrVolatilitySizer(
            target_risk,
            multiplier,
            lookback_bars=lookback_bars,
            lot_size=lot_size,
            max_position_notional=max_position_notional,
            max_position_qty=max_position_qty,
            freeze_limit_qty=freeze_limit_qty,
        )

    @property
    def target_risk(self) -> Decimal:
        """The fraction of equity risked per trade."""
        return self._kernel.target_risk

    @property
    def multiplier(self) -> Decimal:
        """The ATR multiple used as the stop band."""
        return self._kernel.multiplier

    def calculate_size(
        self,
        signal: Signal,
        equity: Money | Decimal,
        **kwargs: Any,
    ) -> PositionSizingResult:
        """Size ``signal`` from its ATR (or the ``atr=`` override)."""
        atr = kwargs.pop("atr", None)
        if atr is None:
            atr = getattr(signal, "volatility_atr", None)
        if atr is None:
            instrument = getattr(signal, "instrument", "?")
            raise ValueError(
                f"signal for {instrument!r} has no volatility_atr; "
                "attach one with Signal.with_volatility_atr(...) or pass atr="
            )
        stop_loss = self._stop_loss(signal, kwargs.pop("stop_loss", None))
        return self._kernel.calculate_size(
            signal.instrument,
            self._entry_price(signal),
            equity,
            atr,
            side=self._side(signal),
            stop_loss=stop_loss,
        )

    def __repr__(self) -> str:
        return (
            "AtrVolatilitySizer("
            f"target_risk={self.target_risk!r}, multiplier={self.multiplier!r})"
        )
