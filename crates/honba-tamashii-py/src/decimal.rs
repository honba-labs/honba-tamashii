//! Exact decimal interop between `rust_decimal` and Python's `decimal.Decimal`.
//!
//! The Zero Floats policy holds at the FFI boundary: a price, quantity or
//! amount crosses as a decimal, never as a float. A Python `float` is refused
//! outright — accepting one would silently lose precision the moment it
//! entered the domain.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyInt, PyString};
use rust_decimal::Decimal;
use std::str::FromStr;

/// The message used when a float is refused.
const FLOAT_REFUSED: &str =
    "floats are not accepted at the money boundary; pass a decimal string or a decimal.Decimal";

/// Builds a Python `decimal.Decimal` from a Rust decimal.
///
/// The value goes through its exact decimal text, so no binary rounding
/// happens on the way.
pub fn to_py_decimal<'py>(py: Python<'py>, value: Decimal) -> PyResult<Bound<'py, PyAny>> {
    let module = py.import_bound("decimal")?;
    module.getattr("Decimal")?.call1((value.to_string(),))
}

/// Reads a Python value as an exact decimal.
///
/// Accepts `decimal.Decimal`, `int` and `str`; refuses `float` (see
/// [`FLOAT_REFUSED`]) and anything that is not a finite decimal.
pub fn from_py_decimal(value: &Bound<'_, PyAny>) -> PyResult<Decimal> {
    if value.is_instance_of::<PyFloat>() {
        return Err(PyTypeError::new_err(FLOAT_REFUSED));
    }
    let text = if let Ok(text) = value.downcast::<PyString>() {
        text.to_str()?.to_string()
    } else if value.is_instance_of::<PyInt>() {
        value.str()?.to_string()
    } else if value.hasattr("__str__")? {
        value.call_method0("__str__")?.extract::<String>()?
    } else {
        return Err(PyTypeError::new_err(format!(
            "expected a decimal string, an int or a decimal.Decimal, got {}",
            value.get_type().name()?
        )));
    };
    Decimal::from_str(&text).map_err(|e| {
        PyValueError::new_err(format!("{text:?} is not a decimal: {e}"))
    })
}

/// Reads an optional Python value as an optional exact decimal.
pub fn from_py_optional_decimal(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Decimal>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(from_py_decimal(value)?)),
    }
}

/// Reads a Python `datetime` as a UTC timestamp.
///
/// Naive datetimes are read as UTC; the domain has one time zone.
pub fn from_py_datetime(value: &Bound<'_, PyAny>) -> PyResult<chrono::DateTime<chrono::Utc>> {
    use chrono::{DateTime, NaiveDateTime, Utc};
    if let Ok(text) = value.extract::<String>() {
        return parse_timestamp(&text);
    }
    let naive: Option<NaiveDateTime> = value.extract().ok();
    if let Some(naive) = naive {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
    }
    Err(PyTypeError::new_err(
        "expected a datetime or an ISO-8601 timestamp string",
    ))
}

/// Parses an ISO-8601 timestamp into UTC.
pub fn parse_timestamp(text: &str) -> PyResult<chrono::DateTime<chrono::Utc>> {
    use chrono::{DateTime, Utc};
    if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
        return Ok(parsed.with_timezone(&Utc));
    }
    // A naive ISO timestamp is read as UTC.
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(
            date.and_hms_opt(0, 0, 0).expect("midnight is a valid time"),
            Utc,
        ));
    }
    Err(PyValueError::new_err(format!(
        "{text:?} is not an ISO-8601 timestamp"
    )))
}

/// Reads an optional timestamp: a string, a `datetime`, or nothing.
pub fn from_py_optional_timestamp(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<chrono::DateTime<chrono::Utc>>> {
    match value {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => Ok(Some(from_py_datetime(value)?)),
    }
}
