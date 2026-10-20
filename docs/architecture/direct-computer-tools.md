# Direct Computer Use tools (2026-09-20)

## Decision and references

Frequent actions should be typed tool calls, not generated scripts. The Rust
Host-neutral gateway now composes existing native operations; the device agent
still owns identity, permissions, frame validation, leases and no-replay.

References inspected:
- Microsoft Playwright MCP: https://github.com/microsoft/playwright-mcp — snapshot refs and dedicated interaction tools, separate code execution.
- Anthropic Computer Use: https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool — named screenshot/click/drag actions and explicit coordinate contracts.
- Current Codex CUA calls in this task: app-bound getAXState/getScreenshot, index-or-coordinate click, drag, setValue, pressKey, per-call titles, changed-tree output. These are observed usage patterns, not claims about its internal implementation.

RWI must not copy another tool's implicit foreground or screenshot coordinate
assumptions. Returned view_id is mandatory for preview pixels. No inference of
window ownership from a human-readable app name, and no input retry.

## New entry points

After targets/discover/bind (existing explicit selection):

```json
{"name":"computer.snapshot","arguments":{"target":"<exact target>"}}
```

One observation, one mapped preview, compact controls with original native IDs.
The default list has at most 64 controls and a 12 KiB metadata-row budget; values
may be display-truncated, never truncated in native authority. observation_ref
retains the complete original observation for computer.result paging. This list
is not a claim of a complete native accessibility tree. Existing observe remains
available for callers that need its full contract.

```json
{"name":"computer.click","arguments":{"target":"<target>","request_id":"unique-press-1","at":{"kind":"element","frame":"<frame>","element":"<id>","action":"AXPress"}}}
{"name":"computer.click","arguments":{"target":"<target>","request_id":"unique-click-1","at":{"kind":"view","view_id":"<view>","x":300,"y":200}}}
{"name":"computer.drag","arguments":{"target":"<target>","request_id":"unique-drag-1","view_id":"<view>","from":[300,200],"to":[450,240],"duration_ms":500}}
```

Element action is optional and must be advertised by the native element. Without
it the native checked click path applies. Drag converts both endpoints through
the same delivered view to native points; the native executor checks the complete
trajectory. Each input owns a short lease, dispatches once, attempts release,
and returns a concise receipt plus retained details. Cleanup failures preserve
both the original action evidence and cleanup error. No automatic screenshot
or model-goal success: inspect computer.snapshot afterward. This avoids surprise
extra captures when a caller wants to inspect AX, a popup, or terminal state.

No implicit app activation, permission grant, modal dismissal, recovery or
retargeting. Stop/cancellation is not proof of native termination. A broken
transport may require native lease expiry and operation inspection.

## Scope and remaining work

These tools are adapter-local composition, not new platform FFI. Initial direct
MCP entry points deliberately do not depend on a JS interpreter. Existing
computer.js and low-level tools remain for advanced workflows. The current JS
worker allowlist has not yet been expanded for these composite calls; invoke
them directly rather than nesting them in computer.js.

Remaining: dedicated text/key/scroll tools, semantic search/paging, snapshot
diffs, preset SDK functions with typed errors, per-stage performance counters,
and controlled live acceptance after the Host reloads its MCP tool catalog.
Do not report mocked transport/unit tests as actual macOS GUI validation.

## Performance interpretation

A direct tool removes generated-code tokens and Host round trips; it does not
remove native AX/capture costs. snapshot still performs observe+image; click
still claims/validates/dispatches/releases; drag currently resolves two endpoints.
Compare model turnaround, RPC count, native stages, output bytes and verified
business results separately. No general speedup percentage is established yet.

## Progress notifications (2026-09-20)

The three direct tools emit standard notifications/progress when the Host puts
_meta.progressToken on tools/call. Messages identify gateway stages (received,
read state, map coordinates, acquire lease, dispatch, release returned, result).
Counters increase within one request; no fabricated percentage or native success
is inferred. Without a token, no notifications are emitted. A notification send
has a 25ms advisory budget and never changes input outcome or triggers replay;
release RPC executes before its notification. Cancelled requests skip status
notifications while native cancellation/lease cleanup retains its own contract.

This works over existing stdio and is transport-independent. Streamable HTTP
listening, native event subscriptions, resume cursors and live image streaming
are NOT implemented by this change. The actual Codex UI consumption of these
notifications still needs observation after its current MCP process reconnects.
The real MCP subprocess regression verifies opt-in behavior, monotonic ordering,
error termination and no false success (without a native device).

