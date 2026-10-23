"""Phase 4 (Zero-Copy Arrow Lakehouse & NSE Loaders) and Phase 5 (100+ Indicators, Dual-Mode Vectorized Compute & StockSharp Strategy Primitives) verification tests."""

from __future__ import annotations

import datetime as dt
import json
from decimal import Decimal
from pathlib import Path

import honba
import pyarrow as pa
import pytest
from honba._honba import (
    ArrowStream,
    Bar,
    BhavRecord,
    InstrumentId,
    Lakehouse,
    Price,
    Tick,
    bars_to_arrow,
    bhavcopy_to_arrow,
    bhavcopy_to_bars,
    export_capsule,
    parse_bhavcopy_csv,
    protocol_capsule,
    ticks_to_arrow,
)
from honba.data import load_bars, load_ticks
from honba.data.loaders.nse import parse_bhavcopy_csv as parse_bhavcopy_dict
from honba.entities.instrument import InstrumentId as PyEntityInstrumentId
from honba.indicators import (
    T3,
    AcceleratorOscillator,
    AdLine,
    Adx,
    Alma,
    Atr,
    AwesomeOscillator,
    BollingerBands,
    Cci,
    ChaikinVolatility,
    Cmf,
    Cmo,
    Cpr,
    Dema,
    Doji,
    DonchianChannel,
    Dpo,
    Ema,
    Engulfing,
    FisherTransform,
    Frama,
    Hammer,
    HistoricalVolatility,
    Hma,
    HonbaSeriesNamespace,
    Ichimoku,
    Kama,
    KeltnerChannel,
    Macd,
    MarketDepthImbalance,
    McGinleyDynamic,
    Mfi,
    Momentum,
    MorningStar,
    Obv,
    Psar,
    Qqe,
    Rma,
    Roc,
    Rsi,
    Sma,
    StandardDeviation,
    Stochastic,
    StochRsi,
    Supertrend,
    Tema,
    ThreeWhiteSoldiers,
    Tsi,
    UltimateOscillator,
    VolumeProfile,
    Vwap,
    Vwma,
    WilliamsR,
    Wma,
    Zlema,
    adx as Adx_vec,
    atr as Atr_vec,
    bollinger_bands as BollingerBands_vec,
    cpr as Cpr_vec,
    donchian as Donchian_vec,
    ema as Ema_vec,
    macd as Macd_vec,
    obv as Obv_vec,
    rma as Rma_vec,
    rsi as Rsi_vec,
    sma as Sma_vec,
    stochastic as Stochastic_vec,
    supertrend as Supertrend_vec,
    vwap as Vwap_vec,
    williams_r as WilliamsR_vec,
)
from honba.strategies import (
    ArbitrageStrategy,
    QuotingStrategy,
    RebalancingStrategy,
    SlicingStrategy,
    Strategy,
)


def _sample_bars() -> list[Bar]:
    inst = InstrumentId.from_string("NSE:RELIANCE")
    base = dt.datetime(2026, 5, 10, 9, 15, tzinfo=dt.UTC)
    out = []
    for i in range(10):
        px = Decimal("2500.00") + Decimal(i * 5)
        out.append(
            Bar(
                inst,
                px,
                px + Decimal("12.50"),
                px - Decimal("7.25"),
                px + Decimal("4.75"),
                Decimal(1000 + i * 100),
                base + dt.timedelta(minutes=i),
            )
        )
    return out


def _sample_ticks() -> list[Tick]:
    return [
        Tick.from_json(
            json.dumps(
                {
                    "type": "ltp",
                    "data": {
                        "instrument": {"symbol": "RELIANCE", "exchange": "NSE"},
                        "price": "2505.25",
                        "volume": "50",
                        "exchange_ts": "2026-05-10T09:15:01Z",
                        "receive_ts": "2026-05-10T09:15:01Z",
                    },
                }
            )
        ),
        Tick.from_json(
            json.dumps(
                {
                    "type": "quote",
                    "data": {
                        "instrument": {"symbol": "RELIANCE", "exchange": "NSE"},
                        "bid": "2505.00",
                        "ask": "2505.50",
                        "bid_qty": "100",
                        "ask_qty": "120",
                        "last_price": "2505.25",
                        "exchange_ts": "2026-05-10T09:15:02Z",
                        "receive_ts": "2026-05-10T09:15:02Z",
                    },
                }
            )
        ),
    ]


