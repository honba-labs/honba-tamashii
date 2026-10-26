"""Unit and integration tests for Phase 3: Strategy Catalog Multi-Version Support."""

from pathlib import Path

import pytest
from honba.strategies import load_named as load_named_pkg
from honba.strategies.loader import (
    CatalogError,
    CatalogStrategy,
    find_catalog,
    load_named,
)

WORKSPACE_ROOT = Path(__file__).resolve().parent.parent.parent
CATALOG_PATH = WORKSPACE_ROOT / "honba-strategies"


def test_find_catalog_with_explicit_path():
    cat = find_catalog(CATALOG_PATH)
    assert cat.is_dir()
    assert (cat / "registry.json").is_file()


def test_load_named_default_latest():
    """Verify loading without version constraint loads latest recommended version (v2.0.0)."""
    strat = load_named("twap", strategies_dir=CATALOG_PATH)
    assert isinstance(strat, CatalogStrategy)
    assert strat.name == "twap"
    assert strat.version == "2.0.0"
    assert strat.strategy_api_version == "2.0.0"
    assert getattr(strat.cls, "__strategy_api_version__", None) == "2.0.0"

    # Instantiate
    instance = strat.instantiate()
    assert instance.name == "twap"
    assert getattr(instance, "__strategy_api_version__", None) == "2.0.0"


def test_load_named_exact_version_v1():
    """Verify loading with exact historical version loads v1 (1.2.0) for existing software."""
    strat = load_named("twap", version="1.2.0", strategies_dir=CATALOG_PATH)
    assert isinstance(strat, CatalogStrategy)
    assert strat.name == "twap"
    assert strat.version == "1.2.0"
    assert strat.strategy_api_version == "1.0.0"
    assert getattr(strat.cls, "__strategy_api_version__", None) == "1.0.0"

    # Instantiate
    instance = strat.instantiate()
    assert instance.name == "twap"
    assert getattr(instance, "__strategy_api_version__", None) == "1.0.0"


def test_load_named_semver_range_caret_v1():
    """Verify SemVer range '^1.0.0' resolves to v1.2.0 for existing software."""
    strat = load_named("twap", version="^1.0.0", strategies_dir=CATALOG_PATH)
    assert strat.version == "1.2.0"
    assert strat.strategy_api_version == "1.0.0"


def test_load_named_semver_range_caret_v2():
    """Verify SemVer range '^2.0.0' resolves to v2.0.0."""
    strat = load_named("twap", version="^2.0.0", strategies_dir=CATALOG_PATH)
    assert strat.version == "2.0.0"
    assert strat.strategy_api_version == "2.0.0"


def test_load_named_semver_range_tilde():
    """Verify SemVer range '~1.2.0' resolves to 1.2.0."""
    strat = load_named("twap", version="~1.2.0", strategies_dir=CATALOG_PATH)
    assert strat.version == "1.2.0"


def test_load_named_comparator_range():
    """Verify comparator '>=1.0.0, <2.0.0' resolves to 1.2.0."""
    strat = load_named("twap", version=">=1.0.0, <2.0.0", strategies_dir=CATALOG_PATH)
    assert strat.version == "1.2.0"


def test_load_named_comparator_gte_picks_highest():
    """Verify '>=1.0.0' picks highest version (2.0.0)."""
    strat = load_named("twap", version=">=1.0.0", strategies_dir=CATALOG_PATH)
    assert strat.version == "2.0.0"


def test_load_named_unsatisfiable_constraint_raises_error():
    """Verify requesting an unavailable version raises a descriptive CatalogError."""
    with pytest.raises(CatalogError) as exc_info:
        load_named("twap", version="^3.0.0", strategies_dir=CATALOG_PATH)
    assert "no version of strategy 'twap' satisfies constraint '^3.0.0'" in str(
        exc_info.value
    )
    assert "1.2.0" in str(exc_info.value)
    assert "2.0.0" in str(exc_info.value)


def test_load_named_single_version_strategy():
    """Verify strategies with standard single-version indexing load properly with SemVer."""
    strat_default = load_named("vwap", strategies_dir=CATALOG_PATH)
    assert strat_default.version == "1.0.0"
    assert strat_default.strategy_api_version == "1.0.0"

    strat_v1 = load_named("vwap", version="^1.0.0", strategies_dir=CATALOG_PATH)
    assert strat_v1.version == "1.0.0"


def test_catalog_strategy_tuple_unpacking():
    """Verify CatalogStrategy supports (cls, config) tuple unpacking for existing callers."""
    cls, config = load_named("twap", strategies_dir=CATALOG_PATH)
    assert cls.name == "twap"
    assert config.name == "twap"
    assert config.params["total_quantity"] == 1000


def test_load_named_exported_from_honba_strategies():
    """Verify load_named is exported at honba.strategies root."""
    strat = load_named_pkg("twap", strategies_dir=CATALOG_PATH)
    assert strat.name == "twap"
    assert strat.version == "2.0.0"
