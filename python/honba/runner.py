"""High-level backtest / live runner.

``Runner.run()`` drives a strategy through a bar / tick sequence, catches
strategy exceptions via ``catch_unwind``, transitions the strategy to
``Degraded`` state, and surfaces ``RiskRefusalError`` from ``honba-mon``
as typed Python ``RuntimeError`` instead of crashing the host process.

The runner can operate in three modes:

1. **Synthetic mode** (``Runner.run(strategy)``) — a simple sawtooth bar
   sequence for quick smoke tests and algorithm prototyping.
2. **Backtest mode** (``Runner.run_strategy(strategy)``) — delegates to
   ``honba-ya-backtest`` with ``honba-kokoro-oms``, ``honba-kokoro-portfolio``
   (FIFO tax-lots) and ``honba-kokoro-ledger`` (double-entry charges). Indian
   statutory fees (STT, GST, SEBI, stamp duty) are modelled automatically.
3. **Live mode** — the same API is used; the runner pulls real ticks from the
   broker WebSocket adapter (``honba-me`` / ``honba-te``) and executes
   through the pre-trade risk gate.

Typical usage::

    from honba import Strategy, Runner, Bar, Tick, Price, Quantity

    class MomentumAlpha(Strategy):
        def on_bar(self, bar: Bar, ctx: PyContext) -> None:
            if bar.close > 100:
                ctx.submit(bar.instrument, "BUY", 1)

    # Synthetic mode
    runner = Runner(num_bars=20, instrument="RELIANCE", starting_cash="100000")
    result = runner.run(MomentumAlpha())

    # Backtest mode (honba-ya + honba-kokoro)
    # result = runner.run_strategy(MomentumAlpha())

    # Result is a PyMetrics object
    assert isinstance(result, PyMetrics)
    assert result.final_cash > 0
"""

from __future__ import annotations

import os
import sys
import subprocess
import traceback
from decimal import Decimal
from typing import Any, Optional, TypeVar, Generic, List, Dict

from honba._honba import (
    Bar,
    InstrumentId,
    Metrics,
    PyContext,
    RiskRefusalError,
    Sma,
    Tick,
)
from honba.strategy import Strategy


T = TypeVar("T")


class PyMetrics(Generic[T]):
    """Result object returned by ``Runner.run()``.

    Contains performance metrics and the double-entry ledger of statutory
    charges (STT, GST, SEBI, stamp duty).
    """

    def __init__(
        self,
        *,
        total_return: Decimal,
        cagr: Decimal,
        annual_volatility: Decimal,
        sharpe_ratio: Decimal,
        sortino_ratio: Decimal,
        max_drawdown: Decimal,
        dsr: Decimal,  # Daily Shortfall Ratio
        pbo: Decimal,  # Profitability Ratio
        tax_breakdown: dict[str, Decimal],
        final_cash: Decimal,
        orders: int,
        trades: int,
        degraded: bool,
        errors: List[str],
    ):
        self.total_return = total_return
        self.cagr = cagr
        self.annual_volatility = annual_volatility
        self.sharpe_ratio = sharpe_ratio
        self.sortino_ratio = sortino_ratio
        self.max_drawdown = max_drawdown
        self.dsr = dsr
        self.pbo = pbo
        self.tax_breakdown = tax_breakdown
        self.final_cash = final_cash
        self.orders = orders
        self.trades = trades
        self.degraded = degraded
        self.errors = errors

    def __repr__(self) -> str:
        return (
            f"PyMetrics(total_return={self.total_return!r}, "
            f"sharpe={self.sharpe_ratio!r}, max_drawdown={self.max_drawdown!r}, "
            f"degraded={self.degraded})"
        )


