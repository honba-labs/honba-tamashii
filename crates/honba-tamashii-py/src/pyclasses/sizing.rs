//! Dynamic position sizing from `honba-core`, exposed to Python (plan.md, Phase 7).
//!
//! Sizing is a pure function of a signal, an account equity and a risk budget:
//! "what to trade" is decided by the strategy, "how much to trade" is decided
//! here. The math is the canonical [`honba_core::sizing`] kernel — this module
//! only marshals values across the FFI boundary, so a Python backtest and a
//! live Rust runner round the same lot the same way, to the last paisa.
//!
//! No float ever enters: every price, quantity and amount is an exact decimal
//! (`rust_decimal::Decimal` here, `decimal.Decimal` in Python), and a Python
//! `float` is refused with a `TypeError`.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;
use rust_decimal::Decimal;

use honba_core::sizing::{
    PositionSizingRequest, PositionSizingResult, RiskBudget, SizingMethod,
    calculate_position_size,
};
use honba_core::{
    InstrumentId, Money, PositionSide, Price, Quantity, Signal, SignalDirection, SignalHorizon,
    Timeframe,
};

use crate::decimal::{
    from_py_decimal, from_py_optional_decimal, from_py_optional_timestamp, to_py_decimal,
};
use crate::pyclasses::core::{PyInstrumentId, PyMoney, PyPrice, PyQuantity};

// ---------------------------------------------------------------------------
// Marshalling helpers
// ---------------------------------------------------------------------------

/// Reads a `Money`, or a bare amount interpreted as INR.
fn money_from(value: &Bound<'_, PyAny>) -> PyResult<Money> {
    if let Ok(money) = value.extract::<PyMoney>() {
        return Ok(money.inner);
    }
    Ok(Money::inr(from_py_decimal(value)?))
}

/// Reads a `Price`, or a bare decimal.
fn price_from(value: &Bound<'_, PyAny>) -> PyResult<Price> {
    if let Ok(price) = value.extract::<PyPrice>() {
        return Ok(price.inner);
    }
    Ok(Price::new(from_py_decimal(value)?))
}

/// Reads a `Quantity`, or a bare decimal.
fn quantity_from(value: &Bound<'_, PyAny>) -> PyResult<Quantity> {
    if let Ok(quantity) = value.extract::<PyQuantity>() {
        return Ok(quantity.inner);
    }
    Ok(Quantity::new(from_py_decimal(value)?))
}

/// Reads an `InstrumentId`, or a symbol / `"NSE:SYMBOL"` string.
fn instrument_from(value: &Bound<'_, PyAny>) -> PyResult<InstrumentId> {
    if let Ok(id) = value.extract::<PyInstrumentId>() {
        return Ok(id.inner);
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(PyInstrumentId::from_string(&text)?.inner);
    }
    Err(PyTypeError::new_err(
        "expected an InstrumentId or a symbol string",
    ))
}

/// Reads `"BUY"` / `"SELL"` (and the `LONG` / `SHORT` aliases).
fn side_from(text: &str) -> PyResult<PositionSide> {
    match text.to_ascii_uppercase().as_str() {
        "BUY" | "LONG" => Ok(PositionSide::Long),
        "SELL" | "SHORT" => Ok(PositionSide::Short),
        other => Err(PyValueError::new_err(format!(
            "side must be BUY or SELL, got {other:?}"
        ))),
    }
}

/// Reads an optional price argument.
fn optional_price(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Price>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(price_from(value)?)),
    }
}

/// Reads an optional quantity argument.
fn optional_quantity(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Quantity>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(quantity_from(value)?)),
    }
}

/// Reads an optional money argument.
fn optional_money(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Money>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(money_from(value)?)),
    }
}

/// Runs the canonical kernel with the GIL released.
fn run(py: Python<'_>, request: PositionSizingRequest) -> PyResult<PyPositionSizingResult> {
    let result = py
        .allow_threads(move || calculate_position_size(&request))
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(PyPositionSizingResult { inner: result })
}

/// Builds a risk budget with the limits every sizer accepts.
#[allow(clippy::too_many_arguments)]
fn make_budget(
    equity: Money,
    risk_pct: Decimal,
    lot_size: Quantity,
    max_notional: Option<Money>,
    max_qty: Option<Quantity>,
    freeze: Option<Quantity>,
) -> RiskBudget {
    let mut budget = RiskBudget::new(equity, risk_pct, lot_size);
    if let Some(max_notional) = max_notional {
        budget = budget.with_max_notional(max_notional);
    }
    if let Some(max_qty) = max_qty {
        budget = budget.with_max_qty(max_qty);
    }
    if let Some(freeze) = freeze {
        budget = budget.with_freeze_limit(freeze);
    }
    budget
}

// ---------------------------------------------------------------------------
// SizingMethod
// ---------------------------------------------------------------------------

/// One dynamic position sizing model.
#[pyclass(name = "SizingMethod", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PySizingMethod {
    pub(crate) inner: SizingMethod,
}

