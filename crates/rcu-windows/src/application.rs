//! Explicit native lifecycle primitives, not launch authorization or input authority.
//! No native operation is run by the offline tests in this module.
use rcu_core::{Result, fault};
#[path = "package_policy.rs"]
mod package_policy;
#[path = "application_pin.rs"]
mod pin;

const MAX_PATH_UNITS: usize = 4_096;
const MAX_PATH_COMPONENTS: usize = 64;
const MAX_COMMAND_UNITS: usize = 32_767;

/// Deliberately narrower than Win32: local drive-absolute paths only, no aliases,
/// streams, dot segments, device names, or normalization-sensitive components.
fn disk_path(path: &str) -> Result<String> {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let b = path.as_bytes();
    if path.encode_utf16().count() > MAX_PATH_UNITS
        || b.len() < 4
        || !b[0].is_ascii_alphabetic()
        || b[1] != b':'
        || b[2] != b'\\'
        || path
            .chars()
            .any(|c| c.is_control() || "\"<>|?*/".contains(c))
        || path[3..].contains(':')
    {
        return Err(fault(
            "unsupported_application_path",
            "Use a local drive-absolute Windows file path, at most 4096 UTF-16 units; no UNC, device, stream, wildcard, or control characters.",
        ));
    }
    let mut count = 0;
    for component in path[3..].split('\\') {
        count += 1;
        let stem = component
            .split('.')
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || reserved
            || count > MAX_PATH_COMPONENTS
        {
            return Err(fault(
                "unsupported_application_path",
                "Path components must be unambiguous, non-device names; at most 64 components.",
            ));
        }
    }
    Ok(path.to_owned())
}

/// MS CRT / CommandLineToArgvW-compatible argument escaping, without a shell.
/// Always quote, double backslashes before a quote and before the closing quote.
fn quote_arg(arg: &str) -> Result<String> {
    if arg.contains('\0') || arg.encode_utf16().count() >= MAX_COMMAND_UNITS {
        return Err(fault(
            "invalid_application_argument",
            "Argument contains NUL or exceeds the native command-line bound.",
        ));
    }
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
            continue;
        }
        quoted.extend(std::iter::repeat_n(
            '\\',
            backslashes * if c == '"' { 2 } else { 1 },
        ));
        if c == '"' {
            quoted.push('\\');
        }
        quoted.push(c);
        backslashes = 0;
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    Ok(quoted)
}

fn command_line(executable: &str, arguments: &[String]) -> Result<Vec<u16>> {
    disk_path(executable)?;
    if arguments.len() > 64 {
        return Err(fault(
            "invalid_application_argument",
            "At most 64 arguments are supported.",
        ));
    }
    let mut command = quote_arg(executable)?;
    for argument in arguments {
        command.push(' ');
        command.push_str(&quote_arg(argument)?);
        if command.encode_utf16().count() >= MAX_COMMAND_UNITS {
            return Err(fault(
                "application_command_too_long",
                "The command line including NUL must fit 32767 UTF-16 units.",
            ));
        }
    }
    let mut wide: Vec<_> = command.encode_utf16().collect();
    wide.push(0);
    if wide.len() > MAX_COMMAND_UNITS {
        return Err(fault(
            "application_command_too_long",
            "The command line including NUL must fit 32767 UTF-16 units.",
        ));
    }
    Ok(wide)
}

/// A close failure is sticky. Drop attempts each native close at most once and
/// never turns an uncertain close into a successful resource-release report.
struct ReleaseGuard<T, F: FnMut(T) -> bool> {
    value: Option<T>,
    release: F,
    released: bool,
}
impl<T, F: FnMut(T) -> bool> ReleaseGuard<T, F> {
    fn new(value: T, release: F) -> Self {
        Self {
            value: Some(value),
            release,
            released: true,
        }
    }
    fn close(&mut self) -> bool {
        if let Some(value) = self.value.take() {
            self.released &= (self.release)(value);
        }
        self.released
    }
}
impl<T, F: FnMut(T) -> bool> Drop for ReleaseGuard<T, F> {
    fn drop(&mut self) {
        self.close();
    }
}

