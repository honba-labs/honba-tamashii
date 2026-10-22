//! Unit tests for `crate`.

use crate::*;
use serde_json::{Value, json};

#[test]
fn the_version_axes_are_owned_by_the_kernel() {
    // The kernel owns the wire types, so it owns their versions; codegen
    // re-exports them rather than repeating the literal.
    assert_eq!(SCHEMA_VERSION, honba_core::SCHEMA_VERSION);
    assert_eq!(API_VERSION, honba_core::API_VERSION);
    assert!(!API_VERSION.is_empty());
}

#[test]
fn the_bundle_declares_a_draft_that_supports_defs() {
    let schema = Codegen::new().json_schema();
    assert_eq!(
        schema["$schema"],
        json!("https://json-schema.org/draft/2020-12/schema")
    );
    assert!(schema["$defs"].is_object());
}

#[test]
fn every_ref_in_the_bundle_is_resolvable() {
    // The defect this guards: the bundle declared `$defs` while the
    // generated types still used `#/definitions/`, so nested references
    // pointed at a key that did not exist.
    let schema = Codegen::new().json_schema();
    let defs = schema["$defs"].as_object().expect("$defs object");
    let mut missing = Vec::new();
    check_refs(&schema["$defs"], "#/$defs/", defs, &mut missing);
    assert!(missing.is_empty(), "unresolvable $refs: {missing:?}");
}

fn check_refs(
    value: &Value,
    prefix: &str,
    defs: &serde_json::Map<String, Value>,
    missing: &mut Vec<String>,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "$ref" {
                    if let Some(reference) = child.as_str() {
                        let name = reference.strip_prefix(prefix).unwrap_or(reference);
                        if !defs.contains_key(name) {
                            missing.push(reference.to_string());
                        }
                    }
                } else {
                    check_refs(child, prefix, defs, missing);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                check_refs(item, prefix, defs, missing);
            }
        }
        _ => {}
    }
}

#[test]
fn the_json_schema_bundle_refs_resolve_as_json_pointers() {
    let unresolved = unresolved_local_refs(&Codegen::new().json_schema());
    assert!(unresolved.is_empty(), "unresolvable $refs: {unresolved:?}");
}

#[test]
fn a_bare_ref_is_reported_as_unresolved() {
    let doc = json!({"components": {"schemas": {"A": {"type": "object"}, "B": {"$ref": "A"}}}});
    assert_eq!(unresolved_local_refs(&doc), vec!["A".to_string()]);
}

#[test]
fn a_pointer_ref_that_exists_resolves() {
    let doc = json!({"$defs": {"A": {"type": "object"}, "B": {"$ref": "#/$defs/A"}}});
    assert!(unresolved_local_refs(&doc).is_empty());
}

#[test]
fn a_pointer_ref_to_a_missing_key_is_unresolved() {
    let doc = json!({"$defs": {"B": {"$ref": "#/$defs/A"}}});
    assert_eq!(unresolved_local_refs(&doc), vec!["#/$defs/A".to_string()]);
}

#[test]
fn pointer_escapes_are_decoded() {
    let doc = json!({"$defs": {"a/b": {"x": 1}, "c~d": {"y": 2}},
                     "r1": {"$ref": "#/$defs/a~1b"}, "r2": {"$ref": "#/$defs/c~0d"}});
    assert!(unresolved_local_refs(&doc).is_empty());
}

#[test]
fn the_bundle_carries_both_version_axes() {
    let schema = Codegen::new().json_schema();
    assert_eq!(schema["schema_version"], json!(SCHEMA_VERSION));
    assert_eq!(schema["api_version"], json!(API_VERSION));
}

#[test]
fn the_core_version_is_reported() {
    assert!(!CORE_VERSION.is_empty());
}

#[test]
fn every_artifact_kind_round_trips_through_its_name() {
    for artifact in Artifact::ALL {
        assert_eq!(Artifact::from_name(artifact.name()), Some(*artifact));
    }
    assert_eq!(Artifact::from_name("nope"), None);
}

#[test]
fn rendered_text_is_exactly_what_the_writer_writes() {
    // `honba schema export` in Python writes `render` output; it must be the
    // same bytes the Rust CLI writes, or the two paths drift.
    let codegen = Codegen::new();
    let dir = std::env::temp_dir().join(format!("honba-codegen-unit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let written = codegen.write_all(&dir).expect("write_all");
    for (artifact, path) in Artifact::ALL.iter().zip(&written) {
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some(artifact.file_name())
        );
        let on_disk = std::fs::read_to_string(path).expect("read");
        assert_eq!(codegen.render(*artifact), on_disk, "{}", artifact.name());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rendered_json_ends_with_a_newline() {
    let text = Codegen::new().render(Artifact::JsonSchema);
    assert!(text.ends_with("}\n"));
}

#[test]
fn the_bundle_root_names_each_wire_type() {
    // The root `properties` let one document carry any wire value under its
    // snake_case name.
    let schema = Codegen::new().json_schema();
    assert_eq!(schema["type"], json!("object"));
    let props = schema["properties"].as_object().unwrap();
    for name in WIRE_TYPES.iter().chain(DECISION_TYPES) {
        assert!(
            props
                .values()
                .any(|v| v["$ref"] == json!(format!("#/$defs/{name}"))),
            "{name}"
        );
    }
}
