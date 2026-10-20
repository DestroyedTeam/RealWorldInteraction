# M3a.4 — native pin access and diagnosable failure

## Why this isolated revision exists

The preserved Windows run `M3-native-20260911-r2-01` failed read-only preflight with
`application_directory_pin_failed / 0x80070005`, before inventory or reservation.
The old fault omitted the failed path, access mask and role. It does **not** prove
that WindowsApps, System32, Temp, or any specific ACL caused that failure.
The real main model never started. Guest cleanup reported completion, but the
independent audit failed at PowerShell quote handling; no passing closure is added.

Product `.3`, runpack `r2` and all failed run evidence remain immutable. New product
`.4` and runpack `r3` are isolated offline candidates, not a live recovery run.

## Native changes and retained constraints

- One production-used `PinPlan` spells out every root-to-parent directory and
  final file open. Directory access changes from metadata-only `0x80` to
  `FILE_TRAVERSE=0x20`; final file access changes from `GENERIC_READ` to
  `FILE_READ_DATA=0x1` for hashing. These are the literal **CreateFileW arguments**,
  not a claim about the kernel's expanded synchronous access mask.
- Every open still uses `FILE_SHARE_READ` only, `OPEN_EXISTING`, and
  `FILE_FLAG_OPEN_REPARSE_POINT`. Directories retain `BACKUP_SEMANTICS`, files
  retain `SEQUENTIAL_SCAN`. There is no weaker retry, listing right, elevation,
  enabled privilege, ACL write, zero-access fallback or swallowed AccessDenied.
- Keep every ancestor handle, volume/index/creation identity and path; check
  non-reparse directory type at initial acquisition, before/after file hashing,
  cached-pin reuse, and before native dispatch. Ignore directory size/mtime and
  unrelated archive-bit churn, which do not identify the directory object.
  Final-file full stamp, exact normalized path, disk type, bounded 512 MiB SHA-256
  and authorized digest remain mandatory.
- Drive-root paths such as `C:\` are valid **only for directory handle checks**.
  File authorization still rejects bare roots, UNC/device paths, ADS, reserved
  names, ambiguous components and paths beyond the original limits.
- `GetFileInformationByHandle` remains the metadata query. Query/path failures
  remain failures; this revision does not silently switch to information classes
  needing broader attribute access or substitute metadata from a setup process.
- Launch permission, observed image permission and allowed file permission stay
  independent. A System32 Win32 entry is not the packaged UI image. Existing
  HWND/PID/full process FILETIME/native epoch checks and durable no-replay remain.

## What the pin does not prove

Traverse/execute and file-data access participate in the documented sharing
checks; metadata-only access does not. Sharing still does **not** lock arbitrary
attribute-only changes. The reparse FSCTL specification permits attribute-write
access, but rejects a nonempty directory. Retained child deny-delete pins plus
type/path/identity rechecks restrict normal namespace substitution; they are not
an OS-global atomicity guarantee or proof against hostile kernels/filesystems.
There remains a bounded observation-to-dispatch interval.

The minimal-mask sequence and actual protected-parent ACL under the **Limited
native worker token** have not been exercised on Windows in this revision. The
setup metadata process's ability to hash an Appx image is not proof of equivalent
worker access. A future access denial must HOLD with exact evidence, not loosen
the pin. Nothing here asserts that the old real failure has already been fixed.

Primary sources: [MS-FSA share checks](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/8c0e3f4f-0729-49f4-a14d-7f7add593819),
[Win32 access constants](https://learn.microsoft.com/en-us/windows/win32/fileio/file-access-rights-constants),
[CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew),
[GetFileInformationByHandle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle),
[reparse FSCTL](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4aeefef8-92c3-4abc-af7a-a610caf8a165).

## Error evidence

`Fault.diagnostic` is optional and backwards-readable. For native pin failures it
contains `schema=rcu.windows.pin.v1`, API, stage, exact requested/native/authorized
paths, kind, zero-based ancestor index/count, literal access/share/flags/disposition,
original HRESULT, Win32 code when HRESULT_FROM_WIN32 applies, and bounded original
message. Caller `verification_context` distinguishes selected launcher, allowed
file, observed application, inventory image and pre-dispatch cached pin.

On native API failure only, read the effective thread token (process token only
after ERROR_NO_TOKEN) with TOKEN_QUERY. Record SID, elevation, session and logon
statistics or per-query unknown/error. This read-only sample never replaces the
original fault and never enables privileges. Handle/LocalFree uncertainty is
sticky in the existing cleanup audit, not converted to success. Semantic guard
failures include stamps/path without inventing a native AccessDenied.

Both nested preflight errors and direct MCP errors preserve the diagnostic.
Legacy top-level MCP `error`/`next_step` fields remain compatible; text-only clients
also receive the original structured diagnostic alongside operation metadata.

## Offline gates and next reservation

Portable tests exercise the same production plan/check helpers: roots, alternate
drives/parents, the selected Win32 and packaged image chains, rejection at each
ancestor, no retry, prior-handle release exactly once, changed/reparse identities,
benign directory metadata changes and file stamp precision. Synthetic ACL rules
are explicitly fixtures, not Windows coverage. Core tests cover all three roles,
cleanup and no reservation/launch; the MCP worker test checks the actual error
serialization boundary. Prior checked-action, TTL, lease, cleanup and replay tests
are retained.

Required final gates are fmt, locked/offline default and all-feature workspace
tests, all-target/all-feature Mac and Windows ARM64 clippy, both release builds,
default zero-target Mac MCP smoke, plugin/skill validation, source archive and
source/binary SHA freeze. Windows is cross-compiled, not executed. No extra actual
model run is substituted for native acceptance.

Only the external coordinator may approve the exact two manifests and book a
fresh **continuous 60-minute** owner slot. The real `gpt-6-astra/max` main model
keeps its **1200-second** budget; the last **10 minutes are cleanup/handback only**.
All tests/builds/source fixes happen before booking. A needed live fix means HOLD,
preserve evidence, scoped cleanup/handback, then offline repair and another slot.
No current owner authority or booking is created by this candidate.
