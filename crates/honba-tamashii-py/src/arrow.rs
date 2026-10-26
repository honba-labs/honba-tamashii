//! Zero-copy Arrow PyCapsule protocol integration, Parquet Lakehouse v2 bindings,
//! NSE Bhavcopy loader bindings, and vectorized batch indicator kernels.
//!
//! Implements the Arrow PyCapsule interface (`__arrow_c_stream__` and
//! `__arrow_c_schema__`) over `arrow::record_batch::RecordBatch` streams so
//! PyArrow (`pyarrow.table(stream)`) and Polars (`polars.from_arrow(stream)`)
//! ingest canonical `honba-core` market data with zero memory copies.

use std::ffi::CString;
use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, Decimal128Array, RecordBatchIterator, StringArray, TimestampNanosecondArray,
};
use arrow::datatypes::SchemaRef;
use arrow::ffi::FFI_ArrowSchema;
use arrow::ffi_stream::FFI_ArrowArrayStream;
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use honba_core::{Bar, DepthLevel, Exchange, InstrumentId, Price, Quantity, Tick, Timeframe};
use honba_feed::lakehouse::{
    Lakehouse, LakehouseCompression, LakehouseError, PRICE_PRECISION, PRICE_SCALE, bar_schema_v2,
    tick_schema_v2,
};
use honba_indicators::{
    Adx, Atr, BollingerBands, Cpr, Donchian, Ema, Indicator, Macd, Obv, Rma, Rsi, Sma, Stochastic,
    Supertrend, Vwap, WilliamsR,
};
use honba_loaders::{
    BhavRecord, bars_of, merge_delivery, parse_bhavcopy_csv as rust_parse_bhavcopy_csv,
    parse_delivery_dat,
};
use pyo3::exceptions::{PyImportError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyTuple};
use rust_decimal::Decimal;

use crate::decimal::to_py_decimal;
use crate::pyclasses::core::{PyBar, PyInstrumentId, PyTick};

fn dec_to_i128(d: Decimal) -> Result<i128, String> {
    let r = d.round_dp(PRICE_SCALE as u32);
    let v = r.mantissa() * 10i128.pow(PRICE_SCALE as u32 - r.scale());
    if v.abs() >= 10i128.pow(PRICE_PRECISION as u32) {
        return Err(format!(
            "{d} does not fit Decimal128({PRICE_PRECISION}, {PRICE_SCALE})"
        ));
    }
    Ok(v)
}

fn nanos(ts: DateTime<Utc>) -> Result<i64, String> {
    ts.timestamp_nanos_opt()
        .ok_or_else(|| format!("timestamp {ts} not representable as nanoseconds"))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DepthJson {
    bids: Vec<DepthLevel>,
    asks: Vec<DepthLevel>,
}

/// Converts a slice of canonical [`Bar`]s into an Arrow [`RecordBatch`] using
/// the canonical Lakehouse v2 bar schema (`Decimal128(18, 4)` prices/quantities
/// and `Timestamp(Nanosecond, "UTC")`).
pub fn bars_to_record_batch(bars: &[Bar]) -> Result<RecordBatch, String> {
    let dec = |f: &dyn Fn(&Bar) -> Decimal| -> Result<ArrayRef, String> {
        let values = bars
            .iter()
            .map(|b| dec_to_i128(f(b)).map(Some))
            .collect::<Result<Vec<_>, _>>()?;
        let arr = Decimal128Array::from(values)
            .with_precision_and_scale(PRICE_PRECISION, PRICE_SCALE)
            .map_err(|e| e.to_string())?;
        Ok(Arc::new(arr))
    };
    let stamps = bars
        .iter()
        .map(|b| nanos(b.timestamp))
        .collect::<Result<Vec<_>, _>>()?;
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(
            bars.iter()
                .map(|b| b.instrument.symbol.as_str())
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            bars.iter()
                .map(|b| b.instrument.exchange.as_str())
                .collect::<Vec<_>>(),
        )),
        Arc::new(TimestampNanosecondArray::from(stamps).with_timezone("UTC")),
        dec(&|b| b.open.0)?,
        dec(&|b| b.high.0)?,
        dec(&|b| b.low.0)?,
        dec(&|b| b.close.0)?,
        dec(&|b| b.volume.0)?,
    ];
    RecordBatch::try_new(Arc::new(bar_schema_v2()), columns).map_err(|e| e.to_string())
}

