// Included only in application::win::package. No subprocess or native test.
use super::super::package_policy as policy;
use super::*;
use rwi_core::Fault;
use windows::Win32::Storage::Packaging::Appx::*;

// Local xwin Win11SDK 10.0.26100 appmodel.h brackets PACKAGE_VERSION,
// PACKAGE_ID and PACKAGE_INFO with pshpack4.h/poppack.h. windows 0.62.2
// represents the 64-bit records with packed(4), including the version union.
// Check actual binding layouts, not a parallel mock struct. These assertions
// cannot reconstruct an uncaptured native buffer or establish flag semantics.
#[cfg(all(
    target_pointer_width = "64",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]
const _: () = {
    use std::mem::{align_of, offset_of, size_of};

    assert!(size_of::<u32>() == 4);
    assert!(size_of::<PWSTR>() == 8);
    assert!(align_of::<PWSTR>() == 8);

    assert!(size_of::<PACKAGE_VERSION>() == 8);
    assert!(align_of::<PACKAGE_VERSION>() == 4);
    assert!(offset_of!(PACKAGE_VERSION, Anonymous) == 0);
    assert!(size_of::<PACKAGE_VERSION_0>() == 8);
    assert!(align_of::<PACKAGE_VERSION_0>() == 4);
    assert!(offset_of!(PACKAGE_VERSION_0, Version) == 0);
    assert!(offset_of!(PACKAGE_VERSION_0, Anonymous) == 0);
    assert!(offset_of!(PACKAGE_VERSION_0_0, Revision) == 0);
    assert!(offset_of!(PACKAGE_VERSION_0_0, Build) == 2);
    assert!(offset_of!(PACKAGE_VERSION_0_0, Minor) == 4);
    assert!(offset_of!(PACKAGE_VERSION_0_0, Major) == 6);

    assert!(size_of::<PACKAGE_ID>() == 48);
    assert!(align_of::<PACKAGE_ID>() == 4);
    assert!(offset_of!(PACKAGE_ID, reserved) == 0);
    assert!(offset_of!(PACKAGE_ID, processorArchitecture) == 4);
    assert!(offset_of!(PACKAGE_ID, version) == 8);
    assert!(offset_of!(PACKAGE_ID, name) == 16);
    assert!(offset_of!(PACKAGE_ID, publisher) == 24);
    assert!(offset_of!(PACKAGE_ID, resourceId) == 32);
    assert!(offset_of!(PACKAGE_ID, publisherId) == 40);

    assert!(size_of::<PACKAGE_INFO>() == 80);
    assert!(align_of::<PACKAGE_INFO>() == 4);
    assert!(offset_of!(PACKAGE_INFO, reserved) == 0);
    assert!(offset_of!(PACKAGE_INFO, flags) == 4);
    assert!(offset_of!(PACKAGE_INFO, path) == 8);
    assert!(offset_of!(PACKAGE_INFO, packageFullName) == 16);
    assert!(offset_of!(PACKAGE_INFO, packageFamilyName) == 24);
    assert!(offset_of!(PACKAGE_INFO, packageId) == 32);
    assert!(offset_of!(PACKAGE_INFO, packageId.name) == 48);
    assert!(offset_of!(PACKAGE_INFO, packageId.publisher) == 56);
    assert!(offset_of!(PACKAGE_INFO, packageId.resourceId) == 64);
    assert!(offset_of!(PACKAGE_INFO, packageId.publisherId) == 72);
};

const _: () = assert!(policy::DEVELOPMENT_MODE == PACKAGE_PROPERTY_DEVELOPMENT_MODE);
const _: () = assert!(policy::ORIGIN_INBOX == PackageOrigin_Inbox.0);
const _: () = assert!(policy::ORIGIN_STORE == PackageOrigin_Store.0);
const _: () = assert!(
    policy::KNOWN_PROPERTIES
        == PACKAGE_PROPERTY_FRAMEWORK
            | PACKAGE_PROPERTY_RESOURCE
            | PACKAGE_PROPERTY_BUNDLE
            | PACKAGE_PROPERTY_OPTIONAL
            | PACKAGE_PROPERTY_DEVELOPMENT_MODE
            | PACKAGE_PROPERTY_IS_IN_RELATED_SET
            | PACKAGE_PROPERTY_STATIC
            | PACKAGE_PROPERTY_DYNAMIC
            | PACKAGE_PROPERTY_HOSTRUNTIME
);

