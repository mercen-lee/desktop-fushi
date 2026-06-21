use crate::math::{RectI, Vec2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
use std::ptr;
use windows::core::{w, BOOL};
use windows::Win32::Foundation::{
    COLORREF, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND, LPARAM, POINT, RECT, SIZE,
};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetMonitorInfoW, MonitorFromWindow,
    SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetForegroundWindow, GetShellWindow, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindowVisible, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    UpdateLayeredWindow, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST, NID_READY, SM_DIGITIZER, SM_MAXIMUMTOUCHES,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA,
    WS_CAPTION, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};

pub fn hwnd_from_window(window: &impl HasWindowHandle) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

pub fn start_on_login_enabled() -> Result<bool, String> {
    unsafe {
        let mut bytes = 0u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            w!("Desktop Fushi"),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        );
        if status == ERROR_SUCCESS {
            Ok(true)
        } else if status == ERROR_FILE_NOT_FOUND {
            Ok(false)
        } else {
            Err(format!("failed to read startup setting: {}", status.0))
        }
    }
}

pub fn set_start_on_login(enabled: bool) -> Result<(), String> {
    unsafe {
        let key = open_startup_run_key(KEY_SET_VALUE)?;
        let result = if enabled {
            let command = startup_command()?;
            let bytes = wide_bytes(&command);
            let status = RegSetValueExW(key, w!("Desktop Fushi"), None, REG_SZ, Some(bytes));
            if status == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(format!("failed to enable startup setting: {}", status.0))
            }
        } else {
            let status = RegDeleteValueW(key, w!("Desktop Fushi"));
            if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                Err(format!("failed to disable startup setting: {}", status.0))
            }
        };
        let _ = RegCloseKey(key);
        result
    }
}

unsafe fn open_startup_run_key(
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> Result<HKEY, String> {
    let mut key = HKEY::default();
    let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
        None,
        access,
        &mut key,
    );
    if status == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(format!("failed to open startup settings key: {}", status.0))
    }
}

fn startup_command() -> Result<Vec<u16>, String> {
    let exe = std::env::current_exe().map_err(|err| format!("failed to locate current executable: {err}"))?;
    let command = format!("\"{}\"", exe.display());
    Ok(command.encode_utf16().chain(std::iter::once(0)).collect())
}

fn wide_bytes(wide: &[u16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, std::mem::size_of_val(wide)) }
}

pub unsafe fn cursor_pos() -> Vec2 {
    let mut p = POINT::default();
    if GetCursorPos(&mut p).is_ok() {
        Vec2::new(p.x as f32, p.y as f32)
    } else {
        Vec2::ZERO
    }
}

pub unsafe fn foreground_fullscreen_except(own_hwnd: HWND) -> bool {
    let fg = GetForegroundWindow();
    if fg.is_invalid() || fg == own_hwnd {
        return false;
    }
    if is_shell_desktop_window(fg) {
        return false;
    }
    if !IsWindowVisible(fg).as_bool() || IsIconic(fg).as_bool() {
        return false;
    }
    let Some(wr) = window_visible_frame_rect(fg) else {
        return false;
    };
    let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
    if monitor.is_invalid() {
        return false;
    }
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
        return false;
    }
    let mb = mi.rcMonitor;
    let tolerance = 2;
    wr.left <= mb.left + tolerance
        && wr.top <= mb.top + tolerance
        && wr.right >= mb.right - tolerance
        && wr.bottom >= mb.bottom - tolerance
}

pub unsafe fn left_mouse_down() -> bool {
    GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0
}

pub unsafe fn touch_input_available() -> bool {
    let digitizer = GetSystemMetrics(SM_DIGITIZER) as u32;
    digitizer & NID_READY != 0 && GetSystemMetrics(SM_MAXIMUMTOUCHES) > 0
}

pub unsafe fn configure_pet_window(hwnd: HWND) {
    ensure_pet_window_styles(hwnd);
}

pub unsafe fn ensure_pet_window_styles(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let desired_style = desired_pet_style(style);
    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    let desired_ex_style = desired_pet_ex_style(ex_style);
    let changed = style != desired_style || ex_style != desired_ex_style;

    if style != desired_style {
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, desired_style);
    }
    if ex_style != desired_ex_style {
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired_ex_style);
    }

    let mut flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    if changed {
        flags |= SWP_FRAMECHANGED;
    }
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
}

pub unsafe fn set_click_through(hwnd: HWND, enabled: bool) {
    let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    style |= WS_EX_LAYERED.0 as isize;
    if enabled {
        style |= WS_EX_TRANSPARENT.0 as isize;
    } else {
        style &= !(WS_EX_TRANSPARENT.0 as isize);
    }
    let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
}

pub unsafe fn show_pet_window(hwnd: HWND) {
    ensure_pet_window_styles(hwnd);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    ensure_pet_window_styles(hwnd);
}

pub unsafe fn hide_pet_window(hwnd: HWND) {
    let _ = ShowWindow(hwnd, SW_HIDE);
}

