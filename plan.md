# Plan: 本場・魂 honba-tamashii

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
