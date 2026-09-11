#!/usr/bin/env python3
"""Offline engineering check, not product runtime or an OS automation bridge.

Explicit --workspace prevents accidentally testing the obsolete repository root
and reporting it as validation of the current candidate. No indexing or writes.
"""
import argparse
import json
from pathlib import Path
import tomllib


def audit(workspace: Path, rules: dict) -> dict:
    manifest = tomllib.loads((workspace / "Cargo.toml").read_text())
    errors, hotspots, crates = [], [], {}
    for member in manifest["workspace"]["members"]:
        folder = workspace / member
        package = tomllib.loads((folder / "Cargo.toml").read_text())
        name = package["package"]["name"]
        crates[name] = package
        dependencies = dict(package.get("dependencies", {}))
        for target in package.get("target", {}).values():
            dependencies.update(target.get("dependencies", {}))
        local = {key for key, value in dependencies.items()
                 if isinstance(value, dict) and "path" in value}
        forbidden = local - set(rules["allowed_production_dependencies"].get(name, []))
        if forbidden:
            errors.append(f"{name}: forbidden production dependencies {sorted(forbidden)}")
        for source in sorted((folder / "src").rglob("*.rs")):
            relative = str(source.relative_to(workspace))
            lines = len(source.read_text().splitlines())
            limit = rules["existing_hotspot_ceilings"].get(relative, rules["new_file_line_limit"])
            if lines > limit:
                errors.append(f"{relative}: {lines} lines exceeds {limit}; split by responsibility")
            if lines > rules["new_file_line_limit"]:
                hotspots.append({"path": relative, "lines": lines, "ceiling": limit})
    if set(crates) != set(rules["allowed_production_dependencies"]):
        errors.append("workspace membership differs from candidate baseline; do not confuse old root and candidate")
    return {"workspace": str(workspace.resolve()), "passed": not errors,
            "violations": errors, "existing_debt": hotspots,
            "architecture_complete": False,
            "note": "Ratchet only: permitted existing debt is not an architecture acceptance."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--rules", required=True, type=Path)
    args = parser.parse_args()
    report = audit(args.workspace, json.loads(args.rules.read_text()))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
