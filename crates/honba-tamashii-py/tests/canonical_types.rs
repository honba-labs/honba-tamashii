//! The native surface, exercised through a real interpreter (plan.md, Phase 1).
//!
//! These are the tests that prove the canonical types cross the boundary
//! exactly: a price is a `decimal.Decimal` in Python and a
//! `rust_decimal::Decimal` in Rust, and a float is refused.

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Imports the extension module the same way Python does.
/// Starts the interpreter once per test binary.
///
/// The extension is built without `auto-initialize` (it is a module, not an
/// embedded interpreter), so a test that drives Python must start it itself.
fn init_python() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        pyo3::prepare_freethreaded_python();
    });
}

/// Builds the extension module in this process.
///
/// The module is registered by calling the same `#[pymodule]` function Python
/// calls, so the test drives the real registration path without needing an
/// installed wheel.
fn native(py: Python<'_>) -> Bound<'_, PyModule> {
    let module = PyModule::new_bound(py, "honba._honba").expect("a module can be created");
    _honba::_honba(&module).expect("the module registers");
    module
}

#[test]
fn a_price_crosses_as_an_exact_decimal() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let price = module
            .getattr("Price")
            .expect("Price is exported")
            .call1(("2540.50",))
            .expect("a decimal string builds a price");
        assert_eq!(price.str().unwrap().to_string(), "2540.50");
        assert_eq!(price.repr().unwrap().to_string(), "Price('2540.50')");

        let decimal = price.call_method0("to_decimal").expect("to_decimal");
        assert_eq!(decimal.get_type().name().unwrap().to_string(), "Decimal");
        assert_eq!(decimal.str().unwrap().to_string(), "2540.50");
    });
}

#[test]
fn a_float_is_refused_at_the_boundary() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let error = module
            .getattr("Price")
            .expect("Price is exported")
            .call1((10.25,))
            .expect_err("a float is refused");
        assert!(error.to_string().contains("floats are not accepted"));
    });
}

#[test]
fn prices_compare_and_add() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let price = module.getattr("Price").unwrap();
        let ten = price.call1(("10",)).unwrap();
        let eleven = price.call1(("11",)).unwrap();
        assert!(
            ten.call_method1("__lt__", (eleven.clone(),))
                .unwrap()
                .is_truthy()
                .unwrap()
        );
        assert!(
            eleven
                .call_method1("__gt__", (ten.clone(),))
                .unwrap()
                .is_truthy()
                .unwrap()
        );
        assert!(
            ten.call_method1("__eq__", (price.call1(("10.00",)).unwrap(),))
                .unwrap()
                .is_truthy()
                .unwrap()
        );

        let sum = ten
            .call_method1("__add__", (price.call1(("2.50",)).unwrap(),))
            .unwrap();
        assert_eq!(sum.str().unwrap().to_string(), "12.50");
    });
}

#[test]
fn a_price_times_a_quantity_is_the_notional() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let price = module
            .getattr("Price")
            .unwrap()
            .call1(("2540.50",))
            .unwrap();
        let quantity = module.getattr("Quantity").unwrap().call1(("10",)).unwrap();
        let notional = price.call_method1("__mul__", (quantity,)).unwrap();
        assert_eq!(notional.get_type().name().unwrap(), "Money");
        assert_eq!(
            notional.call_method0("to_decimal").unwrap().str().unwrap(),
            "25405.00"
        );
        assert_eq!(notional.getattr("currency").unwrap().str().unwrap(), "INR");
    });
}

#[test]
fn the_currency_table_comes_from_the_kernel() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let table = module
            .getattr("currency_minor_units")
            .unwrap()
            .call0()
            .unwrap();
        let table = table.downcast::<PyDict>().unwrap();
        let inr = table.get_item("INR").unwrap().unwrap();
        assert_eq!(inr.get_item(0).unwrap().extract::<u8>().unwrap(), 2);
        assert_eq!(
            inr.get_item(1).unwrap().extract::<String>().unwrap(),
            "paisa"
        );
        assert_eq!(
            inr.get_item(2).unwrap().extract::<String>().unwrap(),
            "paise"
        );
    });
}

#[test]
fn the_version_axes_are_exported() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        assert_eq!(
            module
                .getattr("SCHEMA_VERSION")
                .unwrap()
                .extract::<u32>()
                .unwrap(),
            honba_core::SCHEMA_VERSION
        );
        assert_eq!(
            module
                .getattr("API_VERSION")
                .unwrap()
                .extract::<String>()
                .unwrap(),
            honba_core::API_VERSION
        );
        assert_eq!(
            module
                .getattr("STRATEGY_API_VERSION")
                .unwrap()
                .extract::<String>()
                .unwrap(),
            _honba::pyclasses::manifest::STRATEGY_API_VERSION
        );
    });
}

#[test]
fn the_whole_module_imports() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        for name in [
            "Price",
            "Quantity",
            "Money",
            "InstrumentId",
            "PriceBand",
            "Bar",
            "Tick",
            "DepthLevel",
            "OrderRequest",
            "OrderAck",
            "FillReport",
            "RiskEngine",
            "RiskLimits",
            "RiskRefusalError",
            "Runner",
            "PyContext",
            "Metrics",
            "Sma",
        ] {
            assert!(module.hasattr(name).unwrap(), "{name} is missing");
        }
    });
}
