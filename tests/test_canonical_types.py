"""The canonical value objects (plan.md, Phase 1).

Everything money crosses the boundary as an exact decimal: ``rust_decimal``
in Rust, ``decimal.Decimal`` in Python, and never a float.
"""

from decimal import Decimal

import pytest
from honba import (
    DepthLevel,
    InstrumentId,
    MinorUnit,
    Money,
    Price,
    PriceBand,
    Quantity,
)
from honba._honba import currency_minor_units


def test_a_price_is_built_from_a_decimal_string():
    price = Price("2540.50")
    assert str(price) == "2540.50"
    assert price.to_decimal() == Decimal("2540.50")
    assert repr(price) == "Price('2540.50')"


def test_a_price_accepts_an_int_and_a_decimal():
    assert Price(10).to_decimal() == Decimal(10)
    assert Price(Decimal("10.25")).to_decimal() == Decimal("10.25")


def test_a_price_refuses_a_float():
    with pytest.raises(TypeError, match="floats are not accepted"):
        Price(10.25)


def test_prices_compare_and_add():
    assert Price("10") < Price("11")
    assert Price("10") == Price("10.00")
    assert Price("10") <= Price("10")
    assert Price("11") > Price("10")
    assert (Price("10") + Price("2.50")).to_decimal() == Decimal("12.50")
    assert (Price("10") - Price("2.50")).to_decimal() == Decimal("7.50")


def test_price_times_quantity_is_the_notional():
    money = Price("2540.50") * Quantity("10")
    assert isinstance(money, Money)
    assert money.to_decimal() == Decimal("25405.00")
    assert money.currency == "INR"


def test_price_divided_by_price_is_a_ratio():
    ratio = Price("10") / Price("4")
    assert ratio == Decimal("2.5")


def test_quantities_compare_and_add():
    assert Quantity("10") > Quantity("9")
    assert (Quantity("10") + Quantity("5")).to_decimal() == Decimal(15)


def test_money_carries_its_currency():
    money = Money("1234.50", "INR")
    assert money.to_decimal() == Decimal("1234.50")
    assert money.currency == "INR"
    # The canonical amount is a major-unit decimal; the minor-unit form is
    # the integer number of paise.
    assert money.to_major() == Decimal("1234.50")
    assert money.to_minor() == Decimal(123450)
    with pytest.raises(ValueError, match="unknown currency"):
        Money("1", "JPY")


def test_money_addition_refuses_a_currency_mismatch():
    with pytest.raises(ValueError, match="currency mismatch"):
        Money("1", "INR") + Money("1", "USD")


def test_an_instrument_id_is_canonical():
    instrument = InstrumentId("reliance", "NSE")
    assert instrument.symbol == "RELIANCE"
    assert instrument.exchange == "NSE"
    assert str(instrument) == "NSE:RELIANCE"
    assert InstrumentId.from_string("BSE:TCS").exchange == "BSE"
    with pytest.raises(ValueError, match="unknown exchange"):
        InstrumentId("TCS", "NFO")


def test_a_price_band_knows_what_is_inside_it():
    band = PriceBand("900", "1100", "1000", "0.10")
    assert band.lower.to_decimal() == Decimal(900)
    assert band.band_pct == Decimal("0.10")
    assert band.contains(Price("1000"))
    assert not band.contains(Price("1200"))


def test_a_depth_level_carries_price_and_size():
    level = DepthLevel(Price("100"), Quantity("5"), 2)
    assert level.price.to_decimal() == Decimal(100)
    assert level.quantity.to_decimal() == Decimal(5)
    assert level.orders_count == 2


def test_the_currency_table_comes_from_the_kernel():
    table = currency_minor_units()
    assert table["INR"] == (2, "paisa", "paise")
    assert table["USD"] == (2, "cent", "cents")
    assert table["GBP"] == (2, "penny", "pence")


def test_the_minor_unit_names_are_exposed():
    unit = MinorUnit.of_currency("INR")
    assert unit.singular == "paisa"
    assert unit.plural == "paise"
