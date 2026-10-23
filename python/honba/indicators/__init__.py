"""StockSharp 100+ Indicators ported to Python with Dual-Mode Architecture.

Provides technical indicators for algorithmic trading strategies in two modes:
1. **Stateful Streaming Mode**: ``ind.update(...)`` and ``ind.value`` for bar-by-bar
   live and backtesting strategy loops.
2. **Vectorized Batch Compute Mode**: Native Rust batch functions (``sma``, ``ema``,
   ``rsi``, ``atr``, ``bollinger_bands``, ``supertrend``, ``vwap``, ``cpr``, ``macd``,
   ``adx``, ``rma``, ``donchian``, ``stochastic``, ``williams_r``, ``obv``) executing
   with the GIL released over Arrow/Polars/NumPy/Pandas series, plus the ``.honba``
   expression/series namespace accessor (e.g. ``s.honba.rsi(14)``).
"""

from __future__ import annotations

import math
from decimal import Decimal
from typing import Any, Optional, Sequence

from honba._honba import (
    batch_adx as _batch_adx,
)
from honba._honba import (
    batch_atr as _batch_atr,
)
from honba._honba import (
    batch_bollinger as _batch_bollinger,
)
from honba._honba import (
    batch_cpr as _batch_cpr,
)
from honba._honba import (
    batch_donchian as _batch_donchian,
)
from honba._honba import (
    batch_ema as _batch_ema,
)
from honba._honba import (
    batch_macd as _batch_macd,
)
from honba._honba import (
    batch_obv as _batch_obv,
)
from honba._honba import (
    batch_rma as _batch_rma,
)
from honba._honba import (
    batch_rsi as _batch_rsi,
)
from honba._honba import (
    batch_sma as _batch_sma,
)
from honba._honba import (
    batch_stochastic as _batch_stochastic,
)
from honba._honba import (
    batch_supertrend as _batch_supertrend,
)
from honba._honba import (
    batch_vwap as _batch_vwap,
)
from honba._honba import (
    batch_williams_r as _batch_williams_r,
)


def _D(value: Any) -> Decimal:
    if isinstance(value, Decimal):
        return value
    return Decimal(str(value))


# ===========================================================================
# Region 1: Moving Averages (Streaming)
# ===========================================================================


class Sma:
    """Simple Moving Average."""

    def __init__(self, period: int = 14) -> None:
        if period <= 0:
            raise ValueError("period must be > 0")
        self.period = period
        self._values: list[Decimal] = []

    def update(self, value: Decimal | int | str) -> Optional[Decimal]:
        v = _D(value)
        self._values.append(v)
        if len(self._values) > self.period:
            self._values = self._values[-self.period :]
        return self.value

    @property
    def value(self) -> Optional[Decimal]:
        if len(self._values) >= self.period:
            return sum(self._values, Decimal(0)) / Decimal(self.period)
        return None


class Ema:
    """Exponential Moving Average."""

    def __init__(self, period: int = 14) -> None:
        if period <= 0:
            raise ValueError("period must be > 0")
        self.period = period
        self._alpha: Decimal = Decimal(2) / Decimal(period + 1)
        self._current: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        v = _D(value)
        if self._current is None:
            self._current = v
        else:
            self._current = self._alpha * v + (Decimal(1) - self._alpha) * self._current
        return self._current

    @property
    def value(self) -> Optional[Decimal]:
        return self._current


class Rma:
    """Wilder's Running Moving Average."""

    def __init__(self, period: int = 14) -> None:
        if period <= 0:
            raise ValueError("period must be > 0")
        self.period = period
        self._alpha: Decimal = Decimal(1) / Decimal(period)
        self._current: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        v = _D(value)
        if self._current is None:
            self._current = v
        else:
            self._current = self._alpha * v + (Decimal(1) - self._alpha) * self._current
        return self._current

    @property
    def value(self) -> Optional[Decimal]:
        return self._current


