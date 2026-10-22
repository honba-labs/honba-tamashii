//! Canonical order contracts from `honba-core`: request, acknowledgement, fill.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use honba_core::{FillReport, OrderAck, OrderRequest, OrderSide, OrderStatus, OrderType, TimeInForce};
use honba_core::uuid::Uuid;

use crate::decimal::{from_py_decimal, from_py_optional_decimal, from_py_optional_timestamp};
use crate::pyclasses::core::{PyInstrumentId, PyPrice, PyQuantity};

/// Buy or sell.
#[pyclass(name = "OrderSide", module = "honba._honba", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyOrderSide {
    /// Buy.
    #[pyo3(name = "BUY")]
    Buy,
    /// Sell.
    #[pyo3(name = "SELL")]
    Sell,
}

#[pymethods]
impl PyOrderSide {
    fn __str__(&self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

/// The order type.
#[pyclass(name = "OrderType", module = "honba._honba", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyOrderType {
    /// A market order.
    #[pyo3(name = "MARKET")]
    Market,
    /// A limit order.
    #[pyo3(name = "LIMIT")]
    Limit,
    /// A stop-loss (stop-market) order.
    #[pyo3(name = "STOP_LOSS")]
    StopLoss,
    /// A stop-limit order.
    #[pyo3(name = "STOP_LIMIT")]
    StopLimit,
}

#[pymethods]
impl PyOrderType {
    fn __str__(&self) -> &'static str {
        match self {
            Self::Market => "MARKET",
            Self::Limit => "LIMIT",
            Self::StopLoss => "STOP_LOSS",
            Self::StopLimit => "STOP_LIMIT",
        }
    }
}

/// The order's time in force.
#[pyclass(name = "TimeInForce", module = "honba._honba", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyTimeInForce {
    /// Good for the day.
    #[pyo3(name = "DAY")]
    Day,
    /// Immediate or cancel.
    #[pyo3(name = "IOC")]
    Ioc,
    /// Good till cancelled.
    #[pyo3(name = "GTC")]
    Gtc,
}

#[pymethods]
impl PyTimeInForce {
    fn __str__(&self) -> &'static str {
        match self {
            Self::Day => "DAY",
            Self::Ioc => "IOC",
            Self::Gtc => "GTC",
        }
    }
}

/// The lifecycle state of an order.
#[pyclass(name = "OrderStatus", module = "honba._honba", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyOrderStatus {
    /// Accepted by the OMS, not yet sent.
    #[pyo3(name = "PENDING_NEW")]
    PendingNew,
    /// Live at the exchange.
    #[pyo3(name = "NEW")]
    New,
    /// Partially filled.
    #[pyo3(name = "PARTIALLY_FILLED")]
    PartiallyFilled,
    /// Fully filled.
    #[pyo3(name = "FILLED")]
    Filled,
    /// Cancelled.
    #[pyo3(name = "CANCELLED")]
    Cancelled,
    /// Rejected by the exchange or the risk gate.
    #[pyo3(name = "REJECTED")]
    Rejected,
    /// Expired at the end of the session.
    #[pyo3(name = "EXPIRED")]
    Expired,
}

#[pymethods]
impl PyOrderStatus {
    /// Whether the order is in a final state.
    fn is_terminal(&self) -> bool {
        match self {
            Self::PendingNew | Self::New | Self::PartiallyFilled => false,
            Self::Filled | Self::Cancelled | Self::Rejected | Self::Expired => true,
        }
    }

    fn __str__(&self) -> &'static str {
        match self {
            Self::PendingNew => "pending_new",
            Self::New => "new",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }
}

/// An order to be sent to the exchange.
#[pyclass(name = "OrderRequest", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyOrderRequest {
    pub(crate) inner: OrderRequest,
}

