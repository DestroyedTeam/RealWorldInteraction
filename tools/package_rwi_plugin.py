"""Operator-only packaging; the MCP runtime and native execution remain Rust."""
import argparse
import json
import os
from pathlib import Path
import shutil
import tempfile
import uuid

NAME = "real-world-interaction"
SOURCE = Path(__file__).resolve().parents[1] / "integrations/codex/plugin" / NAME


def package(binary, profile, output, *, source=SOURCE, replace=False):
    binary, profile, output = map(Path, (binary, profile, output))
    if not all(p.is_absolute() for p in (binary, profile, output)):
        raise ValueError("Use explicit absolute installation paths")
    binary, profile = binary.resolve(strict=True), profile.resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK) or not profile.is_file():
        raise ValueError("An executable Rust adapter and an existing private profile are required")
    if output.name != NAME or output.is_symlink():
        raise ValueError("Output must be a real directory named real-world-interaction")
    output = output.parent.resolve(strict=True) / output.name
    source = source.resolve(strict=True)
    if output == source or output.is_relative_to(source) or source.is_relative_to(output):
        raise ValueError("Installation must be separate from canonical source")
    if binary.is_relative_to(output) or profile.is_relative_to(output):
        raise ValueError("Binary, profile and identity stay outside the plugin bundle")
    for p in source.rglob("*"):
        if p.is_symlink():
            raise ValueError("Plugin source must not contain symlinks")
    if output.exists():
        if not replace or json.loads((output / ".codex-plugin/plugin.json").read_text())["name"] != NAME:
            raise ValueError("Refusing to replace an unrelated or unapproved directory")
    staging = Path(tempfile.mkdtemp(prefix=".rwi-plugin-", dir=output.parent))
    backup = None
    try:
        shutil.copytree(source, staging, dirs_exist_ok=True)
        entry = {"mcpServers": {"real_world_interaction": {
            "command": str(binary), "args": ["--profile", str(profile)]}}}
        (staging / ".mcp.json").write_text(json.dumps(entry, indent=2) + "\n")
        if output.exists():
            backup = output.with_name(".rwi-plugin-backup-" + uuid.uuid4().hex)
            output.rename(backup)
        try:
            staging.rename(output)
        except BaseException:
            if backup is not None:
                backup.rename(output)
            raise
    finally:
        if staging.exists():
            shutil.rmtree(staging)
    return {"plugin": str(output), "backup": str(backup) if backup else None,
            "credentials_copied": False, "runtime_started": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("binary", "profile", "output"):
        parser.add_argument("--" + key, required=True, type=Path)
    parser.add_argument("--replace", action="store_true")
    args = parser.parse_args()
    print(json.dumps(package(args.binary, args.profile, args.output, replace=args.replace)))
