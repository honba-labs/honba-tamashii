//! The strategy manifest surface exposed to Python (ADR 0012).
//!
//! `verify_manifest` is the one compiler from a strategy manifest to its IR:
//! Python's `honba.strategies.verify` and `honba verify` wrap it rather than
//! re-implementing the resolution rules.
//!
//! The manifest is data — name, source digest, universe, subscriptions,
//! driving timeframe, warm-up — and the IR is that manifest plus everything a
//! runner would otherwise have to infer: the concrete universe, the
//! per-channel subscription lists and the timeframes to load.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use serde::{Deserialize, Serialize};

/// The strategy API version this bridge speaks.
pub const STRATEGY_API_VERSION: &str = "1.0.0";

/// The version of the manifest schema.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Why `verify` failed: a stable code and a human-readable message.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifyFailure {
    /// The stable code (`deserialize`, `empty_name`, ...).
    pub code: &'static str,
    /// The human-readable message.
    pub message: String,
}

/// One instrument, in the manifest's JSON shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestInstrument {
    /// The trading symbol.
    pub symbol: String,
    /// The exchange code.
    pub exchange: String,
}

/// The universe the strategy may trade.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestUniverse {
    /// A named universe, resolved by the loader before running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub named: Option<String>,
    /// An explicit list of instruments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit: Option<Vec<ManifestInstrument>>,
}

/// The market data the strategy consumes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestSubscriptions {
    /// The instruments subscribed to.
    #[serde(default)]
    pub instruments: Vec<ManifestInstrument>,
    /// Whether quotes are consumed.
    #[serde(default)]
    pub quotes: bool,
    /// Whether trades are consumed.
    #[serde(default)]
    pub trades: bool,
}

/// A bar aggregation interval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeframeSpec {
    /// The interval, in the aggregation's unit.
    pub interval: u32,
    /// The aggregation (`minute`, `day`, ...).
    pub aggregation: String,
}

/// A parameter declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterSpec {
    /// The default value.
    pub default: serde_json::Value,
    /// The sweep range, `min:max:step`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    /// The lower bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The upper bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// The step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// What the parameter is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A strategy manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrategyManifest {
    /// The strategy API version the manifest was written against.
    pub api_version: String,
    /// The strategy's name.
    pub name: String,
    /// The digest of the strategy's source.
    pub source_hash: String,
    /// The universe the strategy may trade.
    pub universe: ManifestUniverse,
    /// The market data the strategy consumes.
    pub subscriptions: ManifestSubscriptions,
    /// The timeframe the strategy is driven by.
    pub driving_timeframe: TimeframeSpec,
    /// The bars consumed before the first decision.
    #[serde(default)]
    pub warmup_bars: u32,
    /// The scheduled actions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub schedules: BTreeMap<String, String>,
    /// The strategy's parameters.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterSpec>,
}

/// The resolved universe of the IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IrUniverse {
    /// The named universe, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named: Option<String>,
    /// The concrete instruments.
    pub instruments: Vec<ManifestInstrument>,
}

/// The resolved subscriptions of the IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IrSubscriptions {
    /// The instruments subscribed to as bars.
    pub bars: Vec<ManifestInstrument>,
    /// The instruments subscribed to as quotes.
    pub quotes: Vec<ManifestInstrument>,
    /// The instruments subscribed to as trades.
    pub trades: Vec<ManifestInstrument>,
}

/// The compiled strategy IR.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StrategyIr {
    /// The schema version of the IR.
    pub schema_version: u32,
    /// The strategy API version.
    pub strategy_api_version: String,
    /// The manifest the IR was compiled from.
    pub manifest: StrategyManifest,
    /// The resolved universe.
    pub universe: IrUniverse,
    /// The resolved subscriptions.
    pub subscriptions: IrSubscriptions,
    /// The driving timeframe.
    pub driving_timeframe: TimeframeSpec,
    /// Every timeframe to load.
    pub timeframes: Vec<TimeframeSpec>,
    /// The bars consumed before the first decision.
    pub warmup_bars: u32,
}

/// The stable code of a manifest failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    /// The strategy has no name.
    EmptyName,
    /// The source digest is missing.
    EmptySourceHash,
    /// The manifest was written against another API version.
    UnsupportedApiVersion,
    /// The driving timeframe has a zero interval.
    ZeroInterval,
    /// A named universe was never resolved to instruments.
    NamedUniverseUnresolved,
    /// The strategy subscribes to nothing.
    NoSubscriptions,
    /// A subscribed instrument is outside the universe.
    SubscriptionOutsideUniverse,
    /// A parameter's range is malformed.
    InvalidParameterRange,
    /// A parameter's default is outside its range.
    ParameterDefaultOutOfRange,
}