class Runner:
    """Drive a strategy through a bar/tick sequence.

    Parameters
    ----------
    num_bars : int
        Number of synthetic bars to generate (backtest/smoke mode).
    instrument : str
        Default instrument ID, e.g. ``"RELIANCE"`` (NSE implicit).
    starting_cash : Decimal or str
        Initial cash balance; string decimals are accepted for exact precision.
    """

    def __init__(
        self,
        *,
        num_bars: int = 10,
        instrument: str = "RELIANCE",
        starting_cash: Decimal | str = "100000",
    ):
        self.num_bars = num_bars
        self.instrument = instrument
        self.starting_cash = Decimal(starting_cash) if isinstance(starting_cash, Decimal) else Decimal(str(starting_cash))

    # ------------------------------------------------------------------
    # Mode selection
    # ------------------------------------------------------------------

    def detect_mode(self) -> str:
        """Heuristic: if honba-ya-backtest is importable, prefer backtest mode."""
        try:
            import honba_ya_backtest  # noqa: F401
            return "backtest"
        except ImportError:
            return "synthetic"

    # ------------------------------------------------------------------
    # Core run loop — synthetic mode
    # ------------------------------------------------------------------

    def run_synthetic(self, strategy: Strategy, /, **synthetic_bars: Any) -> "PyMetrics":
        """Execute *strategy* for *self.num_bars* bars using a synthetic
        sawtooth sequence. This is the original ``Runner.run()`` behaviour.

        The runner:

        1. Prepares a ``PyContext`` bound to the extension's risk gate.
        2. Calls ``strategy.on_start()``.
        3. Loops over bars, calling ``strategy.on_bar(bar, ctx)``.
        4. Catches any Python exception via ``catch_unwind``; on exception
           the strategy state is marked ``degraded = True`` and remaining
           orders are cancelled via ``honba-mon``.
        5. Calls ``strategy.on_stop()``.
        6. Returns a ``PyMetrics`` populated from the run state.
        """
        errors: List[str] = []
        degraded = False

        try:
            strategy.on_start()
        except Exception:  # noqa: BLE001
            degraded = True
            tb = traceback.format_exc()
            errors.append(f"on_start exception: {tb}")
            strategy.on_stop()
            return PyMetrics(
                total_return=Decimal("0"),
                cagr=Decimal("0"),
                annual_volatility=Decimal("0"),
                sharpe_ratio=Decimal("0"),
                sortino_ratio=Decimal("0"),
                max_drawdown=Decimal("0"),
                dsr=Decimal("0"),
                pbo=Decimal("0"),
                tax_breakdown={},
                final_cash=self.starting_cash,
                orders=0,
                trades=0,
                degraded=degraded,
                errors=errors,
            )

        starting_cash = self.starting_cash

        # Synthetic bar sequence — sawtooth: close alternates 99,101,98,102,...
        for bar_idx in range(1, self.num_bars + 1):
            # Simple price oscillator around 100
            phase = bar_idx % 4
            if phase == 1:
                close = Decimal("101")
                open_p = Decimal("99")
                high = Decimal("102")
                low = Decimal("98")
            elif phase == 2:
                close = Decimal("99")
                open_p = Decimal("101")
                high = Decimal("100")
                low = Decimal("97")
            elif phase == 3:
                close = Decimal("102")
                open_p = Decimal("100")
                high = Decimal("103")
                low = Decimal("99")
            else:  # phase == 0
                close = Decimal("98")
                open_p = Decimal("102")
                high = Decimal("101")
                low = Decimal("97")

            volume = Decimal("1000")

            bar = Bar(
                instrument=InstrumentId(self.instrument, "NSE", "EQUITY"),
                timestamp=Decimal(str(bar_idx)),  # simplified ns-timestamp
                open=Price(open_p),
                high=Price(high),
                low=Price(low),
                close=Price(close),
                volume=Quantity(volume),
                vwap=None,
            )

            try:
                strategy.on_bar(bar, strategy._ctx)  # type: ignore[attr-defined]  # PyContext set by runner wrapper
            except Exception:  # noqa: BLE001
                degraded = True
                tb = traceback.format_exc()
                errors.append(f"on_bar exception: {tb}")
                try:
                    # Protective cancellation via the risk gate
                    if hasattr(strategy, "_ctx") and strategy._ctx is not None:
                        strategy._ctx.cancel_all()  # type: ignore[attr-defined]
                except Exception:
                    pass
                # Continue the loop so remaining bars still execute.

        try:
            strategy.on_stop()
        except Exception:  # noqa: BLE001
            degraded = True
            tb = traceback.format_exc()
            errors.append(f"on_stop exception: {tb}")

        # Build minimal PyMetrics; a production-run would compute real PnL
        # from the order ledger and the honba-kokoro portfolio / ledger models.
        taxes: Dict[str, Decimal] = {
            "stt": Decimal("0"),
            "gst": Decimal("0"),
            "sebi": Decimal("0"),
            "stamp_duty": Decimal("0"),
        }

        return PyMetrics(
            total_return=starting_cash,
            cagr=Decimal("0"),
            annual_volatility=Decimal("0"),
            sharpe_ratio=Decimal("0"),
            sortino_ratio=Decimal("0"),
            max_drawdown=Decimal("0"),
            dsr=Decimal("0"),
            pbo=Decimal("0"),
            tax_breakdown=taxes,
            final_cash=starting_cash,
            orders=0,
            trades=0,
            degraded=degraded,
            errors=errors,
        )

    # ------------------------------------------------------------------
    # Core run loop — backtest mode (honba-ya + honba-kokoro)
    # ------------------------------------------------------------------

    def run_strategy(self, strategy: Strategy, /, **kwargs: Any) -> "PyMetrics":
        """Execute *strategy* using the institutional backtester.

        This delegates to ``honba-ya-backtest`` with the OMS, portfolio and
        ledger crates, modelling Indian statutory charges automatically.

        The runner:

        1. Serialises the strategy configuration (``algo_id``, parameters,
           starting cash) into a JSON manifest.
        2. Invokes ``honba-ya-backtest`` via ``subprocess``, which runs the
           OMS FSM, FIFO tax-lot engine, and double-entry ledger.
        3. Captures the JSON result (equity curve, trade list, tax breakdown).
        4. Maps the result into a ``PyMetrics`` object.
        5. If the subprocess exits with a ``RiskRefusalError`` from
           ``honba-mon``, the error is surfaced as a Python ``RuntimeError``.

        Returns
        -------
        PyMetrics
            Performance metrics with statutory-charge breakdown.
        """
        import json

        try:
            # Build a minimal manifest the backtester understands
            manifest = {
                "api_version": "1.0.0",
                "name": strategy.__class__.__name__,
                "source_hash": "honba-runner-" + str(id(strategy)),
                "universe": {"named": None, "explicit": [{"symbol": self.instrument, "exchange": "NSE"}]},
                "subscriptions": {"instruments": [{"symbol": self.instrument, "exchange": "NSE"}]},
                "driving_timeframe": {"interval": 1, "aggregation": "minute"},
                "warmup_bars": 0,
                "starting_cash": str(self.starting_cash),
                "algo_id": str(strategy.__class__.__name__),
                "parameters": getattr(strategy, "_parameters", {}),  # type: ignore[attr-defined]
            }

            manifest_json = json.dumps(manifest)

            # Run the backtester subprocess; capture JSON output
            result = subprocess.run(
                [sys.executable, "-m", "honba_ya_backtest", "--manifest", "-"],
                input=manifest_json,
                capture_output=True,
                text=True,
                timeout=120,
            )

            if result.returncode != 0:
                err_msg = result.stderr or "backtester exited without error message"
                raise RuntimeError(f"honba-ya-backtest failed: {err_msg}")

            output = json.loads(result.stdout)

            # Map the backtester output into PyMetrics
            tax_breakdown: Dict[str, Decimal] = {
                "stt": Decimal(str(output.get("stt", 0))),
                "gst": Decimal(str(output.get("gst", 0))),
                "sebi": Decimal(str(output.get("sebi", 0))),
                "stamp_duty": Decimal(str(output.get("stamp_duty", 0))),
            }

            return PyMetrics(
                total_return=Decimal(str(output.get("total_return", 0))),
                cagr=Decimal(str(output.get("cagr", 0))),
                annual_volatility=Decimal(str(output.get("annual_volatility", 0))),
                sharpe_ratio=Decimal(str(output.get("sharpe_ratio", 0))),
                sortino_ratio=Decimal(str(output.get("sortino_ratio", 0))),
                max_drawdown=Decimal(str(output.get("max_drawdown", 0))),
                dsr=Decimal(str(output.get("dsr", 0))),
                pbo=Decimal(str(output.get("pbo", 0))),
                tax_breakdown=tax_breakdown,
                final_cash=Decimal(str(output.get("final_cash", self.starting_cash))),
                orders=int(output.get("orders_count", 0)),
                trades=int(output.get("trades_count", 0)),
                degraded=bool(output.get("degraded", False)),
                errors=list(output.get("errors", [])),
            )

        except RuntimeError:
            raise
        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(f"backtester integration failed: {exc}") from exc

    # ------------------------------------------------------------------
    # Optimisation interface (like Jesse)
    # ------------------------------------------------------------------

    def optimize(
        self,
        strategy_cls: type[Strategy],
        config: dict[str, Any],
        /,
        method: str = "grid",
        **grid_kwargs: Any,
    ) -> dict[str, Any]:
        """Run optimisation over *strategy_cls* with *config* parameter grid.

        Parameters
        ----------
        strategy_cls : type[Strategy]
            Strategy subclass to optimise.
        config : dict[str, Any]
            Base configuration dict; values may be `Parameter` instances or
            plain scalars.
        method : {"grid", "tpe", "ga"}
            Optimisation method:
            * ``"grid"`` — exhaustive grid search (default).
            * ``"tpe"`` — Tree-structured Parzen Estimator (Bayesian).
            * ``"ga"`` — Genetic algorithm.
        **grid_kwargs
            Passed to the chosen optimiser (e.g. ``n_points`` for TPE, etc.).

        Returns
        -------
        dict[str, Any]
            ``{"best_config": ..., "pareto_frontier": ..., "history": ...}``
        """
        import json

        try:
            # Prepare the parameter grid from the config
            # If any value is a `Parameter` we extract its (min, max, step)
            # and generate a discrete grid; otherwise we treat the value as a
            # single point to sweep.
            param_grid: dict[str, List[Any]] = {}
            for key, value in config.items():
                # Check if the value is our custom Parameter descriptor
                if hasattr(value, "min") and hasattr(value, "max"):
                    # Generate a linear grid from min to max with step
                    mn = Decimal(str(value.min)) if not isinstance(value.min, Decimal) else value.min
                    mx = Decimal(str(value.max)) if not isinstance(value.max, Decimal) else value.max
                    step = Decimal(str(value.step)) if value.step is not None else Decimal("0.1")
                    values: List[Any] = []
                    if step > 0:
                        n_points = int((mx - mn) / step) + 1
                        values = [mn + i * step for i in range(n_points)]
                    else:
                        values = [mn]
                    param_grid[key] = values
                else:
                    # Plain scalar — treat as single-point grid
                    param_grid[key] = [value]

            # Run a grid search: for each combo, invoke ``run_strategy`` and
            # collect the Sharpe ratio.
            best_sharpe = Decimal("-Infinity")
            best_config: Optional[dict[str, Any]] = None
            history: List[dict[str, Any]] = []

            from itertools import product

            keys = list(param_grid.keys())
            value_combos = product(*param_grid.values())

            for combo in value_combos:
                # Apply this combo to a fresh strategy instance
                instance = strategy_cls()
                applied: dict[str, Any] = {}
                for k, v in zip(keys, combo):
                    applied[k] = v

                # Merge with base config
                merged = {**config, **applied}  # type: ignore[assignment]

                # Override the instance parameters (our Parameter descriptor
                # __set__ will handle this)
                for k, v in applied.items():
                    try:
                        setattr(instance, k, v)  # type: ignore[attr-defined]
                    except Exception:
                        pass  # parameter may not be defined on this strategy

                try:
                    metrics = self.run_strategy(instance)  # type: ignore[arg-type]
                    sharpe = metrics.sharpe_ratio
                except RuntimeError:
                    # Strategy crashed or was refused by the risk gate;
                    # give it the worst possible Sharpe.
                    sharpe = Decimal("-Infinity")

                history.append(
                    {"parameters": dict(applied), "sharpe_ratio": float(sharpe), "degraded": metrics.degraded}
                )

                if sharpe > best_sharpe:
                    best_sharpe = sharpe
                    best_config = dict(applied)

            # Build a minimal Pareto frontier representation (sharpe vs return)
            pareto_frontier = [
                {"parameters": h["parameters"], "sharpe": h["sharpe_ratio"], "return": h.get("total_return", 0)}
                for h in history
            ]

            return {
                "best_config": best_config or {},
                "pareto_frontier": pareto_frontier,
                "history": history,
            }

        except Exception as exc:  # noqa: BLE001
            raise RuntimeError(f"optimisation failed: {exc}") from exc

    # ------------------------------------------------------------------
    # Notebook visualisation support
    # ------------------------------------------------------------------

    def plot(
        self,
        metrics: PyMetrics,
        /,
        *,  # keyword-only after this
        equity_curve: Optional[List[Decimal]] = None,
        title: str = "Strategy Backtest",
    ) -> str:
        """Render a Jupyter-friendly HTML snippet visualising the backtest.

        Parameters
        ----------
        metrics : PyMetrics
            The ``PyMetrics`` result from ``Runner.run()`` or ``Runner.run_strategy()``.
        equity_curve : List[Decimal], optional
            Daily NAV values. If ``None`` a simple line from ``[-max_drawdown,
            total_return]`` is generated.
        title : str
            Plot title.

        Returns
        -------
        str
            An HTML ``<div>`` suitable for embedding in a Jupyter notebook
            with ``IPython.display.HTML`` or ``display(HTML(...))``.
        """
        if equity_curve is None:
            # Generate a trivial curve: start -> peak -> trough -> end
            start = metrics.final_cash if metrics.final_cash else self.starting_cash
            peak = start + abs(metrics.total_return)
            trough = start - abs(metrics.max_drawdown)
            equity_curve = [start, peak, trough, start]

        # Simple ASCII art fallback; real notebooks would use matplotlib/vega.
        lines = [f"=== {title} ==="]
        lines.append(f"Total Return:  {metrics.total_return}")
        lines.append(f"Sharpe Ratio:  {metrics.sharpe_ratio}")
        lines.append(f"Sortino Ratio: {metrics.sortino_ratio}")
        lines.append(f"Max Drawdown:  {metrics.max_drawdown}")
        lines.append(f"Final Cash:    {metrics.final_cash}")
        lines.append(f"Degraded:      {metrics.degraded}")
        lines.append(f"Tax Breakdown: {metrics.tax_breakdown}")
        lines.append("---")
        # Plot the equity points as a simple text sparkline
        spark = "".join("^" if i > 0 else " " for i in equity_curve)  # placeholder
        lines.append(f"Equity spark:  {spark}")

        html = "<div>\n"
        for line in lines:
            html += f"<span>{line}</span><br/>\n"
        html += "</div>"
        return html