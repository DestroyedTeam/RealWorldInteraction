#!/usr/bin/env python3
"""Explicit engineering cleanup of Cargo caches below work/, never product code.

Requires a reviewable plan and an apply SHA. Preserves named RWI executables in a
content-addressed archive before deleting disposable build intermediates.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

SIGNATURE = "Signature: 8a477f597d28d172789f06886806bc55"


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def cargo_cache(path):
    return (not path.is_symlink() and path.resolve() == path.absolute()
            and (path / ".rustc_info.json").is_file()
            and (path / "CACHEDIR.TAG").is_file()
            and (path / "CACHEDIR.TAG").read_text().startswith(SIGNATURE))


def named_binaries(cache):
    for directory, dirs, files in os.walk(cache, followlinks=False):
        current = Path(directory)
        depth = len(current.relative_to(cache).parts)
        dirs[:] = [d for d in dirs if depth < 2 and d not in {"deps", "build", "incremental", ".fingerprint"}]
        if current.name not in {"debug", "release"}:
            continue
        for name in files:
            path = current / name
            if name.startswith(("rwi-", "rcu-")) and path.suffix in {"", ".exe"} and not path.is_symlink():
                yield path


def plan(work):
    caches = []
    for directory, dirs, _ in os.walk(work, followlinks=False):
        path = Path(directory)
        if cargo_cache(path):
            caches.append({"path": str(path), "binaries": [
                {"path": str(b), "sha256": digest(b), "bytes": b.stat().st_size}
                for b in named_binaries(path)]})
            dirs[:] = []
    return {"version": 1, "work": str(work), "kind": "cargo_build_caches_only",
            "preserve_registry_and_source_and_evidence": True, "caches": caches}


def apply(manifest, archive):
    work = Path(manifest["work"])
    archive.mkdir(parents=True, exist_ok=True)
    if archive == work or archive.is_relative_to(work):
        raise ValueError("archive must be outside work")
    # Validate every destination before the first deletion. Never follow a new
    # root symlink or accept a hand-edited unrelated path from the manifest.
    for cache in manifest["caches"]:
        root = Path(cache["path"])
        if root == work or not root.is_relative_to(work) or not cargo_cache(root):
            raise ValueError(f"cache boundary changed: {root}")
        for binary in cache["binaries"]:
            source = Path(binary["path"])
            if not source.is_relative_to(root) or source.is_symlink() or digest(source) != binary["sha256"]:
                raise ValueError("binary identity changed")
            destination = archive / binary["sha256"]
            if not destination.exists():
                with destination.open("xb") as out, source.open("rb") as inp:
                    shutil.copyfileobj(inp, out)
                destination.chmod(source.stat().st_mode & 0o777)
            if destination.is_symlink() or digest(destination) != binary["sha256"]:
                raise ValueError("archive verification failed")
    log = archive.parent / "deleted-caches.jsonl"
    with log.open("x") as stream:
        for cache in manifest["caches"]:
            root = Path(cache["path"])
            if not cargo_cache(root):
                raise ValueError("cache changed before deletion")
            stream.write(json.dumps({"intent": str(root), "named_binaries_archived": cache["binaries"]}) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
            shutil.rmtree(root)
            stream.write(json.dumps({"deleted": str(root), "absent": not root.exists()}) + "\n")
            stream.flush()
            os.fsync(stream.fileno())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--apply-sha256")
    parser.add_argument("--archive", type=Path)
    args = parser.parse_args()
    work = args.work.absolute()
    if work.name != "work" or work.resolve() != work:
        raise ValueError("literal work directory required")
    if args.apply_sha256:
        if not args.archive or digest(args.plan) != args.apply_sha256:
            raise ValueError("explicit verified plan SHA and archive required")
        manifest = json.loads(args.plan.read_text())
        if manifest["work"] != str(work) or manifest["kind"] != "cargo_build_caches_only":
            raise ValueError("plan scope mismatch")
        apply(manifest, args.archive.absolute())
        print(json.dumps({"deleted_caches": len(manifest["caches"]), "archive": str(args.archive)}))
    else:
        manifest = plan(work)
        with args.plan.open("x") as stream:
            json.dump(manifest, stream, indent=2)
        print(json.dumps({"caches": len(manifest["caches"]), "named_binaries": sum(len(c["binaries"]) for c in manifest["caches"]), "plan_sha256": digest(args.plan)}))


if __name__ == "__main__":
    main()