struct Context<'a> {
    spec: &'a ObservedPackage,
    key: &'a WindowIdentity,
    stage: &'static str,
    queries: RefCell<policy::QueryTrace>,
}
impl Context<'_> {
    fn decorate(&self, error: Fault, api: &str) -> Fault {
        policy::decorate(
            error,
            self.spec,
            self.key,
            self.stage,
            api,
            &self.queries.borrow(),
        )
    }
    fn observed(&self, api: &str, status: &'static str) {
        self.queries.borrow_mut().record(api, status);
    }
    fn fault(&self, code: &str, api: &str, message: &str) -> Fault {
        self.decorate(fault(code, message), api)
    }
    fn policy<T>(&self, api: &str, value: Result<T>) -> Result<T> {
        value.map_err(|error| self.decorate(error, api))
    }
    fn api<T>(&self, api: &str, result: windows::core::Result<T>) -> Result<T> {
        self.observed(
            api,
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        result.map_err(|native| {
            let mut error = self.fault(
                "observed_package_query_failed", api,
                "Native package proof is unknown; inspect this exact API, stage, path and worker context. No weaker retry or dispatch.",
            );
            error.diagnostic.as_deref_mut().expect("proof diagnostic")["native_error"] = native_error(&native);
            error
        })
    }
    fn status(&self, api: &str, status: WIN32_ERROR) -> Result<()> {
        if status == ERROR_SUCCESS {
            self.observed(api, "succeeded");
            return Ok(());
        }
        let native =
            windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(status.0));
        let mut error = self.api::<()>(api, Err(native)).unwrap_err();
        // These APIs return LONG/Win32 directly, not GetLastError. Preserve all
        // 32 bits even if conversion to HRESULT would truncate them.
        error.diagnostic.as_deref_mut().expect("proof diagnostic")["native_error"]["win32_code"] =
            status.0.into();
        Err(error)
    }
}

// Store the original close error before any further API or diagnostic query.
// No retry after close uncertainty, including during unwinding.
struct OwnedHandle {
    value: Option<HANDLE>,
    close_error: Option<windows::core::Error>,
}
impl OwnedHandle {
    fn new(value: HANDLE) -> Self {
        Self {
            value: Some(value),
            close_error: None,
        }
    }
    fn raw(&self) -> HANDLE {
        self.value.unwrap_or_default()
    }
    fn close(&mut self) -> bool {
        if let Some(value) = self.value.take()
            && let Err(error) = unsafe { CloseHandle(value) }
        {
            self.close_error = Some(error);
            resource_release_uncertain();
        }
        self.close_error.is_none()
    }
    fn finish<T>(&mut self, context: &Context<'_>, result: Result<T>) -> Result<T> {
        self.close();
        let cleanup = self
            .close_error
            .clone()
            .and_then(|error| context.api::<()>("CloseHandle", Err(error)).err());
        with_cleanup(result, cleanup)
    }
}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        self.close();
    }
}

struct PackageReference {
    value: Option<*mut _PACKAGE_INFO_REFERENCE>,
    close_error: Option<WIN32_ERROR>,
}
impl PackageReference {
    fn close(&mut self) {
        if let Some(value) = self.value.take() {
            let status = unsafe { ClosePackageInfo(value) };
            if status != ERROR_SUCCESS {
                self.close_error = Some(status);
                resource_release_uncertain();
            }
        }
    }
    fn finish<T>(&mut self, context: &Context<'_>, result: Result<T>) -> Result<T> {
        self.close();
        let cleanup = self
            .close_error
            .and_then(|error| context.status("ClosePackageInfo", error).err());
        with_cleanup(result, cleanup)
    }
}
impl Drop for PackageReference {
    fn drop(&mut self) {
        self.close();
    }
}