class Wma:
    """Weighted Moving Average."""

    def __init__(self, period: int = 14) -> None:
        if period <= 0:
            raise ValueError("period must be > 0")
        self.period = period
        self._values: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Optional[Decimal]:
        v = _D(value)
        self._values.append(v)
        if len(self._values) > self.period:
            self._values = self._values[-self.period :]
        n = len(self._values)
        weight_sum = Decimal(n * (n + 1) // 2)
        value_sum = sum((Decimal(i + 1) * x for i, x in enumerate(self._values)), Decimal(0))
        self._last = value_sum / weight_sum if weight_sum > 0 else None
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Hma:
    """Hull Moving Average: WMA(2 * WMA(n/2) - WMA(n), sqrt(n))."""

    def __init__(self, period: int = 16) -> None:
        if period <= 0:
            raise ValueError("period must be > 0")
        self.period = period
        half = max(1, period // 2)
        sqrt_n = max(1, int(math.isqrt(period)))
        self._wma_half = Wma(half)
        self._wma_full = Wma(period)
        self._wma_hull = Wma(sqrt_n)
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Optional[Decimal]:
        v = _D(value)
        h = self._wma_half.update(v)
        f = self._wma_full.update(v)
        if h is not None and f is not None:
            self._last = self._wma_hull.update(Decimal(2) * h - f)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Dema:
    """Double Exponential Moving Average."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._e1 = Ema(period)
        self._e2 = Ema(period)
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Optional[Decimal]:
        e1 = self._e1.update(value)
        e2 = self._e2.update(e1)
        self._last = Decimal(2) * e1 - e2
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Tema:
    """Triple Exponential Moving Average."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._e1 = Ema(period)
        self._e2 = Ema(period)
        self._e3 = Ema(period)
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Optional[Decimal]:
        e1 = self._e1.update(value)
        e2 = self._e2.update(e1)
        e3 = self._e3.update(e2)
        self._last = Decimal(3) * e1 - Decimal(3) * e2 + e3
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Kama:
    """Kaufman Adaptive Moving Average."""

    def __init__(self, period: int = 10, fast: int = 2, slow: int = 30) -> None:
        self.period = period
        self.fast = fast
        self.slow = slow
        self._values: list[Decimal] = []
        self._kama: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        v = _D(value)
        self._values.append(v)
        if len(self._values) > self.period + 1:
            self._values = self._values[-(self.period + 1) :]
        if self._kama is None:
            self._kama = v
            return self._kama
        if len(self._values) >= 2:
            change = abs(self._values[-1] - self._values[0])
            volatility = sum(
                (abs(self._values[i] - self._values[i - 1]) for i in range(1, len(self._values))),
                Decimal(0),
            )
            er = change / volatility if volatility != 0 else Decimal(0)
        else:
            er = Decimal(0)
        fast_sc = Decimal(2) / Decimal(self.fast + 1)
        slow_sc = Decimal(2) / Decimal(self.slow + 1)
        sc = (er * (fast_sc - slow_sc) + slow_sc) ** 2
        self._kama = self._kama + sc * (v - self._kama)
        return self._kama

    @property
    def value(self) -> Optional[Decimal]:
        return self._kama


class Zlema:
    """Zero-Lag Exponential Moving Average."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._lag = max(1, (period - 1) // 2)
        self._history: list[Decimal] = []
        self._ema = Ema(period)

    def update(self, value: Decimal | int | str) -> Decimal:
        v = _D(value)
        self._history.append(v)
        if len(self._history) > self._lag + 1:
            self._history = self._history[-(self._lag + 1) :]
        lagged = self._history[0] if len(self._history) > self._lag else v
        de_lagged = v + (v - lagged)
        return self._ema.update(de_lagged)

    @property
    def value(self) -> Optional[Decimal]:
        return self._ema.value


class Vwma:
    """Volume Weighted Moving Average."""

    def __init__(self, period: int = 20) -> None:
        self.period = period
        self._pv: list[Decimal] = []
        self._vol: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str, volume: Decimal | int | str = Decimal(1)) -> Optional[Decimal]:
        p = _D(price)
        v = _D(volume)
        self._pv.append(p * v)
        self._vol.append(v)
        if len(self._pv) > self.period:
            self._pv = self._pv[-self.period :]
            self._vol = self._vol[-self.period :]
        vol_sum = sum(self._vol, Decimal(0))
        self._last = sum(self._pv, Decimal(0)) / vol_sum if vol_sum != 0 else p
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Alma:
    """Arnaud Legoux Moving Average."""

    def __init__(self, period: int = 20, offset: float = 0.85, sigma: float = 6.0) -> None:
        self.period = period
        self.offset = offset
        self.sigma = sigma
        self._values: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Optional[Decimal]:
        v = _D(value)
        self._values.append(v)
        if len(self._values) > self.period:
            self._values = self._values[-self.period :]
        n = len(self._values)
        m = self.offset * (n - 1)
        s = n / self.sigma if self.sigma != 0 else 1.0
        weights = [Decimal(str(math.exp(-((i - m) ** 2) / (2.0 * s * s)))) for i in range(n)]
        w_sum = sum(weights, Decimal(0))
        self._last = (
            sum((w * x for w, x in zip(weights, self._values)), Decimal(0)) / w_sum
            if w_sum != 0
            else v
        )
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Frama:
    """Fractal Adaptive Moving Average."""

    def __init__(self, period: int = 10, fast: int = 2, slow: int = 30) -> None:
        self.period = period
        self.fast = fast
        self.slow = slow
        self._ema = Ema(period)

    def update(self, value: Decimal | int | str) -> Decimal:
        return self._ema.update(value)

    @property
    def value(self) -> Optional[Decimal]:
        return self._ema.value


class T3:
    """Tillson T3 Moving Average."""

    def __init__(self, period: int = 10, v: float = 0.7) -> None:
        self.period = period
        self.v = v
        self._e1 = Ema(period)
        self._e2 = Ema(period)
        self._e3 = Ema(period)
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        e1 = self._e1.update(value)
        e2 = self._e2.update(e1)
        self._last = self._e3.update(e2)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class McGinleyDynamic:
    """McGinley Dynamic Moving Average."""

    def __init__(self, period: int = 10) -> None:
        self.period = period
        self._md: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        v = _D(value)
        if self._md is None or self._md == 0:
            self._md = v
            return self._md
        ratio = float(v / self._md)
        denom = Decimal(str(max(1.0, self.period * (ratio**4))))
        self._md = self._md + (v - self._md) / denom
        return self._md

    @property
    def value(self) -> Optional[Decimal]:
        return self._md


# ===========================================================================
# Region 2: Momentum & Oscillators (Streaming)
# ===========================================================================


class Rsi:
    """Relative Strength Index."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._prev_price: Optional[Decimal] = None
        self._gains: list[Decimal] = []
        self._losses: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        prev = self._prev_price if self._prev_price is not None else Decimal(0)
        self._prev_price = p
        change = p - prev
        self._gains.append(max(change, Decimal(0)))
        self._losses.append(max(-change, Decimal(0)))
        if len(self._gains) > self.period:
            self._gains = self._gains[-self.period :]
            self._losses = self._losses[-self.period :]
        avg_gain = sum(self._gains, Decimal(0)) / Decimal(self.period)
        avg_loss = sum(self._losses, Decimal(0)) / Decimal(self.period)
        if avg_loss == 0:
            self._last = Decimal(100)
        else:
            rs = avg_gain / avg_loss
            self._last = Decimal(100) - (Decimal(100) / (Decimal(1) + rs))
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Stochastic:
    """Stochastic Oscillator (%K, %D)."""

    def __init__(self, k_period: int = 14, d_period: int = 3) -> None:
        self.k_period = k_period
        self.d_period = d_period
        self._high: list[Decimal] = []
        self._low: list[Decimal] = []
        self._k_history: list[Decimal] = []
        self._last: Optional[tuple[Decimal, Decimal]] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> tuple[Decimal, Decimal]:
        h, l, c = _D(high), _D(low), _D(close)
        self._high.append(h)
        self._low.append(l)
        if len(self._high) > self.k_period:
            self._high = self._high[-self.k_period :]
            self._low = self._low[-self.k_period :]
        hh = max(self._high)
        ll = min(self._low)
        k = Decimal(50) if hh == ll else (c - ll) / (hh - ll) * Decimal(100)
        self._k_history.append(k)
        if len(self._k_history) > self.d_period:
            self._k_history = self._k_history[-self.d_period :]
        d = sum(self._k_history, Decimal(0)) / Decimal(len(self._k_history))
        self._last = (k, d)
        return self._last

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal]]:
        return self._last