/// Converts a slice of canonical [`Tick`]s into an Arrow [`RecordBatch`] using
/// the canonical Lakehouse v2 tick schema.
pub fn ticks_to_record_batch(ticks: &[Tick]) -> Result<RecordBatch, String> {
    let n = ticks.len();
    let mut kind = Vec::with_capacity(n);
    let mut symbol = Vec::with_capacity(n);
    let mut exchange = Vec::with_capacity(n);
    let mut token = Vec::with_capacity(n);
    let mut isin = Vec::with_capacity(n);
    let mut exchange_ts = Vec::with_capacity(n);
    let mut receive_ts = Vec::with_capacity(n);
    let mut price = Vec::with_capacity(n);
    let mut volume = Vec::with_capacity(n);
    let mut bid = Vec::with_capacity(n);
    let mut ask = Vec::with_capacity(n);
    let mut bid_qty = Vec::with_capacity(n);
    let mut ask_qty = Vec::with_capacity(n);
    let mut last_price = Vec::with_capacity(n);
    let mut depth_json = Vec::with_capacity(n);
    let mut band_json = Vec::with_capacity(n);

    for tick in ticks {
        match tick {
            Tick::Ltp {
                instrument,
                price: p,
                volume: v,
                exchange_ts: ets,
                receive_ts: rts,
            } => {
                kind.push("ltp");
                symbol.push(instrument.symbol.clone());
                exchange.push(instrument.exchange.as_str());
                token.push(instrument.token.clone());
                isin.push(instrument.isin.clone());
                exchange_ts.push(nanos(*ets)?);
                receive_ts.push(nanos(*rts)?);
                price.push(Some(dec_to_i128(p.0)?));
                volume.push(Some(dec_to_i128(v.0)?));
                bid.push(None);
                ask.push(None);
                bid_qty.push(None);
                ask_qty.push(None);
                last_price.push(None);
                depth_json.push(None);
                band_json.push(None);
            }
            Tick::Quote {
                instrument,
                bid: b,
                ask: a,
                bid_qty: bq,
                ask_qty: aq,
                last_price: lp,
                exchange_ts: ets,
                receive_ts: rts,
                band,
            } => {
                kind.push("quote");
                symbol.push(instrument.symbol.clone());
                exchange.push(instrument.exchange.as_str());
                token.push(instrument.token.clone());
                isin.push(instrument.isin.clone());
                exchange_ts.push(nanos(*ets)?);
                receive_ts.push(nanos(*rts)?);
                price.push(None);
                volume.push(None);
                bid.push(Some(dec_to_i128(b.0)?));
                ask.push(Some(dec_to_i128(a.0)?));
                bid_qty.push(Some(dec_to_i128(bq.0)?));
                ask_qty.push(Some(dec_to_i128(aq.0)?));
                last_price.push(match lp {
                    Some(x) => Some(dec_to_i128(x.0)?),
                    None => None,
                });
                depth_json.push(None);
                band_json.push(match band {
                    Some(b) => Some(serde_json::to_string(b).map_err(|e| e.to_string())?),
                    None => None,
                });
            }
            Tick::Depth {
                instrument,
                bids,
                asks,
                last_price: lp,
                exchange_ts: ets,
                receive_ts: rts,
            } => {
                kind.push("depth");
                symbol.push(instrument.symbol.clone());
                exchange.push(instrument.exchange.as_str());
                token.push(instrument.token.clone());
                isin.push(instrument.isin.clone());
                exchange_ts.push(nanos(*ets)?);
                receive_ts.push(nanos(*rts)?);
                price.push(None);
                volume.push(None);
                bid.push(None);
                ask.push(None);
                bid_qty.push(None);
                ask_qty.push(None);
                last_price.push(match lp {
                    Some(x) => Some(dec_to_i128(x.0)?),
                    None => None,
                });
                let dj = serde_json::to_string(&DepthJson {
                    bids: bids.clone(),
                    asks: asks.clone(),
                })
                .map_err(|e| e.to_string())?;
                depth_json.push(Some(dj));
                band_json.push(None);
            }
        }
    }

    let dec = |v: Vec<Option<i128>>| -> Result<ArrayRef, String> {
        let arr = Decimal128Array::from(v)
            .with_precision_and_scale(PRICE_PRECISION, PRICE_SCALE)
            .map_err(|e| e.to_string())?;
        Ok(Arc::new(arr))
    };
    let ts = |v: Vec<i64>| -> ArrayRef {
        Arc::new(TimestampNanosecondArray::from(v).with_timezone("UTC"))
    };
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(kind)),
        Arc::new(StringArray::from(symbol)),
        Arc::new(StringArray::from(exchange)),
        Arc::new(StringArray::from(token)),
        Arc::new(StringArray::from(isin)),
        ts(exchange_ts),
        ts(receive_ts),
        dec(price)?,
        dec(volume)?,
        dec(bid)?,
        dec(ask)?,
        dec(bid_qty)?,
        dec(ask_qty)?,
        dec(last_price)?,
        Arc::new(StringArray::from(depth_json)),
        Arc::new(StringArray::from(band_json)),
    ];
    RecordBatch::try_new(Arc::new(tick_schema_v2()), columns).map_err(|e| e.to_string())
}

/// Zero-copy Arrow stream implementing the Arrow PyCapsule Protocol
/// (`__arrow_c_stream__` and `__arrow_c_schema__`).
#[pyclass(name = "ArrowStream", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyArrowStream {
    pub schema: SchemaRef,
    pub batches: Vec<RecordBatch>,
}

#[pymethods]
impl PyArrowStream {
    /// Total number of rows across all record batches in the stream.
    #[getter]
    pub fn num_rows(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }

    /// Number of record batches in the stream.
    #[getter]
    pub fn num_batches(&self) -> usize {
        self.batches.len()
    }

    /// Column names in the Arrow schema.
    #[getter]
    pub fn column_names(&self) -> Vec<String> {
        self.schema
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }

    /// Arrow PyCapsule Schema interface (`__arrow_c_schema__`).
    pub fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let ffi_schema = FFI_ArrowSchema::try_from(&*self.schema)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        let name = CString::new("arrow_schema").expect("static CString");
        PyCapsule::new_bound(py, ffi_schema, Some(name))
    }

    /// Arrow PyCapsule Stream interface (`__arrow_c_stream__`).
    #[pyo3(signature = (requested_schema=None))]
    pub fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema;
        let batches = self.batches.clone();
        let schema = Arc::clone(&self.schema);
        let reader = RecordBatchIterator::new(batches.into_iter().map(Ok), schema);
        let ffi_stream = FFI_ArrowArrayStream::new(Box::new(reader));
        let name = CString::new("arrow_array_stream").expect("static CString");
        PyCapsule::new_bound(py, ffi_stream, Some(name))
    }

    /// Exports this stream into a `pyarrow.Table` via the zero-copy C Data Interface.
    pub fn to_pyarrow(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pa = py.import_bound("pyarrow")?;
        let self_py = Py::new(py, self.clone())?;
        if let Ok(table_fn) = pa.getattr("table") {
            let tbl = table_fn.call1((self_py.bind(py),))?;
            return Ok(tbl.unbind());
        }
        let reader_cls = pa.getattr("RecordBatchReader")?;
        let reader = reader_cls.call_method1("from_stream", (self_py.bind(py),))?;
        let tbl = reader.call_method0("read_all")?;
        Ok(tbl.unbind())
    }

    /// Exports this stream into a `polars.DataFrame` via the zero-copy PyCapsule interface.
    pub fn to_polars(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pl = py
            .import_bound("polars")
            .map_err(|_| PyImportError::new_err("polars is not installed"))?;
        let self_py = Py::new(py, self.clone())?;
        let df = pl.call_method1("from_arrow", (self_py.bind(py),))?;
        Ok(df.unbind())
    }

    fn __repr__(&self) -> String {
        format!(
            "ArrowStream(rows={}, batches={}, columns={:?})",
            self.num_rows(),
            self.num_batches(),
            self.column_names()
        )
    }
}

/// Parse an instrument from either a `PyInstrumentId` or `"EXCHANGE:SYMBOL"` / `"SYMBOL"`.
fn parse_instrument(value: &Bound<'_, PyAny>) -> PyResult<InstrumentId> {
    if let Ok(inst) = value.extract::<PyInstrumentId>() {
        return Ok(inst.inner);
    }
    if let Ok(text) = value.extract::<String>() {
        return Ok(PyInstrumentId::from_string(&text)?.inner);
    }
    Err(PyValueError::new_err(
        "expected an InstrumentId or an 'EXCHANGE:SYMBOL' / 'SYMBOL' string",
    ))
}

/// Parse a `Timeframe` from string (`"1m"`, `"5m"`, `"15m"`, `"30m"`, `"1h"`, `"1d"`, `"1w"`).
fn parse_timeframe(tf: &str) -> PyResult<Timeframe> {
    match tf.trim().to_ascii_lowercase().as_str() {
        "1m" | "m1" => Ok(Timeframe::M1),
        "5m" | "m5" => Ok(Timeframe::M5),
        "15m" | "m15" => Ok(Timeframe::M15),
        "30m" | "m30" => Ok(Timeframe::M30),
        "1h" | "h1" | "60m" => Ok(Timeframe::H1),
        "1d" | "d1" | "d" | "day" | "daily" => Ok(Timeframe::D1),
        "1w" | "w1" | "w" | "week" | "weekly" => Ok(Timeframe::W1),
        other => Err(PyValueError::new_err(format!(
            "unsupported timeframe {other:?}; expected one of 1m, 5m, 15m, 30m, 1h, 1d, 1w"
        ))),
    }
}

/// Parse a UTC timestamp from `datetime.datetime`, ISO-8601 string, or `YYYY-MM-DD` date string.
fn parse_datetime_bound(value: &Bound<'_, PyAny>, is_end: bool) -> PyResult<DateTime<Utc>> {
    if let Ok(dt) = value.extract::<DateTime<Utc>>() {
        return Ok(dt);
    }
    if let Ok(s) = value.extract::<String>() {
        let trimmed = s.trim();
        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
            return Ok(dt.with_timezone(&Utc));
        }
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
            let dt = if is_end {
                date.and_hms_nano_opt(23, 59, 59, 999_999_999)
                    .expect("valid time")
            } else {
                date.and_hms_opt(0, 0, 0).expect("valid time")
            };
            return Ok(Utc.from_utc_datetime(&dt));
        }
        return Err(PyValueError::new_err(format!(
            "could not parse timestamp {s:?}; expected RFC3339 or YYYY-MM-DD"
        )));
    }
    Err(PyValueError::new_err(
        "expected a datetime or an ISO-8601 / YYYY-MM-DD string",
    ))
}

fn parse_compression(s: &str) -> PyResult<LakehouseCompression> {
    match s.trim().to_ascii_lowercase().as_str() {
        "zstd" => Ok(LakehouseCompression::Zstd),
        "snappy" => Ok(LakehouseCompression::Snappy),
        "none" | "uncompressed" => Ok(LakehouseCompression::None),
        other => Err(PyValueError::new_err(format!(
            "unknown compression {other:?}; expected 'zstd', 'snappy', or 'none'"
        ))),
    }
}