fn with_cleanup<T>(result: Result<T>, cleanup: Option<Fault>) -> Result<T> {
    match (result, cleanup) {
        (Err(mut error), Some(cleanup)) => {
            let diagnostic = error
                .diagnostic
                .get_or_insert_with(|| Box::new(serde_json::json!({})));
            diagnostic["cleanup_error"] =
                serde_json::to_value(cleanup).expect("fault serialization");
            Err(error)
        }
        (Ok(_), Some(cleanup)) => Err(cleanup),
        (result, None) => result,
    }
}

fn query_string(
    context: &Context<'_>,
    api: &str,
    mut query: impl FnMut(*mut u32, Option<PWSTR>) -> WIN32_ERROR,
) -> Result<String> {
    checkpoint("observed_package_string_query")?;
    let mut length = 0;
    let status = query(&mut length, None);
    if status != ERROR_INSUFFICIENT_BUFFER {
        context.status(api, status)?;
        return Err(context.fault(
            "observed_package_buffer_invalid",
            api,
            "The size query did not return a nonempty bounded string buffer requirement.",
        ));
    }
    context.observed(api, "expected_insufficient_buffer");
    if !(2..=policy::MAX_STRING_UNITS + 1).contains(&(length as usize)) {
        return Err(context.fault(
            "observed_package_buffer_invalid",
            api,
            "Native string requirement exceeds 4096 UTF-16 units plus NUL or is empty.",
        ));
    }
    let mut buffer = vec![0u16; length as usize];
    let status = query(&mut length, Some(PWSTR(buffer.as_mut_ptr())));
    context.status(api, status)?;
    if length < 2 || length as usize > buffer.len() || buffer[length as usize - 1] != 0 {
        return Err(context.fault(
            "observed_package_buffer_invalid",
            api,
            "Native string length/terminator is inconsistent with the returned bounded buffer.",
        ));
    }
    context.policy(api, policy::utf16(&buffer[..length as usize - 1]))
}

fn read_token(context: &Context<'_>, token: HANDLE, source: &str) -> Result<policy::TokenIdentity> {
    let user_api = format!("GetTokenInformation(TokenUser, {source})");
    let api = user_api.as_str();
    let mut length = 0;
    match unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut length) } {
        Err(error)
            if error.code() == windows::core::HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) =>
        {
            context.observed(api, "expected_insufficient_buffer");
        }
        other => {
            context.api(api, other)?;
            return Err(context.fault(
                "observed_package_buffer_invalid",
                api,
                "Token user size query returned no complete SID buffer.",
            ));
        }
    }
    if length as usize > policy::MAX_BUFFER_BYTES || (length as usize) < size_of::<TOKEN_USER>() {
        return Err(context.fault(
            "observed_package_buffer_invalid",
            api,
            "Token user metadata exceeds 64 KiB or cannot contain TOKEN_USER.",
        ));
    }
    let capacity = length;
    let mut storage = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    context.api(api, unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            capacity,
            &mut length,
        )
    })?;
    if length > capacity || (length as usize) < size_of::<TOKEN_USER>() {
        return Err(context.fault(
            "observed_package_buffer_invalid",
            api,
            "Token metadata returned an inconsistent byte count.",
        ));
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), length as usize) };
    let user = unsafe { storage.as_ptr().cast::<TOKEN_USER>().read_unaligned() };
    let sid = context.policy(
        api,
        policy::buffer_sid(
            bytes,
            storage.as_ptr() as usize,
            user.User.Sid.0 as usize,
            size_of::<TOKEN_USER>(),
        ),
    )?;
    let session = context.api(
        &format!("GetTokenInformation(TokenSessionId, {source})"),
        token_query::<u32>(token, TokenSessionId),
    )?;
    let stats = context.api(
        &format!("GetTokenInformation(TokenStatistics, {source})"),
        token_query::<TOKEN_STATISTICS>(token, TokenStatistics),
    )?;
    Ok(policy::TokenIdentity {
        sid,
        session,
        authentication_id: ((stats.AuthenticationId.HighPart as u64) << 32)
            | u64::from(stats.AuthenticationId.LowPart),
        source: source.into(),
    })
}

fn process_token(context: &Context<'_>, process: HANDLE) -> Result<policy::TokenIdentity> {
    let mut token = HANDLE::default();
    context.api("OpenProcessToken(target, TOKEN_QUERY)", unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut token)
    })?;
    let mut token = OwnedHandle::new(token);
    let result = read_token(context, token.raw(), "target_process");
    token.finish(context, result)
}

