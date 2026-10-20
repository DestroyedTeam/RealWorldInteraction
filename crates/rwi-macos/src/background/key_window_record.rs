//! Explicit target-only private key-window record preparation. No global front
//! process setter, previous-app defocus, cursor warp or opaque CGEvent access.
//! Legacy 248-byte encoding is experimental, not a current-OS ABI attestation.
use rwi_core::{Result, fault};
use std::{ffi::c_void, sync::OnceLock};
#[repr(C)]
#[derive(Default)]
struct Psn {
    high: u32,
    low: u32,
}
type Main = unsafe extern "C" fn() -> i32;
type Owner = unsafe extern "C" fn(i32, u32, *mut i32) -> i32;
type ConnectionPsn = unsafe extern "C" fn(i32, *mut Psn) -> i32;
type ProcessPid = unsafe extern "C" fn(*const Psn, *mut i32) -> i32;
type Post = unsafe extern "C" fn(*const Psn, *mut u8) -> i32;
struct Driver {
    main: Main,
    owner: Owner,
    psn: ConnectionPsn,
    pid: ProcessPid,
    post: Post,
    _libraries: [usize; 2],
}
fn driver() -> Option<&'static Driver> {
    static DRIVER: OnceLock<Option<Driver>> = OnceLock::new();
    DRIVER
        .get_or_init(|| unsafe {
            let sky = libc::dlopen(
                c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
                libc::RTLD_LOCAL | libc::RTLD_NOW,
            );
            if sky.is_null() {
                return None;
            }
            let app = libc::dlopen(
                c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices"
                    .as_ptr(),
                libc::RTLD_LOCAL | libc::RTLD_NOW,
            );
            if app.is_null() {
                libc::dlclose(sky);
                return None;
            }
            // Successful libraries have process lifetime; failed resolutions close both.
            let found = (|| {
                macro_rules! resolve {
                    ($lib:expr,$name:expr,$ty:ty) => {{
                        let p = libc::dlsym($lib, $name.as_ptr());
                        if p.is_null() {
                            return None;
                        }
                        std::mem::transmute::<*mut c_void, $ty>(p)
                    }};
                }
                Some(Driver {
                    main: resolve!(sky, c"SLSMainConnectionID", Main),
                    owner: resolve!(sky, c"SLSGetWindowOwner", Owner),
                    psn: resolve!(sky, c"SLSGetConnectionPSN", ConnectionPsn),
                    pid: resolve!(app, c"GetProcessPID", ProcessPid),
                    post: resolve!(sky, c"SLPSPostEventRecordTo", Post),
                    _libraries: [sky as usize, app as usize],
                })
            })();
            if found.is_none() {
                libc::dlclose(app);
                libc::dlclose(sky);
            }
            found
        })
        .as_ref()
}
pub(crate) fn available() -> bool {
    driver().is_some()
}
#[derive(Clone)]
#[repr(align(8))]
pub(crate) struct Record([u8; 248]);
fn records(pid: u32, window: u32) -> Result<(Record, Record)> {
    if pid == 0 || pid > i32::MAX as u32 || window == 0 {
        return Err(fault(
            "invalid_key_record_target",
            "Positive exact PID/window required before any native lookup or input.",
        ));
    }
    let mut down = Record([0; 248]);
    down.0[4..8].copy_from_slice(&248u32.to_le_bytes());
    down.0[8..12].copy_from_slice(&1u32.to_le_bytes());
    down.0[0x20..0x30].fill(0xff); // legacy blank-location focus record, not a point click
    down.0[0x3a] = 0x10;
    down.0[0x3c..0x40].copy_from_slice(&window.to_le_bytes());
    let mut up = down.clone();
    up.0[8..12].copy_from_slice(&2u32.to_le_bytes());
    Ok((down, up))
}
pub(crate) struct Prepared {
    driver: &'static Driver,
    psn: Psn,
    events: (Record, Record),
}
pub(crate) fn prepare(pid: u32, window: u32) -> Result<Prepared> {
    let events = records(pid, window)?;
    let d = driver().ok_or_else(|| {
        fault(
            "key_record_delivery_unavailable",
            "Experimental exact-window record symbols unavailable; no downgrade or input.",
        )
    })?;
    let cid = unsafe { (d.main)() };
    let mut owner = 0;
    let mut psn = Psn::default();
    let mut actual_pid = 0;
    if cid <= 0
        || unsafe { (d.owner)(cid, window, &mut owner) } != 0
        || owner <= 0
        || unsafe { (d.psn)(owner, &mut psn) } != 0
        || (psn.high == 0 && psn.low == 0)
        || unsafe { (d.pid)(&psn, &mut actual_pid) } != 0
        || actual_pid != pid as i32
    {
        return Err(fault(
            "key_record_owner_unknown",
            "Exact window owner connection/PSN/PID must agree. No PID-only fallback or input.",
        ));
    }
    Ok(Prepared {
        driver: d,
        psn,
        events,
    })
}
impl Prepared {
    pub(crate) fn events(&self) -> (Record, Record) {
        self.events.clone()
    }
    /// Only the invocation-local guarded paired dispatcher may call this. Native
    /// status is not effect proof; both edge statuses are checked before keys.
    pub(crate) fn post(&self, event: &Record) -> i32 {
        let mut local = event.clone();
        unsafe { (self.driver.post)(&self.psn, local.0.as_mut_ptr()) }
    }
}
pub(crate) fn capabilities() -> serde_json::Value {
    serde_json::json!({"target_key_window_record248_experimental":{
        "implemented":true,"available":crate::background::key_window_record::available(),
        "opt_in_required":true,"record_layout":"legacy_248_unattested","runtime_record_length_known":false,
        "exact_window_owner_connection_required":true,"pid_fallback":false,
        "global_front_process_setter":false,"previous_process_defocus":false,
        "application_effect_attested":false,"no_replay":true,
        "same_invocation_lease_and_native_guard_required":true}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f293_record_invalid_pid_never_needs_driver() {
        for p in [0, u32::MAX] {
            assert!(records(p, 7).is_err());
        }
    }
    #[test]
    fn f293_record_invalid_window_never_needs_driver() {
        assert!(records(7, 0).is_err());
    }
    #[test]
    fn f293_record_length_and_alignment() {
        let (a, _) = records(7, 9).unwrap();
        assert_eq!(a.0.len(), 248);
        assert!(std::mem::align_of::<Record>() >= 8);
        assert_eq!(u32::from_le_bytes(a.0[4..8].try_into().unwrap()), 248);
    }
    #[test]
    fn f293_record_window_is_exact() {
        let (a, _) = records(7, 0x12345678).unwrap();
        assert_eq!(&a.0[0x3c..0x40], &0x12345678u32.to_le_bytes());
    }
    #[test]
    fn f293_record_edges_differ_only_by_type() {
        let (a, b) = records(7, 9).unwrap();
        assert_eq!(a.0.iter().zip(&b.0).filter(|(a, b)| a != b).count(), 1);
        assert_eq!(a.0[8], 1);
        assert_eq!(b.0[8], 2);
    }
    #[test]
    fn f293_record_no_pointer_payload_or_global_process_data() {
        let (a, _) = records(7, 9).unwrap();
        assert!(a.0[0xe0..].iter().all(|x| *x == 0));
        assert!(a.0[0x20..0x30].iter().all(|x| *x == 255));
        assert_eq!(a.0[0x3a], 0x10);
    }
}
