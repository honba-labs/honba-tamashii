"""The pre-trade risk gate (plan.md, Phase 1).

Every order a Python strategy submits passes through ``honba-mon`` before it
can reach a broker: the SEBI algo-id registry, the order-to-trade ratio
breaker, the scrip price band, the freeze limits and the kill switches.
"""

import pytest
from honba import InstrumentId, OrderRequest, OrderSide, PriceBand
from honba._honba import (
    OtrConfig,
    RiskEngine,
    RiskLimits,
    RiskRefusalError,
    RiskStage,
    TradingState,
)


def _order(algo_id="ALGO_1", quantity="10", price=None, instrument=None):
    return OrderRequest(
        instrument or InstrumentId("RELIANCE", "NSE"),
        OrderSide.BUY,
        quantity,
        order_type="limit" if price is not None else "market",
        price=price,
        algo_id=algo_id,
    )


def test_a_registered_algo_is_approved():
    engine = RiskEngine()
    engine.register_algo("ALGO_1")
    engine.check_order(_order())


def test_an_unregistered_algo_is_refused():
    engine = RiskEngine()
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order(algo_id="UNKNOWN"))
    assert excinfo.value.rule == "algo_not_registered"


def test_the_refusal_carries_the_rule_and_the_details():
    engine = RiskEngine()
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order(algo_id="UNKNOWN"))
    error = excinfo.value
    assert error.rule == "algo_not_registered"
    assert "algo_not_registered" in error.refusal


def test_a_price_outside_the_circuit_band_is_refused():
    engine = RiskEngine()
    engine.register_algo("ALGO_1")
    engine.set_price_band(
        InstrumentId("RELIANCE", "NSE"),
        PriceBand("900", "1100", "1000", "0.10"),
    )
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order(price="1500"))
    assert excinfo.value.rule == "price_band_violation"


def test_a_price_inside_the_circuit_band_is_approved():
    engine = RiskEngine()
    engine.register_algo("ALGO_1")
    engine.set_price_band(
        InstrumentId("RELIANCE", "NSE"),
        PriceBand("900", "1100", "1000", "0.10"),
    )
    engine.check_order(_order(price="1000"))


def test_the_global_kill_switch_refuses_everything():
    engine = RiskEngine()
    engine.register_algo("ALGO_1")
    engine.halt_global("operator")
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order())
    assert excinfo.value.rule == "global_halt"
    engine.resume_global()
    engine.check_order(_order())


def test_a_suspended_algo_is_refused():
    engine = RiskEngine()
    engine.register_algo("ALGO_1")
    engine.suspend_algo("ALGO_1", "under review")
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order())
    assert excinfo.value.rule == "algo_not_registered"
    engine.reactivate_algo("ALGO_1")
    engine.check_order(_order())


def test_a_quantity_over_the_limit_is_refused():
    engine = RiskEngine(RiskLimits(max_quantity_per_order="5"))
    engine.register_algo("ALGO_1")
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order(quantity="10"))
    assert excinfo.value.rule == "max_quantity_exceeded"


def test_the_otr_breaker_trips_on_a_cancellation_storm():
    # SEBI: the order-to-trade ratio above the limit halts the algo.
    engine = RiskEngine(otr=OtrConfig(max_otr="2", min_orders_before_enforcement=0))
    engine.register_algo("ALGO_1")
    for _ in range(5):
        engine.record_cancellation("ALGO_1")
    with pytest.raises(RiskRefusalError) as excinfo:
        engine.check_order(_order())
    assert excinfo.value.rule in {"sebi_otr_breach", "algo_halt"}


def test_limits_are_parsed_from_the_config_table():
    limits = RiskLimits.from_dict(
        {"max_notional": "100000", "order_rate": {"max_orders": 10, "window_ms": 1000}}
    )
    assert limits.max_notional_per_order is not None
    assert limits.is_empty() is False
    with pytest.raises(ValueError, match="unknown risk key"):
        RiskLimits.from_dict({"nope": 1})


def test_a_live_run_needs_limits():
    with pytest.raises(ValueError, match="live trading requires risk limits"):
        RiskLimits().require_live()
    RiskLimits(max_notional_per_order="1000").require_live()


def test_the_stage_answers_instead_of_raising():
    stage = RiskStage()
    stage.register_algo("ALGO_1")
    decision = stage.check(_order())
    assert decision.approved is True
    assert decision.rule == ""

    unregistered = stage.check(_order(algo_id="UNKNOWN"))
    assert unregistered.approved is False
    assert unregistered.rule == "algo_not_registered"


def test_the_trading_state_has_a_wire_spelling():
    assert str(TradingState.ACTIVE) == "active"
    assert str(TradingState.REDUCING) == "reducing"
    assert str(TradingState.HALTED) == "halted"
