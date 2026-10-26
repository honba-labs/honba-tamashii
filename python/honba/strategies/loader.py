"""Load strategies from a ``.py`` file or from a ``honba-strategies`` catalog by registry name.

Infrastructure at the edge: it imports user code. The catalog is a checkout, not an
installed package. Strategies are found through its ``registry.json`` (the top-level
index, then per-strategy ``registry.json`` files) rather than hardcoded paths.

Catalog location (:func:`find_catalog`), first match wins and nothing depends on the
working directory: an explicit path, ``$HONBA_STRATEGIES_DIR``, then (only when the
caller passes ``search_from``) a ``honba-strategies`` directory beside ``search_from``
or one of its parents.
"""

from __future__ import annotations

import hashlib
import importlib.util
import inspect
import json
import os
import sys
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Any

from packaging.specifiers import InvalidSpecifier, SpecifierSet
from packaging.version import Version

from honba.strategies.base import Strategy
from honba.strategies.config import StrategyConfig

__all__ = [
    "CATALOG_DIRNAME",
    "CATALOG_ENV",
    "CatalogError",
    "CatalogRegistryEntry",
    "CatalogStrategy",
    "StrategyVersionInfo",
    "find_catalog",
    "load_catalog_strategy",
    "load_named",
    "load_strategy",
    "resolve_strategy_version",
]

CATALOG_ENV = "HONBA_STRATEGIES_DIR"
CATALOG_DIRNAME = "honba-strategies"


class CatalogError(RuntimeError):
    """A strategy file, the catalog, or a strategy in it could not be found or loaded."""


@dataclass(frozen=True)
class StrategyVersionInfo:
    """Metadata for a specific version of a catalog strategy."""

    version: str
    path: str
    strategy_api_version: str = "1.0.0"
    deprecated: bool = False


@dataclass(frozen=True)
class CatalogRegistryEntry:
    """Strategy catalog registry entry."""

    name: str
    category: str
    latest: str
    versions: dict[str, StrategyVersionInfo]
    description: str = ""
    tags: tuple[str, ...] = ()


@dataclass(frozen=True)
class CatalogStrategy:
    """A catalog strategy with its config and a digest for run provenance."""

    name: str
    path: Path  # strategy directory
    cls: type[Strategy]
    config: StrategyConfig
    module: ModuleType
    source_sha256: str  # sha256 of the whole strategy directory (see _directory_sha256)
    version: str = "1.0.0"
    strategy_api_version: str = "1.0.0"

    def instantiate(self) -> Strategy:
        """Build the strategy: ``cls(config)`` if its constructor takes one, else ``cls()``."""
        return _instantiate(self.cls, self.config)

    def __iter__(self):
        """Allow unpacking as ``(cls, config) = load_named(...)``."""
        return iter((self.cls, self.config))


def _instantiate(cls: type[Strategy], config: StrategyConfig | None) -> Strategy:
    params = [
        p
        for p in inspect.signature(cls.__init__).parameters.values()
        if p.name != "self" and p.kind in (p.POSITIONAL_ONLY, p.POSITIONAL_OR_KEYWORD)
    ]
    if not params or config is None:
        return cls()

    cfg_to_pass: Any = config
    config_cls = getattr(cls, "Config", None)
    if config_cls is not None and not isinstance(config, config_cls):
        raw_params = dict(config.params) if hasattr(config, "params") else {}
        base_kwargs = {
            "name": getattr(config, "name", "strategy"),
            "symbol": getattr(config, "symbol", ""),
            "exchange": getattr(config, "exchange", "NSE"),
        }
        if hasattr(config_cls, "from_dict"):
            try:
                cfg_to_pass = config_cls.from_dict({**base_kwargs, **raw_params})
            except (TypeError, ValueError, AttributeError, KeyError):
                cfg_to_pass = config
        else:
            try:
                cfg_to_pass = config_cls(**base_kwargs, **raw_params)
            except (TypeError, ValueError, AttributeError, KeyError):
                cfg_to_pass = config

    try:
        return cls(cfg_to_pass)  # type: ignore[call-arg]
    except (TypeError, AttributeError):
        if cfg_to_pass is not config:
            return cls(config)
        raise


def _import_file(path: Path, module_name: str) -> ModuleType:
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise CatalogError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    # Registered before exec so decorators (``@dataclass`` with postponed annotations)
    # can look the module up; removed again if the import fails.
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


