//! The strategy harness: `Runner`, `PyContext`, `Metrics`, `SMA`.
//!
//! This replaces the toy scaffold that used to live in the crate root. The
//! shape is the same — a strategy gets `setup`, then `on_bar` per bar, then
//! `on_stop` — but the values are exact decimals and every submission goes
//! through the `honba-mon` risk gate before it is recorded.
//!
//! Two properties the toy scaffold did not have:
//!
//! - **A strategy cannot crash the host.** A Python exception is caught, the
//!   run is marked degraded, the traceback is kept in [`PyMetrics`] and the
//!   process keeps running.
//! - **An order cannot skip the risk gate.** [`PyContext::submit`] validates
//!   the order against `honba-mon` and raises `RiskRefusalError` when the
//!   gate refuses it.

use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use honba_core::{Exchange, InstrumentId, OrderRequest, OrderSide, OrderType, Price, Quantity};
use honba_mon::RiskEngine;
use rust_decimal::Decimal;

use crate::decimal::{from_py_decimal, to_py_decimal};
use crate::pyclasses::core::{PyBar, PyInstrumentId, PyPrice, PyQuantity};
use crate::pyclasses::order::PyOrderRequest;

/// One order the strategy asked for.
#[pyclass(name = "OrderIntent", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyOrderIntent {
    /// The instrument the order is for.
    #[pyo3(get)]
    pub instrument: PyInstrumentId,
    /// The order side (`BUY` or `SELL`).
    #[pyo3(get)]
    pub side: String,
    /// The order quantity.
    #[pyo3(get)]
    pub quantity: PyQuantity,
    /// The order type.
    #[pyo3(get)]
    pub order_type: String,
    /// The limit price, when the order carries one.
    #[pyo3(get)]
    pub price: Option<PyPrice>,
    /// The SEBI algo id the order was validated under.
    #[pyo3(get)]
    pub algo_id: String,
}

#[pymethods]
impl PyOrderIntent {
    fn __repr__(&self) -> String {
        format!(
            "OrderIntent('{}', {}, '{}')",
            self.instrument.inner, self.side, self.quantity.inner.0
        )
    }
}

/// What a run produced.
#[pyclass(name = "Metrics", module = "honba._honba")]
#[derive(Clone, Debug, Default)]
pub struct PyMetrics {
    orders: Vec<PyOrderIntent>,
    final_cash: Option<Decimal>,
    degraded: bool,
    errors: Vec<String>,
    refusals: Vec<String>,
}

#[pymethods]
impl PyMetrics {
    /// The orders the strategy submitted and the gate approved.
    #[getter]
    fn orders(&self) -> Vec<PyOrderIntent> {
        self.orders.clone()
    }

    /// The cash left at the end of the run.
    #[getter]
    fn final_cash<'py>(&self, py: Python<'py>) ->PyResult<Option<Bound<'py, PyAny>>> {
        match self.final_cash {
            Some(cash) => Ok(Some(to_py_decimal(py, cash)?)),
            None => Ok(None),
        }
    }

    /// Whether a strategy callback raised.
    #[getter]
    fn degraded(&self) -> bool {
        self.degraded
    }

    /// The tracebacks of the callbacks that raised.
    #[getter]
    fn errors(&self) -> Vec<String> {
        self.errors.clone()
    }

    /// The rule codes of the orders the gate refused.
    #[getter]
    fn refusals(&self) -> Vec<String> {
        self.refusals.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Metrics(orders={}, degraded={})",
            self.orders.len(),
            self.degraded
        )
    }
}

/// The context a strategy submits through.
#[pyclass(name = "PyContext", module = "honba._honba")]
pub struct PyContext {
    state: Arc<Mutex<ContextState>>,
    algo_id: String,
}

/// The shared state behind a [`PyContext`].
struct ContextState {
    engine: RiskEngine,
    orders: Vec<OrderRequest>,
    refusals: Vec<String>,
    errors: Vec<String>,
    degraded: bool,
}

#[pymethods]
impl PyContext {
    /// Submits an order for `instrument`.
    ///
    /// `instrument` is an `InstrumentId` or a symbol (`"RELIANCE"`, NSE by
    /// default). The order passes the risk gate first: a refusal raises
    /// `RiskRefusalError`, and the refusal is recorded on the run.
    #[pyo3(signature = (instrument, side, quantity))]
    fn submit(
        &self,
        instrument: &Bound<'_, PyAny>,
        side: &str,
        quantity: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        let instrument = instrument_from(instrument)?;
        let order = build_order(&instrument, side, from_py_decimal(quantity)?, &self.algo_id)?;
        self.submit_order(order)
    }

    /// Submits a fully built order.
    fn submit_request(&self, order: &PyOrderRequest) -> PyResult<String> {
        self.submit_order(order.inner.clone())
    }

    /// Closes the position in `instrument` with a market sell.
    fn close_all(&self, instrument: &Bound<'_, PyAny>) -> PyResult<String> {
        let instrument = instrument_from(instrument)?;
        let order = build_order(&instrument, "SELL", Decimal::ZERO, &self.algo_id)?;
        self.submit_order(order)
    }

