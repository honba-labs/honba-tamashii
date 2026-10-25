//! Native Dynamic Dispatch and Versioned Strategy Runner Bindings.
//!
//! Handles both Strategy API v1 and v2 objects via internal dynamic dispatch:
//! - v1 strategies implement `on_bar(bar: Bar, ctx: PyContext)`
//! - v2 strategies implement `on_step(event: MarketEvent, ctx: PyContextV2)`

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::pyclasses::core::{PyBar, PyTick};
use honba_core::MarketEvent;

/// Dynamic strategy binding supporting both Strategy API v1 and v2.
#[derive(Debug)]
pub enum AnyStrategyBinding {
    /// Strategy API v1: implements `on_bar(bar: Bar, ctx: PyContext)`
    V1(Py<PyAny>),
    /// Strategy API v2: implements `on_step(event: MarketEvent, ctx: PyContextV2)`
    V2(Py<PyAny>),
}

impl AnyStrategyBinding {
    /// Detect the strategy API version from `__strategy_api_version__`.
    ///
    /// Defaults to "1.0.0" for backwards compatibility with unversioned legacy strategies.
    pub fn detect(strat: &Bound<'_, PyAny>) -> PyResult<Self> {
        let version: String = strat
            .getattr("__strategy_api_version__")
            .and_then(|v| v.extract())
            .unwrap_or_else(|_| "1.0.0".to_string());

        if version.starts_with("1.") {
            // Existing software using v1 contract runs cleanly:
            Ok(Self::V1(strat.clone().unbind()))
        } else if version.starts_with("2.") {
            // Latest & recommended v2 contract:
            Ok(Self::V2(strat.clone().unbind()))
        } else {
            Err(PyValueError::new_err(format!(
                "Unsupported strategy version: {}",
                version
            )))
        }
    }

    /// Return the canonical version string ("1.0.0" or "2.0.0").
    pub fn version(&self) -> &'static str {
        match self {
            Self::V1(_) => "1.0.0",
            Self::V2(_) => "2.0.0",
        }
    }

    /// Access the underlying Python strategy object.
    pub fn strategy(&self) -> &Py<PyAny> {
        match self {
            Self::V1(s) => s,
            Self::V2(s) => s,
        }
    }

    /// Whether this is a v1 strategy.
    pub fn is_v1(&self) -> bool {
        matches!(self, Self::V1(_))
    }

    /// Whether this is a v2 strategy.
    pub fn is_v2(&self) -> bool {
        matches!(self, Self::V2(_))
    }
}

/// Detect the strategy API version from Python.
#[pyfunction]
pub fn detect_strategy_version(strat: &Bound<'_, PyAny>) -> PyResult<String> {
    let binding = AnyStrategyBinding::detect(strat)?;
    Ok(binding.version().to_string())
}

/// Unified market event for Strategy API v2.
#[pyclass(name = "MarketEvent", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyMarketEvent {
    pub inner: MarketEvent,
}

#[pymethods]
impl PyMarketEvent {
    #[staticmethod]
    pub fn from_bar(bar: &PyBar) -> Self {
        Self {
            inner: MarketEvent::Bar(bar.inner.clone()),
        }
    }

    #[staticmethod]
    pub fn from_tick(tick: &PyTick) -> Self {
        Self {
            inner: MarketEvent::Tick(tick.inner.clone()),
        }
    }

    #[getter]
    pub fn kind(&self) -> &'static str {
        match &self.inner {
            MarketEvent::Bar(_) => "bar",
            MarketEvent::Tick(_) => "tick",
            MarketEvent::ConnectorStateChanged(_) => "connector_state_changed",
        }
    }

    #[getter]
    pub fn bar(&self) -> Option<PyBar> {
        match &self.inner {
            MarketEvent::Bar(b) => Some(PyBar { inner: b.clone() }),
            _ => None,
        }
    }

    #[getter]
    pub fn tick(&self) -> Option<PyTick> {
        match &self.inner {
            MarketEvent::Tick(t) => Some(PyTick { inner: t.clone() }),
            _ => None,
        }
    }

    #[getter]
    pub fn close<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match &self.inner {
            MarketEvent::Bar(b) => Ok(Some(crate::decimal::to_py_decimal(py, b.close.0)?)),
            MarketEvent::Tick(t) => {
                if let Some(price) = t.last_price() {
                    Ok(Some(crate::decimal::to_py_decimal(py, price.0)?))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    fn __repr__(&self) -> String {
        format!("MarketEvent(kind='{}')", self.kind())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMarketEvent>()?;
    m.add_function(wrap_pyfunction!(detect_strategy_version, m)?)?;
    Ok(())
}
