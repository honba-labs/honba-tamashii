"""Base class for honba strategies.

Subclass and override any of:

    setup(self)             — once, before the first bar
    on_bar(self, bar, ctx)  — per bar; call ctx.submit(...) to place orders
    on_stop(self)           — once, after the last bar

The Rust engine calls these in order. A strategy that raises will propagate
out of `Runner.run()` — during the scaffold stage that's the desired
behavior; production wraps these in catch_unwind and marks the strategy
degraded instead.
"""

from __future__ import annotations

from honba._honba import Bar, PyContext


class Strategy:
    def setup(self) -> None:
        """Called once before the first bar. Override to initialise state."""

    def on_bar(self, bar: Bar, ctx: PyContext) -> None:
        """Called for every bar. Override to make trading decisions."""

    def on_stop(self) -> None:
        """Called once after the last bar. Override to log/close out."""