#[pymethods]
impl PySizingMethod {
    /// Risk `risk_pct` of equity per trade against the stop distance.
    #[staticmethod]
    fn fixed_fractional(risk_pct: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: SizingMethod::FixedFractional {
                risk_pct: from_py_decimal(risk_pct)?,
            },
        })
    }

    /// Size inversely proportional to instrument volatility.
    #[staticmethod]
    #[pyo3(signature = (target_annual_vol, lookback_bars=20))]
    fn volatility_target(
        target_annual_vol: &Bound<'_, PyAny>,
        lookback_bars: usize,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: SizingMethod::VolatilityTarget {
                target_annual_vol: from_py_decimal(target_annual_vol)?,
                lookback_bars,
            },
        })
    }

    /// Damped Kelly growth sizing (the default `fraction` is half-Kelly).
    #[staticmethod]
    #[pyo3(signature = (win_rate, payoff_ratio, fraction=None))]
    fn kelly_criterion(
        win_rate: &Bound<'_, PyAny>,
        payoff_ratio: &Bound<'_, PyAny>,
        fraction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let fraction = from_py_optional_decimal(fraction)?
            .unwrap_or_else(|| Decimal::new(5, 1));
        Ok(Self {
            inner: SizingMethod::KellyCriterion {
                fraction,
                win_rate: from_py_decimal(win_rate)?,
                payoff_ratio: from_py_decimal(payoff_ratio)?,
            },
        })
    }

    /// Allocate the risk budget equally across the active signals.
    #[staticmethod]
    fn equal_risk_contribution() -> Self {
        Self {
            inner: SizingMethod::EqualRiskContribution,
        }
    }

    /// Allocate a fixed cash notional.
    #[staticmethod]
    fn fixed_notional(notional: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: SizingMethod::FixedNotional {
                notional: money_from(notional)?,
            },
        })
    }

    /// Reads a method from its canonical JSON form.
    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        let inner = serde_json::from_str::<SizingMethod>(text)
            .map_err(|e| PyValueError::new_err(format!("invalid sizing method: {e}")))?;
        Ok(Self { inner })
    }

    /// The variant name (`fixed_fractional`, `volatility_target`, ...).
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            SizingMethod::FixedFractional { .. } => "fixed_fractional",
            SizingMethod::VolatilityTarget { .. } => "volatility_target",
            SizingMethod::KellyCriterion { .. } => "kelly_criterion",
            SizingMethod::EqualRiskContribution => "equal_risk_contribution",
            SizingMethod::FixedNotional { .. } => "fixed_notional",
        }
    }

    /// The method as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("SizingMethod({})", self.to_json())
    }
}

// ---------------------------------------------------------------------------
// RiskBudget
// ---------------------------------------------------------------------------

/// Account equity, risk fraction and position limits for a sizing run.
#[pyclass(name = "RiskBudget", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyRiskBudget {
    pub(crate) inner: RiskBudget,
}

#[pymethods]
impl PyRiskBudget {
    /// `RiskBudget(equity, risk_pct, min_lot_size=1)`.
    #[new]
    #[pyo3(signature = (equity, risk_pct, min_lot_size=None, max_position_notional=None, max_position_qty=None, max_portfolio_risk_pct=None, freeze_limit_qty=None))]
    fn new(
        equity: &Bound<'_, PyAny>,
        risk_pct: &Bound<'_, PyAny>,
        min_lot_size: Option<&Bound<'_, PyAny>>,
        max_position_notional: Option<&Bound<'_, PyAny>>,
        max_position_qty: Option<&Bound<'_, PyAny>>,
        max_portfolio_risk_pct: Option<&Bound<'_, PyAny>>,
        freeze_limit_qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let lot = optional_quantity(min_lot_size)?.unwrap_or_else(|| Quantity::from_u64(1));
        let mut inner = RiskBudget::new(money_from(equity)?, from_py_decimal(risk_pct)?, lot);
        if let Some(notional) = optional_money(max_position_notional)? {
            inner = inner.with_max_notional(notional);
        }
        if let Some(qty) = optional_quantity(max_position_qty)? {
            inner = inner.with_max_qty(qty);
        }
        if let Some(freeze) = optional_quantity(freeze_limit_qty)? {
            inner = inner.with_freeze_limit(freeze);
        }
        if let Some(max_risk) = from_py_optional_decimal(max_portfolio_risk_pct)? {
            inner = inner.with_max_portfolio_risk(max_risk);
        }
        Ok(Self { inner })
    }

    /// The account equity the budget applies to.
    #[getter]
    fn equity(&self) -> PyMoney {
        PyMoney {
            inner: self.inner.equity,
        }
    }

    /// The default risk fraction per trade.
    #[getter]
    fn risk_pct<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.inner.risk_pct)
    }

    /// The instrument minimum lot size.
    #[getter]
    fn min_lot_size(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.min_lot_size,
        }
    }

    /// The notional cap, if configured.
    #[getter]
    fn max_position_notional(&self) -> Option<PyMoney> {
        self.inner.max_position_notional.map(|m| PyMoney { inner: m })
    }

    /// The quantity cap, if configured.
    #[getter]
    fn max_position_qty(&self) -> Option<PyQuantity> {
        self.inner.max_position_qty.map(|q| PyQuantity { inner: q })
    }

    /// The portfolio risk cap, if configured.
    #[getter]
    fn max_portfolio_risk_pct<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.max_portfolio_risk_pct {
            Some(value) => Ok(Some(to_py_decimal(py, value)?)),
            None => Ok(None),
        }
    }

    /// The scrip freeze limit, if configured.
    #[getter]
    fn freeze_limit_qty(&self) -> Option<PyQuantity> {
        self.inner.freeze_limit_qty.map(|q| PyQuantity { inner: q })
    }

    /// Returns a copy with a notional cap.
    fn with_max_notional(&self, max_notional: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().with_max_notional(money_from(max_notional)?),
        })
    }

    /// Returns a copy with a quantity cap.
    fn with_max_qty(&self, max_qty: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().with_max_qty(quantity_from(max_qty)?),
        })
    }

    /// Returns a copy with a scrip freeze limit.
    fn with_freeze_limit(&self, freeze_qty: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_freeze_limit(quantity_from(freeze_qty)?),
        })
    }

    /// Returns a copy with a portfolio risk cap.
    fn with_max_portfolio_risk(&self, max_risk: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_max_portfolio_risk(from_py_decimal(max_risk)?),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "RiskBudget(equity='{} {}', risk_pct='{}', lot='{}')",
            self.inner.equity.amount,
            self.inner.equity.currency.code(),
            self.inner.risk_pct,
            self.inner.min_lot_size.0
        )
    }
}