fn lakehouse_err(e: LakehouseError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Parquet Lakehouse v2 reader and writer backed by `honba-me-feed`.
///
/// Stores canonical `Bar` and `Tick` records with exact `Decimal128(18, 4)`
/// prices/quantities and Hive-partitioned directory layout, and exports
/// query results as zero-copy [`PyArrowStream`]s.
#[pyclass(name = "Lakehouse", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyLakehouse {
    inner: Lakehouse,
}

#[pymethods]
impl PyLakehouse {
    /// Opens or creates a Parquet lakehouse at `root`.
    #[new]
    #[pyo3(signature = (root, compression="zstd"))]
    pub fn new(root: PathBuf, compression: &str) -> PyResult<Self> {
        let comp = parse_compression(compression)?;
        Ok(Self {
            inner: Lakehouse::new(root).with_compression(comp),
        })
    }

    /// Root directory of the lakehouse.
    #[getter]
    pub fn root(&self) -> String {
        self.inner.root().to_string_lossy().into_owned()
    }

    /// Writes canonical bars to the Hive-partitioned Parquet lakehouse, releasing the GIL.
    pub fn write_bars(
        &self,
        py: Python<'_>,
        timeframe: &str,
        bars: Vec<PyBar>,
    ) -> PyResult<Vec<String>> {
        let tf = parse_timeframe(timeframe)?;
        let core_bars: Vec<Bar> = bars.into_iter().map(|b| b.inner).collect();
        let lh = self.inner.clone();
        let paths = py
            .allow_threads(move || lh.write_bars(tf, &core_bars))
            .map_err(lakehouse_err)?;
        Ok(paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    /// Writes canonical ticks to the Hive-partitioned Parquet lakehouse, releasing the GIL.
    pub fn write_ticks(&self, py: Python<'_>, ticks: Vec<PyTick>) -> PyResult<Vec<String>> {
        let core_ticks: Vec<Tick> = ticks.into_iter().map(|t| t.inner).collect();
        let lh = self.inner.clone();
        let paths = py
            .allow_threads(move || lh.write_ticks(&core_ticks))
            .map_err(lakehouse_err)?;
        Ok(paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }

    /// Reads canonical bars in `[start, end]` from the lakehouse, releasing the GIL.
    pub fn read_bars(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        timeframe: &str,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<PyBar>> {
        let inst = parse_instrument(instrument)?;
        let tf = parse_timeframe(timeframe)?;
        let from = parse_datetime_bound(start, false)?;
        let to = parse_datetime_bound(end, true)?;
        let lh = self.inner.clone();
        let bars = py
            .allow_threads(move || lh.read_bars_range(&inst, tf, from, to))
            .map_err(lakehouse_err)?;
        Ok(bars.into_iter().map(|inner| PyBar { inner }).collect())
    }

    /// Reads canonical ticks in `[start, end]` from the lakehouse, releasing the GIL.
    pub fn read_ticks(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<PyTick>> {
        let inst = parse_instrument(instrument)?;
        let from = parse_datetime_bound(start, false)?;
        let to = parse_datetime_bound(end, true)?;
        let lh = self.inner.clone();
        let ticks = py
            .allow_threads(move || lh.read_ticks_range(&inst, from, to))
            .map_err(lakehouse_err)?;
        Ok(ticks.into_iter().map(|inner| PyTick { inner }).collect())
    }

    /// Reads bars in `[start, end]` from the Parquet lakehouse and returns a
    /// zero-copy [`PyArrowStream`] ready for `pyarrow.table()` or `polars.from_arrow()`.
    pub fn load_bars_arrow(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        timeframe: &str,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
    ) -> PyResult<PyArrowStream> {
        let inst = parse_instrument(instrument)?;
        let tf = parse_timeframe(timeframe)?;
        let from = parse_datetime_bound(start, false)?;
        let to = parse_datetime_bound(end, true)?;
        let lh = self.inner.clone();
        let batch = py
            .allow_threads(move || {
                let bars = lh
                    .read_bars_range(&inst, tf, from, to)
                    .map_err(|e| e.to_string())?;
                bars_to_record_batch(&bars)
            })
            .map_err(PyRuntimeError::new_err)?;
        let schema = batch.schema();
        Ok(PyArrowStream {
            schema,
            batches: vec![batch],
        })
    }

    /// Reads ticks in `[start, end]` from the Parquet lakehouse and returns a
    /// zero-copy [`PyArrowStream`] ready for `pyarrow.table()` or `polars.from_arrow()`.
    pub fn load_ticks_arrow(
        &self,
        py: Python<'_>,
        instrument: &Bound<'_, PyAny>,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
    ) -> PyResult<PyArrowStream> {
        let inst = parse_instrument(instrument)?;
        let from = parse_datetime_bound(start, false)?;
        let to = parse_datetime_bound(end, true)?;
        let lh = self.inner.clone();
        let batch = py
            .allow_threads(move || {
                let ticks = lh
                    .read_ticks_range(&inst, from, to)
                    .map_err(|e| e.to_string())?;
                ticks_to_record_batch(&ticks)
            })
            .map_err(PyRuntimeError::new_err)?;
        let schema = batch.schema();
        Ok(PyArrowStream {
            schema,
            batches: vec![batch],
        })
    }

    fn __repr__(&self) -> String {
        format!("Lakehouse(root={:?})", self.root())
    }
}

/// One scrip's NSE Bhavcopy record (`sec_bhavdata_full`), bridged from `honba-me-loaders`.
#[pyclass(name = "BhavRecord", module = "honba._honba")]
#[derive(Clone, Debug)]
pub struct PyBhavRecord {
    pub inner: BhavRecord,
}

#[pymethods]
impl PyBhavRecord {
    /// Trading date (`YYYY-MM-DD`).
    #[getter]
    pub fn date(&self) -> String {
        self.inner.date.format("%Y-%m-%d").to_string()
    }

    /// Exchange series (`EQ`, `BE`, `SM`, ...).
    #[getter]
    pub fn series(&self) -> String {
        self.inner.series.clone()
    }

    /// Daily OHLCV bar (timestamp at 15:30 IST in UTC).
    #[getter]
    pub fn bar(&self) -> PyBar {
        PyBar {
            inner: self.inner.bar.clone(),
        }
    }

    /// Previous session close as `decimal.Decimal`.
    #[getter]
    pub fn prev_close<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .prev_close
            .map(|d| to_py_decimal(py, d))
            .transpose()
    }

    /// Volume-weighted average price (`AVG_PRICE`) as `decimal.Decimal`.
    #[getter]
    pub fn vwap<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.vwap.map(|d| to_py_decimal(py, d)).transpose()
    }

    /// Number of trades on the day.
    #[getter]
    pub fn trades(&self) -> Option<u64> {
        self.inner.trades
    }

    /// Deliverable quantity as `decimal.Decimal`.
    #[getter]
    pub fn deliverable_qty<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .deliverable_qty
            .map(|d| to_py_decimal(py, d))
            .transpose()
    }

    /// Deliverable percentage (0–100) as `decimal.Decimal`.
    #[getter]
    pub fn deliverable_pct<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .deliverable_pct
            .map(|d| to_py_decimal(py, d))
            .transpose()
    }

    fn __repr__(&self) -> String {
        format!(
            "BhavRecord(symbol='{}', series='{}', date='{}', close='{}')",
            self.inner.bar.instrument.symbol,
            self.inner.series,
            self.inner.date,
            self.inner.bar.close.0
        )
    }
}

fn parse_fallback_date(fallback_date: Option<&str>) -> PyResult<NaiveDate> {
    match fallback_date {
        Some(s) => NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
            .map_err(|e| PyValueError::new_err(format!("invalid fallback_date {s:?}: {e}"))),
        None => Ok(NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date")),
    }
}

fn filter_bars_by_series(records: &[BhavRecord], series: Option<&str>) -> Vec<Bar> {
    match series {
        Some(s) if s.eq_ignore_ascii_case("EQ") => bars_of(records, true),
        Some(s) => records
            .iter()
            .filter(|r| r.series.eq_ignore_ascii_case(s))
            .map(|r| r.bar.clone())
            .collect(),
        None => bars_of(records, false),
    }
}

/// Converts a list of canonical `Bar`s into a zero-copy [`PyArrowStream`].
#[pyfunction]
pub fn bars_to_arrow(py: Python<'_>, bars: Vec<PyBar>) -> PyResult<PyArrowStream> {
    let core_bars: Vec<Bar> = bars.into_iter().map(|b| b.inner).collect();
    let batch = py
        .allow_threads(move || bars_to_record_batch(&core_bars))
        .map_err(PyValueError::new_err)?;
    let schema = batch.schema();
    Ok(PyArrowStream {
        schema,
        batches: vec![batch],
    })
}

/// Converts a list of canonical `Tick`s into a zero-copy [`PyArrowStream`].
#[pyfunction]
pub fn ticks_to_arrow(py: Python<'_>, ticks: Vec<PyTick>) -> PyResult<PyArrowStream> {
    let core_ticks: Vec<Tick> = ticks.into_iter().map(|t| t.inner).collect();
    let batch = py
        .allow_threads(move || ticks_to_record_batch(&core_ticks))
        .map_err(PyValueError::new_err)?;
    let schema = batch.schema();
    Ok(PyArrowStream {
        schema,
        batches: vec![batch],
    })
}

/// Exports a list of `Bar`s directly as an `arrow_array_stream` `PyCapsule`.
#[pyfunction]
pub fn export_capsule<'py>(py: Python<'py>, bars: Vec<PyBar>) -> PyResult<Bound<'py, PyCapsule>> {
    let stream = bars_to_arrow(py, bars)?;
    stream.__arrow_c_stream__(py, None)
}

