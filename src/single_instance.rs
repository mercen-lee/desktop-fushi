use windows::core::{BOOL, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, WAIT_ABANDONED, WAIT_OBJECT_0,
    WPARAM,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, ReleaseMutex, ResetEvent, SetEvent, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, PostMessageW, WM_CLOSE};

const GLOBAL_MUTEX_NAME: &str = "Local\\DesktopFushi.SingleInstance";
const RESTART_EVENT_NAME: &str = "Local\\DesktopFushi.RestartRequested";
const RESTART_WAIT_MS: u32 = 10_000;

pub struct SingleInstanceGuard {
    global_mutex: HANDLE,
    version_mutex: HANDLE,
}

pub struct RestartEvent(HANDLE);

pub fn acquire() -> Option<SingleInstanceGuard> {
    unsafe {
        let (global_mutex, global_exists) = create_mutex(GLOBAL_MUTEX_NAME)?;
        if !global_exists {
            reset_restart_event();
            return create_guard(global_mutex);
        }

        if current_version_is_running() {
            let _ = CloseHandle(global_mutex);
            return None;
        }

        request_existing_instance_exit();
        let wait = WaitForSingleObject(global_mutex, RESTART_WAIT_MS);
        if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
            reset_restart_event();
            create_guard(global_mutex)
        } else {
            let _ = CloseHandle(global_mutex);
            None
        }
    }
}

impl RestartEvent {
    pub fn new() -> Option<Self> {
        unsafe { create_restart_event().map(Self) }
    }

    pub fn requested(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, 0) == WAIT_OBJECT_0 }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.version_mutex);
            let _ = CloseHandle(self.version_mutex);
            let _ = ReleaseMutex(self.global_mutex);
            let _ = CloseHandle(self.global_mutex);
        }
    }
}

impl Drop for RestartEvent {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

unsafe fn create_guard(global_mutex: HANDLE) -> Option<SingleInstanceGuard> {
    let (version_mutex, version_exists) = create_mutex(&version_mutex_name())?;
    if version_exists {
        let _ = CloseHandle(version_mutex);
        let _ = ReleaseMutex(global_mutex);
        let _ = CloseHandle(global_mutex);
        return None;
    }

    Some(SingleInstanceGuard {
        global_mutex,
        version_mutex,
    })
}

unsafe fn current_version_is_running() -> bool {
    let Some((version_mutex, version_exists)) = create_mutex(&version_mutex_name()) else {
        return false;
    };
    if !version_exists {
        let _ = ReleaseMutex(version_mutex);
    }
    let _ = CloseHandle(version_mutex);
    version_exists
}

unsafe fn create_mutex(name: &str) -> Option<(HANDLE, bool)> {
    let name = wide_null(name);
    let handle = CreateMutexW(None, true, PCWSTR(name.as_ptr())).ok()?;
    Some((handle, GetLastError() == ERROR_ALREADY_EXISTS))
}

unsafe fn create_restart_event() -> Option<HANDLE> {
    let name = wide_null(RESTART_EVENT_NAME);
    CreateEventW(None, true, false, PCWSTR(name.as_ptr())).ok()
}

unsafe fn reset_restart_event() {
    if let Some(event) = create_restart_event() {
        let _ = ResetEvent(event);
        let _ = CloseHandle(event);
    }
}

unsafe fn request_existing_instance_exit() {
    if let Some(event) = create_restart_event() {
        let _ = SetEvent(event);
        let _ = CloseHandle(event);
    }
    post_close_to_existing_windows();
}

unsafe fn post_close_to_existing_windows() {
    unsafe extern "system" fn enum_proc(hwnd: HWND, _lparam: LPARAM) -> BOOL {
        unsafe {
            if window_title(hwnd) == "Desktop Fushi" {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        BOOL(1)
    }

    let _ = EnumWindows(Some(enum_proc), LPARAM(0));
}

unsafe fn window_title(hwnd: HWND) -> String {
    let mut title = [0u16; 128];
    let len = GetWindowTextW(hwnd, &mut title);
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&title[..len as usize])
}

fn version_mutex_name() -> String {
    format!(
        "Local\\DesktopFushi.SingleInstance.Version.{}",
        mutex_name_component(env!("CARGO_PKG_VERSION"))
    )
}

fn mutex_name_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
