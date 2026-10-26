//! Tokio-AsyncIO Cooperative Execution Bridge (ADR 0015, Phase 8 / M7).
//!
//! Provides:
//! - Native `pyo3-asyncio` bridge allowing Python `async def` strategy callbacks
//!   to integrate cooperatively with the Rust Tokio event loop.
//! - Timeout and cancellation semantics (`PyCancellationToken`) enabling Tokio
//!   to cleanly abort stalled Python async tasks without deadlocking worker threads.
//! - Dedicated thread pool isolation for blocking Python calls, guaranteeing
//!   the main actor event loop is never starved.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use pyo3::exceptions::{PyRuntimeError, PyTimeoutError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyTuple;

/// Thread-safe cooperative cancellation token for Python and Tokio.
#[pyclass(name = "PyCancellationToken", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyCancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

#[pymethods]
impl PyCancellationToken {
    /// Creates a new uncancelled token.
    #[new]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Triggers cancellation, notifying all waiting Rust and Python tasks.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// Whether this token has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Resets the token to uncancelled state.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    /// Returns an awaitable future that resolves when cancellation is triggered.
    pub fn cancelled<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let notify = Arc::clone(&self.notify);
        let cancelled = Arc::clone(&self.cancelled);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            if !cancelled.load(Ordering::SeqCst) {
                notify.notified().await;
            }
            Python::with_gil(|py| Ok(py.None()))
        })
    }

    fn __repr__(&self) -> String {
        format!("PyCancellationToken(cancelled={})", self.is_cancelled())
    }

    fn __bool__(&self) -> bool {
        !self.is_cancelled()
    }
}

impl Default for PyCancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl PyCancellationToken {
    /// Rust async wait until cancelled.
    pub async fn wait_cancelled(&self) {
        if !self.is_cancelled() {
            self.notify.notified().await;
        }
    }
}

/// Runs a Python coroutine cooperatively with Tokio, honoring timeouts and cancellation.
#[pyfunction]
#[pyo3(signature = (coro, timeout_secs=None, token=None))]
pub fn run_coroutine(
    py: Python<'_>,
    coro: Bound<'_, PyAny>,
    timeout_secs: Option<f64>,
    token: Option<PyCancellationToken>,
) -> PyResult<PyObject> {
    if let Some(secs) = timeout_secs {
        if secs <= 0.0 {
            return Err(PyValueError::new_err("timeout_secs must be positive"));
        }
    }

    let event_loop = py.import_bound("asyncio")?.call_method0("new_event_loop")?;
    let coro_obj = coro.unbind();

    let timeout_duration = timeout_secs.map(Duration::from_secs_f64);

    let res: PyResult<PyObject> =
        pyo3_async_runtimes::tokio::run_until_complete(event_loop, async move {
            let fut = Python::with_gil(|py| {
                pyo3_async_runtimes::tokio::into_future(coro_obj.bind(py).clone())
            })?;

            let execution_fut = async move {
                if let Some(tok) = token {
                    tokio::select! {
                        res = fut => res,
                        _ = tok.wait_cancelled() => {
                            Python::with_gil(|py| {
                                let exc = py.import_bound("asyncio")?.getattr("CancelledError")?;
                                Err(PyErr::from_value_bound(exc.call0()?))
                            })
                        }
                    }
                } else {
                    fut.await
                }
            };

            match timeout_duration {
                Some(duration) => match tokio::time::timeout(duration, execution_fut).await {
                    Ok(inner_res) => inner_res,
                    Err(_) => Python::with_gil(|_py| {
                        Err(PyTimeoutError::new_err(format!(
                            "async task timed out after {:.3}s",
                            duration.as_secs_f64()
                        )))
                    }),
                },
                None => execution_fut.await,
            }
        });

    res
}