/// Parses NSE Bhavcopy CSV text (and optional MTO delivery DAT text) via `honba-me-loaders`.
#[pyfunction]
#[pyo3(signature = (csv_text, fallback_date=None, delivery_dat=None))]
pub fn parse_bhavcopy_csv(
    py: Python<'_>,
    csv_text: &str,
    fallback_date: Option<&str>,
    delivery_dat: Option<&str>,
) -> PyResult<Vec<PyBhavRecord>> {
    let date = parse_fallback_date(fallback_date)?;
    let csv_owned = csv_text.to_owned();
    let dat_owned = delivery_dat.map(str::to_owned);
    let records = py
        .allow_threads(move || -> Result<Vec<BhavRecord>, String> {
            let mut bhav = rust_parse_bhavcopy_csv(&csv_owned, date).map_err(|e| e.to_string())?;
            if let Some(dat) = dat_owned {
                let deliv = parse_delivery_dat(&dat);
                merge_delivery(&mut bhav, &deliv);
            }
            Ok(bhav)
        })
        .map_err(PyValueError::new_err)?;
    Ok(records
        .into_iter()
        .map(|inner| PyBhavRecord { inner })
        .collect())
}

/// Extracts canonical `Bar`s from NSE Bhavcopy CSV text via `honba-me-loaders`.
#[pyfunction]
#[pyo3(signature = (csv_text, fallback_date=None, series=Some("EQ")))]
pub fn bhavcopy_to_bars(
    py: Python<'_>,
    csv_text: &str,
    fallback_date: Option<&str>,
    series: Option<&str>,
) -> PyResult<Vec<PyBar>> {
    let date = parse_fallback_date(fallback_date)?;
    let csv_owned = csv_text.to_owned();
    let series_owned = series.map(str::to_owned);
    let bars = py
        .allow_threads(move || -> Result<Vec<Bar>, String> {
            let records = rust_parse_bhavcopy_csv(&csv_owned, date).map_err(|e| e.to_string())?;
            Ok(filter_bars_by_series(&records, series_owned.as_deref()))
        })
        .map_err(PyValueError::new_err)?;
    Ok(bars.into_iter().map(|inner| PyBar { inner }).collect())
}

