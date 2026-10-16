# M3a application/window lifecycle candidate

Version: 0.4.0-m3a.7. Source is isolated under work/m3a-product-r7/source. Do not substitute root 0.3.0 binaries or mutate frozen predecessor packages. This is not a global install or live acceptance.

## Build and package

Run offline fmt, workspace locked test, host clippy and aarch64-pc-windows-msvc clippy. Build both release targets into work/m3a-product-r7/target using this exact source root. Archive source (without generated binaries), record source snapshot SHA, each command, Cargo identity, package hashes and tool metadata. After any source/package change, repeat applicable gates before freeze.

## Configuration -> call -> verification

1. Owner allocates one configured route and native identity; grants are not inferred from title/PID or an old run. Generate exact lifecycle JSON using the published schema/types. max expiry one hour; separate monotonic expiry enforced at server startup. Local drive-absolute paths only; exact argv (up to 32 / 8192 UTF-16 units), app and allowed-file SHA-256; at most 8 launch permissions, 32 activation permissions, 32 inventory windows and 128 cumulative native events. Files are checked with bounded native reads and participating data/traverse handles under read-only sharing; ancestor identity/type/path checks are retained. This is not a general attribute-write lock or atomic filesystem transaction. Native driver additionally rejects reparse/device/UNC/ADS paths and files over 512 MiB. The grant authorizes the exact command, not a general shell or unbounded filesystem sandbox. App-managed state and dependencies are not implicitly byte-attested.
2. Start the prebuilt guest binary using explicit --state-dir ABS --windows-desktop --windows-lifecycle-grant ABS_JSON. This does not enable SendInput. Keep state directory stable across restarts: once-only reservations are durable files under state/lifecycle, not in-memory request dedup. Use the same state directory across cooperating servers.
3. Read computer.targets; claim an exact desktop lease; launch_application chooses only a configured authorization_id. Driver installs native hooks before complete pre-launch inventory, validates exact image/files and existing-window scope, durably reserves before dispatch, CreateProcessW once, then bounded complete after inventories and cumulative events. Inventory coverage is conservative: unreadable/vanished/unproven native windows can block launch. Do not weaken coverage just to make a live test pass.
4. Interpret observations, not guessed causes: new key -> new_window_observed; same authorized key with change/events -> reused_existing_window; more than one candidate or HWND destruction/reappearance -> ambiguous; accepted process without UI evidence within horizon -> launched_no_window; cancellation/session/coverage/release uncertainty -> uncertain. Return process PID may be a broker, not UI PID. new_window_observed is temporal/image correlation, not proof launch caused it. Goal/input flags stay false.
5. For explicit activation, discover exact HWND/PID/birth/epoch no older than 10 seconds and supply fresh expected foreground. Grant permits one target tuple. Native LastInputInfo/identity/tuple checks surround one SetForegroundWindow call. User intervention yields; OS refusal is not retried. No AttachThreadInput/ALT tricks and no focus side effects in bind. Verify foreground and separately application goal after new observation.
6. Separate input grant still pins root/owned dialogs. Newly launched windows do not automatically qualify; derive and record the exact fresh tuple only within explicit owner-authorized parent-resolution scope before SendInput. Existing authorized windows/tabs can be used without a clean-window requirement.

## Diagnosis and durable receipts

- existing_window_not_authorized: request allocation for the existing exact window if appropriate, not taskkill/close/shared session destruction.
- incomplete_window_inventory / incomplete_native_events: retain partial proof and do not equate it to absence; bound/unknown coverage remains a real limitation.
- idempotency_conflict / uncertain_journal: never replay. Use lifecycle_receipt and owner-visible native inspection. New request_id cannot restart a reserved launch. An interrupted reservation retains uncertain even if no final receipt survived.
- lifecycle_grant_expired / stale_activation_window / foreground_changed: do not extend deadlines or retry focus; return/reobserve within explicit allocation.
- resources_released=false: operation terminal is not cleanup proof. Native calls cannot be forcibly preempted; retain exclusive execution slot until worker exits.

## Native-only 60-minute reservation

All implementation, review, offline tests, compilation and SHA freeze precede booking. No current allocation is established here. Request a continuous 60-minute slot, explicit start/end/target/window/file/action scope and cleanup ownership. Insufficient 30-minute minimum (or insufficient actual scenario budget) means reschedule, never silently shrink full acceptance to five minutes. Short 15-second operation deadlines and 100..60000ms cooperative leases are separate from owner reservation and do not extend it.

