# Low-Level Design (LLD) & Reference Playbook: `honba.wtf`

- **Document**: `03-LLD.md`
- **Part of**: [Honba WTF Design Suite](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/docs/wtf/00-INDEX.md)

---

## 1. Package & Crate Layout (`honba-tamashii`)

With `honba-ya-cli` retired under **Option B**, `honba-tamashii` owns the **single unified `honba` CLI** across the platform and links pure-Rust `honba-ya` libraries (`honba-ya-api-rest`, `honba-ya-persistence`, `honba-ya-enhancer`) directly without any upward `honba-ya -> honba-tamashii` dependency cycle. `honba.wtf` plugs cleanly into this exact structure:

```text
honba-tamashii/
├── _honba.pyi                        # Generated PEP-484 stubs (`serve_api`, `run_migrations`, `wtf_*`, ...)
├── crates/
│   └── honba-tamashii-py/
│       ├── Cargo.toml                # Links honba-ya-api-rest, honba-ya-persistence, honba-te-auth, honba-kokoro-portfolio, honba-nou-research, honba-ya-enhancer
│       └── src/
│           ├── lib.rs                # Registers `server::register(m)?` and `wtf_bridge::register(m)?`
│           ├── server.rs             # GIL-free `serve_api`, `run_migrations`, `import_ticks_ndjson`
│           └── wtf_bridge.rs         # GIL-free `wtf_*` Rust primitives (`py.allow_threads`)
└── python/honba/
    ├── cli/
    │   ├── main.py                   # Single CLI entrypoint (`honba`): `serve`, `migrate`, `backtest`, `wtf`, ...
    │   ├── serve.py                  # `honba serve` & `honba migrate` wrapping `honba._honba`
    │   └── wtf.py                    # `honba wtf` loader, `--set` parser, `-i` / `--batch` driver
    └── wtf/
        ├── __init__.py               # Exports Flow, Broker, Portfolio, INR, Pct, R, Sym, show
        ├── types.py                  # Strict Decimal subclasses (INR, Pct, R) rejecting float
        ├── context.py                # Flow context manager, live/pit descriptors, CLI overrides
        ├── secrets.py                # Unified adapter over FileSecretStore & Rust CredentialStore
        ├── repl.py                   # Embedded `wtf>` Python/IPython breakpoint console
        └── lib/
            ├── __init__.py
            ├── screen.py             # scan_grade() -> list[SetupCandidate]
            ├── devise.py             # devise_rules() -> StrategySpec
            ├── detrap.py             # 5-trap eliminator + sim_refine() wrapper
            ├── sizing.py             # Grade-weighted asymmetric R-multiple sizer
            └── oms.py                # stage(), commit(), reconcile(), halt()
```

---

## 2. PyO3 Native Bridge (`crates/honba-tamashii-py/src/wtf_bridge.rs`)

