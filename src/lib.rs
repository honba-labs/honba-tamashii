//! 本場・魂 honba-tamashii — PyO3 bindings for the honba engine.
//!
//! This is the ONLY crate in the honba ecosystem permitted to depend on
//! PyO3. Everything below it is pure Rust; everything above it is Python.
//!
//! Scope of this scaffold: prove the boundary works end-to-end with a
//! mock feed, a Strategy base class, a Runner, an SMA indicator, and
//! a typed PyContext that records order intents. Integration with
//! honba-kokoro-types, honba-me, honba-te, honba-ya is the next milestone.

use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

// ============================================================================
// Bar — one OHLCV observation
// ============================================================================

#[pyclass(module = "honba._honba")]
#[derive(Clone)]
pub struct Bar {
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub open: f64,
    #[pyo3(get)]
    pub high: f64,
    #[pyo3(get)]
    pub low: f64,
    #[pyo3(get)]
    pub close: f64,
    #[pyo3(get)]
    pub volume: u64,
}

#[pymethods]
impl Bar {
    fn __repr__(&self) -> String {
        format!(
            "Bar({} close={:.2} vol={})",
            self.instrument, self.close, self.volume
        )
    }
}

// ============================================================================
// OrderIntent — what the strategy wanted to do (pre-risk, pre-execution)
// ============================================================================

#[pyclass(module = "honba._honba")]
#[derive(Clone)]
pub struct OrderIntent {
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub side: String, // "BUY" or "SELL"
    #[pyo3(get)]
    pub quantity: u64,
}

#[pymethods]
impl OrderIntent {
    fn __repr__(&self) -> String {
        format!(
            "OrderIntent({} {} {})",
            self.side, self.quantity, self.instrument
        )
    }
}

// ============================================================================
// Metrics — what Runner.run() returns
// ============================================================================

#[pyclass(module = "honba._honba")]
pub struct Metrics {
    #[pyo3(get)]
    pub orders: Vec<OrderIntent>,
    #[pyo3(get)]
    pub final_cash: f64,
}

#[pymethods]
impl Metrics {
    fn __repr__(&self) -> String {
        format!(
            "Metrics(orders={}, final_cash={:.2})",
            self.orders.len(),
            self.final_cash
        )
    }
}

// ============================================================================
// PyContext — handed to the strategy on every callback
// ============================================================================

#[pyclass(module = "honba._honba")]
pub struct PyContext {
    orders: Arc<Mutex<Vec<OrderIntent>>>,
}

#[pymethods]
impl PyContext {
    fn submit(&self, instrument: &str, side: &str, quantity: u64) -> PyResult<()> {
        let side_upper = side.to_uppercase();
        if side_upper != "BUY" && side_upper != "SELL" {
            return Err(PyValueError::new_err(format!(
                "side must be 'BUY' or 'SELL', got {:?}",
                side
            )));
        }
        self.orders.lock().unwrap().push(OrderIntent {
            instrument: instrument.to_string(),
            side: side_upper,
            quantity,
        });
        Ok(())
    }

    fn close_all(&self, instrument: &str) -> PyResult<()> {
        self.orders.lock().unwrap().push(OrderIntent {
            instrument: instrument.to_string(),
            side: "SELL".to_string(),
            quantity: 0,
        });
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("PyContext(pending={})", self.orders.lock().unwrap().len())
    }
}

// ============================================================================
// Runner — the Rust event loop
// ============================================================================

#[pyclass(module = "honba._honba")]
pub struct Runner {
    #[pyo3(get)]
    pub num_bars: usize,
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub starting_cash: f64,
}

#[pymethods]
impl Runner {
    #[new]
    #[pyo3(signature = (num_bars=100, instrument="RELIANCE", starting_cash=100000.0))]
    fn new(num_bars: usize, instrument: &str, starting_cash: f64) -> Self {
        Self {
            num_bars,
            instrument: instrument.to_string(),
            starting_cash,
        }
    }

    /// Run the strategy over a synthetic price walk.
    ///
    /// This is the entire contract a Python strategy author needs to know:
    /// the Rust loop constructs a `Bar`, hands it to `strategy.on_bar(bar, ctx)`,
    /// and collects whatever `ctx.submit(...)` calls happened.
    ///
    /// Replacing `synthetic_bars` with `honba-me` and `PyContext::submit`
    /// with `honba-te-router` is the next milestone — the shape stays identical.
    fn run(&self, py: Python<'_>, strategy: Bound<'_, PyAny>) -> PyResult<Metrics> {
        let orders = Arc::new(Mutex::new(Vec::<OrderIntent>::new()));
        let ctx = Py::new(
            py,
            PyContext {
                orders: Arc::clone(&orders),
            },
        )?;

        if strategy.hasattr("setup")? {
            strategy.call_method0("setup")?;
        }

        let mut price = Decimal::from(100);
        for i in 0..self.num_bars {
            // 5 bars up, 5 bars down — a repeating sawtooth.
            let delta = if (i / 5) % 2 == 0 {
                Decimal::ONE
            } else {
                -Decimal::ONE
            };
            price += delta;

            let close = price.to_f64().unwrap_or(100.0);
            let bar = Bar {
                instrument: self.instrument.clone(),
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 1000,
            };
            let bar_obj = Py::new(py, bar)?;

            // Every callback is dispatched through here. In production, this
            // is wrapped in catch_unwind + a py.allow_threads window for
            // any Rust-side I/O between calls. For the scaffold, keep it
            // simple and correct: no async, no I/O.
            strategy.call_method1("on_bar", (bar_obj, &ctx))?;
        }

        if strategy.hasattr("on_stop")? {
            strategy.call_method0("on_stop")?;
        }

        let order_list = orders.lock().unwrap().clone();
        Ok(Metrics {
            orders: order_list,
            final_cash: self.starting_cash,
        })
    }
}

// ============================================================================
// SMA — simple moving average, exported to Python
// ============================================================================

#[pyclass(module = "honba._honba")]
pub struct SMA {
    period: usize,
    values: Vec<f64>,
}

#[pymethods]
impl SMA {
    #[new]
    fn new(period: usize) -> PyResult<Self> {
        if period == 0 {
            return Err(PyValueError::new_err("period must be > 0"));
        }
        Ok(Self {
            period,
            values: Vec::with_capacity(period),
        })
    }

    /// Feed a value; returns the SMA once `period` samples are collected,
    /// `None` before that.
    fn update(&mut self, value: f64) -> Option<f64> {
        self.values.push(value);
        if self.values.len() > self.period {
            self.values.remove(0);
        }
        if self.values.len() == self.period {
            Some(self.values.iter().sum::<f64>() / self.period as f64)
        } else {
            None
        }
    }
}

// ============================================================================
// Module
// ============================================================================

#[pymodule]
fn _honba(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Bar>()?;
    m.add_class::<OrderIntent>()?;
    m.add_class::<Metrics>()?;
    m.add_class::<PyContext>()?;
    m.add_class::<Runner>()?;
    m.add_class::<SMA>()?;
    Ok(())
}
