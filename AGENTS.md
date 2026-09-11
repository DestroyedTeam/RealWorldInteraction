# Real Computer Use development

- Product backend and MCP runtime are Rust. Native OS FFI is allowed; Python/.NET/PowerShell/AppleScript are not backend implementations or hidden subprocess bridges.
- The old CrossMacro fork remains an external, frozen reference. Do not overwrite installed bridges, NemessixPaste, UI OS, Codex settings or VM launchers.
- Default tests are offline. Do not activate windows, capture a user's screen, send input, grant OS permissions, wake a VM or install globally without an explicitly allocated target/window.
- Preserve host/guest/session/seat/desktop binding, cooperative leases and durable no-replay reservations. An input acknowledgement is never a verified goal.
- User authorization updated 2026-09-11: local commits, branches, and push to the DestroyedTeam GitHub organization are permitted, including this AGENTS.md. Do not force-push, publish a release/tag, or change unrelated repositories. CodeGraph indexing remains unauthorized in this iteration.
- The repository root workspace is the intended single editable product source. Do not create new development source copies under work/. Historical frozen snapshots remain evidence, not development branches.
- Separate behavior changes from architectural moves in reviewable commits. Stage explicit paths; exclude credentials, local Codex/IDE settings, binaries, raw screenshots and transient runtime state. Default a newly created remote repository to private unless the user specifies otherwise.
- Use `rtk` for shell commands. Run `cargo fmt --all -- --check`, `cargo test --workspace --locked`, and `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` before claiming the relevant regression passed. Use `--offline` when cached dependencies suffice.
- Benchmark scopes are explicit: fixture lifecycle, actual MCP process, offscreen native image conversion, and live native operations are distinct. Never infer performance superiority from implementation language.
- Intermediate files belong in `work/`; deliverable documents/evidence belong in `docs/`, `reference/`, or `outputs/`. Keep generated binaries out of Git.
