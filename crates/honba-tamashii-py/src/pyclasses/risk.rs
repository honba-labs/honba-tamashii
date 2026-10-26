//! The pre-trade risk gate: `honba-mon` exposed to Python (plan.md, Phase 1).
//!
//! Every order a Python strategy submits passes through
//! [`honba_mon::RiskEngine`] before it can reach a broker: the SEBI algo-id
//! registry, the order-to-trade ratio breaker, the scrip price band, the
//! exchange freeze limits and the kill switches all live in Rust, so Python
//! and Rust cannot disagree about what is allowed.
//!
//! A refusal raises [`RiskRefusalError`], which carries the stable rule code
//! (`sebi_otr_breach`, `price_band_violation`, ...) and the refusal itself.

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use honba_core::{Decision, Quantity};
use honba_mon::{OtrConfig, RiskEngine, RiskLimits, RiskRefusal};
use rust_decimal::Decimal;

use crate::decimal::{
    from_py_decimal, from_py_optional_decimal, from_py_optional_timestamp, to_py_decimal,
};
use crate::pyclasses::core::{PyInstrumentId, PyPriceBand};
use crate::pyclasses::order::{PyFillReport, PyOrderRequest};

/// A refusal by the risk gate.
#[pyclass(name = "RiskRefusal", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyRiskRefusal {
    pub(crate) inner: RiskRefusal,
}

