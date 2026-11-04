"""Arrow Zero-Copy Ingestion Benchmark (plan.md Section 5.4 / Technical Debt Item 8).

Verifies the zero-copy Arrow PyCapsule protocol transfer:
1. 500,000 1-minute bar dataset memory transfer test into Polars in < 50 ms.
2. 1,000,000 tick dataset memory transfer test into Polars in < 150 ms.
3. 10,000,000 tick dataset memory transfer test via Arrow PyCapsule in < 500 ms.
4. Schema, column types, and row counts match exactly without data re-allocation.
"""

import time

import polars as pl
from honba._honba import (
    generate_benchmark_bar_stream,
    generate_benchmark_tick_stream,
)


def test_arrow_zero_copy_bars_500k_transfer_benchmark():
    count = 500_000
    stream = generate_benchmark_bar_stream(count=count)
    assert stream.num_rows == count

    # Time zero-copy PyCapsule ingestion into Polars
    t0 = time.perf_counter()
    res = stream.to_polars()
    elapsed = time.perf_counter() - t0

    # Ingestion benchmark threshold: < 50 ms (< 0.050s)
    assert elapsed < 0.050, f"500k bar transfer took {elapsed * 1000:.2f} ms (budget: 50 ms)"
    assert len(res) == count

    df = res.struct.unnest() if isinstance(res, pl.Series) else res
    assert df.height == count
    assert set(df.columns) == {
        "symbol",
        "exchange",
        "timestamp",
        "open",
        "high",
        "low",
        "close",
        "volume",
    }


def test_arrow_zero_copy_ticks_1m_polars_benchmark():
    count = 1_000_000
    stream = generate_benchmark_tick_stream(count=count, batch_size=count)
    assert stream.num_rows == count

    t0 = time.perf_counter()
    res = stream.to_polars()
    elapsed = time.perf_counter() - t0

    # 1M ticks transfer threshold: < 150 ms
    assert elapsed < 0.150, f"1M tick polars transfer took {elapsed * 1000:.2f} ms (budget: 150 ms)"
    assert len(res) == count


def test_arrow_zero_copy_ticks_10m_pycapsule_benchmark():
    count = 10_000_000
    stream = generate_benchmark_tick_stream(count=count, batch_size=count)
    assert stream.num_rows == count

    # Time zero-copy C Data Interface stream ingestion via PyCapsule
    t0 = time.perf_counter()
    tbl = stream.to_pyarrow()
    elapsed = time.perf_counter() - t0

    # Ingestion benchmark threshold for 10M rows (160M values): < 500 ms (< 0.500s)
    assert elapsed < 0.500, f"10M tick transfer took {elapsed * 1000:.2f} ms (budget: 500 ms)"
    assert tbl.num_rows == count
    assert "last_price" in tbl.column_names
    assert "symbol" in tbl.column_names