/// Callback storage allocates up front and is cumulative: an overflow remains
/// incomplete even after later snapshots, rather than silently truncating.
struct BoundedEvents<T> {
    events: Vec<T>,
    complete: bool,
}
impl<T> BoundedEvents<T> {
    fn new() -> Self {
        Self {
            events: Vec::with_capacity(rcu_core::lifecycle::MAX_EVENTS),
            complete: true,
        }
    }
    fn push(&mut self, event: T) {
        if self.events.len() == rcu_core::lifecycle::MAX_EVENTS {
            self.complete = false;
        } else {
            self.events.push(event);
        }
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::desktop;
    use pin::{PinKind, PinPlan, PinRequest, PinStamp};
    use rcu_core::{
        Identity,
        lifecycle::{
            AllowedFile, ApplicationIdentity, EventBatch, Inventory, LaunchPermission,
            LifecycleDriver, MAX_METADATA_BYTES, MAX_WINDOWS, NativeEvent, NativeLaunch,
            ObservedPackage, WindowEvidence, WindowIdentity, pin_failure_context,
        },
        operation::{checkpoint, native_call, resource_release_uncertain},
    };
    use sha2::{Digest, Sha256};
    use std::{
        cell::RefCell,
        mem::size_of,
        rc::Rc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use windows::{
        Win32::{
            Foundation::*,
            Security::{Authorization::ConvertSidToStringSidW, *},
            Storage::FileSystem::*,
            System::{StationsAndDesktops::*, Threading::*},
            UI::{
                Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
                WindowsAndMessaging::*,
            },
        },
        core::{BOOL, PCWSTR, PWSTR},
    };

    mod package {
        include!("application_package.rs");
    }
    use package::ObservedProof;

    const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
    const MAX_PINNED_FILES: usize = 64;
    const MAX_ENUMERATED_WINDOWS: usize = 512;

    type Handle = ReleaseGuard<HANDLE, fn(HANDLE) -> bool>;

    fn close_handle(handle: HANDLE) -> bool {
        let released = handle.is_invalid() || unsafe { CloseHandle(handle) }.is_ok();
        if !released {
            resource_release_uncertain();
        }
        released
    }
    fn handle(raw: HANDLE) -> Handle {
        ReleaseGuard::new(raw, close_handle)
    }
    fn raw(handle: &Handle) -> HANDLE {
        handle.value.unwrap_or_default()
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn ticks(t: FILETIME) -> u64 {
        (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
    }
    fn unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis().min(u128::from(u64::MAX)) as u64)
    }

    fn native_error(error: &windows::core::Error) -> serde_json::Value {
        let code = error.code().0 as u32;
        serde_json::json!({
            "hresult": format!("0x{code:08x}"),
            "win32_code": (code & 0xffff_0000 == 0x8007_0000).then_some(code & 0xffff),
            "message": error.message().chars().take(512).collect::<String>(),
        })
    }
    fn token_query<T: Default>(
        h: HANDLE,
        class: TOKEN_INFORMATION_CLASS,
    ) -> windows::core::Result<T> {
        let mut value = T::default();
        let mut length = 0;
        unsafe {
            GetTokenInformation(
                h,
                class,
                Some((&mut value as *mut T).cast()),
                size_of::<T>() as u32,
                &mut length,
            )?;
        }
        if length as usize != size_of::<T>() {
            return Err(windows::core::Error::from_hresult(
                windows::core::HRESULT::from_win32(ERROR_BAD_LENGTH.0),
            ));
        }
        Ok(value)
    }
    fn token_user(h: HANDLE) -> windows::core::Result<String> {
        let mut needed = 0;
        if let Err(e) = unsafe { GetTokenInformation(h, TokenUser, None, 0, &mut needed) }
            && e.code() != windows::core::HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0)
        {
            return Err(e);
        }
        if needed < size_of::<TOKEN_USER>() as u32 || needed > 65_536 {
            return Err(windows::core::Error::from_hresult(
                windows::core::HRESULT::from_win32(ERROR_BAD_LENGTH.0),
            ));
        }
        let capacity = needed;
        let mut storage = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        unsafe {
            GetTokenInformation(
                h,
                TokenUser,
                Some(storage.as_mut_ptr().cast()),
                capacity,
                &mut needed,
            )?;
        }
        if needed > capacity || needed < size_of::<TOKEN_USER>() as u32 {
            return Err(windows::core::Error::from_hresult(
                windows::core::HRESULT::from_win32(ERROR_BAD_LENGTH.0),
            ));
        }
        let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
        let mut sid = PWSTR::null();
        unsafe {
            ConvertSidToStringSidW(user.User.Sid, &mut sid)?;
        }
        let _release = ReleaseGuard::new(HLOCAL(sid.0.cast()), |h| {
            let released = unsafe { LocalFree(Some(h)) }.is_invalid();
            if !released {
                resource_release_uncertain();
            }
            released
        });
        unsafe { Ok(sid.to_string()?) }
    }
    fn diagnostic_value<T>(
        result: windows::core::Result<T>,
        value: impl FnOnce(T) -> serde_json::Value,
    ) -> serde_json::Value {
        match result {
            Ok(v) => serde_json::json!({"status":"observed","value":value(v)}),
            Err(e) => serde_json::json!({"status":"unknown","error":native_error(&e)}),
        }
    }
    // Only on failure; read the effective token with TOKEN_QUERY, never enable
    // privileges or substitute the elevated setup/metadata process's identity.
    fn effective_token_diagnostic() -> serde_json::Value {
        let mut token = HANDLE::default();
        let source = match unsafe {
            OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, false, &mut token)
        } {
            Ok(()) => "thread",
            Err(e) if e.code() == windows::core::HRESULT::from_win32(ERROR_NO_TOKEN.0) => {
                if let Err(error) =
                    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
                {
                    return serde_json::json!({"status":"unknown","api":"OpenProcessToken","error":native_error(&error)});
                }
                "process_no_thread_token"
            }
            Err(error) => {
                return serde_json::json!({"status":"unknown","api":"OpenThreadToken","error":native_error(&error)});
            }
        };
        let token = handle(token);
        serde_json::json!({
            "status":"token_opened", "source":source,
            "process_id":unsafe { GetCurrentProcessId() }, "thread_id":unsafe { GetCurrentThreadId() },
            "requested_access":"TOKEN_QUERY", "privileges_modified":false,
            "user_sid":diagnostic_value(token_user(raw(&token)), |v| v.into()),
            "elevated":diagnostic_value(token_query::<TOKEN_ELEVATION>(raw(&token), TokenElevation), |v| (v.TokenIsElevated != 0).into()),
            "session_id":diagnostic_value(token_query::<u32>(raw(&token), TokenSessionId), |v| v.into()),
            "statistics":diagnostic_value(token_query::<TOKEN_STATISTICS>(raw(&token), TokenStatistics), |v| serde_json::json!({
                "authentication_id_high":v.AuthenticationId.HighPart,"authentication_id_low":v.AuthenticationId.LowPart,
                "token_type":v.TokenType.0,"impersonation_level":v.ImpersonationLevel.0,
            })),
            "sampling":"failure_time_readonly; not an ACL analysis or a new authority",
        })
    }
    fn pin_api<T>(
        request: &PinRequest,
        code: &str,
        api: &str,
        stage: &str,
        result: windows::core::Result<T>,
    ) -> Result<T> {
        result.map_err(|native| {
            let mut error = request.fault(code, api, stage,
                "Inspect this exact path, access mask and effective token; no access fallback or automatic replay.");
            let diagnostic = error.diagnostic.as_deref_mut().expect("pin diagnostic");
            // Capture the original HRESULT/message before further native queries.
            diagnostic["native_error"] = native_error(&native);
            diagnostic["effective_token"] = effective_token_diagnostic();
            error
        })
    }
    fn open_pin(request: &PinRequest) -> Result<Handle> {
        // Keep portable policy values tied to the real SDK aliases on Windows.
        const _: () = assert!(pin::DIRECTORY_ACCESS == FILE_TRAVERSE.0);
        const _: () = assert!(pin::FILE_ACCESS == FILE_READ_DATA.0);
        const _: () = assert!(pin::SHARE == FILE_SHARE_READ.0);
        const _: () = assert!(pin::DISPOSITION == OPEN_EXISTING.0);
        const _: () = assert!(
            pin::DIRECTORY_FLAGS == FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0
        );
        const _: () = assert!(
            pin::FILE_FLAGS == FILE_FLAG_SEQUENTIAL_SCAN.0 | FILE_FLAG_OPEN_REPARSE_POINT.0
        );
        checkpoint("pin_application_path")?;
        let code = if request.kind == PinKind::AncestorDirectory {
            "application_directory_pin_failed"
        } else {
            "application_file_pin_failed"
        };
        let name = wide(&request.native_path);
        let opened = unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                request.desired_access,
                FILE_SHARE_MODE(request.share_mode),
                None,
                FILE_CREATION_DISPOSITION(request.creation_disposition),
                FILE_FLAGS_AND_ATTRIBUTES(request.flags),
                None,
            )
        };
        Ok(handle(pin_api(
            request,
            code,
            "CreateFileW",
            "open",
            opened,
        )?))
    }
    fn file_stamp(h: HANDLE, request: &PinRequest, stage: &str) -> Result<PinStamp> {
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        pin_api(
            request,
            "application_file_identity_unavailable",
            "GetFileInformationByHandle",
            stage,
            unsafe { GetFileInformationByHandle(h, &mut info) },
        )?;
        Ok(PinStamp {
            volume: info.dwVolumeSerialNumber,
            index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
            bytes: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
            modified: ticks(info.ftLastWriteTime),
            created: ticks(info.ftCreationTime),
            attributes: info.dwFileAttributes,
        })
    }
    fn final_path(h: HANDLE, request: &PinRequest, stage: &str) -> Result<String> {
        let mut name = vec![0u16; MAX_PATH_UNITS + 5];
        let n = unsafe {
            GetFinalPathNameByHandleW(
                h,
                &mut name,
                GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
            )
        } as usize;
        if n == 0 {
            let native = windows::core::Error::from_thread();
            return pin_api(
                request,
                "application_file_path_unavailable",
                "GetFinalPathNameByHandleW",
                stage,
                Err(native),
            );
        }
        if n >= name.len() {
            return Err(request.fault(
                "application_file_path_unavailable",
                "GetFinalPathNameByHandleW",
                stage,
                "The handle's final path exceeded the bounded buffer; no dispatch.",
            ));
        }
        String::from_utf16(&name[..n]).map_err(|_| {
            request.fault(
                "application_file_path_unavailable",
                "GetFinalPathNameByHandleW",
                stage,
                "The native path contains non-Unicode data; no dispatch.",
            )
        })
    }
    struct PinnedDirectory {
        handle: Handle,
        request: PinRequest,
        stamp: PinStamp,
    }
    impl PinnedDirectory {
        fn open(request: &PinRequest) -> Result<Self> {
            let handle = open_pin(request)?;
            let stamp = file_stamp(raw(&handle), request, "initial_directory_identity")?;
            request.check(
                &stamp,
                &stamp,
                &final_path(raw(&handle), request, "initial_directory_path")?,
                "initial_directory",
            )?;
            Ok(Self {
                handle,
                request: request.clone(),
                stamp,
            })
        }
        fn check(&self, stage: &str) -> Result<()> {
            checkpoint("check_application_directory_pin")?;
            self.request.check(
                &self.stamp,
                &file_stamp(raw(&self.handle), &self.request, stage)?,
                &final_path(raw(&self.handle), &self.request, stage)?,
                stage,
            )
        }
    }
    struct PinnedFile {
        file: Handle,
        directories: Vec<PinnedDirectory>,
        request: PinRequest,
        path: String,
        sha256: String,
        stamp: PinStamp,
    }
    impl PinnedFile {
        fn open(path: &str, expected_sha: &str) -> Result<Self> {
            Self::open_plan(PinPlan::new(path)?, expected_sha)
        }
        // Only the separate observed-package adapter calls this after native
        // package provenance/root policy passes. Never enters the strong cache.
        fn open_packaged_image(path: &str, expected_sha: &str, install_root: &str) -> Result<Self> {
            Self::open_plan(
                PinPlan::within_registered_package_root(path, install_root)?,
                expected_sha,
            )
        }
        fn open_plan(plan: PinPlan, expected_sha: &str) -> Result<Self> {
            if expected_sha.len() != 64 || !expected_sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(fault(
                    "invalid_application_sha256",
                    "Provide exactly 64 hexadecimal SHA-256 characters.",
                ));
            }
            let _call = native_call("verify_application_file")?;
            let root = wide(&plan.file.path[..3]);
            // DRIVE_FIXED = 3. No mapped network drives or privilege changes.
            if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != 3 {
                return Err(plan.file.fault(
                    "unsupported_application_volume",
                    "GetDriveTypeW",
                    "volume",
                    "Only a local fixed drive is supported for a pinned lifecycle launch.",
                ));
            }
            // Traverse/data access participates in sharing checks, unlike attrs-only
            // opens. Retain READ-only share on every child to deny write/delete.
            // Sharing is NOT a metadata write lock: also revalidate each ancestor
            // and reject reparse/type/identity/path changes. No global atomicity claim.
            let directories = plan.open_directories(PinnedDirectory::open)?;
            let request = plan.file;
            let file = open_pin(&request)?;
            let stamp = file_stamp(raw(&file), &request, "initial_file_identity")?;
            // Distinguish GetFileType failure from a successful non-disk type or
            // the independent size guard; never manufacture AccessDenied.
            unsafe {
                SetLastError(ERROR_SUCCESS);
            }
            let file_type = unsafe { GetFileType(raw(&file)) };
            if file_type == FILE_TYPE_UNKNOWN {
                let last = unsafe { GetLastError() };
                if last != ERROR_SUCCESS {
                    let native = windows::core::Error::from_hresult(
                        windows::core::HRESULT::from_win32(last.0),
                    );
                    pin_api::<()>(
                        &request,
                        "application_file_type_unavailable",
                        "GetFileType",
                        "initial_file_type",
                        Err(native),
                    )?;
                }
            }
            if file_type != FILE_TYPE_DISK {
                return Err(request.fault(
                    "application_file_not_verifiable",
                    "GetFileType",
                    "initial_file_type",
                    "Require a regular non-reparse disk file; no dispatch.",
                ));
            }
            if stamp.bytes > MAX_FILE_BYTES {
                return Err(request.fault(
                    "application_file_not_verifiable",
                    "file_size_bound",
                    "initial_file_size",
                    "The pinned file exceeds the 512 MiB SHA-256 bound; no dispatch.",
                ));
            }
            let mut pinned = Self {
                file,
                directories,
                path: request.path.clone(),
                request,
                sha256: String::new(),
                stamp,
            };
            pinned.check("before_hash")?;
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 65_536];
            let mut total = 0u64;
            loop {
                checkpoint("hash_application_file")?;
                let mut read = 0;
                pin_api(
                    &pinned.request,
                    "application_file_read_failed",
                    "ReadFile",
                    "hash",
                    unsafe {
                        ReadFile(raw(&pinned.file), Some(&mut buffer), Some(&mut read), None)
                    },
                )?;
                if read == 0 {
                    break;
                }
                total += u64::from(read);
                if total > MAX_FILE_BYTES || total > pinned.stamp.bytes {
                    return Err(pinned.request.fault(
                        "application_file_changed",
                        "ReadFile",
                        "hash",
                        "File size changed or exceeded the bounded SHA-256 read.",
                    ));
                }
                hasher.update(&buffer[..read as usize]);
            }
            pinned.check("after_hash")?;
            let sha256 = format!("{:x}", hasher.finalize());
            if total != pinned.stamp.bytes || !sha256.eq_ignore_ascii_case(expected_sha) {
                return Err(pinned.request.fault(
                    "application_file_hash_mismatch",
                    "SHA256",
                    "hash_complete",
                    "The pinned file content/identity does not match the authorized SHA-256.",
                ));
            }
            pinned.sha256 = sha256;
            Ok(pinned)
        }
        fn check(&self, stage: &str) -> Result<()> {
            let _call = native_call("check_application_pin_chain")?;
            for directory in &self.directories {
                directory.check(stage)?;
            }
            self.request.check(
                &self.stamp,
                &file_stamp(raw(&self.file), &self.request, stage)?,
                &final_path(raw(&self.file), &self.request, stage)?,
                stage,
            )
        }
        fn close(&mut self) -> bool {
            let mut released = self.file.close();
            for dir in &mut self.directories {
                released &= dir.handle.close();
            }
            released
        }
    }

    mod events {
        include!("application_events.rs");
    }
    use events::EventSession;

    use pin::window_key;
    fn process_birth(process: HANDLE) -> Result<u64> {
        let (mut created, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        desktop::api("application_process_epoch_unavailable", unsafe {
            GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user)
        })?;
        Ok(ticks(created))
    }
    fn process_image(pid: u32, expected_birth: u64) -> Result<String> {
        let process = handle(desktop::api("application_process_unreadable", unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
        })?);
        if process_birth(raw(&process))? != expected_birth {
            return Err(fault(
                "application_process_replaced",
                "PID creation time changed during inventory.",
            ));
        }
        let mut name = vec![0u16; MAX_PATH_UNITS + 5];
        let mut n = name.len() as u32;
        desktop::api("application_image_path_unavailable", unsafe {
            QueryFullProcessImageNameW(
                raw(&process),
                PROCESS_NAME_WIN32,
                PWSTR(name.as_mut_ptr()),
                &mut n,
            )
        })?;
        if n == 0 || n as usize >= name.len() {
            return Err(fault(
                "application_image_path_unavailable",
                "Process image path exceeded the bounded buffer.",
            ));
        }
        disk_path(&String::from_utf16(&name[..n as usize]).map_err(|_| {
            fault(
                "application_image_path_unavailable",
                "Process image path is not Unicode.",
            )
        })?)
    }

    /// Must be created/used/dropped on one dedicated native worker thread.
    /// Rc makes this !Send/!Sync; callbacks own TLS data, never a driver pointer.
    pub struct WindowsLifecycleDriver {
        thread: u32,
        pins: Vec<PinnedFile>,
        observed_specs: Vec<ObservedPackage>,
        observed_proofs: Vec<ObservedProof>,
        event_session: Option<EventSession>,
        last_events: Option<EventBatch>,
        launch_attempted: bool,
        release_failed: bool,
        bound_identity: Identity,
        activation_attempted: bool,
        _thread_affinity: Rc<()>,
    }
    impl WindowsLifecycleDriver {
        pub fn new() -> Result<Self> {
            Ok(Self {
                thread: unsafe { GetCurrentThreadId() },
                pins: vec![],
                observed_specs: vec![],
                observed_proofs: vec![],
                event_session: None,
                last_events: None,
                launch_attempted: false,
                release_failed: false,
                bound_identity: desktop::identity()?,
                activation_attempted: false,
                _thread_affinity: Rc::new(()),
            })
        }
        fn thread(&self) -> Result<()> {
            if unsafe { GetCurrentThreadId() } != self.thread {
                return Err(fault(
                    "lifecycle_wrong_thread",
                    "Lifecycle resources must remain on their owning native thread.",
                ));
            }
            Ok(())
        }
        fn verify(&mut self, path: &str, sha256: &str) -> Result<()> {
            self.thread()?;
            let canonical = disk_path(path)?;
            if let Some(pin) = self
                .pins
                .iter()
                .find(|pin| pin.path.eq_ignore_ascii_case(&canonical))
            {
                if !pin.sha256.eq_ignore_ascii_case(sha256) {
                    return Err(fault(
                        "application_file_hash_mismatch",
                        "The requested SHA-256 differs from this operation's pinned file.",
                    ));
                }
                return pin.check("reuse");
            }
            if self.pins.len() >= MAX_PINNED_FILES {
                return Err(fault(
                    "application_pin_limit",
                    "At most 64 files may be pinned by one lifecycle operation.",
                ));
            }
            self.pins.push(PinnedFile::open(&canonical, sha256)?);
            Ok(())
        }
        fn prove_packaged_window(
            &mut self,
            package: &ObservedPackage,
            key: &WindowIdentity,
        ) -> Result<()> {
            if let Some(proof) = self
                .observed_proofs
                .iter()
                .find(|proof| proof.matches(package, key))
            {
                return proof.check();
            }
            if self.observed_proofs.len() >= MAX_PINNED_FILES {
                return Err(fault(
                    "observed_package_pin_limit",
                    "At most 64 separate process/package proofs per operation.",
                ));
            }
            self.observed_proofs
                .push(ObservedProof::open(package, key)?);
            Ok(())
        }
        fn release_files(&mut self) -> bool {
            let mut released = true;
            for pin in &mut self.pins {
                released &= pin.close();
            }
            self.pins.clear();
            for proof in &mut self.observed_proofs {
                released &= proof.close();
            }
            self.observed_proofs.clear();
            self.release_failed |= !released;
            !self.release_failed
        }
    }
    impl Drop for WindowsLifecycleDriver {
        fn drop(&mut self) {
            self.event_session.take();
            self.release_files();
        }
    }
    impl LifecycleDriver for WindowsLifecycleDriver {
        fn identity(&mut self) -> Result<Identity> {
            self.thread()?;
            desktop::identity()
        }
        fn verify_application(&mut self, app: &ApplicationIdentity) -> Result<()> {
            self.verify(&app.executable, &app.sha256).map_err(|e| {
                pin_failure_context(e, "native_verify", "application", &app.executable)
            })
        }
        fn verify_file(&mut self, file: &AllowedFile) -> Result<()> {
            self.verify(&file.path, &file.sha256)
                .map_err(|e| pin_failure_context(e, "native_verify", "allowed_file", &file.path))
        }
        fn verify_observed_package(
            &mut self,
            package: &ObservedPackage,
            allocated: &[WindowIdentity],
        ) -> Result<()> {
            self.thread()?;
            if allocated.is_empty() || allocated.len() > MAX_WINDOWS {
                return Err(fault(
                    "existing_packaged_window_required",
                    "Allocate an exact already-running window before this read-only proof.",
                ));
            }
            let expected_image = disk_path(&package.application.executable)?;
            let mut matched = false;
            for key in allocated {
                self.check_window(key)?;
                if process_image(key.pid, key.process_created)?
                    .eq_ignore_ascii_case(&expected_image)
                {
                    self.prove_packaged_window(package, key)?;
                    self.check_window(key)?;
                    matched = true;
                }
            }
            if !matched {
                return Err(fault(
                    "existing_packaged_window_required",
                    "No allocated live window proves this packaged image; no path-launch fallback.",
                ));
            }
            if !self.observed_specs.contains(package) {
                if self.observed_specs.len() >= 8 {
                    return Err(fault(
                        "observed_package_limit",
                        "At most eight explicit package roles.",
                    ));
                }
                self.observed_specs.push(package.clone());
            }
            Ok(())
        }
        fn inventory(&mut self, apps: &[ApplicationIdentity]) -> Result<Inventory> {
            self.thread()?;
            if apps.is_empty() || apps.len() > 64 {
                return Err(fault(
                    "invalid_application_inventory",
                    "Inventory requires 1..64 explicit applications.",
                ));
            }
            let paths = apps
                .iter()
                .map(|app| disk_path(&app.executable))
                .collect::<Result<Vec<_>>>()?;
            let identity = desktop::identity()?;
            struct List {
                handles: Vec<HWND>,
                seen: usize,
                complete: bool,
            }
            unsafe extern "system" fn enumerate(hwnd: HWND, parameter: LPARAM) -> BOOL {
                let list = unsafe { &mut *(parameter.0 as *mut List) };
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    list.seen += 1;
                    if list.handles.len() == MAX_ENUMERATED_WINDOWS {
                        list.complete = false;
                        return BOOL(0);
                    }
                    list.handles.push(hwnd);
                    BOOL(1)
                }))
                .unwrap_or_else(|_| {
                    list.complete = false;
                    BOOL(0)
                })
            }
            let mut list = List {
                handles: Vec::with_capacity(MAX_ENUMERATED_WINDOWS),
                seen: 0,
                complete: true,
            };
            let enumeration_ok = {
                let _call = native_call("EnumDesktopWindows")?;
                let desk = desktop::api("lifecycle_desktop_unavailable", unsafe {
                    GetThreadDesktop(self.thread)
                })?;
                // Borrowed thread desktop handle: CloseDesktop must not be called.
                unsafe {
                    EnumDesktopWindows(
                        Some(desk),
                        Some(enumerate),
                        LPARAM((&mut list as *mut List) as isize),
                    )
                }
                .is_ok()
            };
            let mut result = Inventory {
                identity,
                windows: vec![],
                complete: list.complete && enumeration_ok,
                native_windows_enumerated: list.seen,
                captured_unix_ms: 0,
                notes: vec![],
            };
            let mut unreadable = 0;
            let mut failures = std::collections::BTreeMap::<String, usize>::new();
            let mut samples = Vec::new();
            let mut unmatched = 0;
            let mut rejected = 0;
            for (z, hwnd) in list.handles.into_iter().enumerate() {
                checkpoint("lifecycle_window_inventory")?;
                let item = (|| -> Result<Option<WindowEvidence>> {
                    let record = desktop::record(hwnd, z)?;
                    let key = window_key(&record)?;
                    let image = process_image(key.pid, key.process_created)?;
                    let Some(index) = paths
                        .iter()
                        .position(|path| path.eq_ignore_ascii_case(&image))
                    else {
                        return Ok(None);
                    };
                    if let Some(package) = self
                        .observed_specs
                        .iter()
                        .find(|p| p.application == apps[index])
                        .cloned()
                    {
                        self.prove_packaged_window(&package, &key).map_err(|e| {
                            pin_failure_context(
                                e,
                                "inventory",
                                "observed_package",
                                &package.application.executable,
                            )
                        })?;
                    } else {
                        self.verify_application(&apps[index]).map_err(|e| {
                            pin_failure_context(
                                e,
                                "inventory",
                                "matching_process_image",
                                &apps[index].executable,
                            )
                        })?;
                    }
                    // Observe again after image/path hashing; a vanished/reused
                    // HWND or changed process epoch is not an authorized window.
                    let after = desktop::record(hwnd, z)?;
                    if window_key(&after)? != key
                        || process_image(key.pid, key.process_created)? != image
                    {
                        return Err(fault(
                            "application_window_changed",
                            "Window identity changed while proving application ownership.",
                        ));
                    }
                    if after.title_truncated || after.class.encode_utf16().count() >= 255 {
                        result.complete = false;
                        rejected += 1;
                    }
                    Ok(Some(WindowEvidence {
                        key,
                        application: apps[index].clone(),
                        candidate_facts: Some(pin::candidate_facts(&after)),
                        title: after.title,
                        foreground: after.foreground,
                    }))
                })();
                match item {
                    Ok(Some(window)) if result.windows.len() < MAX_WINDOWS => {
                        result.windows.push(window)
                    }
                    Ok(Some(_)) => {
                        result.complete = false;
                        rejected += 1;
                    }
                    Ok(None) => unmatched += 1,
                    Err(error) => {
                        result.complete = false;
                        unreadable += 1;
                        *failures.entry(error.code.clone()).or_default() += 1;
                        if samples.len() < 8 {
                            if error.diagnostic.as_ref().is_some_and(|d| {
                                d["verification_context"]["role"] == "observed_package"
                            }) {
                                samples.push(format!(
                                    "hwnd:{:x}: {}",
                                    hwnd.0 as usize,
                                    serde_json::to_string(&error).expect("bounded package fault")
                                ));
                            } else {
                                samples.push(format!("hwnd:{:x}: {}", hwnd.0 as usize, error.code));
                            }
                        }
                    }
                }
            }
            if !list.complete || !enumeration_ok {
                result.notes.push("Native desktop enumeration was incomplete or exceeded 512 HWND callbacks; the observed count is not a total.".into());
            }
            if unreadable != 0 {
                result.notes.push(format!("{unreadable} HWNDs had unprovable metadata, process image, or file identity; application coverage is incomplete."));
            }
            if !failures.is_empty() {
                result.notes.push(format!(
                    "coverage_failures_by_code={}",
                    serde_json::to_string(&failures).expect("bounded coverage counts")
                ));
                result.notes.extend(samples);
            }
            if rejected != 0 {
                result.notes.push(format!("{rejected} authorized HWNDs exceeded window or metadata limits; application coverage is incomplete."));
            }
            result.notes.push(format!("{unmatched} readable nonmatching application paths were excluded. Matching uses process birth plus image path and pinned disk SHA-256, not title or PID alone."));
            if desktop::identity()? != result.identity {
                result.complete = false;
                result
                    .notes
                    .push("Native desktop identity changed during enumeration.".into());
            }
            result
                .notes
                .extend(self.observed_proofs.iter().map(ObservedProof::note));
            result.captured_unix_ms = unix_ms();
            while serde_json::to_vec(&result)
                .map_err(|_| {
                    fault(
                        "inventory_encoding_failed",
                        "Inventory could not be encoded.",
                    )
                })?
                .len()
                > MAX_METADATA_BYTES
            {
                result.complete = false;
                if result.windows.pop().is_none() {
                    return Err(fault(
                        "inventory_metadata_limit",
                        "Inventory metadata exceeds 65536 bytes.",
                    ));
                }
                if !result
                    .notes
                    .iter()
                    .any(|n| n == "Inventory metadata was truncated at 65536 bytes.")
                {
                    result
                        .notes
                        .push("Inventory metadata was truncated at 65536 bytes.".into());
                }
            }
            Ok(result)
        }
        fn begin_events(&mut self) -> Result<()> {
            self.thread()?;
            if self.event_session.is_some() {
                return Err(fault(
                    "lifecycle_events_active",
                    "End the existing event collection first.",
                ));
            }
            self.last_events = None;
            self.event_session = Some(EventSession::begin()?);
            Ok(())
        }
        fn events(&mut self) -> Result<EventBatch> {
            self.thread()?;
            let session = self.event_session.as_ref().ok_or_else(|| {
                fault(
                    "lifecycle_events_inactive",
                    "Begin native event collection first.",
                )
            })?;
            session.pump(false)?;
            Ok(session.batch(false))
        }
        fn end_events(&mut self) -> Result<EventBatch> {
            self.thread()?;
            // Verification may fail before CreateProcessW. Explicit end cleanup,
            // not a later Drop, releases those pins before a receipt is issued.
            let files_released = self.release_files();
            self.observed_specs.clear(); // Only operation end; post-launch inventory still needs role routing.
            let Some(mut session) = self.event_session.take() else {
                let mut batch = self.last_events.clone().unwrap_or(EventBatch {
                    events: vec![],
                    complete: false,
                    hooks_released: true,
                });
                batch.hooks_released &= files_released;
                return Ok(batch);
            };
            if session.pump(true).is_err() {
                session.incomplete();
            }
            let released = session.close() & files_released;
            let batch = session.batch(released);
            self.last_events = Some(batch.clone());
            Ok(batch)
        }
        fn launch(&mut self, permission: &LaunchPermission) -> Result<NativeLaunch> {
            self.thread()?;
            if self.launch_attempted {
                return Err(fault(
                    "lifecycle_launch_already_attempted",
                    "This driver never repeats CreateProcessW.",
                ));
            }
            let mut command =
                command_line(&permission.application.executable, &permission.arguments)?;
            self.verify_application(&permission.application)
                .map_err(|e| {
                    pin_failure_context(
                        e,
                        "native_launch_recheck",
                        "selected_launcher",
                        &permission.application.executable,
                    )
                })?;
            for file in &permission.allowed_files {
                self.verify_file(file)?;
            }
            for pin in &self.pins {
                pin.check("before_dispatch").map_err(|e| {
                    pin_failure_context(e, "before_dispatch", "cached_pin", &pin.path)
                })?;
            }
            for proof in &self.observed_proofs {
                proof.check()?;
            }
            let identity = desktop::identity()?;
            if identity != self.bound_identity {
                return Err(fault(
                    "session_mismatch",
                    "Native desktop changed before CreateProcessW; no dispatch.",
                ));
            }
            let mut desktop_name = wide(&format!("{}\\{}", identity.seat, identity.desktop));
            let executable = wide(&permission.application.executable);
            let startup = STARTUPINFOW {
                cb: size_of::<STARTUPINFOW>() as u32,
                lpDesktop: PWSTR(desktop_name.as_mut_ptr()),
                ..Default::default()
            };
            let mut information = PROCESS_INFORMATION::default();
            let call = native_call("CreateProcessW_once")?;
            self.launch_attempted = true;
            let created = desktop::api("application_create_process_failed", unsafe {
                CreateProcessW(
                    PCWSTR(executable.as_ptr()),
                    Some(PWSTR(command.as_mut_ptr())),
                    None,
                    None,
                    false,
                    PROCESS_CREATION_FLAGS(0),
                    None,
                    PCWSTR::null(),
                    &startup,
                    &mut information,
                )
            });
            drop(call);
            let mut process = handle(information.hProcess);
            let mut thread = handle(information.hThread);
            let accepted = created.is_ok();
            let birth = if accepted {
                process_birth(raw(&process)).map(Some)
            } else {
                Ok(None)
            };
            let process_handles_released = process.close() & thread.close() & self.release_files();
            Ok(NativeLaunch {
                api: "CreateProcessW".into(),
                accepted,
                process_id: (accepted && information.dwProcessId != 0)
                    .then_some(information.dwProcessId),
                process_created: birth.as_ref().ok().copied().flatten(),
                process_handles_released,
                error: created.err().or_else(|| birth.err()),
            })
        }
        fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
            self.thread()?;
            let _call = native_call("lifecycle_foreground")?;
            let window = unsafe { GetForegroundWindow() };
            if window.0.is_null() {
                return Ok(None);
            }
            let key = window_key(&desktop::record(window, 0)?)?;
            if unsafe { GetForegroundWindow() } != window {
                return Err(fault(
                    "foreground_changed",
                    "Foreground changed while reading its exact identity.",
                ));
            }
            Ok(Some(key))
        }
        fn user_input_tick(&mut self) -> Result<u64> {
            self.thread()?;
            let _call = native_call("GetLastInputInfo")?;
            let mut input = LASTINPUTINFO {
                cbSize: size_of::<LASTINPUTINFO>() as u32,
                ..Default::default()
            };
            if !unsafe { GetLastInputInfo(&mut input) }.as_bool() {
                return Err(fault(
                    "user_input_state_unavailable",
                    "Last-input observation failed; yield rather than activate.",
                ));
            }
            // This is a session-relative wrapping DWORD token, not wall time.
            Ok(u64::from(input.dwTime))
        }
        fn check_window(&mut self, window: &WindowIdentity) -> Result<()> {
            self.thread()?;
            let _call = native_call("lifecycle_check_window")?;
            if window_key(&desktop::record(desktop::hwnd(&window.window)?, 0)?)? != *window {
                return Err(fault(
                    "application_window_identity_changed",
                    "HWND, PID, process creation time or native epoch no longer matches.",
                ));
            }
            Ok(())
        }
        fn activate(&mut self, window: &WindowIdentity) -> Result<bool> {
            if self.activation_attempted {
                return Err(fault(
                    "lifecycle_activation_already_attempted",
                    "This driver never repeats SetForegroundWindow.",
                ));
            }
            self.check_window(window)?;
            if desktop::identity()? != self.bound_identity {
                return Err(fault(
                    "session_mismatch",
                    "Native desktop changed before SetForegroundWindow; yield.",
                ));
            }
            let hwnd = desktop::hwnd(&window.window)?;
            let _call = native_call("SetForegroundWindow_once")?;
            self.activation_attempted = true;
            Ok(unsafe { SetForegroundWindow(hwnd) }.as_bool())
        }
        fn pause(&mut self, milliseconds: u64) {
            // Short bounded sleeps; the controller owns the overall wait budget.
            std::thread::sleep(Duration::from_millis(milliseconds.min(100)));
        }
    }
}
#[cfg(windows)]
pub use win::WindowsLifecycleDriver;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn event_overflow_stays_incomplete_and_preserves_prior_events() {
        let mut events = BoundedEvents::new();
        for i in 0..rcu_core::lifecycle::MAX_EVENTS {
            events.push(i);
        }
        assert!(events.complete);
        events.push(999);
        events.push(1_000);
        assert!(!events.complete);
        assert_eq!(events.events.len(), rcu_core::lifecycle::MAX_EVENTS);
        assert_eq!(events.events[0], 0);
        assert_eq!(
            events.events.last(),
            Some(&(rcu_core::lifecycle::MAX_EVENTS - 1))
        );
    }

    #[test]
    fn path_contract_rejects_ambiguous_or_indirect_names() {
        for bad in [
            "app.exe",
            r"C:app.exe",
            r"\app.exe",
            r"\\server\share\app.exe",
            r"\\.\C:\app.exe",
            r"C:\app.exe:stream",
            r"C:\foo\..\app.exe",
            r"C:\foo\.\app.exe",
            "C:\\app.exe ",
            "C:\\app.exe.",
            r"C:\NUL.txt",
            r"C:\COM1\x.exe",
            r"C:\foo\\x.exe",
            r"C:/x.exe",
            "C:\\x\0.exe",
            r"C:\foo\",
        ] {
            assert!(disk_path(bad).is_err(), "accepted {bad:?}");
        }
        assert_eq!(
            disk_path(r"\\?\C:\Program Files\应用\app.exe").unwrap(),
            r"C:\Program Files\应用\app.exe"
        );
        assert!(disk_path(&format!("C:\\{}", "x".repeat(MAX_PATH_UNITS))).is_err());
        assert!(disk_path(&format!("C:\\{}x.exe", "a\\".repeat(MAX_PATH_COMPONENTS))).is_err());
    }
    #[test]
    fn quoting_empty_spaces_quotes_backslashes_and_unicode() {
        assert_eq!(quote_arg("").unwrap(), "\"\"");
        assert_eq!(quote_arg("a b").unwrap(), "\"a b\"");
        assert_eq!(quote_arg("a\"b").unwrap(), "\"a\\\"b\"");
        assert_eq!(quote_arg("a\\\"b").unwrap(), "\"a\\\\\\\"b\"");
        assert_eq!(quote_arg("tail\\").unwrap(), "\"tail\\\\\"");
        assert_eq!(quote_arg("你好	🦀").unwrap(), "\"你好	🦀\"");
        assert!(quote_arg("bad\0arg").is_err());
    }
    #[test]
    fn command_line_has_explicit_argv_zero_and_bounded_utf16() {
        let line = command_line(
            r"C:\Program Files\app.exe",
            &["".into(), "tail\\".into(), "x&y".into()],
        )
        .unwrap();
        assert_eq!(line.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&line[..line.len() - 1]).unwrap(),
            "\"C:\\Program Files\\app.exe\" \"\" \"tail\\\\\" \"x&y\""
        );
        assert!(command_line(r"C:\a.exe", &["🦀".repeat(MAX_COMMAND_UNITS / 2)]).is_err());
        assert!(command_line(r"C:\a.exe", &vec!["a".into(); 65]).is_err());
    }
    #[test]
    fn release_guard_closes_once_on_explicit_close_and_drop() {
        let calls = Rc::new(Cell::new(0));
        {
            let count = calls.clone();
            let mut guard = ReleaseGuard::new(7, move |h| {
                assert_eq!(h, 7);
                count.set(count.get() + 1);
                true
            });
            assert!(guard.close());
            assert!(guard.close());
        }
        assert_eq!(calls.get(), 1);
        {
            let count = calls.clone();
            let _guard = ReleaseGuard::new(8, move |_| {
                count.set(count.get() + 1);
                true
            });
        }
        assert_eq!(calls.get(), 2);
    }
    #[test]
    fn release_failure_stays_uncertain_and_unwind_cleans_up() {
        let calls = Rc::new(Cell::new(0));
        {
            let count = calls.clone();
            let mut guard = ReleaseGuard::new(9, move |_| {
                count.set(count.get() + 1);
                false
            });
            assert!(!guard.close());
            assert!(!guard.close());
        }
        assert_eq!(calls.get(), 1);
        let count = calls.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ReleaseGuard::new(10, move |_| {
                count.set(count.get() + 1);
                true
            });
            panic!("offline cleanup test");
        }));
        assert_eq!(calls.get(), 2);
    }
}
