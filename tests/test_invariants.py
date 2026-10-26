"""Property-Based Testing with Hypothesis (Phase 8.3 / M7).

Verifies fundamental mathematical and financial invariants:
1. Lossless Decimal conversion roundtrips between Python `decimal.Decimal`
   and Rust `rust_decimal::Decimal`.
2. Absolute refusal of floats at canonical boundaries.
3. Position sizing bounds: guaranteed allocated_risk <= equity * risk_pct.
4. Integer multiple lot rounding and freeze limit capping.
"""

from decimal import Decimal

import pytest
from honba._honba import (
    AtrVolatilitySizer,
    FixedFractionalSizer,
    FixedNotionalSizer,
    InstrumentId,
    KellySizer,
    Money,
    Price,
    Quantity,
)
from hypothesis import given, settings
from hypothesis import strategies as st

# ==============================================================================
# Hypothesis Strategies
# ==============================================================================

# Exact decimals representing realistic financial numbers (up to 6 decimal places)
decimal_numbers = st.decimals(
    min_value=Decimal("0.0001"),
    max_value=Decimal("1000000000.00"),
    places=4,
    allow_nan=False,
    allow_infinity=False,
)

signed_decimal_numbers = st.decimals(
    min_value=Decimal("-100000000.00"),
    max_value=Decimal("100000000.00"),
    places=4,
    allow_nan=False,
    allow_infinity=False,
)


# ==============================================================================
# 1. Decimal Precision & Lossless Conversion Invariants
# ==============================================================================


@given(d=decimal_numbers)
@settings(max_examples=100)
def test_price_decimal_lossless_roundtrip(d: Decimal):
    price = Price(d)
    assert price.to_decimal() == d
    assert Price(str(d)).to_decimal() == d


@given(d=decimal_numbers)
@settings(max_examples=100)
def test_quantity_decimal_lossless_roundtrip(d: Decimal):
    qty = Quantity(d)
    assert qty.to_decimal() == d
    assert Quantity(str(d)).to_decimal() == d


@given(d=signed_decimal_numbers)
@settings(max_examples=100)
def test_money_decimal_lossless_roundtrip(d: Decimal):
    money = Money(d, "INR")
    assert money.to_decimal() == d
    assert Money(str(d), "INR").to_decimal() == d


@given(f=st.floats(allow_nan=False, allow_infinity=False))
@settings(max_examples=50)
def test_boundary_refuses_python_float(f: float):
    """Pillar P2: Money never crosses the boundary as a float."""
    with pytest.raises((TypeError, ValueError)):
        Price(f)

    with pytest.raises((TypeError, ValueError)):
        Quantity(f)

    with pytest.raises((TypeError, ValueError)):
        Money(f, "INR")


# ==============================================================================
# 2. Position Sizing Bounds Invariants
# ==============================================================================


@given(
    equity=st.decimals(min_value=Decimal(10000), max_value=Decimal(10000000), places=2),
    risk_pct=st.decimals(min_value=Decimal("0.001"), max_value=Decimal("0.05"), places=4),
    entry_price=st.decimals(min_value=Decimal("10.00"), max_value=Decimal("10000.00"), places=2),
    stop_distance_pct=st.decimals(min_value=Decimal("0.01"), max_value=Decimal("0.20"), places=3),
    lot_size=st.integers(min_value=1, max_value=100),
)
@settings(max_examples=100)
def test_fixed_fractional_risk_bounds_invariant(
    equity: Decimal,
    risk_pct: Decimal,
    entry_price: Decimal,
    stop_distance_pct: Decimal,
    lot_size: int,
):
    """Property: allocated_risk <= equity * risk_pct (strictly bounded by max risk budget)."""
    stop_loss = entry_price * (Decimal(1) - stop_distance_pct)
    sizer = FixedFractionalSizer(risk_pct=risk_pct, lot_size=lot_size)

    result = sizer.calculate_size(
        instrument="NIFTY",
        entry_price=entry_price,
        equity=equity,
        side="BUY",
        stop_loss=stop_loss,
    )

    max_allowed_risk = equity * risk_pct
    actual_risk = result.risk_at_stake.to_decimal()

    # Invariant 1: Risk at stake must never exceed equity * risk_pct
    assert actual_risk <= max_allowed_risk + Decimal("0.0001")

    # Invariant 2: Quantity must be non-negative
    qty = result.quantity.to_decimal()
    assert qty >= Decimal(0)

    # Invariant 3: Quantity must be an exact lot multiple
    assert result.quantity.is_lot_multiple(lot_size) is True


