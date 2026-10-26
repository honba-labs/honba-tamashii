"""Unit and integration tests for Phase 2: Tamashii Strategy Dynamic Dispatch."""

from collections.abc import Mapping
from decimal import Decimal
from typing import Any

import honba
import pytest
from honba import (
    PortfolioRunner,
    Runner,
    Strategy,
    StrategyV1,
    StrategyV2,
)
from honba.client import Client
from honba.client.transport import Response
from honba.compat import (
    IncompatibleHonbaVersionError,
    IncompatibleStrategyApiError,
    check_compatibility,
    require_honba,
    require_strategy_api,
)
from honba.v1.strategies import SmaCrossover as SmaV1
from honba.v1.strategies import Strategy as V1Strategy
from honba.v2.strategies import AdaptiveMomentum as MomentumV2
from honba.v2.strategies import Strategy as V2Strategy

# ==============================================================================
# 1. Compat version validation tests
# ==============================================================================


def test_require_honba_success():
    ver = require_honba(">=0.1.0")
    assert ver == honba.__version__

    ver2 = require_honba(">=0.1.0, <1.0.0")
    assert ver2 == honba.__version__


def test_require_honba_failure():
    with pytest.raises(IncompatibleHonbaVersionError):
        require_honba(">=1.0.0")


def test_require_strategy_api_success():
    assert require_strategy_api(major=1) == "1.0.0"
    assert require_strategy_api(major=2) == "2.0.0"
    assert require_strategy_api(1) == "1.0.0"
    assert require_strategy_api(2) == "2.0.0"
    assert require_strategy_api("1.0.0") == "1.0.0"
    assert require_strategy_api(">=2.0.0") == "2.0.0"


def test_require_strategy_api_failure():
    with pytest.raises(IncompatibleStrategyApiError):
        require_strategy_api(major=3)

    with pytest.raises(IncompatibleStrategyApiError):
        require_strategy_api(">=3.0.0")


def test_check_compatibility_helper():
    assert check_compatibility(honba_spec=">=0.1.0", major=1) is True
    assert check_compatibility(honba_spec=">=0.1.0", major=2) is True
    assert check_compatibility(honba_spec=">=10.0.0", major=1) is False
    assert check_compatibility(honba_spec=">=0.1.0", major=5) is False


# ==============================================================================
# 2. Dual namespace exports tests
# ==============================================================================


def test_dual_namespace_exports():
    assert V1Strategy.__strategy_api_version__ == "1.0.0"
    assert V2Strategy.__strategy_api_version__ == "2.0.0"
    assert StrategyV1.__strategy_api_version__ == "1.0.0"
    assert StrategyV2.__strategy_api_version__ == "2.0.0"
    # Default top-level Strategy is latest & recommended (v2)
    assert Strategy.__strategy_api_version__ == "2.0.0"


# ==============================================================================
# 3. Dynamic Dispatch in Runner
# ==============================================================================


class CustomV1(V1Strategy):
    name = "custom_v1"

    def __init__(self):
        super().__init__()
        self.bars = 0

    def on_bar(self, bar, ctx=None):
        self.bars += 1
        if self.bars == 1 and ctx is not None:
            ctx.submit("RELIANCE", "BUY", "10")


class CustomV2(V2Strategy):
    name = "custom_v2"

    def __init__(self):
        super().__init__()
        self.steps = 0
        self.received_event = None

    def on_step(self, event, ctx=None):
        self.steps += 1
        self.received_event = event
        assert ctx is not None and ctx.version == "2.0.0"
        if self.steps == 1:
            ctx.submit("RELIANCE", "BUY", "20")


class LegacyUnversioned:
    """Strategy class with no __strategy_api_version__ defined."""

    def __init__(self):
        self.bars = 0

    def on_bar(self, bar, ctx):
        self.bars += 1
        if self.bars == 1:
            ctx.submit("RELIANCE", "BUY", "5")


def test_runner_v1_dispatch():
    runner = Runner(num_bars=10, instrument="RELIANCE")
    strat = CustomV1()
    metrics = runner.run(strat)
    assert metrics.degraded is False
    assert strat.bars == 10
    assert len(metrics.orders) == 1
    assert str(metrics.orders[0].quantity) == "10"


def test_runner_v2_dispatch():
    runner = Runner(num_bars=10, instrument="RELIANCE")
    strat = CustomV2()
    metrics = runner.run(strat)
    assert metrics.degraded is False
    assert strat.steps == 10
    assert strat.received_event is not None
    assert strat.received_event.kind == "bar"
    assert len(metrics.orders) == 1
    assert str(metrics.orders[0].quantity) == "20"


def test_runner_legacy_unversioned_fallback():
    runner = Runner(num_bars=10, instrument="RELIANCE")
    strat = LegacyUnversioned()
    metrics = runner.run(strat)
    assert metrics.degraded is False
    assert strat.bars == 10
    assert len(metrics.orders) == 1
    assert str(metrics.orders[0].quantity) == "5"


# ==============================================================================
# 4. PortfolioRunner side-by-side execution
# ==============================================================================


def test_portfolio_runner_side_by_side():
    portfolio = PortfolioRunner(capital=10_000_000)
    sma_v1 = SmaV1(fast=2, slow=5)
    momentum_v2 = MomentumV2(lookback=5)

    portfolio.add_strategy("legacy_sma", sma_v1, allocation=0.4)
    portfolio.add_strategy("new_momentum", momentum_v2, allocation=0.6)

    results = portfolio.run(num_bars=25)
    assert results.degraded is False
    assert "legacy_sma" in results
    assert "new_momentum" in results
    assert results["legacy_sma"]["version"] == "1.0.0"
    assert results["new_momentum"]["version"] == "2.0.0"
    assert results["legacy_sma"]["final_cash"] == Decimal("4000000.0")
    assert results["new_momentum"]["final_cash"] == Decimal("6000000.0")
    assert results.total_orders > 0


# ==============================================================================
# 5. Dual REST Client Support
# ==============================================================================


class MockTransport:
    def __init__(self):
        self.last_method = None
        self.last_path = None

    def request(
        self,
        method: str,
        path: str,
        *,
        query: Mapping[str, Any] | None = None,
        body: Any = None,
    ) -> Response:
        self.last_method = method
        self.last_path = path
        if "depth" in path:
            data = {"bids": [], "asks": []}
        else:
            data = {"instruments": []}
        return Response(200, {"schema_version": 8, "data": data})


def test_dual_client_support():
    transport = MockTransport()

    # Unified client with sub-namespaces
    client = Client(transport)
    assert client.api_version is None
    assert client.v1.api_version == "v1"
    assert client.v2.api_version == "v2"

    client.v1.get_instruments()
    assert transport.last_path == "/api/v1/instruments"

    client.v2.get_depth("RELIANCE")
    assert transport.last_path == "/api/v2/depth/RELIANCE.NSE"

    # Explicitly pinned clients
    client_v1 = Client(transport, api_version="v1")
    client_v1.instruments()
    assert transport.last_path == "/api/v1/instruments"

    client_v2 = Client(transport, api_version="v2")
    client_v2.depth("RELIANCE")
    assert transport.last_path == "/api/v2/depth/RELIANCE.NSE"