- 0-5 min: owner handoff, scope/native identity/deadline and frozen package hash checks.
- 5-40 min: agreed real native launch/reuse/activation cases and direct goal evidence.
- 40-50 min: agreed failure/cancel/intervention cases and evidence completeness.
- 50-60 min: no new scenarios; scoped release, proof and handback. Close only explicitly owned temporary windows; never kill shared applications.

If a fix or replacement candidate is needed, end native work, preserve failure evidence and safely return the allocation. Repair and rerun offline gates before a new booking. No unit tests, builds or source changes inside the live window.

## Remaining live scope

No VM/native launch/capture/input test has run for this candidate. Notepad forced new-window CLI is unsupported/unverified. Windows native API behavior, real app broker/reuse behavior, practical coverage and goal/task completion need a separately authorized live run. Cross-compilation and synthetic fakes do not substitute for it.

## M3a.2 cleanup, operation IDs and read-only preflight

All native closer failures feed a sticky operation/lifecycle cleanup audit. Launch and activation receipts plus their durable final records include resources_released and resource_release_uncertain; a successful hook or process closer cannot erase an earlier temporary-handle close failure. On uncertainty, stop new dispatch and do not classify a successful launch/activation. A worker reaching terminal is not automatically resource-release proof.

Native call nesting is counted. native_call_depth remains nonzero until every scope drops, including exception unwinding. finish_requested while a scope is alive does not report terminal/resources_released; the last Drop resolves completion without discarding uncertainty.

Every MCP response for an operation that was created (success, core fault or worker failure) exposes its operation_id and an accurate status snapshot. Query computer.operation with that returned ID, including when the caller omitted one. An ID was not created when the request was rejected before operation registration.

Before a native launch, call the read-only computer.lifecycle_preflight(target, authorization_id). It returns a bounded partial-or-complete inventory with native_windows_enumerated, complete and notes (native failure-code counts and up to eight HWND/fault examples), event readiness, error and cleanup state. It creates no durable reservation and dispatches no launch, activation or input. ready_for_launch is only point-in-time diagnostic readiness, not an owner grant, lease or future outcome. Unknown windows remain incomplete; never upgrade them to complete just to unblock a test.

The prior 0.4.0-m3a.1 is frozen but not approved for live use following independent reproduction of cleanup, nested-scope and missing-operation-ID defects. This new revision requires the complete offline gates and another parent review before any 60-minute booking.

## M3a.3 checked semantic action and bounded document preconditions

The actual Mac GPT6 max synthetic control reproduced a normal observe-to-act receive interval of 12.327921 seconds against .2's 10-second reference limit. This is a client receive measurement, not server timing or Windows evidence. .3 adds computer.act_checked(target, frame, element, lease, request_id, intent, value?) so one service operation revalidates the exact proposal, makes a fresh semantic resolution, and dispatches at most once using the existing durable reservation and native guards. No coordinates/name lookup, automatic retry, focus change, grant refresh, or lower-model workaround is involved. The original act and pixel-reference TTL stay 10 seconds. Checked proposals have a separate maximum 120-second horizon; leases remain at most 60 seconds and must be valid at dispatch.

Windows MutationContext requires an untruncated exact window title plus the hash of all visible Edit/Document native values reached in a complete bounded UIA traversal. Up to 8 values, total 16,384 UTF-16 units; unknown provider/password/visibility/runtime ID/coverage or invalid Unicode gives unavailable, not an empty successful proof. TextPattern is request-bounded; ValuePattern allocates before the post-read bound. This does not claim hard provider preemption or allocator protection. A genuinely complete walk with no editable controls has an explicit empty-set digest. macOS does not supply this document proof yet, so checked actions there fail closed.

Changing document text with identical HWND, editor runtime ID, name, geometry and focus is rejected before reservation. Title, identity, focus, layout and exact element changes are also rejected. Before the first SendInput batch, native document state is read again; title and the existing native guards are checked per batch. Own text edits change values, so the original document value is not compared on later batches. An app's own dirty-title change may conservatively stop a multi-batch text action. For the planned Notepad main case, verify run baseline first; Ctrl+A, separate Backspace, independently verify empty, fresh observe the actual dirty title, then Text. No guessed removal of title markers.