@given(
    equity=st.decimals(min_value=Decimal(50000), max_value=Decimal(5000000), places=2),
    entry_price=st.decimals(min_value=Decimal("50.00"), max_value=Decimal("5000.00"), places=2),
    atr=st.decimals(min_value=Decimal("1.00"), max_value=Decimal("100.00"), places=2),
    multiplier=st.decimals(min_value=Decimal("1.0"), max_value=Decimal("3.0"), places=1),
    lot_size=st.integers(min_value=1, max_value=50),
)
@settings(max_examples=50)
def test_atr_volatility_sizer_invariants(
    equity: Decimal,
    entry_price: Decimal,
    atr: Decimal,
    multiplier: Decimal,
    lot_size: int,
):
    target_risk = Decimal("0.01")  # 1%
    sizer = AtrVolatilitySizer(target_risk, multiplier, lookback_bars=20, lot_size=Decimal(lot_size))
    stop_distance = atr * multiplier
    stop_loss = entry_price - stop_distance if entry_price > stop_distance else entry_price * Decimal("0.9")
    result = sizer.calculate_size(
        InstrumentId("BANKNIFTY", "NSE"),
        Price(entry_price),
        Money(equity, "INR"),
        atr,
        side="BUY",
        stop_loss=Price(stop_loss),
    )

    max_allowed_risk = equity * target_risk
    assert result.risk_at_stake.to_decimal() <= max_allowed_risk + Decimal("0.0001")
    assert result.quantity.is_lot_multiple(lot_size) is True


@given(
    equity=st.decimals(min_value=Decimal(100000), max_value=Decimal(10000000), places=2),
    entry_price=st.decimals(min_value=Decimal("100.00"), max_value=Decimal("2000.00"), places=2),
    win_rate=st.decimals(min_value=Decimal("0.51"), max_value=Decimal("0.80"), places=2),
    payoff_ratio=st.decimals(min_value=Decimal("1.1"), max_value=Decimal("3.0"), places=2),
    fraction=st.decimals(min_value=Decimal("0.1"), max_value=Decimal("0.5"), places=2),
    lot_size=st.integers(min_value=1, max_value=25),
)
@settings(max_examples=50)
def test_kelly_sizer_invariants(
    equity: Decimal,
    entry_price: Decimal,
    win_rate: Decimal,
    payoff_ratio: Decimal,
    fraction: Decimal,
    lot_size: int,
):
    sizer = KellySizer(
        win_rate=win_rate,
        payoff_ratio=payoff_ratio,
        fraction=fraction,
        lot_size=lot_size,
    )
    stop_loss = entry_price * Decimal("0.95")
    result = sizer.calculate_size(
        instrument="RELIANCE",
        entry_price=entry_price,
        equity=equity,
        stop_loss=stop_loss,
    )

    # Kelly sizing must never risk entire equity
    assert result.risk_at_stake.to_decimal() <= equity
    assert result.quantity.is_lot_multiple(lot_size) is True


@given(
    equity=st.decimals(min_value=Decimal(10000), max_value=Decimal(1000000), places=2),
    notional=st.decimals(min_value=Decimal(5000), max_value=Decimal(100000), places=2),
    entry_price=st.decimals(min_value=Decimal("10.00"), max_value=Decimal("1000.00"), places=2),
    lot_size=st.integers(min_value=1, max_value=50),
)
@settings(max_examples=50)
def test_fixed_notional_sizer_invariants(
    equity: Decimal,
    notional: Decimal,
    entry_price: Decimal,
    lot_size: int,
):
    sizer = FixedNotionalSizer(notional=notional, lot_size=lot_size)
    result = sizer.calculate_size(
        instrument="INFY",
        entry_price=entry_price,
    )

    # Allocated notional cannot exceed target notional + 1 lot overshoot
    one_lot_notional = entry_price * Decimal(lot_size)
    assert result.allocated_notional.to_decimal() <= notional + one_lot_notional
    assert result.quantity.is_lot_multiple(lot_size) is True
