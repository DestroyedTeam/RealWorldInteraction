//! Portable predicates and bounded decoders called by the native package proof.
//! This policy does not simulate package registration, Windows access or ACLs.
use super::disk_path;
use rwi_core::{
    Fault, Result, digest, fault,
    lifecycle::{ObservedPackage, WindowIdentity},
};
use serde::Serialize;

pub(super) const MAX_BUFFER_BYTES: usize = 65_536;
pub(super) const MAX_STRING_UNITS: usize = 4_096;
pub(super) const DEVELOPMENT_MODE: u32 = 0x1_0000;
pub(super) const KNOWN_PROPERTIES: u32 = 0x3d_000f;
// Narrow compatibility for the exact current-user, single-HEAD snapshot shape
// observed in r5. The SDK defines input PACKAGE_FILTER_HEAD with this value,
// but its public docs do not guarantee that bit's returned-flags semantics.
// Do not generalize to other filter bits or require this annotation to exist.
// Only used after all identity, same-user and selected-root predicates below.
const OBSERVED_SINGLE_HEAD_ANNOTATION: u32 = 0x10;
const KNOWN_SINGLE_HEAD_FLAGS: u32 = KNOWN_PROPERTIES | OBSERVED_SINGLE_HEAD_ANNOTATION;
pub(super) const ORIGIN_INBOX: i32 = 2;
pub(super) const ORIGIN_STORE: i32 = 3;
const MAX_DIAGNOSTIC_TEXT_BYTES: usize = 256;
const MAX_QUERY_RECORDS: usize = 96;