fn effective_token(context: &Context<'_>) -> Result<policy::TokenIdentity> {
    let mut token = HANDLE::default();
    let source = match unsafe {
        OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, false, &mut token)
    } {
        Ok(()) => {
            context.observed("OpenThreadToken(worker, TOKEN_QUERY)", "succeeded");
            // OpenPackageInfoByFullName's current-user view must be the actual
            // worker process registration, not a thread impersonation identity.
            let mut token = OwnedHandle::new(token);
            let result = Err(context.fault("observed_package_worker_impersonating",
                "OpenThreadToken(worker, TOKEN_QUERY)",
                "A worker thread token is present; current-user package proof requires no impersonation."));
            return token.finish(context, result);
        }
        Err(error) if error.code() == windows::core::HRESULT::from_win32(ERROR_NO_TOKEN.0) => {
            context.observed(
                "OpenThreadToken(worker, TOKEN_QUERY)",
                "expected_no_thread_token",
            );
            context.api("OpenProcessToken(worker, TOKEN_QUERY)", unsafe {
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            })?;
            "worker_process_no_thread_token"
        }
        Err(error) => return context.api("OpenThreadToken(worker, TOKEN_QUERY)", Err(error)),
    };
    let mut token = OwnedHandle::new(token);
    let result = read_token(context, token.raw(), source);
    token.finish(context, result)
}

fn process_identity(context: &Context<'_>, process: HANDLE) -> Result<policy::ProcessIdentity> {
    checkpoint("observed_package_process_query")?;
    let pid = unsafe { GetProcessId(process) };
    if pid == 0 {
        return context.api("GetProcessId", Err(windows::core::Error::from_thread()));
    }
    context.observed("GetProcessId", "succeeded");
    let (mut created, mut exit, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    context.api("GetProcessTimes", unsafe {
        GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user)
    })?;
    let mut exit_code = 0;
    context.api("GetExitCodeProcess", unsafe {
        GetExitCodeProcess(process, &mut exit_code)
    })?;
    // GetProcessTimes exit-time contents are undefined while live. Do not turn
    // a guessed exit-time value into evidence. STILL_ACTIVE is coupled to a
    // contemporaneous live HWND/PID check, not claimed as an atomic lifetime lock.
    let live = exit_code == STILL_ACTIVE.0 as u32;
    let mut image = vec![0u16; policy::MAX_STRING_UNITS + 1];
    let mut length = image.len() as u32;
    context.api("QueryFullProcessImageNameW", unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(image.as_mut_ptr()),
            &mut length,
        )
    })?;
    if length == 0 || length as usize > policy::MAX_STRING_UNITS {
        return Err(context.fault(
            "observed_package_buffer_invalid",
            "QueryFullProcessImageNameW",
            "Process image did not fit the bounded native path buffer.",
        ));
    }
    let image = context.policy(
        "QueryFullProcessImageNameW",
        policy::utf16(&image[..length as usize]),
    )?;
    let image = context.policy("QueryFullProcessImageNameW", disk_path(&image))?;
    let full_name = query_string(context, "GetPackageFullName", |length, value| unsafe {
        GetPackageFullName(process, length, value)
    })?;
    let family = query_string(context, "GetPackageFamilyName", |length, value| unsafe {
        GetPackageFamilyName(process, length, value)
    })?;
    Ok(policy::ProcessIdentity {
        pid,
        birth: ticks(created),
        live,
        image,
        full_name,
        family,
        token: process_token(context, process)?,
    })
}

