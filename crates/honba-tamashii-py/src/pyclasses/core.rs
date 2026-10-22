//! Canonical value objects from `honba-core`, exposed to Python (plan.md, Phase 1).
//!
//! Every price, quantity and amount that crosses the FFI boundary is an exact
//! decimal: `rust_decimal::Decimal` in Rust, `decimal.Decimal` in Python.
//! There is no float anywhere in this module, and the constructors refuse one.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use honba_core::{Currency, DepthLevel, Exchange, InstrumentId, Money, Price, PriceBand, Quantity, Tick};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;

use crate::decimal::{from_py_decimal, from_py_optional_decimal, to_py_decimal};

/// The name of a Python type, for error messages.
fn type_name(value: &Bound<'_, PyAny>) -> String {
    value
        .get_type()
        .name()
        .and_then(|name| name.extract::<String>())
        .unwrap_or_else(|_| "?".to_string())
}

/// The error for "not a `Price`" and friends: a `TypeError` naming the operand.
fn operand_error(op: &str, expected: &str, got: &Bound<'_, PyAny>) -> PyErr {
    PyTypeError::new_err(format!(
        "unsupported operand type(s) for {op}: expected {expected}, got {}",
        type_name(got)
    ))
}

/// Extracts a `Price` from a Python operand.
fn as_price(value: &Bound<'_, PyAny>) -> PyResult<Price> {
    if let Ok(price) = value.extract::<PyPrice>() {
        return Ok(price.inner);
    }
    Err(PyTypeError::new_err(format!(
        "expected a Price, got {}",
        type_name(value)
    )))
}

/// Reads a comparison operand: a `Price` or any exact decimal.
fn as_price_operand(value: &Bound<'_, PyAny>) -> PyResult<Price> {
    if let Ok(price) = value.extract::<PyPrice>() {
        return Ok(price.inner);
    }
    Ok(Price::new(from_py_decimal(value)?))
}

/// Reads a comparison operand: a `Quantity` or any exact decimal.
fn as_quantity_operand(value: &Bound<'_, PyAny>) -> PyResult<Quantity> {
    if let Ok(quantity) = value.extract::<PyQuantity>() {
        return Ok(quantity.inner);
    }
    Ok(Quantity::new(from_py_decimal(value)?))
}

/// Extracts a `Quantity` from a Python operand.
fn as_quantity(value: &Bound<'_, PyAny>) -> PyResult<Quantity> {
    if let Ok(quantity) = value.extract::<PyQuantity>() {
        return Ok(quantity.inner);
    }
    Err(PyTypeError::new_err(format!(
        "expected a Quantity, got {}",
        type_name(value)
    )))
}

/// An exact decimal price.
#[pyclass(name = "Price", module = "honba._honba")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PyPrice {
    pub(crate) inner: Price,
}

