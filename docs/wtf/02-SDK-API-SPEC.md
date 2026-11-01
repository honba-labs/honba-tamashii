# Python SDK & `wtf>` Debugger Specification (`honba.wtf`)

- **Document**: `02-SDK-API-SPEC.md`
- **Part of**: [Honba WTF Design Suite](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/docs/wtf/00-INDEX.md)

---

## 1. Exact Financial Value Types (`INR`, `Pct`, `R`, `Sym`)

In line with [`honba-tamashii-py/src/lib.rs`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/crates/honba-tamashii-py/src/lib.rs#L17-L20) (*"Money never crosses as a float... the boundary refuses a Python `float` outright"*), `honba.wtf` provides thin `decimal.Decimal` subclasses with Indian unit parsing (reusing [`honba.query.quantity`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/python/honba/query/quantity.py)):

```python
from decimal import Decimal
from honba.wtf import INR, Pct, R, Sym

cash  = INR("15L")         # Decimal('1500000.00')  -- also accepts "2.5Cr", "50k", int, Decimal
stop  = INR("1845.50")     # Decimal('1845.50')
risk  = Pct("1.0%")        # Decimal('0.010')
rr    = R("2.5")           # Decimal('2.50')
sym   = Sym("NSE:INFY")    # honba.wire.InstrumentId (bare "INFY" defaults to NSE:INFY)

INR(1845.50)               # TypeError: float forbidden for INR; pass str, int, or Decimal
```

### Presentation Views (`acid` Format Codes in Python)
Every value and domain model integrates directly with [`honba.display`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/python/honba/display.py):
- `cash.inr` $\rightarrow$ `'₹15,00,000.00'` (Indian Lakh/Crore comma grouping)
- `cash.compact` $\rightarrow$ `'₹15.00L'` (`'₹2.50Cr'` for $\ge 10^7$)
- `wtf.show(records, fmt="table" | "plain" | "json" | "csv")` $\rightarrow$ Renders deterministic Rich/ASCII tables (such as the Actionable Trade Card) or machine-verifiable JSON.

---

## 2. Declarative `Flow`, `Broker`, and `Portfolio` API

```python
from honba import wtf
from honba.wtf import INR, Pct

flow = wtf.Flow(
    name="swing_playbook",
    broker=wtf.Broker.zerodha(
        secrets=wtf.secrets.keyring("zerodha:primary"),  # or wtf.secrets.file("~/.honba/secrets.enc")
        auth="totp_headless",
        edis="on_sell",
    ),
    portfolio=wtf.Portfolio.sandbox(
        "vcp_swing_nov26",
        cash=INR("15L"),
        mode="paper",            # "paper" | "backtest" | "live_ringfenced"
        max_dd=Pct("5%"),
        ephemeral=False,
    ),
    algo_id="SEBI_REG_SWING_V1",
    params={                     # Overridable from CLI via `--set key=val`
        "risk_pct": Pct("1.0%"),
        "max_pos_pct": Pct("25%"),
        "min_grade": "B",
    },
)
```

### Supported `Broker` Constructors
- `Broker.zerodha(secrets, auth="totp_headless", edis="on_sell")`
- `Broker.upstox(secrets, auth="oauth_headless", edis="on_sell")`
- `Broker.angelone(secrets, auth="smart_totp")`
- `Broker.paper(feed="zerodha", fill_model="l2_realistic")` — Live ticks from `honba-me`, simulated L2 fills via [`honba.backtest.l2_sim`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/python/honba/backtest/l2_sim.py).
- `Broker.sim(slippage_bps=Decimal("5"))` — Offline historical backtest venue.
- `Broker.custom(adapter)` — Community [`BrokerAdapter`](file:///home/pmallapp/Devel/Finance/honba-workspace/honba-tamashii/python/honba/adapters/base.py) instance (still gated by Rust `honba-mon`).

### Supported `Portfolio` Constructors
- `Portfolio.existing(account_id: str)` — Loads persisted portfolio with CAS revision check and reconciles T+1 settlement & broker positions.
- `Portfolio.sandbox(sandbox_id: str, *, cash: INR, mode: str = "paper", max_dd: Pct = Pct("5%"), parent: str | None = None, ephemeral: bool = False)` — Ring-fenced virtual sub-portfolio.

---

## 3. Dual State Addressing: `flow.live` vs. `flow.pit`

Inspired directly by `acid`'s `*addr` (live memory) and `@addr` (static binary/file image):

### 3.1 `flow.live` — Live Runtime & Broker State
```python
q     = flow.live["NSE:TCS"]          # -> QuoteTick (ltp, bid, ask, lower_band, upper_band, rvol)
depth = flow.live.depth("NSE:TCS")    # -> Depth (20-level L2 book, spread_bps)
snap  = flow.live.pf                  # -> PortfolioSnapshot (nav, free_cash, positions, mtm_pnl)
otr   = flow.live.otr                 # -> OtrSnapshot (ratio, headroom, algo_id, status)
ords  = flow.live.orders              # -> list[LedgerOrder] (working & filled orders)
```

### 3.2 `flow.pit` — Point-in-Time Lakehouse State (Zero Look-Ahead)
```python
# Zero-copy Polars DataFrame via PyArrow PyCapsule (`honba._honba.arrow`)
bars_df = flow.pit.bars("NSE:TCS", timeframe="1d", lookback=250)

# Point-in-time DuckDB fundamentals (`filing_date <= flow.as_of`)
fund    = flow.pit.fundamentals("NSE:TCS")

# Point-in-time index constituents (`list[InstrumentId]`)
syms    = flow.pit.universe("nse500")
```

---

## 4. Standard Playbook Library (`honba.wtf.lib`)

Following `acid`'s library model (`/lib/acid/*`), reusable quant steps are ordinary, composable Python functions in `honba.wtf.lib`:

| Module & Function | Signature & Responsibility |
|:---|:---|
| **`screen.scan_grade`** | `scan_grade(flow, syms, *, patterns, min_rs=70, min_grade="B") -> list[SetupCandidate]`<br>Scores multi-timeframe VCP contraction, pocket-pivot RVOL ($\ge 2.5\times$), RS vs. Nifty 50, and circuit-band cushion into `"A+"`, `"A"`, `"B"`, `"C"`. |
| **`devise.devise_rules`** | `devise_rules(setup, *, entry_buf=Pct("0.05%"), stop_atr=Decimal("1.5"), trail="be_1r_chandelier_2atr", target_r=R("2.5")) -> StrategySpec`<br>Produces a deterministic closed-grammar `StrategySpec` with entry trigger, initial stop, Jesse-style breakeven shift at $+1R$, dynamic trailing stop, and partial/runner exits. |
| **`detrap.is_trapped`** | `is_trapped(flow, setup, cfg: TrapConfig) -> TrapVerdict | None`<br>Checks historical lower-circuit locks (`CIRCUIT_TRAP`), upcoming earnings/board/ex-dividend dates (`EARNINGS_GAP_TRAP`), 20d median turnover & L2 spread (`LIQUIDITY_TRAP`), and upper-wick rejection rate (`WICK_TRAP`). |
| **`detrap.sim_refine`** | `sim_refine(flow, setup, spec, cfg: SimLoopConfig) -> SimRefinementOutcome`<br>Runs GIL-free Rust `ParityBacktestRunner` + `honba-ya-enhancer` closed-loop AI refinement (WFO + Monte Carlo + DSR $\ge 0.95$, PBO $< 0.10$, WFE $\ge 0.60$) to eliminate `OVERFIT_TRAP`. |
| **`sizing.size_shortlist`** | `size_shortlist(flow, survivors, *, base_risk: Pct, max_pos_pct: Pct, grade_weights=...) -> list[SizedTrade]`<br>Allocates asymmetric $R$ ($A+: 1.25R$, $A: 1.0R$, $B: 0.5R$), computes exact share quantity `floor(risk_inr / (entry - stop))`, rounds to lot size, deducts `IndianStatutoryCostCalculator` fees, and caps exposure. |
| **`oms.stage` / `commit`** | `stage(flow, plan) -> StagedBatch` / `commit(flow, batch) -> list[OrderAck]`<br>Validates SEBI `algo_id`, checks `honba-mon` OTR & price bands, stages bracket/GTT orders, and commits atomically with `SqliteAuditLedger` provenance. |
| **`oms.halt`** | `halt(flow, reason: str) -> TradingState`<br>Triggers immediate `honba-mon` kill-switch, cancels working orders in scope, and sets Reduce-Only mode. |

---

## 5. Interactive `wtf>` Breakpoint Shell & Hooks

### 5.1 In-Flight Breakpoints (`flow.confirm`)
Calling `shortlist = flow.confirm(shortlist, "Review shortlisted setups")` in an interactive TTY:
1. Renders `wtf.show(shortlist)` plus a compact status footer (Portfolio Free Cash, Proposed Exposure, SEBI OTR ratio).
2. Opens an embedded Python REPL (`wtf>`) whose namespace contains **the caller's live locals** (`shortlist`, `survivors`, `flow`) plus concise `acid`-inspired shell helpers:
   - `show(x, fmt="table")` — Render any object/list/DataFrame.
   - `live("NSE:TCS")` / `pit("NSE:TCS")` — Quick state probe.
   - `drop(items, *syms_or_grades)` — Return filtered trade list (e.g. `shortlist = drop(shortlist, "B", "NSE:HDFCBANK")`).
   - `cont(return_val=None)` — Exit `wtf>` and resume the workflow (returning the updated `shortlist` from `flow.confirm()`!).
   - `abort(reason="operator cancelled")` — Raise `FlowAborted` cleanly without submitting orders.
   - `halt(reason="emergency")` — Immediately invoke `oms.halt(flow, reason)`.

### 5.2 Programmable Event Hooks (`@flow.on_trap`, `@flow.on_stopped`)
Just as `acid` lets users override `stopped(pid)` to customize debugger reactions:

```python
@flow.on_trap
def log_trap(sym: Sym, verdict: wtf.TrapVerdict) -> None:
    print(f"[trap] {sym:<14} -> {verdict.code}: {verdict.summary}")
```
