//! The JSON wire contract exposed to Python (ADR 006).
//!
//! `canonical_json` parses a payload with the canonical Rust types and
//! serializes it back, so Python can check that its models agree with Rust
//! exactly. The types are the ones in `honba-core`, the shared kernel.

use std::collections::BTreeMap;

use honba_core::{
    Bar, CancelRequest, Decision, DepthLevel, Exchange, FillReport, InstrumentId, ModifyRequest,
    OrderAck, OrderRequest, OrderSide, OrderStatus, OrderType, Tick, TimeInForce,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Wire-contract type names accepted by [`canonical_json`].
pub const KINDS: [&str; 13] = [
    "InstrumentId",
    "Bar",
    "Tick",
    "DepthLevel",
    "PriceBand",
    "OrderRequest",
    "OrderAck",
    "FillReport",
    "CancelRequest",
    "ModifyRequest",
    "Decision",
    "OrderSide",
    "OrderType",
];

/// Wire enum names accepted by [`canonical`] and listed by [`enum_values`].
pub const ENUM_KINDS: [&str; 6] = [
    "OrderSide",
    "OrderType",
    "OrderStatus",
    "TimeInForce",
    "Exchange",
    "Currency",
];

fn wire_strings<T: Serialize>(value: &T) -> Vec<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => vec![s],
        Ok(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// Every variant of every wire enum, as its JSON string, keyed by enum name.
pub fn enum_values() -> BTreeMap<&'static str, Vec<String>> {
    BTreeMap::from([
        ("OrderSide", wire_strings(&[OrderSide::Buy, OrderSide::Sell])),
        (
            "OrderType",
            wire_strings(&[
                OrderType::Market,
                OrderType::Limit(honba_core::Price::ZERO),
                OrderType::StopLoss(honba_core::Price::ZERO),
            ]),
        ),
        (
            "OrderStatus",
            wire_strings(&[
                OrderStatus::PendingNew,
                OrderStatus::New,
                OrderStatus::PartiallyFilled,
                OrderStatus::Filled,
                OrderStatus::Cancelled,
                OrderStatus::Rejected,
                OrderStatus::Expired,
            ]),
        ),
        (
            "TimeInForce",
            wire_strings(&[TimeInForce::Day, TimeInForce::Ioc, TimeInForce::Gtc]),
        ),
        (
            "Exchange",
            wire_strings(&[Exchange::Nse, Exchange::Bse, Exchange::Mcx]),
        ),
        (
            "Currency",
            wire_strings(&[
                honba_core::Currency::Inr,
                honba_core::Currency::Usd,
                honba_core::Currency::Eur,
                honba_core::Currency::Gbp,
            ]),
        ),
    ])
}

fn reserialize<T: Serialize + DeserializeOwned>(payload: &str) -> Result<String, String> {
    let value: T = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

/// Parses `payload` as the Rust type named `kind` and returns its canonical JSON.
pub fn canonical(kind: &str, payload: &str) -> Result<String, String> {
    match kind {
        "InstrumentId" => reserialize::<InstrumentId>(payload),
        "Bar" => reserialize::<Bar>(payload),
        "Tick" => reserialize::<Tick>(payload),
        "DepthLevel" => reserialize::<DepthLevel>(payload),
        "PriceBand" => reserialize::<honba_core::PriceBand>(payload),
        "OrderRequest" => reserialize::<OrderRequest>(payload),
        "OrderAck" => reserialize::<OrderAck>(payload),
        "FillReport" => reserialize::<FillReport>(payload),
        "CancelRequest" => reserialize::<CancelRequest>(payload),
        "ModifyRequest" => reserialize::<ModifyRequest>(payload),
        "Decision" => reserialize::<Decision>(payload),
        "OrderSide" => reserialize::<OrderSide>(payload),
        "OrderType" => reserialize::<OrderType>(payload),
        "OrderStatus" => reserialize::<OrderStatus>(payload),
        "TimeInForce" => reserialize::<TimeInForce>(payload),
        "Exchange" => reserialize::<Exchange>(payload),
        "Currency" => reserialize::<honba_core::Currency>(payload),
        other => Err(format!(
            "unknown kind '{other}'; expected one of {KINDS:?} or {ENUM_KINDS:?}"
        )),
    }
}

/// Parse a JSON payload with the Rust type `kind` and return its canonical JSON.
///
/// `kind` is a wire type (`KINDS`) or a wire enum (`ENUM_KINDS`).
///
/// Raises `ValueError` for an unknown kind or a payload Rust rejects.
#[pyfunction]
pub fn canonical_json(kind: &str, payload: &str) -> PyResult<String> {
    canonical(kind, payload).map_err(PyValueError::new_err)
}

/// Every variant of every wire enum, as its JSON string, keyed by enum name.
#[pyfunction]
pub fn wire_enum_values() -> BTreeMap<&'static str, Vec<String>> {
    enum_values()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("SCHEMA_VERSION", honba_core::SCHEMA_VERSION)?;
    m.add("API_VERSION", honba_core::API_VERSION)?;
    m.add_function(wrap_pyfunction!(canonical_json, m)?)?;
    m.add_function(wrap_pyfunction!(wire_enum_values, m)?)?;
    Ok(())
}
