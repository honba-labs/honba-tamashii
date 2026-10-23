//! Unit tests for `honba_codegen::registry`.

use honba_codegen::registry::*;
use std::collections::BTreeSet;

#[test]
fn every_published_name_is_actually_registered() {
    // Guards the failure where a name is listed in a constant but never
    // added, so the artifact silently omits it.
    let set = full_registry();
    for name in published_names() {
        assert!(
            set.get(name).is_some(),
            "{name} is published but not registered"
        );
    }
}

#[test]
fn the_registry_has_no_dangling_references() {
    assert_eq!(full_registry().dangling_references(), Vec::<String>::new());
}

#[test]
fn published_names_are_unique() {
    let unique: BTreeSet<_> = published_names().into_iter().collect();
    assert_eq!(unique.len(), published_names().len());
}

#[test]
fn the_canonical_wire_types_are_published() {
    let set = full_registry();
    for name in [
        "InstrumentId",
        "Bar",
        "Tick",
        "DepthLevel",
        "OrderRequest",
        "OrderAck",
        "FillReport",
    ] {
        assert!(set.get(name).is_some(), "{name} is missing from the registry");
    }
}

#[test]
fn the_decision_grammar_is_published() {
    // The closed AI grammar is part of the contract: a client must be able to
    // read a decision without a second schema.
    let set = full_registry();
    assert!(set.get("Decision").is_some());
}