/// Observability only: no entry in this trace can authorize a native operation.
#[derive(Default, Serialize)]
pub(super) struct QueryTrace {
    entries: Vec<serde_json::Value>,
    omitted: usize,
}
impl QueryTrace {
    pub(super) fn record(&mut self, api: &str, status: &'static str) {
        if self.entries.len() == MAX_QUERY_RECORDS {
            self.omitted = self.omitted.saturating_add(1);
            return;
        }
        let bounded_api: String = api
            .chars()
            .take(96)
            .map(|c| {
                if c.is_ascii_graphic() && !matches!(c, '"' | '\\') || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.entries.push(serde_json::json!({
            "api":bounded_api,"api_truncated":api.chars().count() > 96,"status":status,
        }));
    }
    fn diagnostic(&self) -> serde_json::Value {
        serde_json::json!({
            "complete":self.omitted == 0,"omitted":self.omitted,"entries":self.entries,
            "completeness_semantics":"no trace entries omitted; not a native coverage authorization",
        })
    }
}

/// Called by the actual Windows Context, so the offline test covers the same
/// envelope/cause preservation without executing or pretending to execute FFI.
pub(super) fn decorate(
    mut error: Fault,
    spec: &ObservedPackage,
    key: &WindowIdentity,
    stage: &str,
    api: &str,
    queries: &QueryTrace,
) -> Fault {
    let cause = error.diagnostic.take();
    error.diagnostic = Some(Box::new(serde_json::json!({
        "schema":"rcu.windows.observed_package.v1", "role":"observed_package",
        "api":api, "stage":stage, "path":spec.application.executable,
        "package_full_name":spec.package_full_name, "install_root":spec.install_path,
        "allocated_window":key, "native_error":null, "cause":cause,
        "query_provenance":queries.diagnostic(),
        "permission_fallback_attempted":false, "acl_or_privileges_modified":false,
    })));
    error
}

pub(super) fn origin_symbol(origin: i32) -> &'static str {
    match origin {
        0 => "PackageOrigin_Unknown",
        1 => "PackageOrigin_Unsigned",
        2 => "PackageOrigin_Inbox",
        3 => "PackageOrigin_Store",
        4 => "PackageOrigin_DeveloperUnsigned",
        5 => "PackageOrigin_DeveloperSigned",
        6 => "PackageOrigin_LineOfBusiness",
        _ => "unrecognized_native_origin",
    }
}

fn diagnostic_text(value: &str) -> serde_json::Value {
    let mut end = value.len().min(MAX_DIAGNOSTIC_TEXT_BYTES);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    serde_json::json!({
        "value_prefix":&value[..end], "truncated":end != value.len(),
        "utf8_bytes":value.len(), "utf16_units":value.encode_utf16().count(),
        "sha256_utf8":digest(value.as_bytes()),
    })
}

fn snapshot_diagnostic(current: &PackageSnapshot) -> serde_json::Value {
    let process = &current.process;
    let token = |t: &TokenIdentity| {
        serde_json::json!({
            "sid":diagnostic_text(&t.sid), "session":t.session,
            "authentication_id":t.authentication_id, "source":diagnostic_text(&t.source),
        })
    };
    serde_json::json!({
        "process":{
            "pid":process.pid, "birth":process.birth, "live":process.live,
            "image":diagnostic_text(&process.image),
            "full_name":diagnostic_text(&process.full_name), "family":diagnostic_text(&process.family),
            "token":token(&process.token),
        },
        "worker":token(&current.worker), "head_count":current.head_count,
        "full_name":diagnostic_text(&current.full_name), "family":diagnostic_text(&current.family),
        "family_from_full_name":diagnostic_text(&current.family_from_full_name),
        "graph_path":diagnostic_text(&current.graph_path),
        "install_path":diagnostic_text(&current.install_path),
        "effective_path":diagnostic_text(&current.effective_path),
        "query_sources":{
            "process.pid":"GetProcessId", "process.birth":"GetProcessTimes",
            "process.live":"GetExitCodeProcess plus IsWindow/GetWindowThreadProcessId",
            "process.image":"QueryFullProcessImageNameW", "process.full_name":"GetPackageFullName",
            "process.family":"GetPackageFamilyName", "process.token":"OpenProcessToken(target, TOKEN_QUERY) + GetTokenInformation",
            "worker":"OpenThreadToken expected ERROR_NO_TOKEN; OpenProcessToken(worker, TOKEN_QUERY) + GetTokenInformation",
            "head_count/full_name/family/graph_path/flags":"OpenPackageInfoByFullName + GetPackageInfo(PACKAGE_FILTER_HEAD)",
            "family_from_full_name":"PackageFamilyNameFromFullName", "origin":"GetStagedPackageOrigin",
            "install_path":"GetPackagePathByFullName2(Install)", "effective_path":"GetPackagePathByFullName2(Effective)",
        },
        "query_status_location":"enclosing Context.query_provenance records actual returns; this projection does not manufacture query results",
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct TokenIdentity {
    pub sid: String,
    pub session: u32,
    /// The worker and target must share a native logon; never use setup metadata.
    pub authentication_id: u64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct ProcessIdentity {
    pub pid: u32,
    pub birth: u64,
    pub live: bool,
    pub image: String,
    pub full_name: String,
    pub family: String,
    pub token: TokenIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PackageSnapshot {
    pub process: ProcessIdentity,
    pub worker: TokenIdentity,
    pub head_count: u32,
    pub full_name: String,
    pub family: String,
    /// Native PackageFamilyNameFromFullName of the exact grant full name.
    pub family_from_full_name: String,
    pub graph_path: String,
    pub install_path: String,
    pub effective_path: String,
    pub flags: u32,
    pub origin: i32,
}

fn text(value: &str) -> bool {
    !value.is_empty()
        && value.encode_utf16().count() <= MAX_STRING_UNITS
        && !value.chars().any(char::is_control)
}

fn same_path(a: &str, b: &str) -> Result<bool> {
    Ok(disk_path(a)?.eq_ignore_ascii_case(&disk_path(b)?))
}

pub(super) fn same_user(target: &TokenIdentity, worker: &TokenIdentity) -> Result<()> {
    if !text(&target.sid)
        || !text(&worker.sid)
        || target.source != "target_process"
        || worker.source != "worker_process_no_thread_token"
        || target.sid != worker.sid
        || target.session != worker.session
        || target.authentication_id != worker.authentication_id
    {
        return Err(fault(
            "observed_package_user_mismatch",
            "Current-user registration requires matching target/worker SID, session and authentication LUID with no worker thread token; no impersonation or ForUser fallback.",
        ));
    }
    Ok(())
}

pub(super) fn validate(
    spec: &ObservedPackage,
    key: &WindowIdentity,
    current: &PackageSnapshot,
) -> Result<()> {
    let process = &current.process;
    let complete = [
        &spec.package_full_name,
        &current.full_name,
        &current.family,
        &current.family_from_full_name,
        &process.full_name,
        &process.family,
        &key.window,
        &key.native_epoch,
    ]
    .into_iter()
    .all(|s| text(s));
    if !complete || current.head_count != 1 {
        return Err(fault(
            "observed_package_metadata_incomplete",
            "Require exactly one complete package HEAD and bounded nonempty native identities.",
        ));
    }
    if key.pid == 0
        || key.process_created == 0
        || !process.live
        || process.pid != key.pid
        || process.birth != key.process_created
        || !same_path(&process.image, &spec.application.executable)?
    {
        return Err(fault(
            "observed_package_process_mismatch",
            "The retained live process must match the allocated PID, full creation FILETIME and exact granted image path.",
        ));
    }
    same_user(&process.token, &current.worker)?;
    if current.full_name != spec.package_full_name
        || process.full_name != spec.package_full_name
        || current.family != process.family
        || current.family != current.family_from_full_name
    {
        return Err(fault(
            "observed_package_identity_mismatch",
            "Process, registered HEAD and native full-name-derived family must all identify the exact granted package.",
        ));
    }
    let root = disk_path(&spec.install_path)?;
    let image = disk_path(&spec.application.executable)?;
    if !same_path(&current.graph_path, &root)?
        || !same_path(&current.install_path, &root)?
        || !same_path(&current.effective_path, &root)?
        || !image
            .to_ascii_lowercase()
            .starts_with(&format!("{}\\", root.to_ascii_lowercase()))
    {
        return Err(fault(
            "observed_package_root_mismatch",
            "HEAD, Install and Effective must equal the granted root and contain the exact image; no different selected effective root is accepted.",
        ));
    }
    if spec.application.sha256.len() != 64
        || !spec
            .application
            .sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(fault(
            "invalid_application_sha256",
            "Require an exact SHA-256 for the separately pinned observed image.",
        ));
    }
    // The observed annotation is recognized only after the complete single-HEAD,
    // process, same-user, package and selected-root checks above. The subsequent
    // native image hash pin, before/after revalidation and no-replay stay required.
    // Development and origin rules are unchanged; every other unknown bit holds.
    let development = current.flags & DEVELOPMENT_MODE != 0;
    let unknown_properties = current.flags & !KNOWN_SINGLE_HEAD_FLAGS != 0;
    let disallowed_origin = !matches!(current.origin, ORIGIN_INBOX | ORIGIN_STORE);
    let failures: Vec<_> = [
        (development, "development"),
        (unknown_properties, "unknown_properties"),
        (disallowed_origin, "disallowed_origin"),
    ]
    .into_iter()
    .filter_map(|(failed, category)| failed.then_some(category))
    .collect();
    if let Some(primary) = failures.first() {
        let mut error = fault(
            &format!("observed_package_{primary}"),
            "Package policy rejected the observed values; inspect every failed predicate. Store/Inbox allowlist, no-development and unknown-bit rejection remain required after exact single-HEAD identity/root checks.",
        );
        error.diagnostic = Some(Box::new(serde_json::json!({
            "schema":"rcu.windows.package_policy_rejection.v2",
            "primary_category":primary,"failed_predicates":failures,
            "origin":{"raw":current.origin,"symbol":origin_symbol(current.origin),"source_api":"GetStagedPackageOrigin"},
            "head_flags":{
                "raw":current.flags,"hex":format!("0x{:08x}",current.flags),
                "unknown_bits":current.flags & !KNOWN_SINGLE_HEAD_FLAGS,
                "unknown_bits_hex":format!("0x{:08x}",current.flags & !KNOWN_SINGLE_HEAD_FLAGS),
                "known_properties_mask":KNOWN_PROPERTIES,"development_mask":DEVELOPMENT_MODE,
                "known_single_head_flags_mask":KNOWN_SINGLE_HEAD_FLAGS,
                "documented_property_bits":current.flags & KNOWN_PROPERTIES,
                "observed_single_head_annotation_bits":current.flags & OBSERVED_SINGLE_HEAD_ANNOTATION,
                "annotation_policy":"r5 exact single-HEAD compatibility after identity/user/root checks; not a universal SDK returned-bit guarantee",
                "source_api":"GetPackageInfo(PACKAGE_FILTER_HEAD)",
            },
            "predicates":{
                "development_absent":!development,"properties_known":!unknown_properties,"origin_allowed":!disallowed_origin,
            },
            "snapshot":snapshot_diagnostic(current),
            "native_error":null,
            "decision_kind":"policy_rejection_after_native_snapshot; not a fabricated Win32 API failure",
        })));
        return Err(error);
    }
    Ok(())
}

pub(super) fn unchanged<T: PartialEq>(before: &T, after: &T) -> Result<()> {
    if before != after {
        return Err(fault(
            "observed_package_changed",
            "Process, effective user or current package metadata changed during proof; hold without replay.",
        ));
    }
    Ok(())
}

pub(super) fn matches(
    cached_spec: &ObservedPackage,
    cached_key: &WindowIdentity,
    spec: &ObservedPackage,
    key: &WindowIdentity,
) -> bool {
    cached_spec == spec
        && cached_key == key
        && key.pid != 0
        && key.process_created != 0
        && text(&key.window)
        && text(&key.native_epoch)
}

/// Same epoch formula as desktop::record, but supplied with creation time from
/// the retained process handle instead of reopening a handle by PID.
pub(super) fn window_identity(
    window: String,
    pid: u32,
    birth: u64,
    class: &str,
) -> Result<WindowIdentity> {
    if pid == 0 || birth == 0 || !text(&window) || !text(class) {
        return Err(fault(
            "observed_package_window_incomplete",
            "The window class, HWND, PID and process creation time must all be proven.",
        ));
    }
    Ok(WindowIdentity {
        native_epoch: digest(format!("{window}:{pid}:{birth}:{class}").as_bytes()),
        window,
        pid,
        process_created: birth,
    })
}

pub(super) fn utf16(units: &[u16]) -> Result<String> {
    if units.is_empty() || units.len() > MAX_STRING_UNITS || units.contains(&0) {
        return Err(fault(
            "observed_package_buffer_invalid",
            "Native text is empty, contains embedded NUL or exceeds 4096 UTF-16 units.",
        ));
    }
    let decoded = String::from_utf16(units).map_err(|_| {
        fault(
            "observed_package_buffer_invalid",
            "Native metadata contains invalid UTF-16; no lossy identity conversion.",
        )
    })?;
    if !text(&decoded) {
        return Err(fault(
            "observed_package_buffer_invalid",
            "Native identity contains control characters.",
        ));
    }
    Ok(decoded)
}

pub(super) fn buffer_range(
    bytes: &[u8],
    base: usize,
    address: usize,
    minimum_offset: usize,
    length: usize,
    alignment: usize,
) -> Result<std::ops::Range<usize>> {
    let end = base.checked_add(bytes.len());
    let offset = address.checked_sub(base);
    let range = offset.and_then(|start| start.checked_add(length).map(|end| start..end));
    match range {
        Some(range)
            if !bytes.is_empty()
                && bytes.len() <= MAX_BUFFER_BYTES
                && end.is_some()
                && alignment != 0
                && address.is_multiple_of(alignment)
                && range.start >= minimum_offset
                && range.end <= bytes.len() =>
        {
            Ok(range)
        }
        _ => Err(fault(
            "observed_package_buffer_invalid",
            "Native metadata pointer lies outside the returned bounded buffer, is misaligned or overlaps its header.",
        )),
    }
}

/// Decode only within the returned buffer; never call PWSTR::to_string/len.
pub(super) fn buffer_string(
    bytes: &[u8],
    base: usize,
    address: usize,
    minimum_offset: usize,
) -> Result<String> {
    let start = buffer_range(bytes, base, address, minimum_offset, 2, 2)?.start;
    let mut units = Vec::with_capacity(((bytes.len() - start) / 2).min(MAX_STRING_UNITS));
    for unit in bytes[start..].chunks_exact(2).take(MAX_STRING_UNITS + 1) {
        let unit = u16::from_le_bytes([unit[0], unit[1]]);
        if unit == 0 {
            return utf16(&units);
        }
        units.push(unit);
    }
    Err(fault(
        "observed_package_buffer_invalid",
        "Native metadata lacks a bounded in-buffer UTF-16 terminator.",
    ))
}

/// TOKEN_USER's SID is variable length. Validate the entire binary SID before
/// reading subauthorities; formatting avoids an unbounded LocalAlloc PWSTR.
pub(super) fn buffer_sid(
    bytes: &[u8],
    base: usize,
    address: usize,
    minimum_offset: usize,
) -> Result<String> {
    let header = buffer_range(bytes, base, address, minimum_offset, 8, 4)?;
    let revision = bytes[header.start];
    let count = usize::from(bytes[header.start + 1]);
    if revision != 1 || count > 15 {
        return Err(fault(
            "observed_package_buffer_invalid",
            "Require a bounded revision-1 Windows SID.",
        ));
    }
    let sid = buffer_range(bytes, base, address, minimum_offset, 8 + count * 4, 4)?;
    let sid = &bytes[sid];
    let authority = sid[2..8]
        .iter()
        .fold(0u64, |value, byte| (value << 8) | u64::from(*byte));
    let mut text = format!("S-{revision}-{authority}");
    for subauthority in sid[8..].chunks_exact(4) {
        let value = u32::from_le_bytes(subauthority.try_into().expect("four bytes"));
        text.push_str(&format!("-{value}"));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rwi_core::lifecycle::ApplicationIdentity;

    fn fixture() -> (ObservedPackage, WindowIdentity, PackageSnapshot) {
        let full = "Example.Editor_1.2.3.4_arm64__abcdefghijklm".to_owned();
        let family = "Example.Editor_abcdefghijklm".to_owned();
        let root = format!(r"C:\Program Files\WindowsApps\{full}");
        let spec = ObservedPackage {
            application: ApplicationIdentity {
                executable: format!(r"{root}\Editor\Editor.exe"),
                sha256: "a".repeat(64),
            },
            package_full_name: full.clone(),
            install_path: root.clone(),
        };
        let key =
            window_identity("hwnd:123".into(), 42, 134_000_000_000_000_123, "Editor").unwrap();
        let token = TokenIdentity {
            sid: "S-1-5-21-123-456-789-1000".into(),
            session: 1,
            authentication_id: 1234,
            source: "target_process".into(),
        };
        let mut worker = token.clone();
        worker.source = "worker_process_no_thread_token".into();
        let snapshot = PackageSnapshot {
            process: ProcessIdentity {
                pid: key.pid,
                birth: key.process_created,
                live: true,
                image: spec.application.executable.clone(),
                full_name: full.clone(),
                family: family.clone(),
                token,
            },
            worker,
            head_count: 1,
            full_name: full,
            family: family.clone(),
            family_from_full_name: family,
            graph_path: root.clone(),
            install_path: root.clone(),
            effective_path: root,
            flags: 0,
            origin: ORIGIN_STORE,
        };
        (spec, key, snapshot)
    }

    #[test]
    fn native_snapshot_policy_accepts_store_or_inbox_and_exact_same_user() {
        let (spec, key, mut current) = fixture();
        validate(&spec, &key, &current).unwrap();
        current.origin = ORIGIN_INBOX;
        current.install_path = current.install_path.to_ascii_uppercase();
        current.effective_path = format!(r"\\?\{}", current.effective_path);
        validate(&spec, &key, &current).unwrap();
    }

    #[test]
    fn wrong_full_family_and_native_derived_family_hold() {
        let (spec, key, current) = fixture();
        for field in 0..5 {
            let mut changed = current.clone();
            match field {
                0 => changed.full_name.push('X'),
                1 => changed.family.push('X'),
                2 => changed.process.full_name.push('X'),
                3 => changed.process.family.push('X'),
                _ => changed.family_from_full_name.push('X'),
            }
            assert_eq!(
                validate(&spec, &key, &changed).unwrap_err().code,
                "observed_package_identity_mismatch"
            );
        }
    }

    #[test]
    fn development_unknown_flags_and_every_untrusted_origin_hold() {
        let (spec, key, current) = fixture();
        for origin in [-1, 0, 1, 4, 5, 6, 7, i32::MAX] {
            let mut changed = current.clone();
            changed.origin = origin;
            assert!(validate(&spec, &key, &changed).is_err());
        }
        for flags in [DEVELOPMENT_MODE, 0x8000_0000] {
            let mut changed = current.clone();
            changed.flags = flags;
            assert!(validate(&spec, &key, &changed).is_err());
        }
    }

    #[test]
    fn graph_install_mutable_or_external_effective_redirection_holds() {
        let (spec, key, current) = fixture();
        for root in [r"C:\Users\user\Mutable", r"D:\External", r"C:\OtherPackage"] {
            for field in 0..3 {
                let mut changed = current.clone();
                match field {
                    0 => changed.graph_path = root.into(),
                    1 => changed.install_path = root.into(),
                    _ => changed.effective_path = root.into(),
                }
                assert_eq!(
                    validate(&spec, &key, &changed).unwrap_err().code,
                    "observed_package_root_mismatch"
                );
            }
        }
        // The real predicate proves no different selected Effective root, not
        // the absence of every possible mutable/external configuration.
        let mut outside = spec.clone();
        outside.install_path.push_str("-lookalike");
        assert!(validate(&outside, &key, &current).is_err());
    }

    #[test]
    fn pid_reuse_exit_path_substitution_and_incomplete_metadata_hold() {
        let (spec, key, current) = fixture();
        for field in 0..8 {
            let mut changed = current.clone();
            match field {
                0 => changed.process.birth += 1, // Preserve all 100 ns bits.
                1 => changed.process.pid += 1,
                2 => changed.process.live = false,
                3 => changed.process.image = r"C:\Users\user\Editor.exe".into(),
                4 => changed.head_count = 0,
                5 => changed.head_count = 2,
                6 => changed.family.clear(),
                _ => changed.effective_path.clear(),
            }
            assert!(validate(&spec, &key, &changed).is_err());
        }
    }

    #[test]
    fn different_or_unknown_effective_user_and_session_hold() {
        let (spec, key, current) = fixture();
        for field in 0..6 {
            let mut changed = current.clone();
            match field {
                0 => changed.worker.sid.push('1'),
                1 => changed.worker.session += 1,
                2 => changed.process.token.sid.clear(),
                3 => changed.worker.source.clear(),
                4 => changed.worker.authentication_id += 1,
                _ => changed.worker.source = "worker_thread".into(),
            }
            assert_eq!(
                validate(&spec, &key, &changed).unwrap_err().code,
                "observed_package_user_mismatch"
            );
        }
    }

    #[test]
    fn cached_proof_is_exact_spec_full_window_and_process_birth_not_pid_only() {
        let (spec, key, snapshot) = fixture();
        assert!(matches(&spec, &key, &spec, &key));
        for field in 0..4 {
            let mut changed = key.clone();
            match field {
                0 => changed.pid += 1,
                1 => changed.process_created += 1,
                2 => changed.window.push('1'),
                _ => changed.native_epoch.push('1'),
            }
            assert!(!matches(&spec, &key, &spec, &changed));
        }
        for field in 0..4 {
            let mut changed = spec.clone();
            match field {
                0 => changed.application.executable.push('x'),
                1 => changed.application.sha256 = "b".repeat(64),
                2 => changed.package_full_name.push('x'),
                _ => changed.install_path.push('x'),
            }
            assert!(!matches(&spec, &key, &changed, &key));
        }
        unchanged(&snapshot, &snapshot).unwrap();
        let mut changed = snapshot.clone();
        changed.worker.authentication_id += 1;
        assert!(unchanged(&snapshot, &changed).is_err());
        changed = snapshot.clone();
        changed.effective_path.push('x');
        assert!(unchanged(&snapshot, &changed).is_err());
        assert_eq!(
            key.native_epoch,
            digest(format!("{}:{}:{}:Editor", key.window, key.pid, key.process_created).as_bytes())
        );
    }

    #[test]
    fn bounded_pointer_decoder_rejects_oob_header_unaligned_overflow_and_surrogates() {
        let mut bytes = vec![0u8; 32];
        bytes[16..22].copy_from_slice(&[b'o', 0, b'k', 0, 0, 0]);
        assert_eq!(buffer_string(&bytes, 0x1000, 0x1010, 16).unwrap(), "ok");
        for pointer in [0, 0xffe, 0x1000, 0x100f, 0x1011, 0x101f, 0x1020, usize::MAX] {
            assert!(buffer_string(&bytes, 0x1000, pointer, 16).is_err());
        }
        assert!(buffer_string(&bytes, usize::MAX - 8, usize::MAX - 4, 0).is_err());
        assert!(buffer_string(&vec![0; MAX_BUFFER_BYTES + 1], 0x1000, 0x1010, 16).is_err());
        assert!(utf16(&[0xd800]).is_err());
        assert!(utf16(&[b'a' as u16, 0, b'b' as u16]).is_err());
        assert!(utf16(&[10]).is_err());
    }

    #[test]
    fn bounded_pointer_decoder_requires_in_buffer_terminator_and_size_bound() {
        let bytes = [65u16; MAX_STRING_UNITS]
            .into_iter()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            buffer_string(&bytes, 0x1000, 0x1000, 0).unwrap().len(),
            MAX_STRING_UNITS
        );
        assert!(buffer_string(&bytes[..bytes.len() - 2], 0x1000, 0x1000, 0).is_err());
        let oversized = [65u16; MAX_STRING_UNITS + 1]
            .into_iter()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert!(buffer_string(&oversized, 0x1000, 0x1000, 0).is_err());
    }

    #[test]
    fn token_sid_parser_checks_full_variable_length_range() {
        let mut bytes = vec![0u8; 32];
        bytes[16..28].copy_from_slice(&[1, 1, 0, 0, 0, 0, 0, 5, 232, 3, 0, 0]);
        assert_eq!(
            buffer_sid(&bytes, 0x1000, 0x1010, 16).unwrap(),
            "S-1-5-1000"
        );
        assert!(buffer_sid(&bytes[..27], 0x1000, 0x1010, 16).is_err());
        assert!(buffer_sid(&bytes, 0x1000, 0x1000, 16).is_err());
        bytes[17] = 16;
        assert!(buffer_sid(&bytes, 0x1000, 0x1010, 16).is_err());
        bytes[17] = 1;
        bytes[16] = 2;
        assert!(buffer_sid(&bytes, 0x1000, 0x1010, 16).is_err());
    }

    #[test]
    fn rejection_categories_retain_every_raw_origin_including_unknown_values() {
        let (spec, key, mut current) = fixture();
        for origin in [i32::MIN, -1, 0, 1, 4, 5, 6, 7, i32::MAX] {
            current.origin = origin;
            let error = validate(&spec, &key, &current).unwrap_err();
            assert_eq!(error.code, "observed_package_disallowed_origin");
            let d = error.diagnostic.unwrap();
            assert_eq!(d["origin"]["raw"], origin);
            assert_eq!(d["origin"]["symbol"], origin_symbol(origin));
            assert_eq!(
                d["failed_predicates"],
                serde_json::json!(["disallowed_origin"])
            );
            assert_eq!(
                d["predicates"],
                serde_json::json!({
                    "development_absent":true,"properties_known":true,"origin_allowed":false,
                })
            );
            assert!(d["native_error"].is_null());
        }
        assert_eq!(origin_symbol(0), "PackageOrigin_Unknown");
        assert_eq!(origin_symbol(4), "PackageOrigin_DeveloperUnsigned");
        assert_eq!(origin_symbol(5), "PackageOrigin_DeveloperSigned");
        assert_eq!(origin_symbol(6), "PackageOrigin_LineOfBusiness");
        assert_eq!(origin_symbol(i32::MAX), "unrecognized_native_origin");
    }

    #[test]
    fn legacy_combinations_keep_denial_outcomes_and_all_failures_survive() {
        let (spec, key, mut current) = fixture();
        for flags in [
            0,
            1,
            DEVELOPMENT_MODE,
            0x8000_0000,
            DEVELOPMENT_MODE | 0x8000_0001,
            KNOWN_PROPERTIES,
            u32::MAX,
        ] {
            for origin in [
                i32::MIN,
                -1,
                0,
                1,
                ORIGIN_INBOX,
                ORIGIN_STORE,
                4,
                5,
                6,
                7,
                i32::MAX,
            ] {
                current.flags = flags;
                current.origin = origin;
                let prior_denial = flags & DEVELOPMENT_MODE != 0
                    || flags & !KNOWN_PROPERTIES != 0
                    || !matches!(origin, ORIGIN_INBOX | ORIGIN_STORE);
                let result = validate(&spec, &key, &current);
                assert_eq!(result.is_err(), prior_denial);
                if let Err(error) = result {
                    let mut expected = Vec::new();
                    if flags & DEVELOPMENT_MODE != 0 {
                        expected.push("development");
                    }
                    if flags & !KNOWN_PROPERTIES != 0 {
                        expected.push("unknown_properties");
                    }
                    if !matches!(origin, ORIGIN_INBOX | ORIGIN_STORE) {
                        expected.push("disallowed_origin");
                    }
                    assert_eq!(error.code, format!("observed_package_{}", expected[0]));
                    let d = error.diagnostic.unwrap();
                    assert_eq!(d["failed_predicates"], serde_json::json!(expected));
                    assert_eq!(d["head_flags"]["raw"], flags);
                    assert_eq!(d["head_flags"]["hex"], format!("0x{flags:08x}"));
                    assert_eq!(
                        d["head_flags"]["unknown_bits"],
                        flags & !KNOWN_SINGLE_HEAD_FLAGS
                    );
                    assert_eq!(
                        d["head_flags"]["unknown_bits_hex"],
                        format!("0x{:08x}", flags & !KNOWN_SINGLE_HEAD_FLAGS)
                    );
                    assert_eq!(
                        d["predicates"]["development_absent"],
                        flags & DEVELOPMENT_MODE == 0
                    );
                    assert_eq!(
                        d["predicates"]["properties_known"],
                        flags & !KNOWN_SINGLE_HEAD_FLAGS == 0
                    );
                    assert_eq!(
                        d["predicates"]["origin_allowed"],
                        matches!(origin, ORIGIN_INBOX | ORIGIN_STORE)
                    );
                }
            }
        }
    }

    #[test]
    fn rejection_snapshot_identity_and_query_sources_are_complete_without_fabricated_api_error() {
        let (spec, key, mut current) = fixture();
        current.flags = 0x8000_0000;
        let error = validate(&spec, &key, &current).unwrap_err();
        assert_eq!(error.code, "observed_package_unknown_properties");
        let d = error.diagnostic.unwrap();
        assert_eq!(d["origin"]["source_api"], "GetStagedPackageOrigin");
        assert_eq!(
            d["head_flags"]["source_api"],
            "GetPackageInfo(PACKAGE_FILTER_HEAD)"
        );
        let snapshot = &d["snapshot"];
        assert_eq!(snapshot["process"]["pid"], key.pid);
        assert_eq!(snapshot["process"]["birth"], key.process_created);
        assert_eq!(snapshot["process"]["live"], true);
        assert_eq!(
            snapshot["process"]["image"]["value_prefix"],
            spec.application.executable
        );
        assert_eq!(
            snapshot["process"]["image"]["sha256_utf8"],
            digest(spec.application.executable.as_bytes())
        );
        assert_eq!(
            snapshot["full_name"]["value_prefix"],
            spec.package_full_name
        );
        assert_eq!(
            snapshot["worker"]["authentication_id"],
            current.worker.authentication_id
        );
        assert_eq!(
            snapshot["process"]["token"]["authentication_id"],
            current.process.token.authentication_id
        );
        for field in [
            "full_name",
            "family",
            "family_from_full_name",
            "graph_path",
            "install_path",
            "effective_path",
        ] {
            assert!(snapshot[field]["value_prefix"].is_string());
            assert_eq!(snapshot[field]["truncated"], false);
        }
        let sources = snapshot["query_sources"].as_object().unwrap();
        assert_eq!(sources.len(), 13);
        assert!(
            sources
                .values()
                .all(|value| value.as_str().is_some_and(|s| !s.is_empty()))
        );
        assert!(d["native_error"].is_null());
        assert!(d.get("win32_code").is_none());
    }

    #[test]
    fn native_context_decorator_and_outer_verification_context_preserve_cause_and_actual_trace() {
        let (spec, key, mut current) = fixture();
        current.origin = -9123;
        current.flags = DEVELOPMENT_MODE | 0x8000_0000;
        // Explicit offline fixture returns; no claim that these APIs ran in test.
        let mut queries = QueryTrace::default();
        queries.record("OpenPackageInfoByFullName", "succeeded");
        queries.record(
            "GetPackageInfo(PACKAGE_FILTER_HEAD)",
            "expected_insufficient_buffer",
        );
        queries.record("GetPackageInfo(PACKAGE_FILTER_HEAD)", "succeeded");
        queries.record("GetStagedPackageOrigin", "succeeded");
        let error = decorate(
            validate(&spec, &key, &current).unwrap_err(),
            &spec,
            &key,
            "before_package_image_hash",
            "native_package_snapshot_policy",
            &queries,
        );
        let error = rwi_core::lifecycle::pin_failure_context(
            error,
            "readonly_preflight",
            "observed_package",
            &spec.application.executable,
        );
        let d = error.diagnostic.as_deref().unwrap();
        assert_eq!(d["cause"]["origin"]["raw"], -9123);
        assert_eq!(
            d["cause"]["failed_predicates"],
            serde_json::json!(["development", "unknown_properties", "disallowed_origin"])
        );
        assert_eq!(d["allocated_window"], serde_json::to_value(&key).unwrap());
        assert_eq!(d["verification_context"]["phase"], "readonly_preflight");
        assert_eq!(d["query_provenance"]["complete"], true);
        assert_eq!(
            d["query_provenance"]["entries"].as_array().unwrap().len(),
            4
        );
        assert_eq!(d["query_provenance"]["entries"][3]["status"], "succeeded");
        assert!(d["native_error"].is_null());
        let encoded = serde_json::to_string(&error).unwrap();
        println!("SYNTHETIC_POLICY_REJECTION_JSON={encoded}");
        let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            decoded["diagnostic"]["cause"]["head_flags"]["unknown_bits"],
            0x8000_0000u32
        );
        let nested = decorate(
            error,
            &spec,
            &key,
            "outer",
            "outer_context",
            &QueryTrace::default(),
        );
        assert_eq!(
            nested.diagnostic.unwrap()["cause"]["cause"]["origin"]["raw"],
            -9123
        );
    }

    #[test]
    fn diagnostic_snapshot_and_query_trace_are_bounded_with_explicit_truncation() {
        let (mut spec, key, mut current) = fixture();
        spec.install_path = format!(r"C:\{}", "界".repeat(4_090));
        spec.application.executable = format!("{}\\a", spec.install_path);
        current.process.image = spec.application.executable.clone();
        current.graph_path = spec.install_path.clone();
        current.install_path = spec.install_path.clone();
        current.effective_path = spec.install_path.clone();
        current.flags = DEVELOPMENT_MODE;
        let mut trace = QueryTrace::default();
        for _ in 0..MAX_QUERY_RECORDS + 3 {
            trace.record(&"x".repeat(4_096), "succeeded");
        }
        let error = decorate(
            validate(&spec, &key, &current).unwrap_err(),
            &spec,
            &key,
            "before_package_image_hash",
            "native_package_snapshot_policy",
            &trace,
        );
        let error = rwi_core::lifecycle::pin_failure_context(
            error,
            "readonly_preflight",
            "observed_package",
            &spec.application.executable,
        );
        let d = error.diagnostic.as_deref().unwrap();
        assert_eq!(
            d["cause"]["snapshot"]["process"]["image"]["truncated"],
            true
        );
        assert_eq!(
            d["cause"]["snapshot"]["process"]["image"]["sha256_utf8"],
            digest(spec.application.executable.as_bytes())
        );
        assert_eq!(d["query_provenance"]["complete"], false);
        assert_eq!(d["query_provenance"]["omitted"], 3);
        assert_eq!(
            d["query_provenance"]["entries"].as_array().unwrap().len(),
            MAX_QUERY_RECORDS
        );
        let bytes = serde_json::to_vec(&error).unwrap().len();
        println!(
            "SYNTHETIC_POLICY_BOUNDS_JSON={}",
            serde_json::json!({
                "serialized_fault_bytes":bytes,"limit_bytes":MAX_BUFFER_BYTES,
                "trace_entries":MAX_QUERY_RECORDS,"trace_omitted":3,
            })
        );
        assert!(bytes < MAX_BUFFER_BYTES);
    }
}

#[cfg(test)]
#[path = "package_policy_r7_tests.rs"]
mod real_snapshot_regressions;
