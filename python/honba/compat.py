"""Runtime version compatibility checks for Honba ecosystem.

Provides explicit version validation guards:
- `require_honba(">=0.1.0, <0.2.0")`
- `require_strategy_api(major=1)` or `require_strategy_api("2.0.0")`
"""

from __future__ import annotations

import re
from typing import Any

from packaging.specifiers import InvalidSpecifier, SpecifierSet
from packaging.version import InvalidVersion, Version

import honba

__all__ = [
    "CURRENT_STRATEGY_API_VERSION",
    "DEFAULT_STRATEGY_API_VERSION",
    "SUPPORTED_STRATEGY_API_MAJORS",
    "SUPPORTED_STRATEGY_API_VERSIONS",
    "IncompatibleHonbaVersionError",
    "IncompatibleStrategyApiError",
    "IncompatibleVersionError",
    "check_compatibility",
    "require_honba",
    "require_strategy_api",
]

SUPPORTED_STRATEGY_API_VERSIONS: tuple[str, ...] = ("1.0.0", "2.0.0")
SUPPORTED_STRATEGY_API_MAJORS: tuple[int, ...] = (1, 2)
DEFAULT_STRATEGY_API_VERSION: str = "2.0.0"
CURRENT_STRATEGY_API_VERSION: str = "2.0.0"


class IncompatibleVersionError(RuntimeError):
    """Base exception raised when a version constraint is not satisfied."""


class IncompatibleHonbaVersionError(IncompatibleVersionError):
    """Raised when the running honba package version violates a required constraint."""


class IncompatibleStrategyApiError(IncompatibleVersionError):
    """Raised when a required strategy API version is unsupported by the current engine."""


def require_honba(spec: str) -> str:
    """Validate that the current ``honba`` package satisfies *spec*.

    Parameters
    ----------
    spec : str
        PEP 440 version specifier, e.g. ``">=0.1.0, <0.2.0"`` or ``">=0.2.0"``.

    Returns
    -------
    str
        Current ``honba.__version__``.

    Raises
    ------
    IncompatibleHonbaVersionError
        If the current version does not satisfy the constraint.
    """
    current_str = getattr(honba, "__version__", "0.1.0")
    try:
        current_ver = Version(current_str)
    except InvalidVersion:
        # Fallback loose version check if needed
        current_ver = Version("0.1.0")

    # Normalize caret syntax (e.g. ^0.1.0) to standard specifier if provided
    normalized_spec = _normalize_spec(spec)

    try:
        spec_set = SpecifierSet(normalized_spec)
    except InvalidSpecifier as err:
        raise ValueError(f"Invalid honba version specifier: {spec!r}") from err

    if current_ver not in spec_set:
        raise IncompatibleHonbaVersionError(
            f"Installed honba version {current_str} does not satisfy requirement {spec!r}"
        )

    return current_str


def require_strategy_api(
    spec_or_major: int | str | None = None,
    *,
    major: int | None = None,
) -> str:
    """Validate that the engine supports the requested Strategy API version.

    Parameters
    ----------
    spec_or_major : int or str, optional
        Either a major version int (e.g. ``1``, ``2``), a version string (``"1.0.0"``),
        or a version constraint string (``">=1.0.0, <3.0.0"``, ``"^2.0.0"``).
    major : int, optional
        Keyword argument to specify the major version explicitly (e.g. ``major=1``).

    Returns
    -------
    str
        The matched strategy API version (e.g. ``"1.0.0"`` or ``"2.0.0"``).

    Raises
    ------
    IncompatibleStrategyApiError
        If no supported Strategy API version matches the request.
    """
    if major is not None:
        target_major = major
    elif isinstance(spec_or_major, int):
        target_major = spec_or_major
    elif isinstance(spec_or_major, str) and spec_or_major.isdigit():
        target_major = int(spec_or_major)
    else:
        target_major = None

    if target_major is not None:
        if target_major not in SUPPORTED_STRATEGY_API_MAJORS:
            raise IncompatibleStrategyApiError(
                f"Strategy API major version {target_major} is not supported by this engine. "
                f"Supported majors: {list(SUPPORTED_STRATEGY_API_MAJORS)}"
            )
        # Return canonical version for that major
        return f"{target_major}.0.0"

    if isinstance(spec_or_major, str):
        normalized_spec = _normalize_spec(spec_or_major)
        try:
            spec_set = SpecifierSet(normalized_spec)
        except InvalidSpecifier as err:
            raise ValueError(f"Invalid strategy API version specifier: {spec_or_major!r}") from err

        matched = [
            v for v in SUPPORTED_STRATEGY_API_VERSIONS
            if Version(v) in spec_set
        ]
        if not matched:
            raise IncompatibleStrategyApiError(
                f"No supported Strategy API version satisfies constraint {spec_or_major!r}. "
                f"Available versions: {list(SUPPORTED_STRATEGY_API_VERSIONS)}"
            )
        # Return the highest matching version
        return matched[-1]

    # If nothing passed, default to current/latest
    return CURRENT_STRATEGY_API_VERSION


def check_compatibility(
    *,
    honba_spec: str | None = None,
    strategy_api: str | int | None = None,
    major: int | None = None,
) -> bool:
    """Return ``True`` if both honba and strategy API requirements are met, else ``False``."""
    try:
        if honba_spec is not None:
            require_honba(honba_spec)
        if strategy_api is not None or major is not None:
            require_strategy_api(strategy_api, major=major)
        return True
    except (IncompatibleVersionError, ValueError):
        return False


def _normalize_spec(spec: str) -> str:
    """Handle bare versions and SemVer-style '^' and '~' syntax if encountered in specs."""
    spec = spec.strip()
    if not spec:
        return ">=0.0.0"
    if spec.startswith(("<", ">", "=", "!", "~")):
        return spec
    if spec.startswith("^"):
        v_str = spec[1:].strip()
        v = Version(v_str)
        if v.major > 0:
            return f">={v}, <{v.major + 1}.0.0"
        if v.minor > 0:
            return f">={v}, <0.{v.minor + 1}.0"
        return f">={v}, <0.0.{v.micro + 1}"
    return f"=={spec}"