/// Parses NSE Bhavcopy CSV text and exports the resulting bars as a zero-copy [`PyArrowStream`].
#[pyfunction]
#[pyo3(signature = (csv_text, fallback_date=None, series=Some("EQ")))]
pub fn bhavcopy_to_arrow(
    py: Python<'_>,
    csv_text: &str,
    fallback_date: Option<&str>,
    series: Option<&str>,
) -> PyResult<PyArrowStream> {
    let date = parse_fallback_date(fallback_date)?;
    let csv_owned = csv_text.to_owned();
    let series_owned = series.map(str::to_owned);
    let batch = py
        .allow_threads(move || -> Result<RecordBatch, String> {
            let records = rust_parse_bhavcopy_csv(&csv_owned, date).map_err(|e| e.to_string())?;
            let bars = filter_bars_by_series(&records, series_owned.as_deref());
            bars_to_record_batch(&bars)
        })
        .map_err(PyValueError::new_err)?;
    let schema = batch.schema();
    Ok(PyArrowStream {
        schema,
        batches: vec![batch],
    })
}

/// Protocol registration helper (returns an empty bar-schema `ArrowStream`).
#[pyfunction]
pub fn protocol_capsule(py: Python<'_>) -> PyResult<PyArrowStream> {
    bars_to_arrow(py, Vec::new())
}

// ---------------------------------------------------------------------------
// Vectorized Batch Indicator Kernels (backed by `honba-me-indicators`, GIL-free)
// ---------------------------------------------------------------------------

fn synth_bar(high: f64, low: f64, close: f64, volume: f64) -> Bar {
    let dec = |v: f64| Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO);
    Bar {
        instrument: InstrumentId::new(Exchange::Nse, "VEC"),
        open: Price(dec(close)),
        high: Price(dec(high)),
        low: Price(dec(low)),
        close: Price(dec(close)),
        volume: Quantity(dec(volume.max(0.0))),
        timestamp: DateTime::<Utc>::from_timestamp(0, 0).expect("epoch"),
    }
}

/// Vectorized SMA over a float series, executed in Rust with the GIL released.
#[pyfunction]
pub fn batch_sma(py: Python<'_>, values: Vec<f64>, period: usize) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Sma::new(period);
        values.into_iter().map(|v| ind.update(v)).collect()
    }))
}

/// Vectorized EMA over a float series, executed in Rust with the GIL released.
#[pyfunction]
pub fn batch_ema(py: Python<'_>, values: Vec<f64>, period: usize) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Ema::new(period);
        values.into_iter().map(|v| ind.update(v)).collect()
    }))
}

/// Vectorized Wilder's RMA over a float series, executed in Rust with the GIL released.
#[pyfunction]
pub fn batch_rma(py: Python<'_>, values: Vec<f64>, period: usize) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Rma::new(period);
        values.into_iter().map(|v| ind.update(v)).collect()
    }))
}

/// Vectorized RSI over a float series, executed in Rust with the GIL released.
#[pyfunction]
pub fn batch_rsi(py: Python<'_>, values: Vec<f64>, period: usize) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Rsi::new(period);
        values.into_iter().map(|v| ind.update(v)).collect()
    }))
}

/// Vectorized Bollinger Bands `(middle, upper, lower)`, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (values, period=20, num_std=2.0))]
pub fn batch_bollinger(
    py: Python<'_>,
    values: Vec<f64>,
    period: usize,
    num_std: f64,
) -> PyResult<(Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>)> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    Ok(py.allow_threads(move || {
        let mut ind = BollingerBands::new(period, num_std);
        let mut middle = Vec::with_capacity(values.len());
        let mut upper = Vec::with_capacity(values.len());
        let mut lower = Vec::with_capacity(values.len());
        for v in values {
            match ind.update(v) {
                Some(b) => {
                    middle.push(Some(b.middle));
                    upper.push(Some(b.upper));
                    lower.push(Some(b.lower));
                }
                None => {
                    middle.push(None);
                    upper.push(None);
                    lower.push(None);
                }
            }
        }
        (middle, upper, lower)
    }))
}

