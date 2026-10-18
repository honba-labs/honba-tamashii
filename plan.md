# Plan: 本場・魂 honba-tamashii

## Unfinished cleanup carried in from the M0.5 bulk import

Out of staged order: `python/honba/` also carries a bulk, mostly-mechanical import of the
pure-Python parts of the legacy honba-labs package (`honba.domain`, `.entities`, `.markets`,
`.screener`, `.data`, `.research`, `.backtest`, `.strategies`, `.adapters`, `.ai`, `.api`,
`.client`, `.cli`, `.query`, `.utils`, `.wire`, `.analytics`, `.algo_analytics`, `.core`, plus
top-level `risk.py` / `session.py` / `event_loop.py` / `_native.py` / `display.py` /
`report.py` / `incubation.py` / `log.py` / `log_main.py`), done pragmatically so
`honba-examples` could drop its dependency on honba-labs today, ahead of M1-M5 actually
wiring this surface to the new Rust crates. This is a deviation from the staged plan below,
not a replacement for it — M1-M5 should treat the bulk-imported tree as the thing being
replaced piece by piece (e.g. M4's `honba.indicators` / `honba.types` already exist here in
pure-Python legacy form; M1's Bar/OrderIntent wrappers should absorb `honba.entities`/
`honba.domain`'s pure-Python versions rather than add a third copy).

Known rough edges left behind, for M1+ to actually fix rather than re-discover:
- The bulk-imported code was written against a *different* native extension (also named
  `honba._honba`, built from the unrelated `honba-labs/honba/crates/honba-py` crate) with a
  much larger surface (`RiskStage`, `canonical_json`, `NextOpenSimulator`, `L2Simulator`,
  `GatewayServer`, `run_strategy`, `verify_manifest`, `nse_equity_settlement_days`,
  `currency_minor_units`, `wire_enum_values`, `TradingState`, ...). Most of that code reaches
  the extension lazily (`honba._native.native_attr`, or a `try/except ImportError` at call
  time), which is what let the bulk import work with only two real patches:
  - `honba/domain/money.py`: `_minor_units()` now falls back to a hardcoded 4-currency table
    (INR/USD/EUR/GBP, mirroring `schema/conformance/currency_minor_units.json`) when
    `currency_minor_units` is missing from this repo's own `_honba`. This is currently
    load-bearing (every `Money` construction goes through it), not just a convenience.
  - `honba/strategies/runner.py`: `_active()` (the default `TradingState`) falls back to a
    tiny pure-Python stand-in whose `str()` is `"active"`, since `TradingState` isn't
    exported here either.
  - Both are marked `# TODO(tamashii-migration)`. A honba-tamashii crate should eventually
    own `TradingState` / the currency table for real, at which point these fallbacks become
    dead code and can be deleted.
- Two path-calculation helpers (`honba/screener/catalog.py::default_catalog_path`,
  `honba/ai/mcp/tools.py::schema_path`) hardcode how many `.parents[N]` steps lead back to
  the repo root; they were off by one for this repo's `python/honba/...` layout (legacy has
  an extra `python/src/honba/...` level) and have been corrected, also marked
  `# TODO(tamashii-migration)`.
- Not imported at all: the legacy native extension's two stale `.pyi` stubs
  (`honba/_honba.pyi`, `honba/_lib/__init__.pyi`) — they describe the *other* `honba._honba`
  and would mislead type checkers about this repo's own compiled module. Also not imported:
  the compiled `.so` files themselves (obviously — that's the whole point of keeping this
  repo's own `crates/honba-tamashii-py` as the one and only `honba._honba`).
- Not audited line-by-line: `honba.ai.*` (rl/alignment/llm/mcp/autoresearch/journal_data/
  verification) beyond confirming `import honba.ai` and the few submodules
  `honba_examples` actually uses (`ai.ask`, `ai.decision`, `ai.knowledge`, `ai.llm.provider`)
  work without the heavy optional deps (`torch`, `transformers`, `llama-cpp-python`, ...)
  installed. Those stay behind the `ai` / `alignment` / `llm` / `rl` extras in
  `pyproject.toml`; nothing at package-import time requires them.
- `honba.adapters` here is only the contract/registry/testing/secrets infrastructure.
  Concrete broker adapters (dhan, ...) live in the separate `honba-adapters` repo, as the
  module's own docstring says — unaffected by this import.

The acceptance bar for the bulk import was `honba-examples`' test suite at least matching
its pre-migration baseline (768 passed / 1 skipped / 6 failed, all 6 in
`tests/unit/test_06_mcp_integration.py`, a pre-existing stale-assertion bug) sourced
entirely from this repo. It now passes 773 / 1 skipped / 1 failed (same pre-existing bug,
now only tripping one assertion instead of six) with zero runtime reference to honba-labs.

## M0 — Boundary proof (W1)
☑ Scaffold crate, pyproject.toml, maturin config
☑ Runner, Strategy, Bar, PyContext, Metrics, SMA
☑ Passing pytest on a fresh maturin develop
□ GitHub Actions: build wheels for 5 targets, upload to PyPI on tag
□ py.typed + mypy --strict in CI

## M1 — Wire to real crates (W2–4)
□ Depend on honba-kokoro-types via path — replace Bar/OrderIntent
with wrappers around honba_kokoro_types::{Bar, Price, Quantity, ...}
□ Runner delegates to honba-ya-engine — the real event loop
□ PyContext::submit calls honba-te-router (OTR-aware)
□ algo_id passed through from config, never defaulted

## M2 — Market data & persistence (W5–6)
□ Runner picks feed: honba-me-loaders (backtest) or
honba-me-<broker> (live)
□ Runner picks execution: honba-ya-sim (backtest) or
honba-te-<broker> (live)
□ Optional runner.snapshot(path) for session replay

## M3 — Async and error discipline (W7–8)
□ Wrap every Python callback in catch_unwind; mark strategy
Degraded on exception, log traceback, continue
□ Release GIL with py.allow_threads around all Rust I/O between
callbacks
□ Runner.shutdown() — Ctrl-C safe, calls on_stop, flushes fills

## M4 — Python SDK surface (W9–12)
□ honba.adapters.BrokerAdapter — Python-side broker trait for
community adapters (escape hatch before graduating to Rust honba-te)
□ honba.indicators — re-export SMA/EMA/RSI from honba-me-indicators
□ honba.types — Price, Quantity, InstrumentId as PyO3 wrappers
with proper __eq__ / __gt__ / __float__

## M5 — Distribution (W13)
□ maturin publish dry-run against TestPyPI
□ Compatibility table in README: honba 0.1.x ↔ honba-kokoro 0.1.x
□ honba.abi3.so size budget: < 20 MB stripped