impl ManifestError {
    /// The stable code of this failure.
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyName => "empty_name",
            Self::EmptySourceHash => "empty_source_hash",
            Self::UnsupportedApiVersion => "unsupported_api_version",
            Self::ZeroInterval => "zero_interval",
            Self::NamedUniverseUnresolved => "named_universe_unresolved",
            Self::NoSubscriptions => "no_subscriptions",
            Self::SubscriptionOutsideUniverse => "subscription_outside_universe",
            Self::InvalidParameterRange => "invalid_parameter_range",
            Self::ParameterDefaultOutOfRange => "parameter_default_out_of_range",
        }
    }
}

/// Checks a manifest's internal consistency.
pub fn validate(manifest: &StrategyManifest) -> Result<(), ManifestError> {
    if manifest.name.trim().is_empty() {
        return Err(ManifestError::EmptyName);
    }
    if manifest.source_hash.trim().is_empty() {
        return Err(ManifestError::EmptySourceHash);
    }
    if manifest.api_version != STRATEGY_API_VERSION {
        return Err(ManifestError::UnsupportedApiVersion);
    }
    if manifest.driving_timeframe.interval == 0 {
        return Err(ManifestError::ZeroInterval);
    }
    if manifest.universe.named.is_some() && manifest.subscriptions.instruments.is_empty() {
        return Err(ManifestError::NamedUniverseUnresolved);
    }
    for (name, parameter) in &manifest.parameters {
        if let (Some(min), Some(max)) = (parameter.min, parameter.max) {
            if min > max {
                return Err(ManifestError::InvalidParameterRange);
            }
        }
        if parameter.step.is_some_and(|step| step <= 0.0) {
            return Err(ManifestError::InvalidParameterRange);
        }
        if let Some(default) = parameter.default.as_f64() {
            if parameter.min.is_some_and(|min| default < min) {
                return Err(ManifestError::ParameterDefaultOutOfRange);
            }
            if parameter.max.is_some_and(|max| default > max) {
                return Err(ManifestError::ParameterDefaultOutOfRange);
            }
        }
        let _ = name;
    }
    Ok(())
}

/// Compiles a manifest into its IR.
pub fn compile(manifest: StrategyManifest) -> Result<StrategyIr, ManifestError> {
    validate(&manifest)?;

    // A named universe is resolved by the loader into an explicit list of
    // instruments; the compiler only ever sees the explicit form.
    if manifest.universe.named.is_some() {
        return Err(ManifestError::NamedUniverseUnresolved);
    }

    let instruments = manifest.subscriptions.instruments.clone();
    if instruments.is_empty() {
        return Err(ManifestError::NoSubscriptions);
    }

    // An explicit universe is a hard boundary: a subscription outside it is a
    // manifest error, not a warning.
    if let Some(explicit) = &manifest.universe.explicit {
        for subscribed in &instruments {
            if !explicit.contains(subscribed) {
                return Err(ManifestError::SubscriptionOutsideUniverse);
            }
        }
    }

    let mut universe_instruments = instruments.clone();
    if let Some(explicit) = &manifest.universe.explicit {
        for instrument in explicit {
            if !universe_instruments.contains(instrument) {
                universe_instruments.push(instrument.clone());
            }
        }
    }

    let quotes = if manifest.subscriptions.quotes {
        instruments.clone()
    } else {
        Vec::new()
    };
    let trades = if manifest.subscriptions.trades {
        instruments.clone()
    } else {
        Vec::new()
    };

    Ok(StrategyIr {
        schema_version: MANIFEST_SCHEMA_VERSION,
        strategy_api_version: STRATEGY_API_VERSION.to_string(),
        driving_timeframe: manifest.driving_timeframe.clone(),
        timeframes: vec![manifest.driving_timeframe.clone()],
        warmup_bars: manifest.warmup_bars,
        universe: IrUniverse {
            named: manifest.universe.named.clone(),
            instruments: universe_instruments,
        },
        subscriptions: IrSubscriptions {
            bars: instruments,
            quotes,
            trades,
        },
        manifest,
    })
}

/// Parses manifest JSON and compiles it; returns the IR as JSON.
pub fn verify(manifest_json: &str) -> Result<String, VerifyFailure> {
    let manifest: StrategyManifest =
        serde_json::from_str(manifest_json).map_err(|e| VerifyFailure {
            code: "deserialize",
            message: e.to_string(),
        })?;
    let ir = compile(manifest).map_err(|e| VerifyFailure {
        code: e.code(),
        message: e.code().to_string(),
    })?;
    serde_json::to_string(&ir).map_err(|e| VerifyFailure {
        code: "serialize",
        message: e.to_string(),
    })
}

/// Compile a strategy manifest (JSON) into its IR (JSON).
///
/// Raises `ValueError("<code>: <message>")`, where `code` is `deserialize` or
/// the manifest error code.
#[pyfunction]
pub fn verify_manifest(manifest_json: &str) -> PyResult<String> {
    verify(manifest_json).map_err(|f| PyValueError::new_err(format!("{}: {}", f.code, f.message)))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("STRATEGY_API_VERSION", STRATEGY_API_VERSION)?;
    m.add_function(wrap_pyfunction!(verify_manifest, m)?)?;
    Ok(())
}