/// Vectorized MACD `(macd, signal, histogram)`, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (values, fast_period=12, slow_period=26, signal_period=9))]
pub fn batch_macd(
    py: Python<'_>,
    values: Vec<f64>,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> PyResult<(Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>)> {
    if fast_period == 0 || slow_period == 0 || signal_period == 0 || fast_period >= slow_period {
        return Err(PyValueError::new_err(
            "periods must be > 0 and fast_period < slow_period",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Macd::new(fast_period, slow_period, signal_period);
        let mut macd = Vec::with_capacity(values.len());
        let mut signal = Vec::with_capacity(values.len());
        let mut hist = Vec::with_capacity(values.len());
        for v in values {
            match ind.update(v) {
                Some(m) => {
                    macd.push(Some(m.macd));
                    signal.push(Some(m.signal));
                    hist.push(Some(m.histogram));
                }
                None => {
                    macd.push(None);
                    signal.push(None);
                    hist.push(None);
                }
            }
        }
        (macd, signal, hist)
    }))
}

/// Vectorized ATR over `(high, low, close)` series, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn batch_atr(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    period: usize,
) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    if high.len() != low.len() || high.len() != close.len() {
        return Err(PyValueError::new_err(
            "high, low, and close must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Atr::new(period);
        high.into_iter()
            .zip(low)
            .zip(close)
            .map(|((h, l), c)| ind.update_hlc(h, l, c))
            .collect()
    }))
}

/// Vectorized Donchian Channel `(middle, upper, lower)`, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (high, low, period=20))]
pub fn batch_donchian(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    period: usize,
) -> PyResult<(Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>)> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    if high.len() != low.len() {
        return Err(PyValueError::new_err("high and low must have equal length"));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Donchian::new(period);
        let mut middle = Vec::with_capacity(high.len());
        let mut upper = Vec::with_capacity(high.len());
        let mut lower = Vec::with_capacity(high.len());
        for (h, l) in high.into_iter().zip(low) {
            match ind.update_hl(h, l) {
                Some(d) => {
                    middle.push(Some(d.middle));
                    upper.push(Some(d.upper));
                    lower.push(Some(d.lower));
                }
                None => {
                    middle.push(None);
                    upper.push(None);
                    lower.push(None);
                }
            }
        }
        (middle, upper, lower)
    }))
}

/// Vectorized Stochastic `(k, d)`, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (high, low, close, k_period=14, d_period=3))]
pub fn batch_stochastic(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    k_period: usize,
    d_period: usize,
) -> PyResult<(Vec<Option<f64>>, Vec<Option<f64>>)> {
    if k_period == 0 || d_period == 0 {
        return Err(PyValueError::new_err("k_period and d_period must be > 0"));
    }
    if high.len() != low.len() || high.len() != close.len() {
        return Err(PyValueError::new_err(
            "high, low, and close must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Stochastic::new(k_period, d_period);
        let mut k_out = Vec::with_capacity(high.len());
        let mut d_out = Vec::with_capacity(high.len());
        for ((h, l), c) in high.into_iter().zip(low).zip(close) {
            match ind.update_hlc(h, l, c) {
                Some(s) => {
                    k_out.push(Some(s.k));
                    d_out.push(s.d);
                }
                None => {
                    k_out.push(None);
                    d_out.push(None);
                }
            }
        }
        (k_out, d_out)
    }))
}

/// Vectorized Williams %R, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn batch_williams_r(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    period: usize,
) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    if high.len() != low.len() || high.len() != close.len() {
        return Err(PyValueError::new_err(
            "high, low, and close must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = WilliamsR::new(period);
        high.into_iter()
            .zip(low)
            .zip(close)
            .map(|((h, l), c)| ind.update_hlc(h, l, c))
            .collect()
    }))
}

/// Vectorized Supertrend `(supertrend, direction)` where `direction` is `1` (uptrend) or `-1` (downtrend).
#[pyfunction]
#[pyo3(signature = (high, low, close, period=10, multiplier=3.0))]
pub fn batch_supertrend(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    period: usize,
    multiplier: f64,
) -> PyResult<(Vec<Option<f64>>, Vec<i32>)> {
    if period == 0 || multiplier <= 0.0 {
        return Err(PyValueError::new_err(
            "period and multiplier must be positive",
        ));
    }
    if high.len() != low.len() || high.len() != close.len() {
        return Err(PyValueError::new_err(
            "high, low, and close must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Supertrend::new(period, multiplier);
        let mut st_out = Vec::with_capacity(high.len());
        let mut dir_out = Vec::with_capacity(high.len());
        for ((h, l), c) in high.into_iter().zip(low).zip(close) {
            let bar = synth_bar(h, l, c, 1.0);
            let val = ind.update(&bar);
            st_out.push(val);
            dir_out.push(if ind.is_uptrend() { 1 } else { -1 });
        }
        (st_out, dir_out)
    }))
}

