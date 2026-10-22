//! The Python-facing wire contract (`canonical_json`) agrees with the shared
//! golden vectors in `schema/golden/` (ADR 006).
//!
//! The vectors are the kernel's (`honba-core/schema/golden`); this copy is
//! the one the Python bridge is checked against, so a change to the canonical
//! types that the bridge does not agree with fails here.
//!
//! Exercises `_honba::pyclasses::wire::canonical` through the Rust API only:
//! no Python interpreter is started, so this runs under plain `cargo test`.

use std::path::PathBuf;

use _honba::pyclasses::wire::{KINDS, canonical};
use serde_json::Value;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/golden")
}

fn golden_files() -> Vec<(String, Value)> {
    let mut files: Vec<_> = std::fs::read_dir(golden_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            (p.file_name().unwrap().to_string_lossy().into_owned(), doc)
        })
        .collect()
}

/// The wire kind each golden file pins.
fn kind_of(file: &str) -> &'static str {
    match file {
        "tick_ltp_golden.json" | "tick_quote_golden.json" => "Tick",
        "decision_placeorder_golden.json" => "Decision",
        "price_band_golden.json" => "PriceBand",
        other => panic!("no wire kind pinned for {other}"),
    }
}

#[test]
fn every_golden_vector_round_trips_unchanged_and_idempotently() {
    for (file, doc) in golden_files() {
        let kind = kind_of(&file);
        let out = canonical(kind, &doc.to_string())
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(&parsed, &doc, "{file}");
        assert_eq!(canonical(kind, &out).unwrap(), out, "{file}");
    }
}

#[test]
fn a_malformed_payload_is_refused() {
    assert!(canonical("Tick", "{\"type\": \"ltp\"}").is_err());
    assert!(canonical("Decision", "{\"action\": \"place_order\"}").is_err());
    assert!(canonical("NotAKind", "{}").is_err());
}

#[test]
fn the_wire_kinds_cover_the_canonical_types() {
    for kind in [
        "InstrumentId",
        "Bar",
        "Tick",
        "DepthLevel",
        "OrderRequest",
        "OrderAck",
        "FillReport",
        "Decision",
    ] {
        assert!(KINDS.contains(&kind), "{kind} is not a wire kind");
    }
}
