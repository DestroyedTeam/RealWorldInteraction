#!/usr/bin/env python3
"""Read-only, deliberately small Git source hygiene gate. Never prints secrets.

Not a complete secret detector. Combine it with explicit staged-file review.
"""
import argparse
import json
import re
import subprocess
from pathlib import PurePosixPath


def git(*args):
    return subprocess.check_output(["rtk", "proxy", "git", *args])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--all-tracked", action="store_true")
    args = parser.parse_args()
    paths = (git("ls-files", "-z") if args.all_tracked else
             git("diff", "--cached", "--name-only", "--diff-filter=ACMR", "-z"))
    findings, count, size = [], 0, 0
    patterns = [rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----\s*\r?\n[A-Za-z0-9+/=]",
                rb"gh[pousr]_[A-Za-z0-9]{30,}", rb"github_pat_[A-Za-z0-9_]{30,}",
                rb"sk-[A-Za-z0-9_-]{30,}"]
    for path in filter(None, paths.decode().split("\0")):
        data = git("show", ":" + path)
        count += 1
        size += len(data)
        parts = PurePosixPath(path).parts
        if parts[0] in {"work", "target", ".codex", ".idea", "outputs"}:
            findings.append({"path": path, "reason": "runtime or local settings tracked"})
        if len(data) > 2_000_000 or b"\0" in data:
            findings.append({"path": path, "reason": "binary or oversized source"})
        if any(re.search(pattern, data) for pattern in patterns):
            findings.append({"path": path, "reason": "credential-shaped content"})
    print(json.dumps({"files": count, "bytes": size, "findings": findings}, indent=2))
    return bool(findings)


if __name__ == "__main__":
    raise SystemExit(main())