// ---------------------------------------------------------------------------
// PositionSizingRequest / PositionSizingResult
// ---------------------------------------------------------------------------

/// A sizing question: instrument, side, entry, stop, equity, budget, method.
#[pyclass(name = "PositionSizingRequest", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyPositionSizingRequest {
    pub(crate) inner: PositionSizingRequest,
}

#[pymethods]
impl PyPositionSizingRequest {
    #[new]
    #[pyo3(signature = (instrument, side, entry_price, account_equity, risk_budget, sizing_method))]
    fn new(
        instrument: &Bound<'_, PyAny>,
        side: &str,
        entry_price: &Bound<'_, PyAny>,
        account_equity: &Bound<'_, PyAny>,
        risk_budget: PyRiskBudget,
        sizing_method: PySizingMethod,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: PositionSizingRequest::new(
                instrument_from(instrument)?,
                side_from(side)?,
                price_from(entry_price)?,
                money_from(account_equity)?,
                risk_budget.inner,
                sizing_method.inner,
            ),
        })
    }

    /// Attaches a stop loss price.
    fn with_stop_loss(&self, stop_loss: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().with_stop_loss(price_from(stop_loss)?),
        })
    }

    /// Attaches a current volatility estimate (e.g. ATR as a fraction).
    fn with_volatility(&self, volatility: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_volatility(from_py_decimal(volatility)?),
        })
    }

    /// Computes the sizing result.
    fn calculate(&self, py: Python<'_>) -> PyResult<PyPositionSizingResult> {
        run(py, self.inner.clone())
    }

    /// The target instrument.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument.clone(),
        }
    }

    /// The planned side (`LONG` / `SHORT`).
    #[getter]
    fn side(&self) -> &'static str {
        match self.inner.side {
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
        }
    }

    /// The expected entry price.
    #[getter]
    fn entry_price(&self) -> PyPrice {
        PyPrice {
            inner: self.inner.entry_price,
        }
    }

    /// The stop loss, if defined.
    #[getter]
    fn stop_loss(&self) -> Option<PyPrice> {
        self.inner.stop_loss.map(|p| PyPrice { inner: p })
    }

    /// The current volatility estimate, if defined.
    #[getter]
    fn current_volatility<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.current_volatility {
            Some(value) => Ok(Some(to_py_decimal(py, value)?)),
            None => Ok(None),
        }
    }

    /// The available account equity.
    #[getter]
    fn account_equity(&self) -> PyMoney {
        PyMoney {
            inner: self.inner.account_equity,
        }
    }

    /// The risk budget.
    #[getter]
    fn risk_budget(&self) -> PyRiskBudget {
        PyRiskBudget {
            inner: self.inner.risk_budget.clone(),
        }
    }

    /// The sizing method.
    #[getter]
    fn sizing_method(&self) -> PySizingMethod {
        PySizingMethod {
            inner: self.inner.sizing_method.clone(),
        }
    }

    /// The request as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionSizingRequest('{}', {}, entry='{}')",
            self.inner.instrument, self.side(), self.inner.entry_price
        )
    }
}

/// The answer: a lot-rounded quantity plus its notional and risk.
#[pyclass(name = "PositionSizingResult", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyPositionSizingResult {
    pub(crate) inner: PositionSizingResult,
}

impl PyPositionSizingResult {
    pub(crate) fn from_inner(inner: PositionSizingResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionSizingResult {
    /// The target instrument.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument.clone(),
        }
    }

    /// The executable quantity, already rounded down to whole lots.
    #[getter]
    fn quantity(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.quantity,
        }
    }

    /// `entry_price * quantity`.
    #[getter]
    fn allocated_notional(&self) -> PyMoney {
        PyMoney {
            inner: self.inner.allocated_notional,
        }
    }

    /// `|entry_price - stop_loss| * quantity` (the notional when there is no stop).
    #[getter]
    fn risk_at_stake(&self) -> PyMoney {
        PyMoney {
            inner: self.inner.risk_at_stake,
        }
    }

    /// `allocated_notional / account_equity`, if defined.
    #[getter]
    fn target_weight<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.target_weight {
            Some(value) => Ok(Some(to_py_decimal(py, value)?)),
            None => Ok(None),
        }
    }

    /// Whether a limit or freeze cap reduced the quantity.
    #[getter]
    fn capped_by_limit(&self) -> bool {
        self.inner.capped_by_limit
    }

    /// Which limit reduced the quantity, if any.
    #[getter]
    fn reason(&self) -> Option<String> {
        self.inner.reason.clone()
    }

    /// The result as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionSizingResult('{}', quantity='{}', notional='{} {}')",
            self.inner.instrument,
            self.inner.quantity.0,
            self.inner.allocated_notional.amount,
            self.inner.allocated_notional.currency.code()
        )
    }
}

// ---------------------------------------------------------------------------
// Signal
// ---------------------------------------------------------------------------

