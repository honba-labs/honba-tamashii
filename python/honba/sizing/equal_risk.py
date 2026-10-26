"""Equal-risk (risk-parity) position sizing.

Allocates gross exposure across a universe of candidates in inverse proportion
to their volatility, then rounds every leg down to whole lots through the
canonical kernel. The native
:class:`honba._honba.EqualRiskSizer` owns the batch allocation; a single signal
is treated as the only candidate (weight ``1.0``).

    weight_i = (1 / vol_i) / sum_j (1 / vol_j)
    notional_i = Equity * gross_exposure * weight_i
    Q_i = floor(notional_i / P_entry_i / LotSize) * LotSize
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any, Sequence

from honba._honba import (
    EqualRiskSizer as _NativeEqualRiskSizer,
    FixedNotionalSizer as _NativeFixedNotionalSizer,
)
from honba._honba import Money, PositionSizingResult, Signal

from honba.sizing.base import BasePositionSizer

__all__ = ["EqualRiskSizer"]


class EqualRiskSizer(BasePositionSizer):
    """Inverse-volatility risk parity across active signals."""

    name = "equal_risk_contribution"

    def __init__(
        self,
        risk_pct: Decimal | str = Decimal("0.01"),
        *,
        lot_size: Any = None,
        max_position_notional: Any = None,
        max_position_qty: Any = None,
        freeze_limit_qty: Any = None,
    ) -> None:
        """Build an equal-risk sizer.

        Args:
            risk_pct: Portfolio risk fraction recorded on each request.
            lot_size: Exchange lot size used for discretisation (default 1).
            max_position_notional: Optional cash notional cap.
            max_position_qty: Optional quantity cap.
            freeze_limit_qty: Optional scrip freeze limit.
        """
        self._kernel = _NativeEqualRiskSizer(
            risk_pct,
            lot_size=lot_size,
            max_position_notional=max_position_notional,
            max_position_qty=max_position_qty,
            freeze_limit_qty=freeze_limit_qty,
        )
        # Single-signal sizing delegates to the canonical fixed-notional kernel
        # with the full gross exposure as the one candidate's weight (1.0).
        self._notional_kernel: _NativeFixedNotionalSizer | None = None

    @property
    def risk_pct(self) -> Decimal:
        """The portfolio risk fraction recorded on each request."""
        return self._kernel.risk_pct

    @property
    def lot_size(self) -> Any:
        """The exchange lot size used for rounding."""
        return self._kernel.lot_size

    def _single_notional_kernel(
        self, equity: Money | Decimal, gross_exposure: Decimal
    ) -> _NativeFixedNotionalSizer:
        if self._notional_kernel is None:
            notional = _money_amount(equity) * gross_exposure
            currency = _money_currency(equity)
            self._notional_kernel = _NativeFixedNotionalSizer(
                notional,
                lot_size=self.lot_size,
                max_position_notional=None,
                max_position_qty=None,
                freeze_limit_qty=None,
            )
        return self._notional_kernel

    def calculate_size(
        self,
        signal: Signal,
        equity: Money | Decimal,
        **kwargs: Any,
    ) -> PositionSizingResult:
        """Size ``signal`` as the only candidate (weight 1.0) of gross exposure."""
        gross_exposure = kwargs.pop("gross_exposure", Decimal("1"))
        if isinstance(gross_exposure, (int, float, str)):
            gross_exposure = Decimal(str(gross_exposure))
        stop_loss = self._stop_loss(signal, kwargs.pop("stop_loss", None))
        # One candidate: weight 1.0, so the full gross exposure goes to it.
        kernel = self._single_notional_kernel(equity, gross_exposure)
        return kernel.calculate_size(
            signal.instrument,
            self._entry_price(signal),
            side=self._side(signal),
        )

    def allocate(
        self,
        equity: Money | Decimal,
        instruments: Sequence[Any],
        entry_prices: Sequence[Any],
        volatilities: Sequence[Any],
        gross_exposure: Decimal | str = Decimal("1"),
    ) -> list[PositionSizingResult]:
        """Size a whole universe in one native batch call (zero-copy friendly).

        Args:
            equity: Account equity.
            instruments: Candidate instruments.
            entry_prices: Candidate entry prices.
            volatilities: Candidate volatilities (higher vol → smaller weight).
            gross_exposure: Fraction of equity deployed across the universe.

        Returns:
            One lot-rounded :class:`PositionSizingResult` per candidate.
        """
        if isinstance(gross_exposure, (int, float, str)):
            gross_exposure = Decimal(str(gross_exposure))
        return self._kernel.allocate(
            equity,
            list(instruments),
            list(entry_prices),
            list(volatilities),
            gross_exposure=gross_exposure,
        )

    def __repr__(self) -> str:
        return f"EqualRiskSizer(risk_pct={self.risk_pct!r})"


def _money_amount(value: Money | Decimal) -> Decimal:
    return value.to_decimal() if hasattr(value, "to_decimal") else Decimal(str(value))


def _money_currency(value: Money | Decimal) -> str:
    return getattr(value, "currency", None) or "INR"
