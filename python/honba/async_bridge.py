"""AsyncIO-Tokio cooperative execution bridge (Phase 8 / M7).

Enables Python `async def` strategy callbacks to integrate cooperatively with
the Rust Tokio event loop, provides `CancellationToken` / `PyCancellationToken`
for clean timeouts and cancellation, and isolates blocking Python calls onto a
dedicated thread pool to prevent starving Tokio's actor loop.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from honba._honba import (
    PyCancellationToken,
)
from honba._honba import (
    run_coroutine as _run_coroutine,
)
from honba._honba import (
    run_in_blocking_pool as _run_in_blocking_pool,
)
from honba._honba import (
    spawn_blocking_call as _spawn_blocking_call,
)

CancellationToken = PyCancellationToken

__all__ = [
    "CancellationToken",
    "PyCancellationToken",
    "run_coroutine",
    "run_in_blocking_pool",
    "spawn_blocking",
]


def run_coroutine(
    coro: Any,
    timeout_secs: float | None = None,
    token: PyCancellationToken | None = None,
) -> Any:
    """Run a Python coroutine cooperatively with Tokio."""
    return _run_coroutine(coro, timeout_secs, token)


def spawn_blocking(
    callable: Callable[..., Any],
    *args: Any,
    timeout_secs: float | None = None,
) -> Any:
    """Run a blocking callable on the dedicated isolated thread pool, returning an awaitable future."""
    arg_tuple = args if args else None
    return _spawn_blocking_call(callable, arg_tuple, timeout_secs)


def run_in_blocking_pool(
    callable: Callable[..., Any],
    *args: Any,
) -> Any:
    """Synchronously run a blocking callable in the dedicated isolated thread pool while releasing the GIL."""
    arg_tuple = args if args else None
    return _run_in_blocking_pool(callable, arg_tuple)
