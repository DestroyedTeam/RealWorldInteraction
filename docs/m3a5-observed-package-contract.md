# M3a.5 role-specific observed packaged-image proof (offline design)

## Established cause and preserved successes

The actual r3 Limited worker could pin the System32 launcher and run baseline,
then failed CreateFileW for `C:\Program Files\WindowsApps`, ancestor 2 of 5 of the
observed Notepad image, with access 32/share 1/flags 35651584, Win32 5. The new .4
diagnostic worked; changing to traverse did not solve that access denial. No
launch reservation, activation, input or main model began.

r3 quoting and independent cleanup did pass that real transport: both sides
reported complete zero process/task residue, retained 18 files, and generated a
real closed record. The owner then revoked the slot. Old .4/r3 and both real runs
are immutable; no authority or live query is part of this revision.

## Explicit roles, never denial-triggered fallback

1. `application` and `allowed_files`: unchanged strong root-to-file native pin
   policy. These paths may be used in a future CreateProcess/argv action.
2. `observed_applications`: unchanged legacy strong file proof.
3. New `observed_packages`: explicitly granted, already-running, OS-managed
   packaged images only. Each entry supplies exact application path/SHA,
   package full name and install root. This is a different trust contract, not
   an AccessDenied retry. Missing/incorrect package evidence never downgrades
   another role or grants input/launch authority.

The new cache/resources are separate from strong pins. An observed proof cannot
be reused as selected-launcher or allowed-file attestation, even for the same
path/hash. Grant validation rejects ambiguous conflicting role declarations.

## Proposed worker-native proof chain

- Start from an explicitly allocated existing HWND/PID/full creation FILETIME/
  native epoch. Read the window and process again, not a setup process's token.
- Hold an ordinary `PROCESS_QUERY_LIMITED_INFORMATION` process handle. Verify
  creation time, live state and exact QueryFullProcessImageNameW path. Obtain
  package full name/family from that same process handle.
- Compare the target token user SID, session and AuthenticationId with the worker
  effective token before and after. No impersonation or cross-user registration
  fallback is created. A query failure or identity mismatch is HOLD.
- Open package HEAD metadata for that same current user. Match full/family identity;
  reject development mode and all origins except Store/Inbox. Require the native
  Install and Effective roots to equal the granted root. Incomplete or unsupported
  queries fail. Do not interpret arbitrary query errors as an absent external root.
- Only after the package policy is proven, pin the registered package root and
  its descendants through the final image with the existing traverse/data mask,
  read-only sharing and no-reparse policy. The inaccessible ancestors *above*
  that root are trusted through OS package registration/provenance instead of
  requiring a directory handle. This narrow substitution never applies globally.
- Check the retained image handle's file type, identity, final normalized path
  and bounded native SHA against the grant. Keep root/descendant type/path/identity
  checks and final-file pin. Reobserve process identity, package registration and
  HWND epoch after hashing; revalidate before reuse and dispatch. New inventory
  HWNDs require their own window identity and same full native package proof.
- Keep bounded success/failure evidence and audited releases. Cache keys include
  proof kind, exact package/application and process birth, never PID alone.

`GetPackageFullName` explicitly permits the limited query right. The package-info
API opens current-user registration. Effective location may select mutable or
external roots, so package name or install-path spelling alone is insufficient.
See [GetPackageFullName](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getpackagefullname),
[OpenPackageInfoByFullName](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-openpackageinfobyfullname),
[PackagePathType](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/ne-appmodel-packagepathtype),
[package properties](https://learn.microsoft.com/en-us/windows/win32/appxpkg/package-constants).

Install == Effective is the positive predicate “no different effective root was
selected”, not proof that every mutable/external location is unconfigured. No
absent-error whitelist or false optional-location flags are emitted.

## Threat model and limits

Reject absent/wrong package, development registration or a different effective root,
untrusted origin, a path outside the root, reparse/identity changes, stale HWND or
PID reuse, early process exit and incomplete native metadata. Setup metadata only
proposes a grant; its elevated hash is not worker proof. The worker must do all
native reads itself. No ACL edits, elevation, zero-access fallback, deleting the
observed image, or Python/PowerShell GUI/backend bridge is permitted.

The trust root is a non-hostile Windows package manager/OS/filesystem and its
Store/Inbox registration semantics, plus an exact authorized disk-image hash.
This does not attest arbitrary loaded DLLs, process memory, or kernel integrity.
Queries and checks are not an OS-wide atomic transaction. An in-memory modified
process is outside disk-image attestation; short observation-to-dispatch intervals
and package updates may still HOLD. Ordinary-worker API availability, exact
package-root handle access and all live behavior are **unverified until a future
owner-allocated Windows run**. Offline fixtures/cross-compilation cannot prove them.

All implementation, public negative/positive regression, fmt, dual-target clippy/
release, packaging and SHA freeze precede any reservation. The external parent
reviews exact new manifests. Future slot: continuous 60 minutes, main model
gpt-6-astra/max full 1200 seconds, last 10 minutes cleanup-only. Need a source fix
in the slot: HOLD, preserve, cleanup/handback, repair offline and rebook.
