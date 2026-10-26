"""Contract Regression Test Suite (Phase 8.1 / M7).

Verifies that strategies written against version N-1 execute without
modification against version N.
"""

from decimal import Decimal
import pytest

import honba
from honba import (
    Bar,
    MarketEvent,
    Money,
    PortfolioResult,
    PortfolioRunner,
    Price,
    PyContext,
    PyContextV2,
    Quantity,
    Runner,
    Strategy,
    StrategyV1,
    StrategyV2,
)
from honba.compat import (
    check_compatibility,
    require_honba,
    require_strategy_api,
)
from honba.v1.strategies import SmaCrossover as SmaCrossoverV1
from honba.v1.strategies import Strategy as V1BaseStrategy
from honba.v2.strategies import AdaptiveMomentum as MomentumV2
from honba.v2.strategies import Strategy as V2BaseStrategy


# ==============================================================================
# 1. Unversioned Legacy Strategy (Pre-v1) Regression Test
# ==============================================================================


class LegacyUnversionedStrategy:
    """Strategy written before versioning was introduced (no __strategy_api_version__)."""

    def __init__(self):
        self.setup_called = False
        self.bar_count = 0
        self.stop_called = False

    def setup(self):
        self.setup_called = True

    def on_bar(self, bar: Bar, ctx: PyContext):
        self.bar_count += 1
        if self.bar_count == 5:
            ctx.submit(bar.instrument, "BUY", Decimal("100"))

    def on_stop(self):
        self.stop_called = True


def test_legacy_unversioned_strategy_executes_on_version_n():
    """Unversioned legacy strategy executes cleanly without modification."""
    strat = LegacyUnversionedStrategy()
    runner = Runner(num_bars=10, instrument="TCS", algo_id="HARNESS_ALGO")
    metrics = runner.run(strat)

    assert strat.setup_called is True
    assert strat.stop_called is True
    assert strat.bar_count == 10
    assert metrics.degraded is False
    assert len(metrics.orders) == 1
    assert metrics.orders[0].side == "BUY"
    assert metrics.orders[0].quantity.to_decimal() == Decimal("100")


# ==============================================================================
# 2. Version N-1 (v1.0.0) Strategy Contract Regression
# ==============================================================================


class VersionOneGoldenStrategy(V1BaseStrategy):
    """Canonical Strategy API v1.0.0 contract implementation."""

    name = "v1_golden"

    def __init__(self):
        super().__init__()
        self.bars = []
        self.equities = []

    def setup(self):
        pass

    def on_bar(self, bar: Bar, ctx: PyContext):
        self.bars.append(bar)
        self.equities.append(ctx.equity.to_decimal())
        if len(self.bars) == 10:
            # Submit using v1 submit signature: instrument, side, quantity
            ctx.submit("RELIANCE", "BUY", Decimal("200"))
        elif len(self.bars) == 20:
            ctx.submit("RELIANCE", "SELL", Decimal("200"))

    def on_stop(self):
        pass


def test_v1_strategy_contract_regression():
    """Version N-1 (v1.0.0) strategies run on Version N with identical outputs."""
    strat = VersionOneGoldenStrategy()
    assert strat.__strategy_api_version__ == "1.0.0"

    runner = Runner(num_bars=30, instrument="RELIANCE", starting_cash=Decimal("500000"), algo_id="HARNESS_ALGO")
    metrics = runner.run(strat)

    assert len(strat.bars) == 30
    assert len(strat.equities) == 30
    assert strat.equities[0] == Decimal("500000")
    assert metrics.degraded is False
    assert len(metrics.orders) == 2
    assert metrics.orders[0].side == "BUY"
    assert metrics.orders[0].quantity.to_decimal() == Decimal("200")
    assert metrics.orders[1].side == "SELL"
    assert metrics.orders[1].quantity.to_decimal() == Decimal("200")


def test_v1_sma_crossover_reference_strategy():
    """Reference SmaCrossover from v1 namespace works without modification in PortfolioRunner."""
    portfolio = PortfolioRunner(capital=5_000_000)
    sma_v1 = SmaCrossoverV1(fast=2, slow=5)
    assert sma_v1.__strategy_api_version__ == "1.0.0"

    portfolio.add_strategy("legacy_sma", sma_v1, allocation=1.0)
    results = portfolio.run(num_bars=25)

    assert results.degraded is False
    assert "legacy_sma" in results
    assert results["legacy_sma"]["version"] == "1.0.0"
    assert results.total_orders > 0


# ==============================================================================
# 3. Version Compatibility Guarantees
# ==============================================================================


def test_semver_compatibility_contract():
    """Version check helpers guarantee N-1 compatibility."""
    assert check_compatibility(honba_spec=">=0.1.0", major=1) is True
    assert check_compatibility(honba_spec=">=0.1.0", major=2) is True
    assert require_strategy_api(1) == "1.0.0"
    assert require_strategy_api(2) == "2.0.0"
    assert require_honba(">=0.1.0") == honba.__version__
