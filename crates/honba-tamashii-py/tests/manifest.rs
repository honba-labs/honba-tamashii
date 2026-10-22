//! The strategy manifest compiler (ADR 0012).
//!
//! A manifest is data; the IR is that manifest plus everything a runner would
//! otherwise have to infer. The compiler is the one place the resolution rules
//! live, so Python and Rust cannot disagree about what a strategy subscribes
//! to.

use _honba::pyclasses::manifest::{
    ManifestError, STRATEGY_API_VERSION, StrategyManifest, compile, validate, verify,
};
use serde_json::{Value, json};

fn manifest_json(universe: Value, subscriptions: Value) -> String {
    json!({
        "api_version": STRATEGY_API_VERSION,
        "name": "demo",
        "source_hash": "sha256:abc",
        "universe": universe,
        "subscriptions": subscriptions,
        "driving_timeframe": {"interval": 1, "aggregation": "minute"},
        "warmup_bars": 10,
    })
    .to_string()
}

fn explicit() -> Value {
    json!({"explicit": [{"symbol": "RELIANCE", "exchange": "NSE"}]})
}

fn subscribed() -> Value {
    json!({"instruments": [{"symbol": "RELIANCE", "exchange": "NSE"}]})
}

#[test]
fn a_valid_manifest_compiles_to_an_ir() {
    let ir = verify(&manifest_json(explicit(), subscribed())).expect("the manifest compiles");
    let ir: Value = serde_json::from_str(&ir).unwrap();
    assert_eq!(ir["strategy_api_version"], json!(STRATEGY_API_VERSION));
    assert_eq!(ir["warmup_bars"], json!(10));
    assert_eq!(ir["universe"]["instruments"][0]["symbol"], json!("RELIANCE"));
    assert_eq!(ir["subscriptions"]["bars"][0]["exchange"], json!("NSE"));
    assert!(ir["subscriptions"]["quotes"].as_array().unwrap().is_empty());
}

#[test]
fn an_empty_name_is_refused() {
    let mut manifest: StrategyManifest =
        serde_json::from_str(&manifest_json(explicit(), subscribed())).unwrap();
    manifest.name = "  ".to_string();
    assert_eq!(validate(&manifest), Err(ManifestError::EmptyName));
}

#[test]
fn an_unresolved_named_universe_is_refused() {
    let manifest: StrategyManifest =
        serde_json::from_str(&manifest_json(json!({"named": "nifty50"}), subscribed())).unwrap();
    assert_eq!(
        validate(&manifest),
        Err(ManifestError::NamedUniverseUnresolved)
    );
}

#[test]
fn a_manifest_without_subscriptions_is_refused() {
    let manifest: StrategyManifest =
        serde_json::from_str(&manifest_json(explicit(), json!({"instruments": []}))).unwrap();
    assert_eq!(compile(manifest), Err(ManifestError::NoSubscriptions));
}

#[test]
fn a_subscription_outside_the_universe_is_refused() {
    let manifest: StrategyManifest = serde_json::from_str(&manifest_json(
        explicit(),
        json!({"instruments": [{"symbol": "TCS", "exchange": "NSE"}]}),
    ))
    .unwrap();
    assert_eq!(
        compile(manifest),
        Err(ManifestError::SubscriptionOutsideUniverse)
    );
}

#[test]
fn a_malformed_manifest_is_refused_with_a_code() {
    let failure = verify("{not json").expect_err("a malformed manifest is refused");
    assert_eq!(failure.code, "deserialize");
}

#[test]
fn an_unknown_field_is_refused() {
    let text = json!({
        "api_version": STRATEGY_API_VERSION,
        "name": "demo",
        "source_hash": "sha256:abc",
        "universe": explicit(),
        "subscriptions": subscribed(),
        "driving_timeframe": {"interval": 1, "aggregation": "minute"},
        "warmup_bars": 0,
        "surprise": true,
    })
    .to_string();
    let failure = verify(&text).expect_err("an unknown field is refused");
    assert_eq!(failure.code, "deserialize");
}
