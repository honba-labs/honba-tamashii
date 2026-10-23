/// Zero-copy Arrow PyCapsule protocol integration.
///
/// Exposes Rust Arrow data to Python with zero-copy semantics.
use pyo3::prelude::*;

/// No-op capsule for protocol registration.
#[pyfunction]
fn protocol_capsule() -> PyResult<()> {
    Ok(())
}

/// Register Arrow functions into honba.data namespace.
pub fn register(_module: &Bound<'_, PyModule>) -> PyResult<()> {
    _module.add_function(wrap_pyfunction!(protocol_capsule, _module)?)?;
    Ok(())
}