/// Converts a direction string to the canonical enum.
fn direction_from(text: &str) -> PyResult<SignalDirection> {
    match text.to_ascii_lowercase().as_str() {
        "bullish" | "long" => Ok(SignalDirection::Bullish),
        "bearish" | "short" => Ok(SignalDirection::Bearish),
        "neutral" => Ok(SignalDirection::Neutral),
        other => Err(PyValueError::new_err(format!(
            "direction must be bullish, bearish or neutral, got {other:?}"
        ))),
    }
}

/// Converts a horizon string to the canonical enum.
fn horizon_from(text: &str) -> PyResult<SignalHorizon> {
    match text.to_ascii_lowercase().as_str() {
        "intraday" => Ok(SignalHorizon::Intraday),
        "swing" => Ok(SignalHorizon::Swing),
        "positional" => Ok(SignalHorizon::Positional),
        other => Err(PyValueError::new_err(format!(
            "horizon must be intraday, swing or positional, got {other:?}"
        ))),
    }
}

/// Converts a timeframe string to the canonical enum.
fn timeframe_from(text: &str) -> PyResult<Timeframe> {
    match text.to_ascii_lowercase().as_str() {
        "1m" | "m1" => Ok(Timeframe::M1),
        "5m" | "m5" => Ok(Timeframe::M5),
        "15m" | "m15" => Ok(Timeframe::M15),
        "30m" | "m30" => Ok(Timeframe::M30),
        "1h" | "h1" => Ok(Timeframe::H1),
        "1d" | "d1" => Ok(Timeframe::D1),
        "1w" | "w1" => Ok(Timeframe::W1),
        other => Err(PyValueError::new_err(format!(
            "unknown timeframe {other:?}; expected 1m, 5m, 15m, 30m, 1h, 1d or 1w"
        ))),
    }
}

/// A strategy signal: what to trade, in which direction, at which levels.
#[pyclass(name = "Signal", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PySignal {
    pub(crate) inner: Signal,
}

#[pymethods]
impl PySignal {
    /// `Signal(instrument, direction="bullish", entry_price="2500")`.
    #[new]
    #[pyo3(signature = (instrument, direction="bullish", horizon="intraday", timeframe="1d", strength=None, generated_by="python", timestamp=None, entry_price=None, stop_loss=None, take_profit=None, volatility_atr=None))]
    fn new(
        instrument: &Bound<'_, PyAny>,
        direction: &str,
        horizon: &str,
        timeframe: &str,
        strength: Option<&Bound<'_, PyAny>>,
        generated_by: &str,
        timestamp: Option<&Bound<'_, PyAny>>,
        entry_price: Option<&Bound<'_, PyAny>>,
        stop_loss: Option<&Bound<'_, PyAny>>,
        take_profit: Option<&Bound<'_, PyAny>>,
        volatility_atr: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let strength = from_py_optional_decimal(strength)?.unwrap_or(Decimal::ONE);
        let timestamp = from_py_optional_timestamp(timestamp)?.unwrap_or_else(chrono::Utc::now);
        let mut inner = Signal::new(
            instrument_from(instrument)?,
            direction_from(direction)?,
            horizon_from(horizon)?,
            timeframe_from(timeframe)?,
            strength,
            generated_by,
            timestamp,
        );
        if let Some(price) = optional_price(entry_price)? {
            inner = inner.with_entry_price(price);
        }
        if let Some(price) = optional_price(stop_loss)? {
            inner = inner.with_stop_loss(price);
        }
        if let Some(price) = optional_price(take_profit)? {
            inner = inner.with_take_profit(price);
        }
        if let Some(volatility) = from_py_optional_decimal(volatility_atr)? {
            inner = inner.with_volatility_atr(volatility);
        }
        Ok(Self { inner })
    }

    /// Attaches a recommended entry price.
    fn with_entry_price(&self, price: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().with_entry_price(price_from(price)?),
        })
    }

    /// Attaches a recommended stop loss.
    fn with_stop_loss(&self, price: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().with_stop_loss(price_from(price)?),
        })
    }

    /// Attaches a recommended take profit.
    fn with_take_profit(&self, price: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().with_take_profit(price_from(price)?),
        })
    }

    /// Attaches an ATR / volatility estimate.
    fn with_volatility_atr(&self, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_volatility_atr(from_py_decimal(value)?),
        })
    }

    /// The instrument the signal is about.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument.clone(),
        }
    }

    /// The directional bias.
    #[getter]
    fn direction(&self) -> &'static str {
        match self.inner.direction {
            SignalDirection::Bullish => "bullish",
            SignalDirection::Bearish => "bearish",
            SignalDirection::Neutral => "neutral",
        }
    }

    /// The order side the direction implies (`BUY`, `SELL`, or `None`).
    #[getter]
    fn side(&self) -> Option<&'static str> {
        match self.inner.direction {
            SignalDirection::Bullish => Some("BUY"),
            SignalDirection::Bearish => Some("SELL"),
            SignalDirection::Neutral => None,
        }
    }

    /// The intended holding horizon.
    #[getter]
    fn horizon(&self) -> &'static str {
        match self.inner.horizon {
            SignalHorizon::Intraday => "intraday",
            SignalHorizon::Swing => "swing",
            SignalHorizon::Positional => "positional",
        }
    }

    /// The bar interval.
    #[getter]
    fn timeframe(&self) -> &'static str {
        match self.inner.timeframe {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
        }
    }

    /// The confidence, in `[0, 1]`.
    #[getter]
    fn strength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.inner.strength)
    }

    /// The strategy / algo id that generated the signal.
    #[getter]
    fn generated_by(&self) -> String {
        self.inner.generated_by.clone()
    }

    /// When the signal was generated (UTC).
    #[getter]
    fn timestamp(&self) -> String {
        self.inner.timestamp.to_rfc3339()
    }

    /// The recommended entry price, if any.
    #[getter]
    fn entry_price(&self) -> Option<PyPrice> {
        self.inner.entry_price.map(|p| PyPrice { inner: p })
    }

    /// The recommended stop loss, if any.
    #[getter]
    fn stop_loss(&self) -> Option<PyPrice> {
        self.inner.stop_loss.map(|p| PyPrice { inner: p })
    }

    /// The recommended take profit, if any.
    #[getter]
    fn take_profit(&self) -> Option<PyPrice> {
        self.inner.take_profit.map(|p| PyPrice { inner: p })
    }

    /// The ATR / volatility estimate, if any.
    #[getter]
    fn volatility_atr<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.volatility_atr {
            Some(value) => Ok(Some(to_py_decimal(py, value)?)),
            None => Ok(None),
        }
    }

    /// The signal as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "Signal('{}', {}, entry={:?})",
            self.inner.instrument, self.direction(), self.inner.entry_price
        )
    }
}

