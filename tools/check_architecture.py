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
    manifest = tomllib.loads((workspace / "Cargo.toml").read_text(encoding="utf-8"))
    errors, hotspots, crates = [], [], {}
    for member in manifest["workspace"]["members"]:
        folder = workspace / member
        package = tomllib.loads((folder / "Cargo.toml").read_text(encoding="utf-8"))
        name = package["package"]["name"]
        crates[name] = package
        dependencies = list(package.get("dependencies", {}).items())
        dependencies.extend(package.get("build-dependencies", {}).items())
        for target in package.get("target", {}).values():
            dependencies.extend(target.get("dependencies", {}).items())
            dependencies.extend(target.get("build-dependencies", {}).items())
        local, external = set(), set()
        for key, value in dependencies:
            if not isinstance(value, dict):
                external.add(key)
                continue
            if value.get("workspace"):
                value = manifest["workspace"].get("dependencies", {}).get(key, {})
            if isinstance(value, dict) and "path" in value:
                local.add(value.get("package", key))
            else:
                external.add(value.get("package", key) if isinstance(value, dict) else key)
        forbidden = local - set(rules["allowed_production_dependencies"].get(name, []))
        if forbidden:
            errors.append(f"{name}: forbidden production dependencies {sorted(forbidden)}")
        # Presentation leaves may not grow an OS/transport dependency through
        # a registry package, renamed key, workspace alias or target/build edge.
        # This manifest ratchet is not a proof of source-code purity.
        presentation = rules.get("presentation_only_packages", {}).get(name)
        if presentation is not None:
            unexpected = external - set(presentation["external_dependencies"])
            if unexpected:
                errors.append(f"{name}: forbidden presentation dependencies {sorted(unexpected)}")
            if package["package"].get("build") or (folder / "build.rs").exists():
                errors.append(f"{name}: presentation package must not have a build script")
        for source in sorted((folder / "src").rglob("*.rs")):
            relative = source.relative_to(workspace).as_posix()
            lines = len(source.read_text(encoding="utf-8").splitlines())
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
    report = audit(args.workspace, json.loads(args.rules.read_text(encoding="utf-8")))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