#[pymethods]
impl PyPrice {
    /// `Price("2540.50")` — a decimal string, an int or a `decimal.Decimal`.
    #[new]
    pub fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Price::new(from_py_decimal(value)?),
        })
    }

    /// The price as a `decimal.Decimal`.
    fn to_decimal<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.inner.0)
    }

    /// The price as its canonical JSON text (a decimal string).
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    /// Whether the price is strictly positive.
    fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Whether the price is aligned to `tick_size`.
    fn is_tick_aligned(&self, tick_size: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_tick_aligned(from_py_decimal(tick_size)?))
    }

    fn __repr__(&self) -> String {
        format!("Price('{}')", self.inner.0)
    }

    fn __str__(&self) -> String {
        self.inner.0.to_string()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.0.hash(&mut hasher);
        hasher.finish()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other) = other.extract::<PyPrice>() {
            return Ok(self.inner == other.inner);
        }
        Ok(false)
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_price_operand(other).map_err(|_| operand_error("<", "Price", other))?;
        Ok(self.inner < other)
    }

    fn __le__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_price_operand(other).map_err(|_| operand_error("<=", "Price", other))?;
        Ok(self.inner <= other)
    }

    fn __gt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_price_operand(other).map_err(|_| operand_error(">", "Price", other))?;
        Ok(self.inner > other)
    }

    fn __ge__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_price_operand(other).map_err(|_| operand_error(">=", "Price", other))?;
        Ok(self.inner >= other)
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other = as_price(other).map_err(|_| operand_error("+", "Price", other))?;
        Ok(Self {
            inner: Price::new(self.inner.0 + other.0),
        })
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other = as_price(other).map_err(|_| operand_error("-", "Price", other))?;
        Ok(Self {
            inner: Price::new(self.inner.0 - other.0),
        })
    }

    /// `price * quantity` is the notional, as `Money`.
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyMoney> {
        let quantity = as_quantity(other).map_err(|_| operand_error("*", "Quantity", other))?;
        Ok(PyMoney {
            inner: Money::notional(self.inner, quantity),
        })
    }

    /// `price / price` is a dimensionless ratio; `price / n` scales the price.
    fn __truediv__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) ->PyResult<Py<PyAny>> {
        if let Ok(other) = as_price(other) {
            let ratio = self.inner.0 / other.0;
            return Ok(to_py_decimal(py, ratio)?.unbind());
        }
        if let Ok(divisor) = from_py_decimal(other) {
            if divisor.is_zero() {
                return Err(PyValueError::new_err("division by zero"));
            }
            let scaled = Py::new(
                py,
                PyPrice {
                    inner: Price::new(self.inner.0 / divisor),
                },
            )?;
            return Ok(scaled.into_any());
        }
        Err(operand_error("/", "Price or a decimal", other))
    }

    fn __neg__(&self) -> Self {
        Self {
            inner: Price::new(-self.inner.0),
        }
    }
}

/// An exact decimal quantity.
#[pyclass(name = "Quantity", module = "honba._honba")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PyQuantity {
    pub(crate) inner: Quantity,
}

#[pymethods]
impl PyQuantity {
    /// `Quantity(10)`, `Quantity("10.5")` or `Quantity(Decimal("10.5"))`.
    #[new]
    pub fn new(value: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: Quantity::new(from_py_decimal(value)?),
        })
    }

    /// The quantity as a `decimal.Decimal`.
    fn to_decimal<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.inner.0)
    }

    /// The quantity as its canonical JSON text (a decimal string).
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    /// Whether the quantity is strictly positive.
    fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Whether the quantity is a whole multiple of `lot_size`.
    fn is_lot_multiple(&self, lot_size: u64) -> bool {
        self.inner.is_lot_multiple(lot_size)
    }

    fn __repr__(&self) -> String {
        format!("Quantity('{}')", self.inner.0)
    }

    fn __str__(&self) -> String {
        self.inner.0.to_string()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.0.hash(&mut hasher);
        hasher.finish()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other) = other.extract::<PyQuantity>() {
            return Ok(self.inner == other.inner);
        }
        Ok(false)
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_quantity_operand(other).map_err(|_| operand_error("<", "Quantity", other))?;
        Ok(self.inner < other)
    }

    fn __le__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_quantity_operand(other).map_err(|_| operand_error("<=", "Quantity", other))?;
        Ok(self.inner <= other)
    }

    fn __gt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_quantity_operand(other).map_err(|_| operand_error(">", "Quantity", other))?;
        Ok(self.inner > other)
    }

    fn __ge__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let other = as_quantity_operand(other).map_err(|_| operand_error(">=", "Quantity", other))?;
        Ok(self.inner >= other)
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other = as_quantity(other).map_err(|_| operand_error("+", "Quantity", other))?;
        Ok(Self {
            inner: Quantity::new(self.inner.0 + other.0),
        })
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other = as_quantity(other).map_err(|_| operand_error("-", "Quantity", other))?;
        Ok(Self {
            inner: Quantity::new(self.inner.0 - other.0),
        })
    }

    /// `quantity * price` is the notional, as `Money`.
    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<PyMoney> {
        let price = as_price(other).map_err(|_| operand_error("*", "Price", other))?;
        Ok(PyMoney {
            inner: Money::notional(price, self.inner),
        })
    }

    fn __truediv__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) ->PyResult<Py<PyAny>> {
        if let Ok(other) = as_quantity(other) {
            if other.0.is_zero() {
                return Err(PyValueError::new_err("division by zero"));
            }
            return Ok(to_py_decimal(py, self.inner.0 / other.0)?.unbind());
        }
        if let Ok(divisor) = from_py_decimal(other) {
            if divisor.is_zero() {
                return Err(PyValueError::new_err("division by zero"));
            }
            let scaled = Py::new(
                py,
                PyQuantity {
                    inner: Quantity::new(self.inner.0 / divisor),
                },
            )?;
            return Ok(scaled.into_any());
        }
        Err(operand_error("/", "Quantity or a decimal", other))
    }

    fn __neg__(&self) -> Self {
        Self {
            inner: Quantity::new(-self.inner.0),
        }
    }
}