## Control lookup and keyboard entry points (2026-09-20)

```json
{"name":"computer.controls","arguments":{"observation_ref":"<snapshot observation_ref>","frame":"<same frame>","query":"Undo","limit":16}}
{"name":"computer.type_text","arguments":{"target":"<target>","frame":"<frame>","element":"<focused editable id>","request_id":"text-1","text":"你好"}}
{"name":"computer.press_key","arguments":{"target":"<target>","frame":"<frame>","element":"<focused id>","request_id":"key-1","key":"escape"}}
```

controls reads retained original metadata only: no capture/device RPC, no frame
renewal and no input. Name/description/role matching is a case-insensitive
substring. All matches remain explicit (including duplicates/disabled controls),
with next_offset into the filtered list. Omitted controls can thus be discovered
without generating JSON-pointer paging code. The original native IDs remain
bound to the original frame. Missing, expired, wrong-source or mismatched-frame
cache references fail; they never trigger an implicit new screenshot. Oversized
rows fail explicitly rather than returning a non-advancing cursor.

Text and key tools use computer.act_checked with an internally managed lease;
they do not implement a second input path. Text is bounded to 4096 UTF-8 bytes,
keys to 128 bytes. Text is native typing, not AXSetValue, clipboard paste or a
promise to clear/replace the entire field. Selection behavior belongs to the
application. The native backend must attest focus and acceptable text class;
unknown/secure targets do not fall back to global keyboard input. Chord support
is platform-specific. Unsupported keys fail instead of being substituted.

The standard progress notifier wraps these entry points too. Status messages
never contain the typed text. Control-query replies are not projected into an
input receipt: matches, original frame and pagination remain visible.

These are direct MCP entry points. The legacy JS worker allowlist still does
not route the adapter-local composite tools; use raw native calls or direct MCP
until that worker's composite-cancellation contract is separately implemented.

## Precise pre-dispatch rejection receipts (2026-09-20)

`dispatch=rejected_before_dispatch` means the backend failed and its known,
unsaturated native side-effect counter was unchanged across the invocation.
Error names alone never establish this. Missing, changed, reset or saturated
counters retain `uncertain`; native success remains `returned`, not goal success.
The durable pre-call reservation remains uncertain until the final receipt is
persisted. Both rejected and uncertain request IDs stay forbidden to replay,
including after restart. Observed backend wrappers must forward native counters.
This applies to input and window placement; it does not modify lease, permission,
focus, resource-release or recovery requirements.

### Invocation-local text editor resizing

macOS public element IDs include geometry. Text input can resize the very same
AX editor between UTF-16 batches (Paint S also shrinks its initial empty line
height slightly). After the first fully grounded pair, continuation may accept
size-only changes at the original x/y, but only after CFEqual window/editor,
process/session, scope, focus, layout and text classification continuity checks.
The original point must still lie in the current valid editor/window bounds,
and writability/permissions/modal checks still apply. The private comparison
normalizes the geometry-derived ID only for that invocation; no external frame,
reference, TTL or retry permission is renewed. Pointer/chord paths retain strict
geometry checks. Failures now include bounded, value-free changed-fact diagnostics.

## Per-call stage timings

Direct tools now return `stage_timings`: ordered, aggregated rows of tool name,
call count and elapsed milliseconds around each adapter invoke. Two drag
resolve_view calls aggregate into one row; failed calls and lease cleanup are
included. Sub-millisecond durations aggregate before rounding. No arguments,
text, target secrets or global request history are stored. Invalid arguments
that invoke no stage report []; poisoned diagnostic storage reports null.
These are adapter wall times (routing/transport/native work), not native CPU or
separate AX/capture costs. Progress notification delivery and presentation time
are outside the rows but inside total elapsed_ms. This is instrumentation, not
an acceleration claim; output schema and device protocol remain unchanged.

## Double-click without generated scripts

`computer.click` accepts optional `click_count: 1 | 2` (default 1). Count 2 maps
to one existing native `double_click` intent under one lease/request ID, never a
loop of two click requests or a retry. Explicit AX actions require count 1;
ambiguous combinations and counts outside 1/2 are rejected before lease/input.
The native double-click continuation checks still apply. New optional schema
requires Host tool-catalog refresh before live convenience-tool acceptance.