// ---------------------------------------------------------------------------
// Native sizer kernels
// ---------------------------------------------------------------------------

/// The sizer kernels share the same limit set.
#[derive(Clone, Debug)]
struct Limits {
    lot_size: Quantity,
    max_notional: Option<Money>,
    max_qty: Option<Quantity>,
    freeze: Option<Quantity>,
}

impl Limits {
    fn new(
        lot_size: Option<&Bound<'_, PyAny>>,
        max_notional: Option<&Bound<'_, PyAny>>,
        max_qty: Option<&Bound<'_, PyAny>>,
        freeze: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            lot_size: optional_quantity(lot_size)?.unwrap_or_else(|| Quantity::from_u64(1)),
            max_notional: optional_money(max_notional)?,
            max_qty: optional_quantity(max_qty)?,
            freeze: optional_quantity(freeze)?,
        })
    }

    fn budget(&self, equity: Money, risk_pct: Decimal) -> RiskBudget {
        make_budget(
            equity,
            risk_pct,
            self.lot_size,
            self.max_notional,
            self.max_qty,
            self.freeze,
        )
    }
}

/// Risk a fixed fraction of equity against the stop distance.
#[pyclass(name = "FixedFractionalSizer", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyFixedFractionalSizer {
    risk_pct: Decimal,
    limits: Limits,
}

#[pymethods]
impl PyFixedFractionalSizer {
    #[new]
    #[pyo3(signature = (risk_pct=None, lot_size=None, max_position_notional=None, max_position_qty=None, freeze_limit_qty=None))]
    fn new(
        risk_pct: Option<&Bound<'_, PyAny>>,
        lot_size: Option<&Bound<'_, PyAny>>,
        max_position_notional: Option<&Bound<'_, PyAny>>,
        max_position_qty: Option<&Bound<'_, PyAny>>,
        freeze_limit_qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            risk_pct: from_py_optional_decimal(risk_pct)?
                .unwrap_or_else(|| Decimal::new(1, 2)),
            limits: Limits::new(
                lot_size,
                max_position_notional,
                max_position_qty,
                freeze_limit_qty,
            )?,
        })
    }

    /// `Q = floor(equity * risk_pct / |entry - stop| / lot) * lot`.
    #[pyo3(signature = (instrument, entry_price, equity, side="BUY", stop_loss=None))]
    fn calculate_size(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        entry_price: &Bound<'_, PyAny>,
        equity: &Bound<'_, PyAny>,
        side: &str,
        stop_loss: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyPositionSizingResult> {
        let equity = money_from(equity)?;
        let mut request = PositionSizingRequest::new(
            instrument_from(instrument)?,
            side_from(side)?,
            price_from(entry_price)?,
            equity,
            self.limits.budget(equity, self.risk_pct),
            SizingMethod::FixedFractional {
                risk_pct: self.risk_pct,
            },
        );
        if let Some(stop) = optional_price(stop_loss)? {
            request = request.with_stop_loss(stop);
        }
        run(py, request)
    }

    #[getter]
    fn risk_pct<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.risk_pct)
    }

    #[getter]
    fn lot_size(&self) -> PyQuantity {
        PyQuantity {
            inner: self.limits.lot_size,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedFractionalSizer(risk_pct='{}', lot='{}')",
            self.risk_pct, self.limits.lot_size.0
        )
    }
}

/// Size from an ATR band: the stop distance is `ATR * multiplier`.
#[pyclass(name = "AtrVolatilitySizer", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyAtrVolatilitySizer {
    target_risk: Decimal,
    multiplier: Decimal,
    lookback_bars: usize,
    limits: Limits,
}