def _strategy_classes(module: ModuleType) -> list[type[Strategy]]:
    classes = [
        obj
        for _, obj in inspect.getmembers(module, inspect.isclass)
        if issubclass(obj, Strategy)
        and obj is not Strategy
        and obj.__module__ == module.__name__
    ]
    if not classes:
        classes = [
            obj
            for _, obj in inspect.getmembers(module, inspect.isclass)
            if issubclass(obj, Strategy)
            and obj is not Strategy
            and not getattr(obj, "__name__", "").startswith("_")
        ]
    return classes


def load_strategy(path: str | Path) -> type[Strategy]:
    """Import ``path`` and return the one ``Strategy`` subclass it defines."""
    path = Path(path).resolve()
    if not path.is_file():
        raise CatalogError(f"strategy file {path} not found")
    digest = hashlib.sha256(str(path).encode()).hexdigest()[:12]
    module = _import_file(path, f"honba_user_strategy_{path.stem}_{digest}")
    classes = _strategy_classes(module)
    if len(classes) != 1:
        found = (
            "no Strategy subclass"
            if not classes
            else f"{len(classes)} Strategy subclasses"
        )
        raise CatalogError(
            f"{path}: expected exactly one Strategy subclass, found {found}"
        )
    return classes[0]


def _is_catalog(path: Path) -> bool:
    return (path / "registry.json").is_file()


def find_catalog(
    explicit: str | Path | None = None,
    *,
    environ: Mapping[str, str] | None = None,
    search_from: str | Path | None = None,
) -> Path:
    """Return the catalog root, or raise ``CatalogError`` saying how to point at one."""
    environ = os.environ if environ is None else environ
    if explicit is not None:
        if not _is_catalog(Path(explicit)):
            raise CatalogError(
                f"{explicit} is not a honba-strategies catalog (no registry.json)"
            )
        return Path(explicit).resolve()
    env = environ.get(CATALOG_ENV, "").strip()
    if env:
        if not _is_catalog(Path(env)):
            raise CatalogError(
                f"${CATALOG_ENV}={env} is not a catalog (no registry.json)"
            )
        return Path(env).resolve()
    if search_from is not None:
        start = Path(search_from).resolve()
        for base in (start, *start.parents):
            candidate = base.parent / CATALOG_DIRNAME
            if _is_catalog(candidate):
                return candidate.resolve()
    raise CatalogError(
        f"honba-strategies catalog not found: pass its path or set ${CATALOG_ENV}."
    )


_SKIPPED_DIRS = frozenset(
    {"node_modules", "__pycache__", "venv", "site-packages", "build", "dist"}
)


def _scannable(relative: Path) -> bool:
    """False for hidden (``.git``, ``.venv``) and vendor directories anywhere in the path."""
    return not any(
        part.startswith(".") or part in _SKIPPED_DIRS for part in relative.parts
    )