/// An exact monetary amount in a currency.
#[pyclass(name = "Money", module = "honba._honba")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyMoney {
    pub(crate) inner: Money,
}

#[pymethods]
impl PyMoney {
    /// `Money("1234.50", "INR")` — the amount and the currency code.
    #[new]
    #[pyo3(signature = (amount, currency="INR"))]
    pub fn new(amount: &Bound<'_, PyAny>, currency: &str) -> PyResult<Self> {
        let currency = Currency::from_code(currency)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Money::new(from_py_decimal(amount)?, currency),
        })
    }

    /// The amount as a `decimal.Decimal`.
    fn to_decimal<'py>(&self, py: Python<'py>) ->PyResult<Bound<'py, PyAny>> {
        to_py_decimal(py, self.inner.amount)
    }

    /// The currency code (`INR`).
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.code().to_string()
    }

    /// The amount in the currency's major unit.
    fn to_major<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // The canonical amount *is* a major-unit decimal, so this is the amount
        // itself; `to_minor` is the integer minor-unit form.
        to_py_decimal(py, self.inner.amount)
    }

    /// The amount in the currency's minor unit (`1234.50` INR is `123450` paise).
    fn to_minor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let exponent = self.inner.currency.minor_exponent() as u32;
        let scale = Decimal::from(10u64.pow(exponent));
        to_py_decimal(py, self.inner.amount * scale)
    }

    /// The amount as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("Money('{}', '{}')", self.inner.amount, self.inner.currency.code())
    }

    fn __str__(&self) -> String {
        format!("{} {}", self.inner.amount, self.inner.currency.code())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other) = other.extract::<PyMoney>() {
            return Ok(self.inner == other.inner);
        }
        Ok(false)
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other = other
            .extract::<PyMoney>()
            .map_err(|_| PyTypeError::new_err("Money + Money expects a Money"))?;
        if other.inner.currency != self.inner.currency {
            return Err(PyValueError::new_err(format!(
                "currency mismatch: {} vs {}",
                self.inner.currency.code(),
                other.inner.currency.code()
            )));
        }
        Ok(Self {
            inner: Money::new(self.inner.amount + other.inner.amount, self.inner.currency),
        })
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let other = other
            .extract::<PyMoney>()
            .map_err(|_| PyTypeError::new_err("Money - Money expects a Money"))?;
        if other.inner.currency != self.inner.currency {
            return Err(PyValueError::new_err(format!(
                "currency mismatch: {} vs {}",
                self.inner.currency.code(),
                other.inner.currency.code()
            )));
        }
        Ok(Self {
            inner: Money::new(self.inner.amount - other.inner.amount, self.inner.currency),
        })
    }

    fn __neg__(&self) -> Self {
        Self {
            inner: Money::new(-self.inner.amount, self.inner.currency),
        }
    }
}

/// A canonical instrument identifier (`NSE:RELIANCE`).
#[pyclass(name = "InstrumentId", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PyInstrumentId {
    pub(crate) inner: InstrumentId,
}