#[pymethods]
impl PyRiskRefusal {
    /// The stable rule code (`sebi_otr_breach`, `price_band_violation`, ...).
    #[getter]
    fn rule(&self) -> &'static str {
        self.inner.rule()
    }

    /// The refusal variant (`OtrBreached`, `PriceBandViolation`, ...).
    #[getter]
    fn reason(&self) -> &'static str {
        match &self.inner {
            RiskRefusal::GlobalHalted { .. } => "GlobalHalted",
            RiskRefusal::AlgoHalted { .. } => "AlgoHalted",
            RiskRefusal::AlgoNotRegistered { .. } => "AlgoNotRegistered",
            RiskRefusal::MarketCircuitHalt { .. } => "MarketCircuitHalt",
            RiskRefusal::ScripOtrBreached { .. } => "ScripOtrBreached",
            RiskRefusal::OtrBreached { .. } => "OtrBreached",
            RiskRefusal::PriceBandViolation { .. } => "PriceBandViolation",
            RiskRefusal::MaxNotionalExceeded { .. } => "MaxNotionalExceeded",
            RiskRefusal::FreezeLimitExceeded { .. } => "FreezeLimitExceeded",
            RiskRefusal::MaxQuantityExceeded { .. } => "MaxQuantityExceeded",
            RiskRefusal::OrderRateExceeded { .. } => "OrderRateExceeded",
            RiskRefusal::DailyLossExceeded { .. } => "DailyLossExceeded",
            RiskRefusal::ReduceOnlyViolation { .. } => "ReduceOnlyViolation",
            RiskRefusal::PositionConcentrationExceeded { .. } => "PositionConcentrationExceeded",
            RiskRefusal::MisLeverageExceeded { .. } => "MisLeverageExceeded",
            RiskRefusal::InvalidOrder { .. } => "InvalidOrder",
        }
    }

    /// The refusal's details, keyed by field name.
    fn details<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let details = PyDict::new_bound(py);
        match &self.inner {
            RiskRefusal::GlobalHalted { reason } => {
                details.set_item("reason", reason)?;
            }
            RiskRefusal::AlgoHalted { algo_id, reason } => {
                details.set_item("algo_id", algo_id)?;
                details.set_item("reason", reason)?;
            }
            RiskRefusal::AlgoNotRegistered { algo_id, reason } => {
                details.set_item("algo_id", algo_id)?;
                details.set_item("reason", reason)?;
            }
            RiskRefusal::MarketCircuitHalt {
                index_symbol,
                stage_pct,
                resume_at,
            } => {
                details.set_item("index_symbol", index_symbol)?;
                details.set_item("stage_pct", stage_pct)?;
                details.set_item(
                    "resume_at",
                    resume_at.map(|ts| ts.to_rfc3339()).unwrap_or_default(),
                )?;
            }
            RiskRefusal::ScripOtrBreached {
                instrument,
                current_otr,
                max_otr,
            } => {
                details.set_item("instrument", instrument.to_string())?;
                details.set_item("current_otr", current_otr.to_string())?;
                details.set_item("max_otr", max_otr.to_string())?;
            }
            RiskRefusal::OtrBreached {
                algo_id,
                current_otr,
                max_otr,
            } => {
                details.set_item("algo_id", algo_id)?;
                details.set_item("current_otr", current_otr.to_string())?;
                details.set_item("max_otr", max_otr.to_string())?;
            }
            RiskRefusal::PriceBandViolation { price, band } => {
                details.set_item("price", price.0.to_string())?;
                details.set_item("lower", band.lower.0.to_string())?;
                details.set_item("upper", band.upper.0.to_string())?;
                details.set_item("reference_price", band.reference_price.0.to_string())?;
            }
            RiskRefusal::MaxNotionalExceeded { requested, limit } => {
                details.set_item("requested", requested.to_string())?;
                details.set_item("limit", limit.to_string())?;
            }
            RiskRefusal::FreezeLimitExceeded {
                instrument,
                requested,
                limit,
            } => {
                details.set_item("instrument", instrument.to_string())?;
                details.set_item("requested", requested.0.to_string())?;
                details.set_item("limit", limit.0.to_string())?;
            }
            RiskRefusal::MaxQuantityExceeded { requested, limit } => {
                details.set_item("requested", requested.0.to_string())?;
                details.set_item("limit", limit.0.to_string())?;
            }
            RiskRefusal::OrderRateExceeded {
                count,
                limit,
                window_ms,
            } => {
                details.set_item("count", count)?;
                details.set_item("limit", limit)?;
                details.set_item("window_ms", window_ms)?;
            }
            RiskRefusal::DailyLossExceeded {
                current_loss,
                limit,
            } => {
                details.set_item("current_loss", current_loss.to_string())?;
                details.set_item("limit", limit.to_string())?;
            }
            RiskRefusal::ReduceOnlyViolation { details: text } => {
                details.set_item("details", text)?;
            }
            RiskRefusal::PositionConcentrationExceeded {
                instrument,
                position_value,
                total_equity,
                max_pct,
            } => {
                details.set_item("instrument", instrument.to_string())?;
                details.set_item("position_value", position_value.to_string())?;
                details.set_item("total_equity", total_equity.to_string())?;
                details.set_item("max_pct", max_pct.to_string())?;
            }
            RiskRefusal::MisLeverageExceeded {
                gross_exposure,
                total_equity,
                max_leverage,
            } => {
                details.set_item("gross_exposure", gross_exposure.to_string())?;
                details.set_item("total_equity", total_equity.to_string())?;
                details.set_item("max_leverage", max_leverage.to_string())?;
            }
            RiskRefusal::InvalidOrder { error } => {
                details.set_item("error", error)?;
            }
        }
        Ok(details)
    }

    /// The refusal as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("RiskRefusal('{}')", self.inner.rule())
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

// Raised when the risk gate refuses an order.
//
// The exception carries the stable rule code and the refusal as JSON, so a
// Python caller can branch on the rule without parsing a message.
pyo3::create_exception!(honba._honba, RiskRefusalError, PyException);

/// The limits a run trades under.
#[pyclass(name = "RiskLimits", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyRiskLimits {
    pub(crate) inner: RiskLimits,
}

#[pymethods]
impl PyRiskLimits {
    /// Builds limits from the keyword arguments.
    #[new]
    #[pyo3(signature = (max_notional_per_order=None, max_quantity_per_order=None, max_daily_loss=None, order_rate_limit=None, price_collar_bps=None))]
    pub fn new(
        max_notional_per_order: Option<&Bound<'_, PyAny>>,
        max_quantity_per_order: Option<&Bound<'_, PyAny>>,
        max_daily_loss: Option<&Bound<'_, PyAny>>,
        order_rate_limit: Option<(u32, u64)>,
        price_collar_bps: Option<u32>,
    ) -> PyResult<Self> {
        let mut inner = RiskLimits::default();
        inner.max_notional_per_order = from_py_optional_decimal(max_notional_per_order)?;
        inner.max_quantity_per_order =
            from_py_optional_decimal(max_quantity_per_order)?.map(Quantity::new);
        inner.max_daily_loss = from_py_optional_decimal(max_daily_loss)?;
        inner.price_collar_bps = price_collar_bps;
        if let Some((max_orders, window_ms)) = order_rate_limit {
            inner.order_rate_limit = Some(honba_mon::OrderRateLimit {
                max_orders,
                window_ms,
            });
        }
        Ok(Self { inner })
    }