#[pymethods]
impl PyAtrVolatilitySizer {
    #[new]
    #[pyo3(signature = (target_risk=None, multiplier=None, lookback_bars=20, lot_size=None, max_position_notional=None, max_position_qty=None, freeze_limit_qty=None))]
    fn new(
        target_risk: Option<&Bound<'_, PyAny>>,
        multiplier: Option<&Bound<'_, PyAny>>,
        lookback_bars: usize,
        lot_size: Option<&Bound<'_, PyAny>>,
        max_position_notional: Option<&Bound<'_, PyAny>>,
        max_position_qty: Option<&Bound<'_, PyAny>>,
        freeze_limit_qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            target_risk: from_py_optional_decimal(target_risk)?
                .unwrap_or_else(|| Decimal::new(1, 2)),
            multiplier: from_py_optional_decimal(multiplier)?.unwrap_or(Decimal::ONE),
            lookback_bars,
            limits: Limits::new(
                lot_size,
                max_position_notional,
                max_position_qty,
                freeze_limit_qty,
            )?,
        })
    }

    /// `Q = floor(equity * target_risk / (ATR * multiplier) / lot) * lot`.
    #[pyo3(signature = (instrument, entry_price, equity, atr, side="BUY", stop_loss=None))]
    fn calculate_size(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        entry_price: &Bound<'_, PyAny>,
        equity: &Bound<'_, PyAny>,
        atr: &Bound<'_, PyAny>,
        side: &str,
        stop_loss: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyPositionSizingResult> {
        let equity = money_from(equity)?;
        let entry = price_from(entry_price)?;
        if !entry.is_positive() {
            return Err(PyValueError::new_err("entry price must be strictly positive"));
        }
        // ATR is a price distance; the canonical volatility target is
        // dimensionless, so express the band as a fraction of the entry price.
        let volatility = (from_py_decimal(atr)? * self.multiplier) / entry.value();
        let mut request = PositionSizingRequest::new(
            instrument_from(instrument)?,
            side_from(side)?,
            entry,
            equity,
            self.limits.budget(equity, self.target_risk),
            SizingMethod::VolatilityTarget {
                target_annual_vol: self.target_risk,
                lookback_bars: self.lookback_bars,
            },
        )
        .with_volatility(volatility);
        if let Some(stop) = optional_price(stop_loss)? {
            request = request.with_stop_loss(stop);
        }
        run(py, request)
    }

    #[getter]
    fn target_risk<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.target_risk)
    }

    #[getter]
    fn multiplier<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.multiplier)
    }

    #[getter]
    fn lot_size(&self) -> PyQuantity {
        PyQuantity {
            inner: self.limits.lot_size,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "AtrVolatilitySizer(target_risk='{}', multiplier='{}')",
            self.target_risk, self.multiplier
        )
    }
}

/// Damped Kelly sizing: `f* = p - (1 - p) / b`, scaled by `fraction`.
#[pyclass(name = "KellySizer", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyKellySizer {
    win_rate: Decimal,
    payoff_ratio: Decimal,
    fraction: Decimal,
    limits: Limits,
}

#[pymethods]
impl PyKellySizer {
    #[new]
    #[pyo3(signature = (win_rate, payoff_ratio, fraction=None, lot_size=None, max_position_notional=None, max_position_qty=None, freeze_limit_qty=None))]
    fn new(
        win_rate: &Bound<'_, PyAny>,
        payoff_ratio: &Bound<'_, PyAny>,
        fraction: Option<&Bound<'_, PyAny>>,
        lot_size: Option<&Bound<'_, PyAny>>,
        max_position_notional: Option<&Bound<'_, PyAny>>,
        max_position_qty: Option<&Bound<'_, PyAny>>,
        freeze_limit_qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            win_rate: from_py_decimal(win_rate)?,
            payoff_ratio: from_py_decimal(payoff_ratio)?,
            fraction: from_py_optional_decimal(fraction)?
                .unwrap_or_else(|| Decimal::new(5, 1)),
            limits: Limits::new(
                lot_size,
                max_position_notional,
                max_position_qty,
                freeze_limit_qty,
            )?,
        })
    }

    /// The raw (undamped) Kelly fraction, `p - (1 - p) / b`. Never negative.
    #[getter]
    fn kelly_fraction<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let kelly = if self.payoff_ratio > Decimal::ZERO {
            self.win_rate - ((Decimal::ONE - self.win_rate) / self.payoff_ratio)
        } else {
            Decimal::ZERO
        };
        to_py_decimal(py, kelly.max(Decimal::ZERO))
    }

    /// Applies the damped Kelly fraction to the equity and the stop distance.
    #[pyo3(signature = (instrument, entry_price, equity, side="BUY", stop_loss=None))]
    fn calculate_size(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        entry_price: &Bound<'_, PyAny>,
        equity: &Bound<'_, PyAny>,
        side: &str,
        stop_loss: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyPositionSizingResult> {
        let equity = money_from(equity)?;
        let mut request = PositionSizingRequest::new(
            instrument_from(instrument)?,
            side_from(side)?,
            price_from(entry_price)?,
            equity,
            self.limits.budget(equity, self.win_rate),
            SizingMethod::KellyCriterion {
                fraction: self.fraction,
                win_rate: self.win_rate,
                payoff_ratio: self.payoff_ratio,
            },
        );
        if let Some(stop) = optional_price(stop_loss)? {
            request = request.with_stop_loss(stop);
        }
        run(py, request)
    }

    #[getter]
    fn win_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.win_rate)
    }

    #[getter]
    fn payoff_ratio<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.payoff_ratio)
    }

    #[getter]
    fn fraction<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.fraction)
    }

    #[getter]
    fn lot_size(&self) -> PyQuantity {
        PyQuantity {
            inner: self.limits.lot_size,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "KellySizer(win_rate='{}', payoff_ratio='{}', fraction='{}')",
            self.win_rate, self.payoff_ratio, self.fraction
        )
    }
}

/// Inverse-volatility risk parity across a universe of candidates.
#[pyclass(name = "EqualRiskSizer", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyEqualRiskSizer {
    risk_pct: Decimal,
    limits: Limits,
}

