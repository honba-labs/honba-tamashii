"""``honba serve`` and ``honba migrate``: launch the REST/WS server and run DB migrations.

Thin Typer front ends over ``honba._honba.serve_api`` (``honba_api_rest::run_server_blocking``)
and ``honba._honba.run_migrations`` (``honba_persistence::connect_and_migrate``). Both release
the Python GIL while Tokio runs in Rust.
"""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer

from honba._native import native_attr


def serve(
    data_dir: Annotated[
        Path,
        typer.Option(
            "--data-dir",
            help="Directory holding SYMBOL.EXCHANGE.parquet files or partitioned catalog/.",
        ),
    ],
    addr: Annotated[
        str,
        typer.Option(
            "--addr",
            help="host:port to bind (127.0.0.1:0 picks an OS-assigned port).",
        ),
    ] = "127.0.0.1:8000",
    cors_origin: Annotated[
        list[str] | None,
        typer.Option(
            "--cors-origin",
            help="Browser origin allowed by CORS (repeatable).",
        ),
    ] = None,
    trading_state: Annotated[
        str,
        typer.Option(
            "--trading-state",
            help="Initial trading state: active, reducing, or halted.",
        ),
    ] = "active",
    journals_dir: Annotated[
        Path,
        typer.Option(
            "--journals-dir",
            help="Directory holding one append-only <run_id>.ndjson journal per run.",
        ),
    ] = Path("data/journals"),
    profiles_dir: Annotated[
        Path,
        typer.Option(
            "--profiles-dir",
            help="Directory holding one <id>.json trader profile document per trader.",
        ),
    ] = Path("data/profiles"),
    portfolios_dir: Annotated[
        Path,
        typer.Option(
            "--portfolios-dir",
            help="Directory holding one <id>.json portfolio document per account.",
        ),
    ] = Path("data/portfolios"),
    operator_journal: Annotated[
        Path | None,
        typer.Option(
            "--operator-journal",
            help="Append-only NDJSON file of operator control events.",
        ),
    ] = None,
    max_concurrent_runs: Annotated[
        int | None,
        typer.Option(
            "--max-concurrent-runs",
            help="Worker threads executing runs at once (default: server default).",
        ),
    ] = None,
    max_queued_runs: Annotated[
        int,
        typer.Option(
            "--max-queued-runs",
            help="Bound on runs waiting in Pending before POST /backtests returns 429.",
        ),
    ] = 64,
    shutdown_grace_secs: Annotated[
        int,
        typer.Option(
            "--shutdown-grace-secs",
            help="Seconds a graceful shutdown waits for Running runs to finish.",
        ),
    ] = 10,
    keep_runs: Annotated[
        int,
        typer.Option(
            "--keep-runs",
            help="Maximum terminal run journals kept on disk across startups (0 keeps none).",
        ),
    ] = 1000,
    keep_days: Annotated[
        int,
        typer.Option(
            "--keep-days",
            help="Maximum age in days of terminal run journals kept on disk (0 disables).",
        ),
    ] = 30,
    tokens_file: Annotated[
        Path | None,
        typer.Option(
            "--tokens-file",
            help="Static bearer token config JSON (BearerAuthLayer::from_json).",
        ),
    ] = None,
    audit_log_file: Annotated[
        Path | None,
        typer.Option(
            "--audit-log-file",
            help="Append-only NDJSON audit log file for bearer-authenticated requests.",
        ),
    ] = None,
    jwt_secret: Annotated[
        str | None,
        typer.Option(
            "--jwt-secret",
            envvar="HONBA_JWT_SECRET",
            help="HMAC-SHA256 shared secret for JWT bearer authentication.",
        ),
    ] = None,
    jwt_rsa_key: Annotated[
        Path | None,
        typer.Option(
            "--jwt-rsa-key",
            help="RSA PEM public key path for RS256 JWT bearer authentication.",
        ),
    ] = None,
    jwt_issuer: Annotated[
        str,
        typer.Option(
            "--jwt-issuer",
            help="Expected JWT `iss` claim when --jwt-secret or --jwt-rsa-key is active.",
        ),
    ] = "honba",
    jwt_audience: Annotated[
        str,
        typer.Option(
            "--jwt-audience",
            help="Expected JWT `aud` claim when --jwt-secret or --jwt-rsa-key is active.",
        ),
    ] = "honba-api",
    bar_ingest: Annotated[
        bool,
        typer.Option(
            "--bar-ingest",
            help="Enable POST /market/bars live bar ingestion endpoint.",
        ),
    ] = False,
    bar_ingest_capacity: Annotated[
        int,
        typer.Option(
            "--bar-ingest-capacity",
            help="Per-instrument ring buffer capacity for POST /market/bars.",
        ),
    ] = 1024,
    risk_config: Annotated[
        Path | None,
        typer.Option(
            "--risk-config",
            help="Optional TOML risk limits config ([risk] table or flat RiskConfig).",
        ),
    ] = None,
    fill_model: Annotated[
        str | None,
        typer.Option(
            "--fill-model",
            help="Server default fill model: bar_fill, bar_fill_through, or l2.",
        ),
    ] = None,
    ticks: Annotated[
        Path | None,
        typer.Option(
            "--ticks",
            help="Optional NDJSON ticks file (QuoteTick / TradeTick, ADR 0030) loaded at startup.",
        ),
    ] = None,
) -> None:
    """Start the Honba REST & WebSocket API server (`honba-ya-api-rest`)."""
    for flag_name, num_val in (
        ("--max-concurrent-runs", max_concurrent_runs),
        ("--max-queued-runs", max_queued_runs),
        ("--shutdown-grace-secs", shutdown_grace_secs),
        ("--keep-runs", keep_runs),
        ("--keep-days", keep_days),
        ("--bar-ingest-capacity", bar_ingest_capacity),
    ):
        if num_val is not None and num_val < 0:
            typer.echo(f"error: {flag_name} must be >= 0", err=True)
            raise typer.Exit(code=2)

    try:
        serve_fn = native_attr("serve_api")
        serve_fn(
            str(data_dir),
            addr=addr,
            cors_origins=list(cors_origin) if cors_origin else None,
            trading_state=trading_state,
            journals_dir=str(journals_dir),
            profiles_dir=str(profiles_dir),
            portfolios_dir=str(portfolios_dir),
            operator_journal=str(operator_journal) if operator_journal is not None else None,
            max_concurrent_runs=max_concurrent_runs,
            max_queued_runs=max_queued_runs,
            shutdown_grace_secs=shutdown_grace_secs,
            keep_runs=keep_runs,
            keep_days=keep_days,
            tokens_file=str(tokens_file) if tokens_file is not None else None,
            audit_log_file=str(audit_log_file) if audit_log_file is not None else None,
            jwt_secret=jwt_secret,
            jwt_rsa_key=str(jwt_rsa_key) if jwt_rsa_key is not None else None,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            bar_ingest=bar_ingest,
            bar_ingest_capacity=bar_ingest_capacity,
            risk_config=str(risk_config) if risk_config is not None else None,
            fill_model=fill_model,
            ticks=str(ticks) if ticks is not None else None,
        )
    except ValueError as exc:
        typer.echo(f"error: {exc}", err=True)
        raise typer.Exit(code=2) from None
    except (ImportError, RuntimeError) as exc:
        typer.echo(f"error: {exc}", err=True)
        raise typer.Exit(code=1) from None