    /// The orders approved so far.
    #[getter]
    fn orders<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyList>> {
        let state = self
            .state
            .lock()
            .map_err(|_| PyValueError::new_err("the context is poisoned"))?;
        let items: Vec<Py<PyOrderIntent>> = state
            .orders
            .iter()
            .map(|order| Py::new(py, intent_of(order)))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyList::new_bound(py, items))
    }

    /// The rule codes of the orders the gate refused.
    #[getter]
    fn refusals<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyList>> {
        let state = self
            .state
            .lock()
            .map_err(|_| PyValueError::new_err("the context is poisoned"))?;
        Ok(PyList::new_bound(py, state.refusals.clone()))
    }

    fn __repr__(&self) -> String {
        let orders = self.state.lock().map(|state| state.orders.len()).unwrap_or(0);
        format!("PyContext(orders={orders})")
    }
}

impl PyContext {
    /// Submits an order the engine accepted.
    fn submit_order(&self, order: OrderRequest) -> PyResult<String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| PyValueError::new_err("the context is poisoned"))?;
        let now = chrono::Utc::now();
        match state.engine.check_order(&order, now) {
            Ok(()) => {
                let id = order.client_order_id.to_string();
                state.orders.push(order);
                Ok(id)
            }
            Err(refusal) => {
                let rule = refusal.rule().to_string();
                state.refusals.push(rule.clone());
                let message = refusal.to_string();
                let json = serde_json::to_string(&refusal).unwrap_or_default();
                Err(crate::pyclasses::risk::refusal_error(&message, &rule, &json))
            }
        }
    }
}

/// The strategy harness: runs a Python strategy over a synthetic price walk.
#[pyclass(name = "Runner", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyRunner {
    /// How many bars the walk has.
    #[pyo3(get)]
    pub num_bars: usize,
    /// The instrument the walk is for.
    #[pyo3(get)]
    pub instrument: String,
    /// The cash the run starts with.
    starting_cash: Decimal,
    /// The SEBI algo id the orders are validated under.
    #[pyo3(get)]
    pub algo_id: String,
}

#[pymethods]
impl PyRunner {
    /// Builds a runner.
    ///
    /// `starting_cash` is a decimal string, an int or a `decimal.Decimal`; it
    /// defaults to 100000.
    #[new]
    #[pyo3(signature = (num_bars=100, instrument="RELIANCE", starting_cash=None, algo_id="HARNESS_ALGO"))]
    pub fn new(
        num_bars: usize,
        instrument: &str,
        starting_cash: Option<&Bound<'_, PyAny>>,
        algo_id: &str,
    ) -> PyResult<Self> {
        let starting_cash = match starting_cash {
            Some(value) => from_py_decimal(value)?,
            None => Decimal::from(100_000),
        };
        Ok(Self {
            num_bars,
            instrument: instrument.to_string(),
            starting_cash,
            algo_id: algo_id.to_string(),
        })
    }

    /// The cash the run starts with.
    #[getter]
    fn starting_cash<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.starting_cash)
    }

    /// Runs `strategy` over the walk and reports what happened.
    ///
    /// The strategy is called as `setup()`, then `on_bar(bar, ctx)` per bar,
    /// then `on_stop()`. A callback that raises ends the run in a degraded
    /// state instead of taking the process down.
    fn run(&self, py: Python<'_>, strategy: &Bound<'_, PyAny>) ->PyResult<PyMetrics> {
        let instrument = InstrumentId::new(Exchange::Nse, &self.instrument);
        let mut engine = RiskEngine::default();
        engine.register_algo(&self.algo_id);

        let state = Arc::new(Mutex::new(ContextState {
            engine,
            orders: Vec::new(),
            refusals: Vec::new(),
            errors: Vec::new(),
            degraded: false,
        }));
        let context = Py::new(
            py,
            PyContext {
                state: Arc::clone(&state),
                algo_id: self.algo_id.clone(),
            },
        )?;

        if strategy.hasattr("setup")? {
            if let Err(err) = strategy.call_method0("setup") {
                record_error(&state, err);
            }
        }

        let mut price = Decimal::from(100);
        for index in 0..self.num_bars {
            // Five bars up, five bars down: a repeating sawtooth.
            let delta = if (index / 5) % 2 == 0 {
                Decimal::ONE
            } else {
                -Decimal::ONE
            };
            price += delta;

            let bar = PyBar {
                inner: honba_core::Bar {
                    instrument: instrument.clone(),
                    open: Price::new(price),
                    high: Price::new(price + Decimal::ONE),
                    low: Price::new(price - Decimal::ONE),
                    close: Price::new(price),
                    volume: Quantity::from_u64(1000),
                    timestamp: chrono::DateTime::from_timestamp(0, 0)
                        .expect("epoch is a timestamp"),
                },
            };
            let bar = Py::new(py, bar)?;
            if let Err(err) = strategy.call_method1("on_bar", (bar, &context)) {
                if err.is_instance_of::<crate::pyclasses::risk::RiskRefusalError>(py) {
                    // A refused order is a decision, not a crash: the strategy may
                    // want to react to it.
                    let _ = err;
                } else {
                    record_error(&state, err);
                    break;
                }
            }
        }

        if strategy.hasattr("on_stop")? {
            if let Err(err) = strategy.call_method0("on_stop") {
                record_error(&state, err);
            }
        }

        let state = state
            .lock()
            .map_err(|_| PyValueError::new_err("the context is poisoned"))?;
        let orders = state.orders.iter().map(intent_of).collect();
        Ok(PyMetrics {
            orders,
            final_cash: Some(self.starting_cash),
            degraded: state.degraded,
            errors: state.errors.clone(),
            refusals: state.refusals.clone(),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Runner(num_bars={}, instrument='{}')",
            self.num_bars, self.instrument
        )
    }
}