def _read_registry(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise CatalogError(f"unreadable registry {path}: {exc}") from exc


def _normalize_version_spec(spec: str) -> str:
    """Handle SemVer caret and tilde syntax into PEP 440 specifiers."""
    spec = spec.strip()
    if not spec or spec == "*":
        return ">=0.0.0"
    if spec.startswith("^"):
        v_str = spec[1:].strip()
        v = Version(v_str)
        if v.major > 0:
            return f">={v}, <{v.major + 1}.0.0"
        if v.minor > 0:
            return f">={v}, <0.{v.minor + 1}.0"
        return f">={v}, <0.0.{v.micro + 1}"
    if spec.startswith("~"):
        v_str = spec[1:].strip()
        v = Version(v_str)
        return f">={v}, <{v.major}.{v.minor + 1}.0"
    if spec.startswith(("<", ">", "=", "!")):
        return spec
    return f"=={spec}"


def resolve_strategy_version(
    entry: CatalogRegistryEntry,
    version_spec: str | None = None,
) -> StrategyVersionInfo:
    """Resolve a version requirement against an entry's available versions."""
    versions = entry.versions
    if not versions:
        raise CatalogError(
            f"strategy {entry.name!r} has no versions defined in catalog"
        )

    if (
        version_spec is None
        or version_spec == ""
        or version_spec == "*"
        or version_spec == "latest"
    ):
        if entry.latest in versions:
            return versions[entry.latest]
        sorted_vers = sorted(
            versions.values(),
            key=lambda v: (not v.deprecated, Version(v.version)),
            reverse=True,
        )
        return sorted_vers[0]

    if version_spec in versions:
        return versions[version_spec]

    norm = _normalize_version_spec(version_spec)
    try:
        spec_set = SpecifierSet(norm)
    except InvalidSpecifier as exc:
        raise CatalogError(
            f"invalid version constraint {version_spec!r}: {exc}"
        ) from exc

    matching = [
        v_info for v_info in versions.values() if Version(v_info.version) in spec_set
    ]
    if not matching:
        avail = ", ".join(sorted(versions.keys(), key=lambda v: Version(v)))
        raise CatalogError(
            f"no version of strategy {entry.name!r} satisfies constraint {version_spec!r}; available: {avail}"
        )

    return max(
        matching,
        key=lambda v: (not v.deprecated, Version(v.version)),
    )


def _parse_registry(catalog: Path) -> dict[str, CatalogRegistryEntry]:
    """Parse top-level registry.json and sub-registries into CatalogRegistryEntry items."""
    entries: dict[str, CatalogRegistryEntry] = {}
    top_path = catalog / "registry.json"
    top = _read_registry(top_path)
    if not isinstance(top, dict):
        raise CatalogError(f"malformed registry {top_path}: expected a JSON object")
    listed = top.get("strategies", [])
    if not isinstance(listed, list):
        raise CatalogError(
            f"malformed registry {top_path}: 'strategies' must be a list"
        )
    for e in listed:
        if not (isinstance(e, dict) and isinstance(e.get("name"), str)):
            raise CatalogError(
                f"malformed registry {top_path}: each strategy needs a string 'name', got {e!r}"
            )
        name = e["name"]
        category = e.get("category", "")
        description = e.get("description", "")
        tags = tuple(e.get("tags", []))

        versions: dict[str, StrategyVersionInfo] = {}
        if "versions" in e and isinstance(e["versions"], dict):
            for v_str, v_meta in e["versions"].items():
                if isinstance(v_meta, dict) and "path" in v_meta:
                    versions[v_str] = StrategyVersionInfo(
                        version=v_str,
                        path=v_meta["path"],
                        strategy_api_version=v_meta.get(
                            "strategy_api_version", "1.0.0"
                        ),
                        deprecated=bool(v_meta.get("deprecated", False)),
                    )
            latest = e.get("latest")
            if not latest or latest not in versions:
                latest = max(versions.keys(), key=lambda v: Version(v))
        elif "path" in e and isinstance(e["path"], str):
            path = e["path"]
            ver = "1.0.0"
            api_ver = "1.0.0"
            versions[ver] = StrategyVersionInfo(
                version=ver,
                path=path,
                strategy_api_version=api_ver,
                deprecated=False,
            )
            latest = ver
        else:
            raise CatalogError(
                f"malformed registry {top_path}: strategy {name!r} must have 'path' or 'versions'"
            )

        entries[name] = CatalogRegistryEntry(
            name=name,
            category=category,
            latest=latest,
            versions=versions,
            description=description,
            tags=tags,
        )

    for reg in sorted(catalog.glob("**/registry.json")):
        if reg.parent == catalog or not _scannable(reg.parent.relative_to(catalog)):
            continue
        e = _read_registry(reg)
        if isinstance(e, dict) and isinstance(e.get("name"), str):
            name = e["name"]
            path = e.get("path")
            if path is not None and not isinstance(path, str):
                raise CatalogError(f"malformed registry {reg}: 'path' must be a string")
            rel_path = path or str(reg.parent.relative_to(catalog))
            if name not in entries:
                entries[name] = CatalogRegistryEntry(
                    name=name,
                    category=str(reg.parent.parent.relative_to(catalog)),
                    latest="1.0.0",
                    versions={
                        "1.0.0": StrategyVersionInfo(
                            version="1.0.0",
                            path=rel_path,
                            strategy_api_version="1.0.0",
                            deprecated=False,
                        )
                    },
                    description=e.get("description", ""),
                    tags=tuple(e.get("tags", [])),
                )
    return entries


def _registry_entries(catalog: Path) -> dict[str, str]:
    """``{name: relative path}`` for the default/latest version of every strategy."""
    parsed = _parse_registry(catalog)
    return {name: entry.versions[entry.latest].path for name, entry in parsed.items()}


def _directory_sha256(root: Path) -> str:
    """Deterministic digest of every file under ``root`` (run provenance).

    Files are visited in sorted relative-path order; each contributes its length-prefixed
    POSIX relative path and length-prefixed bytes, so file boundaries cannot be confused.
    Hidden files and directories and ``__pycache__`` are ignored.
    """
    digest = hashlib.sha256()
    files = sorted(
        (p.relative_to(root) for p in root.rglob("*") if p.is_file()),
        key=lambda r: r.as_posix(),
    )
    for rel in files:
        if any(part.startswith(".") or part == "__pycache__" for part in rel.parts):
            continue
        name = rel.as_posix().encode()
        data = (root / rel).read_bytes()
        digest.update(len(name).to_bytes(8, "big") + name)
        digest.update(len(data).to_bytes(8, "big") + data)
    return digest.hexdigest()


def _load_module_isolated(
    strategy_dir: Path, catalog: Path, module_name: str
) -> ModuleType:
    """Import ``strategy.py`` with its directory chain on ``sys.path``, then undo the leaks."""
    saved_path = list(sys.path)
    before = set(sys.modules)
    for p in (catalog, strategy_dir.parent.parent, strategy_dir.parent, strategy_dir):
        sys.path.insert(0, str(p))
    try:
        return _import_file(strategy_dir / "strategy.py", module_name)
    finally:
        sys.path[:] = saved_path
        for mod_name in set(sys.modules) - before - {module_name}:
            mod_file = getattr(sys.modules[mod_name], "__file__", None)
            if mod_file and Path(mod_file).resolve().is_relative_to(strategy_dir):
                del sys.modules[mod_name]


def load_catalog_strategy(
    name: str,
    catalog: str | Path,
    version: str | None = None,
) -> CatalogStrategy:
    """Import the strategy registered as ``name`` with optional version resolution.

    The strategy directory, its two parents and the catalog root are put on ``sys.path`` while importing
    (see :func:`_load_module_isolated`). A registry path that resolves outside the
    catalog is refused.
    """
    catalog = Path(catalog).resolve()
    reg_entries = _parse_registry(catalog)
    if name not in reg_entries:
        raise CatalogError(
            f"no strategy named {name!r} in {catalog}; available: {', '.join(sorted(reg_entries))}"
        )
    entry = reg_entries[name]
    v_info = resolve_strategy_version(entry, version)
    strategy_dir = (catalog / v_info.path).resolve()
    if not strategy_dir.is_relative_to(catalog):
        raise CatalogError(
            f"strategy {name!r}: path {v_info.path!r} resolves outside the catalog {catalog}"
        )
    if not (strategy_dir / "strategy.py").is_file():
        raise CatalogError(
            f"strategy {name!r}: {strategy_dir / 'strategy.py'} not found"
        )
    mod_suffix = v_info.version.replace(".", "_").replace("-", "_")
    module = _load_module_isolated(
        strategy_dir, catalog, f"honba_catalog_{name}_{mod_suffix}"
    )
    classes = _strategy_classes(module)
    named = [c for c in classes if getattr(c, "name", None) == name]
    if len(named) == 1:
        cls = named[0]
    elif len(classes) == 1:
        cls = classes[0]
    else:
        raise CatalogError(
            f"{strategy_dir / 'strategy.py'}: expected one Strategy named {name!r}"
        )
    config_file = strategy_dir / "config.toml"
    if not config_file.is_file() and (strategy_dir.parent / "config.toml").is_file():
        config_file = strategy_dir.parent / "config.toml"
    return CatalogStrategy(
        name=name,
        path=strategy_dir,
        cls=cls,
        config=StrategyConfig.from_toml(config_file),
        module=module,
        source_sha256=_directory_sha256(strategy_dir),
        version=v_info.version,
        strategy_api_version=v_info.strategy_api_version,
    )


def load_named(
    name: str,
    version: str | None = None,
    catalog: str | Path | None = None,
    *,
    search_from: str | Path | None = None,
    strategies_dir: str | Path | None = None,
) -> CatalogStrategy:
    """Load a named strategy from the catalog with semantic version resolution.

    Examples
    --------
    >>> # New software loads latest recommended version (v2.0.0)
    >>> strat_latest = load_named("twap")

    >>> # Existing software continues to load v1 without breaking
    >>> strat_v1 = load_named("twap", version="1.2.0")

    >>> # SemVer range resolution for existing pipelines
    >>> strat_v1_compatible = load_named("twap", version="^1.0.0")
    """
    cat = find_catalog(catalog or strategies_dir, search_from=search_from)
    return load_catalog_strategy(name, cat, version=version)
