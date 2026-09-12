# Desktop discovery is not an input grant

The production device service authenticates and authorizes a principal to observe
one native user/session/seat/desktop. Selecting a particular application narrows
input to that exact root and its permitted owned windows; it must not silently
narrow that principal's desktop **discovery** to the last selected application.

The Windows implementation previously reused the narrowed input grant while
filtering discovery. A real Paint test exposed the consequence: a context last
bound to the closed pointer fixture returned an empty Paint catalog, while a fresh
authorized context found the actual Paint window. Transport remained healthy.

For a production `DesktopPolicy`, read-only discovery now keeps its existing
desktop authorization and still checks current native identity. Explicit `initial`
selection can bind another freshly discovered exact epoch under that policy,
including after the previous selected-input grant expires. The old grant is not
renewed to act on its old window: the new binding narrows its own input authority,
and the controller invalidates old frames/references. Owned-popup selection and
ordinary input still retain their narrower checks.

Legacy allocated-window connections do not acquire desktop policy and keep their
original root/owner-chain filtering, expiry and initial-selection restrictions.
This is not a fallback from failed input authorization and does not grant observe
or input authority to an unauthenticated principal. It contains no Paint-specific
application, process, title, timing or business-condition rule.

On non-Windows hosts, workspace tests do not execute Windows native FFI. A Windows
cross-build and live service test must accompany this change; they are separate
from offline regression and from final icon drawing acceptance.
