# Native Windows deployment validation — 2026-09-11

Status: **in progress, NOT native Codex input/save acceptance**. See the companion
JSON for scoped results and content-addressed evidence references.

## Changes actually exercised

- Explicit `asInvoker` manifests remove unintended Windows installer-detection
  elevation of the ordinary-user setup binary. No global compatibility setting.
- File-based private bundle validation/import works on the real ARM64 guest,
  including fixed installation paths, exact hashes and protected user-only ACLs.
- Ordinary/Admin modes are explicit and verified using actual token facts in
  setup, agent and retained worker/pipe-peer validation. No automatic elevation.
- Package bootstrap plans require real pre-existing native window identities.
  Invalid core launch grants now fail during preparation, before worker startup.
- A failed preflight with `inventory: null` now reports the actual bounded native
  fault code rather than a misleading `invalid type: null, expected Inventory`.
- Actual Codex plugin installation and Skill discovery were verified in isolated
  HOME/CODEX_HOME directories. No global Codex settings or other project bridges
  were modified. The held-task negative case rejects MCP startup.

## Native findings, not fixture assumptions

1. First failed deployment: invalid observed-package grant with no allocated
   existing window. Complete installation archived; HOLD retained; no replay.
2. Second deployment: the user closed the allocated empty Notepad. Native
   re-enumeration found no process/window, consistent with `window_gone`. The user
   confirmed the close. Worker exit was observed, code 0, without forced kill.
3. Third deployment: live package-root `CreateFileW(FILE_TRAVERSE)` returned
   `ERROR_ACCESS_DENIED` under the ordinary limited token. This was NOT a VM
   foreground failure, hung cancellation, or a missing screenshot.
4. A separate Rust native diagnostic compared access masks against the exact
   registered package directory, and independently tested rename exclusion on
   disposable directories created by that diagnostic:

| Token | Desired access | Package directory open | Owned directory rename while handle held |
|---|---:|---|---|
| Ordinary, limited, integrity 8192 | `FILE_TRAVERSE` (32) | Access denied | Rejected, sharing violation 32 |
| Ordinary | `FILE_READ_ATTRIBUTES` (128) | Succeeded, metadata/path verified | Succeeded — NOT a namespace pin |
| Ordinary | 0 | Succeeded, metadata/path verified | Succeeded — NOT a namespace pin |
| Admin, full, integrity 12288 | `FILE_TRAVERSE` (32) | Succeeded, metadata/path verified | Rejected, sharing violation 32 |

Consequently, **no fallback to access 0/128 was implemented**. It would lose the
actual rename-exclusion property despite successful metadata queries. The target
package directory, ACLs and privileges were not modified by the diagnostic.
The original strong pin policy remains unchanged.

