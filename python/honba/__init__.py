"""本場 honba — Rust core, Python surface.

Two layers of history coexist in this package, by design during the M0 -> M1+ migration
(see plan.md, "Unfinished cleanup carried in from the M0.5 bulk import"):

1. The compiled extension `honba._honba`, built from this repo's own Rust crate
   (crates/honba-tamashii-py). It exports the M0 backtest-harness surface: Runner,
   PyContext, Bar, Metrics, SMA. Everything below re-exports those, plus the thin Python
   `Strategy` base class from `honba/strategy.py`.
2. A bulk-imported tree of pure-Python modules carried over from the legacy honba-labs
   package (honba.domain, honba.entities, honba.markets, honba.screener, honba.data,
   honba.research, honba.backtest, honba.strategies, honba.adapters, honba.ai, honba.api,
   honba.client, honba.cli, honba.query, honba.utils, honba.wire, honba.analytics,
   honba.algo_analytics, honba.core, plus top-level risk.py / session.py / event_loop.py /
   _native.py / display.py / report.py / incubation.py / log.py / log_main.py). That code
   was written against a DIFFERENT compiled extension — also historically named
   `honba._honba`, but built from an unrelated Rust crate in honba-labs with a much larger
   surface (RiskStage, canonical_json, NextOpenSimulator, L2Simulator, GatewayServer,
   run_strategy, verify_manifest, ...). Its native calls are deliberately lazy (see
   `honba._native.native_attr`), so plain `import honba` and ordinary submodule imports
   never touch the extension; only a call into a function this extension doesn't have
   fails at call time with a clear ImportError / RuntimeError, or silently falls back to
   an existing pure-Python path where the legacy code already has one (e.g.
   `honba.backtest.simulated.resolve_backend("auto")` and
   `honba.markets.india.settlement.nse_equity_settlement_days`).

Nothing here re-exports the legacy package's own top-level convenience names (Bar,
OrderIntent, Strategy, Portfolio, ...) at `honba.*` — they would collide with (1) above.
Reach them via their real submodule paths instead, e.g. `honba.domain.bar.Bar` or
`honba.entities.bar.Bar`, exactly as `honba_examples` already does.
"""
from honba._honba import (
    Bar,
    Metrics,
    OrderIntent,
    PyContext,
    Runner,
    SMA,
)
from honba.strategy import Strategy

__all__ = [
    "Bar",
    "Metrics",
    "OrderIntent",
    "PyContext",
    "Runner",
    "SMA",
    "Strategy",
]

__version__ = "0.1.0"