These are optimistic preconditions, NOT native file/tab identity or OS-atomic compare-and-write. Different documents with identical title/value may be indistinguishable; changes and changes-back between reads and changes during a native dispatch are not fully excluded. A static window grant does not authorize all old tabs. The main run must identify the unique run-owned filename AND independently read exact run baseline before any edit; after any unexpected document/focus change it holds instead of choosing another editor. Real Notepad provider coverage, title behavior and dialog workflow remain unverified. Synthetic fixtures and native-like constant-revision tests are labeled separately from live Windows evidence.

The default production binary exposes 19 tools and zero targets. The synthetic-fixture feature is a separate test build with strict --synthetic-only --state-dir ABS entry: it rejects native selectors and must never be shipped as the Windows candidate. Receipt.goal_verified remains false; an independent UIA/file oracle is mandatory.

### Final-guard review correction

Review found that a blocking document walk after an exact focus/hit guard could invalidate that guard. The final sequence now uses the actual portable GuardPhases path: bounded document read, exact UIA scope, native identity/display/window/geometry/hit/user and grant recheck, then lease/cancellation checkpoint before dispatch. There is no additional document walk after the final exact guard. Fault-injection tests change focus/geometry under the same HWND/title/value during the document phase and require zero dispatch; they remain offline sequencing evidence, not native execution. For multiline tasks, native Text forbids control characters: use separate line Text actions and separately grounded Enter key chords, never send CRLF inside Text.

## M3a.4 native pin correction

See [native pin contract](m3a4-native-pin-contract.md) for exact masks, diagnostics, Limited-token boundaries, root handling and regression scope. That historical .4 candidate did not identify or claim to have cleared the then-unlocalized AccessDenied; the later actual r3 evidence below now localizes it.

## M3a.5 explicit observed package proof

Actual r3 localized the Limited-worker `CreateFileW` failure to the `WindowsApps` ancestor of the observed image, after the System32 launcher and allowed run-file pins passed. No access masks are changed in .5. Original launcher, allowed-file and legacy observed application strong pins remain; new `observed_packages` is a separate explicit role, never an AccessDenied fallback or permission to launch that path.

Each item supplies `{application:{executable,sha256}, package_full_name, install_path}` and requires an already-running allocated HWND/PID/full creation FILETIME/epoch. The ordinary Rust worker independently proves the retained process image/birth/window, matching target/worker user-session-logon, exact process/full/family/current-user package HEAD, non-development Store/Inbox origin, and equal native Install/Effective/granted roots. It then holds the package root and every descendant to the final image using unchanged no-reparse/path/identity/hash checks; revalidates before/after and at reuse/dispatch. Above-root trust is OS-managed registration, not a full ancestor handle chain. No declaration/hash from setup replaces these native reads.

The separate cache cannot attest a launcher or allowed file. Its handles release after dispatch, while the explicit role selection survives through post-launch inventory; new windows need a new full native proof. Any unknown identity/query/registration, different effective root, stale epoch, token impersonation or release failure HOLD. Install==Effective does not establish absence of all alternative location configurations. This is on-disk identity, not loaded modules/process-memory integrity or global atomicity. See `m3a5-observed-package-contract.md` for the exact boundary and primary docs.

r3 quoting/cleanup actually passed and its closed run remains immutable. That result does not validate this new .5 native proof. Limited-worker package API calls and root-down handle availability still require a separately allocated Windows run after exact-manifest external review.


## M3a.7: narrow observed single-HEAD compatibility

The versioned sections above describe their respective historical changes. The
.7 policy recognizes only observed bit 0x10 in an otherwise completely validated
current-user, single-HEAD snapshot: count, exact full/family/process identity, same
SID/session/logon and equal graph/Install/Effective/grant roots must pass first.
This is grounded in the preserved .6/r5 native record, not a universal Microsoft
returned-flags guarantee; the public SDK documents HEAD as an input filter.
Other previously-unknown bits (including other input-only filters), development
and non-Store/Inbox origins still reject. Existing static/dynamic properties are
not new allowances. Raw flags remain unchanged for before/after checks.
No bypass of subsequent native image hash pin, resource release or no-replay.
Public actual-snapshot regressions are offline data only; the historical grant
fixture is revoked, is never executed and cannot authorize a new run.
See docs/m3a7-single-head-compatibility.md in the source archive for exact evidence
and the explicit test-expectation delta. .7 native acceptance remains not_run.
