# M3a.7 single-HEAD flags compatibility contract

## Evidence and non-claims

- Actual .6/r5 preflight SHA `76c36b81973e9e3143ee5c4d8b63b8458523b9efbb4b4bd7c10471dc15558539` reports raw flags 16, origin 3 (Store), sole failure unknown_properties, HEAD count 1, complete exact native strings, same SID/session/LUID and 55 actual trace entries. It stopped before image hash/root-down pin. No launch/input/capture/model followed.
- Historical launch-grant SHA `465ab927a32930ef81420fac29cf648edee114d9076c773665ae26792fedf75c` is an immutable offline fixture ONLY. The allocation was revoked at 2026-09-10T21:02:29Z. No tests or code execute that grant.
- [GetPackageInfo](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getpackageinfo) documents input filtering and the returned PACKAGE_INFO array. [PACKAGE_INFO](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/ns-appmodel-package_info) describes its flags as properties without enumerating returned HEAD bits. [Package constants](https://learn.microsoft.com/en-us/windows/win32/appxpkg/package-constants) defines HEAD=0x10 as an INPUT filter; numerical equality is not an output guarantee.
- [Microsoft SDK header](https://github.com/microsoft/win32metadata/blob/main/generation/WinSDK/RecompiledIdlHeaders/um/appmodel.h), the cached actual xwin SDK and windows 0.62.2 bindings agree on packed-4 layout, flags offset 4. Candidate compile-time checks cover 64-bit PACKAGE_INFO (80 bytes, align 4) and PACKAGE_ID (48 bytes). Cross-build compilation is not execution on Windows. r5 contains no raw native buffer, so this is not independent re-decoding of that buffer.
- [WindowsAppSDK PackageGraphManager](https://github.com/microsoft/WindowsAppSDK/blob/main/dev/DynamicDependency/API/PackageGraphManager.cpp) copies returned flags, and [MddDetourPackageGraph](https://github.com/microsoft/WindowsAppSDK/blob/main/dev/DynamicDependency/API/MddDetourPackageGraph.cpp) passes native results through. Neither proves a universal returned HEAD interpretation.

## Exact policy delta

`KNOWN_PROPERTIES=0x003d000f` is unchanged. Only `OBSERVED_SINGLE_HEAD_ANNOTATION=0x10` is added to recognition after existing count, full/family, process, effective-user, root equality and image-hash-format checks have passed. Recognition is a deliberately narrow empirical compatibility policy, not a newly invented PACKAGE_PROPERTY_HEAD or automatic echo of query inputs. The existing root/hash-format block now precedes flags classification, so invalid-root/invalid-hash errors may take precedence in multiply-invalid snapshots. Native API order and later image byte-hash pin do not change.

Flags=0 stays compatible because the public contract does not make bit 0x10 mandatory. Raw values are never normalized before snapshot drift comparisons. Other input-only filters/information bits (0x20/0x40/0x80/0x100/0x20000), unknown future bits, development and disallowed origins remain rejected. Existing STATIC/DYNAMIC/HOSTRUNTIME properties remain as before. Recognition never supplies an owner grant, native image proof, window ownership, lease or no-replay exception.

Diagnostic v2 preserves the existing rejection code/categories while distinguishing documented property bits, observed annotation bits, recognized-single-HEAD mask and still-unknown bits.

## Public tests and preservation

The first 10 legacy package-policy tests stay byte-identical. The original 77-row diagnostic matrix retains its old denial and all-failure-category assertions; only 3 expected unknown-bit projections now use the recognized-single-HEAD mask, and its name reflects that it is a legacy-outcome regression. This is an explicit correction of previously wrong bit classification in a new isolated source, not weakening a frozen parent verifier. Other existing diagnostic assertions stay intact.

Seven new public tests reconstruct the exact r5 snapshot from original bytes after SHA, complete-text length/hash and trace checks; compare old-mask rejection versus new conditional recognition; cover all 32 individual bits, input-filter combinations, each remaining unknown bit combined with development and bad origin, the seven defined origin enums plus four boundary/unknown origin samples, 20 identity/user mutations, selected-root/hash-format failures and raw-flags drift/cache mismatches. These tests validate portable predicates only, not live Windows behavior or filesystem pin success.

No old .6/r5 source, binaries, run or parent evidence is changed. r4 raw origin/flags remain unknown; r5 cleanup success does not prove r4 PrlJob_GetResult/255 fixed. .7/r6 needs external exact-manifest review before a fresh continuous 60-minute native-only slot, full main gpt-6-astra/max 1200s, last 600s cleanup only. No unit tests, build, edits, replacement packages or replay inside the live slot.