    /// Parses the `[risk]` table of a run's configuration.
    ///
    /// Unknown keys are refused, so a typo in a config file is an error rather
    /// than a silently ignored limit.
    #[staticmethod]
    pub fn from_dict(config: &Bound<'_, PyDict>) -> PyResult<Self> {
        let mut inner = RiskLimits::default();
        for (key, value) in config.iter() {
            let key: String = key.extract()?;
            match key.as_str() {
                "max_notional" | "max_notional_per_order" => {
                    inner.max_notional_per_order = Some(from_py_decimal(&value)?);
                }
                "max_quantity" | "max_quantity_per_order" => {
                    inner.max_quantity_per_order = Some(Quantity::new(from_py_decimal(&value)?));
                }
                "max_daily_loss" => {
                    inner.max_daily_loss = Some(from_py_decimal(&value)?);
                }
                "price_collar_bps" => {
                    inner.price_collar_bps = Some(value.extract::<u32>()?);
                }
                "order_rate" => {
                    let table = value.downcast::<PyDict>()?;
                    let mut max_orders = None;
                    let mut window_ms = None;
                    for (key, value) in table.iter() {
                        let key: String = key.extract()?;
                        match key.as_str() {
                            "max_orders" => max_orders = Some(value.extract::<u32>()?),
                            "window_ms" => window_ms = Some(value.extract::<u64>()?),
                            other => {
                                return Err(PyValueError::new_err(format!(
                                    "unknown risk.order_rate key {other:?}"
                                )));
                            }
                        }
                    }
                    match (max_orders, window_ms) {
                        (Some(max_orders), Some(window_ms)) => {
                            inner.order_rate_limit = Some(honba_mon::OrderRateLimit {
                                max_orders,
                                window_ms,
                            });
                        }
                        _ => {
                            return Err(PyValueError::new_err(
                                "risk.order_rate needs both max_orders and window_ms",
                            ));
                        }
                    }
                }
                other => return Err(PyValueError::new_err(format!("unknown risk key {other:?}"))),
            }
        }
        Ok(Self { inner })
    }

    /// The maximum notional per order, if any.
    #[getter]
    fn max_notional_per_order<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(match self.inner.max_notional_per_order {
            Some(value) => Some(to_py_decimal(py, value)?),
            None => None,
        })
    }

    /// The maximum quantity per order, if any.
    #[getter]
    fn max_quantity_per_order<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(match self.inner.max_quantity_per_order {
            Some(value) => Some(to_py_decimal(py, value.0)?),
            None => None,
        })
    }

    /// The maximum daily loss, if any.
    #[getter]
    fn max_daily_loss<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(match self.inner.max_daily_loss {
            Some(value) => Some(to_py_decimal(py, value)?),
            None => None,
        })
    }

    /// Whether any limit at all is configured.
    fn is_empty(&self) -> bool {
        self.inner.max_notional_per_order.is_none()
            && self.inner.max_quantity_per_order.is_none()
            && self.inner.max_daily_loss.is_none()
            && self.inner.order_rate_limit.is_none()
            && self.inner.price_collar_bps.is_none()
            && self.inner.freeze_limits.is_empty()
    }

    /// Refuses a live run that has no limits.
    ///
    /// A live session without limits is an unbounded order flow, so it is an
    /// error rather than a warning.
    fn require_live(&self) -> PyResult<()> {
        if self.is_empty() {
            return Err(PyValueError::new_err(
                "live trading requires risk limits: set at least one of max_notional, \
                 max_quantity, max_daily_loss or order_rate",
            ));
        }
        Ok(())
    }

    /// The limits as their canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("RiskLimits({})", self.to_json())
    }
}

/// The order-to-trade ratio breaker's configuration.
#[pyclass(name = "OtrConfig", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyOtrConfig {
    pub(crate) inner: OtrConfig,
}