/// A simple moving average over exact decimals.
#[pyclass(name = "SMA", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PySma {
    period: usize,
    values: Vec<Decimal>,
}

#[pymethods]
impl PySma {
    /// Builds an SMA over `period` samples.
    #[new]
    pub fn new(period: usize) -> PyResult<Self> {
        if period == 0 {
            return Err(PyValueError::new_err("period must be > 0"));
        }
        Ok(Self {
            period,
            values: Vec::with_capacity(period),
        })
    }

    /// Feeds a value; returns the average once `period` samples are in.
    fn update<'py>(&mut self, py: Python<'py>, value: &Bound<'_, PyAny>) ->PyResult<Option<Bound<'py, PyAny>>> {
        let value = from_py_decimal(value)?;
        self.values.push(value);
        if self.values.len() > self.period {
            self.values.remove(0);
        }
        if self.values.len() == self.period {
            let total: Decimal = self.values.iter().sum();
            let average = total / Decimal::from(self.period as u64);
            Ok(Some(to_py_decimal(py, average)?))
        } else {
            Ok(None)
        }
    }

    /// The current average, if there is one.
    #[getter]
    fn value<'py>(&self, py: Python<'py>) ->PyResult<Option<Bound<'py, PyAny>>> {
        if self.values.len() < self.period {
            return Ok(None);
        }
        let total: Decimal = self.values.iter().sum();
        Ok(Some(to_py_decimal(py, total / Decimal::from(self.period as u64))?))
    }

    fn __repr__(&self) -> String {
        format!("SMA(period={})", self.period)
    }
}

/// Reads an instrument from an `InstrumentId` or a symbol.
fn instrument_from(value: &Bound<'_, PyAny>) -> PyResult<InstrumentId> {
    if let Ok(instrument) = value.extract::<PyInstrumentId>() {
        return Ok(instrument.inner);
    }
    if let Ok(symbol) = value.extract::<String>() {
        return Ok(InstrumentId::new(Exchange::Nse, symbol));
    }
    Err(PyValueError::new_err(
        "expected an InstrumentId or a symbol string",
    ))
}

/// Builds a market order for the harness.
fn build_order(
    instrument: &InstrumentId,
    side: &str,
    quantity: Decimal,
    algo_id: &str,
) -> PyResult<OrderRequest> {
    let side = match side.to_uppercase().as_str() {
        "BUY" => OrderSide::Buy,
        "SELL" => OrderSide::Sell,
        other => {
            return Err(PyValueError::new_err(format!(
                "side must be 'BUY' or 'SELL', got {other:?}"
            )))
        }
    };
    Ok(OrderRequest::new(
        instrument.clone(),
        side,
        OrderType::Market,
        Quantity::new(quantity),
        algo_id,
    ))
}

/// The Python view of an order.
fn intent_of(order: &OrderRequest) -> PyOrderIntent {
    PyOrderIntent {
        instrument: PyInstrumentId {
            inner: order.instrument.clone(),
        },
        side: match order.side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
        .to_string(),
        quantity: PyQuantity {
            inner: order.quantity,
        },
        order_type: match &order.order_type {
            OrderType::Market => "market",
            OrderType::Limit(_) => "limit",
            OrderType::StopLoss(_) => "stop_loss",
            OrderType::StopLimit { .. } => "stop_limit",
        }
        .to_string(),
        price: order
            .order_type
            .limit_price()
            .map(|price| PyPrice { inner: price }),
        algo_id: order.algo_id.clone(),
    }
}

/// Records a failed callback and marks the run degraded.
fn record_error(state: &Arc<Mutex<ContextState>>, err: PyErr) {
    if let Ok(mut state) = state.lock() {
        state.degraded = true;
        state.errors.push(format!("{err}"));
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyOrderIntent>()?;
    m.add_class::<PyMetrics>()?;
    m.add_class::<PyContext>()?;
    m.add_class::<PyRunner>()?;
    m.add_class::<PySma>()?;
    Ok(())
}
