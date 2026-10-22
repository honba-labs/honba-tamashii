//! Rust-backed Python classes exposed to the embedded interpreter.
//!
//! Every class here wraps a canonical type from `honba-core` (values, market
//! data, orders) or the `honba-mon` risk gate. The layer above this one is
//! Python; the layer below is pure Rust.

use pyo3::prelude::*;
use pyo3::types::PyModule;

pub mod codegen;
pub mod core;
pub mod domain;
pub mod harness;
pub mod manifest;
pub mod order;
pub mod risk;
pub mod strategy;
pub mod wire;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    codegen::register(m)?;
    core::register(m)?;
    domain::register(m)?;
    harness::register(m)?;
    manifest::register(m)?;
    order::register(m)?;
    risk::register(m)?;
    strategy::register(m)?;
    wire::register(m)?;
    Ok(())
}
