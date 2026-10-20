# Contributing

Work from the repository root workspace, on a named feature/fix branch. Do not
create another editable product source under `work/`. The initial Git baseline
preserves the old root; the consolidation commit brings the current candidate
into the root. Neither is a release tag or native acceptance claim.

Keep behavior changes and module extraction reviewable. Explain module ownership
and forbidden dependencies, not just directory names. Update an ADR for a changed
boundary. Existing hotspot ceilings are debt, not permission to grow large files.

Before a relevant change is described as passing, run the root fmt/test/clippy
commands in README and the architecture checker. The default test suite must not
activate a real desktop. All-feature link tests and actual Codex-host tests need
an explicit separately built fixture worker; see `integrations/codex/README.md`.

Stage explicit paths, inspect `git diff --cached`, and run:

```sh
rtk proxy python3 tools/check_staged.py
rtk git diff --cached --check
```

The lightweight hygiene check is not an exhaustive credential scanner. Keep keys,
private deployment profiles, raw captures, local Codex/IDE config, build products,
and runtime journals out of Git. Preserve failure evidence locally and publish
only a reviewed, small validation index with hashes and scope labels.

The CI workflow checks macOS and Windows compilation/default tests; it does not
grant GUI access, test real Windows business goals, invoke a model, publish a
package, or install an agent. Dependency downloads in fresh CI are intentional;
local runs should use cached `--offline` dependencies when available.
