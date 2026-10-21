use honba_messages::{ENDPOINTS, WRITE_PATHS};

use crate::docs::render;
use crate::registry::{
    API_TYPES, CONFIG_TYPES, MANIFEST_TYPES, SCREENER_TYPES, WIRE_ENUMS, WIRE_TYPES,
    request_type_names,
};

#[test]
fn header_carries_versions_and_do_not_edit() {
    let md = render(4, "1.2.3");
    assert!(md.contains("# Honba API Reference"));
    assert!(md.contains("DO NOT EDIT"));
    assert!(md.contains("`4`"));
    assert!(md.contains("`1.2.3`"));
}

#[test]
fn rest_table_contains_every_registry_endpoint() {
    let md = render(4, "1.0.0");
    for (method, path) in ENDPOINTS {
        assert!(
            md.contains(&format!("| `{method}` | `{path}` |")),
            "missing endpoint row: {method} {path}"
        );
    }
}

#[test]
fn rest_table_marks_write_access() {
    let md = render(4, "1.0.0");
    for (method, path) in WRITE_PATHS {
        assert!(
            md.contains(&format!("| `{method}` | `{path}` | write |")),
            "write route not marked write: {method} {path}"
        );
    }
}

#[test]
fn rest_table_marks_streaming_route() {
    let md = render(4, "1.0.0");
    let line = md
        .lines()
        .find(|l| l.contains("`/backtests/{id}/events`"))
        .expect("streaming endpoint row");
    assert!(line.contains("text/event-stream"));
}

#[test]
fn domain_types_list_every_published_name() {
    let md = render(4, "1.0.0");
    for name in WIRE_TYPES
        .iter()
        .chain(SCREENER_TYPES)
        .chain(WIRE_ENUMS)
        .chain(API_TYPES)
        .chain(MANIFEST_TYPES)
        .chain(CONFIG_TYPES)
        .chain(request_type_names())
    {
        assert!(
            md.contains(&format!("- `{name}`\n")),
            "missing type: {name}"
        );
    }
}