fn check_window(context: &Context<'_>, process: &policy::ProcessIdentity) -> Result<()> {
    let hwnd = context.policy("parse_allocated_HWND", desktop::hwnd(&context.key.window))?;
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        context.observed("IsWindow", "returned_false");
        return Err(context.fault(
            "observed_package_window_gone",
            "IsWindow",
            "The allocated HWND no longer exists; no package proof or dispatch.",
        ));
    }
    context.observed("IsWindow", "succeeded");
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if thread == 0 {
        return context.api(
            "GetWindowThreadProcessId",
            Err(windows::core::Error::from_thread()),
        );
    }
    context.observed("GetWindowThreadProcessId", "succeeded");
    if !process.live || pid != process.pid {
        return Err(context.fault(
            "observed_package_process_mismatch",
            "GetWindowThreadProcessId",
            "The live HWND does not belong to the retained process object.",
        ));
    }
    let mut class = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut class) };
    if length == 0 {
        return context.api("GetClassNameW", Err(windows::core::Error::from_thread()));
    }
    context.observed("GetClassNameW", "succeeded");
    if length < 0 || length as usize >= class.len() - 1 {
        return Err(context.fault(
            "observed_package_window_incomplete",
            "GetClassNameW",
            "Native class is incomplete at the existing desktop epoch bound.",
        ));
    }
    let class = context.policy("GetClassNameW", policy::utf16(&class[..length as usize]))?;
    let key = context.policy(
        "native_window_epoch",
        policy::window_identity(desktop::window_id(hwnd), pid, process.birth, &class),
    )?;
    context.policy("native_window_epoch", policy::unchanged(context.key, &key))
}

struct Head {
    count: u32,
    full_name: String,
    family: String,
    path: String,
    flags: u32,
}

fn package_head(context: &Context<'_>, name: PCWSTR) -> Result<Head> {
    let mut reference = std::ptr::null_mut();
    context.status("OpenPackageInfoByFullName", unsafe {
        OpenPackageInfoByFullName(name, None, &mut reference)
    })?;
    if reference.is_null() {
        return Err(context.fault("observed_package_metadata_incomplete", "OpenPackageInfoByFullName",
            "Successful package open returned a null reference; no current-user registration proof."));
    }
    let mut reference = PackageReference {
        value: Some(reference),
        close_error: None,
    };
    let result = (|| {
        let api = "GetPackageInfo(PACKAGE_FILTER_HEAD)";
        let mut length = 0;
        let mut count = 0;
        let raw = reference.value.expect("open package reference");
        let status = unsafe {
            GetPackageInfo(
                raw,
                PACKAGE_FILTER_HEAD,
                &mut length,
                None,
                Some(&mut count),
            )
        };
        if status != ERROR_INSUFFICIENT_BUFFER {
            context.status(api, status)?;
            return Err(context.fault(
                "observed_package_metadata_incomplete",
                api,
                "HEAD size query did not provide complete package metadata.",
            ));
        }
        context.observed(api, "expected_insufficient_buffer");
        if length as usize > policy::MAX_BUFFER_BYTES
            || (length as usize) < size_of::<PACKAGE_INFO>()
        {
            return Err(context.fault(
                "observed_package_buffer_invalid",
                api,
                "Package HEAD metadata exceeds 64 KiB or cannot contain PACKAGE_INFO.",
            ));
        }
        let capacity = length;
        let mut storage = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
        context.status(api, unsafe {
            GetPackageInfo(
                raw,
                PACKAGE_FILTER_HEAD,
                &mut length,
                Some(storage.as_mut_ptr().cast()),
                Some(&mut count),
            )
        })?;
        if length > capacity || (length as usize) < size_of::<PACKAGE_INFO>() || count != 1 {
            return Err(context.fault("observed_package_metadata_incomplete", api,
                "Require exactly one HEAD with a valid returned byte count; no truncated or dependency-only proof."));
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), length as usize) };
        // PACKAGE_INFO is packed(4) on Windows ARM64/x64; copy without taking
        // potentially misaligned field references. Never follow native PWSTRs.
        let head = unsafe { storage.as_ptr().cast::<PACKAGE_INFO>().read_unaligned() };
        let string = |value: PWSTR| {
            context.policy(
                api,
                policy::buffer_string(
                    bytes,
                    storage.as_ptr() as usize,
                    value.0 as usize,
                    size_of::<PACKAGE_INFO>(),
                ),
            )
        };
        let path = string(head.path)?;
        Ok(Head {
            count,
            full_name: string(head.packageFullName)?,
            family: string(head.packageFamilyName)?,
            path: context.policy(api, disk_path(&path))?,
            flags: head.flags,
        })
    })();
    reference.finish(context, result)
}

