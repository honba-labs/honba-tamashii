//! PyO3 bindings for `honba serve`, `honba migrate`, and NDJSON tick store import.
//!
//! All blocking/Tokio execution releases the Python GIL via `py.allow_threads(...)`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use honba_api_rest::{
    ServeAuthConfig, ServeConfig, ServeFillConfig, ServeIngestConfig, ServeRiskConfig,
    ServeRunsConfig, parse_fill_model, parse_trading_state, run_server_blocking,
};
use honba_persistence::{MigrationOutcome, connect_and_migrate};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Register server and persistence functions on `honba._honba`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(serve_api, m)?)?;
    m.add_function(wrap_pyfunction!(run_migrations, m)?)?;
    m.add_function(wrap_pyfunction!(import_ticks_ndjson, m)?)?;
    Ok(())
}

/// Launch the `honba-ya-api-rest` server until `SIGINT` (`Ctrl-C`) or `SIGTERM`.
///
/// Validation errors (`addr`, `trading_state`, `fill_model`) raise `ValueError`;
/// startup I/O or runtime errors raise `RuntimeError` with full error chain context.
#[pyfunction]
#[pyo3(signature = (
    data_dir,
    addr="127.0.0.1:8000",
    cors_origins=None,
    trading_state="active",
    journals_dir="data/journals",
    profiles_dir="data/profiles",
    portfolios_dir="data/portfolios",
    operator_journal=None,
    max_concurrent_runs=None,
    max_queued_runs=64,
    shutdown_grace_secs=10,
    keep_runs=1000,
    keep_days=30,
    tokens_file=None,
    audit_log_file=None,
    jwt_secret=None,
    jwt_rsa_key=None,
    jwt_issuer="honba",
    jwt_audience="honba-api",
    bar_ingest=false,
    bar_ingest_capacity=1024,
    risk_config=None,
    fill_model=None,
    ticks=None,
))]
#[allow(clippy::too_many_arguments)]
pub fn serve_api(
    py: Python<'_>,
    data_dir: &str,
    addr: &str,
    cors_origins: Option<Vec<String>>,
    trading_state: &str,
    journals_dir: &str,
    profiles_dir: &str,
    portfolios_dir: &str,
    operator_journal: Option<String>,
    max_concurrent_runs: Option<u64>,
    max_queued_runs: u64,
    shutdown_grace_secs: u64,
    keep_runs: u64,
    keep_days: u64,
    tokens_file: Option<String>,
    audit_log_file: Option<String>,
    jwt_secret: Option<String>,
    jwt_rsa_key: Option<String>,
    jwt_issuer: &str,
    jwt_audience: &str,
    bar_ingest: bool,
    bar_ingest_capacity: u64,
    risk_config: Option<String>,
    fill_model: Option<String>,
    ticks: Option<String>,
) -> PyResult<()> {
    let parsed_addr: SocketAddr = addr
        .parse()
        .map_err(|e| PyValueError::new_err(format!("invalid --addr {addr:?}: {e}")))?;
    let parsed_state = parse_trading_state(trading_state).map_err(PyValueError::new_err)?;
    let parsed_fill_model = match fill_model.as_deref() {
        Some(raw) => Some(parse_fill_model(raw).map_err(PyValueError::new_err)?),
        None => None,
    };

    let config = ServeConfig {
        data_dir: PathBuf::from(data_dir),
        addr: parsed_addr,
        cors_origins: cors_origins.unwrap_or_default(),
        trading_state: parsed_state,
        runs: ServeRunsConfig {
            journals_dir: PathBuf::from(journals_dir),
            profiles_dir: PathBuf::from(profiles_dir),
            portfolios_dir: PathBuf::from(portfolios_dir),
            operator_journal: operator_journal.map(PathBuf::from),
            max_concurrent_runs,
            max_queued_runs,
            shutdown_grace_secs,
            keep_runs,
            keep_days,
        },
        auth: ServeAuthConfig {
            tokens_file: tokens_file.map(PathBuf::from),
            audit_log_file: audit_log_file.map(PathBuf::from),
            jwt_secret,
            jwt_rsa_key: jwt_rsa_key.map(PathBuf::from),
            jwt_issuer: jwt_issuer.to_string(),
            jwt_audience: jwt_audience.to_string(),
        },
        ingest: ServeIngestConfig {
            bar_ingest,
            bar_ingest_capacity,
        },
        risk: ServeRiskConfig {
            risk_config: risk_config.map(PathBuf::from),
        },
        fill: ServeFillConfig {
            fill_model: parsed_fill_model,
            ticks: ticks.map(PathBuf::from),
        },
    };

    config
        .runs
        .validate()
        .map_err(|e| PyValueError::new_err(format!("{e:#}")))?;
    config
        .ingest
        .validate()
        .map_err(|e| PyValueError::new_err(format!("{e:#}")))?;

    let result = py.allow_threads(|| run_server_blocking(&config));

    // If SIGINT triggered Tokio's graceful shutdown, signal-hook may also have chained into
    // Python's C SIGINT flag; clear any queued KeyboardInterrupt when shutdown succeeded cleanly.
    if result.is_ok() {
        let _ = py.check_signals();
        PyErr::take(py);
    }

    result.map_err(|e| PyRuntimeError::new_err(format!("{e:#}")))
}

/// Apply pending relational schema migrations (or roll back the most recent one when
/// `rollback=True`) via `honba-ya-persistence`.
///
/// Returns the migration version numbers applied or rolled back.
#[pyfunction]
#[pyo3(signature = (database_url="sqlite://honba.db", rollback=false))]
pub fn run_migrations(py: Python<'_>, database_url: &str, rollback: bool) -> PyResult<Vec<i64>> {
    let url = database_url.to_string();
    let res: Result<Vec<i64>, String> = py.allow_threads(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("starting tokio runtime for migrations: {e}"))?;
        rt.block_on(async {
            match connect_and_migrate(&url, rollback)
                .await
                .map_err(|e| format!("{e}"))?
            {
                MigrationOutcome::Applied(versions) => Ok(versions),
                MigrationOutcome::RolledBack(Some(v)) => Ok(vec![v]),
                MigrationOutcome::RolledBack(None) => Ok(Vec::new()),
            }
        })
    });
    res.map_err(PyRuntimeError::new_err)
}

/// Import an ADR 0030 NDJSON ticks file into `{out_dir}/ticks/ticks.parquet`, returning
/// `(quote_count, trade_count)`.
#[pyfunction]
#[pyo3(signature = (ticks_path, out_dir))]
pub fn import_ticks_ndjson(
    py: Python<'_>,
    ticks_path: &str,
    out_dir: &str,
) -> PyResult<(usize, usize)> {
    let ticks = PathBuf::from(ticks_path);
    let out = PathBuf::from(out_dir);
    py.allow_threads(move || {
        honba_api_rest::import_ticks_ndjson(Path::new(&ticks), Path::new(&out))
    })
    .map_err(|e| PyRuntimeError::new_err(format!("{e:#}")))
}