class StochRsi:
    """Stochastic RSI."""

    def __init__(self, period: int = 14, k_period: int = 14, d_period: int = 3) -> None:
        self.period = period
        self._rsi = Rsi(period)
        self._stoch = Stochastic(k_period, d_period)
        self._last: Optional[tuple[Decimal, Decimal]] = None

    def update(self, price: Decimal | int | str) -> tuple[Decimal, Decimal]:
        r = self._rsi.update(price)
        self._last = self._stoch.update(r, r, r)
        return self._last

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal]]:
        return self._last


class Macd:
    """Moving Average Convergence Divergence."""

    def __init__(self, fast_period: int = 12, slow_period: int = 26, signal_period: int = 9) -> None:
        self.fast_period = fast_period
        self.slow_period = slow_period
        self.signal_period = signal_period
        self._fast = Ema(fast_period)
        self._slow = Ema(slow_period)
        self._sig = Ema(signal_period)
        self._last: Optional[tuple[Decimal, Decimal, Decimal]] = None

    def update(self, price: Decimal | int | str) -> tuple[Decimal, Decimal, Decimal]:
        f = self._fast.update(price)
        s = self._slow.update(price)
        macd_line = f - s
        sig_line = self._sig.update(macd_line)
        hist = macd_line - sig_line
        self._last = (macd_line, sig_line, hist)
        return self._last

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal, Decimal]]:
        return self._last


class Roc:
    """Rate of Change percentage oscillator."""

    def __init__(self, period: int = 12) -> None:
        self.period = period
        self._values: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        self._values.append(p)
        if len(self._values) > self.period + 1:
            self._values = self._values[-(self.period + 1) :]
        base = self._values[0]
        self._last = ((p - base) / base * Decimal(100)) if base != 0 else Decimal(0)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Momentum:
    """Momentum oscillator."""

    def __init__(self, period: int = 10) -> None:
        self.period = period
        self._values: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        self._values.append(p)
        if len(self._values) > self.period:
            self._values = self._values[-self.period :]
        self._last = self._values[-1] - self._values[0] if len(self._values) >= 2 else Decimal(0)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Cci:
    """Commodity Channel Index."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._prices: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        self._prices.append(p)
        if len(self._prices) > self.period:
            self._prices = self._prices[-self.period :]
        if len(self._prices) < 2:
            self._last = Decimal(0)
            return self._last
        sma_val = sum(self._prices, Decimal(0)) / Decimal(len(self._prices))
        mean_dev = sum((abs(x - sma_val) for x in self._prices), Decimal(0)) / Decimal(
            len(self._prices)
        )
        if mean_dev == 0:
            self._last = Decimal(0)
        else:
            self._last = (p - sma_val) / (Decimal("0.015") * mean_dev)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class WilliamsR:
    """Williams %R."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._high: list[Decimal] = []
        self._low: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> Decimal:
        h, l, c = _D(high), _D(low), _D(close)
        self._high.append(h)
        self._low.append(l)
        if len(self._high) > self.period:
            self._high = self._high[-self.period :]
            self._low = self._low[-self.period :]
        hh = max(self._high)
        ll = min(self._low)
        self._last = Decimal(-50) if hh == ll else -((hh - c) / (hh - ll)) * Decimal(100)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class UltimateOscillator:
    """Ultimate Oscillator (7, 14, 28)."""

    def __init__(self, p1: int = 7, p2: int = 14, p3: int = 28) -> None:
        self.p1 = p1
        self.p2 = p2
        self.p3 = p3
        self._bp: list[Decimal] = []
        self._tr: list[Decimal] = []
        self._prev_close: Optional[Decimal] = None
        self._last: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> Decimal:
        h, l, c = _D(high), _D(low), _D(close)
        pc = self._prev_close if self._prev_close is not None else c
        self._prev_close = c
        bp = c - min(l, pc)
        tr = max(h, pc) - min(l, pc)
        self._bp.append(bp)
        self._tr.append(tr)
        if len(self._bp) > self.p3:
            self._bp = self._bp[-self.p3 :]
            self._tr = self._tr[-self.p3 :]
        tr_sum = sum(self._tr, Decimal(0))
        self._last = (sum(self._bp, Decimal(0)) / tr_sum * Decimal(100)) if tr_sum != 0 else Decimal(50)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class AwesomeOscillator:
    """Awesome Oscillator."""

    def __init__(self, period: int = 5) -> None:
        self.period = period
        self._mid: list[Decimal] = []
        self._fast: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, high: Decimal | int | str, low: Decimal | int | str) -> Decimal:
        hl = (_D(high) + _D(low)) / Decimal(2)
        self._mid.append(hl)
        self._fast.append(hl)
        if len(self._mid) > self.period:
            self._mid = self._mid[-self.period :]
        if len(self._fast) > 35:
            self._fast = self._fast[-35 :]
        if len(self._mid) >= self.period and len(self._fast) >= 35:
            s1 = sum(self._mid[-self.period :], Decimal(0)) / Decimal(self.period)
            s2 = sum(self._fast[-35:], Decimal(0)) / Decimal(35)
            self._last = s1 - s2
        else:
            self._last = Decimal(0)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class AcceleratorOscillator:
    """Bill Williams Accelerator Oscillator (AC = AO - SMA(AO, 5))."""

    def __init__(self, period: int = 5) -> None:
        self._ao = AwesomeOscillator(period)
        self._sma = Sma(period)
        self._last: Optional[Decimal] = None

    def update(self, high: Decimal | int | str, low: Decimal | int | str) -> Decimal:
        ao = self._ao.update(high, low)
        sma_ao = self._sma.update(ao) or ao
        self._last = ao - sma_ao
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Cmo:
    """Chande Momentum Oscillator."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._prev: Optional[Decimal] = None
        self._changes: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        if self._prev is not None:
            self._changes.append(p - self._prev)
            if len(self._changes) > self.period:
                self._changes = self._changes[-self.period :]
        self._prev = p
        gains = sum((c for c in self._changes if c > 0), Decimal(0))
        losses = sum((-c for c in self._changes if c < 0), Decimal(0))
        denom = gains + losses
        self._last = ((gains - losses) / denom * Decimal(100)) if denom != 0 else Decimal(0)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Dpo:
    """Detrended Price Oscillator."""

    def __init__(self, period: int = 20) -> None:
        self.period = period
        self._sma = Sma(period)
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        avg = self._sma.update(p) or p
        self._last = p - avg
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class FisherTransform:
    """Fisher Transform."""

    def __init__(self, period: int = 10) -> None:
        self.period = period
        self._values: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        v = max(Decimal(-1), min(Decimal(1), _D(value)))
        self._values.append(v)
        self._last = v
        return v

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Qqe:
    """Qualitative Quantitative Estimate."""

    def __init__(self, period: int = 10, qqe_smoothing: int = 5) -> None:
        self.period = period
        self.qqe_smoothing = qqe_smoothing
        self._rsi = Rsi(period)
        self._ema = Ema(qqe_smoothing)

    def update(self, price: Decimal | int | str) -> Decimal:
        return self._ema.update(self._rsi.update(price))

    @property
    def value(self) -> Optional[Decimal]:
        return self._ema.value


class Tsi:
    """True Strength Index."""

    def __init__(self, period1: int = 13, period2: int = 25) -> None:
        self.period1 = period1
        self.period2 = period2
        self._prev: Optional[Decimal] = None
        self._ema1 = Ema(period2)
        self._ema2 = Ema(period1)
        self._abs_ema1 = Ema(period2)
        self._abs_ema2 = Ema(period1)
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        p = _D(price)
        diff = p - self._prev if self._prev is not None else Decimal(0)
        self._prev = p
        num = self._ema2.update(self._ema1.update(diff))
        den = self._abs_ema2.update(self._abs_ema1.update(abs(diff)))
        self._last = (num / den * Decimal(100)) if den != 0 else Decimal(0)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Adx:
    """Average Directional Index."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._atr = Atr(period)
        self._last: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
        prev_close: Optional[Decimal | int | str] = None,
    ) -> Decimal:
        h, l, c = _D(high), _D(low), _D(close)
        pc = _D(prev_close) if prev_close is not None else c
        self._last = self._atr.update(h, l, c, pc)
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


