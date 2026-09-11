//! Native execution identity and physical-pixel geometry. Never switches desktops.
use crate::{NativeWindowKey, WindowRecord};
use rcu_core::{
    Identity, Result, digest, fault,
    operation::{checkpoint, native_call},
};
use rcu_imaging::Rect;
use serde::Serialize;
use std::{ffi::c_void, mem::size_of};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        Security::*,
        System::{Registry::*, RemoteDesktop::*, StationsAndDesktops::*, Threading::*},
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    core::{BOOL, PCWSTR, PWSTR, w},
};

pub fn api<T>(stage: &str, r: windows::core::Result<T>) -> Result<T> {
    r.map_err(|e|fault(stage,&format!("Native Windows error 0x{:08x}; reobserve the allocated session; do not replay input.",e.code().0 as u32)))
}
pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct InputDesktop(HDESK);
impl Drop for InputDesktop {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseDesktop(self.0);
        }
    }
}
pub struct DpiScope(DPI_AWARENESS_CONTEXT);
impl DpiScope {
    pub fn enter() -> Result<Self> {
        let old =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if old.0.is_null() {
            return Err(fault(
                "dpi_context_unavailable",
                "Physical-pixel mapping requires a per-monitor-v2 thread context.",
            ));
        }
        Ok(Self(old))
    }
}
impl Drop for DpiScope {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
pub fn hwnd(id: &str) -> Result<HWND> {
    let n = usize::from_str_radix(
        id.strip_prefix("hwnd:")
            .ok_or_else(|| fault("invalid_window", "Use a discovered HWND token."))?,
        16,
    )
    .map_err(|_| fault("invalid_window", "Use a discovered HWND token."))?;
    if n == 0 {
        return Err(fault("invalid_window", "Null HWND is not a window."));
    }
    Ok(HWND(n as *mut c_void))
}
pub fn window_id(w: HWND) -> String {
    format!("hwnd:{:x}", w.0 as usize)
}
fn wide(v: &[u16]) -> String {
    String::from_utf16_lossy(&v[..v.iter().position(|c| *c == 0).unwrap_or(v.len())])
}
fn object_name(h: HANDLE) -> Result<String> {
    let mut b = [0u16; 256];
    let mut n = 0;
    api("desktop_name_unavailable", unsafe {
        GetUserObjectInformationW(
            h,
            UOI_NAME,
            Some(b.as_mut_ptr().cast()),
            size_of_val(&b) as u32,
            Some(&mut n),
        )
    })?;
    if n == 0 || n as usize > size_of_val(&b) {
        return Err(fault(
            "desktop_name_unavailable",
            "Native name exceeded the bounded buffer.",
        ));
    }
    Ok(wide(&b))
}
pub fn identity() -> Result<Identity> {
    let _call = native_call("windows_identity")?;
    let mut machine = [0u16; 256];
    let mut n = size_of_val(&machine) as u32;
    api("machine_identity_unavailable", unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Cryptography"),
            w!("MachineGuid"),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            None,
            Some(machine.as_mut_ptr().cast()),
            Some(&mut n),
        )
        .ok()
    })?;
    let machine = wide(&machine);
    if machine.is_empty() {
        return Err(fault(
            "machine_identity_unavailable",
            "MachineGuid is empty.",
        ));
    }
    let mut sid = 0;
    api("session_identity_unavailable", unsafe {
        ProcessIdToSessionId(GetCurrentProcessId(), &mut sid)
    })?;
    let mut token = HANDLE::default();
    api("logon_identity_unavailable", unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
    })?;
    let token = Handle(token);
    let mut stats = TOKEN_STATISTICS::default();
    let mut len = 0;
    api("logon_identity_unavailable", unsafe {
        GetTokenInformation(
            token.0,
            TokenStatistics,
            Some((&mut stats as *mut TOKEN_STATISTICS).cast()),
            size_of::<TOKEN_STATISTICS>() as u32,
            &mut len,
        )
    })?;
    let station = object_name(HANDLE(
        api("window_station_unavailable", unsafe {
            GetProcessWindowStation()
        })?
        .0,
    ))?;
    let desk = object_name(HANDLE(
        api("desktop_unavailable", unsafe {
            GetThreadDesktop(GetCurrentThreadId())
        })?
        .0,
    ))?;
    let input = InputDesktop(api("input_desktop_unavailable", unsafe {
        OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
    })?);
    let input_name = object_name(HANDLE(input.0.0))?;
    let mut ptr = PWSTR::null();
    let mut count = 0;
    api("wts_session_unavailable", unsafe {
        WTSQuerySessionInformationW(None, sid, WTSSessionInfoEx, &mut ptr, &mut count)
    })?;
    struct Wts(PWSTR);
    impl Drop for Wts {
        fn drop(&mut self) {
            unsafe {
                WTSFreeMemory(self.0.0.cast());
            }
        }
    }
    let data = Wts(ptr);
    if data.0.is_null() || count < size_of::<WTSINFOEXW>() as u32 {
        return Err(fault(
            "wts_session_unavailable",
            "Extended session information is incomplete.",
        ));
    }
    let info = unsafe { &*data.0.0.cast::<WTSINFOEXW>() };
    if info.Level != 1 {
        return Err(fault(
            "wts_session_unavailable",
            "Unsupported WTS information level.",
        ));
    }
    let info = unsafe { info.Data.WTSInfoExLevel1 };
    if sid == 0
        || info.SessionId != sid
        || info.SessionState != WTSActive
        || info.SessionFlags != WTS_SESSIONSTATE_UNLOCK as i32
        || !station.eq_ignore_ascii_case("WinSta0")
        || desk != input_name
    {
        return Err(fault(
            "inactive_or_locked_desktop",
            "Run the candidate in the explicitly allocated, unlocked interactive user desktop. No desktop/session switching is performed.",
        ));
    }
    Ok(Identity {
        host: format!("windows-machine:{}", digest(machine.as_bytes())),
        guest: None,
        session: format!(
            "wts:{sid}:logon:{}:luid:{:x}:{:x}",
            info.LogonTime, stats.AuthenticationId.HighPart, stats.AuthenticationId.LowPart
        ),
        seat: station,
        desktop: desk,
    })
}
pub fn process_created(pid: u32) -> Result<u64> {
    let h = Handle(api("process_epoch_unavailable", unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
    })?);
    let (mut a, mut b, mut c, mut d) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    api("process_epoch_unavailable", unsafe {
        GetProcessTimes(h.0, &mut a, &mut b, &mut c, &mut d)
    })?;
    Ok((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime))
}
pub fn rect(r: RECT) -> Option<Rect> {
    let w = i64::from(r.right) - i64::from(r.left);
    let h = i64::from(r.bottom) - i64::from(r.top);
    if w <= 0 || h <= 0 {
        return None;
    }
    let r = Rect {
        x: r.left,
        y: r.top,
        width: u32::try_from(w).ok()?,
        height: u32::try_from(h).ok()?,
    };
    r.checked().ok()
}
fn key(w: HWND) -> Option<NativeWindowKey> {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(w, Some(&mut pid));
    }
    Some(NativeWindowKey {
        window: window_id(w),
        pid,
        process_created: process_created(pid).ok()?,
    })
}
pub fn record(w: HWND, z: usize) -> Result<WindowRecord> {
    checkpoint("window_metadata")?;
    if !unsafe { IsWindow(Some(w)) }.as_bool() {
        return Err(fault("window_gone", "Discover again."));
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(w, Some(&mut pid));
    }
    let created = process_created(pid).ok();
    let mut title = [0u16; 512];
    let mut class = [0u16; 256];
    let title_len = unsafe { GetWindowTextW(w, &mut title) };
    unsafe {
        GetClassNameW(w, &mut class);
    }
    let class = wide(&class);
    let mut r = RECT::default();
    let rectangle = unsafe { GetWindowRect(w, &mut r) }
        .ok()
        .and_then(|_| rect(r));
    let mut dwm = RECT::default();
    let dwm_visible_frame = unsafe {
        DwmGetWindowAttribute(
            w,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut dwm as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
    }
    .ok()
    .and_then(|_| rect(dwm));
    let mut cloaked = 0u32;
    let cloaked = unsafe {
        DwmGetWindowAttribute(
            w,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    }
    .ok()
    .map(|_| cloaked != 0);
    let mut chain = vec![];
    let mut owner = w;
    let mut complete = false;
    for _ in 0..16 {
        unsafe {
            SetLastError(ERROR_SUCCESS);
        }
        let next = unsafe { GetWindow(owner, GW_OWNER) }.unwrap_or_default();
        if next.0.is_null() {
            complete = unsafe { GetLastError() } == ERROR_SUCCESS;
            break;
        }
        let Some(k) = key(next) else {
            break;
        };
        if chain.contains(&k) {
            break;
        }
        chain.push(k);
        owner = next;
    }
    let parent = unsafe { GetAncestor(w, GA_PARENT) };
    let dpi = unsafe { GetDpiForWindow(w) };
    Ok(WindowRecord {
        window: window_id(w),
        pid,
        process_created: created,
        native_epoch: created
            .map(|c| digest(format!("{}:{pid}:{c}:{class}", window_id(w)).as_bytes())),
        title: wide(&title),
        title_truncated: title_len as usize >= title.len() - 1,
        class,
        rectangle,
        dwm_visible_frame,
        visible: unsafe { IsWindowVisible(w) }.as_bool(),
        enabled: unsafe { IsWindowEnabled(w) }.as_bool(),
        minimized: unsafe { IsIconic(w) }.as_bool(),
        cloaked,
        foreground: unsafe { GetForegroundWindow() } == w,
        window_dpi: (dpi != 0).then_some(dpi),
        window_dpi_awareness: unsafe {
            GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(w))
        }
        .0,
        owner_chain: chain,
        owner_chain_complete: complete,
        parent: (!parent.0.is_null()).then(|| window_id(parent)),
        z_index: z,
    })
}
pub fn windows() -> Result<(Vec<WindowRecord>, bool)> {
    struct List {
        handles: Vec<usize>,
        complete: bool,
    }
    unsafe extern "system" fn cb(w: HWND, p: LPARAM) -> BOOL {
        // All allocation/panic is contained before crossing the ABI boundary.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let l = unsafe { &mut *(p.0 as *mut List) };
            if l.handles.len() == 256 {
                l.complete = false;
                return BOOL(0);
            }
            l.handles.push(w.0 as usize);
            BOOL(1)
        }))
        .unwrap_or(BOOL(0))
    }
    let mut list = List {
        handles: Vec::with_capacity(256),
        complete: true,
    };
    let _call = native_call("EnumWindows")?;
    let result = unsafe { EnumWindows(Some(cb), LPARAM((&mut list as *mut List) as isize)) };
    drop(_call);
    if list.complete {
        api("window_enumeration_failed", result)?;
    }
    let mut records = vec![];
    let mut complete = list.complete;
    for (z, w) in list.handles.into_iter().enumerate() {
        checkpoint("window_enumeration")?;
        if let Ok(r) = record(HWND(w as *mut c_void), z) {
            records.push(r);
        } else {
            complete = false;
        }
    }
    Ok((records, complete))
}
#[derive(Debug, Clone, Serialize)]
pub struct Display {
    pub device: String,
    pub physical_rect: Rect,
    pub primary: bool,
    pub mode_size: Option<[u32; 2]>,
    pub rotation_quarter_turns: Option<u32>,
    pub effective_dpi: Option<[u32; 2]>,
    pub dpi_note: String,
}
pub fn displays() -> Result<Vec<Display>> {
    struct List {
        items: Vec<Display>,
        failed: bool,
    }
    unsafe extern "system" fn cb(m: HMONITOR, _dc: HDC, _r: *mut RECT, p: LPARAM) -> BOOL {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let l = unsafe { &mut *(p.0 as *mut List) };
            if l.items.len() >= 16 {
                l.failed = true;
                return BOOL(0);
            }
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            if !unsafe { GetMonitorInfoW(m, &mut info.monitorInfo) }.as_bool() {
                l.failed = true;
                return BOOL(0);
            }
            let Some(r) = rect(info.monitorInfo.rcMonitor) else {
                l.failed = true;
                return BOOL(0);
            };
            let mut mode = DEVMODEW {
                dmSize: size_of::<DEVMODEW>() as u16,
                ..Default::default()
            };
            let ok = unsafe {
                EnumDisplaySettingsExW(
                    PCWSTR(info.szDevice.as_ptr()),
                    ENUM_CURRENT_SETTINGS,
                    &mut mode,
                    ENUM_DISPLAY_SETTINGS_FLAGS(0),
                )
            }
            .as_bool();
            l.items.push(Display{device:wide(&info.szDevice),physical_rect:r,primary:info.monitorInfo.dwFlags&1!=0,mode_size:ok.then_some([mode.dmPelsWidth,mode.dmPelsHeight]),rotation_quarter_turns:ok.then_some(unsafe{mode.Anonymous1.Anonymous2.dmDisplayOrientation.0}),effective_dpi:None,dpi_note:"No monitor-wide effective DPI inferred from GetDpiForMonitor; window DPI is separately reported with target awareness. Geometry is physical pixels under PMv2.".into()});
            BOOL(1)
        }));
        res.unwrap_or(BOOL(0))
    }
    let mut list = List {
        items: vec![],
        failed: false,
    };
    let _call = native_call("display_enumeration")?;
    if !unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(cb),
            LPARAM((&mut list as *mut List) as isize),
        )
    }
    .as_bool()
        || list.failed
        || list.items.is_empty()
    {
        return Err(fault(
            "display_mapping_unavailable",
            "Monitor enumeration failed or exceeded 16 displays.",
        ));
    }
    Ok(list.items)
}
pub fn hit_window(point: [f64; 2]) -> Option<String> {
    if !point.iter().all(|x| x.is_finite()) {
        return None;
    }
    let w = unsafe {
        WindowFromPoint(POINT {
            x: point[0].round() as i32,
            y: point[1].round() as i32,
        })
    };
    if w.0.is_null() {
        return None;
    }
    Some(window_id(unsafe { GetAncestor(w, GA_ROOT) }))
}
