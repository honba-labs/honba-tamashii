"""Phase 7: High-performance dynamic position sizing bridge verification.

Covers the native :mod:`honba._honba` sizing kernels and the idiomatic
:mod:`honba.sizing` Python SDK: exact-decimal precision, lot rounding, risk
bounds, float refusal at the boundary, and ``ctx.size_signal`` integration.
"""

from __future__ import annotations

from decimal import Decimal
from typing import Any

import honba
import pytest
from honba._honba import (
    AtrVolatilitySizer,
    EqualRiskSizer,
    FixedFractionalSizer,
    FixedNotionalSizer,
    InstrumentId,
    KellySizer,
    Money,
    PositionSizingRequest,
    PositionSizingResult,
    Price,
    Quantity,
    RiskBudget,
    Signal,
    SizingMethod,
    size_batch,
)
from honba.sizing import (
    BasePositionSizer,
    EqualRiskSizer as PyEqualRiskSizer,
    FixedFractionalSizer as PyFixedFractionalSizer,
    FixedNotionalSizer as PyFixedNotionalSizer,
    KellySizer as PyKellySizer,
    SIZERS,
    sizer_by_name,
)
from honba.sizing.volatility import AtrVolatilitySizer as PyAtrVolatilitySizer

RELIANCE = InstrumentId.from_string("NSE:RELIANCE")
TCS = InstrumentId.from_string("NSE:TCS")


def _money(amount: str | Decimal) -> Money:
    return Money(str(amount), "INR")


def _price(text: str) -> Price:
    return Price(text)


def _qty(text: str) -> Quantity:
    return Quantity(text)


def _signal(
    instrument: InstrumentId = RELIANCE,
    direction: str = "bullish",
    entry: str = "2540.50",
    stop: str | None = "2500.00",
) -> Signal:
    s = Signal(instrument, direction, entry_price=_price(entry))
    if stop is not None:
        s = s.with_stop_loss(_price(stop))
    return s


class TestNativeSizingKernels:
    """The Rust sizing kernels exposed on ``honba._honba``."""

    def test_fixed_fractional_lot_rounding_and_risk_bound(self) -> None:
        # Equity 1,000,000 INR, risk 1% -> risk budget 10,000.
        # Stop distance |2540.50 - 2500.00| = 40.50.
        # Q = floor(10_000 / 40.50 / 1) * 1 = floor(246.91...) = 246.
        budget = RiskBudget(_money("1000000"), Decimal("0.01"), _qty("1"))
        method = SizingMethod.fixed_fractional(Decimal("0.01"))
        request = PositionSizingRequest(
            RELIANCE, "BUY", _price("2540.50"), _money("1000000"), budget, method
        ).with_stop_loss(_price("2500.00"))
        result = request.calculate()
        assert result.quantity == _qty("246")
        assert result.allocated_notional.to_decimal() == Decimal("624963.00")
        # 246 * 40.50 = 9,963.00 <= 10,000 risk budget.
        assert result.risk_at_stake.to_decimal() == Decimal("9963.00")
        assert result.risk_at_stake.to_decimal() <= Decimal("10000")
        assert not result.capped_by_limit
        assert result.reason is None

    def test_fixed_fractional_sizer_native(self) -> None:
        sizer = FixedFractionalSizer(Decimal("0.01"), lot_size=Decimal("1"))
        result = sizer.calculate_size(
            RELIANCE,
            _price("2540.50"),
            _money("1000000"),
            side="BUY",
            stop_loss=_price("2500.00"),
        )
        assert result.quantity == _qty("246")

    def test_risk_bound_never_exceeds_budget(self) -> None:
        # A tight stop still cannot allocate more than risk_pct of equity.
        sizer = FixedFractionalSizer(Decimal("0.02"), lot_size=Decimal("1"))
        result = sizer.calculate_size(
            RELIANCE,
            _price("1000.00"),
            _money("500000"),
            side="BUY",
            stop_loss=_price("999.90"),  # distance 0.10
        )
        assert result.risk_at_stake.to_decimal() <= Decimal("10000")

    def test_atr_volatility_sizer(self) -> None:
        sizer = AtrVolatilitySizer(
            Decimal("0.01"), Decimal("2"), lookback_bars=20, lot_size=Decimal("1")
        )
        # equity 1,000,000 * 1% = 10,000; ATR band 2 * 20 = 40.
        # Q = floor(10_000 / 40) = 250.
        result = sizer.calculate_size(
            RELIANCE,
            _price("2540.50"),
            _money("1000000"),
            Decimal("20"),
            side="BUY",
        )
        assert result.quantity == _qty("250")

    def test_kelly_sizer(self) -> None:
        sizer = KellySizer(Decimal("0.6"), Decimal("2.0"), fraction=Decimal("0.5"))
        # f* = 0.6 - 0.4/2 = 0.4; damped 0.5 -> 0.2.
        # risk = 0.2 * 1,000,000 = 200,000; stop distance 40.50.
        # Q = floor(200_000 / 40.50) = 4938.
        result = sizer.calculate_size(
            RELIANCE,
            _price("2540.50"),
            _money("1000000"),
            side="BUY",
            stop_loss=_price("2500.00"),
        )
        assert result.quantity == _qty("4938")
        assert sizer.kelly_fraction == Decimal("0.4")

    def test_fixed_notional_sizer(self) -> None:
        sizer = FixedNotionalSizer(Decimal("254050"), lot_size=Decimal("1"))
        # Q = floor(254050 / 2540.50) = 100.
        result = sizer.calculate_size(RELIANCE, _price("2540.50"), side="BUY")
        assert result.quantity == _qty("100")
        assert result.allocated_notional.to_decimal() == Decimal("254050.00")

    def test_equal_risk_sizer_single(self) -> None:
        sizer = PyEqualRiskSizer(Decimal("0.01"), lot_size=Decimal("1"))
        result = sizer.calculate_size(
            _signal(entry="2540.50"), _money("1000000"), gross_exposure=Decimal("1")
        )
        # One candidate: weight 1.0 -> notional 1,000,000; Q = floor(1_000_000/2540.50)=393.
        assert result.quantity == _qty("393")

    def test_equal_risk_allocate_batch(self) -> None:
        results = EqualRiskSizer(Decimal("0.01"), lot_size=Decimal("1")).allocate(
            _money("1000000"),
            [RELIANCE, TCS],
            [_price("2540.50"), _price("4000.00")],
            [Decimal("20"), Decimal("40")],
            gross_exposure=Decimal("1"),
        )
        assert len(results) == 2
        # Inverse vol: RELIANCE weight 2/3, TCS weight 1/3.
        # RELIANCE notional = 666,666.67 -> Q = floor(666666.67/2540.50) = 262.
        assert results[0].quantity == _qty("262")
        # TCS notional = 333,333.33 -> Q = floor(333333.33/4000) = 83.
        assert results[1].quantity == _qty("83")

    def test_size_batch_vectorized(self) -> None:
        results = size_batch(
            ["NSE:RELIANCE", "NSE:TCS"],
            [_price("2540.50"), _price("4000.00")],
            _money("1000000"),
            risk_pct=Decimal("0.01"),
            lot_size=_qty("1"),
            stop_losses=[_price("2500.00"), _price("3900.00")],
            method="fixed_fractional",
        )
        assert len(results) == 2
        assert results[0].quantity == _qty("246")
        # TCS: risk 10,000 / |4000-3900|=100 -> floor(100) = 100.
        assert results[1].quantity == _qty("100")

    def test_float_refused_at_boundary(self) -> None:
        with pytest.raises(TypeError):
            FixedFractionalSizer(10.25)  # type: ignore[arg-type]
        with pytest.raises((TypeError, ValueError)):
            FixedFractionalSizer(Decimal("0.01")).calculate_size(
                RELIANCE, 2540.50, _money("1000000")  # type: ignore[arg-type]
            )