# ===========================================================================
# Region 3: Volatility, Range & Bands (Streaming)
# ===========================================================================


class Atr:
    """Average True Range."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._trues: list[Decimal] = []
        self._prev_close: Optional[Decimal] = None
        self._last: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
        prev_close: Optional[Decimal | int | str] = None,
    ) -> Decimal:
        h, l, c = _D(high), _D(low), _D(close)
        pc = _D(prev_close) if prev_close is not None else (self._prev_close or c)
        self._prev_close = c
        tr = max(h - l, abs(h - pc), abs(l - pc))
        self._trues.append(tr)
        if len(self._trues) > self.period:
            self._trues = self._trues[-self.period :]
        self._last = sum(self._trues, Decimal(0)) / Decimal(len(self._trues))
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class BollingerBands:
    """Bollinger Bands (middle, upper, lower)."""

    def __init__(self, period: int = 20, num_std: float = 2.0) -> None:
        self.period = period
        self.num_std = num_std
        self._close: list[Decimal] = []
        self._last: Optional[tuple[Decimal, Decimal, Decimal]] = None

    def update(self, close: Decimal | int | str) -> tuple[Decimal, Decimal, Decimal]:
        c = _D(close)
        self._close.append(c)
        if len(self._close) > self.period:
            self._close = self._close[-self.period :]
        if len(self._close) >= self.period:
            middle = sum(self._close, Decimal(0)) / Decimal(self.period)
            variance = sum(((x - middle) ** 2 for x in self._close), Decimal(0)) / Decimal(
                self.period
            )
            std = variance.sqrt()
            k = _D(self.num_std)
            self._last = (middle, middle + k * std, middle - k * std)
            return self._last
        return Decimal(0), Decimal(0), Decimal(0)

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal, Decimal]]:
        return self._last


class KeltnerChannel:
    """Keltner Channel."""

    def __init__(self, period: int = 20, num_std: float = 2.0) -> None:
        self.period = period
        self.num_std = num_std
        self._close: list[Decimal] = []
        self._last: Optional[tuple[Decimal, Decimal, Decimal]] = None

    def update(self, close: Decimal | int | str) -> tuple[Decimal, Decimal, Decimal]:
        c = _D(close)
        self._close.append(c)
        if len(self._close) >= self.period:
            middle = sum(self._close[-self.period :], Decimal(0)) / Decimal(self.period)
            self._last = (middle, middle, middle)
            return self._last
        return Decimal(0), Decimal(0), Decimal(0)

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal, Decimal]]:
        return self._last


class DonchianChannel:
    """Donchian Channel (middle, upper, lower)."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._high: list[Decimal] = []
        self._low: list[Decimal] = []
        self._last: Optional[tuple[Decimal, Decimal, Decimal]] = None

    def update(self, high: Decimal | int | str, low: Decimal | int | str) -> tuple[Decimal, Decimal, Decimal]:
        self._high.append(_D(high))
        self._low.append(_D(low))
        if len(self._high) >= self.period:
            hh = max(self._high[-self.period :])
            ll = min(self._low[-self.period :])
            middle = (hh + ll) / Decimal(2)
            self._last = (middle, hh, ll)
            return self._last
        return Decimal(0), Decimal(0), Decimal(0)

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal, Decimal]]:
        return self._last


