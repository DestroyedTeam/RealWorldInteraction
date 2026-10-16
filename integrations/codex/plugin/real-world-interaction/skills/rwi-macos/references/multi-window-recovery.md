# macOS multi-window and error recovery

## Short ordinary path

Select the returned macOS device/context and discover the exact authorized native window. When attachment is advertised, call `computer.attach_window` with the original desktop target and that fresh selection, then call `computer.snapshot` with the **new** target. Otherwise bind the original target and observe after every window switch. For a named control omitted from the compact list, `computer.controls` searches the retained `observation_ref` and frame; it does not capture again. Use `computer.click` with that exact frame and element, optionally its advertised AX action. These tools manage the short lease. Verify the result with a new snapshot; a returned dispatch is not application success.

This avoids copying `withInput`/pagination loops for ordinary clicks. Use JS only when the flow needs real branching or bounded related logic. Do not mix convenience-tool element IDs with JS summary aliases such as `e5`.

## Window selection is not relationship detection

The current macOS agent accepts fresh explicit `initial` selections. It does not implement native `owned_popup` or `return_to_owner` transitions. After a new window appears, discover through the original desktop target and explicitly attach its exact PID/window/epoch on the authorized desktop. This works for independently selected sibling windows as well as panels; it does not prove an owner relationship. Returning to a retained main-window attachment needs a fresh snapshot, not rebinding. On older agents without attachment support, select the main window freshly when returning. Do not infer relationships from title or same PID, silently fall back to another window, or activate/move a window as part of selection.

`rwi.window(target)` wraps one exact target. `state.main` and `state.dialog` with the same target are not independent windows. Use `await desktop.attach(selection)` when supported; each returned wrapper has its own window, frames, semantic-object registry and Live session. Root rebinding does not replace those attachments. A wrapper's `target` is a routing locator, not proof that an old frame remains current.

Attachments share the original desktop identity, authorization, input resource domain and cooperative lease. A lease for window A cannot authorize B; release A before claiming B. At most 16 targets (root plus 15 attachments) exist per context. Detach idle bindings when finished: release their lease first, then use `computer.detach_window` or `await window.detach()`. It drops that binding's Live and cached frames, not the native application; original durable request receipts remain queryable. Context close/restart/reconnection invalidates all attachments. Never replay an attachment/action to recover a lost reply: refresh `computer.targets` for owned handles or query the exact operation.

New tools may be globally listed while the connected Agent is older or the selected platform is unsupported. Check the selected device's implementation flags; do not route to another device or treat an attachment refusal as permission to activate or retry input.

## Distinguish failed capture from failed input

A cell can contain a returned `computer.act_checked` followed by a failed `computer.observe`. The outer `isError` describes the cell, not every earlier native operation. `execution_summary` separates returned dispatches, rejected-before-dispatch attempts and unconfirmed attempts. Exact operation/request identities remain in `calls`. Missing evidence stays unknown; do not infer rollback, no input, held-resource recovery or application success.

On error, local JS objects are discarded. Rebuild only the read-only device/window state; inspect original receipts before another intended mutation. Do not rerun the original cell. Read-only settling is different from retrying input.

## Coverage and native refusals

Paging retained metadata cannot recover nodes the native AX walk never captured. An incomplete tree does not prove a control absent. `visibility_unknown` means this exact control lacks positive visibility evidence; `semantic_walk_incomplete` on older agents means the resolver cannot prove a unique object within its full-tree budget. Agents with retained semantic objects revalidate the original object and native ancestry directly, but incomplete initial coverage still cannot find uncaptured controls. Do not respond by silently switching to guessed pixel clicks or raising depth/node limits in a calling script.

Use a read-only exact-window capture to understand the UI and report the native evidence gap. App/window identity, bitmap mapping and semantic action support are separate facts. Current macOS lifecycle activation is not implemented merely because a global tool is listed.
