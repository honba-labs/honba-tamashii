"""NSE Bhavcopy downloader and MarketDataProvider implementation (Design.md Section 12.4).

Bridges ``honba-me-loaders`` (`parse_bhavcopy_csv`, `BhavRecord`, `bhavcopy_to_bars`,
`bhavcopy_to_arrow`) into Python alongside the legacy `NseBhavcopyProvider`.
"""

from __future__ import annotations

import csv
import datetime as dt
import io
import logging
import lzma
import zipfile
from pathlib import Path
from typing import Any

from honba._honba import (
    BhavRecord,
    bhavcopy_to_arrow,
    bhavcopy_to_bars,
)
from honba._honba import (
    parse_bhavcopy_csv as parse_bhavcopy_records,
)
from honba.domain.bar import Bar
from honba.domain.instrument import InstrumentId
from honba.markets.india.calendar import IST, NseCalendar
from honba_data.store.coverage import DateInterval

logger = logging.getLogger(__name__)


def validate_bar(bar: Bar) -> None:
    """Validate basic OHLCV bar invariants."""
    if bar.high < bar.low:
        raise ValueError(f"bar high ({bar.high}) < low ({bar.low})")
    if bar.volume < 0:
        raise ValueError(f"bar volume ({bar.volume}) < 0")


# Primary User-Agent headers to access NSE public archives
DEFAULT_HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36"
    ),
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.5",
}


def parse_bhavcopy_csv(csv_content: str, exchange: str = "NSE") -> dict[str, Bar]:
    """Parse NSE Bhavcopy CSV text and return map of symbol -> Bar."""
    f = io.StringIO(csv_content)
    reader = csv.DictReader(f)
    if not reader.fieldnames:
        return {}

    field_map = {fn.strip().upper(): fn for fn in reader.fieldnames if fn}

    sym_col = field_map.get("TCKRSYMB") or field_map.get("SYMBOL")
    series_col = field_map.get("SCTYSRS") or field_map.get("SERIES")
    open_col = (
        field_map.get("OPNPRIC") or field_map.get("OPEN_PRICE") or field_map.get("OPEN")
    )
    high_col = (
        field_map.get("HGHPRIC") or field_map.get("HIGH_PRICE") or field_map.get("HIGH")
    )
    low_col = (
        field_map.get("LWPRIC") or field_map.get("LOW_PRICE") or field_map.get("LOW")
    )
    close_col = (
        field_map.get("CLSPRIC")
        or field_map.get("CLOSE_PRICE")
        or field_map.get("CLOSE")
    )
    qty_col = (
        field_map.get("TTLTRADGVOL")
        or field_map.get("TTLTRADQTY")
        or field_map.get("TTL_TRD_QNTY")
        or field_map.get("TOTTRDQTY")
    )
    date_col = (
        field_map.get("TRADDT") or field_map.get("DATE1") or field_map.get("TIMESTAMP")
    )

    if not all([sym_col, open_col, high_col, low_col, close_col, qty_col]):
        first_hdr = str(reader.fieldnames[0]) if reader.fieldnames else ""
        if not first_hdr.startswith(("PK", "<", "<?", "{")):
            logger.debug("Unrecognized bhavcopy header format: %s", reader.fieldnames)
        return {}

    bars: dict[str, Bar] = {}

    for row in reader:
        series = (row.get(series_col) or "").strip().upper() if series_col else "EQ"
        if series not in ("EQ", "BE", "BZ"):
            continue

        raw_sym = (row.get(sym_col) or "").strip()
        if not raw_sym:
            continue

        try:
            op = float(row[open_col])
            hi = float(row[high_col])
            lo = float(row[low_col])
            cl = float(row[close_col])
            vol = float(row[qty_col])
        except (ValueError, TypeError, KeyError):
            continue

        hi = max(hi, op, cl)
        lo = min(lo, op, cl)
        if vol < 0:
            vol = 0.0

        raw_date_str = (row.get(date_col) or "").strip() if date_col else ""
        bar_date = _parse_date(raw_date_str) or dt.datetime.now(IST).date()

        ts = int(
            dt.datetime.combine(bar_date, dt.time(9, 15), tzinfo=IST).timestamp() * 1e9
        )
        inst_id = InstrumentId(symbol=raw_sym, exchange=exchange)

        b = Bar(
            instrument_id=inst_id,
            ts=ts,
            open=op,
            high=hi,
            low=lo,
            close=cl,
            volume=vol,
        )
        bars[raw_sym] = b

    return bars


def _parse_date(raw: str) -> dt.date | None:
    if not raw:
        return None
    for fmt in ("%Y-%m-%d", "%d-%b-%Y", "%d-%m-%Y", "%Y%m%d"):
        try:
            return dt.datetime.strptime(raw, fmt).date()
        except ValueError:
            pass
    return None


