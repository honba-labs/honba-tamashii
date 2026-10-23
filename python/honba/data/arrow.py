"""Zero-copy Arrow PyCapsule protocol & Parquet Lakehouse integration for ``honba.data``.

Connects ``honba.data`` directly to ``honba-me-feed`` Parquet Lakehouse v2 datasets
and ``honba-me-loaders`` NSE Bhavcopy parsers, providing zero-copy export to
PyArrow (``pyarrow.Table``) and Polars (``polars.DataFrame``) via the Arrow
PyCapsule Interface (``__arrow_c_stream__`` / ``__arrow_c_schema__``).
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any, Sequence

from honba._honba import (
    ArrowStream,
    Bar,
    BhavRecord,
    Lakehouse,
    Tick,
    bars_to_arrow,
    bhavcopy_to_arrow,
    bhavcopy_to_bars,
    export_capsule,
    parse_bhavcopy_csv,
    protocol_capsule,
    ticks_to_arrow,
)

__all__ = [
    "ArrowStream",
    "BhavRecord",
    "Lakehouse",
    "bars_to_arrow",
    "bhavcopy_to_arrow",
    "bhavcopy_to_bars",
    "export_capsule",
    "load_bars",
    "load_bhavcopy_day",
    "load_ticks",
    "parse_bhavcopy_csv",
    "protocol_capsule",
    "ticks_to_arrow",
    "to_polars",
    "to_pyarrow",
]


def _as_arrow_stream(data: Any) -> ArrowStream:
    if isinstance(data, ArrowStream):
        return data
    if isinstance(data, Sequence):
        items = list(data)
        if not items:
            return bars_to_arrow([])
        if isinstance(items[0], Bar):
            return bars_to_arrow(items)
        if isinstance(items[0], Tick):
            return ticks_to_arrow(items)
    raise TypeError(
        f"expected ArrowStream, Sequence[Bar], or Sequence[Tick], got {type(data).__name__}"
    )


def to_pyarrow(data: Any) -> Any:
    """Export an ``ArrowStream``, ``Sequence[Bar]``, or ``Sequence[Tick]`` to a ``pyarrow.Table`` with zero copies."""
    if hasattr(data, "to_pyarrow"):
        return data.to_pyarrow()
    if hasattr(data, "__arrow_c_stream__"):
        import pyarrow as pa

        if hasattr(pa, "table"):
            return pa.table(data)
        return pa.RecordBatchReader.from_stream(data).read_all()
    stream = _as_arrow_stream(data)
    return stream.to_pyarrow()


def to_polars(data: Any) -> Any:
    """Export an ``ArrowStream``, ``Sequence[Bar]``, or ``Sequence[Tick]`` to a ``polars.DataFrame`` with zero copies."""
    try:
        import polars as pl
    except ImportError as exc:
        raise ImportError("polars is required for to_polars()") from exc

    if hasattr(data, "__arrow_c_stream__"):
        return pl.from_arrow(data)
    stream = _as_arrow_stream(data)
    return pl.from_arrow(stream)


def _resolve_lakehouse(root: str | Path | Lakehouse | None) -> Lakehouse:
    if isinstance(root, Lakehouse):
        return root
    resolved = root or os.environ.get("HONBA_LAKEHOUSE_ROOT", "./data/lakehouse")
    return Lakehouse(str(resolved))


def load_bars(
    instrument: Any,
    start: Any = "2020-01-01",
    end: Any = "2099-12-31",
    timeframe: str = "1m",
    *,
    root: str | Path | Lakehouse | None = None,
    backend: str = "auto",
) -> Any:
    """Load canonical OHLCV bars from the ``honba-me-feed`` Parquet Lakehouse.

    Parameters
    ----------
    instrument : InstrumentId or str
        Canonical instrument identifier (e.g. ``"NSE:RELIANCE"`` or ``"RELIANCE"``).
    start : str or datetime
        Start date/timestamp (inclusive), e.g. ``"2026-01-01"``.
    end : str or datetime
        End date/timestamp (inclusive), e.g. ``"2026-12-31"``.
    timeframe : str
        Bar timeframe (``"1m"``, ``"5m"``, ``"15m"``, ``"30m"``, ``"1h"``, ``"1d"``, ``"1w"``).
    root : str, Path, or Lakehouse, optional
        Lakehouse directory path or ``Lakehouse`` instance.
    backend : {"auto", "polars", "pyarrow", "arrow"}
        Output container format. ``"auto"`` returns a ``polars.DataFrame`` when
        Polars is installed, otherwise a ``pyarrow.Table``.
    """
    lh = _resolve_lakehouse(root)
    stream = lh.load_bars_arrow(instrument, timeframe, start, end)
    mode = backend.lower()
    if mode in ("arrow", "stream", "capsule"):
        return stream
    if mode == "polars":
        return stream.to_polars()
    if mode == "pyarrow":
        return stream.to_pyarrow()
    if mode == "auto":
        try:
            import polars  # noqa: F401

            return stream.to_polars()
        except ImportError:
            return stream.to_pyarrow()
    raise ValueError(f"unsupported backend {backend!r}; expected 'auto', 'polars', 'pyarrow', or 'arrow'")


def load_ticks(
    instrument: Any,
    start: Any = "2020-01-01",
    end: Any = "2099-12-31",
    *,
    root: str | Path | Lakehouse | None = None,
    backend: str = "auto",
) -> Any:
    """Load canonical ticks from the ``honba-me-feed`` Parquet Lakehouse via zero-copy Arrow."""
    lh = _resolve_lakehouse(root)
    stream = lh.load_ticks_arrow(instrument, start, end)
    mode = backend.lower()
    if mode in ("arrow", "stream", "capsule"):
        return stream
    if mode == "polars":
        return stream.to_polars()
    if mode == "pyarrow":
        return stream.to_pyarrow()
    if mode == "auto":
        try:
            import polars  # noqa: F401

            return stream.to_polars()
        except ImportError:
            return stream.to_pyarrow()
    raise ValueError(f"unsupported backend {backend!r}; expected 'auto', 'polars', 'pyarrow', or 'arrow'")


def load_bhavcopy_day(
    csv_or_path: str | Path,
    fallback_date: str | None = None,
    *,
    delivery_dat: str | None = None,
    series: str | None = "EQ",
    as_arrow: bool = False,
) -> list[BhavRecord] | ArrowStream:
    """Parse an NSE daily Bhavcopy CSV (bridging ``honba-me-loaders`` into Python).

    Parameters
    ----------
    csv_or_path : str or Path
        Either a filesystem path to a ``sec_bhavdata_full_*.csv`` file or the raw CSV text.
    fallback_date : str, optional
        Fallback ``YYYY-MM-DD`` date if ``DATE1`` cannot be parsed.
    delivery_dat : str, optional
        Optional raw ``MTO_*.DAT`` delivery report text to merge into the records.
    series : str, optional
        Series filter when ``as_arrow=True`` (defaults to ``"EQ"``).
    as_arrow : bool, default=False
        When ``True``, returns a zero-copy ``ArrowStream`` of the parsed daily bars.
    """
    if isinstance(csv_or_path, Path):
        csv_text = csv_or_path.read_text(encoding="utf-8")
    elif "\n" not in csv_or_path and Path(csv_or_path).is_file():
        csv_text = Path(csv_or_path).read_text(encoding="utf-8")
    else:
        csv_text = csv_or_path

    if as_arrow:
        return bhavcopy_to_arrow(csv_text, fallback_date=fallback_date, series=series)
    return parse_bhavcopy_csv(csv_text, fallback_date=fallback_date, delivery_dat=delivery_dat)