Following [`python_first.md §3.1`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-docs/docs/architecture/python_first.md) and [`crates/honba-tamashii-py/src/server.rs`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/crates/honba-tamashii-py/src/server.rs), all compute-heavy and stateful Rust operations release the Python GIL via `py.allow_threads` and marshal monetary values strictly through [`src/decimal.rs`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/crates/honba-tamashii-py/src/lib.rs#L35):

```rust
use pyo3::prelude::*;
use crate::decimal::PyDecimal; // Refuses Python `float`, converts `decimal.Decimal` <-> `rust_decimal::Decimal`

#[pyclass(frozen)]
pub struct PyBrokerSession {
    pub venue: String,
    pub(crate) inner: std::sync::Arc<BrokerSessionInner>, // Wraps honba_te_auth::SessionManager + BrokerExecutor
}

#[pyclass]
pub struct PyPortfolioHandle {
    pub id: String,
    pub is_sandbox: bool,
    pub(crate) binding: std::sync::Mutex<PortfolioBindingState>,
}

/// Headless broker login / cached 08:30 IST token lookup via `honba-te-auth`.
#[pyfunction]
pub fn wtf_connect_broker(
    py: Python<'_>,
    venue: &str,
    cred_source: &str,
    auth_mode: &str,
    edis_policy: &str,
) -> PyResult<PyBrokerSession> {
    py.allow_threads(|| connect_broker_blocking(venue, cred_source, auth_mode, edis_policy))
        .map_err(to_py_err)
}

/// Multi-symbol setup scan + VCP/RVOL/RS grading over Arrow lakehouse slices.
#[pyfunction]
pub fn wtf_scan_grade(
    py: Python<'_>,
    symbols: Vec<String>,
    config_json: &str,
    as_of_utc: &str,
) -> PyResult<String> {
    py.allow_threads(|| run_rust_scan_grade(&symbols, config_json, as_of_utc))
        .map_err(to_py_err)
}

/// Closed-loop parity backtest + AI refinement + WFO/Monte Carlo/DSR/PBO verification.
#[pyfunction]
pub fn wtf_sim_refine(
    py: Python<'_>,
    spec_json: &str,
    loop_cfg_json: &str,
) -> PyResult<String> {
    py.allow_threads(|| {
        // Calls honba_nou_research::ParityBacktestRunner (with IndianStatutoryCostCalculator)
        // + honba_ya_enhancer::run_closed_loop_refinement + StatisticalVerifier
        run_parity_refinement(spec_json, loop_cfg_json)
    })
    .map_err(to_py_err)
}

/// Synchronous pre-trade `honba-mon::RiskEngine` evaluation & OMS staging.
#[pyfunction]
pub fn wtf_stage_orders(
    py: Python<'_>,
    broker: &PyBrokerSession,
    portfolio: &PyPortfolioHandle,
    algo_id: &str,
    orders_json: &str,
) -> PyResult<String> {
    py.allow_threads(|| stage_with_risk_gate(broker, portfolio, algo_id, orders_json))
        .map_err(to_py_err)
}
```

### Zero-Copy Historical Addressing (`flow.pit.bars`)
`flow.pit.bars(sym, timeframe, lookback)` delegates directly to the existing Arrow PyCapsule bridge in `crates/honba-tamashii-py/src/arrow.rs`:
1. `honba-me-loaders` reads Parquet bars and applies corporate-action adjustments in Rust.
2. An Arrow `RecordBatch` pointer is handed across `PyCapsule` (`__arrow_c_stream__`) to `polars.from_arrow()` in microseconds with **zero serialization overhead**.

---

## 3. Sandboxed vs. Existing Portfolio Mechanics

`PyPortfolioHandle` reuses the atomic compare-and-swap JSON document model of [`portfolio_store.rs`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-ya/crates/honba-ya-api-rest/src/portfolio_store.rs):

1. **Storage Layout**:
   - `Portfolio.existing("live_main")` $\rightarrow$ `~/.honba/portfolios/accounts/live_main.json`
   - `Portfolio.sandbox("vcp_swing_nov26", cash=INR("15L"))` $\rightarrow$ `~/.honba/portfolios/sandboxes/vcp_swing_nov26.json` (unless `ephemeral=True`, which uses `MemoryStore`).
2. **Capital & Order Ring-Fencing**:
   - Every order staged from a sandbox carries `client_order_id = f"sbx:{sandbox_id}:{ulid}"`.
   - `free_cash` for sizing is strictly `allocated_cash + realized_sandbox_pnl - open_sandbox_cost - reserved_statutory_fees`.
   - Even in `mode="live_ringfenced"`, `oms.close_all(flow)` or drawdown breakers **only** touch positions matching `sbx:<sandbox_id>:*`, guaranteeing zero interference with core long-term Demat holdings in the same broker account.
3. **Exact Statutory Cost Accounting**:
   - Both `paper` sandboxes and `sim_refine` invoke `honba_nou_research::IndianStatutoryCostCalculator` on every simulated or staged trade (brokerage cap, STT delivery vs. intraday, NSE/BSE turnover charge, 18% GST, stamp duty, and SEBI turnover fee) so $R$-multiple expectancy reflects true net-of-cost Indian equity returns.

---

## 4. Interactive `wtf>` Breakpoint Implementation (`python/honba/wtf/repl.py`)

When a Python playbook calls `shortlist = flow.confirm(shortlist, prompt=...)`:

```python
def confirm(self, value: T, prompt: str = "Confirm before continuing") -> T:
    wtf_show(value)
    self._render_status_bar(proposed=value)

    if self.batch_mode:
        self._journal_staged_artifact(value, prompt)
        if not self.auto_commit:
            raise BatchConfirmStagedOnly(prompt)
        return value

    # Capture caller stack locals + inject concise acid-style helpers
    caller_locals = inspect.currentframe().f_back.f_locals.copy()
    box = {"result": value, "resume": False}

    env = {
        **caller_locals,
        "flow": self,
        "show": wtf_show,
        "live": lambda s: self.live[s],
        "pit": lambda s: self.pit.bars(s),
        "drop": _drop_helper,
        "cont": lambda updated=None: _trigger_continue(box, updated, env, value_name=...),
        "abort": _trigger_abort,
        "halt": lambda reason="operator halt": oms.halt(self, reason),
    }
    run_wtf_console(banner=f"[stopped] {prompt}", local_ns=env)
    return box["result"]
```

- Uses `IPython.terminal.embed.InteractiveShellEmbed(prompts_class=WtfPrompts)` if available in `.venv`, falling back cleanly to Python's stdlib `code.InteractiveConsole` + `readline` tab-completion.

---

## 5. End-to-End Reference Playbook: `flows/swing_playbook.py`

Below is the complete, idiomatic Python workflow implementing:
**Screen ($A+, A, B, C$) $\rightarrow$ Devise Rules $\rightarrow$ Jesse-Style AI Sim & 5-Trap Purge $\rightarrow$ Asymmetric $R$ Position Sizing $\rightarrow$ Interactive `wtf>` Inspection $\rightarrow$ SEBI-Gated OMS Staging**.

```python
"""Quant Swing Playbook: Graded Setups -> AI Sim De-Trap -> Asymmetric R Sizing."""

from decimal import Decimal
from honba import wtf
from honba.wtf import INR, Pct, R, Sym
from honba.wtf.lib import detrap, devise, oms, screen, sizing

# 1. Define Flow Context: Zerodha (headless TOTP) + Ring-Fenced ₹15L Sandbox Portfolio
flow = wtf.Flow(
    name="swing_playbook",
    broker=wtf.Broker.zerodha(
        secrets=wtf.secrets.keyring("zerodha:primary"),
        auth="totp_headless",
        edis="on_sell",
    ),
    portfolio=wtf.Portfolio.sandbox(
        "vcp_swing_nov26",
        cash=INR("15L"),
        mode="paper",          # Pass `--portfolio existing:live_main` on CLI when promoting
        max_dd=Pct("5%"),
    ),
    algo_id="SEBI_REG_SWING_V1",
    params={
        "base_risk": Pct("1.0%"),     # 1R = ₹15,000 on ₹15L capital
        "max_pos_pct": Pct("25%"),    # Max ₹3.75L–₹6.5L exposure cap per scrip
        "min_grade": "B",
    },
)


@flow.on_trap
def _explain_trap(sym: Sym, verdict: detrap.TrapVerdict) -> None:
    print(f"  [trap] {sym:<14} -> {verdict.code:<18} ({verdict.reason})")


@flow.main
def run(f: wtf.Flow) -> None:
    # 2. Screen NSE 500 for A+, A, B, C setups; discard C-grade noise automatically
    universe = f.pit.universe("nse500")
    candidates = screen.scan_grade(
        f,
        universe,
        patterns=["vcp_contraction", "pocket_pivot", "ema20_pullback"],
        min_rs=70,
        min_grade=f.params["min_grade"],
    )
    print(f"[wtf] Screened {len(candidates)} setups >= grade {f.params['min_grade']}")

    # 3. Devise Rules & Run Jesse-Style Closed-Loop Sim to Purge All 5 Market Traps
    trap_cfg = detrap.TrapConfig(
        lookback_days=120,
        max_lower_circuit_hits=0,        # Trap 1: Circuit lock trap
        min_circuit_band=Pct("10%"),
        earnings_buffer_sessions=5,      # Trap 2: Earnings / board / dividend gap trap
        min_median_turnover=INR("5Cr"),  # Trap 3: Illiquidity & L2 spread trap
        max_spread_bps=Decimal("15"),
        max_upper_wick_ratio=Decimal("0.35"),  # Trap 4: False-breakout rejection wick trap
    )
    sim_cfg = detrap.SimLoopConfig(
        ai_rounds=4,                     # Trap 5: Overfitting trap (WFO + Monte Carlo + DSR/PBO)
        walk_forward_window="6m",
        monte_carlo_runs=500,
        min_wfe=0.60,
        min_dsr=0.95,
        max_pbo=0.10,
    )

    survivors = []
    for setup in candidates:
        if verdict := detrap.is_trapped(f, setup, trap_cfg):
            f.emit_trap(setup.sym, verdict)
            continue

        spec = devise.devise_rules(
            setup,
            entry_buffer=Pct("0.05%"),
            stop_atr=Decimal("1.5"),
            breakeven_at=R("1.0"),
            trail_after=R("2.0"),
            trail_mode="chandelier_2atr",
            partial_target=R("2.5"),
            partial_exit_pct=Pct("50%"),
        )
        outcome = detrap.sim_refine(f, setup, spec, sim_cfg)
        if outcome.passed:
            survivors.append(outcome.candidate)
        else:
            f.emit_trap(setup.sym, outcome.trap_verdict)

    # 4. Asymmetric R-Multiple Position Sizing (A+: 1.25R, A: 1.0R, B: 0.5R)
    shortlist = sizing.size_shortlist(
        f,
        survivors,
        base_risk=f.params["base_risk"],
        max_pos_pct=f.params["max_pos_pct"],
        grade_weights={"A+": R("1.25"), "A": R("1.00"), "B": R("0.50")},
    )
    if not shortlist:
        print("[wtf] Zero candidates survived de-trapping & WFO/DSR verification.")
        return

    # 5. Acid-Style Interactive Breakpoint: Inspect, Filter in Python, and Commit
    shortlist = f.confirm(shortlist, "Review shortlisted setups before SEBI OMS staging")
    staged = oms.stage(f, shortlist)
    acks = oms.commit(f, staged)
    print(f"[wtf] Committed {len(acks)} bracket orders into {f.portfolio.id}")
```

### Operator Terminal Session (`honba wtf swing_playbook`)

```text
$ honba wtf swing_playbook
[wtf] Flow 'swing_playbook' | Broker: zerodha (TOTP cached 08:30 IST) | Portfolio: sandbox:vcp_swing_nov26 (₹15.00L, paper)
[wtf] Screened 12 setups >= grade B
  [trap] NSE:ZEEL       -> CIRCUIT_TRAP       (2 lower-circuit locks in 120d)
  [trap] NSE:TATAMOTORS -> EARNINGS_GAP_TRAP  (Q2 results board meeting in 3 sessions)
  [trap] NSE:SUZLON     -> OVERFIT_TRAP       (WFE 0.41 < 0.60 after 4 AI refinement rounds)
...
┌──────────────┬───────┬───────────┬───────────┬─────────────────────┬───────────┬───────┬─────┬──────────┬─────────┬──────┬──────┐
│ Sym          │ Grade │ Entry     │ Stoploss  │ Trail Criteria      │ Target 1  │ R:R   │ Qty │ Risk (R) │ Value   │ WFE  │ DSR  │
├──────────────┼───────┼───────────┼───────────┼─────────────────────┼───────────┼───────┼─────┼──────────┼─────────┼──────┼──────┤
│ NSE:TCS      │ A+    │ ₹4,120.00 │ ₹4,025.00 │ BE@1R -> 2.0xATR    │ ₹4,357.50 │ 2.50R │ 197 │ ₹18,715  │ ₹8.11L  │ 0.74 │ 0.98 │
│ NSE:INFY     │ A     │ ₹1,898.00 │ ₹1,845.00 │ BE@1R -> 2.0xATR    │ ₹2,030.50 │ 2.50R │ 283 │ ₹14,999  │ ₹5.37L  │ 0.68 │ 0.97 │
│ NSE:HDFCBANK │ B     │ ₹1,650.00 │ ₹1,610.00 │ BE@1R -> 2.0xATR    │ ₹1,750.00 │ 2.50R │ 187 │ ₹7,480   │ ₹3.08L  │ 0.63 │ 0.96 │
└──────────────┴───────┴───────────┴───────────┴─────────────────────┴───────────┴───────┼─────┴──────────┴─────────┴──────┴──────┘
  Free Cash: ₹15.00L | Proposed Value: ₹16.56L | SEBI Algo: SEBI_REG_SWING_V1 (OTR: 4.2/50)
[stopped] Review shortlisted setups before SEBI OMS staging

wtf> live("NSE:HDFCBANK")
QuoteTick(sym='NSE:HDFCBANK', ltp=INR('1648.40'), lower_band=INR('1485.00'), upper_band=INR('1815.00'))

wtf> shortlist = [t for t in shortlist if t.grade in ("A+", "A")]   # Standard Python comprehension!
wtf> show(shortlist)
  [2 setups: NSE:TCS (₹8.11L), NSE:INFY (₹5.37L) — Total Value: ₹13.48L <= ₹15.00L Free Cash]

wtf> cont(shortlist)
[wtf] RiskEngine PASSED (algo_id=SEBI_REG_SWING_V1, OTR=4.4, bands verified).
[wtf] Committed 2 bracket orders into sandbox:vcp_swing_nov26
```

---

## 6. Implementation Phasing

| Phase | Milestone | Files / Crates | Verification Gate |
|:---:|:---|:---|:---|
| **P0 (Done)** | Retire `honba-ya-cli` (Option B) & Unify `honba` CLI (`serve`, `migrate`, `data ticks-import`) | `honba-ya-api-rest/src/{bootstrap,ticks_file}.rs`, `honba-tamashii-py/src/server.rs`, `python/honba/cli/serve.py` | `dependency_graph.py` (0 Rust bins, 0 cycles), `check_stub_compatibility.py`, `test_cli_serve.py` |
| **P1** | Strict Decimal Types (`INR`, `Pct`, `R`) & `Flow` / `Portfolio` Python Models | `python/honba/wtf/{types,context,secrets}.py` | Hypothesis property tests ensuring `float` rejection & Indian unit parsing (`15L`, `5Cr`) |
| **P2** | PyO3 `wtf_bridge.rs` (Auth, Sandbox Ledger, Risk Gate) | `crates/honba-tamashii-py/src/wtf_bridge.rs`, `_honba.pyi` | `check_stub_compatibility.py` + CAS sandbox isolation unit tests |
| **P3** | Quant Playbook Library (`screen`, `devise`, `detrap`, `sizing`, `oms`) | `python/honba/wtf/lib/*.py` | Golden conformance tests over deterministic Lakehouse fixtures |
| **P4** | `honba wtf` Typer CLI + Embedded `wtf>` Breakpoint Console | `python/honba/cli/wtf.py`, `python/honba/wtf/repl.py` | End-to-end TTY (`pexpect` / `pytest`) & `--batch` staging tests |