/// A dedicated thread pool for executing blocking Python calls without starving
/// the main Tokio actor event loop.
static DEDICATED_BLOCKING_POOL: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn get_blocking_pool() -> &'static tokio::runtime::Runtime {
    DEDICATED_BLOCKING_POOL.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("honba-py-blocking")
            .enable_all()
            .build()
            .expect("failed to initialize dedicated blocking python thread pool")
    })
}

/// Executes a blocking Python callable on the dedicated isolated thread pool,
/// returning an awaitable asyncio Future.
///
/// This releases the GIL immediately on the calling thread so the main Tokio actor
/// event loop and other async tasks are never starved.
#[pyfunction]
#[pyo3(signature = (callable, args=None, timeout_secs=None))]
pub fn spawn_blocking_call<'py>(
    py: Python<'py>,
    callable: Py<PyAny>,
    args: Option<Py<PyTuple>>,
    timeout_secs: Option<f64>,
) -> PyResult<Bound<'py, PyAny>> {
    let pool = get_blocking_pool();
    let timeout_duration = timeout_secs.map(Duration::from_secs_f64);

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Spawn on dedicated blocking pool
        pool.spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                Python::with_gil(|py| {
                    let callable_bound = callable.bind(py);
                    match args {
                        Some(a) => callable_bound.call1(a.bind(py)),
                        None => callable_bound.call0(),
                    }
                    .map(|res| res.unbind())
                })
            })
            .await;
            let _ = tx.send(res);
        });

        let wait_fut = async move {
            match rx.await {
                Ok(Ok(py_res)) => py_res,
                Ok(Err(join_err)) => Python::with_gil(|_py| {
                    Err(PyRuntimeError::new_err(format!(
                        "blocking call failed: {join_err}"
                    )))
                }),
                Err(_) => Python::with_gil(|_py| {
                    Err(PyRuntimeError::new_err("blocking call channel closed"))
                }),
            }
        };

        if let Some(duration) = timeout_duration {
            match tokio::time::timeout(duration, wait_fut).await {
                Ok(inner) => inner,
                Err(_) => Python::with_gil(|_py| {
                    Err(PyTimeoutError::new_err(format!(
                        "blocking call timed out after {:.3}s",
                        duration.as_secs_f64()
                    )))
                }),
            }
        } else {
            wait_fut.await
        }
    })
}

/// Synchronously runs a blocking callable on the dedicated isolated thread pool
/// while releasing the GIL on the current thread.
#[pyfunction]
#[pyo3(signature = (callable, args=None))]
pub fn run_in_blocking_pool(
    py: Python<'_>,
    callable: Bound<'_, PyAny>,
    args: Option<&Bound<'_, PyTuple>>,
) -> PyResult<PyObject> {
    let callable_obj = callable.unbind();
    let args_obj = args.map(|a| a.clone().unbind());

    py.allow_threads(move || {
        let pool = get_blocking_pool();
        pool.block_on(async {
            tokio::task::spawn_blocking(move || {
                Python::with_gil(|py| {
                    let c = callable_obj.bind(py);
                    match args_obj {
                        Some(a) => c.call1(a.bind(py)),
                        None => c.call0(),
                    }
                    .map(|v| v.unbind())
                })
            })
            .await
            .unwrap_or_else(|e| Python::with_gil(|_py| Err(PyRuntimeError::new_err(e.to_string()))))
        })
    })
}

/// Helper to execute an awaitable or synchronous callback.
/// If `result` is a coroutine/awaitable, runs it cooperatively with Tokio.
/// If regular value, returns it as-is.
pub fn resolve_callback_cooperative(
    py: Python<'_>,
    result: Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    if result.hasattr("__await__")? {
        run_coroutine(py, result, Some(30.0), None)
    } else {
        Ok(result.unbind())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCancellationToken>()?;
    m.add_function(wrap_pyfunction!(run_coroutine, m)?)?;
    m.add_function(wrap_pyfunction!(spawn_blocking_call, m)?)?;
    m.add_function(wrap_pyfunction!(run_in_blocking_pool, m)?)?;
    Ok(())
}