fn snapshot(context: &Context<'_>, process: HANDLE) -> Result<policy::PackageSnapshot> {
    checkpoint("observed_package_snapshot")?;
    let before = process_identity(context, process)?;
    check_window(context, &before)?;
    let worker = effective_token(context)?;
    context.policy(
        "target_and_effective_worker_TokenUser/TokenSessionId",
        policy::same_user(&before.token, &worker),
    )?;
    let native_name = wide(&context.spec.package_full_name);
    let name = PCWSTR(native_name.as_ptr());
    let head = package_head(context, name)?;
    let family_from_full_name = query_string(
        context,
        "PackageFamilyNameFromFullName",
        |length, value| unsafe { PackageFamilyNameFromFullName(name, length, value) },
    )?;
    let mut origin = PackageOrigin_Unknown;
    context.status("GetStagedPackageOrigin", unsafe {
        GetStagedPackageOrigin(name, &mut origin)
    })?;
    let install_path = query_string(
        context,
        "GetPackagePathByFullName2(Install)",
        |length, value| unsafe {
            GetPackagePathByFullName2(name, PackagePathType_Install, length, value)
        },
    )?;
    let effective_path = query_string(
        context,
        "GetPackagePathByFullName2(Effective)",
        |length, value| unsafe {
            GetPackagePathByFullName2(name, PackagePathType_Effective, length, value)
        },
    )?;
    // Do not query Mutable/External and reinterpret arbitrary errors as absence.
    // The supported predicate is only successful Install == Effective == root.
    let worker_after = effective_token(context)?;
    let after = process_identity(context, process)?;
    check_window(context, &after)?;
    context.policy(
        "process_identity_before_after",
        policy::unchanged(&before, &after),
    )?;
    context.policy(
        "effective_token_before_after",
        policy::unchanged(&worker, &worker_after),
    )?;
    let result = policy::PackageSnapshot {
        process: after,
        worker: worker_after,
        head_count: head.count,
        full_name: head.full_name,
        family: head.family,
        family_from_full_name,
        graph_path: head.path,
        install_path: context.policy(
            "GetPackagePathByFullName2(Install)",
            disk_path(&install_path),
        )?,
        effective_path: context.policy(
            "GetPackagePathByFullName2(Effective)",
            disk_path(&effective_path),
        )?,
        flags: head.flags,
        origin: origin.0,
    };
    context.policy(
        "native_package_snapshot_policy",
        policy::validate(context.spec, context.key, &result),
    )?;
    Ok(result)
}