class StandardDeviation:
    """Rolling Standard Deviation."""

    def __init__(self, period: int = 20) -> None:
        self.period = period
        self._values: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, value: Decimal | int | str) -> Decimal:
        self._values.append(_D(value))
        if len(self._values) > self.period:
            self._values = self._values[-self.period :]
        if len(self._values) < 2:
            self._last = Decimal(0)
            return self._last
        mean = sum(self._values, Decimal(0)) / Decimal(len(self._values))
        variance = sum(((v - mean) ** 2 for v in self._values), Decimal(0)) / Decimal(
            len(self._values)
        )
        self._last = variance.sqrt()
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class HistoricalVolatility:
    """Annualized Historical Volatility."""

    def __init__(self, period: int = 20, annualization_factor: float = 16.0) -> None:
        self.period = period
        self.annualization_factor = annualization_factor
        self._prices: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, price: Decimal | int | str) -> Decimal:
        self._prices.append(_D(price))
        if len(self._prices) > self.period:
            self._prices = self._prices[-self.period :]
        if len(self._prices) < 2:
            self._last = Decimal(0)
            return self._last
        returns = [
            self._prices[i] / self._prices[i - 1] - Decimal(1)
            for i in range(1, len(self._prices))
            if self._prices[i - 1] != 0
        ]
        if len(returns) < 2:
            self._last = Decimal(0)
            return self._last
        mean = sum(returns, Decimal(0)) / Decimal(len(returns))
        variance = sum(((r - mean) ** 2 for r in returns), Decimal(0)) / Decimal(len(returns))
        self._last = variance.sqrt() * _D(self.annualization_factor).sqrt()
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class ChaikinVolatility:
    """Chaikin Volatility."""

    def __init__(self, period: int = 10) -> None:
        self.period = period
        self._high_lows: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(self, high: Decimal | int | str, low: Decimal | int | str) -> Decimal:
        hl = _D(high) - _D(low)
        self._high_lows.append(hl)
        if len(self._high_lows) > self.period:
            self._high_lows = self._high_lows[-self.period :]
        if len(self._high_lows) < 2:
            self._last = Decimal(0)
            return self._last
        self._last = (max(self._high_lows) - min(self._high_lows)) / (
            max(self._high_lows) + Decimal(1)
        )
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Supertrend:
    """ATR-based Supertrend indicator."""

    def __init__(self, period: int = 10, multiplier: float = 3.0) -> None:
        self.period = period
        self.multiplier = multiplier
        self._atr_ind = Atr(period)
        self._direction: int = 1
        self._final: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> tuple[Decimal, int]:
        h, l, c = _D(high), _D(low), _D(close)
        atr_val = self._atr_ind.update(h, l, c)
        hl2 = (h + l) / Decimal(2)
        m = _D(self.multiplier)
        upper = hl2 + m * atr_val
        lower = hl2 - m * atr_val
        if self._final is None:
            self._final = lower
            self._direction = 1
        elif c > self._final:
            self._direction = 1
            self._final = max(lower, self._final)
        else:
            self._direction = -1
            self._final = min(upper, self._final)
        return self._final, self._direction

    @property
    def value(self) -> Optional[Decimal]:
        return self._final


class Psar:
    """Parabolic SAR."""

    def __init__(self, step: float = 0.02, max_step: float = 0.20) -> None:
        self.step = _D(step)
        self.max_step = _D(max_step)
        self._sar: Optional[Decimal] = None

    def update(self, high: Decimal | int | str, low: Decimal | int | str) -> Decimal:
        h, l = _D(high), _D(low)
        if self._sar is None:
            self._sar = l
        else:
            self._sar = self._sar + self.step * (h - self._sar)
        return self._sar

    @property
    def value(self) -> Optional[Decimal]:
        return self._sar


class Cpr:
    """Central Pivot Range (Pivot, BC, TC)."""

    def __init__(self) -> None:
        self._last: Optional[tuple[Decimal, Decimal, Decimal]] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> tuple[Decimal, Decimal, Decimal]:
        h, l, c = _D(high), _D(low), _D(close)
        pivot = (h + l + c) / Decimal(3)
        bc = (h + l) / Decimal(2)
        tc = Decimal(2) * pivot - bc
        self._last = (pivot, bc, tc)
        return self._last

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal, Decimal]]:
        return self._last


class Ichimoku:
    """Ichimoku Cloud (Tenkan-sen, Kijun-sen, Senkou Span A/B)."""

    def __init__(self, tenkan: int = 9, kijun: int = 26, senkou_b: int = 52) -> None:
        self._tenkan = DonchianChannel(tenkan)
        self._kijun = DonchianChannel(kijun)
        self._senkou_b = DonchianChannel(senkou_b)
        self._last: Optional[tuple[Decimal, Decimal, Decimal, Decimal]] = None

    def update(
        self, high: Decimal | int | str, low: Decimal | int | str
    ) -> tuple[Decimal, Decimal, Decimal, Decimal]:
        t_mid, _, _ = self._tenkan.update(high, low)
        k_mid, _, _ = self._kijun.update(high, low)
        sb_mid, _, _ = self._senkou_b.update(high, low)
        sa = (t_mid + k_mid) / Decimal(2)
        self._last = (t_mid, k_mid, sa, sb_mid)
        return self._last

    @property
    def value(self) -> Optional[tuple[Decimal, Decimal, Decimal, Decimal]]:
        return self._last


# ===========================================================================
# Region 4: Volume & Microstructure (Streaming)
# ===========================================================================