#[pymethods]
impl PyOtrConfig {
    /// Builds an OTR configuration.
    #[new]
    #[pyo3(signature = (max_otr=None, min_orders_before_enforcement=20, warning_threshold=None, window_1min_ms=60_000, window_5min_ms=300_000, halt_cooldown_ms=0))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_otr: Option<&Bound<'_, PyAny>>,
        min_orders_before_enforcement: u64,
        warning_threshold: Option<&Bound<'_, PyAny>>,
        window_1min_ms: u64,
        window_5min_ms: u64,
        halt_cooldown_ms: u64,
    ) -> PyResult<Self> {
        let max_otr = match max_otr {
            Some(value) => from_py_decimal(value)?,
            None => Decimal::from(50),
        };
        let warning_threshold = match warning_threshold {
            Some(value) => from_py_decimal(value)?,
            None => Decimal::from(40),
        };
        Ok(Self {
            inner: OtrConfig {
                max_otr,
                min_orders_before_enforcement,
                warning_threshold,
                window_1min_ms,
                window_5min_ms,
                halt_cooldown_ms,
            },
        })
    }

    /// The maximum order-to-trade ratio.
    #[getter]
    fn max_otr<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.inner.max_otr)
    }

    /// The configuration as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("OtrConfig({})", self.to_json())
    }
}

/// The pre-trade risk gate.
#[pyclass(name = "RiskEngine", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyRiskEngine {
    pub(crate) inner: RiskEngine,
}

#[pymethods]
impl PyRiskEngine {
    /// Builds a risk engine.
    #[new]
    #[pyo3(signature = (limits=None, otr=None))]
    pub fn new(limits: Option<PyRiskLimits>, otr: Option<PyOtrConfig>) -> Self {
        Self {
            inner: RiskEngine::new(
                limits.map(|limits| limits.inner).unwrap_or_default(),
                otr.map(|otr| otr.inner).unwrap_or_default(),
            ),
        }
    }

    /// Registers a SEBI algo id.
    fn register_algo(&mut self, algo_id: &str) {
        self.inner.register_algo(algo_id);
    }

    /// Suspends a registered algo id.
    fn suspend_algo(&mut self, algo_id: &str, reason: &str) {
        self.inner.suspend_algo(algo_id, reason);
    }

    /// Reactivates a suspended algo id.
    fn reactivate_algo(&mut self, algo_id: &str) {
        self.inner.reactivate_algo(algo_id);
    }

    /// Removes an algo id from the registry.
    fn unregister_algo(&mut self, algo_id: &str) {
        self.inner.unregister_algo(algo_id);
    }

    /// Whether an algo id is registered.
    fn is_algo_registered(&self, algo_id: &str) -> bool {
        self.inner.is_algo_registered(algo_id)
    }

    /// Halt every order (the global kill switch).
    #[pyo3(signature = (reason, now=None))]
    fn halt_global(&mut self, reason: &str, now: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let now = timestamp_or_now(now)?;
        self.inner.halt_global(reason, now);
        Ok(())
    }

    /// Lifts the global kill switch.
    fn resume_global(&mut self) {
        self.inner.resume_global();
    }

    /// Halt one algo id.
    #[pyo3(signature = (algo_id, reason, now=None))]
    fn halt_algo(
        &mut self,
        algo_id: &str,
        reason: &str,
        now: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let now = timestamp_or_now(now)?;
        self.inner.halt_algo(algo_id, reason, now);
        Ok(())
    }

    /// Resume one algo id.
    fn resume_algo(&mut self, algo_id: &str) {
        self.inner.resume_algo(algo_id);
    }

    /// Registers the circuit band of a scrip.
    fn set_price_band(&mut self, instrument: &PyInstrumentId, band: &PyPriceBand) {
        self.inner
            .set_price_band(instrument.inner.clone(), band.inner);
    }

    /// Checks an order against every rule.
    ///
    /// Raises `RiskRefusalError` when the order is refused.
    #[pyo3(signature = (order, now=None))]
    fn check_order(
        &mut self,
        order: &PyOrderRequest,
        now: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let now = timestamp_or_now(now)?;
        match self.inner.check_order(&order.inner, now) {
            Ok(()) => Ok(()),
            Err(refusal) => Err(refusal_error(
                &refusal.to_string(),
                refusal.rule(),
                &serde_json::to_string(&refusal).unwrap_or_default(),
            )),
        }
    }

