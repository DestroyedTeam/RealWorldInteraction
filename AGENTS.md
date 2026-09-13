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

## Approved multi-endpoint architecture

- User clarification 2026-09-13: supplemental bug reports reprioritize the full
  implementation plan; they do not replace it or make one fix/commit a stopping
  condition. Continue through the remaining plan. Notify only at meaningful
  milestones or when user intervention is necessary. Acceptance deliverables
  use concise Chinese explanations and chronological actual screenshots, with
  each step's result/problem explicit. Track current work in
  `docs/product/implementation-status.md`; historical frozen runbooks are not
  current slot limits or permission rules.
- User clarification 2026-09-13: the authorized Windows desktop includes OS
  dialogs, Save/Open, Explorer and other applications, not a Paint-only app list.
  Application allowlisting is optional future policy, not a current task gate.
  Fresh window selection/owner/foreground grounding still applies; an ordinary
  owned popup is not a new permission request. Secure-desktop/session changes
  remain distinct native identities, never inferred from ordinary dialog access.
- User clarification 2026-09-12: GUI and terminal are parallel complementary
  channels within one flow, including cross-execution, verification, feedback
  and iteration. Neither is a replacement/fallback for the other. Preserve
  each channel's device/user/session identity, authorization and evidence.
- User clarification 2026-09-12: production RCU MCP registration is user/Host-wide,
  not project-specific. Keep the installed adapter, paired client identity and
  profile outside source checkouts/worktrees. Isolated test configurations are
  not production installation. Change only RCU settings when authorized; retain
  unrelated Host configuration and verify a real fresh task without local config.
- ADR 0002 (`docs/adr/0002-headless-control-infrastructure-review.md`) is the user-approved architecture baseline, not a trial topology. P0–P6 are delivery stages only.
- Production deployment, service startup, tool discovery and transport must not depend on a business task, application window, expected document text, business Stage or oracle.
- Business scenarios and oracles belong to test/caller layers. Keep native identity, authorization, resource leases, operation deduplication and uncertainty handling, scoped to the affected resource. Reconnection must not replay input.
- The MCP adapter is host/platform-neutral. The device agent owns native execution and authorization. The production transport dependency graph must not include platform FFI or business harness code. Report unsupported platforms/capabilities honestly.
