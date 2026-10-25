"""Compatibility re-exports for honba.screener.coverage."""

from honba_data.store.coverage import (
    BAR_SCHEMA,
    CoverageRecord,
    CoverageStatus,
    DateInterval,
    merge_close_intervals,
    merge_intervals,
    plan_gaps,
    subtract_intervals,
)

__all__ = [
    "BAR_SCHEMA",
    "CoverageRecord",
    "CoverageStatus",
    "DateInterval",
    "merge_close_intervals",
    "merge_intervals",
    "plan_gaps",
    "subtract_intervals",
]