class TestPhase4ArrowPyCapsuleAndLakehouse:
    """Phase 4: Zero-Copy Arrow PyCapsule Stream, Parquet Lakehouse v2, and NSE Bhavcopy loaders."""

    def test_arrow_stream_pycapsule_protocol_to_pyarrow(self) -> None:
        bars = _sample_bars()
        stream = bars_to_arrow(bars)
        assert isinstance(stream, ArrowStream)
        assert stream.num_rows == 10
        assert stream.num_batches == 1
        assert stream.column_names == [
            "symbol",
            "exchange",
            "timestamp",
            "open",
            "high",
            "low",
            "close",
            "volume",
        ]

        # Direct PyArrow ingestion via __arrow_c_stream__ PyCapsule protocol
        table = pa.table(stream)
        assert table.num_rows == 10
        assert table.column("symbol").to_pylist() == ["RELIANCE"] * 10
        assert table.column("exchange").to_pylist() == ["NSE"] * 10
        # Exact Decimal128(18, 4) preservation across the Arrow C Data Interface
        assert table.schema.field("close").type == pa.decimal128(18, 4)
        closes = table.column("close").to_pylist()
        assert closes[0] == Decimal("2504.7500")
        assert closes[-1] == Decimal("2549.7500")

        # Convenience .to_pyarrow()
        table2 = stream.to_pyarrow()
        assert table2.num_rows == 10

        # Schema capsule and direct stream capsule
        schema_cap = stream.__arrow_c_schema__()
        stream_cap = export_capsule(bars)
        proto = protocol_capsule()
        assert schema_cap is not None
        assert stream_cap is not None
        assert proto.num_rows == 0

    def test_ticks_to_arrow_stream(self) -> None:
        ticks = _sample_ticks()
        stream = ticks_to_arrow(ticks)
        assert stream.num_rows == 2
        table = pa.table(stream)
        assert table.num_rows == 2
        assert table.column("kind").to_pylist() == ["ltp", "quote"]
        assert table.column("price").to_pylist()[0] == Decimal("2505.2500")
        assert table.column("bid").to_pylist()[1] == Decimal("2505.0000")
        assert table.column("ask").to_pylist()[1] == Decimal("2505.5000")

    def test_lakehouse_bars_and_ticks_roundtrip_and_arrow_export(
        self, tmp_path: Path
    ) -> None:
        lh = Lakehouse(str(tmp_path / "lakehouse"))
        bars = _sample_bars()
        written_bar_files = lh.write_bars("1m", bars)
        assert len(written_bar_files) == 1

        read_back = lh.read_bars("NSE:RELIANCE", "1m", "2026-05-10", "2026-05-10")
        assert len(read_back) == len(bars)
        assert read_back[0].close == bars[0].close
        assert read_back[-1].volume == bars[-1].volume

        # High-level honba.data.load_bars zero-copy Arrow bridge
        tbl = load_bars(
            "NSE:RELIANCE",
            "2026-05-10",
            "2026-05-10",
            timeframe="1m",
            root=tmp_path / "lakehouse",
            backend="pyarrow",
        )
        assert tbl.num_rows == 10
        assert tbl.column("close").to_pylist()[0] == Decimal("2504.7500")

        raw_stream = load_bars(
            "NSE:RELIANCE",
            "2026-05-10",
            "2026-05-10",
            timeframe="1m",
            root=tmp_path / "lakehouse",
            backend="arrow",
        )
        assert isinstance(raw_stream, ArrowStream)
        assert raw_stream.num_rows == 10

        # Ticks roundtrip
        ticks = _sample_ticks()
        written_tick_files = lh.write_ticks(ticks)
        assert len(written_tick_files) == 1
        read_ticks = lh.read_ticks("NSE:RELIANCE", "2026-05-10", "2026-05-10")
        assert len(read_ticks) == 2
        tick_tbl = load_ticks(
            "NSE:RELIANCE",
            "2026-05-10",
            "2026-05-10",
            root=tmp_path / "lakehouse",
            backend="pyarrow",
        )
        assert tick_tbl.num_rows == 2

    def test_nse_bhavcopy_rust_loader_and_arrow_bridge(self) -> None:
        csv_text = (
            "SYMBOL,SERIES,DATE1,PREV_CLOSE,OPEN_PRICE,HIGH_PRICE,LOW_PRICE,"
            "LAST_PRICE,CLOSE_PRICE,AVG_PRICE,TTL_TRD_QNTY,TURNOVER_LACS,"
            "NO_OF_TRADES,DELIV_QTY,DELIV_PER\n"
            "RELIANCE, EQ, 10-May-2026, 2500.00, 2510.00, 2545.50, 2495.25, "
            "2535.00, 2532.75, 2521.40, 1250000, 31517.50, 45210, 750000, 60.00\n"
            "TCS, EQ, 10-May-2026, 3900.00, 3910.00, 3955.00, 3890.00, "
            "3940.00, 3942.50, 3928.10, 640000, 25139.84, 28100, 416000, 65.00\n"
            "LIQUIDBEES, BE, 10-May-2026, 1000.00, 1000.00, 1000.05, 999.95, "
            "1000.00, 1000.00, 1000.00, 50000, 500.00, 900, -, -\n"
        )
        records = parse_bhavcopy_csv(csv_text)
        assert len(records) == 3
        assert isinstance(records[0], BhavRecord)
        assert records[0].date == "2026-05-10"
        assert records[0].series == "EQ"
        assert records[0].bar.instrument.symbol == "RELIANCE"
        assert records[0].bar.close == Price("2532.75")
        assert records[0].prev_close == Decimal("2500.00")
        assert records[0].vwap == Decimal("2521.40")
        assert records[0].trades == 45210
        assert records[0].deliverable_qty == Decimal(750000)
        assert records[0].deliverable_pct == Decimal("60.00")

        eq_bars = bhavcopy_to_bars(csv_text, series="EQ")
        assert len(eq_bars) == 2

        arrow_stream = bhavcopy_to_arrow(csv_text, series="EQ")
        tbl = pa.table(arrow_stream)
        assert tbl.num_rows == 2
        assert tbl.column("symbol").to_pylist() == ["RELIANCE", "TCS"]

        # Python loader wrapper compatibility
        py_bars = parse_bhavcopy_dict(csv_text)
        assert "RELIANCE" in py_bars
        assert py_bars["RELIANCE"].close == pytest.approx(2532.75)


