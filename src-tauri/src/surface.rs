//! Native video surface: a child window of the app window that libVLC renders
//! into (Direct3D11 output). The web UI reports the rectangle it reserves for
//! video and we keep this window positioned above the webview at that spot.

#[cfg(windows)]
mod imp {
    use std::ptr::null;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH, HBRUSH};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Must be called on the UI thread.
    pub fn create(parent: isize) -> Result<isize, String> {
        unsafe {
            let class = wide("HalcyonVideoSurface");
            let hinst = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(DefWindowProcW),
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
                0,
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
    pub fn place(hwnd: isize, x: i32, y: i32, w: i32, h: i32, visible: bool) {
        unsafe {
            let flag = if visible && w > 0 && h > 0 { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
            SetWindowPos(hwnd as HWND, HWND_TOP, x, y, w.max(1), h.max(1), SWP_NOACTIVATE | flag);
        }
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
pub fn create(_parent: isize) -> Result<isize, String> {
    Err("Native video surface is only implemented on Windows so far".into())
}
#[cfg(not(windows))]
pub fn place(_h: isize, _x: i32, _y: i32, _w: i32, _hh: i32, _v: bool) {}
