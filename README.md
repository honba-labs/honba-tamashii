# 本場・魂 honba-tamashii

> *The spirit that animates the body.*

PyO3 bindings for the honba Rust engine. Publishes as **`honba`** on PyPI.

This is the **only** crate in the honba ecosystem permitted to depend on
PyO3. Everything below it is pure Rust; everything above it is Python.

## Scope of this scaffold

A minimal but **end-to-end working** boundary:

- `Runner` (Rust) — synthetic price walk, calls your Python strategy per bar
- `Strategy` (Python) — subclass and override `setup` / `on_bar` / `on_stop`
- `PyContext` (Rust) — `ctx.submit(...)`, `ctx.close_all(...)`
- `Metrics` (Rust) — collected order intents returned from `run()`
- `SMA` (Rust) — reference indicator proving numeric types flow across the FFI

Not yet wired: real market data (`honba-me`), risk (`honba-kokoro-risk`),
execution (`honba-te`), persistence (`honba-ya`). Those are the next
milestones in `plan.md`. The **shape** of the call boundary (`Runner.run` →
`strategy.on_bar(bar, ctx)` → `ctx.submit` → recorded) will not change when
those land.

## Usage

```python
from honba import Runner, Strategy, Bar, PyContext

class MomentumSMA(Strategy):
    def on_bar(self, bar: Bar, ctx: PyContext) -> None:
        if bar.close > 100.0:
            ctx.submit(bar.instrument, "BUY", 1)

runner = Runner(num_bars=20, instrument="RELIANCE", starting_cash=100_000)
result = runner.run(MomentumSMA())
print(result.orders)     # list[OrderIntent]
print(result.final_cash) # 100_000.0
Prerequisites
Rust stable (1.85+)

Python 3.10+

maturin:

bash
pip install maturin
# or: cargo install --locked maturin
Develop
bash
# build & install into the current venv, editable
maturin develop

# run tests
pytest
Build a wheel
bash
maturin build --release
# → target/wheels/honba-0.1.0-cp310-abi3-<platform>.whl
pip install target/wheels/honba-*.whl
Because abi3-py310 is set, one wheel per OS/arch works for Python
3.10+. Five wheels total: linux-x64, linux-arm64, macos-arm64,
macos-x64, win-x64.

Architecture constraint
Do not add pyo3 to any other crate in honba-*. If you find yourself
wanting to, the layer boundary is wrong — put the logic in Rust, expose it
here as a thin wrapper.

License: AGPL-3.0.
