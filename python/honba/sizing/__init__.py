"""Dynamic position sizing engines for :mod:`honba`.

A sizer answers one question: *given this signal and this account equity, how
many lots may I trade?* The answer is always an exact
:class:`honba.PositionSizingResult` — a lot-rounded quantity plus its notional
and risk at stake — never a float.

Sizing is deliberately separate from signal generation. A strategy decides
*what* to trade; a sizer decides *how much*. Swapping a
:class:`FixedFractionalSizer` for an :class:`AtrVolatilitySizer` changes
nothing else in the strategy, which is what makes research-to-live parity
testable.

Standard sizers
---------------
* :class:`FixedFractionalSizer` — risk a fixed % of equity against the stop.
* :class:`AtrVolatilitySizer` — risk a fixed % of equity against an ATR band.
* :class:`KellySizer` — damped Kelly growth sizing.
* :class:`EqualRiskSizer` — inverse-volatility risk parity (single or batch).
* :class:`FixedNotionalSizer` — deploy a fixed cash notional per trade.

All of them wrap the native :mod:`honba._honba` kernels, so a Python backtest
and a live Rust runner round the same lot the same way, to the last paisa.
"""

from __future__ import annotations

from honba._honba import PositionSizingResult, Signal  # noqa: F401  (re-exported contract types)
from honba.sizing.base import BasePositionSizer
from honba.sizing.equal_risk import EqualRiskSizer
from honba.sizing.fixed_fractional import FixedFractionalSizer
from honba.sizing.fixed_notional import FixedNotionalSizer
from honba.sizing.kelly import KellySizer
from honba.sizing.volatility import AtrVolatilitySizer

__all__ = [
    "BasePositionSizer",
    "FixedFractionalSizer",
    "AtrVolatilitySizer",
    "KellySizer",
    "EqualRiskSizer",
    "FixedNotionalSizer",
    "PositionSizingResult",
    "Signal",
]

# Canonical sizer registry keyed by the stable ``name`` of each sizer, so a
# strategy (or ``ctx.size_signal(sizer=...)``) can select one by string.
SIZERS: dict[str, type[BasePositionSizer]] = {
    sizer.name: sizer
    for sizer in (
        FixedFractionalSizer,
        AtrVolatilitySizer,
        KellySizer,
        EqualRiskSizer,
        FixedNotionalSizer,
    )
}

def sizer_by_name(name: str) -> type[BasePositionSizer]:
    """Return the sizer class registered under ``name``.

    Raises :class:`KeyError` for an unknown name.
    """
    try:
        return SIZERS[name]
    except KeyError as exc:
        raise KeyError(
            f"unknown sizer {name!r}; expected one of {sorted(SIZERS)}"
        ) from exc