#[pymethods]
impl PyInstrumentId {
    /// `InstrumentId("RELIANCE", "NSE")` — the symbol is upper-cased.
    #[new]
    #[pyo3(signature = (symbol, exchange="NSE", token=None, isin=None))]
    pub fn new(
        symbol: &str,
        exchange: &str,
        token: Option<String>,
        isin: Option<String>,
    ) -> PyResult<Self> {
        let exchange = Exchange::from_str(exchange)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let mut inner = InstrumentId::new(exchange, symbol);
        if let Some(token) = token {
            inner = inner.with_token(token);
        }
        if let Some(isin) = isin {
            inner = inner.with_isin(isin);
        }
        Ok(Self { inner })
    }

    /// Parses `"NSE:RELIANCE"`.
    #[staticmethod]
    pub fn from_string(text: &str) -> PyResult<Self> {
        let (exchange, symbol) = match text.split_once(':') {
            Some((exchange, symbol)) => (exchange, symbol),
            None => ("NSE", text),
        };
        Self::new(symbol, exchange, None, None)
    }

    /// The trading symbol.
    #[getter]
    fn symbol(&self) -> String {
        self.inner.symbol.clone()
    }

    /// The exchange code (`NSE`, `BSE`, `MCX`).
    #[getter]
    fn exchange(&self) -> String {
        self.inner.exchange.to_string()
    }

    /// The broker instrument token, if any.
    #[getter]
    fn token(&self) -> Option<String> {
        self.inner.token.clone()
    }

    /// The ISIN, if any.
    #[getter]
    fn isin(&self) -> Option<String> {
        self.inner.isin.clone()
    }

    /// The identifier as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("InstrumentId('{}')", self.inner)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.hash(&mut hasher);
        hasher.finish()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other) = other.extract::<PyInstrumentId>() {
            return Ok(self.inner == other.inner);
        }
        Ok(false)
    }
}

/// A scrip-wise circuit band.
#[pyclass(name = "PriceBand", module = "honba._honba")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyPriceBand {
    pub(crate) inner: PriceBand,
}

#[pymethods]
impl PyPriceBand {
    /// `PriceBand(lower, upper, reference_price, band_pct=None)`.
    #[new]
    #[pyo3(signature = (lower, upper, reference_price, band_pct=None))]
    pub fn new(
        lower: &Bound<'_, PyAny>,
        upper: &Bound<'_, PyAny>,
        reference_price: &Bound<'_, PyAny>,
        band_pct: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mut inner = PriceBand::new(
            Price::new(from_py_decimal(lower)?),
            Price::new(from_py_decimal(upper)?),
            Price::new(from_py_decimal(reference_price)?),
        );
        inner.band_pct = from_py_optional_decimal(band_pct)?;
        Ok(Self { inner })
    }

    /// The lower circuit.
    #[getter]
    fn lower(&self) -> PyPrice {
        PyPrice { inner: self.inner.lower }
    }

    /// The upper circuit.
    #[getter]
    fn upper(&self) -> PyPrice {
        PyPrice { inner: self.inner.upper }
    }

    /// The reference price the band is centred on.
    #[getter]
    fn reference_price(&self) -> PyPrice {
        PyPrice {
            inner: self.inner.reference_price,
        }
    }

    /// The band width as a fraction (`0.05` is 5%), if known.
    #[getter]
    fn band_pct<'py>(&self, py: Python<'py>) ->PyResult<Option<Bound<'py, PyAny>>> {
        match self.inner.band_pct {
            Some(pct) => Ok(Some(to_py_decimal(py, pct)?)),
            None => Ok(None),
        }
    }

    /// Whether `price` is inside the inclusive band.
    fn contains(&self, price: &Bound<'_, PyAny>) -> PyResult<bool> {
        let price = as_price(price)?;
        Ok(self.inner.contains(price))
    }

    /// The band as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "PriceBand('{}', '{}')",
            self.inner.lower.0, self.inner.upper.0
        )
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(other) = other.extract::<PyPriceBand>() {
            return Ok(self.inner == other.inner);
        }
        Ok(false)
    }
}

/// One level of the order book.
#[pyclass(name = "DepthLevel", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyDepthLevel {
    pub(crate) inner: DepthLevel,
}

