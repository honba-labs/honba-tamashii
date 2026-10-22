//! The one list of types the wire contract publishes.
//!
//! Every artifact — the JSON Schema and the Python stubs — is rendered from
//! this list. Adding a type here is the only step needed to expose it
//! everywhere; that is the whole point of inverting the direction from
//! Python-generated to Rust-generated.
//!
//! The source of truth is `honba-core`, the shared kernel: it owns the
//! canonical value objects and the wire contracts, and it is the only crate
//! this generator depends on.

use schemars::JsonSchema;
use serde_json::{Value, json};

use crate::schemas::SchemaSet;

/// The data-carrying types that appear on the wire.
pub const WIRE_TYPES: &[&str] = &[
    "InstrumentId",
    "Bar",
    "Tick",
    "DepthLevel",
    "OrderRequest",
    "OrderAck",
    "FillReport",
    "CancelRequest",
    "ModifyRequest",
];

/// The enums whose variants are published as stable string constants.
pub const WIRE_ENUMS: &[&str] = &[
    "Exchange",
    "Currency",
    "OrderSide",
    "OrderType",
    "OrderStatus",
    "TimeInForce",
];

/// The closed AI decision grammar.
pub const DECISION_TYPES: &[&str] = &["Decision"];

/// Every type name the contract publishes, in a stable order.
pub fn published_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    names.extend_from_slice(WIRE_TYPES);
    names.extend_from_slice(DECISION_TYPES);
    names.extend_from_slice(WIRE_ENUMS);
    names
}

/// Adds every published type to `set`.
///
/// Kept in one function so a new type cannot be added to the JSON Schema
/// artifact but forgotten by the stub one: both artifacts read the same set.
pub fn register_all(set: &mut SchemaSet) {
    set.add_type::<honba_core::InstrumentId>("InstrumentId");
    set.add_type::<honba_core::Bar>("Bar");
    set.add_type::<honba_core::Tick>("Tick");
    set.add_type::<honba_core::DepthLevel>("DepthLevel");
    set.add_type::<honba_core::OrderRequest>("OrderRequest");
    set.add_type::<honba_core::OrderAck>("OrderAck");
    set.add_type::<honba_core::FillReport>("FillReport");
    set.add_type::<honba_core::CancelRequest>("CancelRequest");
    set.add_type::<honba_core::ModifyRequest>("ModifyRequest");

    set.add_type::<honba_core::Decision>("Decision");

    set.add_type::<honba_core::Exchange>("Exchange");
    set.add_type::<honba_core::Currency>("Currency");
    set.add_type::<honba_core::OrderSide>("OrderSide");
    set.add_type::<honba_core::OrderType>("OrderType");
    set.add_type::<honba_core::OrderStatus>("OrderStatus");
    set.add_type::<honba_core::TimeInForce>("TimeInForce");
}

/// Builds the full registry used by every artifact.
pub fn full_registry() -> SchemaSet {
    let mut set = SchemaSet::new();
    register_all(&mut set);
    set
}

/// Returns the JSON Schema of `T`, normalized into the shared representation.
pub fn schema_of<T: JsonSchema>(preferred_name: &str) -> Value {
    let mut set = SchemaSet::new();
    set.add_type::<T>(preferred_name);
    let name = set.names().first().copied().unwrap_or(preferred_name);
    set.get(name).cloned().unwrap_or_else(|| json!({}))
}
