#!/usr/bin/env python3
"""CI Stub Diff Linter and SemVer Backward Compatibility Checker (Phase 8.1 / M7).

Asserts that generated `_honba.pyi` stubs never remove public symbols or mutate
parameter signatures without a major version bump. Also runs `stubtest honba._honba`.
"""

from __future__ import annotations

import ast
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
STUB_FILE = ROOT_DIR / "_honba.pyi"
BASELINE_FILE = ROOT_DIR / "tests" / "fixtures" / "stub_baseline.json"
PYPROJECT_FILE = ROOT_DIR / "pyproject.toml"

SEMVER_REGEX = re.compile(
    r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)"
    r"(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?"
    r"(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)


def get_package_version() -> tuple[int, int, int]:
    """Parse major, minor, patch from pyproject.toml."""
    text = PYPROJECT_FILE.read_text(encoding="utf-8")
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("version ="):
            version_str = line.split("=")[1].strip().strip('"').strip("'")
            match = SEMVER_REGEX.match(version_str)
            if not match:
                raise ValueError(f"Version '{version_str}' violates strict SemVer!")
            return (
                int(match.group("major")),
                int(match.group("minor")),
                int(match.group("patch")),
            )
    raise ValueError("Could not find version in pyproject.toml")


def extract_api_surface(stub_path: Path) -> dict[str, dict]:
    """Extract public classes, functions, and methods with signatures from .pyi."""
    tree = ast.parse(stub_path.read_text(encoding="utf-8"))
    surface = {
        "constants": {},
        "classes": {},
        "functions": {},
    }

    for node in tree.body:
        if isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            # Module-level constant
            surface["constants"][node.target.id] = ast.unparse(node.annotation)
        elif isinstance(node, ast.FunctionDef):
            # Module-level function
            params = [arg.arg for arg in node.args.args]
            ret = ast.unparse(node.returns) if node.returns else "None"
            surface["functions"][node.name] = {
                "params": params,
                "returns": ret,
            }
        elif isinstance(node, ast.ClassDef):
            # Class
            cls_info = {
                "attributes": {},
                "methods": {},
            }
            for item in node.body:
                if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name):
                    cls_info["attributes"][item.target.id] = ast.unparse(item.annotation)
                elif isinstance(item, ast.FunctionDef):
                    params = [arg.arg for arg in item.args.args]
                    ret = ast.unparse(item.returns) if item.returns else "None"
                    cls_info["methods"][item.name] = {
                        "params": params,
                        "returns": ret,
                    }
            surface["classes"][node.name] = cls_info

    return surface


def compare_surfaces(
    baseline: dict, current: dict, current_version: tuple[int, int, int]
) -> list[str]:
    """Check for removals or signature mutations without a major version bump."""
    violations: list[str] = []
    baseline_version = baseline.get("version", (0, 1, 0))
    if isinstance(baseline_version, list):
        baseline_version = tuple(baseline_version)

    is_major_bump = current_version[0] > baseline_version[0]

    # Check constants
    base_consts = baseline.get("surface", {}).get("constants", {})
    curr_consts = current.get("constants", {})
    for const in base_consts:
        if const not in curr_consts:
            violations.append(f"Removed public constant '{const}'")

    # Check functions
    base_funcs = baseline.get("surface", {}).get("functions", {})
    curr_funcs = current.get("functions", {})
    for fn, info in base_funcs.items():
        if fn not in curr_funcs:
            violations.append(f"Removed public function '{fn}'")
        else:
            curr_params = curr_funcs[fn]["params"]
            base_params = info["params"]
            # Check if any baseline parameter was removed or reordered
            if not is_major_bump:
                if len(curr_params) < len(base_params):
                    violations.append(
                        f"Function '{fn}' reduced parameter count from {len(base_params)} to {len(curr_params)}"
                    )
                for i, p in enumerate(base_params):
                    if i < len(curr_params) and curr_params[i] != p:
                        violations.append(
                            f"Function '{fn}' mutated parameter {i} from '{p}' to '{curr_params[i]}'"
                        )

    # Check classes
    base_classes = baseline.get("surface", {}).get("classes", {})
    curr_classes = current.get("classes", {})
    for cls_name, cls_info in base_classes.items():
        if cls_name not in curr_classes:
            violations.append(f"Removed public class '{cls_name}'")
        else:
            curr_cls = curr_classes[cls_name]
            # Check attributes
            for attr in cls_info.get("attributes", {}):
                if attr not in curr_cls.get("attributes", {}):
                    violations.append(
                        f"Class '{cls_name}' removed public attribute '{attr}'"
                    )
            # Check methods
            for meth, meth_info in cls_info.get("methods", {}).items():
                if meth not in curr_cls.get("methods", {}):
                    violations.append(
                        f"Class '{cls_name}' removed public method '{meth}'"
                    )
                else:
                    curr_params = curr_cls["methods"][meth]["params"]
                    base_params = meth_info["params"]
                    if not is_major_bump:
                        if len(curr_params) < len(base_params):
                            violations.append(
                                f"Method '{cls_name}.{meth}' reduced parameter count from {len(base_params)} to {len(curr_params)}"
                            )
                        for i, p in enumerate(base_params):
                            if i < len(curr_params) and curr_params[i] != p:
                                violations.append(
                                    f"Method '{cls_name}.{meth}' mutated parameter {i} from '{p}' to '{curr_params[i]}'"
                                )

    if violations and not is_major_bump:
        return [
            f"Backward compatibility violation without major version bump (current version {current_version}, baseline {baseline_version}):\n  - "
            + "\n  - ".join(violations)
        ]
    return []


def run_stubtest() -> int:
    """Run stubtest honba._honba."""
    cmd = [sys.executable, "-m", "mypy.stubtest", "honba._honba"]
    proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        print("stubtest failed:\n", proc.stdout, proc.stderr, file=sys.stderr)
        return proc.returncode
    return 0


def main() -> int:
    update_mode = "--update-baseline" in sys.argv
    version = get_package_version()

    if not STUB_FILE.exists():
        print(f"Error: {STUB_FILE} not found!", file=sys.stderr)
        return 1

    current_surface = extract_api_surface(STUB_FILE)

    if update_mode or not BASELINE_FILE.exists():
        BASELINE_FILE.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "version": version,
            "surface": current_surface,
        }
        BASELINE_FILE.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"Updated stub baseline at {BASELINE_FILE} (version {version})")
        return 0

    # Load baseline and compare
    baseline_data = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))
    violations = compare_surfaces(baseline_data, current_surface, version)
    if violations:
        for v in violations:
            print(v, file=sys.stderr)
        return 1

    print("Stub backward compatibility check passed: 0 breaking changes detected.")

    # Run stubtest
    code = run_stubtest()
    if code == 0:
        print("stubtest validation passed: _honba.pyi accurately describes runtime extension.")
    return code


if __name__ == "__main__":
    sys.exit(main())