class Obv:
    """On-Balance Volume."""

    def __init__(self) -> None:
        self._volume: Decimal = Decimal(0)
        self._price: Decimal = Decimal(0)

    def update(self, price: Decimal | int | str, volume: Decimal | int | str) -> Decimal:
        p, v = _D(price), _D(volume)
        if p > self._price:
            self._volume += v
        elif p < self._price:
            self._volume -= v
        self._price = p
        return self._volume

    @property
    def value(self) -> Decimal:
        return self._volume


class Vwap:
    """Session Volume Weighted Average Price."""

    def __init__(self) -> None:
        self._total_price_volume: Decimal = Decimal(0)
        self._total_volume: Decimal = Decimal(0)

    def update(
        self, price: Decimal | int | str, volume: Decimal | int | str
    ) -> tuple[Decimal, Decimal]:
        p, v = _D(price), _D(volume)
        self._total_price_volume += p * v
        self._total_volume += v
        if self._total_volume != 0:
            return self._total_price_volume / self._total_volume, self._total_volume
        return Decimal(0), Decimal(0)

    @property
    def value(self) -> Optional[Decimal]:
        if self._total_volume != 0:
            return self._total_price_volume / self._total_volume
        return None


class AdLine:
    """Accumulation/Distribution Line."""

    def __init__(self) -> None:
        self._value: Decimal = Decimal(0)

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
        volume: Decimal | int | str,
    ) -> Decimal:
        h, l, c, v = _D(high), _D(low), _D(close), _D(volume)
        mf_multiplier = (
            ((c - l) - (h - c)) / (h - l + Decimal(1)) if (h - l) != 0 else Decimal(0)
        )
        self._value += mf_multiplier * v
        return self._value

    @property
    def value(self) -> Decimal:
        return self._value


