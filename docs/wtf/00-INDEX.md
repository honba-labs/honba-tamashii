# Honba WTF (`honba wtf` / `honba.wtf`) — Design Suite Index
> **"Work The Flow": A Plan 9 `acid`-Inspired Trading Environment Built on the Honba Python SDK**

- **Status**: Preliminary Engineering Design (Review Draft)
- **Target Repository**: `honba-tamashii` (`python/honba/wtf/`, `python/honba/cli/wtf.py`, `crates/honba-tamashii-py/`)
- **CLI Entrypoint**: `honba wtf [<options>] [<flow_module_or_path>]` (registered on the single unified `honba` CLI in `honba-tamashii`)

---

## 1. Core Insight: Python IS Our `acid` Language

In Phil Winterbottom's 1994 Plan 9 paper [*Acid: A Debugger Built From A Language*](https://9p.io/sys/doc/acidpaper.html), two lessons stand out:

1. **The Architectural Triumph**: Instead of hardcoding dozens of rigid CLI commands (`gdb`/`dbx`), expose a small set of orthogonal primitives (process control, live memory vs. static file indirection, typed struct formatters) inside an interpreted language so **commands and verification workflows are ordinary functions**.
2. **The #1 Regret (Section 16)**: *"Acid has two areas of weakness. As with other language-based tools like awk, a programmer must learn yet another language to step beyond the normal debugging functions and use the full power of the debugger."*

Honba's **Python-First, Rust-Core** architecture ([python_first.md](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-docs/docs/architecture/python_first.md)) resolves Winterbottom's regret directly:
- **Zero new language or parser to invent**: Workflows are plain, typed Python files (`flows/swing_playbook.py`) using `from honba import wtf`—with full IDE `.pyi` autocompletion, Polars zero-copy DataFrames, `pytest` testability, and Jupyter/Marimo compatibility.
- **Rust owns every hot & safety-critical path** (`honba._honba` via PyO3): Headless TOTP broker auth (`honba-te-auth`), zero-copy Arrow historical series (`honba-me-loaders`), event-driven simulation & WFO/DSR refinement (`honba-nou-research`, `honba-ya-enhancer`), exact `Decimal` math, and mandatory SEBI `algo_id` / OTR / price-band risk gates (`honba-mon`).
- **`acid`-Style Interactive Flow Debugging**: When `honba wtf swing_playbook` hits a `flow.confirm(shortlist)` breakpoint, execution suspends into an embedded **`wtf>` Python/IPython prompt** with live access to every local variable, live market ticks (`flow.live["NSE:TCS"]`), and portfolio state (`flow.live.pf`).

---

## 2. Document Map

| Document | Scope | Key Contents |
|:---|:---|:---|
| **[01-HLD.md](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/docs/wtf/01-HLD.md)** | **High-Level Design** | Honba Rule 1–6 & Option B single-CLI compliance, `acid`-to-Python architectural mapping, Broker auth lifecycle, Sandboxed vs. Existing Portfolio isolation model, Execution modes. |
| **[02-SDK-API-SPEC.md](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/docs/wtf/02-SDK-API-SPEC.md)** | **Python SDK & `wtf>` REPL Spec** | Exact `Decimal` types (`INR`, `Pct`, `R` — float rejected), `Flow` / `Broker` / `Portfolio` builders, `flow.live` vs. `flow.pit` indirection, standard playbook library (`honba.wtf.lib`), and `wtf>` breakpoint shell ergonomics. |
| **[03-LLD.md](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/docs/wtf/03-LLD.md)** | **Low-Level Design & Reference Flow** | `honba-tamashii` package & PyO3 `wtf_bridge.rs` / `server.rs` layout, GIL-free Rust primitives (`py.allow_threads`), Arrow zero-copy contracts, and the complete `swing_playbook.py` end-to-end reference workflow. |

---

## 3. CLI Surface Summary (`honba` Unified CLI)

Under **Option B (`honba-ya-cli` retired)**, `honba-tamashii` ([`honba/cli/main.py`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/python/honba/cli/main.py)) is the **single CLI entrypoint** for the entire Honba workspace (`honba wtf`, `honba serve`, `honba migrate`, `honba backtest`, `honba data`, `honba screener`, `honba verify`, etc.). `honba-ya` contains **zero Rust binaries** and zero upward dependencies on `honba-tamashii`:

```bash
# 1. Run a Python workflow (resolves ./flows/<name>.py, ~/.honba/flows/<name>.py, or path)
honba wtf swing_playbook

# 2. Override flow bindings from CLI without editing Python code (e.g. promote paper -> live)
honba wtf swing_playbook --portfolio existing:live_main --set risk_pct=0.8%

# 3. Drop straight into the `wtf>` interactive Python debugger bound to the flow's context
honba wtf -i swing_playbook

# 4. Non-interactive scheduled execution (stages orders + writes audit artifact, no live commit unless signed off)
honba wtf --batch swing_playbook

# 5. Daemon & DB lifecycle on the same unified CLI (backed by honba-ya-api-rest & honba-ya-persistence)
honba serve --data-dir ../honba-data/data --addr 127.0.0.1:8000
honba migrate --database-url sqlite://honba.db
```

---

## 4. Open Questions for Review

1. **`wtf>` Interactive Shell Backend**: Should the `wtf>` breakpoint prompt use Python's stdlib `code.InteractiveConsole` + `readline` as zero-dependency baseline and automatically upgrade to `IPython.terminal.embed` when IPython is installed in `.venv`?
2. **Flow Discovery Path**: Is resolving `<flow_name>` across `./flows/<name>.py` $\rightarrow$ `honba_strategies/flows/<name>.py` $\rightarrow$ `~/.honba/flows/<name>.py` convenient for how you organize `honba-strategies` and personal playbooks?
3. **Sandboxed Portfolio Default Mode**: Should `wtf.Portfolio.sandbox("name", cash=INR("15L"))` default to `mode="paper"` (live broker market feed + simulated L2 fills, persisted across runs), requiring explicit `mode="live_ringfenced"` before routing real orders to `honba-te`?