#[pymethods]
impl PyOrderRequest {
    /// Builds an order request.
    ///
    /// `algo_id` is mandatory (SEBI Retail Algo circular); an order without one
    /// is refused by the risk gate.
    #[new]
    #[pyo3(signature = (instrument, side, quantity, order_type="market", price=None, trigger_price=None, limit_price=None, tif="day", algo_id="", tag=None, client_order_id=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instrument: &PyInstrumentId,
        side: PyOrderSide,
        quantity: &Bound<'_, PyAny>,
        order_type: &str,
        price: Option<&Bound<'_, PyAny>>,
        trigger_price: Option<&Bound<'_, PyAny>>,
        limit_price: Option<&Bound<'_, PyAny>>,
        tif: &str,
        algo_id: &str,
        tag: Option<String>,
        client_order_id: Option<String>,
    ) -> PyResult<Self> {
        let order_type = parse_order_type(order_type, price, trigger_price, limit_price)?;
        let tif = match tif.to_uppercase().as_str() {
            "DAY" => TimeInForce::Day,
            "IOC" => TimeInForce::Ioc,
            "GTC" => TimeInForce::Gtc,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown time in force {other:?}; expected DAY, IOC or GTC"
                )))
            }
        };
        let quantity = honba_core::Quantity::new(from_py_decimal(quantity)?);
        let mut inner = OrderRequest::new(
            instrument.inner.clone(),
            match side {
                PyOrderSide::Buy => OrderSide::Buy,
                PyOrderSide::Sell => OrderSide::Sell,
            },
            order_type,
            quantity,
            algo_id,
        );
        inner.tif = tif;
        if let Some(tag) = tag {
            inner.tag = Some(tag);
        }
        if let Some(client_order_id) = client_order_id {
            let parsed = Uuid::parse_str(&client_order_id).map_err(|e| {
                PyValueError::new_err(format!("client_order_id must be a UUID: {e}"))
            })?;
            inner.client_order_id = parsed;
            inner.correlation_id = Some(parsed);
        }
        Ok(Self { inner })
    }

    /// The instrument the order is for.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument.clone(),
        }
    }

    /// The order side.
    #[getter]
    fn side(&self) -> PyOrderSide {
        match self.inner.side {
            OrderSide::Buy => PyOrderSide::Buy,
            OrderSide::Sell => PyOrderSide::Sell,
        }
    }

    /// The order type.
    #[getter]
    fn order_type(&self) -> PyOrderType {
        match &self.inner.order_type {
            OrderType::Market => PyOrderType::Market,
            OrderType::Limit(_) => PyOrderType::Limit,
            OrderType::StopLoss(_) => PyOrderType::StopLoss,
            OrderType::StopLimit { .. } => PyOrderType::StopLimit,
        }
    }

    /// The order quantity.
    #[getter]
    fn quantity(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.quantity,
        }
    }

    /// The limit price, for a limit or stop-limit order.
    #[getter]
    fn price(&self) -> Option<PyPrice> {
        self.inner
            .order_type
            .limit_price()
            .map(|price| PyPrice { inner: price })
    }

    /// The trigger price, for a stop order.
    #[getter]
    fn trigger_price(&self) -> Option<PyPrice> {
        self.inner
            .order_type
            .trigger_price()
            .map(|price| PyPrice { inner: price })
    }

    /// The time in force.
    #[getter]
    fn tif(&self) -> PyTimeInForce {
        match self.inner.tif {
            TimeInForce::Day => PyTimeInForce::Day,
            TimeInForce::Ioc => PyTimeInForce::Ioc,
            TimeInForce::Gtc => PyTimeInForce::Gtc,
        }
    }

    /// The SEBI algo id.
    #[getter]
    fn algo_id(&self) -> String {
        self.inner.algo_id.clone()
    }

    /// The client order id.
    #[getter]
    fn client_order_id(&self) -> String {
        self.inner.client_order_id.to_string()
    }

    /// The request as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "OrderRequest('{}', {:?}, '{}')",
            self.inner.instrument, self.inner.side, self.inner.quantity.0
        )
    }
}

/// The exchange's acknowledgement of an order.
#[pyclass(name = "OrderAck", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyOrderAck {
    pub(crate) inner: OrderAck,
}