class Cmf:
    """Chaikin Money Flow."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._mf_multipliers: list[Decimal] = []
        self._mf_volumes: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
        volume: Decimal | int | str,
    ) -> Decimal:
        h, l, c, v = _D(high), _D(low), _D(close), _D(volume)
        mf_multiplier = (
            ((c - l) - (h - c)) / (h - l + Decimal(1)) if (h - l) != 0 else Decimal(0)
        )
        self._mf_multipliers.append(mf_multiplier)
        self._mf_volumes.append(v)
        if len(self._mf_multipliers) > self.period:
            self._mf_multipliers = self._mf_multipliers[-self.period :]
            self._mf_volumes = self._mf_volumes[-self.period :]
        vol_sum = sum(self._mf_volumes, Decimal(0))
        if vol_sum == 0:
            self._last = Decimal(0)
        else:
            mf_sum = sum(
                (m * vol for m, vol in zip(self._mf_multipliers, self._mf_volumes)), Decimal(0)
            )
            self._last = mf_sum / vol_sum
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class Mfi:
    """Money Flow Index."""

    def __init__(self, period: int = 14) -> None:
        self.period = period
        self._prev_tp: Optional[Decimal] = None
        self._pos_mf: list[Decimal] = []
        self._neg_mf: list[Decimal] = []
        self._last: Optional[Decimal] = None

    def update(
        self,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
        volume: Decimal | int | str,
    ) -> Decimal:
        h, l, c, v = _D(high), _D(low), _D(close), _D(volume)
        tp = (h + l + c) / Decimal(3)
        raw_mf = tp * v
        if self._prev_tp is not None:
            if tp >= self._prev_tp:
                self._pos_mf.append(raw_mf)
                self._neg_mf.append(Decimal(0))
            else:
                self._pos_mf.append(Decimal(0))
                self._neg_mf.append(raw_mf)
            if len(self._pos_mf) > self.period:
                self._pos_mf = self._pos_mf[-self.period :]
                self._neg_mf = self._neg_mf[-self.period :]
        self._prev_tp = tp
        pos_sum = sum(self._pos_mf, Decimal(0))
        neg_sum = sum(self._neg_mf, Decimal(0))
        if neg_sum == 0:
            self._last = Decimal(50) if pos_sum == 0 else Decimal(100)
        else:
            mfr = pos_sum / neg_sum
            self._last = Decimal(100) - (Decimal(100) / (Decimal(1) + mfr))
        return self._last

    @property
    def value(self) -> Optional[Decimal]:
        return self._last


class VolumeProfile:
    """Volume Profile."""

    def __init__(self) -> None:
        self._price_levels: dict[Decimal, Decimal] = {}

    def update(self, price: Decimal | int | str, volume: Decimal | int | str) -> Decimal:
        p, v = _D(price), _D(volume)
        self._price_levels[p] = self._price_levels.get(p, Decimal(0)) + v
        return self.value or p

    @property
    def value(self) -> Optional[Decimal]:
        if not self._price_levels:
            return None
        return max(self._price_levels.items(), key=lambda kv: kv[1])[0]


class MarketDepthImbalance:
    """Market Depth Order Book Imbalance."""

    def __init__(self) -> None:
        self._buy_volume: Decimal = Decimal(0)
        self._sell_volume: Decimal = Decimal(0)

    def update(
        self, buy_volume: Decimal | int | str, sell_volume: Decimal | int | str
    ) -> tuple[Decimal, Decimal]:
        self._buy_volume += _D(buy_volume)
        self._sell_volume += _D(sell_volume)
        return self._buy_volume, self._sell_volume

    @property
    def value(self) -> Decimal:
        total = self._buy_volume + self._sell_volume
        if total == 0:
            return Decimal(0)
        return (self._buy_volume - self._sell_volume) / total


# ===========================================================================
# Region 5: Candlestick Patterns
# ===========================================================================


class Doji:
    """Doji candlestick pattern detector."""

    def __init__(self, body_threshold: float = 0.1) -> None:
        self.body_threshold = body_threshold

    def detect(
        self,
        open: Decimal | int | str,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> bool:
        o, h, l, c = _D(open), _D(high), _D(low), _D(close)
        rng = h - l
        if rng <= 0:
            return True
        return abs(c - o) <= _D(self.body_threshold) * rng


class Hammer:
    """Hammer candlestick pattern detector."""

    def __init__(self, body_threshold: float = 0.3, lower_shadow_ratio: float = 2.0) -> None:
        self.body_threshold = body_threshold
        self.lower_shadow_ratio = lower_shadow_ratio

    def detect(
        self,
        open: Decimal | int | str,
        high: Decimal | int | str,
        low: Decimal | int | str,
        close: Decimal | int | str,
    ) -> bool:
        o, h, l, c = _D(open), _D(high), _D(low), _D(close)
        rng = h - l
        if rng <= 0:
            return False
        body = abs(c - o)
        lower_shadow = min(o, c) - l
        return body <= _D(self.body_threshold) * rng and lower_shadow >= _D(self.lower_shadow_ratio) * body


class Engulfing:
    """Engulfing candlestick pattern detector."""

    def detect(
        self,
        prev_open: Decimal | int | str,
        prev_close: Decimal | int | str,
        open: Decimal | int | str,
        close: Decimal | int | str,
    ) -> str:
        po, pc, o, c = _D(prev_open), _D(prev_close), _D(open), _D(close)
        if o > pc and c < po:
            return "bearish"
        if o < pc and c > po:
            return "bullish"
        return "neutral"


class MorningStar:
    """Morning Star candlestick pattern detector."""

    def detect(
        self,
        prev_open: Decimal | int | str,
        prev_close: Decimal | int | str,
        open: Decimal | int | str,
        mid: Decimal | int | str,
        close: Decimal | int | str,
    ) -> str:
        return "bullish" if _D(close) > _D(open) else "neutral"


class ThreeWhiteSoldiers:
    """Three White Soldiers candlestick pattern detector."""

    def detect(
        self,
        o1: Decimal | int | str,
        c1: Decimal | int | str,
        o2: Decimal | int | str,
        c2: Decimal | int | str,
        o3: Decimal | int | str,
        c3: Decimal | int | str,
    ) -> str:
        pairs = [(_D(c1), _D(o1)), (_D(c2), _D(o2)), (_D(c3), _D(o3))]
        return "bullish" if all(c > o for c, o in pairs) else "neutral"


# ===========================================================================
# Region 6: Vectorized Batch Compute (Rust-accelerated + Arrow/Polars/Pandas)
# ===========================================================================


def _to_float_list(series: Any) -> list[float]:
    if hasattr(series, "to_list"):
        return [float(x) for x in series.to_list()]
    if hasattr(series, "tolist"):
        return [float(x) for x in series.tolist()]
    return [float(x) for x in series]


def _wrap_output(result: list[Optional[float]], template: Any) -> Any:
    cls_name = type(template).__module__.split(".")[0]
    if cls_name == "polars":
        import polars as pl

        return pl.Series(values=result, dtype=pl.Float64)
    if cls_name == "pyarrow":
        import pyarrow as pa

        return pa.array(result, type=pa.float64())
    return result


def sma(values: Sequence[Any], period: int = 14) -> Any:
    """Vectorized SMA computed in Rust (`honba-me-indicators`) with the GIL released."""
    return _wrap_output(_batch_sma(_to_float_list(values), period), values)


def ema(values: Sequence[Any], period: int = 14) -> Any:
    """Vectorized EMA computed in Rust (`honba-me-indicators`) with the GIL released."""
    return _wrap_output(_batch_ema(_to_float_list(values), period), values)


def rma(values: Sequence[Any], period: int = 14) -> Any:
    """Vectorized Wilder RMA computed in Rust (`honba-me-indicators`) with the GIL released."""
    return _wrap_output(_batch_rma(_to_float_list(values), period), values)


def rsi(values: Sequence[Any], period: int = 14) -> Any:
    """Vectorized RSI computed in Rust (`honba-me-indicators`) with the GIL released."""
    return _wrap_output(_batch_rsi(_to_float_list(values), period), values)


def bollinger_bands(
    values: Sequence[Any], period: int = 20, num_std: float = 2.0
) -> tuple[Any, Any, Any]:
    """Vectorized Bollinger Bands `(middle, upper, lower)` computed in Rust."""
    m, u, l = _batch_bollinger(_to_float_list(values), period, float(num_std))
    return _wrap_output(m, values), _wrap_output(u, values), _wrap_output(l, values)


def macd(
    values: Sequence[Any],
    fast_period: int = 12,
    slow_period: int = 26,
    signal_period: int = 9,
) -> tuple[Any, Any, Any]:
    """Vectorized MACD `(macd, signal, histogram)` computed in Rust."""
    m, s, h = _batch_macd(_to_float_list(values), fast_period, slow_period, signal_period)
    return _wrap_output(m, values), _wrap_output(s, values), _wrap_output(h, values)


def atr(
    high: Sequence[Any], low: Sequence[Any], close: Sequence[Any], period: int = 14
) -> Any:
    """Vectorized ATR computed in Rust (`honba-me-indicators`) with the GIL released."""
    out = _batch_atr(_to_float_list(high), _to_float_list(low), _to_float_list(close), period)
    return _wrap_output(out, close)


def donchian(
    high: Sequence[Any], low: Sequence[Any], period: int = 20
) -> tuple[Any, Any, Any]:
    """Vectorized Donchian Channel `(middle, upper, lower)` computed in Rust."""
    m, u, l = _batch_donchian(_to_float_list(high), _to_float_list(low), period)
    return _wrap_output(m, high), _wrap_output(u, high), _wrap_output(l, high)


def stochastic(
    high: Sequence[Any],
    low: Sequence[Any],
    close: Sequence[Any],
    k_period: int = 14,
    d_period: int = 3,
) -> tuple[Any, Any]:
    """Vectorized Stochastic `(%K, %D)` computed in Rust."""
    k, d = _batch_stochastic(
        _to_float_list(high), _to_float_list(low), _to_float_list(close), k_period, d_period
    )
    return _wrap_output(k, close), _wrap_output(d, close)


def williams_r(
    high: Sequence[Any], low: Sequence[Any], close: Sequence[Any], period: int = 14
) -> Any:
    """Vectorized Williams %R computed in Rust."""
    out = _batch_williams_r(
        _to_float_list(high), _to_float_list(low), _to_float_list(close), period
    )
    return _wrap_output(out, close)


def supertrend(
    high: Sequence[Any],
    low: Sequence[Any],
    close: Sequence[Any],
    period: int = 10,
    multiplier: float = 3.0,
) -> tuple[Any, list[int]]:
    """Vectorized Supertrend `(supertrend, direction)` computed in Rust."""
    st, direction = _batch_supertrend(
        _to_float_list(high),
        _to_float_list(low),
        _to_float_list(close),
        period,
        float(multiplier),
    )
    return _wrap_output(st, close), direction


def vwap(
    high: Sequence[Any],
    low: Sequence[Any],
    close: Sequence[Any],
    volume: Sequence[Any],
) -> Any:
    """Vectorized VWAP computed in Rust."""
    out = _batch_vwap(
        _to_float_list(high),
        _to_float_list(low),
        _to_float_list(close),
        _to_float_list(volume),
    )
    return _wrap_output(out, close)


def cpr(
    high: Sequence[Any], low: Sequence[Any], close: Sequence[Any]
) -> tuple[Any, Any, Any, Any]:
    """Vectorized Central Pivot Range `(pivot, bc, tc, width)` computed in Rust."""
    p, bc, tc, w = _batch_cpr(
        _to_float_list(high), _to_float_list(low), _to_float_list(close)
    )
    return (
        _wrap_output(p, close),
        _wrap_output(bc, close),
        _wrap_output(tc, close),
        _wrap_output(w, close),
    )


def adx(
    high: Sequence[Any], low: Sequence[Any], close: Sequence[Any], period: int = 14
) -> Any:
    """Vectorized ADX computed in Rust."""
    out = _batch_adx(_to_float_list(high), _to_float_list(low), _to_float_list(close), period)
    return _wrap_output(out, close)


def obv(close: Sequence[Any], volume: Sequence[Any]) -> Any:
    """Vectorized On-Balance Volume (OBV) computed in Rust."""
    out = _batch_obv(_to_float_list(close), _to_float_list(volume))
    return _wrap_output(out, close)


class HonbaSeriesNamespace:
    """Vectorized `.honba` namespace accessor for series and expressions."""

    def __init__(self, series: Any) -> None:
        self._s = series

    def sma(self, period: int = 14) -> Any:
        return sma(self._s, period)

    def ema(self, period: int = 14) -> Any:
        return ema(self._s, period)

    def rma(self, period: int = 14) -> Any:
        return rma(self._s, period)

    def rsi(self, period: int = 14) -> Any:
        return rsi(self._s, period)

    def bollinger_bands(self, period: int = 20, num_std: float = 2.0) -> tuple[Any, Any, Any]:
        return bollinger_bands(self._s, period, num_std)

    def macd(
        self, fast_period: int = 12, slow_period: int = 26, signal_period: int = 9
    ) -> tuple[Any, Any, Any]:
        return macd(self._s, fast_period, slow_period, signal_period)


def _register_namespaces() -> None:
    """Register `.honba` namespace on Polars Series/Expr and Pandas Series when available."""
    try:
        import polars as pl

        @pl.api.register_series_namespace("honba")
        class _PolarsSeriesHonba(HonbaSeriesNamespace):
            pass

        @pl.api.register_expr_namespace("honba")
        class _PolarsExprHonba:
            def __init__(self, expr: pl.Expr) -> None:
                self._expr = expr

            def sma(self, period: int = 14) -> pl.Expr:
                return self._expr.map_batches(lambda s: sma(s, period))

            def ema(self, period: int = 14) -> pl.Expr:
                return self._expr.map_batches(lambda s: ema(s, period))

            def rma(self, period: int = 14) -> pl.Expr:
                return self._expr.map_batches(lambda s: rma(s, period))

            def rsi(self, period: int = 14) -> pl.Expr:
                return self._expr.map_batches(lambda s: rsi(s, period))
    except Exception:
        pass


_register_namespaces()


__all__ = [
    "T3",
    "AcceleratorOscillator",
    "AdLine",
    "Adx",
    "Alma",
    "Atr",
    "AwesomeOscillator",
    "BollingerBands",
    "Cci",
    "ChaikinVolatility",
    "Cmf",
    "Cmo",
    "Cpr",
    "Dema",
    "Doji",
    "DonchianChannel",
    "Dpo",
    "Ema",
    "Engulfing",
    "FisherTransform",
    "Frama",
    "Hammer",
    "HistoricalVolatility",
    "Hma",
    "HonbaSeriesNamespace",
    "Ichimoku",
    "Kama",
    "KeltnerChannel",
    "Macd",
    "MarketDepthImbalance",
    "McGinleyDynamic",
    "Mfi",
    "Momentum",
    "MorningStar",
    "Obv",
    "Psar",
    "Qqe",
    "Rma",
    "Roc",
    "Rsi",
    "Sma",
    "StandardDeviation",
    "StochRsi",
    "Stochastic",
    "Supertrend",
    "Tema",
    "ThreeWhiteSoldiers",
    "Tsi",
    "UltimateOscillator",
    "VolumeProfile",
    "Vwap",
    "Vwma",
    "WilliamsR",
    "Wma",
    "Zlema",
    "adx",
    "atr",
    "bollinger_bands",
    "cpr",
    "donchian",
    "ema",
    "macd",
    "obv",
    "rma",
    "rsi",
    "sma",
    "stochastic",
    "supertrend",
    "vwap",
    "williams_r",
]
