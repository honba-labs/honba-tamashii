"""End-to-end and unit tests for ``honba serve``, ``honba migrate``, and ``honba data ticks-import``."""

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import sys
import urllib.request
from pathlib import Path

from honba.cli.main import app
from typer.testing import CliRunner

runner = CliRunner()


def test_serve_and_migrate_help() -> None:
    res_serve = runner.invoke(app, ["serve", "--help"])
    assert res_serve.exit_code == 0, res_serve.output
    assert "--data-dir" in res_serve.output
    assert "--trading-state" in res_serve.output
    assert "--jwt-secret" in res_serve.output
    assert "--fill-model" in res_serve.output

    res_migrate = runner.invoke(app, ["migrate", "--help"])
    assert res_migrate.exit_code == 0, res_migrate.output
    assert "--database-url" in res_migrate.output
    assert "--rollback" in res_migrate.output

    res_ticks = runner.invoke(app, ["data", "ticks-import", "--help"])
    assert res_ticks.exit_code == 0, res_ticks.output
    assert "--ticks" in res_ticks.output
    assert "--out" in res_ticks.output


def test_serve_validation_errors(tmp_path: Path) -> None:
    # Missing required --data-dir exits non-zero
    missing_dir = runner.invoke(app, ["serve"])
    assert missing_dir.exit_code != 0

    # Invalid --addr exits with usage code 2
    bad_addr = runner.invoke(
        app, ["serve", "--data-dir", str(tmp_path), "--addr", "not-a-socket"]
    )
    assert bad_addr.exit_code == 2
    assert "invalid --addr" in bad_addr.output

    # Invalid --trading-state exits with usage code 2
    bad_state = runner.invoke(
        app, ["serve", "--data-dir", str(tmp_path), "--trading-state", "paused"]
    )
    assert bad_state.exit_code == 2
    assert "expected active, reducing or halted" in bad_state.output

    # Invalid --fill-model exits with usage code 2
    bad_fill = runner.invoke(
        app, ["serve", "--data-dir", str(tmp_path), "--fill-model", "bogus"]
    )
    assert bad_fill.exit_code == 2
    assert "unknown fill model" in bad_fill.output

    # Non-existent --data-dir exits with runtime error code 1
    nonexistent = runner.invoke(
        app,
        [
            "serve",
            "--data-dir",
            str(tmp_path / "does_not_exist"),
            "--addr",
            "127.0.0.1:0",
        ],
    )
    assert nonexistent.exit_code == 1


def test_migrate_sqlite_apply_and_rollback(tmp_path: Path) -> None:
    db_file = tmp_path / "honba_cli_test.db"
    db_url = f"sqlite://{db_file}?mode=rwc"

    apply_first = runner.invoke(app, ["migrate", "--database-url", db_url])
    assert apply_first.exit_code == 0, apply_first.output
    assert "Applied" in apply_first.output

    apply_second = runner.invoke(app, ["migrate", "--database-url", db_url])
    assert apply_second.exit_code == 0, apply_second.output
    assert "0 pending migrations" in apply_second.output

    rollback_res = runner.invoke(
        app, ["migrate", "--database-url", db_url, "--rollback"]
    )
    assert rollback_res.exit_code == 0, rollback_res.output
    assert "Rolled back migration version" in rollback_res.output


def test_data_ticks_import_ndjson(tmp_path: Path) -> None:
    ticks_ndjson = tmp_path / "sample_ticks.ndjson"
    out_dir = tmp_path / "store"
    lines = [
        json.dumps({"honba_ticks": 1}),
        json.dumps(
            {
                "kind": "quote",
                "symbol": "TCS",
                "exchange": "NSE",
                "ts_event": 1_704_067_200_000_000_000,
                "bid_price": 99.9,
                "ask_price": 100.1,
                "bid_size": 500,
                "ask_size": 400,
            }
        ),
        json.dumps(
            {
                "kind": "trade",
                "symbol": "TCS",
                "exchange": "NSE",
                "ts_event": 1_704_067_200_100_000_000,
                "price": 100.0,
                "size": 50,
                "aggressor": "buyer",
                "trade_id": "T-1",
            }
        ),
    ]
    ticks_ndjson.write_text("\n".join(lines) + "\n", encoding="utf-8")

    res = runner.invoke(
        app,
        ["data", "ticks-import", "--ticks", str(ticks_ndjson), "--out", str(out_dir)],
    )
    assert res.exit_code == 0, res.output
    assert "imported 1 quote tick(s) and 1 trade tick(s)" in res.output
    assert (out_dir / "ticks" / "ticks.parquet").is_file()


def test_serve_ephemeral_port_health_and_sigint_clean_shutdown(tmp_path: Path) -> None:
    data_dir = tmp_path / "data"
    data_dir.mkdir()
    journals_dir = tmp_path / "journals"
    profiles_dir = tmp_path / "profiles"
    portfolios_dir = tmp_path / "portfolios"

    env = os.environ.copy()
    python_src = str(Path(__file__).resolve().parent.parent / "python")
    existing_pp = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = f"{python_src}:{existing_pp}" if existing_pp else python_src

    proc = subprocess.Popen(
        [
            sys.executable,
            "-u",
            "-m",
            "honba.cli.main",
            "serve",
            "--data-dir",
            str(data_dir),
            "--addr",
            "127.0.0.1:0",
            "--journals-dir",
            str(journals_dir),
            "--profiles-dir",
            str(profiles_dir),
            "--portfolios-dir",
            str(portfolios_dir),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    try:
        assert proc.stdout is not None
        first_line = proc.stdout.readline().strip()
        match = re.search(r"listening on (?:http://)?(127\.0\.0\.1:\d+)", first_line)
        assert match is not None, f"unexpected startup stdout: {first_line!r}"
        bound_addr = match.group(1)

        with urllib.request.urlopen(f"http://{bound_addr}/health", timeout=5.0) as resp:
            assert resp.status == 200
            payload = json.loads(resp.read().decode("utf-8"))
            status_val = payload.get("data", payload).get("status")
            assert status_val == "ok", f"unexpected /health payload: {payload!r}"

        proc.send_signal(signal.SIGINT)
        rc = proc.wait(timeout=10.0)
        stderr_out = proc.stderr.read() if proc.stderr is not None else ""
        assert (
            rc == 0
        ), f"expected clean exit 0 on SIGINT, got {rc}; stderr:\n{stderr_out}"
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5.0)