class TestPhase5IndicatorsAndStrategyPrimitives:
    """Phase 5: 100+ Indicators, Dual-Mode Streaming + Vectorized Compute & StockSharp Strategy Primitives."""

    def test_streaming_indicators_update_and_value(self) -> None:
        prices = [Decimal("100.0") + Decimal(i) * Decimal("1.5") for i in range(40)]

        # 1. Single-price indicators
        for ind in [
            Sma(5),
            Ema(5),
            Wma(5),
            Hma(9),
            Dema(5),
            Tema(5),
            Kama(5),
            Zlema(5),
            Alma(5),
            Frama(5),
            T3(5),
            McGinleyDynamic(5),
            Rma(5),
            Rsi(5),
            StochRsi(5, 5),
            Roc(5),
            Momentum(5),
            Cci(5),
            Cmo(5),
            Dpo(5),
            FisherTransform(5),
            Qqe(5),
            Tsi(5, 10),
            Macd(5, 10, 3),
            BollingerBands(5, 2.0),
            KeltnerChannel(5, 2.0),
            StandardDeviation(5),
            HistoricalVolatility(5),
        ]:
            last = None
            for p in prices:
                last = ind.update(p)
            assert last is not None
            assert ind.value is not None

        # 2. High-Low indicators
        for hl_ind in [
            DonchianChannel(5),
            ChaikinVolatility(5),
            AwesomeOscillator(5),
            AcceleratorOscillator(5),
            Psar(),
            Ichimoku(5, 10, 20),
        ]:
            last = None
            for p in prices:
                last = hl_ind.update(p + Decimal(2), p - Decimal(2))
            assert last is not None
            assert hl_ind.value is not None

        # 3. High-Low-Close indicators
        for hlc_ind in [
            Atr(5),
            Adx(5),
            Stochastic(5, 3),
            WilliamsR(5),
            UltimateOscillator(3, 6, 12),
            Supertrend(5, 2.0),
            Cpr(),
        ]:
            last = None
            for p in prices:
                last = hlc_ind.update(p + Decimal(2), p - Decimal(2), p)
            assert last is not None
            assert hlc_ind.value is not None

        # 4. Price-Volume and OHLCV indicators
        for pv_ind in [Vwma(5), Obv(), Vwap(), VolumeProfile()]:
            for p in prices:
                pv_ind.update(p, Decimal(1000))
            assert pv_ind.value is not None

        for ohlcv_ind in [AdLine(), Cmf(5), Mfi(5)]:
            for p in prices:
                ohlcv_ind.update(p + Decimal(2), p - Decimal(2), p, Decimal(1000))
            assert ohlcv_ind.value is not None

        mdi = MarketDepthImbalance()
        mdi.update(Decimal(600), Decimal(400))
        assert mdi.value == Decimal("0.2")

        # 5. Candlestick pattern detectors
        assert Doji().detect(Decimal(100), Decimal(105), Decimal(95), Decimal("100.2"))
        assert Hammer().detect(Decimal(100), Decimal(101), Decimal(90), Decimal("100.5"))
        assert (
            Engulfing().detect(Decimal(102), Decimal(100), Decimal(99), Decimal(103))
            == "bullish"
        )
        assert (
            MorningStar().detect(
                Decimal(102), Decimal(100), Decimal(99), Decimal(100), Decimal(103)
            )
            == "bullish"
        )
        assert ThreeWhiteSoldiers().detect(100, 102, 102, 104, 104, 106) == "bullish"

    def test_streaming_and_vectorized_numerical_equivalence(self) -> None:
        values = [100.0 + i * 1.25 + ((-1) ** i) * 0.6 for i in range(30)]
        high = [v + 2.5 for v in values]
        low = [v - 2.0 for v in values]
        close = values
        volume = [1000.0 + i * 25.0 for i in range(30)]

        # SMA parity across full series (warmup None + exact post-warmup values)
        s_ind = Sma(5)
        s_stream = [s_ind.update(v) for v in values]
        s_vec = Sma_vec(values, 5)
        for a, b in zip(s_stream, s_vec):
            assert (a is None and b is None) or pytest.approx(float(a), rel=1e-9) == b

        # EMA & RMA vectorized series warm up after `period` bars and converge
        e_ind = Ema(5)
        e_stream = [e_ind.update(v) for v in values]
        e_vec = Ema_vec(values, 5)
        assert e_vec[:4] == [None, None, None, None]
        assert e_vec[-1] == pytest.approx(float(e_stream[-1]), rel=1e-3)

        r_ind = Rma(5)
        r_stream = [r_ind.update(v) for v in values]
        r_vec = Rma_vec(values, 5)
        assert r_vec[:4] == [None, None, None, None]
        assert r_vec[-1] == pytest.approx(float(r_stream[-1]), rel=1e-3)

        # Batch functions return expected shapes and non-null tail values
        assert Rsi_vec(values, 5)[-1] is not None
        mid, up, lo = BollingerBands_vec(values, 5, 2.0)
        assert mid[-1] is not None and up[-1] > mid[-1] > lo[-1]
        m_line, sig_line, hist_line = Macd_vec(values, 4, 8, 3)
        assert (
            m_line[-1] is not None
            and sig_line[-1] is not None
            and hist_line[-1] is not None
        )
        assert Atr_vec(high, low, close, 5)[-1] is not None
        d_mid, d_up, d_lo = Donchian_vec(high, low, 5)
        assert d_mid[-1] is not None and d_up[-1] >= d_mid[-1] >= d_lo[-1]
        k_vals, d_vals = Stochastic_vec(high, low, close, 5, 3)
        assert k_vals[-1] is not None and d_vals[-1] is not None
        assert WilliamsR_vec(high, low, close, 5)[-1] is not None
        st_vals, st_dir = Supertrend_vec(high, low, close, 5, 2.0)
        assert st_vals[-1] is not None and st_dir[-1] in (-1, 1)
        assert Vwap_vec(high, low, close, volume)[-1] is not None
        piv, bc_v, tc_v, w_v = Cpr_vec(high, low, close)
        assert (
            piv[-1] is not None
            and bc_v[-1] is not None
            and tc_v[-1] is not None
            and w_v[-1] is not None
        )
        assert Adx_vec(high, low, close, 5)[-1] is not None
        assert Obv_vec(close, volume)[-1] is not None

        # HonbaSeriesNamespace (.honba accessor)
        ns = HonbaSeriesNamespace(values)
        assert ns.sma(5)[-1] == pytest.approx(s_vec[-1])
        assert ns.ema(5)[-1] == pytest.approx(e_vec[-1])
        assert ns.rsi(5)[-1] is not None

    def test_stocksharp_strategy_primitives(self) -> None:
        rel_inst = PyEntityInstrumentId("RELIANCE", "NSE")
        ongc_inst = PyEntityInstrumentId("ONGC", "NSE")
        tcs_inst = PyEntityInstrumentId("TCS", "NSE")

        # 1. QuotingStrategy
        quoting = QuotingStrategy(
            quoting_side="BUY",
            quoting_quantity=Decimal(10),
            price_tolerance=Decimal("0.50"),
        )
        intent1 = quoting.update_quote(rel_inst, Decimal("2500.00"))
        assert intent1 is not None
        assert quoting.quoted_price == Decimal("2500.00")
        assert not quoting.need_requote(Decimal("2500.25"))
        assert quoting.need_requote(Decimal("2501.00"))
        intent2 = quoting.update_quote(rel_inst, Decimal("2501.00"))
        assert intent2 is not None and intent2 is not intent1

        # 2. ArbitrageStrategy
        arb = ArbitrageStrategy(
            entry_spread=Decimal("5.0"),
            exit_spread=Decimal("0.5"),
            hedge_ratio=Decimal("1.0"),
            leg_quantity=Decimal(5),
        )
        pos = arb.evaluate_legs(rel_inst, Decimal(110), ongc_inst, Decimal(100))
        assert pos == -1
        assert len(arb.drain_intents()) == 2
        pos_exit = arb.evaluate_legs(
            rel_inst, Decimal("100.2"), ongc_inst, Decimal(100)
        )
        assert pos_exit == 0
        assert len(arb.drain_intents()) == 2

        # 3. SlicingStrategy (TWAP / Iceberg)
        slicer = SlicingStrategy(
            total_quantity=Decimal(100),
            slice_size=Decimal(25),
            side="BUY",
        )
        for _ in range(4):
            slicer.execute_slice(rel_inst, price=Decimal("2500.00"))
        assert slicer.is_complete
        assert slicer.remaining_to_dispatch == Decimal(0)
        assert slicer.slices_sent == 4
        assert len(slicer.drain_intents()) == 4

        # 4. RebalancingStrategy
        rebal = RebalancingStrategy(
            target_weights={"RELIANCE": Decimal("0.5"), "TCS": Decimal("0.5")},
            drift_threshold_bps=Decimal(50),
        )
        orders_placed = rebal.rebalance(
            instruments={"RELIANCE": rel_inst, "TCS": tcs_inst},
            portfolio_value=Decimal(100000),
            current_positions={"RELIANCE": Decimal(0), "TCS": Decimal(0)},
            prices={"RELIANCE": Decimal(2500), "TCS": Decimal(4000)},
        )
        assert len(orders_placed) == 2
        assert len(rebal.drain_intents()) == 2
        assert issubclass(QuotingStrategy, Strategy)
        assert issubclass(ArbitrageStrategy, Strategy)
        assert issubclass(SlicingStrategy, Strategy)
        assert issubclass(RebalancingStrategy, Strategy)
        assert hasattr(honba, "QuotingStrategy")
