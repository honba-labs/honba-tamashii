"""Unit tests for CI stub diff linter and SemVer backward compatibility (Phase 8.1 / M7)."""

import copy
import sys
from pathlib import Path
import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from scripts.check_stub_compatibility import (
    SEMVER_REGEX,
    compare_surfaces,
    extract_api_surface,
    get_package_version,
)


def test_semver_format_validation():
    import honba

    assert SEMVER_REGEX.match(honba.__version__) is not None
    major, minor, patch = get_package_version()
    assert (major, minor, patch) == (0, 1, 0)


def test_stub_surface_extraction():
    stub_path = Path("_honba.pyi")
    surface = extract_api_surface(stub_path)

    # Asserts canonical classes and functions exist
    assert "Price" in surface["classes"]
    assert "Quantity" in surface["classes"]
    assert "Money" in surface["classes"]
    assert "Runner" in surface["classes"]
    assert "PyCancellationToken" in surface["classes"]

    assert "canonical_json" in surface["functions"]
    assert "run_coroutine" in surface["functions"]
    assert "spawn_blocking_call" in surface["functions"]


def test_stub_backward_compatibility_no_breaking_changes():
    stub_path = Path("_honba.pyi")
    surface = extract_api_surface(stub_path)
    baseline = {
        "version": (0, 1, 0),
        "surface": copy.deepcopy(surface),
    }

    # Same surface should have zero violations
    violations = compare_surfaces(baseline, surface, (0, 1, 0))
    assert violations == []


def test_stub_backward_compatibility_catches_removal():
    stub_path = Path("_honba.pyi")
    surface = extract_api_surface(stub_path)
    baseline = {
        "version": (0, 1, 0),
        "surface": copy.deepcopy(surface),
    }

    # Simulate removal of a class
    mutated_surface = copy.deepcopy(surface)
    del mutated_surface["classes"]["Price"]

    violations = compare_surfaces(baseline, mutated_surface, (0, 1, 1))
    assert len(violations) > 0
    assert "Removed public class 'Price'" in violations[0]


def test_stub_backward_compatibility_catches_parameter_mutation():
    stub_path = Path("_honba.pyi")
    surface = extract_api_surface(stub_path)
    baseline = {
        "version": (0, 1, 0),
        "surface": copy.deepcopy(surface),
    }

    # Simulate mutated parameter
    mutated_surface = copy.deepcopy(surface)
    mutated_surface["classes"]["Runner"]["methods"]["run"]["params"] = ["self", "mutated_arg"]

    violations = compare_surfaces(baseline, mutated_surface, (0, 2, 0))
    assert len(violations) > 0
    assert "mutated parameter" in violations[0]


def test_stub_major_bump_permits_breaking_changes():
    stub_path = Path("_honba.pyi")
    surface = extract_api_surface(stub_path)
    baseline = {
        "version": (0, 1, 0),
        "surface": copy.deepcopy(surface),
    }

    # Major version bump from 0.x to 1.x allows breaking changes
    mutated_surface = copy.deepcopy(surface)
    del mutated_surface["classes"]["Price"]

    violations = compare_surfaces(baseline, mutated_surface, (1, 0, 0))
    assert violations == []
