# Purpose-scoped Windows revalidation

This is a private backend optimization, not a new MCP tool or an exception for
Paint. Discovery, authorization and device services remain task-independent.

| Read purpose | Current native work |
|---|---|
| Public observation / ROI observation | Existing full bounded UIA walk and native capture |
| Semantic reference revalidation | Existing full UIA walk; exact element revision/point/visibility retained |
| Pixel reference with native-element desktop policy | Fresh UIA root identity, full native image, pre/post window/session/layout checks |
| Legacy whole-document policy | Full walk and document preconditions, unchanged |

Pixel grounding compares the full PNG hash, native root/window epoch, exact
window/PID, foreground, layout and title context; it does not use descendant
element rows. That path may omit the unrelated descendant walk, but it may not
omit capture or reuse cached pixels/root identity. Both existing revalidation
boundaries remain. The final input sink still re-proves UIA hit/ancestry,
password/enabled state, native identity/focus/geometry, grants, lease and key
state. No permissions, deduplication, cancellation or late-drag limits changed.

A root-only read reports incomplete descendant coverage and no document proof.
It is internal: it does not mint a public semantic frame or make unknown
elements actionable. Failed root reads remain unavailable, not empty success.
`revalidation_timings` separates full-tree/root-only counts, failures and elapsed
time. These measurements include backend scene creation, not transport/model time.

The JavaScript helper separately rejects known blocked/fenced or unknown
mutation readiness before a lease, and checks each subsequent observed step.
This local check can only reject; it is not authority to bypass the server.
`RcuInputNotReadyError.data.source=client_precondition` distinguishes it from a
native receipt. Prior action receipts and normal release behavior remain intact.

Still pending: a public image-only visual channel (including feedback), targeted
semantic revalidation, authoritative focused-element selection, and bounded
multi-action orchestration. Do not present this optimization as those features.