fn desired_pet_style(style: isize) -> isize {
    let remove = (WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0;
    (style & !(remove as isize)) | WS_POPUP.0 as isize
}

fn desired_pet_ex_style(style: isize) -> isize {
    let mut style = style;
    style |= WS_EX_LAYERED.0 as isize;
    style |= WS_EX_NOACTIVATE.0 as isize;
    style |= WS_EX_TOOLWINDOW.0 as isize;
    style &= !(WS_EX_APPWINDOW.0 as isize);
    style
}

pub struct LayeredWindowBuffer {
    width: u32,
    height: u32,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old_object: HGDIOBJ,
    bits: *mut c_void,
}

impl LayeredWindowBuffer {
    pub unsafe fn new(width: u32, height: u32) -> Result<Self, String> {
        let mem_dc = CreateCompatibleDC(None);
        if mem_dc.0.is_null() {
            return Err("CreateCompatibleDC failed".to_string());
        }
        let expected_len = width as usize * height as usize * 4;
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: expected_len as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = match CreateDIBSection(
            None,
            &mut bitmap_info as *mut BITMAPINFO,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) {
            Ok(bitmap) => bitmap,
            Err(err) => {
                let _ = DeleteDC(mem_dc);
                return Err(format!("CreateDIBSection failed: {err}"));
            }
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);
            return Err("CreateDIBSection returned a null pixel buffer".to_string());
        }

        let old_object = SelectObject(mem_dc, HGDIOBJ(bitmap.0));

        Ok(Self {
            width,
            height,
            mem_dc,
            bitmap,
            old_object,
            bits,
        })
    }

    pub fn matches(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }

    pub unsafe fn update(
        &mut self,
        hwnd: HWND,
        x: i32,
        y: i32,
        bgra_premultiplied: &[u8],
    ) -> Result<(), String> {
        ensure_pet_window_styles(hwnd);

        let expected_len = self.width as usize * self.height as usize * 4;
        if bgra_premultiplied.len() != expected_len {
            return Err(format!(
                "layered frame size mismatch: got {}, expected {}",
                bgra_premultiplied.len(),
                expected_len
            ));
        }
        ptr::copy_nonoverlapping(
            bgra_premultiplied.as_ptr(),
            self.bits as *mut u8,
            bgra_premultiplied.len(),
        );

        let dst = POINT { x, y };
        let size = SIZE {
            cx: self.width as i32,
            cy: self.height as i32,
        };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let result = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&dst),
            Some(&size),
            Some(self.mem_dc),
            Some(&src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        result.map_err(|err| format!("UpdateLayeredWindow failed: {err}"))
    }
}

impl Drop for LayeredWindowBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.old_object.0.is_null() {
                let _ = SelectObject(self.mem_dc, self.old_object);
            }
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.mem_dc);
        }
    }
}

pub unsafe fn visible_window_rects_excluding(excluded_hwnds: &[HWND]) -> Vec<(isize, RectI)> {
    struct WindowEnumState {
        excluded_hwnds: Vec<HWND>,
        windows: Vec<(isize, RectI)>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut WindowEnumState);
        if state.excluded_hwnds.contains(&hwnd)
            || hwnd.is_invalid()
            || is_shell_desktop_window(hwnd)
            || is_cloaked_window(hwnd)
            || !IsWindowVisible(hwnd).as_bool()
            || IsIconic(hwnd).as_bool()
        {
            return BOOL(1);
        }

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if ex_style & (WS_EX_TOOLWINDOW.0 as isize) != 0
            || ex_style & (WS_EX_NOACTIVATE.0 as isize) != 0
            || ex_style & (WS_EX_TRANSPARENT.0 as isize) != 0
        {
            return BOOL(1);
        }

        let class_name = window_class_name(hwnd);
        if is_ignored_window_class(&class_name) {
            return BOOL(1);
        }

        let Some(rect) = window_visible_frame_rect(hwnd) else {
            return BOOL(1);
        };
        if is_fullscreen_rect(hwnd, rect) {
            return BOOL(1);
        }
        if rect.width() < 96 || rect.height() < 48 {
            return BOOL(1);
        }

        state.windows.push((hwnd.0 as isize, rect));
        BOOL(1)
    }

    let mut state = WindowEnumState {
        excluded_hwnds: excluded_hwnds.to_vec(),
        windows: Vec::new(),
    };
    let _ = EnumWindows(
        Some(enum_proc),
        LPARAM((&mut state as *mut WindowEnumState) as isize),
    );
    state.windows
}

unsafe fn is_fullscreen_rect(hwnd: HWND, wr: RectI) -> bool {
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if monitor.is_invalid() {
        return false;
    }
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
        return false;
    }
    let mb = mi.rcMonitor;
    let tolerance = 2;
    wr.left <= mb.left + tolerance
        && wr.top <= mb.top + tolerance
        && wr.right >= mb.right - tolerance
        && wr.bottom >= mb.bottom - tolerance
}

unsafe fn window_visible_frame_rect(hwnd: HWND) -> Option<RectI> {
    let mut frame = RECT::default();
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut frame as *mut RECT as *mut c_void,
        std::mem::size_of::<RECT>() as u32,
    )
    .is_ok()
    {
        let rect = rect_from_win32(frame);
        if is_valid_rect(rect) {
            return Some(rect);
        }
    }

    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_ok() {
        let rect = rect_from_win32(rect);
        if is_valid_rect(rect) {
            return Some(rect);
        }
    }

    None
}

fn rect_from_win32(rect: RECT) -> RectI {
    RectI::new(rect.left, rect.top, rect.right, rect.bottom)
}

fn is_valid_rect(rect: RectI) -> bool {
    rect.right > rect.left && rect.bottom > rect.top
}

unsafe fn is_cloaked_window(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as *mut c_void,
        std::mem::size_of::<u32>() as u32,
    )
    .is_ok()
        && cloaked != 0
}

unsafe fn is_shell_desktop_window(hwnd: HWND) -> bool {
    let shell = GetShellWindow();
    if !shell.is_invalid() && hwnd == shell {
        return true;
    }

    let name = window_class_name(hwnd);
    matches!(name.as_str(), "Progman" | "WorkerW")
}

unsafe fn window_class_name(hwnd: HWND) -> String {
    let mut class_name = [0u16; 64];
    let len = GetClassNameW(hwnd, &mut class_name);
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&class_name[..len as usize])
}

fn is_ignored_window_class(name: &str) -> bool {
    matches!(
        name,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "DesktopFushiWgpuWindow"
    )
}
