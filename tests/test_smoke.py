"""Smoke tests: prove the Rust <-> Python boundary works."""

from honba import Bar, Metrics, PyContext, Runner, SMA, Strategy


def test_sma_accumulates():
    sma = SMA(3)
    assert sma.update(1.0) is None
    assert sma.update(2.0) is None
    assert sma.update(3.0) == 2.0
    # rolling window: [2, 3, 4] -> 3.0
    assert sma.update(4.0) == 3.0


def test_runner_runs_strategy():
    class BuyAbove100(Strategy):
        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            if bar.close > 100.0:
                ctx.submit(bar.instrument, "BUY", 1)

    runner = Runner(num_bars=20, instrument="RELIANCE", starting_cash=100_000.0)
    result = runner.run(BuyAbove100())

    assert isinstance(result, Metrics)
    # synthetic sawtooth crosses 100 multiple times
    assert len(result.orders) > 0
    assert all(o.side == "BUY" for o in result.orders)
    assert all(o.instrument == "RELIANCE" for o in result.orders)


def test_runner_rejects_invalid_side():
    import pytest

    class BadSide(Strategy):
        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            ctx.submit(bar.instrument, "HOLD", 1)

    runner = Runner(num_bars=1)
    with pytest.raises(ValueError, match="side must be"):
        runner.run(BadSide())


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
