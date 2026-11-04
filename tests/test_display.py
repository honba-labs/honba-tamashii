"""Tests for honba.display: Column.style_fn conditional per-cell styling."""

import io

from honba.display import Column, render_table


def _render(rows, columns, *, isatty=True):
    out = io.StringIO()
    out.isatty = lambda: isatty  # pretend this is a colour-capable terminal
    render_table(rows, columns, out=out)
    return out.getvalue()


def test_style_fn_applies_per_cell_style():
    rows = [{"sym": "AAA", "pnl": 5.0}, {"sym": "BBB", "pnl": -3.0}]
    cols = [
        Column("sym"),
        Column("pnl", style_fn=lambda v: "green" if v >= 0 else "red"),
    ]
    text = _render(rows, cols)
    # Rich emits ANSI SGR codes for green (32) and red (31) on the respective cells.
    assert "\x1b[32m" in text
    assert "\x1b[31m" in text
    assert "5.00" in text
    assert "-3.00" in text


def test_style_fn_none_falls_back_to_static_style():
    rows = [{"sym": "AAA", "score": 10}]
    cols = [
        Column("sym", style_fn=lambda v: None, style="cyan"),
        Column("score"),
    ]
    # The static `style` keeps applying (ANSI 36 = cyan) when style_fn returns falsy.
    text = _render(rows, cols)
    assert "\x1b[36m" in text
    assert "AAA" in text
    assert "10" in text


def test_style_fn_ignored_without_color():
    rows = [{"sym": "AAA", "pnl": -1.0}]
    cols = [Column("sym"), Column("pnl", style_fn=lambda v: "red")]
    # No colour (not a tty): output renders the same values, just without ANSI styling.
    text = _render(rows, cols, isatty=False)
    assert "AAA" in text
    assert "-1.00" in text
    assert "\x1b[" not in text


def test_style_fn_absent_is_backward_compatible():
    rows = [{"a": 1, "b": "x"}]
    cols = [Column("a"), Column("b", style="dim")]
    # No style_fn at all: behaves exactly as before the extension.
    text = _render(rows, cols)
    assert "1" in text
    assert "x" in text


def test_plain_mode_ignores_style_fn():
    rows = [{"a": 1}]
    out = io.StringIO()
    render_table(rows, [Column("a", style_fn=lambda v: "red")], out=out, plain=True)
    assert "1" in out.getvalue()
