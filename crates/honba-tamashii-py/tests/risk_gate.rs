//! The pre-trade risk gate, exercised through a real interpreter
//! (plan.md, Phase 1).
//!
//! The rules live in `honba-mon`; these tests check that the bridge refuses
//! what the engine refuses, with the engine's own rule codes.

use pyo3::prelude::*;
use pyo3::types::PyDict;

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

/// Builds an order request in Python.
fn order<'py>(
    py: Python<'py>,
    module: &Bound<'py, PyModule>,
    algo_id: &str,
    quantity: &str,
) -> Bound<'py, PyAny> {
    let instrument = module
        .getattr("InstrumentId")
        .unwrap()
        .call1(("RELIANCE", "NSE"))
        .unwrap();
    let side = module.getattr("OrderSide").unwrap().getattr("BUY").unwrap();
    let kwargs = PyDict::new_bound(py);
    kwargs.set_item("algo_id", algo_id).unwrap();
    module
        .getattr("OrderRequest")
        .unwrap()
        .call((instrument, side, quantity), Some(&kwargs))
        .unwrap()
}

#[test]
fn an_unregistered_algo_is_refused_with_the_engines_rule_code() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let engine = module.getattr("RiskEngine").unwrap().call0().unwrap();
        let request = order(py, &module, "UNKNOWN", "10");

        let error = engine
            .call_method1("check_order", (request,))
            .expect_err("an unregistered algo is refused");
        assert!(error
            .get_type_bound(py)
            .name()
            .unwrap()
            .to_string()
            .contains("RiskRefusalError"));

        let rule = error
            .value_bound(py)
            .getattr("rule")
            .unwrap()
            .extract::<String>()
            .unwrap();
        assert_eq!(rule, "algo_not_registered");

        // The refusal travels with the exception as JSON.
        let refusal = error
            .value_bound(py)
            .getattr("refusal")
            .unwrap()
            .extract::<String>()
            .unwrap();
        assert!(refusal.contains("algo_not_registered"));
    });
}

#[test]
fn a_registered_algo_is_approved() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let engine = module.getattr("RiskEngine").unwrap().call0().unwrap();
        engine.call_method1("register_algo", ("ALGO_1",)).unwrap();
        let request = order(py, &module, "ALGO_1", "10");
        engine
            .call_method1("check_order", (request,))
            .expect("a registered algo is approved");
    });
}

#[test]
fn a_price_outside_the_circuit_band_is_refused() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let engine = module.getattr("RiskEngine").unwrap().call0().unwrap();
        engine.call_method1("register_algo", ("ALGO_1",)).unwrap();

        let instrument = module
            .getattr("InstrumentId")
            .unwrap()
            .call1(("RELIANCE", "NSE"))
            .unwrap();
        let band = module
            .getattr("PriceBand")
            .unwrap()
            .call1(("900", "1100", "1000"))
            .unwrap();
        engine
            .call_method1("set_price_band", (instrument.clone(), band))
            .unwrap();

        let kwargs = PyDict::new_bound(py);
        kwargs.set_item("order_type", "limit").unwrap();
        kwargs.set_item("price", "1500").unwrap();
        kwargs.set_item("algo_id", "ALGO_1").unwrap();
        let request = module
            .getattr("OrderRequest")
            .unwrap()
            .call((instrument, module.getattr("OrderSide").unwrap().getattr("BUY").unwrap(), "10"), Some(&kwargs))
            .unwrap();

        let error = engine
            .call_method1("check_order", (request,))
            .expect_err("a price outside the band is refused");
        assert_eq!(
            error
                .value_bound(py)
                .getattr("rule")
                .unwrap()
                .extract::<String>()
                .unwrap(),
            "price_band_violation"
        );
    });
}

#[test]
fn the_global_kill_switch_refuses_everything() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let engine = module.getattr("RiskEngine").unwrap().call0().unwrap();
        engine.call_method1("register_algo", ("ALGO_1",)).unwrap();
        engine
            .call_method1("halt_global", ("operator",))
            .unwrap();
        let request = order(py, &module, "ALGO_1", "10");
        let error = engine
            .call_method1("check_order", (request,))
            .expect_err("the kill switch refuses every order");
        assert_eq!(
            error
                .value_bound(py)
                .getattr("rule")
                .unwrap()
                .extract::<String>()
                .unwrap(),
            "global_halt"
        );
    });
}

#[test]
fn limits_are_parsed_from_the_config_table() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let table = PyDict::new_bound(py);
        table.set_item("max_notional", "100000").unwrap();
        let limits = module
            .getattr("RiskLimits")
            .unwrap()
            .call_method1("from_dict", (table,))
            .unwrap();
        assert!(!limits.call_method0("is_empty").unwrap().is_truthy().unwrap());

        // An unknown key is refused, so a typo in a config file is an error.
        let bad = PyDict::new_bound(py);
        bad.set_item("nope", 1).unwrap();
        assert!(module
            .getattr("RiskLimits")
            .unwrap()
            .call_method1("from_dict", (bad,))
            .is_err());
    });
}