#[pymethods]
impl PyEqualRiskSizer {
    #[new]
    #[pyo3(signature = (risk_pct=None, lot_size=None, max_position_notional=None, max_position_qty=None, freeze_limit_qty=None))]
    fn new(
        risk_pct: Option<&Bound<'_, PyAny>>,
        lot_size: Option<&Bound<'_, PyAny>>,
        max_position_notional: Option<&Bound<'_, PyAny>>,
        max_position_qty: Option<&Bound<'_, PyAny>>,
        freeze_limit_qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            risk_pct: from_py_optional_decimal(risk_pct)?
                .unwrap_or_else(|| Decimal::new(1, 2)),
            limits: Limits::new(
                lot_size,
                max_position_notional,
                max_position_qty,
                freeze_limit_qty,
            )?,
        })
    }

    /// Allocates `gross_exposure * equity` across the candidates in inverse
    /// proportion to their volatility; each leg is then lot-rounded by the
    /// canonical kernel.
    #[pyo3(signature = (equity, instruments, entry_prices, volatilities, gross_exposure=None))]
    fn allocate(
        &self,
        py: Python<'_>,
        equity: &Bound<'_, PyAny>,
        instruments: Vec<Bound<'_, PyAny>>,
        entry_prices: Vec<Bound<'_, PyAny>>,
        volatilities: Vec<Bound<'_, PyAny>>,
        gross_exposure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<PyPositionSizingResult>> {
        if instruments.len() != entry_prices.len() || instruments.len() != volatilities.len() {
            return Err(PyValueError::new_err(
                "instruments, entry_prices and volatilities must have the same length",
            ));
        }
        if instruments.is_empty() {
            return Ok(Vec::new());
        }

        let equity = money_from(equity)?;
        let gross = from_py_optional_decimal(gross_exposure)?.unwrap_or(Decimal::ONE);

        let mut parsed = Vec::with_capacity(instruments.len());
        let mut inverse_total = Decimal::ZERO;
        for index in 0..instruments.len() {
            let instrument = instrument_from(&instruments[index])?;
            let entry = price_from(&entry_prices[index])?;
            let volatility = from_py_decimal(&volatilities[index])?;
            if !volatility.is_zero() {
                inverse_total += Decimal::ONE / volatility;
            }
            parsed.push((instrument, entry, volatility));
        }
        if inverse_total.is_zero() {
            return Err(PyValueError::new_err(
                "at least one candidate must have a positive volatility",
            ));
        }

        let mut requests = Vec::with_capacity(parsed.len());
        for (instrument, entry, volatility) in parsed {
            let weight = if volatility.is_zero() {
                Decimal::ZERO
            } else {
                (Decimal::ONE / volatility) / inverse_total
            };
            let notional = Money::new(equity.amount * gross * weight, equity.currency);
            requests.push(PositionSizingRequest::new(
                instrument,
                PositionSide::Long,
                entry,
                equity,
                self.limits.budget(equity, self.risk_pct),
                SizingMethod::FixedNotional { notional },
            ));
        }

        let results = py
            .allow_threads(move || {
                requests
                    .iter()
                    .map(calculate_position_size)
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(results
            .into_iter()
            .map(PyPositionSizingResult::from_inner)
            .collect())
    }

    #[getter]
    fn risk_pct<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.risk_pct)
    }

    #[getter]
    fn lot_size(&self) -> PyQuantity {
        PyQuantity {
            inner: self.limits.lot_size,
        }
    }

    fn __repr__(&self) -> String {
        format!("EqualRiskSizer(risk_pct='{}')", self.risk_pct)
    }
}

/// Convert a fixed cash notional to a lot-rounded quantity.
#[pyclass(name = "FixedNotionalSizer", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyFixedNotionalSizer {
    notional: Money,
    limits: Limits,
}

#[pymethods]
impl PyFixedNotionalSizer {
    #[new]
    #[pyo3(signature = (notional, lot_size=None, max_position_notional=None, max_position_qty=None, freeze_limit_qty=None))]
    fn new(
        notional: &Bound<'_, PyAny>,
        lot_size: Option<&Bound<'_, PyAny>>,
        max_position_notional: Option<&Bound<'_, PyAny>>,
        max_position_qty: Option<&Bound<'_, PyAny>>,
        freeze_limit_qty: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(Self {
            notional: money_from(notional)?,
            limits: Limits::new(
                lot_size,
                max_position_notional,
                max_position_qty,
                freeze_limit_qty,
            )?,
        })
    }

    /// `Q = floor(notional / entry / lot) * lot`.
    #[pyo3(signature = (instrument, entry_price, side="BUY"))]
    fn calculate_size(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        entry_price: &Bound<'_, PyAny>,
        side: &str,
    ) -> PyResult<PyPositionSizingResult> {
        let equity = Money::new(self.notional.amount, self.notional.currency);
        let request = PositionSizingRequest::new(
            instrument_from(instrument)?,
            side_from(side)?,
            price_from(entry_price)?,
            equity,
            self.limits.budget(equity, Decimal::ZERO),
            SizingMethod::FixedNotional {
                notional: self.notional,
            },
        );
        run(py, request)
    }

    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney {
            inner: self.notional,
        }
    }

    #[getter]
    fn lot_size(&self) -> PyQuantity {
        PyQuantity {
            inner: self.limits.lot_size,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FixedNotionalSizer(notional='{} {}')",
            self.notional.amount,
            self.notional.currency.code()
        )
    }
}

// ---------------------------------------------------------------------------
// Context helper + vectorized batch
// ---------------------------------------------------------------------------