pub(super) struct ObservedProof {
    spec: ObservedPackage,
    key: WindowIdentity,
    process: OwnedHandle,
    image: PinnedFile,
    snapshot: policy::PackageSnapshot,
}
impl ObservedProof {
    pub(super) fn open(spec: &ObservedPackage, key: &WindowIdentity) -> Result<Self> {
        let context = Context {
            spec,
            key,
            stage: "before_package_image_hash",
            queries: RefCell::default(),
        };
        let _call = context.policy("native_call", native_call("open_observed_package_proof"))?;
        // Validate input bounds before wide allocation or any package API.
        context.policy(
            "authorized_image_path",
            disk_path(&spec.application.executable),
        )?;
        context.policy("authorized_install_path", disk_path(&spec.install_path))?;
        if spec
            .package_full_name
            .encode_utf16()
            .take(policy::MAX_STRING_UNITS + 1)
            .count()
            > policy::MAX_STRING_UNITS
        {
            return Err(context.fault(
                "observed_package_buffer_invalid",
                "authorized_package_full_name",
                "The granted package name exceeds the bounded native string limit.",
            ));
        }
        context.policy(
            "authorized_package_full_name",
            policy::utf16(&spec.package_full_name.encode_utf16().collect::<Vec<_>>()),
        )?;
        let process = OwnedHandle::new(
            context.api("OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)", unsafe {
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, key.pid)
            })?,
        );
        let before = snapshot(&context, process.raw())?;
        let image = PinnedFile::open_packaged_image(
            &spec.application.executable,
            &spec.application.sha256,
            &spec.install_path,
        )
        .map_err(|error| {
            pin_failure_context(
                error,
                "observed_package_image_hash",
                "observed_package",
                &spec.application.executable,
            )
        })?;
        let context = Context {
            spec,
            key,
            stage: "after_package_image_hash",
            queries: RefCell::default(),
        };
        let after = snapshot(&context, process.raw())?;
        context.policy(
            "package_snapshot_before_after_hash",
            policy::unchanged(&before, &after),
        )?;
        image
            .check("observed_package_after_metadata")
            .map_err(|error| {
                pin_failure_context(
                    error,
                    "after_package_image_hash",
                    "observed_package",
                    &spec.application.executable,
                )
            })?;
        Ok(Self {
            spec: spec.clone(),
            key: key.clone(),
            process,
            image,
            snapshot: after,
        })
    }
    pub(super) fn matches(&self, spec: &ObservedPackage, key: &WindowIdentity) -> bool {
        self.process.value.is_some()
            && self.image.file.value.is_some()
            && policy::matches(&self.spec, &self.key, spec, key)
    }
    pub(super) fn check(&self) -> Result<()> {
        let context = Context {
            spec: &self.spec,
            key: &self.key,
            stage: "revalidate_observed_package",
            queries: RefCell::default(),
        };
        let _call = context.policy("native_call", native_call("check_observed_package_proof"))?;
        if self.process.value.is_none() || self.image.file.value.is_none() {
            return Err(context.fault(
                "observed_package_proof_closed",
                "retained_resource_state",
                "Observed proof resources have already been released; no reuse or dispatch.",
            ));
        }
        self.image
            .check("observed_package_before_metadata")
            .map_err(|error| {
                pin_failure_context(
                    error,
                    context.stage,
                    "observed_package",
                    &self.spec.application.executable,
                )
            })?;
        let current = snapshot(&context, self.process.raw())?;
        context.policy(
            "package_snapshot_cached_identity",
            policy::unchanged(&self.snapshot, &current),
        )?;
        self.image
            .check("observed_package_after_metadata")
            .map_err(|error| {
                pin_failure_context(
                    error,
                    context.stage,
                    "observed_package",
                    &self.spec.application.executable,
                )
            })
    }
    pub(super) fn note(&self) -> String {
        // Every source identity is bounded before caching. Identity fields are
        // complete, not silently truncated; the caller retains its global
        // MAX_METADATA_BYTES/incomplete-inventory rule for multiple proofs.
        serde_json::json!({
            "proof":"observed_os_managed_package_disk_image",
            "package_full_name":self.snapshot.full_name, "package_family":self.snapshot.family,
            "registered_install_root":self.snapshot.install_path,
            "effective_root_equals_install":self.snapshot.effective_path.eq_ignore_ascii_case(&self.snapshot.install_path),
            "origin":if self.snapshot.origin == policy::ORIGIN_STORE { "store" } else { "inbox" },
            "origin_raw":self.snapshot.origin,"origin_symbol":policy::origin_symbol(self.snapshot.origin),
            "head_flags":self.snapshot.flags, "window":self.key,
            "head_flags_hex":format!("0x{:08x}",self.snapshot.flags),
            "head_unknown_bits":self.snapshot.flags & !policy::KNOWN_PROPERTIES,
            "process_birth":self.snapshot.process.birth, "process_image":self.snapshot.process.image,
            "disk_sha256":self.image.sha256,
            "file_id":{"volume":self.image.stamp.volume,"index":self.image.stamp.index},
            "user_sid":self.snapshot.worker.sid,"user_session":self.snapshot.worker.session,
            "worker_authentication_id":self.snapshot.worker.authentication_id,
            "target_authentication_id":self.snapshot.process.token.authentication_id,
            "worker_token_source":self.snapshot.worker.source,
            "scope":"current-user registration plus root-to-image native pin; no different selected effective root observed",
            "limits":"disk image, not process memory or loaded DLLs; sampled checks, not OS-wide atomicity",
        }).to_string()
    }
    pub(super) fn close(&mut self) -> bool {
        // Evaluate both releases even if one fails. PinnedFile and OwnedHandle
        // have sticky audited close outcomes and do not retry uncertain closes.
        self.image.close() & self.process.close()
    }
}
impl Drop for ObservedProof {
    fn drop(&mut self) {
        self.close();
    }
}
