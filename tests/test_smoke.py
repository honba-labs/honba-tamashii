"""Smoke tests: prove the Rust <-> Python boundary works.

The values that cross the boundary are exact decimals: a price is a
``decimal.Decimal``, never a float (the Zero Floats policy).
"""

from decimal import Decimal

import pytest

from honba import Bar, Metrics, PyContext, Runner, SMA, Strategy


def test_sma_accumulates():
    sma = SMA(3)
    assert sma.update(1) is None
    assert sma.update(2) is None
    assert sma.update(3) == Decimal("2")
    # rolling window: [2, 3, 4] -> 3
    assert sma.update(4) == Decimal("3")


def test_sma_refuses_a_float():
    # A float would lose precision the moment it entered the domain.
    with pytest.raises(TypeError, match="floats are not accepted"):
        SMA(3).update(1.0)


def test_runner_runs_strategy():
    class BuyAbove100(Strategy):
        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            if bar.close > 100:
                ctx.submit(bar.instrument, "BUY", 1)

    runner = Runner(num_bars=20, instrument="RELIANCE", starting_cash="100000")
    result = runner.run(BuyAbove100())

    assert isinstance(result, Metrics)
    # synthetic sawtooth crosses 100 multiple times
    assert len(result.orders) > 0
    assert all(o.side == "BUY" for o in result.orders)
    assert all(str(o.instrument) == "NSE:RELIANCE" for o in result.orders)
    assert result.final_cash == Decimal("100000")
    assert result.degraded is False


def test_runner_rejects_invalid_side():
    # A strategy that submits an invalid order does not take the host down:
    # the run is degraded and the reason is kept.
    class BadSide(Strategy):
        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            ctx.submit(bar.instrument, "HOLD", 1)

    result = Runner(num_bars=1).run(BadSide())

    assert result.degraded is True
    assert any("side must be" in error for error in result.errors)


def test_lifecycle_order():
    events: list[str] = []

    class Traced(Strategy):
        def setup(self) -> None:
            events.append("setup")

        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            if not events or events[-1] != "on_bar":
                events.append("on_bar")

        def on_stop(self) -> None:
            events.append("on_stop")

    Runner(num_bars=5).run(Traced())
    assert events[0] == "setup"
    assert events[-1] == "on_stop"
    assert "on_bar" in events


def test_a_raising_strategy_does_not_take_the_host_down():
    # The harness isolates a broken strategy: the run is degraded and the
    # traceback is kept, but the process keeps running.
    class Exploding(Strategy):
        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            raise ZeroDivisionError("boom")

    result = Runner(num_bars=5).run(Exploding())

    assert result.degraded is True
    assert result.errors
    assert "ZeroDivisionError" in result.errors[0]


def test_the_risk_gate_refuses_an_unregistered_algo():
    # The harness registers its own algo id, so an order from a strategy is
    # validated; the gate is what stands between a strategy and a broker.
    from honba._honba import RiskEngine

    engine = RiskEngine()
    with pytest.raises(Exception) as excinfo:
        engine.check_order(
            _order(algo_id="NOT_REGISTERED"),
        )
    assert getattr(excinfo.value, "rule", "") == "algo_not_registered"


def _order(algo_id: str):
    from honba import InstrumentId, OrderRequest, OrderSide

    return OrderRequest(
        InstrumentId("RELIANCE", "NSE"),
        OrderSide.BUY,
        10,
        algo_id=algo_id,
    )
