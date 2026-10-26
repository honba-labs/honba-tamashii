use pyo3::prelude::*;
use std::time::Duration;

fn init_python() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        pyo3::prepare_freethreaded_python();
    });
}

#[test]
fn test_pyo3_async_timeout_cancellation() {
    init_python();
    Python::with_gil(|py| {
        let code = r#"
import asyncio

cancelled = False

async def slow_task():
    global cancelled
    try:
        await asyncio.sleep(10)
        return "finished"
    except asyncio.CancelledError:
        cancelled = True
        raise
"#;
        let locals = pyo3::types::PyDict::new_bound(py);
        py.run_bound(code, Some(&locals), Some(&locals)).unwrap();
        let slow_task_fn = locals.get_item("slow_task").unwrap().unwrap();

        let event_loop = py
            .import_bound("asyncio")
            .unwrap()
            .call_method0("new_event_loop")
            .unwrap();

        let coro = slow_task_fn.call0().unwrap().unbind();

        let res = pyo3_async_runtimes::tokio::run_until_complete(event_loop, async move {
            let fut = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(coro.bind(py).clone())
            })?;
            // Wrap in tokio timeout
            match tokio::time::timeout(Duration::from_millis(50), fut).await {
                Ok(inner) => inner,
                Err(_elapsed) => {
                    // Timeout occurred
                    Python::with_gil(|_py| {
                        Err(pyo3::exceptions::PyTimeoutError::new_err(
                            "async task timed out",
                        ))
                    })
                }
            }
        });
        println!("Result: {:?}", res);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.is_instance_of::<pyo3::exceptions::PyTimeoutError>(py));
    });
}
