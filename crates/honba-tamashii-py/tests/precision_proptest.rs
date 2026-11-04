//! Dedicated proptest precision and lossless roundtrip invariant test suite.
//!
//! Asserts that converting `Price`, `Quantity`, `Money`, `RiskBudget`, and `Decimal`
//! between Rust `rust_decimal::Decimal` and Python `decimal.Decimal` over randomized
//! inputs produces zero precision loss, preserves scales, and enforces strict
//! boundary float refusal.

use honba_core::{Money, Price, Quantity};
use proptest::prelude::*;
use pyo3::prelude::*;
use pyo3::types::PyFloat;
use rust_decimal::Decimal;
use std::sync::Once;

fn init_python() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        pyo3::prepare_freethreaded_python();
    });
}

fn native_module(py: Python<'_>) -> Bound<'_, PyModule> {
    let module = PyModule::new_bound(py, "honba._honba").expect("a module can be created");
    _honba::_honba(&module).expect("the module registers");
    module
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn decimal_lossless_roundtrip_prop(
        mantissa in -1_000_000_000_000i64..1_000_000_000_000i64,
        scale in 0u32..=8u32,
    ) {
        init_python();
        let original = Decimal::new(mantissa, scale);
        Python::with_gil(|py| -> Result<(), TestCaseError> {
            let py_dec = _honba::decimal::to_py_decimal(py, original)
                .expect("to_py_decimal succeeds");
            let roundtripped = _honba::decimal::from_py_decimal(&py_dec)
                .expect("from_py_decimal succeeds");

            prop_assert_eq!(original, roundtripped);
            prop_assert_eq!(original.scale(), roundtripped.scale());
            prop_assert_eq!(original.to_string(), roundtripped.to_string());
            Ok(())
        })?;
    }

    #[test]
    fn price_quantity_roundtrip_prop(
        mantissa in 1i64..100_000_000_000i64,
        scale in 0u32..=4u32,
    ) {
        init_python();
        let dec = Decimal::new(mantissa, scale);
        let dec_str = dec.to_string();

        Python::with_gil(|py| -> Result<(), TestCaseError> {
            let m = native_module(py);
            let price_cls = m.getattr("Price").expect("Price is exported");
            let qty_cls = m.getattr("Quantity").expect("Quantity is exported");

            let py_price = price_cls.call1((&dec_str,)).expect("Price constructor succeeds");
            let py_qty = qty_cls.call1((&dec_str,)).expect("Quantity constructor succeeds");

            let price_to_dec = py_price.call_method0("to_decimal").expect("to_decimal succeeds");
            let qty_to_dec = py_qty.call_method0("to_decimal").expect("to_decimal succeeds");

            let price_rust = _honba::decimal::from_py_decimal(&price_to_dec).expect("price dec read");
            let qty_rust = _honba::decimal::from_py_decimal(&qty_to_dec).expect("qty dec read");

            prop_assert_eq!(Price::new(dec).0, price_rust);
            prop_assert_eq!(Quantity::new(dec).0, qty_rust);
            Ok(())
        })?;
    }

    #[test]
    fn money_and_risk_budget_roundtrip_prop(
        equity_mantissa in 100_000i64..10_000_000_000i64,
        risk_pct_scaled in 1u32..=500u32, // 0.0001 to 0.0500
    ) {
        init_python();
        let equity_dec = Decimal::new(equity_mantissa, 2);
        let risk_pct_dec = Decimal::new(risk_pct_scaled as i64, 4);

        Python::with_gil(|py| -> Result<(), TestCaseError> {
            let m = native_module(py);
            let money_cls = m.getattr("Money").expect("Money is exported");
            let rb_cls = m.getattr("RiskBudget").expect("RiskBudget is exported");

            let py_money = money_cls.call1((equity_dec.to_string(), "INR")).expect("Money constructor");
            let money_to_dec = py_money.call_method0("to_decimal").expect("Money to_decimal");
            let money_rust = _honba::decimal::from_py_decimal(&money_to_dec).expect("read money");
            prop_assert_eq!(Money::inr(equity_dec).amount, money_rust);

            let py_rb = rb_cls.call1((py_money, risk_pct_dec.to_string())).expect("RiskBudget constructor");
            let rb_equity = py_rb.getattr("equity").expect("get equity");
            let rb_equity_dec = rb_equity.call_method0("to_decimal").expect("rb equity to_decimal");
            let rb_equity_rust = _honba::decimal::from_py_decimal(&rb_equity_dec).expect("read rb equity");
            prop_assert_eq!(equity_dec, rb_equity_rust);

            let rb_risk_pct = py_rb.getattr("risk_pct").expect("get risk_pct");
            let rb_risk_pct_rust = _honba::decimal::from_py_decimal(&rb_risk_pct).expect("read rb risk_pct");
            prop_assert_eq!(risk_pct_dec, rb_risk_pct_rust);

            Ok(())
        })?;
    }

    #[test]
    fn boundary_refuses_float_prop(f in any::<f64>()) {
        if !f.is_nan() && !f.is_infinite() {
            init_python();
            Python::with_gil(|py| -> Result<(), TestCaseError> {
                let py_float = PyFloat::new_bound(py, f);
                let err = _honba::decimal::from_py_decimal(py_float.as_any())
                    .expect_err("float must be refused at decimal boundary");
                prop_assert!(err.to_string().contains("floats are not accepted"));
                Ok(())
            })?;
        }
    }
}