/// Vectorized VWAP over `(high, low, close, volume)`, executed in Rust with the GIL released.
#[pyfunction]
pub fn batch_vwap(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    volume: Vec<f64>,
) -> PyResult<Vec<Option<f64>>> {
    if high.len() != low.len() || high.len() != close.len() || high.len() != volume.len() {
        return Err(PyValueError::new_err(
            "high, low, close, and volume must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Vwap::new();
        high.into_iter()
            .zip(low)
            .zip(close)
            .zip(volume)
            .map(|(((h, l), c), v)| {
                let bar = synth_bar(h, l, c, v);
                ind.update(&bar)
            })
            .collect()
    }))
}

/// Vectorized Central Pivot Range `(pivot, bc, tc, width)` over `(high, low, close)`.
#[pyfunction]
pub fn batch_cpr(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
) -> PyResult<(
    Vec<Option<f64>>,
    Vec<Option<f64>>,
    Vec<Option<f64>>,
    Vec<Option<f64>>,
)> {
    if high.len() != low.len() || high.len() != close.len() {
        return Err(PyValueError::new_err(
            "high, low, and close must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Cpr::new();
        let n = high.len();
        let mut pivot = Vec::with_capacity(n);
        let mut bc = Vec::with_capacity(n);
        let mut tc = Vec::with_capacity(n);
        let mut width = Vec::with_capacity(n);
        for ((h, l), c) in high.into_iter().zip(low).zip(close) {
            let bar = synth_bar(h, l, c, 1.0);
            let _ = ind.update(&bar);
            pivot.push(ind.pivot());
            bc.push(ind.bc());
            tc.push(ind.tc());
            width.push(ind.width());
        }
        (pivot, bc, tc, width)
    }))
}

/// Vectorized ADX over `(high, low, close)`, executed in Rust with the GIL released.
#[pyfunction]
#[pyo3(signature = (high, low, close, period=14))]
pub fn batch_adx(
    py: Python<'_>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    period: usize,
) -> PyResult<Vec<Option<f64>>> {
    if period == 0 {
        return Err(PyValueError::new_err("period must be > 0"));
    }
    if high.len() != low.len() || high.len() != close.len() {
        return Err(PyValueError::new_err(
            "high, low, and close must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Adx::new(period);
        high.into_iter()
            .zip(low)
            .zip(close)
            .map(|((h, l), c)| {
                let bar = synth_bar(h, l, c, 1.0);
                ind.update(&bar)
            })
            .collect()
    }))
}

/// Vectorized On-Balance Volume (OBV) over `(close, volume)`, executed in Rust with the GIL released.
#[pyfunction]
pub fn batch_obv(py: Python<'_>, close: Vec<f64>, volume: Vec<f64>) -> PyResult<Vec<Option<f64>>> {
    if close.len() != volume.len() {
        return Err(PyValueError::new_err(
            "close and volume must have equal length",
        ));
    }
    Ok(py.allow_threads(move || {
        let mut ind = Obv::new();
        close
            .into_iter()
            .zip(volume)
            .map(|(c, v)| {
                let bar = synth_bar(c, c, c, v);
                ind.update(&bar)
            })
            .collect()
    }))
}

/// Register Arrow, Lakehouse, Bhavcopy, and vectorized indicator bindings on `honba._honba`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let _ = PyTuple::empty_bound(m.py());
    m.add_class::<PyArrowStream>()?;
    m.add_class::<PyLakehouse>()?;
    m.add_class::<PyBhavRecord>()?;
    m.add_function(wrap_pyfunction!(protocol_capsule, m)?)?;
    m.add_function(wrap_pyfunction!(bars_to_arrow, m)?)?;
    m.add_function(wrap_pyfunction!(ticks_to_arrow, m)?)?;
    m.add_function(wrap_pyfunction!(export_capsule, m)?)?;
    m.add_function(wrap_pyfunction!(parse_bhavcopy_csv, m)?)?;
    m.add_function(wrap_pyfunction!(bhavcopy_to_bars, m)?)?;
    m.add_function(wrap_pyfunction!(bhavcopy_to_arrow, m)?)?;
    m.add_function(wrap_pyfunction!(batch_sma, m)?)?;
    m.add_function(wrap_pyfunction!(batch_ema, m)?)?;
    m.add_function(wrap_pyfunction!(batch_rma, m)?)?;
    m.add_function(wrap_pyfunction!(batch_rsi, m)?)?;
    m.add_function(wrap_pyfunction!(batch_bollinger, m)?)?;
    m.add_function(wrap_pyfunction!(batch_macd, m)?)?;
    m.add_function(wrap_pyfunction!(batch_atr, m)?)?;
    m.add_function(wrap_pyfunction!(batch_donchian, m)?)?;
    m.add_function(wrap_pyfunction!(batch_stochastic, m)?)?;
    m.add_function(wrap_pyfunction!(batch_williams_r, m)?)?;
    m.add_function(wrap_pyfunction!(batch_supertrend, m)?)?;
    m.add_function(wrap_pyfunction!(batch_vwap, m)?)?;
    m.add_function(wrap_pyfunction!(batch_cpr, m)?)?;
    m.add_function(wrap_pyfunction!(batch_adx, m)?)?;
    m.add_function(wrap_pyfunction!(batch_obv, m)?)?;
    Ok(())
}
