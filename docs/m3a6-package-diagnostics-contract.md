# M3a.6 package policy diagnostics — offline-only candidate

## Evidence, not inferred cause

The immutable r4 preflight failed with `observed_package_origin_untrusted` before
package image hashing. That old OR combined development mode, unknown HEAD bits,
and origin outside Store/Inbox. It omitted raw origin and flags. Which predicate
actually fired remains unknown. Root-down image pin was not reached. The model,
launch, activation and input did not start.

## Narrow production change

Only `package_policy.rs` and `application_package.rs` change production Rust.
The three numeric predicates are unchanged: DEVELOPMENT_MODE `0x00010000`,
KNOWN_PROPERTIES `0x003d000f`, allowed origins only 2 and 3. No new query, token,
access/share flag, retry, fallback, lifetime, grant or release exception is added.
Separate errors identify development, unknown properties and disallowed origin;
raw values, all failing predicates and stable primary precedence are retained.
The symbol map covers the frozen windows 0.62.2 binding (0..6); unfamiliar values
remain raw and rejected. A new symbol alone never grants permission.

Native Context records returns at the actual existing API call sites. Query
source labels do not pretend the call ran. Real Win32 failures retain their
codes; policy rejection does not fabricate a Win32 error. The production-used
portable decorator retains the complete cause and the outer context. Snapshot
strings are bounded with original lengths/SHA and truncation flags; trace count
is bounded and omissions explicit. Trace completeness is not OS coverage.

## Verification and use

Run all offline formatting/tests/dual-target clippy/release gates before freezing.
Public local tests cover multiple predicates, unknown signed values, envelope
retention, bounded output and 77-case old/new admission equivalence. They cannot
recover r4 missing values or establish native Windows package availability.

After exact frozen-product/runpack review only, request a fresh continuous
60-minute owner allocation. No testing, compilation or editing inside it. Main
model is gpt-6-astra/max with its full 1200-second budget; final 600 seconds are
cleanup only. Insufficient time means no start. A failure needing a source change
ends this round with retained evidence and scoped cleanup; change it offline.

This revision adds observability; the original package rejection and the host
PrlJob_GetResult transport failure remain unproven as fixed. Old failed runs,
no-replay states and closure gaps stay unchanged. Separate owner recovery does
not turn the original parent cleanup into success.
