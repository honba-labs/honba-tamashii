//! The Indian market helpers: F&O contract parsing and settlement days.
//!
//! The source of truth is `honba-me-market`, the market-data crate that owns
//! the Indian calendar, cost model and F&O grammar.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use honba_market::india::fno::{FnoContract, parse_fno_symbol as parse_market_fno_symbol};
use honba_market::india::profile::IndiaMarketProfile;

/// Parsed Indian F&O contract details.
#[pyclass(name = "FnoContract", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyFnoContract {
    /// The formatted contract symbol.
    #[pyo3(get)]
    pub symbol: String,
    /// The underlying symbol.
    #[pyo3(get)]
    pub underlying: String,
    /// `future` or `option`.
    #[pyo3(get)]
    pub kind: String,
    /// The expiry year.
    #[pyo3(get)]
    pub expiry_year: i32,
    /// The expiry month.
    #[pyo3(get)]
    pub expiry_month: u32,
    /// The expiry day, for a weekly contract.
    #[pyo3(get)]
    pub expiry_day: Option<u32>,
    /// Whether the contract expires weekly.
    #[pyo3(get)]
    pub is_weekly: bool,
    /// The strike price, for an option.
    #[pyo3(get)]
    pub strike: Option<String>,
    /// The option type (`CE` or `PE`).
    #[pyo3(get)]
    pub option_type: Option<String>,
}

#[pymethods]
impl PyFnoContract {
    fn __repr__(&self) -> String {
        format!("FnoContract('{}')", self.symbol)
    }

    fn __str__(&self) -> String {
        self.symbol.clone()
    }
}

/// Parse an Indian F&O contract symbol into its structured components.
#[pyfunction]
pub fn parse_fno_symbol(symbol: &str) -> PyResult<PyFnoContract> {
    let contract: FnoContract =
        parse_market_fno_symbol(symbol).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(fno_contract_of(&contract))
}

/// The Python view of an F&O contract.
fn fno_contract_of(contract: &FnoContract) -> PyFnoContract {
    let expiry = contract.expiry();
    PyFnoContract {
        symbol: contract.format_symbol(),
        underlying: contract.underlying().to_string(),
        kind: match contract.kind() {
            honba_market::expiry::InstrumentKind::Future => "future".to_string(),
            honba_market::expiry::InstrumentKind::Option => "option".to_string(),
            honba_market::expiry::InstrumentKind::Equity => "equity".to_string(),
        },
        expiry_year: expiry.year(),
        expiry_month: expiry.month(),
        expiry_day: expiry.day(),
        is_weekly: expiry.is_weekly(),
        strike: contract.strike().map(|strike| strike.0.to_string()),
        option_type: contract
            .option_type()
            .map(|option_type| option_type.code().to_string()),
    }
}

/// India (NSE/BSE) equity delivery settlement cycle in days.
///
/// With no `as_of` this is the cycle in force today (T+1); with an ISO date
/// (`"YYYY-MM-DD"`) it is the cycle on that date (T+2 before 2023-01-27, T+1
/// from then).
#[pyfunction]
#[pyo3(signature = (as_of=None))]
pub fn nse_equity_settlement_days(as_of: Option<&str>) -> PyResult<usize> {
    match as_of {
        None => Ok(IndiaMarketProfile::equity_settlement_days()),
        Some(text) => {
            let date = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|e| {
                PyValueError::new_err(format!(
                    "as_of must be an ISO date (YYYY-MM-DD), got {text:?}: {e}"
                ))
            })?;
            Ok(IndiaMarketProfile::equity_settlement_days_as_of(date))
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFnoContract>()?;
    m.add_function(wrap_pyfunction!(parse_fno_symbol, m)?)?;
    m.add_function(wrap_pyfunction!(nse_equity_settlement_days, m)?)?;
    Ok(())
}
