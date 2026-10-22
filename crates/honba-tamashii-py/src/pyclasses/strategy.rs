//! Rust-backed strategy helpers.
//!
//! The rolling window and the position state live in Rust, so a Python
//! strategy that wants a fast signal does not pay for Python objects per bar.
//!
//! Prices and quantities cross as exact decimals: the signal is a
//! `(side, quantity)` pair whose quantity is a `decimal.Decimal`.

use pyo3::prelude::*;
use pyo3::types::PyModule;
use rust_decimal::Decimal;

use crate::decimal::{from_py_decimal, to_py_decimal};
use crate::pyclasses::core::PyBar;

/// Rust-backed SMA crossover.
///
/// Same semantics as the Python `SmaCrossover`, but the rolling window and
/// the position state live in Rust.
#[pyclass(name = "RustSmaCrossover", module = "honba._honba")]
pub struct RustSmaCrossover {
    fast: usize,
    slow: usize,
    qty: Decimal,
    position: Decimal,
    prices: Vec<Decimal>,
    prev_diff: Option<Decimal>,
    intents: Vec<(String, Decimal)>,
}

#[pymethods]
impl RustSmaCrossover {
    /// Builds a crossover over `fast` and `slow` simple moving averages.
    #[new]
    #[pyo3(signature = (fast=3, slow=8, qty=None))]
    pub fn new(
        fast: usize,
        slow: usize,
        qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let qty = match qty {
            Some(value) => from_py_decimal(value)?,
            None => Decimal::ONE,
        };
        Ok(Self {
            fast,
            slow,
            qty,
            position: Decimal::ZERO,
            prices: Vec::new(),
            prev_diff: None,
            intents: Vec::new(),
        })
    }

    /// Feed a bar. Returns `(side, qty)` if a signal fired, else `None`.
    fn on_bar<'py>(
        &mut self,
        py: Python<'py>,
        bar: &PyBar,
    ) ->PyResult<Option<(String, Bound<'py, PyAny>)>> {
        self.push(py, bar.inner.close.0)
    }

    /// Feed a plain close price instead of a `Bar`.
    fn on_close<'py>(
        &mut self,
        py: Python<'py>,
        close: &Bound<'_, PyAny>,
    ) ->PyResult<Option<(String, Bound<'py, PyAny>)>> {
        self.push(py, from_py_decimal(close)?)
    }

    /// The current position.
    #[getter]
    fn position<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.position)
    }

    /// How many signals fired.
    #[getter]
    fn intent_count(&self) -> usize {
        self.intents.len()
    }

    /// The signals that fired.
    fn intents<'py>(&self, py: Python<'py>) ->PyResult<Vec<(String, Bound<'py, PyAny>)>> {
        self.intents
            .iter()
            .map(|(side, qty)| Ok((side.clone(), to_py_decimal(py, *qty)?)))
            .collect()
    }
}

impl RustSmaCrossover {
    /// Feeds one close price and returns the signal, if any.
    fn push<'py>(
        &mut self,
        py: Python<'py>,
        close: Decimal,
    ) ->PyResult<Option<(String, Bound<'py, PyAny>)>> {
        self.prices.push(close);
        if self.prices.len() < self.slow {
            return Ok(None);
        }
        let (Some(fast), Some(slow)) = (
            sma(&self.prices, self.fast),
            sma(&self.prices, self.slow),
        ) else {
            return Ok(None);
        };
        let diff = fast - slow;

        let mut fired: Option<(String, Decimal)> = None;
        if let Some(prev) = self.prev_diff {
            if prev <= Decimal::ZERO && diff > Decimal::ZERO && self.position.is_zero() {
                self.position = self.qty;
                fired = Some(("buy".to_string(), self.qty));
            } else if prev >= Decimal::ZERO && diff < Decimal::ZERO && self.position > Decimal::ZERO {
                let quantity = self.position;
                self.position = Decimal::ZERO;
                fired = Some(("sell".to_string(), quantity));
            }
        }
        self.prev_diff = Some(diff);

        match fired {
            Some((side, quantity)) => {
                self.intents.push((side.clone(), quantity));
                Ok(Some((side, to_py_decimal(py, quantity)?)))
            }
            None => Ok(None),
        }
    }
}

/// The simple moving average of the last `n` values.
fn sma(values: &[Decimal], n: usize) -> Option<Decimal> {
    if n == 0 || values.len() < n {
        return None;
    }
    let total: Decimal = values[values.len() - n..].iter().sum();
    Some(total / Decimal::from(n as u64))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustSmaCrossover>()?;
    Ok(())
}