def migrate(
    database_url: Annotated[
        str,
        typer.Option(
            "--database-url",
            envvar="DATABASE_URL",
            help="SQLite (`sqlite://honba.db`) or PostgreSQL (`postgres://...`) connection URL.",
        ),
    ] = "sqlite://honba.db",
    rollback: Annotated[
        bool,
        typer.Option(
            "--rollback",
            help="Roll back the most recently applied migration instead of applying pending ones.",
        ),
    ] = False,
) -> None:
    """Apply or roll back relational schema migrations (`honba-ya-persistence`)."""
    try:
        migrate_fn = native_attr("run_migrations")
        versions: list[int] = migrate_fn(database_url=database_url, rollback=rollback)
    except (ImportError, RuntimeError, ValueError) as exc:
        typer.echo(f"error: {exc}", err=True)
        raise typer.Exit(code=1) from None

    if rollback:
        if versions:
            typer.echo(f"Rolled back migration version {versions[0]} on {database_url}")
        else:
            typer.echo(f"No applied migrations to roll back on {database_url}")
    else:
        if versions:
            typer.echo(
                f"Applied {len(versions)} migration(s) ({versions}) on {database_url}"
            )
        else:
            typer.echo(
                f"Database schema up to date on {database_url} (0 pending migrations)"
            )