#[pymethods]
impl PyDepthLevel {
    /// `DepthLevel(price, quantity, orders_count=None)`.
    #[new]
    #[pyo3(signature = (price, quantity, orders_count=None))]
    pub fn new(
        price: &Bound<'_, PyAny>,
        quantity: &Bound<'_, PyAny>,
        orders_count: Option<u32>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: DepthLevel {
                price: Price::new(from_py_decimal(price)?),
                quantity: Quantity::new(from_py_decimal(quantity)?),
                orders_count,
            },
        })
    }

    /// The price of this level.
    #[getter]
    fn price(&self) -> PyPrice {
        PyPrice {
            inner: self.inner.price,
        }
    }

    /// The size resting at this level.
    #[getter]
    fn quantity(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.quantity,
        }
    }

    /// The number of orders at this level, if known.
    #[getter]
    fn orders_count(&self) -> Option<u32> {
        self.inner.orders_count
    }

    fn __repr__(&self) -> String {
        format!(
            "DepthLevel('{}', '{}')",
            self.inner.price.0, self.inner.quantity.0
        )
    }
}

/// One OHLCV bar.
#[pyclass(name = "Bar", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyBar {
    pub(crate) inner: honba_core::Bar,
}

#[pymethods]
impl PyBar {
    /// `Bar(instrument, open, high, low, close, volume, timestamp=None)`.
    #[new]
    #[pyo3(signature = (instrument, open, high, low, close, volume, timestamp=None))]
    pub fn new(
        instrument: &PyInstrumentId,
        open: &Bound<'_, PyAny>,
        high: &Bound<'_, PyAny>,
        low: &Bound<'_, PyAny>,
        close: &Bound<'_, PyAny>,
        volume: &Bound<'_, PyAny>,
        timestamp: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let timestamp = crate::decimal::from_py_optional_timestamp(timestamp)?
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch is a timestamp"));
        Ok(Self {
            inner: honba_core::Bar {
                instrument: instrument.inner.clone(),
                open: Price::new(from_py_decimal(open)?),
                high: Price::new(from_py_decimal(high)?),
                low: Price::new(from_py_decimal(low)?),
                close: Price::new(from_py_decimal(close)?),
                volume: Quantity::new(from_py_decimal(volume)?),
                timestamp,
            },
        })
    }

    /// The instrument the bar belongs to.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument.clone(),
        }
    }

    /// The open price.
    #[getter]
    fn open(&self) -> PyPrice {
        PyPrice { inner: self.inner.open }
    }

    /// The high price.
    #[getter]
    fn high(&self) -> PyPrice {
        PyPrice { inner: self.inner.high }
    }

    /// The low price.
    #[getter]
    fn low(&self) -> PyPrice {
        PyPrice { inner: self.inner.low }
    }

    /// The close price.
    #[getter]
    fn close(&self) -> PyPrice {
        PyPrice { inner: self.inner.close }
    }

    /// The traded volume.
    #[getter]
    fn volume(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.volume,
        }
    }

    /// The bar's timestamp.
    #[getter]
    fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.inner.timestamp
    }

    /// The bar as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "Bar('{}', close='{}')",
            self.inner.instrument, self.inner.close.0
        )
    }
}

/// A market-data tick: last price, quote or depth.
#[pyclass(name = "Tick", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyTick {
    pub(crate) inner: Tick,
}

#[pymethods]
impl PyTick {
    /// Parses a canonical tick JSON.
    #[staticmethod]
    pub fn from_json(payload: &str) -> PyResult<Self> {
        let inner: Tick = serde_json::from_str(payload)
            .map_err(|e| PyValueError::new_err(format!("invalid tick: {e}")))?;
        Ok(Self { inner })
    }