#[pymethods]
impl PyOrderAck {
    /// Builds an acknowledgement.
    #[new]
    #[pyo3(signature = (order_id, client_order_id, status, timestamp=None))]
    pub fn new(
        order_id: &str,
        client_order_id: &str,
        status: PyOrderStatus,
        timestamp: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let client_order_id = Uuid::parse_str(client_order_id)
            .map_err(|e| PyValueError::new_err(format!("client_order_id must be a UUID: {e}")))?;
        let timestamp = from_py_optional_timestamp(timestamp)?
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch is a timestamp"));
        Ok(Self {
            inner: OrderAck::new(order_id, client_order_id, status.into(), timestamp),
        })
    }

    /// The broker order id.
    #[getter]
    fn order_id(&self) -> String {
        self.inner.order_id.clone()
    }

    /// The client order id.
    #[getter]
    fn client_order_id(&self) -> String {
        self.inner.client_order_id.to_string()
    }

    /// The order status.
    #[getter]
    fn status(&self) -> PyOrderStatus {
        self.inner.status.into()
    }

    /// The acknowledgement timestamp.
    #[getter]
    fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.inner.timestamp
    }

    /// The acknowledgement as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!("OrderAck('{}', {:?})", self.inner.order_id, self.inner.status)
    }
}

/// A trade execution.
#[pyclass(name = "FillReport", module = "honba._honba")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyFillReport {
    pub(crate) inner: FillReport,
}

#[pymethods]
impl PyFillReport {
    /// Builds a fill report.
    #[new]
    #[pyo3(signature = (fill_id, order_id, client_order_id, instrument, side, price, quantity, algo_id, timestamp=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fill_id: &str,
        order_id: &str,
        client_order_id: &str,
        instrument: &PyInstrumentId,
        side: PyOrderSide,
        price: &Bound<'_, PyAny>,
        quantity: &Bound<'_, PyAny>,
        algo_id: &str,
        timestamp: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let client_order_id = Uuid::parse_str(client_order_id)
            .map_err(|e| PyValueError::new_err(format!("client_order_id must be a UUID: {e}")))?;
        let timestamp = from_py_optional_timestamp(timestamp)?
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch is a timestamp"));
        let inner = FillReport::new(
            fill_id,
            order_id,
            client_order_id,
            instrument.inner.clone(),
            match side {
                PyOrderSide::Buy => OrderSide::Buy,
                PyOrderSide::Sell => OrderSide::Sell,
            },
            honba_core::Price::new(from_py_decimal(price)?),
            honba_core::Quantity::new(from_py_decimal(quantity)?),
            algo_id,
            timestamp,
        );
        Ok(Self { inner })
    }

    /// The fill id.
    #[getter]
    fn fill_id(&self) -> String {
        self.inner.fill_id.clone()
    }

    /// The order id this fill belongs to.
    #[getter]
    fn order_id(&self) -> String {
        self.inner.order_id.clone()
    }

    /// The instrument that traded.
    #[getter]
    fn instrument(&self) -> PyInstrumentId {
        PyInstrumentId {
            inner: self.inner.instrument.clone(),
        }
    }

    /// The side of the fill.
    #[getter]
    fn side(&self) -> PyOrderSide {
        match self.inner.side {
            OrderSide::Buy => PyOrderSide::Buy,
            OrderSide::Sell => PyOrderSide::Sell,
        }
    }

    /// The execution price.
    #[getter]
    fn price(&self) -> PyPrice {
        PyPrice {
            inner: self.inner.price,
        }
    }

    /// The executed quantity.
    #[getter]
    fn quantity(&self) -> PyQuantity {
        PyQuantity {
            inner: self.inner.quantity,
        }
    }

    /// The SEBI algo id.
    #[getter]
    fn algo_id(&self) -> String {
        self.inner.algo_id.clone()
    }

    /// The fill timestamp.
    #[getter]
    fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.inner.timestamp
    }

    /// The fill as its canonical JSON text.
    fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_default()
    }

    fn __repr__(&self) -> String {
        format!(
            "FillReport('{}', {:?}, '{}')",
            self.inner.fill_id, self.inner.side, self.inner.quantity.0
        )
    }
}

