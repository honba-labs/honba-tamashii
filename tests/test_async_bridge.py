"""Tests for Tokio-AsyncIO Cooperative Execution Bridge (Phase 8.2 / M7)."""

import asyncio
import time
from decimal import Decimal
import pytest

from honba import (
    Bar,
    CancellationToken,
    InstrumentId,
    MarketEvent,
    Money,
    Price,
    PyCancellationToken,
    PyContext,
    PyContextV2,
    Quantity,
    Runner,
    StrategyV1,
    StrategyV2,
    async_bridge,
)


# ==============================================================================
# 1. CancellationToken Unit Tests
# ==============================================================================


def test_cancellation_token_lifecycle():
    token = CancellationToken()
    assert token.is_cancelled() is False
    assert bool(token) is True
    assert "cancelled=false" in repr(token).lower()

    token.cancel()
    assert token.is_cancelled() is True
    assert bool(token) is False
    assert "cancelled=true" in repr(token).lower()

    token.reset()
    assert token.is_cancelled() is False
    assert bool(token) is True


@pytest.mark.asyncio
async def test_cancellation_token_async_wait():
    token = CancellationToken()

    async def trigger_cancel():
        await asyncio.sleep(0.02)
        token.cancel()

    asyncio.create_task(trigger_cancel())
    await token.cancelled()
    assert token.is_cancelled() is True


# ==============================================================================
# 2. run_coroutine Cooperative Execution & Timeouts
# ==============================================================================


def test_run_coroutine_success():
    async def multiply(a: int, b: int) -> int:
        await asyncio.sleep(0.01)
        return a * b

    result = async_bridge.run_coroutine(multiply(6, 7))
    assert result == 42


def test_run_coroutine_invalid_timeout():
    async def noop():
        return 1

    c1 = noop()
    try:
        with pytest.raises(ValueError, match="timeout_secs must be positive"):
            async_bridge.run_coroutine(c1, timeout_secs=0.0)
    finally:
        c1.close()

    c2 = noop()
    try:
        with pytest.raises(ValueError, match="timeout_secs must be positive"):
            async_bridge.run_coroutine(c2, timeout_secs=-1.5)
    finally:
        c2.close()


def test_run_coroutine_timeout_abort():
    async def stall():
        await asyncio.sleep(5.0)
        return "done"

    start = time.perf_counter()
    with pytest.raises(TimeoutError, match="timed out"):
        async_bridge.run_coroutine(stall(), timeout_secs=0.05)
    elapsed = time.perf_counter() - start
    assert elapsed < 1.0


def test_run_coroutine_token_cancellation():
    token = CancellationToken()

    async def cooperative_task():
        await asyncio.sleep(5.0)
        return "done"

    # Pre-cancel token
    token.cancel()
    with pytest.raises(asyncio.CancelledError):
        async_bridge.run_coroutine(cooperative_task(), token=token)


# ==============================================================================
# 3. Thread Pool Isolation for Blocking Python Calls
# ==============================================================================


def test_run_in_blocking_pool():
    def cpu_heavy(n: int) -> int:
        return sum(i * i for i in range(n))

    res = async_bridge.run_in_blocking_pool(cpu_heavy, 100)
    assert res == sum(i * i for i in range(100))


@pytest.mark.asyncio
async def test_spawn_blocking_async():
    def compute(x: int, y: int) -> int:
        time.sleep(0.02)
        return x + y

    res = await async_bridge.spawn_blocking(compute, 15, 27)
    assert res == 42


@pytest.mark.asyncio
async def test_spawn_blocking_timeout():
    def blocking_sleep():
        time.sleep(1.0)
        return "finished"

    with pytest.raises(TimeoutError, match="timed out"):
        await async_bridge.spawn_blocking(blocking_sleep, timeout_secs=0.05)


# ==============================================================================
# 4. Async Strategy Callback Execution in Runner
# ==============================================================================


class AsyncV1Strategy(StrategyV1):
    name = "async_v1"

    def __init__(self):
        super().__init__()
        self.setup_called = False
        self.stop_called = False
        self.bars = 0

    async def setup(self):
        await asyncio.sleep(0.01)
        self.setup_called = True

    async def on_bar(self, bar: Bar, ctx: PyContext):
        await asyncio.sleep(0.001)
        self.bars += 1
        if self.bars == 10:
            ctx.submit(bar.instrument, "BUY", Decimal("100"))

    async def on_stop(self):
        await asyncio.sleep(0.01)
        self.stop_called = True


def test_runner_executes_async_v1_strategy():
    strategy = AsyncV1Strategy()
    runner = Runner(num_bars=20, instrument="RELIANCE", algo_id="HARNESS_ALGO")
    metrics = runner.run(strategy)

    assert strategy.setup_called is True
    assert strategy.stop_called is True
    assert strategy.bars == 20
    assert metrics.degraded is False
    assert len(metrics.orders) == 1
    assert metrics.orders[0].side == "BUY"
    assert metrics.orders[0].quantity.to_decimal() == Decimal("100")


class AsyncV2Strategy(StrategyV2):
    name = "async_v2"

    def __init__(self):
        super().__init__()
        self.events = 0

    async def on_step(self, event: MarketEvent, ctx: PyContextV2):
        await asyncio.sleep(0.001)
        self.events += 1
        if self.events == 5:
            ctx.submit("INFY", "BUY", Decimal("50"))


def test_runner_executes_async_v2_strategy():
    strategy = AsyncV2Strategy()
    runner = Runner(num_bars=15, instrument="INFY", algo_id="HARNESS_ALGO")
    metrics = runner.run(strategy)

    assert strategy.events == 15
    assert metrics.degraded is False
    assert len(metrics.orders) == 1
    assert metrics.orders[0].quantity.to_decimal() == Decimal("50")


class StalledAsyncStrategy(StrategyV1):
    name = "stalled_async"

    async def on_bar(self, bar: Bar, ctx: PyContext):
        # Deliberately raise or hang (we simulate an error)
        raise RuntimeError("Async callback failure")


def test_runner_isolates_async_strategy_failure():
    strategy = StalledAsyncStrategy()
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="HARNESS_ALGO")
    metrics = runner.run(strategy)

    assert metrics.degraded is True
    assert len(metrics.errors) > 0
    assert "Async callback failure" in metrics.errors[0]
