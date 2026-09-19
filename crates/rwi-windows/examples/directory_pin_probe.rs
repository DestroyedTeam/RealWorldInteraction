//! Explicit read-only native diagnostic plus rename checks on owned temp dirs.
//! Not an input bridge, permission repair, or runtime fallback.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;
    use windows::{
        Win32::{
            Foundation::{CloseHandle, HANDLE},
            Storage::FileSystem::*,
        },
        core::PCWSTR,
    };
    struct Pin(HANDLE);
    impl Drop for Pin {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    fn open(path: &str, access: u32) -> windows::core::Result<Pin> {
        let name: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
        unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                access,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
            .map(Pin)
        }
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("one explicit local directory path required".into());
    }
    rwi_windows::agent_contract::validate_local_dos_path(&args[0]).map_err(|e| e.code)?;
    let mut rows = Vec::new();
    for access in [FILE_TRAVERSE.0, FILE_READ_ATTRIBUTES.0, 0] {
        let native = match open(&args[0], access) {
            Err(e) => json!({"opened":false,"hresult":format!("0x{:08x}",e.code().0 as u32)}),
            Ok(pin) => {
                let mut info = BY_HANDLE_FILE_INFORMATION::default();
                let information = unsafe { GetFileInformationByHandle(pin.0, &mut info) };
                let mut name = vec![0u16; 32768];
                let n = unsafe {
                    GetFinalPathNameByHandleW(
                        pin.0,
                        &mut name,
                        GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
                    )
                } as usize;
                json!({"opened":true,"information_ok":information.is_ok(),
                    "final_path":if n>0 && n<name.len() {Some(String::from_utf16(&name[..n])?)} else {None},
                    "attributes":info.dwFileAttributes,"volume":info.dwVolumeSerialNumber,
                    "file_index":(u64::from(info.nFileIndexHigh)<<32)|u64::from(info.nFileIndexLow)})
            }
        };
        // Verify the share-lock property independently on disposable data we own.
        let temp = tempfile::tempdir()?;
        let original = temp.path().join("owned-pin-test");
        let moved = temp.path().join("owned-pin-moved");
        std::fs::create_dir(&original)?;
        let pin = open(original.to_str().ok_or("temp path unicode")?, access)?;
        let rename = std::fs::rename(&original, &moved);
        let rename_error = rename.as_ref().err().and_then(|e| e.raw_os_error());
        drop(pin);
        rows.push(
            json!({"desired_access":access,"share_mode":FILE_SHARE_READ.0,
            "target_readonly":native,"owned_temp_rename_while_pin":rename.is_ok(),
            "owned_temp_rename_error":rename_error}),
        );
    }
    println!(
        "{}",
        json!({"diagnostic":"directory_pin_probe_v1","rows":rows,
        "privilege":rwi_windows::agent_binding::current_token_privilege().map_err(|e|e.code)?,
        "target_mutated":false,"acl_or_privileges_changed":false,"input_sent":false})
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("directory_pin_probe requires an explicitly allocated native Windows target");
}