    /// Checks an order on the fast path (no allocation-heavy bookkeeping).
    #[pyo3(signature = (algo_id, instrument, quantity, limit_price=None, now=None))]
    fn check_order_fast(
        &mut self,
        algo_id: &str,
        instrument: &PyInstrumentId,
        quantity: &Bound<'_, PyAny>,
        limit_price: Option<&Bound<'_, PyAny>>,
        now: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let now = timestamp_or_now(now)?;
        let quantity = Quantity::new(from_py_decimal(quantity)?);
        let limit_price = from_py_optional_decimal(limit_price)?.map(honba_core::Price::new);
        match self
            .inner
            .check_order_fast(algo_id, &instrument.inner, quantity, limit_price, now)
        {
            Ok(()) => Ok(()),
            Err(refusal) => Err(refusal_error(
                &refusal.to_string(),
                refusal.rule(),
                &serde_json::to_string(&refusal).unwrap_or_default(),
            )),
        }
    }

    /// Checks an AI decision against every rule.
    #[pyo3(signature = (decision, now=None))]
    fn check_decision(&mut self, decision: &str, now: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let now = timestamp_or_now(now)?;
        let decision: Decision = serde_json::from_str(decision)
            .map_err(|e| PyValueError::new_err(format!("invalid decision: {e}")))?;
        match self.inner.check_decision(&decision, now) {
            Ok(()) => Ok(()),
            Err(refusal) => Err(refusal_error(
                &refusal.to_string(),
                refusal.rule(),
                &serde_json::to_string(&refusal).unwrap_or_default(),
            )),
        }
    }

    /// Records a fill, so the OTR ratio falls.
    fn record_fill(&mut self, fill: &PyFillReport) {
        self.inner.record_fill(&fill.inner);
    }

    /// Records a cancellation, so the OTR ratio rises.
    #[pyo3(signature = (algo_id, now=None))]
    fn record_cancellation(
        &mut self,
        algo_id: &str,
        now: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let now = timestamp_or_now(now)?;
        self.inner.record_cancellation(algo_id, now);
        Ok(())
    }

    /// The cumulative order-to-trade ratio of an algo id.
    fn otr_for_algo<'py>(
        &self,
        algo_id: &str,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        Ok(match self.inner.otr_for_algo(algo_id) {
            Some(value) => Some(to_py_decimal(py, value)?),
            None => None,
        })
    }

    /// How many orders the gate has refused.
    fn total_refusals(&self) -> u64 {
        self.inner.total_refusals()
    }

    /// How many orders the gate refused for one rule.
    fn refusals_for_rule(&self, rule: &str) -> u64 {
        self.inner.refusals_for_rule(rule)
    }

    /// The engine as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner.limits).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        "RiskEngine(...)".to_string()
    }
}

/// The trading state a session can be in.
#[pyclass(name = "TradingState", module = "honba._honba", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyTradingState {
    /// Trading normally.
    #[pyo3(name = "ACTIVE")]
    Active,
    /// Closing positions only.
    #[pyo3(name = "REDUCING")]
    Reducing,
    /// Not trading at all.
    #[pyo3(name = "HALTED")]
    Halted,
}

#[pymethods]
impl PyTradingState {
    fn __str__(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Reducing => "reducing",
            Self::Halted => "halted",
        }
    }
}

/// The result of one risk check.
#[pyclass(name = "RiskDecision", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyRiskDecision {
    /// Whether the order may go out.
    #[pyo3(get)]
    pub approved: bool,
    /// The wire error code, empty when approved.
    #[pyo3(get)]
    pub code: String,
    /// The rule that decided, empty when approved.
    #[pyo3(get)]
    pub rule: String,
    /// The refusal details, empty when approved.
    #[pyo3(get)]
    pub context: String,
}

#[pymethods]
impl PyRiskDecision {
    fn __repr__(&self) -> String {
        format!(
            "RiskDecision(approved={}, rule='{}')",
            self.approved, self.rule
        )
    }
}