/// The default sizer used by `ctx.size_signal` when none is supplied: 1 % of
/// equity against the signal's stop, in whole units.
pub(crate) fn size_signal_default(
    py: Python<'_>,
    signal: &PySignal,
    equity: Money,
) -> PyResult<PyPositionSizingResult> {
    let side = match signal.inner.direction {
        SignalDirection::Bullish => PositionSide::Long,
        SignalDirection::Bearish => PositionSide::Short,
        SignalDirection::Neutral => {
            return Err(PyValueError::new_err(
                "a neutral signal has no order side to size",
            ));
        }
    };
    let entry = signal.inner.entry_price.ok_or_else(|| {
        PyValueError::new_err("the signal has no entry_price; attach one before sizing")
    })?;
    let risk_pct = Decimal::new(1, 2);
    let limits = Limits {
        lot_size: Quantity::from_u64(1),
        max_notional: None,
        max_qty: None,
        freeze: None,
    };
    let mut request = PositionSizingRequest::new(
        signal.inner.instrument.clone(),
        side,
        entry,
        equity,
        limits.budget(equity, risk_pct),
        SizingMethod::FixedFractional { risk_pct },
    );
    if let Some(stop) = signal.inner.stop_loss {
        request = request.with_stop_loss(stop);
    }
    run(py, request)
}

/// Vectorized fixed-fractional sizing across parallel columns.
///
/// `instruments`, `entry_prices` and the optional `stop_losses` /
/// `volatilities` are equal-length sequences (Arrow-backed columns work: they
/// come across as lists of exact values). The kernel loop releases the GIL, so
/// a large universe is sized without blocking other Python threads.
#[pyfunction]
#[pyo3(signature = (instruments, entry_prices, equity, risk_pct=None, lot_size=None, stop_losses=None, volatilities=None, method="fixed_fractional"))]
pub fn size_batch(
    py: Python<'_>,
    instruments: Vec<Bound<'_, PyAny>>,
    entry_prices: Vec<Bound<'_, PyAny>>,
    equity: &Bound<'_, PyAny>,
    risk_pct: Option<&Bound<'_, PyAny>>,
    lot_size: Option<&Bound<'_, PyAny>>,
    stop_losses: Option<Vec<Bound<'_, PyAny>>>,
    volatilities: Option<Vec<Bound<'_, PyAny>>>,
    method: &str,
) -> PyResult<Vec<PyPositionSizingResult>> {
    let count = instruments.len();
    if entry_prices.len() != count {
        return Err(PyValueError::new_err(
            "instruments and entry_prices must have the same length",
        ));
    }
    if let Some(ref stops) = stop_losses {
        if stops.len() != count {
            return Err(PyValueError::new_err(
                "stop_losses must have the same length as instruments",
            ));
        }
    }
    if let Some(ref vols) = volatilities {
        if vols.len() != count {
            return Err(PyValueError::new_err(
                "volatilities must have the same length as instruments",
            ));
        }
    }

    let equity = money_from(equity)?;
    let risk_pct = from_py_optional_decimal(risk_pct)?
        .unwrap_or_else(|| Decimal::new(1, 2));
    let lot = optional_quantity(lot_size)?.unwrap_or_else(|| Quantity::from_u64(1));
    let caps = make_budget(equity, risk_pct, lot, None, None, None);

    let sizing = match method {
        "fixed_fractional" => SizingMethod::FixedFractional { risk_pct },
        "equal_risk_contribution" => SizingMethod::EqualRiskContribution,
        "volatility_target" => SizingMethod::VolatilityTarget {
            target_annual_vol: risk_pct,
            lookback_bars: 20,
        },
        other => {
            return Err(PyValueError::new_err(format!(
                "batch method must be fixed_fractional, equal_risk_contribution or volatility_target, got {other:?}"
            )));
        }
    };

    let mut requests = Vec::with_capacity(count);
    for index in 0..count {
        let entry = price_from(&entry_prices[index])?;
        let mut request = PositionSizingRequest::new(
            instrument_from(&instruments[index])?,
            PositionSide::Long,
            entry,
            equity,
            caps.clone(),
            sizing.clone(),
        );
        if let Some(ref stops) = stop_losses {
            if !stops[index].is_none() {
                request = request.with_stop_loss(price_from(&stops[index])?);
            }
        }
        if let Some(ref vols) = volatilities {
            if !vols[index].is_none() {
                let volatility = from_py_decimal(&vols[index])?;
                if matches!(&sizing, SizingMethod::VolatilityTarget { .. }) {
                    // ATR-style price distance, as a fraction of the entry.
                    request = request.with_volatility(volatility / entry.value());
                } else {
                    request = request.with_volatility(volatility);
                }
            }
        }
        requests.push(request);
    }

    let results = py
        .allow_threads(move || {
            requests
                .iter()
                .map(calculate_position_size)
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(results
        .into_iter()
        .map(PyPositionSizingResult::from_inner)
        .collect())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Registers every sizing class and function on `honba._honba`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySizingMethod>()?;
    m.add_class::<PyRiskBudget>()?;
    m.add_class::<PyPositionSizingRequest>()?;
    m.add_class::<PyPositionSizingResult>()?;
    m.add_class::<PySignal>()?;
    m.add_class::<PyFixedFractionalSizer>()?;
    m.add_class::<PyAtrVolatilitySizer>()?;
    m.add_class::<PyKellySizer>()?;
    m.add_class::<PyEqualRiskSizer>()?;
    m.add_class::<PyFixedNotionalSizer>()?;
    m.add_function(wrap_pyfunction!(size_batch, m)?)?;
    Ok(())
}