    /// The tick kind: `ltp`, `quote` or `depth`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            Tick::Ltp { .. } => "ltp",
            Tick::Quote { .. } => "quote",
            Tick::Depth { .. } => "depth",
        }
    }

    /// The instrument the tick belongs to.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument().clone(),
        }
    }

    /// The last traded price, if the tick carries one.
    #[getter]
    fn last_price(&self) -> Option<PyPrice> {
        self.inner.last_price().map(|price| PyPrice { inner: price })
    }

    /// The best bid, for a quote tick.
    #[getter]
    fn bid(&self) -> Option<PyPrice> {
        match &self.inner {
            Tick::Quote { bid, .. } => Some(PyPrice { inner: *bid }),
            _ => None,
        }
    }

    /// The best ask, for a quote tick.
    #[getter]
    fn ask(&self) -> Option<PyPrice> {
        match &self.inner {
            Tick::Quote { ask, .. } => Some(PyPrice { inner: *ask }),
            _ => None,
        }
    }

    /// The traded volume, for a last-price tick.
    #[getter]
    fn volume(&self) -> Option<PyQuantity> {
        match &self.inner {
            Tick::Ltp { volume, .. } => Some(PyQuantity { inner: *volume }),
            _ => None,
        }
    }

    /// The exchange timestamp.
    #[getter]
    fn exchange_ts(&self) -> chrono::DateTime<chrono::Utc> {
        match &self.inner {
            Tick::Ltp { exchange_ts, .. }
            | Tick::Quote { exchange_ts, .. }
            | Tick::Depth { exchange_ts, .. } => *exchange_ts,
        }
    }

    /// The receive timestamp.
    #[getter]
    fn receive_ts(&self) -> chrono::DateTime<chrono::Utc> {
        self.inner.receive_ts()
    }

    /// The circuit band, for a quote tick.
    #[getter]
    fn band(&self) -> Option<PyPriceBand> {
        match &self.inner {
            Tick::Quote { band, .. } => band.map(|band| PyPriceBand { inner: band }),
            _ => None,
        }
    }

    /// The tick as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "Tick('{}', kind='{}')",
            self.inner.instrument(),
            self.kind()
        )
    }
}

/// The display names of a currency's minor unit.
#[pyclass(name = "MinorUnit", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyMinorUnit {
    singular: String,
    plural: String,
}

#[pymethods]
impl PyMinorUnit {
    /// `MinorUnit("paisa", "paise")`.
    #[new]
    pub fn new(singular: &str, plural: &str) -> Self {
        Self {
            singular: singular.to_string(),
            plural: plural.to_string(),
        }
    }

    /// The minor unit of a currency, straight from the kernel's table.
    #[staticmethod]
    pub fn of_currency(currency: &str) -> PyResult<Self> {
        let currency = Currency::from_code(currency)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let unit = currency.minor_unit();
        Ok(Self {
            singular: unit.singular.to_string(),
            plural: unit.plural.to_string(),
        })
    }

    /// The singular name (`paisa`).
    #[getter]
    fn singular(&self) -> String {
        self.singular.clone()
    }

    /// The plural name (`paise`).
    #[getter]
    fn plural(&self) -> String {
        self.plural.clone()
    }

    fn __repr__(&self) -> String {
        format!("MinorUnit('{}', '{}')", self.singular, self.plural)
    }
}

/// The currency minor-unit table, read from `honba-core`.
#[pyfunction]
#[pyo3(name = "currency_minor_units")]
pub fn py_currency_minor_units() -> BTreeMap<String, (u8, &'static str, &'static str)> {
    Currency::ALL
        .iter()
        .map(|currency| {
            let unit = currency.minor_unit();
            (
                currency.code().to_owned(),
                (currency.minor_exponent(), unit.singular, unit.plural),
            )
        })
        .collect()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPrice>()?;
    m.add_class::<PyQuantity>()?;
    m.add_class::<PyMoney>()?;
    m.add_class::<PyInstrumentId>()?;
    m.add_class::<PyPriceBand>()?;
    m.add_class::<PyDepthLevel>()?;
    m.add_class::<PyBar>()?;
    m.add_class::<PyTick>()?;
    m.add_class::<PyMinorUnit>()?;
    m.add_function(wrap_pyfunction!(py_currency_minor_units, m)?)?;
    Ok(())
}