/// A risk gate in front of an order flow.
///
/// The gate is the `honba-mon` engine behind a stable Python face: `check`
/// answers with a [`PyRiskDecision`] instead of raising, for callers that want
/// to branch on the outcome rather than handle an exception.
#[pyclass(name = "RiskStage", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyRiskStage {
    pub(crate) engine: PyRiskEngine,
}

#[pymethods]
impl PyRiskStage {
    /// Builds a stage over a fresh engine.
    #[new]
    #[pyo3(signature = (limits=None, otr=None))]
    pub fn new(limits: Option<PyRiskLimits>, otr: Option<PyOtrConfig>) -> Self {
        Self {
            engine: PyRiskEngine::new(limits, otr),
        }
    }

    /// The engine behind this stage.
    #[getter]
    fn engine(&self) -> PyRiskEngine {
        self.engine.clone()
    }

    /// Checks an order, answering instead of raising.
    #[pyo3(signature = (order, now=None))]
    fn check(
        &mut self,
        order: &PyOrderRequest,
        now: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRiskDecision> {
        let now = timestamp_or_now(now)?;
        match self.engine.inner.check_order(&order.inner, now) {
            Ok(()) => Ok(PyRiskDecision {
                approved: true,
                code: String::new(),
                rule: String::new(),
                context: String::new(),
            }),
            Err(refusal) => Ok(PyRiskDecision {
                approved: false,
                code: serde_json::to_string(&refusal).unwrap_or_default(),
                rule: refusal.rule().to_string(),
                context: refusal.to_string(),
            }),
        }
    }

    /// Registers a SEBI algo id.
    fn register_algo(&mut self, algo_id: &str) {
        self.engine.register_algo(algo_id);
    }

    fn __repr__(&self) -> String {
        "RiskStage(...)".to_string()
    }
}

/// An operator command a live shell accepts.
#[pyclass(name = "Command", module = "honba._honba")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyCommand {
    /// `"stop"` or `"state"`.
    #[pyo3(get)]
    pub kind: &'static str,
    /// The trading state to move to, for a `state` command.
    #[pyo3(get)]
    pub state: Option<PyTradingState>,
}

#[pymethods]
impl PyCommand {
    /// The command to stop trading.
    #[staticmethod]
    pub fn stop() -> Self {
        Self {
            kind: "stop",
            state: None,
        }
    }

    /// The command to move to `state`.
    #[staticmethod]
    pub fn state(state: PyTradingState) -> Self {
        Self {
            kind: "state",
            state: Some(state),
        }
    }

    fn __repr__(&self) -> String {
        match self.state {
            Some(state) => format!("Command(kind='state', state={state:?})"),
            None => "Command(kind='stop')".to_string(),
        }
    }
}

/// Turns a refusal into the Python exception the gate raises.
pub fn refusal_error(message: &str, rule: &str, json: &str) -> PyErr {
    let err = RiskRefusalError::new_err(message.to_string());
    // The rule code and the refusal travel with the exception, so the caller
    // does not have to parse the message.
    Python::with_gil(|py| {
        let instance = err.value_bound(py);
        let _ = instance.setattr("rule", rule);
        let _ = instance.setattr("refusal", json);
    });
    err
}

/// The timestamp a check runs at: the caller's, or now.
///
/// The domain never reads the clock itself; this is the one place where the
/// boundary may ask for "now" explicitly.
fn timestamp_or_now(now: Option<&Bound<'_, PyAny>>) -> PyResult<chrono::DateTime<chrono::Utc>> {
    match from_py_optional_timestamp(now)? {
        Some(timestamp) => Ok(timestamp),
        None => Ok(chrono::Utc::now()),
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add(
        "RiskRefusalError",
        m.py().get_type_bound::<RiskRefusalError>(),
    )?;
    m.add_class::<PyRiskRefusal>()?;
    m.add_class::<PyRiskLimits>()?;
    m.add_class::<PyOtrConfig>()?;
    m.add_class::<PyRiskEngine>()?;
    m.add_class::<PyTradingState>()?;
    m.add_class::<PyRiskDecision>()?;
    m.add_class::<PyRiskStage>()?;
    m.add_class::<PyCommand>()?;
    Ok(())
}
