//! Tests for Phase 2: AnyStrategyBinding dynamic dispatch and versioning.

use pyo3::prelude::*;
use pyo3::types::PyDict;

fn init_python() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        pyo3::prepare_freethreaded_python();
    });
}

fn native(py: Python<'_>) -> Bound<'_, PyModule> {
    let module = PyModule::new_bound(py, "honba._honba").expect("a module can be created");
    _honba::_honba(&module).expect("the module registers");
    module
}

#[test]
fn strategy_version_detection_and_defaults() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let globals = PyDict::new_bound(py);
        globals.set_item("_honba", &module).unwrap();

        // 1. Unversioned legacy strategy -> defaults to 1.0.0 (V1)
        let strat_v1_legacy = py
            .eval_bound(
                "type('LegacyStrat', (), {'on_bar': lambda self, bar, ctx: None})()",
                Some(&globals),
                None,
            )
            .unwrap();
        let binding = _honba::runner::AnyStrategyBinding::detect(&strat_v1_legacy).unwrap();
        assert!(binding.is_v1());
        assert_eq!(binding.version(), "1.0.0");

        // 2. Explicit v1 strategy
        let strat_v1 = py.eval_bound(
            "type('V1Strat', (), {'__strategy_api_version__': '1.2.0', 'on_bar': lambda self, bar, ctx: None})()",
            Some(&globals),
            None,
        ).unwrap();
        let binding = _honba::runner::AnyStrategyBinding::detect(&strat_v1).unwrap();
        assert!(binding.is_v1());
        assert_eq!(binding.version(), "1.0.0");

        // 3. Explicit v2 strategy
        let strat_v2 = py.eval_bound(
            "type('V2Strat', (), {'__strategy_api_version__': '2.0.0', 'on_step': lambda self, ev, ctx: None})()",
            Some(&globals),
            None,
        ).unwrap();
        let binding = _honba::runner::AnyStrategyBinding::detect(&strat_v2).unwrap();
        assert!(binding.is_v2());
        assert_eq!(binding.version(), "2.0.0");

        // 4. Unsupported version -> Error
        let strat_v3 = py
            .eval_bound(
                "type('V3Strat', (), {'__strategy_api_version__': '3.0.0'})()",
                Some(&globals),
                None,
            )
            .unwrap();
        assert!(_honba::runner::AnyStrategyBinding::detect(&strat_v3).is_err());
    });
}

#[test]
fn runner_executes_v1_and_v2_strategies() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let globals = PyDict::new_bound(py);
        globals.set_item("_honba", &module).unwrap();

        let runner = module
            .getattr("Runner")
            .unwrap()
            .call1((10, "RELIANCE"))
            .unwrap();

        // V1 strategy places order in on_bar(bar, ctx)
        py.run_bound(
            r#"
class StrategyV1:
    __strategy_api_version__ = "1.0.0"
    def __init__(self):
        self.bars = 0
    def on_bar(self, bar, ctx):
        self.bars += 1
        if self.bars == 1:
            ctx.submit("RELIANCE", "BUY", "10")
v1_inst = StrategyV1()
"#,
            Some(&globals),
            None,
        )
        .unwrap();

        let v1_inst = globals.get_item("v1_inst").unwrap().unwrap();
        let metrics_v1 = runner.call_method1("run", (v1_inst,)).unwrap();
        let orders_v1 = metrics_v1.getattr("orders").unwrap();
        let orders_v1_len: usize = orders_v1.len().unwrap();
        assert_eq!(orders_v1_len, 1);

        // V2 strategy places order in on_step(event, ctx)
        py.run_bound(
            r#"
class StrategyV2:
    __strategy_api_version__ = "2.0.0"
    def __init__(self):
        self.steps = 0
    def on_step(self, event, ctx):
        self.steps += 1
        assert ctx.version == "2.0.0"
        assert event.kind == "bar"
        if self.steps == 1:
            ctx.submit("RELIANCE", "BUY", "25")
v2_inst = StrategyV2()
"#,
            Some(&globals),
            None,
        )
        .unwrap();

        let v2_inst = globals.get_item("v2_inst").unwrap().unwrap();
        let metrics_v2 = runner.call_method1("run", (v2_inst,)).unwrap();
        let orders_v2 = metrics_v2.getattr("orders").unwrap();
        let orders_v2_len: usize = orders_v2.len().unwrap();
        assert_eq!(orders_v2_len, 1);
    });
}

#[test]
fn market_event_and_context_v2() {
    init_python();
    Python::with_gil(|py| {
        let module = native(py);
        let market_event_cls = module.getattr("MarketEvent").unwrap();
        assert!(market_event_cls.is_instance_of::<pyo3::types::PyType>());

        let ctx_v2_cls = module.getattr("PyContextV2").unwrap();
        assert!(ctx_v2_cls.is_instance_of::<pyo3::types::PyType>());
    });
}