class NseBhavcopyProvider:
    """Fetches official daily EOD Bhavcopies from NSE India public archives."""

    def __init__(
        self,
        cache_dir: str | Path | None = None,
        timeout: float = 15.0,
        calendar: NseCalendar | None = None,
        max_workers: int = 12,
    ) -> None:
        self.cache_dir = Path(cache_dir) if cache_dir else None
        if self.cache_dir:
            self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.timeout = timeout
        self.calendar = calendar or NseCalendar()
        self.max_workers = max_workers
        self._client: Any = None

    def _get_client(self) -> Any:
        import httpx

        if self._client is None or self._client.is_closed:
            limits = httpx.Limits(
                max_keepalive_connections=self.max_workers * 2,
                max_connections=self.max_workers * 4,
            )
            self._client = httpx.Client(
                headers=DEFAULT_HEADERS,
                timeout=self.timeout,
                limits=limits,
                follow_redirects=True,
            )
        return self._client

    @property
    def name(self) -> str:
        return "nse_bhavcopy"

    def _get_urls_for_date(self, d: dt.date) -> list[str]:
        ymd = d.strftime("%Y%m%d")
        dmy = d.strftime("%d%m%Y")
        year = d.strftime("%Y")
        mon = d.strftime("%b").upper()
        cm_dmy = d.strftime("%d%b%Y").upper()

        return [
            f"https://nsearchives.nseindia.com/content/cm/BhavCopy_NSE_CM_0_0_0_{ymd}_F_0000.csv.zip",
            f"https://archives.nseindia.com/content/historical/EQUITIES/{year}/{mon}/cm{cm_dmy}bhav.csv.zip",
            f"https://nsearchives.nseindia.com/content/historical/EQUITIES/{year}/{mon}/cm{cm_dmy}bhav.csv.zip",
            f"https://nsearchives.nseindia.com/products/content/sec_bhavdata_full_{dmy}.csv",
            f"https://archives.nseindia.com/products/content/sec_bhavdata_full_{dmy}.csv",
        ]

    def download_session_bhavcopy(
        self, session_date: dt.date, client: Any = None
    ) -> dict[str, Bar]:
        date_str = session_date.strftime("%Y%m%d")
        xz_cache_file = (
            self.cache_dir / f"bhavcopy_{date_str}.csv.xz" if self.cache_dir else None
        )
        csv_cache_file = (
            self.cache_dir / f"bhavcopy_{date_str}.csv" if self.cache_dir else None
        )

        if xz_cache_file and xz_cache_file.exists():
            try:
                with lzma.open(xz_cache_file, mode="rt", encoding="utf-8") as f:
                    return parse_bhavcopy_csv(f.read())
            except (
                OSError,
                lzma.LZMAError,
                EOFError,
                UnicodeDecodeError,
                ValueError,
                KeyError,
            ) as exc:
                logger.warning("Error reading xz cache %s: %s", xz_cache_file, exc)

        if csv_cache_file and csv_cache_file.exists():
            try:
                content_str = csv_cache_file.read_text(encoding="utf-8")
                if xz_cache_file:
                    try:
                        with lzma.open(xz_cache_file, mode="wt", encoding="utf-8") as f:
                            f.write(content_str)
                        csv_cache_file.unlink(missing_ok=True)
                    except (OSError, lzma.LZMAError) as e:
                        logger.debug(
                            "Failed auto-compression of %s: %s", csv_cache_file, e
                        )
                return parse_bhavcopy_csv(content_str)
            except (OSError, UnicodeDecodeError, ValueError, KeyError) as exc:
                logger.warning("Error reading csv cache %s: %s", csv_cache_file, exc)

        http_client = client or self._get_client()
        urls = self._get_urls_for_date(session_date)
        for url in urls:
            try:
                resp = http_client.get(url)
                if resp.status_code != 200:
                    continue

                content = resp.content
                csv_text = ""
                is_zip = url.endswith(".zip") or content.startswith(b"PK\x03\x04")

                if is_zip:
                    try:
                        with zipfile.ZipFile(io.BytesIO(content)) as z:
                            for name in z.namelist():
                                if name.lower().endswith(".csv"):
                                    csv_text = z.read(name).decode(
                                        "utf-8", errors="ignore"
                                    )
                                    break
                    except zipfile.BadZipFile:
                        continue
                else:
                    if not content.startswith((b"<!DOCTYPE", b"<html", b"<?xml")):
                        csv_text = content.decode("utf-8", errors="ignore")

                if csv_text:
                    parsed = parse_bhavcopy_csv(csv_text)
                    if parsed:
                        if xz_cache_file:
                            try:
                                with lzma.open(
                                    xz_cache_file, mode="wt", encoding="utf-8"
                                ) as f:
                                    f.write(csv_text)
                            except (OSError, lzma.LZMAError) as e:
                                logger.warning(
                                    "Could not write xz cache %s: %s", xz_cache_file, e
                                )
                        return parsed
            except Exception as exc:  # noqa: BLE001
                logger.debug("Failed to fetch %s: %s", url, exc)

        return {}


__all__ = [
    "BhavRecord",
    "DateInterval",
    "NseBhavcopyProvider",
    "bhavcopy_to_arrow",
    "bhavcopy_to_bars",
    "parse_bhavcopy_csv",
    "parse_bhavcopy_records",
    "validate_bar",
]
