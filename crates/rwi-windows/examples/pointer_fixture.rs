//! Explicit live-test application, NOT an agent/backend or hidden input bridge.
//! Draws only mouse events received by its own window; create-new event log is
//! an independent GUI-consumer oracle. It never injects input or changes policy.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(not(windows))]
fn main() {
    eprintln!("This opt-in fixture needs Windows; no native operation performed.");
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    native::run()
}

#[cfg(windows)]
mod native {
    use std::{
        fs::{File, OpenOptions},
        io::Write,
        sync::{Mutex, OnceLock},
        time::Instant,
    };
    use windows::{
        Win32::{
            Foundation::*,
            Graphics::Gdi::*,
            System::LibraryLoader::GetModuleHandleW,
            UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
        },
        core::{Result, w},
    };
    struct State {
        file: File,
        start: Instant,
        strokes: Vec<Vec<POINT>>,
        held: bool,
    }
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    fn event(state: &mut State, kind: &str, point: Option<POINT>) {
        let v = serde_json::json!({"kind":kind,"elapsed_ms":state.start.elapsed().as_micros() as f64/1000.,"pid":std::process::id(),"point":point.map(|p|[p.x,p.y]),"left_down":unsafe {GetAsyncKeyState(1)}<0});
        // GUI validation stops recording if the owned log fails; never forges an oracle.
        let _ = writeln!(state.file, "{v}");
        let _ = state.file.flush();
    }
    unsafe extern "system" fn procedure(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let p = POINT {
            x: (lparam.0 as u16) as i16 as i32,
            y: ((lparam.0 >> 16) as u16) as i16 as i32,
        };
        match msg {
            WM_LBUTTONDOWN | WM_MOUSEMOVE | WM_LBUTTONUP => {
                if let Some(state) = STATE.get() {
                    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
                    if msg == WM_LBUTTONDOWN {
                        state.held = true;
                        state.strokes.push(vec![p]);
                        event(&mut state, "down", Some(p));
                    } else if msg == WM_MOUSEMOVE && state.held {
                        if let Some(stroke) = state.strokes.last_mut() {
                            stroke.push(p);
                        }
                        event(&mut state, "move", Some(p));
                    } else if msg == WM_LBUTTONUP {
                        state.held = false;
                        event(&mut state, "up", Some(p));
                    }
                    drop(state);
                    if msg == WM_LBUTTONDOWN {
                        unsafe {
                            let _ = SetFocus(Some(hwnd));
                            let _ = SetCapture(hwnd);
                        }
                    }
                    if msg == WM_LBUTTONUP {
                        unsafe {
                            let _ = ReleaseCapture();
                            SetTimer(Some(hwnd), 1, 50, None);
                        }
                    }
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_TIMER => {
                unsafe {
                    let _ = KillTimer(Some(hwnd), 1);
                }
                if let Some(s) = STATE.get() {
                    event(
                        &mut s.lock().unwrap_or_else(|p| p.into_inner()),
                        "settled",
                        None,
                    );
                }
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 == 0x74 => {
                if let Some(s) = STATE.get() {
                    event(&mut s.lock().unwrap_or_else(|p| p.into_inner()), "f5", None);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = unsafe { BeginPaint(hwnd, &mut paint) };
                let mut rect = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rect);
                    FillRect(dc, &rect, HBRUSH(GetStockObject(WHITE_BRUSH).0));
                }
                let mut text =
                    "RWI native pointer validation — received window events, no input injection"
                        .encode_utf16()
                        .collect::<Vec<_>>();
                let mut label = RECT {
                    left: 16,
                    top: 16,
                    right: 950,
                    bottom: 50,
                };
                unsafe {
                    DrawTextW(dc, &mut text, &mut label, DT_LEFT | DT_TOP);
                }
                let pen = unsafe { CreatePen(PS_SOLID, 4, COLORREF(0x00d06020)) };
                let previous = unsafe { SelectObject(dc, pen.into()) };
                if let Some(s) = STATE.get() {
                    let s = s.lock().unwrap_or_else(|p| p.into_inner());
                    for points in &s.strokes {
                        if points.len() > 1 {
                            unsafe {
                                let _ = Polyline(dc, points);
                            }
                        }
                    }
                }
                unsafe {
                    SelectObject(dc, previous);
                    let _ = DeleteObject(pen.into());
                    let _ = EndPaint(hwnd, &paint);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
    pub(super) fn run() -> Result<()> {
        let path = std::env::args_os()
            .nth(1)
            .expect("Supply an allocated, absent event-log path");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("Create a new fixture-owned log, never overwrite");
        let _ = STATE.set(Mutex::new(State {
            file,
            start: Instant::now(),
            strokes: vec![],
            held: false,
        }));
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
        }
        let instance: HINSTANCE = unsafe { GetModuleHandleW(None)? }.into();
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("RCUPointerFixture"),
            hCursor: unsafe { LoadCursorW(None, IDC_CROSS)? },
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("RCUPointerFixture"),
                w!("RWI Pointer Validation"),
                WS_OVERLAPPEDWINDOW,
                300,
                300,
                1000,
                700,
                None,
                None,
                Some(instance),
                None,
            )?
        };
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = UpdateWindow(hwnd);
        }
        event(
            &mut STATE
                .get()
                .unwrap()
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
            "ready",
            None,
        );
        let mut message = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            if result == 0 {
                break;
            }
            if result < 0 {
                return Err(windows::core::Error::from_thread());
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        Ok(())
    }
}