class TestPythonSizingSdk:
    """The idiomatic :mod:`honba.sizing` wrappers."""

    def test_sizer_registry(self) -> None:
        assert set(SIZERS) == {
            "fixed_fractional",
            "atr_volatility",
            "kelly_criterion",
            "equal_risk_contribution",
            "fixed_notional",
        }
        assert sizer_by_name("kelly_criterion") is PyKellySizer
        with pytest.raises(KeyError):
            sizer_by_name("no_such_sizer")

    def test_fixed_fractional_python_wrapper(self) -> None:
        sizer = PyFixedFractionalSizer(Decimal("0.01"), lot_size=Decimal("1"))
        assert isinstance(sizer, BasePositionSizer)
        assert sizer.name == "fixed_fractional"
        result = sizer.calculate_size(_signal(), _money("1000000"))
        assert result.quantity == _qty("246")
        assert result.risk_at_stake.to_decimal() <= Decimal("10000")
        again = sizer.calculate_size(_signal(), _money("1000000"))
        assert again.quantity == result.quantity
        assert again.risk_at_stake.to_decimal() == result.risk_at_stake.to_decimal()

    def test_atr_volatility_python_wrapper(self) -> None:
        sizer = PyAtrVolatilitySizer(Decimal("0.01"), Decimal("2"))
        result = sizer.calculate_size(
            _signal(entry="2540.50", stop="2500.00"),
            _money("1000000"),
            atr=Decimal("20"),
        )
        assert result.quantity == _qty("250")

    def test_kelly_python_wrapper(self) -> None:
        sizer = PyKellySizer(Decimal("0.6"), Decimal("2.0"), fraction=Decimal("0.5"))
        assert sizer.kelly_fraction == Decimal("0.4")
        result = sizer.calculate_size(
            _signal(entry="2540.50", stop="2500.00"),
            _money("1000000"),
        )
        assert result.quantity == _qty("4938")

    def test_fixed_notional_python_wrapper(self) -> None:
        sizer = PyFixedNotionalSizer(Decimal("254050"), lot_size=Decimal("1"))
        result = sizer.calculate_size(_signal(entry="2540.50"), _money("1000000"))
        assert result.quantity == _qty("100")
        assert result.allocated_notional.to_decimal() == Decimal("254050.00")

    def test_equal_risk_python_wrapper_allocate(self) -> None:
        sizer = PyEqualRiskSizer(Decimal("0.01"), lot_size=Decimal("1"))
        results = sizer.allocate(
            _money("1000000"),
            [RELIANCE, TCS],
            [_price("2540.50"), _price("4000.00")],
            [Decimal("20"), Decimal("40")],
            gross_exposure=Decimal("1"),
        )
        assert results[0].quantity == _qty("262")
        assert results[1].quantity == _qty("83")

    def test_callable_alias(self) -> None:
        sizer = PyFixedFractionalSizer(Decimal("0.01"), lot_size=Decimal("1"))
        assert sizer(_signal(), _money("1000000")).quantity == _qty("246")

    def test_honba_export(self) -> None:
        assert hasattr(honba, "sizing")
        assert honba.sizing.FixedFractionalSizer is PyFixedFractionalSizer
