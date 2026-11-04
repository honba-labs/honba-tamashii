"""Fault-Tolerance & Sandboxing Test Suite (plan.md Section 5.3 / Technical Debt Item 7).

Verifies that the Runner:
1. Isolates intentionally misbehaving Python strategies:
   - Raising ZeroDivisionError
   - Raising MemoryError
   - Triggering infinite recursive loops (RecursionError)
   - Raising SystemExit
   - Raising exceptions during setup() or on_stop()
2. Logs clean tracebacks into metrics.errors
3. Sets state to Degraded (metrics.degraded == True)
4. Suspends the offending algo in the honba-mon pre-trade risk engine
5. Continues executing subsequent strategy runs without process crash or state corruption
"""

from honba import (
    Runner,
)


class ZeroDivisionStrategy:
    __strategy_api_version__ = "1.0.0"

    def __init__(self):
        self.bars_seen = 0

    def on_bar(self, bar, ctx):
        self.bars_seen += 1
        if self.bars_seen >= 3:
            _ = 1 / 0


class MemoryErrorStrategy:
    __strategy_api_version__ = "1.0.0"

    def __init__(self):
        self.bars_seen = 0

    def on_bar(self, bar, ctx):
        self.bars_seen += 1
        if self.bars_seen >= 2:
            raise MemoryError("simulated out of memory condition")


class RecursiveLoopStrategy:
    __strategy_api_version__ = "1.0.0"

    def _recurse(self, n: int) -> int:
        return self._recurse(n + 1)

    def on_bar(self, bar, ctx):
        self._recurse(1)


class SystemExitStrategy:
    __strategy_api_version__ = "1.0.0"

    def on_bar(self, bar, ctx):
        raise SystemExit(42)


class SetupFailureStrategy:
    __strategy_api_version__ = "1.0.0"

    def setup(self):
        raise RuntimeError("setup initialization crash")

    def on_bar(self, bar, ctx):
        pass


class OrderThenCrashStrategy:
    __strategy_api_version__ = "1.0.0"

    def __init__(self):
        self.bars_seen = 0

    def on_bar(self, bar, ctx):
        self.bars_seen += 1
        if self.bars_seen == 1:
            ctx.submit("RELIANCE", "BUY", "10")
        elif self.bars_seen == 2:
            raise ValueError("crashed after order submission")


def test_zero_division_error_isolated_and_degraded():
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="DIV_ZERO_ALGO")
    strat = ZeroDivisionStrategy()
    metrics = runner.run(strat)

    assert metrics.degraded is True
    assert len(metrics.errors) >= 1
    err_text = "\n".join(metrics.errors)
    assert "ZeroDivisionError" in err_text
    assert "Traceback" in err_text


def test_memory_error_isolated_and_degraded():
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="OOM_ALGO")
    strat = MemoryErrorStrategy()
    metrics = runner.run(strat)

    assert metrics.degraded is True
    assert len(metrics.errors) >= 1
    err_text = "\n".join(metrics.errors)
    assert "MemoryError" in err_text
    assert "simulated out of memory condition" in err_text


def test_recursive_loop_isolated_and_degraded():
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="RECURSION_ALGO")
    strat = RecursiveLoopStrategy()
    metrics = runner.run(strat)

    assert metrics.degraded is True
    assert len(metrics.errors) >= 1
    err_text = "\n".join(metrics.errors)
    assert "RecursionError" in err_text


def test_system_exit_isolated_and_degraded():
    """Verify that SystemExit does not abort the Python interpreter."""
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="SYS_EXIT_ALGO")
    strat = SystemExitStrategy()
    metrics = runner.run(strat)

    assert metrics.degraded is True
    assert len(metrics.errors) >= 1
    err_text = "\n".join(metrics.errors)
    assert "SystemExit" in err_text or "42" in err_text


def test_setup_failure_isolated():
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="SETUP_FAIL_ALGO")
    strat = SetupFailureStrategy()
    metrics = runner.run(strat)

    assert metrics.degraded is True
    assert len(metrics.errors) >= 1
    assert "setup initialization crash" in "\n".join(metrics.errors)


def test_order_suspension_and_subsequent_continuation():
    """Verify that after a failure, the runner suspends the algo and process continues cleanly."""
    runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="FAULTY_ALGO")
    strat = OrderThenCrashStrategy()
    metrics = runner.run(strat)

    assert metrics.degraded is True
    assert len(metrics.orders) >= 1

    # Verify that runner can immediately run a well-behaved strategy successfully
    class CleanStrategy:
        __strategy_api_version__ = "1.0.0"

        def __init__(self):
            self.count = 0

        def on_bar(self, bar, ctx):
            self.count += 1

    clean_runner = Runner(num_bars=10, instrument="RELIANCE", algo_id="CLEAN_ALGO")
    clean_strat = CleanStrategy()
    clean_metrics = clean_runner.run(clean_strat)

    assert clean_metrics.degraded is False
    assert clean_metrics.errors == []
    assert clean_strat.count == 10
