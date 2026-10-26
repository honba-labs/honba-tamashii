"""The position sizer contract every ``honba.sizing`` sizer implements.

A sizer answers one question: *given this signal and this account equity, how
many lots may I trade?* The answer is always an exact
:class:`honba.PositionSizingResult` — a lot-rounded quantity plus its notional
and risk at stake — never a float.

Sizing is deliberately separate from signal generation. A strategy decides
*what* to trade; a sizer decides *how much*. Swapping a
:class:`~honba.sizing.FixedFractionalSizer` for an
:class:`~honba.sizing.AtrVolatilitySizer` changes nothing else in the
strategy, which is what makes research-to-live parity testable.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from decimal import Decimal
from typing import Any, ClassVar

from honba._honba import Money, PositionSizingResult, Signal

__all__ = ["BasePositionSizer"]


class BasePositionSizer(ABC):
    """Abstract base class for dynamic position sizers.

    Subclasses implement :meth:`calculate_size`. The signature is fixed so a
    strategy can be written against the interface and retargeted between
    sizers — and between Python and the native Rust kernels — without edits.
    """

    #: Stable identifier used in logs, reports and configuration files.
    name: ClassVar[str] = "base"

    @abstractmethod
    def calculate_size(
        self,
        signal: Signal,
        equity: Money | Decimal,
        **kwargs: Any,
    ) -> PositionSizingResult:
        """Size ``signal`` against ``equity``.

        Args:
            signal: The trading signal. It must carry an ``entry_price`` and a
                non-neutral direction (exposed as ``.side``).
            equity: Account equity as :class:`honba.Money` or a bare
                :class:`decimal.Decimal` amount (interpreted as INR).
            **kwargs: Sizer-specific parameters (``atr``, ``stop_loss``, ...).

        Returns:
            The executable quantity, already rounded down to whole lots, with
            its notional and risk at stake.
        """
        raise NotImplementedError

    def __call__(
        self,
        signal: Signal,
        equity: Money | Decimal,
        **kwargs: Any,
    ) -> PositionSizingResult:
        """Alias for :meth:`calculate_size`, so a sizer is directly callable."""
        return self.calculate_size(signal, equity, **kwargs)

    # -- helpers shared by the concrete sizers --------------------------------

    @staticmethod
    def _entry_price(signal: Signal) -> Any:
        """The signal's entry price, or a clear error when it has none."""
        entry = getattr(signal, "entry_price", None)
        if entry is None:
            instrument = getattr(signal, "instrument", "?")
            raise ValueError(
                f"signal for {instrument!r} has no entry_price; "
                "attach one with Signal.with_entry_price(...) before sizing"
            )
        return entry

    @staticmethod
    def _side(signal: Signal) -> str:
        """The order side implied by the direction (``BUY`` / ``SELL``)."""
        side = getattr(signal, "side", None)
        if side is None:
            raise ValueError("a neutral signal has no order side to size")
        return side

    @staticmethod
    def _stop_loss(signal: Signal, override: Any = None) -> Any:
        """The explicit stop, else the signal's own stop, else ``None``."""
        if override is not None:
            return override
        return getattr(signal, "stop_loss", None)