Microsoft documents zero-access metadata opens and the independent sharing rules:
[CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
The actual rename results above are native test evidence, not inferred from docs.

## Recovery and current boundary

Each failed installation is retained separately with its original journal head,
HOLD, response blobs and no-replay reservations. The operator verified retained
agent handles by exact birth, image path and SHA before stopping only the owned
agent, then moved the complete installation. This is operator recovery, NOT a
product graceful-stop or completed-task receipt. No user application was killed
by these archive scripts.

The administrator directory diagnostic passed. Administrator target preparation
was initially canceled at UAC, then retried only after the user explicitly asked.
Administrator import/start subsequently succeeded. Bootstrap and Preflight passed;
CreateProcessW accepted and Notepad opened allocated.txt. Launch was classified
as ambiguous because newly created hidden IME windows were included as candidates.
The new candidate-facts filter retains the complete inventory but requires known
visible, uncloaked, unowned roots with complete owner-chain evidence. Missing or
unknown facts are ineligible; multiple real roots remain ambiguous. Regression
cases pass, but the fix is not live-accepted in this snapshot. The old agent and
workers exited; moving the HOLD installation is currently blocked by an open file
or directory handle, pending closure of the test Notepad/Explorer windows.
No replay or privilege/ACL bypass was used to replace that confirmation.

## Acceptance gaps

- Administrator deployment, Bootstrap and Preflight passed. Unique Launch
  correlation, Activate and actual Codex ModelReady still require completion.
- Native model mouse/keyboard dispatch, independent UI verification, exact file
  SHA readback and clean ModelEnd/Oracle/Cleanup are not yet accepted.
- Store Notepad cold-start support is not proved: this workflow deliberately
  supplies a real existing window as operator setup.
- Ordinary-mode installation works; this protected Store-package lifecycle is
  blocked at strong directory pinning. Do not label it full ordinary-mode parity.
- The operator-driven real Codex Host harness is not independent model inference,
  a tool installed into the current task, or a registered ChatGPT App.
- Startup rejection still appears as a generic initialize/connection-close error
  in the Codex Host. Better native-link startup diagnostics remain follow-up work.

Offline regression: 227 passed / 0 failed / 1 ignored default; 269 / 0 / 2 with all
features and the explicitly built fixture worker. Host and Windows ARM64 clippy
passed with `-D warnings`; Windows uses the explicit cached xwin SDK configuration.
The first plain cross-target clippy attempt lacked SDK C headers; that failed log
is retained rather than counted as a successful Windows check. Architecture
ratchet and its 4 tests passed. No indexing, commit or push was performed here.

## Real-user Codex installation priority

The existing project `rcu_m1_local` points at an ARM64 Mach-O MCP binary with only
`--state-dir`, no Windows transport. It is retained unchanged. The actual desktop
CLI is `/Applications/ChatGPT.app/Contents/Resources/codex` (0.153.4); the older
Homebrew CLI (0.133.0) cannot parse the user's `max` reasoning setting. No global
setting was downgraded. A separate personal native plugin is the next installation
step; previous isolated HOME tests are not user installation acceptance.


## Latest live result and user installation gate

After the user closed the test windows, admin4's full HOLD installation was
archived and a fresh admin5 task/pairing was prepared. The candidate-facts fix
passed **Prepare → Bootstrap → Preflight → Launch → Activate** on the real
Windows guest. `ready_for_model_attachment: true` is NOT a ModelReady receipt,
current-task tool discovery, input acknowledgement, or saved-file acceptance.

A new project MCP entry `real_computer_use_native_link` and project Skill were
installed additively; `rcu_m1_local` and the user global config were preserved.
The real desktop Codex 0.153.4 `config/read` explicitly reports this project's
configuration layer **disabled because the project is not trusted**. Its
`skills/list` does discover the new project Skill. Do not use CLI `mcp get` alone
as a project-layer diagnosis. No project trust was changed without confirmation;
trust would also enable this project's hooks and exec policies. No competing
Model connection was opened just to probe config/Skill discovery.

The user also reported the resident agent's black console. The new agent uses
the Windows GUI subsystem (PE subsystem 2); the existing live agent is subsystem
3. A new ARM64 GUI-subsystem binary passed a real Windows `--binding` diagnostic
with stdout redirected, without capture or input. It has NOT replaced the active
image-pinned admin5 agent. Setup/control remain console diagnostics; the existing
MCP worker already uses a GUI subsystem and `CREATE_NO_WINDOW` when spawned.

Current production-selection regression: default 230/0/1 and all-features
272/0/2; final background-entry/config-probe gates are retained separately in the
work evidence logs. Full native Codex input/save acceptance remains outstanding.


## User-requested forced stop and background replacement

The user explicitly authorized force-stopping the resident console agent. PID
2276 was stopped only after retained handle/path/birth/SHA checks. No installation
worker remained. The old executable and agent config were backed up; only the
agent image and its exact configuration hash were replaced. New GUI-subsystem
agent PID 1260 starts without a console; real mTLS status readback succeeds.

The journal was NOT reset: native startup correctly marked the interrupted admin5
task `agent_restart_unfinished_task` HOLD. No earlier launch/input was replayed.
The new project MCP entry is temporarily disabled to prevent connecting a model
to this interrupted task. It must be rebound to a fresh approved task before
activation. Project-trust consent is still needed; no global trust was modified.


## Explicit project-trust approval completed

The user explicitly approved trusting this exact RCU project. Only that path's
`trust_level = "trusted"` was added to the user config, with a private backup and
semantic before/after verification of every other setting. Actual desktop Codex
`config/read` now includes the native entry in effective config; the project layer
has no disabledReason. The project Skill remains discoverable. This is not MCP
execution acceptance: the interrupted admin5 entry remains intentionally disabled
and must be rebound to a fresh task before native input/save validation.


## User correction: keep MCP enabled

The user explicitly corrected the extra disablement: only the Windows agent's
console startup should change; `real_computer_use_native_link` must remain enabled.
Runtime HOLD is independent from configuration enablement. CONTROL CENTER owns
restoring only that boolean; this task must not edit the project config in parallel.
Earlier disabled-entry statements describe historical actions, not current policy.
No journal reset or operation replay is authorized by restoring enablement.


CONTROL CENTER completed the single-boolean restoration. This task independently
read back `enabled = true`, project-config SHA256 `3ce469175ae6b5c6f62243c0a15902b81512cad6452fd85c6767617ecd68dc55`.
Project trust and runtime HOLD remain separately recorded. No config edits or
Model attachment were performed during this readback.
