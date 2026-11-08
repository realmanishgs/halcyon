//! Native video surface: a child window of the app window that libVLC renders
//! into (Direct3D11 output). The web UI reports the rectangle it reserves for
//! video and we keep this window positioned above the webview at that spot.
//!
//! Because the surface sits above the webview, it also owns pointer input and
//! file drops over the video. libVLC's own windows have mouse input disabled,
//! so the system routes those messages here; we forward them to the UI.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SurfaceEvent {
    Down,
    Click,
    DoubleClick,
    Context,
    Move,
    Wheel { delta: i32 },
    Drop { paths: Vec<String> },
}

#[cfg(windows)]
mod imp {
    use super::SurfaceEvent;
    use std::ptr::null;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH, HBRUSH};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    type Sink = Box<dyn Fn(SurfaceEvent) + Send + Sync>;
    static SINK: OnceLock<Sink> = OnceLock::new();
    static HIDE_CURSOR: AtomicBool = AtomicBool::new(false);
    static LAST_MOVE_MS: AtomicU64 = AtomicU64::new(0);

    fn emit(e: SurfaceEvent) {
        if let Some(s) = SINK.get() {
            s(e);
        }
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
    }

    unsafe fn dropped_paths(h: HDROP) -> Vec<String> {
        let n = DragQueryFileW(h, u32::MAX, std::ptr::null_mut(), 0);
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let len = DragQueryFileW(h, i, std::ptr::null_mut(), 0);
            let mut buf = vec![0u16; len as usize + 1];
            DragQueryFileW(h, i, buf.as_mut_ptr(), buf.len() as u32);
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        DragFinish(h);
        out
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_LBUTTONDOWN => emit(SurfaceEvent::Down),
            WM_LBUTTONUP => emit(SurfaceEvent::Click),
            WM_LBUTTONDBLCLK => emit(SurfaceEvent::DoubleClick),
            WM_RBUTTONUP => emit(SurfaceEvent::Context),
            WM_MOUSEMOVE => {
                // Throttled: the UI only needs it to reveal auto-hidden controls.
                let t = now_ms();
                if t.saturating_sub(LAST_MOVE_MS.load(Ordering::Relaxed)) > 120 {
                    LAST_MOVE_MS.store(t, Ordering::Relaxed);
                    emit(SurfaceEvent::Move);
                }
            }
            WM_MOUSEWHEEL => emit(SurfaceEvent::Wheel { delta: ((wp >> 16) & 0xffff) as u16 as i16 as i32 }),
            WM_DROPFILES => {
                emit(SurfaceEvent::Drop { paths: dropped_paths(wp as HDROP) });
                return 0;
            }
            WM_SETCURSOR if HIDE_CURSOR.load(Ordering::Relaxed) => {
                SetCursor(std::ptr::null_mut());
                return 1;
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Must be called on the UI thread. `sink` receives input/drop events.
    pub fn create(parent: isize, sink: impl Fn(SurfaceEvent) + Send + Sync + 'static) -> Result<isize, String> {
        let _ = SINK.set(Box::new(sink));
        unsafe {
            let class = wide("HalcyonVideoSurface");
            let hinst = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: std::ptr::null_mut(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc); // fails harmlessly if already registered
            let hwnd = CreateWindowExW(
                WS_EX_ACCEPTFILES,
                class.as_ptr(),
                null(),
                WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                0, 0, 1, 1,
                parent as HWND,
                std::ptr::null_mut(),
                hinst,
                null(),
            );
            if hwnd.is_null() {
                return Err("Could not create video surface".into());
            }
            Ok(hwnd as isize)
        }
    }

    /// Physical-pixel rectangle relative to the parent's client area.
    /// (A window region cannot be used to let HTML show through: libVLC's
    /// Direct3D11 flip-model swapchain ignores the parent's clip region.)
    pub fn place(hwnd: isize, x: i32, y: i32, w: i32, h: i32, visible: bool) {
        unsafe {
            let flag = if visible && w > 0 && h > 0 { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
            SetWindowPos(hwnd as HWND, HWND_TOP, x, y, w.max(1), h.max(1), SWP_NOACTIVATE | flag);
        }
    }

    /// Hide the pointer while it is over the video (fullscreen idle).
    pub fn set_cursor_hidden(hidden: bool) {
        HIDE_CURSOR.store(hidden, Ordering::Relaxed);
        if hidden {
            unsafe { SetCursor(std::ptr::null_mut()) };
        }
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
pub fn create(_parent: isize, _sink: impl Fn(SurfaceEvent) + Send + Sync + 'static) -> Result<isize, String> {
    Err("Native video surface is only implemented on Windows so far".into())
}
#[cfg(not(windows))]
pub fn place(_h: isize, _x: i32, _y: i32, _w: i32, _hh: i32, _v: bool) {}
#[cfg(not(windows))]
pub fn set_cursor_hidden(_hidden: bool) {}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
    use windows_sys::Win32::UI::Shell::DROPFILES;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static GOT: Mutex<Vec<SurfaceEvent>> = Mutex::new(Vec::new());

    /// Builds the same HDROP Explorer hands to WM_DROPFILES and delivers it to
    /// a real surface window; the sink must receive the exact (Unicode) paths.
    #[test]
    fn wm_dropfiles_reaches_sink_with_unicode_paths() {
        unsafe {
            let cls: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let parent = CreateWindowExW(0, cls.as_ptr(), std::ptr::null(), WS_OVERLAPPED, 0, 0, 100, 100,
                std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
            assert!(!parent.is_null());
            let hwnd = create(parent as isize, |e| GOT.lock().unwrap().push(e)).unwrap();

            let files = [r"C:\media\ünïcødé 日本語.mkv", r"\nas\share\movie.mp4"];
            let mut payload: Vec<u16> = Vec::new();
            for f in files {
                payload.extend(f.encode_utf16());
                payload.push(0);
            }
            payload.push(0);
            let header = std::mem::size_of::<DROPFILES>();
            let mem = GlobalAlloc(GHND, header + payload.len() * 2);
            let p = GlobalLock(mem) as *mut u8;
            *(p as *mut DROPFILES) = DROPFILES { pFiles: header as u32, pt: POINT { x: 5, y: 5 }, fNC: 0, fWide: 1 };
            std::ptr::copy_nonoverlapping(payload.as_ptr() as *const u8, p.add(header), payload.len() * 2);
            GlobalUnlock(mem);

            SendMessageW(hwnd as _, WM_DROPFILES, mem as usize, 0); // handler frees it (DragFinish)
            SendMessageW(hwnd as _, WM_MOUSEWHEEL, (-120i16 as u16 as usize) << 16, 0);
            SendMessageW(hwnd as _, WM_LBUTTONDBLCLK, 0, 0);

            let got = GOT.lock().unwrap();
            match &got[0] {
                SurfaceEvent::Drop { paths } => assert_eq!(paths, &files.map(String::from).to_vec()),
                other => panic!("expected drop, got {other:?}"),
            }
            assert!(matches!(got[1], SurfaceEvent::Wheel { delta: -120 }));
            assert!(matches!(got[2], SurfaceEvent::DoubleClick));
            DestroyWindow(parent);
        }
    }
}