/// Parses an order type with its prices.
fn parse_order_type(
    order_type: &str,
    price: Option<&Bound<'_, PyAny>>,
    trigger_price: Option<&Bound<'_, PyAny>>,
    limit_price: Option<&Bound<'_, PyAny>>,
) -> PyResult<OrderType> {
    let price = from_py_optional_decimal(price)?;
    let trigger_price = from_py_optional_decimal(trigger_price)?;
    let limit_price = from_py_optional_decimal(limit_price)?;
    match order_type.to_lowercase().as_str() {
        "market" => Ok(OrderType::Market),
        "limit" => {
            let limit = limit_price
                .or(price)
                .ok_or_else(|| PyValueError::new_err("a limit order needs a price"))?;
            Ok(OrderType::Limit(honba_core::Price::new(limit)))
        }
        "stop" | "stop_loss" | "stop_market" => {
            let trigger = trigger_price
                .or(price)
                .ok_or_else(|| PyValueError::new_err("a stop order needs a trigger price"))?;
            Ok(OrderType::StopLoss(honba_core::Price::new(trigger)))
        }
        "stop_limit" => {
            let trigger = trigger_price
                .ok_or_else(|| PyValueError::new_err("a stop-limit order needs a trigger price"))?;
            let limit = limit_price
                .or(price)
                .ok_or_else(|| PyValueError::new_err("a stop-limit order needs a limit price"))?;
            Ok(OrderType::StopLimit {
                trigger: honba_core::Price::new(trigger),
                limit: honba_core::Price::new(limit),
            })
        }
        other => Err(PyValueError::new_err(format!(
            "unknown order type {other:?}; expected market, limit, stop_loss or stop_limit"
        ))),
    }
}

impl From<OrderStatus> for PyOrderStatus {
    fn from(status: OrderStatus) -> Self {
        match status {
            OrderStatus::PendingNew => PyOrderStatus::PendingNew,
            OrderStatus::New => PyOrderStatus::New,
            OrderStatus::PartiallyFilled => PyOrderStatus::PartiallyFilled,
            OrderStatus::Filled => PyOrderStatus::Filled,
            OrderStatus::Cancelled => PyOrderStatus::Cancelled,
            OrderStatus::Rejected => PyOrderStatus::Rejected,
            OrderStatus::Expired => PyOrderStatus::Expired,
        }
    }
}

impl From<PyOrderStatus> for OrderStatus {
    fn from(status: PyOrderStatus) -> Self {
        match status {
            PyOrderStatus::PendingNew => OrderStatus::PendingNew,
            PyOrderStatus::New => OrderStatus::New,
            PyOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            PyOrderStatus::Filled => OrderStatus::Filled,
            PyOrderStatus::Cancelled => OrderStatus::Cancelled,
            PyOrderStatus::Rejected => OrderStatus::Rejected,
            PyOrderStatus::Expired => OrderStatus::Expired,
        }
    }
}

/// Reads an order request out of a Python object.
///
/// The risk gate takes either a typed `OrderRequest` or a mapping with the
/// same keys, so a caller can hand over plain JSON.
pub fn order_request_from(value: &Bound<'_, PyAny>) -> PyResult<OrderRequest> {
    if let Ok(request) = value.extract::<PyOrderRequest>() {
        return Ok(request.inner);
    }
    Err(PyTypeError::new_err(format!(
        "expected an OrderRequest, got {}",
        value
            .get_type()
            .name()
            .and_then(|name| name.extract::<String>())
            .unwrap_or_else(|_| "?".to_string())
    )))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyOrderSide>()?;
    m.add_class::<PyOrderType>()?;
    m.add_class::<PyTimeInForce>()?;
    m.add_class::<PyOrderStatus>()?;
    m.add_class::<PyOrderRequest>()?;
    m.add_class::<PyOrderAck>()?;
    m.add_class::<PyFillReport>()?;
    Ok(())
}
