#![cfg(target_os = "macos")]

use std::ffi::{c_char, c_void};
use std::fs::{self, OpenOptions};
use std::io::Cursor;
use std::io::Write;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering as AtomicOrdering};
use std::thread;
use std::time::Duration;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSColor, NSScreen, NSStatusWindowLevel, NSWindow, NSWindowAnimationBehavior, NSWindowCollectionBehavior,
};
use tao::platform::macos::WindowExtMacOS;
use tao::window::Window;
use tray_icon::Icon;

use crate::desktop::MonitorArea;
use crate::math::{RectI, Vec2};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *mut c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventSourceButtonState(state_id: u32, button: u32) -> bool;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);

    fn CGMainDisplayID() -> u32;
    fn CGGetActiveDisplayList(max_displays: u32, active_displays: *mut u32, display_count: *mut u32) -> i32;
    fn CGDisplayBounds(display: u32) -> CGRect;

    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *const c_void;
    fn CGRectMakeWithDictionaryRepresentation(dict: *const c_void, rect: *mut CGRect) -> u8;

    static kCGWindowBounds: *const c_void;
    static kCGWindowNumber: *const c_void;
    static kCGWindowOwnerPID: *const c_void;
    static kCGWindowLayer: *const c_void;
    static kCGWindowAlpha: *const c_void;

    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
    fn AXUIElementCopyAttributeValue(
        element: *const c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    fn AXValueGetType(value: *const c_void) -> i32;
    fn AXValueGetValue(value: *const c_void, value_type: i32, value_ptr: *mut c_void) -> bool;

    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *const c_void,
        order: isize,
    ) -> *const c_void;
    fn CFRunLoopGetMain() -> *const c_void;
    fn CFRunLoopAddSource(run_loop: *const c_void, source: *const c_void, mode: *const c_void);
    fn CFDictionaryGetValueIfPresent(
        dict: *const c_void,
        key: *const c_void,
        value: *mut *const c_void,
    ) -> u8;
    fn CFNumberGetValue(number: *const c_void, the_type: i32, value_ptr: *mut c_void) -> u8;
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;

    static kCFBooleanTrue: *const c_void;
    static kCFRunLoopCommonModes: *const c_void;
}

const K_AX_ERROR_SUCCESS: i32 = 0;
const K_AX_VALUE_CGPOINT_TYPE: i32 = 1;
const K_AX_VALUE_CGSIZE_TYPE: i32 = 2;

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: u32 = 0;
const K_CG_MOUSE_BUTTON_LEFT: u32 = 0;
const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const K_CF_NUMBER_SINT32_TYPE: i32 = 3;
const K_CF_NUMBER_FLOAT64_TYPE: i32 = 13;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CG_MENUBAR_WINDOW_LAYER: i32 = 24;
const AX_CG_WINDOW_MATCH_TOLERANCE: i32 = 48;
const EVENT_TAP_CAPTURE_COORD_SCALE: f32 = 16.0;
const ACCESSIBILITY_RECHECK_ATTEMPTS: usize = 12;
const ACCESSIBILITY_RECHECK_DELAY: Duration = Duration::from_millis(250);

static EVENT_TAP_LEFT_MOUSE_DOWN: AtomicBool = AtomicBool::new(false);
static EVENT_TAP_CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
static EVENT_TAP_DRAGGING: AtomicBool = AtomicBool::new(false);
static EVENT_TAP_CAPTURE_X: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_CAPTURE_Y: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_CAPTURE_RADIUS: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_DRAG_ORIGIN_X: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_DRAG_ORIGIN_Y: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_CURSOR_X: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_CURSOR_Y: AtomicI32 = AtomicI32::new(0);
static EVENT_TAP_SERIAL: AtomicU64 = AtomicU64::new(0);

pub struct MouseEventTap {
    tap: *mut c_void,
    source: *const c_void,
}

impl MouseEventTap {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let mask = (1_u64 << K_CG_EVENT_LEFT_MOUSE_DOWN)
                | (1_u64 << K_CG_EVENT_LEFT_MOUSE_UP)
                | (1_u64 << K_CG_EVENT_LEFT_MOUSE_DRAGGED);
            let tap = CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_DEFAULT,
                mask,
                mouse_event_tap_callback,
                null_mut(),
            );
            if tap.is_null() {
                return Err(
                    "failed to create macOS mouse event tap; Accessibility permission is required"
                        .to_string(),
                );
            }

            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                CFRelease(tap as *const c_void);
                return Err("failed to create macOS mouse event tap run loop source".to_string());
            }

            CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            Ok(Self { tap, source })
        }
    }
}

impl Drop for MouseEventTap {
    fn drop(&mut self) {
        unsafe {
            CGEventTapEnable(self.tap, false);
            CFRelease(self.source);
            CFRelease(self.tap as *const c_void);
        }
    }
}

unsafe extern "C" fn mouse_event_tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    _user_info: *mut c_void,
) -> *mut c_void {
    match event_type {
        K_CG_EVENT_LEFT_MOUSE_DOWN => {
            if event_tap_should_capture(event) {
                event_tap_store_point(event, true);
                EVENT_TAP_DRAGGING.store(true, AtomicOrdering::SeqCst);
                EVENT_TAP_LEFT_MOUSE_DOWN.store(true, AtomicOrdering::SeqCst);
                EVENT_TAP_SERIAL.fetch_add(1, AtomicOrdering::SeqCst);
                return null_mut();
            }
            EVENT_TAP_LEFT_MOUSE_DOWN.store(false, AtomicOrdering::SeqCst);
        }
        K_CG_EVENT_LEFT_MOUSE_DRAGGED => {
            if EVENT_TAP_DRAGGING.load(AtomicOrdering::SeqCst) {
                event_tap_store_point(event, false);
                EVENT_TAP_LEFT_MOUSE_DOWN.store(true, AtomicOrdering::SeqCst);
                EVENT_TAP_SERIAL.fetch_add(1, AtomicOrdering::SeqCst);
                return event;
            }
        }
        K_CG_EVENT_LEFT_MOUSE_UP => {
            event_tap_store_point(event, false);
            let dragging = EVENT_TAP_DRAGGING.swap(false, AtomicOrdering::SeqCst);
            EVENT_TAP_LEFT_MOUSE_DOWN.store(false, AtomicOrdering::SeqCst);
            EVENT_TAP_SERIAL.fetch_add(1, AtomicOrdering::SeqCst);
            if dragging {
                return null_mut();
            }
        }
        _ => {}
    }
    event
}

unsafe fn event_tap_store_point(event: *mut c_void, origin: bool) {
    let point = CGEventGetLocation(event);
    let x = (point.x as f32 * EVENT_TAP_CAPTURE_COORD_SCALE).round() as i32;
    let y = (point.y as f32 * EVENT_TAP_CAPTURE_COORD_SCALE).round() as i32;
    EVENT_TAP_CURSOR_X.store(x, AtomicOrdering::SeqCst);
    EVENT_TAP_CURSOR_Y.store(y, AtomicOrdering::SeqCst);
    if origin {
        EVENT_TAP_DRAG_ORIGIN_X.store(x, AtomicOrdering::SeqCst);
        EVENT_TAP_DRAG_ORIGIN_Y.store(y, AtomicOrdering::SeqCst);
    }
}

unsafe fn event_tap_should_capture(event: *mut c_void) -> bool {
    if !EVENT_TAP_CAPTURE_ENABLED.load(AtomicOrdering::SeqCst) {
        return false;
    }
    let radius = EVENT_TAP_CAPTURE_RADIUS.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE;
    if radius <= 0.0 {
        return false;
    }

    let point = CGEventGetLocation(event);
    let x = EVENT_TAP_CAPTURE_X.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE;
    let y = EVENT_TAP_CAPTURE_Y.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE;
    let dx = point.x as f32 - x;
    let dy = point.y as f32 - y;
    dx * dx + dy * dy <= radius * radius
}

pub fn set_event_tap_capture_enabled(enabled: bool) {
    EVENT_TAP_CAPTURE_ENABLED.store(enabled, AtomicOrdering::SeqCst);
}

pub fn set_event_tap_capture_region(enabled: bool, center: Vec2, radius: f32) {
    if enabled {
        EVENT_TAP_CAPTURE_X.store(
            (center.x * EVENT_TAP_CAPTURE_COORD_SCALE).round() as i32,
            AtomicOrdering::SeqCst,
        );
        EVENT_TAP_CAPTURE_Y.store(
            (center.y * EVENT_TAP_CAPTURE_COORD_SCALE).round() as i32,
            AtomicOrdering::SeqCst,
        );
        EVENT_TAP_CAPTURE_RADIUS.store(
            (radius.max(1.0) * EVENT_TAP_CAPTURE_COORD_SCALE).round() as i32,
            AtomicOrdering::SeqCst,
        );
    }
    EVENT_TAP_CAPTURE_ENABLED.store(enabled, AtomicOrdering::SeqCst);
}

pub fn ensure_app_bundle_launch() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|err| format!("failed to resolve Desktop Fushi executable path: {err}"))?;
    if launched_from_app_bundle(&exe) {
        return Ok(());
    }

    Err(
        "Desktop Fushi must be launched as Desktop Fushi.app on macOS. Accessibility permission is tied to the app bundle, so direct cargo/terminal binary launches are not supported."
            .to_string(),
    )
}

fn launched_from_app_bundle(exe: &Path) -> bool {
    let Some(macos_dir) = exe.parent() else {
        return false;
    };
    if macos_dir.file_name().and_then(|name| name.to_str()) != Some("MacOS") {
        return false;
    }
    let Some(contents_dir) = macos_dir.parent() else {
        return false;
    };
    if contents_dir.file_name().and_then(|name| name.to_str()) != Some("Contents") {
        return false;
    }
    contents_dir
        .parent()
        .and_then(|bundle| bundle.extension())
        .and_then(|extension| extension.to_str())
        == Some("app")
}

pub fn ensure_accessibility_permission() -> Result<(), String> {
    if accessibility_trusted(false) {
        return Ok(());
    }
    let _ = accessibility_trusted(true);
    for _ in 0..ACCESSIBILITY_RECHECK_ATTEMPTS {
        thread::sleep(ACCESSIBILITY_RECHECK_DELAY);
        if accessibility_trusted(false) {
            return Ok(());
        }
    }

    let exe = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unknown executable".to_string());
    Err(format!(
        "Desktop Fushi requires macOS Accessibility permission for this app bundle. Grant it in System Settings, then restart Desktop Fushi. Executable: {exe}"
    ))
}

fn accessibility_trusted(prompt: bool) -> bool {
    unsafe {
        if !prompt {
            return AXIsProcessTrustedWithOptions(std::ptr::null());
        }

        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        );
        if options.is_null() {
            return AXIsProcessTrustedWithOptions(std::ptr::null());
        }
        let trusted = AXIsProcessTrustedWithOptions(options);
        CFRelease(options);
        trusted
    }
}

pub fn cursor_pos() -> Option<Vec2> {
    unsafe {
        let event = CGEventCreate(null_mut());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event as *const c_void);
        Some(Vec2::new(point.x as f32, point.y as f32))
    }
}

pub fn left_mouse_down() -> bool {
    if EVENT_TAP_LEFT_MOUSE_DOWN.load(AtomicOrdering::SeqCst) {
        return true;
    }
    unsafe {
        CGEventSourceButtonState(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
            K_CG_MOUSE_BUTTON_LEFT,
        )
    }
}

pub fn event_tap_left_mouse_down() -> bool {
    EVENT_TAP_LEFT_MOUSE_DOWN.load(AtomicOrdering::SeqCst)
}

pub fn event_tap_drag_origin() -> Option<Vec2> {
    if !EVENT_TAP_DRAGGING.load(AtomicOrdering::SeqCst) {
        return None;
    }
    Some(Vec2::new(
        EVENT_TAP_DRAG_ORIGIN_X.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE,
        EVENT_TAP_DRAG_ORIGIN_Y.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE,
    ))
}

pub fn event_tap_cursor_pos() -> Option<Vec2> {
    if !EVENT_TAP_DRAGGING.load(AtomicOrdering::SeqCst)
        && !EVENT_TAP_LEFT_MOUSE_DOWN.load(AtomicOrdering::SeqCst)
    {
        return None;
    }
    Some(Vec2::new(
        EVENT_TAP_CURSOR_X.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE,
        EVENT_TAP_CURSOR_Y.load(AtomicOrdering::SeqCst) as f32 / EVENT_TAP_CAPTURE_COORD_SCALE,
    ))
}

pub fn debug_enabled() -> bool {
    std::env::var_os("DESKTOP_FUSHI_DEBUG").is_some()
}

pub fn reset_debug_log() {
    if !debug_enabled() {
        return;
    }
    if let Some(path) = debug_log_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, "");
    }
}

pub fn append_debug_log(line: &str) {
    if !debug_enabled() {
        return;
    }
    let Some(path) = debug_log_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn debug_log_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    Some(
        home.join("Library")
            .join("Logs")
            .join("Desktop Fushi")
            .join("debug.log"),
    )
}

pub fn configure_pet_window(window: &Window) {
    unsafe {
        let Some(ns_window) = (window.ns_window() as *mut NSWindow).as_ref() else {
            return;
        };

        ns_window.setOpaque(false);
        let clear = NSColor::clearColor();
        ns_window.setBackgroundColor(Some(&clear));
        ns_window.setHasShadow(false);
        ns_window.setLevel(NSStatusWindowLevel);
        ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
    }
}

pub fn set_click_through(window: &Window, enabled: bool) {
    let _ = window.set_ignore_cursor_events(enabled);
    unsafe {
        if let Some(ns_window) = (window.ns_window() as *mut NSWindow).as_ref() {
            ns_window.setIgnoresMouseEvents(enabled);
        }
    }
}

pub fn load_status_icon(png_bytes: &'static [u8]) -> Result<Icon, String> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|err| format!("failed to read macOS status icon: {err}"))?;
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "macOS status icon is too large to decode".to_string())?;
    let mut decoded = vec![0; output_size];
    let info = reader
        .next_frame(&mut decoded)
        .map_err(|err| format!("failed to decode macOS status icon: {err}"))?;
    let bytes = &decoded[..info.buffer_size()];
    let rgba = match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgba, png::BitDepth::Eight) => bytes.to_vec(),
        (png::ColorType::Rgb, png::BitDepth::Eight) => {
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for pixel in bytes.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
            rgba
        }
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => {
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for pixel in bytes.chunks_exact(2) {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            rgba
        }
        (png::ColorType::Grayscale, png::BitDepth::Eight) => {
            let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
            for &gray in bytes {
                rgba.extend_from_slice(&[gray, gray, gray, 255]);
            }
            rgba
        }
        _ => {
            return Err(format!(
                "unsupported macOS status icon format: {:?} {:?}",
                info.color_type, info.bit_depth
            ));
        }
    };

    Icon::from_rgba(rgba, info.width, info.height)
        .map_err(|err| format!("failed to create macOS status icon: {err}"))
}

pub fn monitor_areas() -> Vec<MonitorArea> {
    unsafe {
        let mut displays = [0u32; 32];
        let mut count = 0u32;
        let result = CGGetActiveDisplayList(displays.len() as u32, displays.as_mut_ptr(), &mut count);
        if result != 0 || count == 0 {
            return Vec::new();
        }

        let primary = CGMainDisplayID();
        let mut monitors: Vec<_> = displays
            .iter()
            .take(count as usize)
            .filter_map(|display| {
                let rect = rect_from_cg(CGDisplayBounds(*display));
                if rect.width() <= 0 || rect.height() <= 0 {
                    return None;
                }
                Some(MonitorArea {
                    bounds: rect,
                    // Querying the exact macOS visible frame needs AppKit.  The full display is a
                    // safe fallback for a borderless agent window and keeps the pet inside screen space.
                    work: rect,
                    primary: *display == primary,
                })
            })
            .collect();
        if !apply_screen_visible_work_area(&mut monitors) {
            apply_menu_bar_work_area(&mut monitors);
        }
        monitors
    }
}

#[derive(Clone, Copy, Debug)]
struct ScreenVisibleInsets {
    frame_width: i32,
    frame_height: i32,
    frame_x: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn apply_screen_visible_work_area(monitors: &mut [MonitorArea]) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };

    let screens = NSScreen::screens(mtm);
    let mut insets = Vec::new();
    for screen in screens.iter() {
        let frame = screen.frame();
        let visible = screen.visibleFrame();
        if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
            continue;
        }

        let frame_right = frame.origin.x + frame.size.width;
        let frame_top = frame.origin.y + frame.size.height;
        let visible_right = visible.origin.x + visible.size.width;
        let visible_top = visible.origin.y + visible.size.height;
        insets.push(ScreenVisibleInsets {
            frame_width: frame.size.width.round() as i32,
            frame_height: frame.size.height.round() as i32,
            frame_x: frame.origin.x.round() as i32,
            left: (visible.origin.x - frame.origin.x).round().max(0.0) as i32,
            top: (frame_top - visible_top).round().max(0.0) as i32,
            right: (frame_right - visible_right).round().max(0.0) as i32,
            bottom: (visible.origin.y - frame.origin.y).round().max(0.0) as i32,
        });
    }

    if insets.is_empty() {
        return false;
    }

    let mut used = vec![false; insets.len()];
    let mut applied = false;
    for monitor in monitors.iter_mut() {
        let Some(index) = best_visible_insets_for_monitor(monitor.bounds, &insets, &used) else {
            continue;
        };
        used[index] = true;
        let inset = insets[index];
        let work = RectI::new(
            (monitor.bounds.left + inset.left).min(monitor.bounds.right),
            (monitor.bounds.top + inset.top).min(monitor.bounds.bottom),
            (monitor.bounds.right - inset.right).max(monitor.bounds.left),
            (monitor.bounds.bottom - inset.bottom).max(monitor.bounds.top),
        );
        if work.width() > 64 && work.height() > 64 {
            monitor.work = work;
            applied = true;
        }
    }
    applied
}

fn best_visible_insets_for_monitor(
    bounds: RectI,
    insets: &[ScreenVisibleInsets],
    used: &[bool],
) -> Option<usize> {
    insets
        .iter()
        .enumerate()
        .filter(|(index, _)| !used.get(*index).copied().unwrap_or(false))
        .min_by_key(|(_, inset)| {
            (bounds.width() - inset.frame_width).abs()
                + (bounds.height() - inset.frame_height).abs()
                + ((bounds.left - inset.frame_x).abs() / 2)
        })
        .map(|(index, _)| index)
}

fn apply_menu_bar_work_area(monitors: &mut [MonitorArea]) {
    for menu_bar in menu_bar_rects() {
        if menu_bar.height() < 16 || menu_bar.height() > 96 {
            continue;
        }
        for monitor in monitors.iter_mut() {
            if !rects_intersect(menu_bar, monitor.bounds) {
                continue;
            }
            monitor.work.top = monitor.work.top.max(menu_bar.bottom);
        }
    }
}

fn menu_bar_rects() -> Vec<RectI> {
    unsafe {
        let options = K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
        let list = CGWindowListCopyWindowInfo(options, 0);
        if list.is_null() {
            return Vec::new();
        }

        let mut rects = Vec::new();
        let count = CFArrayGetCount(list);
        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(list, i);
            if dict.is_null() {
                continue;
            }
            if cf_i32(dict, kCGWindowLayer).unwrap_or(0) != K_CG_MENUBAR_WINDOW_LAYER {
                continue;
            }
            if let Some(rect) = cg_window_rect(dict) {
                rects.push(rect);
            }
        }
        CFRelease(list);
        rects
    }
}

pub fn visible_window_rects() -> Vec<(isize, RectI)> {
    if !accessibility_trusted(false) {
        return Vec::new();
    }

    let monitors = monitor_areas();
    unsafe {
        let options = K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
        let list = CGWindowListCopyWindowInfo(options, 0);
        if list.is_null() {
            return Vec::new();
        }

        let mut windows = Vec::new();
        let current_pid = std::process::id() as i32;
        let count = CFArrayGetCount(list);
        for i in 0..count {
            let dict = CFArrayGetValueAtIndex(list, i);
            if dict.is_null() {
                continue;
            }

            let owner_pid = cf_i32(dict, kCGWindowOwnerPID).unwrap_or(-1);
            if owner_pid == current_pid {
                continue;
            }
            if cf_i32(dict, kCGWindowLayer).unwrap_or(0) != 0 {
                continue;
            }
            if cf_f64(dict, kCGWindowAlpha).unwrap_or(1.0) <= 0.05 {
                continue;
            }

            let Some(rect) = cg_window_rect(dict) else {
                continue;
            };
            if rect.width() < 96 || rect.height() < 48 {
                continue;
            }
            if !window_intersects_any_monitor(rect, &monitors) {
                continue;
            }
            if window_center_covered_by_upper_windows(rect, &windows) {
                continue;
            }

            let Some(window_number) = cf_i32(dict, kCGWindowNumber) else {
                continue;
            };
            let Some(rect) = ax_window_rect_matching(owner_pid, window_number, rect) else {
                continue;
            };

            windows.push((window_number as isize, rect));
        }
        CFRelease(list);
        windows
    }
}

fn window_intersects_any_monitor(rect: RectI, monitors: &[MonitorArea]) -> bool {
    if monitors.is_empty() {
        return true;
    }

    monitors
        .iter()
        .any(|monitor| rects_intersect(rect, monitor.bounds.inflate(64)))
}

fn rects_intersect(a: RectI, b: RectI) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

fn window_center_covered_by_upper_windows(rect: RectI, upper_windows: &[(isize, RectI)]) -> bool {
    let center = Vec2::new(
        (rect.left + rect.right) as f32 * 0.5,
        (rect.top + rect.bottom) as f32 * 0.5,
    );
    upper_windows
        .iter()
        .any(|(_, upper)| upper.inflate(-8).contains(center))
}

fn ax_window_rect_matching(pid: i32, cg_window_number: i32, cg_rect: RectI) -> Option<RectI> {
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return None;
        }

        let windows_attr = cf_string(b"AXWindows\0");
        if windows_attr.is_null() {
            CFRelease(app);
            return None;
        }

        let mut windows_value: *const c_void = std::ptr::null();
        let result = AXUIElementCopyAttributeValue(app, windows_attr, &mut windows_value);
        CFRelease(windows_attr);
        CFRelease(app);
        if result != K_AX_ERROR_SUCCESS || windows_value.is_null() {
            return None;
        }

        let count = CFArrayGetCount(windows_value);
        let mut saw_window_number = false;
        let mut best: Option<(RectI, i32)> = None;
        for i in 0..count {
            let window = CFArrayGetValueAtIndex(windows_value, i);
            if window.is_null() {
                continue;
            }
            if let Some(window_number) = ax_window_number(window) {
                saw_window_number = true;
                if window_number == cg_window_number {
                    let rect = ax_window_rect(window);
                    CFRelease(windows_value);
                    return rect;
                }
                continue;
            }
            let Some(rect) = ax_window_rect(window) else {
                continue;
            };
            let score = ax_cg_rect_match_score(cg_rect, rect);
            if score <= AX_CG_WINDOW_MATCH_TOLERANCE {
                let replace = best
                    .as_ref()
                    .map(|(_, best_score)| score < *best_score)
                    .unwrap_or(true);
                if replace {
                    best = Some((rect, score));
                }
            }
        }
        CFRelease(windows_value);
        if saw_window_number {
            return None;
        }
        best.map(|(rect, _)| rect)
    }
}

unsafe fn ax_window_number(window: *const c_void) -> Option<i32> {
    let number_attr = cf_string(b"AXWindowNumber\0");
    if number_attr.is_null() {
        return None;
    }
    let mut number_value: *const c_void = std::ptr::null();
    let result = AXUIElementCopyAttributeValue(window, number_attr, &mut number_value);
    CFRelease(number_attr);
    if result != K_AX_ERROR_SUCCESS || number_value.is_null() {
        return None;
    }
    let number = cf_number_i32(number_value);
    CFRelease(number_value);
    number
}

unsafe fn ax_window_rect(window: *const c_void) -> Option<RectI> {
    let position_attr = cf_string(b"AXPosition\0");
    if position_attr.is_null() {
        return None;
    }
    let mut position_value: *const c_void = std::ptr::null();
    let result = AXUIElementCopyAttributeValue(window, position_attr, &mut position_value);
    CFRelease(position_attr);
    if result != K_AX_ERROR_SUCCESS || position_value.is_null() {
        return None;
    }
    let position = ax_value_point(position_value);
    CFRelease(position_value);

    let size_attr = cf_string(b"AXSize\0");
    if size_attr.is_null() {
        return None;
    }
    let mut size_value: *const c_void = std::ptr::null();
    let result = AXUIElementCopyAttributeValue(window, size_attr, &mut size_value);
    CFRelease(size_attr);
    if result != K_AX_ERROR_SUCCESS || size_value.is_null() {
        return None;
    }
    let size = ax_value_size(size_value);
    CFRelease(size_value);

    let position = position?;
    let size = size?;
    let left = position.x.round() as i32;
    let top = position.y.round() as i32;
    let right = (position.x + size.width).round() as i32;
    let bottom = (position.y + size.height).round() as i32;
    let rect = RectI::new(left, top, right, bottom);
    if rect.width() <= 0 || rect.height() <= 0 {
        None
    } else {
        Some(rect)
    }
}

unsafe fn ax_value_point(value: *const c_void) -> Option<CGPoint> {
    if AXValueGetType(value) != K_AX_VALUE_CGPOINT_TYPE {
        return None;
    }
    let mut point = CGPoint::default();
    if AXValueGetValue(
        value,
        K_AX_VALUE_CGPOINT_TYPE,
        &mut point as *mut CGPoint as *mut c_void,
    ) {
        Some(point)
    } else {
        None
    }
}

unsafe fn ax_value_size(value: *const c_void) -> Option<CGSize> {
    if AXValueGetType(value) != K_AX_VALUE_CGSIZE_TYPE {
        return None;
    }
    let mut size = CGSize::default();
    if AXValueGetValue(
        value,
        K_AX_VALUE_CGSIZE_TYPE,
        &mut size as *mut CGSize as *mut c_void,
    ) {
        Some(size)
    } else {
        None
    }
}

fn ax_cg_rect_match_score(cg: RectI, ax: RectI) -> i32 {
    let cg_center_x = cg.left + cg.width() / 2;
    let cg_center_y = cg.top + cg.height() / 2;
    let ax_center_x = ax.left + ax.width() / 2;
    let ax_center_y = ax.top + ax.height() / 2;
    let center_delta = (cg_center_x - ax_center_x).abs() + (cg_center_y - ax_center_y).abs();
    let size_delta = (cg.width() - ax.width()).abs() + (cg.height() - ax.height()).abs();
    (center_delta + size_delta / 2).max(0)
}

unsafe fn cf_string(bytes: &'static [u8]) -> *const c_void {
    CFStringCreateWithCString(
        std::ptr::null(),
        bytes.as_ptr() as *const c_char,
        K_CF_STRING_ENCODING_UTF8,
    )
}

unsafe fn cg_window_rect(dict: *const c_void) -> Option<RectI> {
    let bounds = dict_value(dict, kCGWindowBounds)?;
    let mut rect = CGRect::default();
    if CGRectMakeWithDictionaryRepresentation(bounds, &mut rect) == 0 {
        return None;
    }
    Some(rect_from_cg(rect))
}

unsafe fn cf_i32(dict: *const c_void, key: *const c_void) -> Option<i32> {
    let value = dict_value(dict, key)?;
    let mut out = 0i32;
    if CFNumberGetValue(
        value,
        K_CF_NUMBER_SINT32_TYPE,
        &mut out as *mut i32 as *mut c_void,
    ) == 0
    {
        return None;
    }
    Some(out)
}

unsafe fn cf_f64(dict: *const c_void, key: *const c_void) -> Option<f64> {
    let value = dict_value(dict, key)?;
    let mut out = 0.0f64;
    if CFNumberGetValue(
        value,
        K_CF_NUMBER_FLOAT64_TYPE,
        &mut out as *mut f64 as *mut c_void,
    ) == 0
    {
        return None;
    }
    Some(out)
}

unsafe fn cf_number_i32(number: *const c_void) -> Option<i32> {
    let mut out = 0i32;
    if CFNumberGetValue(
        number,
        K_CF_NUMBER_SINT32_TYPE,
        &mut out as *mut i32 as *mut c_void,
    ) == 0
    {
        return None;
    }
    Some(out)
}

unsafe fn dict_value(dict: *const c_void, key: *const c_void) -> Option<*const c_void> {
    let mut value: *const c_void = std::ptr::null();
    if CFDictionaryGetValueIfPresent(dict, key, &mut value) == 0 || value.is_null() {
        return None;
    }
    Some(value)
}

fn rect_from_cg(rect: CGRect) -> RectI {
    let left = rect.origin.x.round() as i32;
    let top = rect.origin.y.round() as i32;
    let right = (rect.origin.x + rect.size.width).round() as i32;
    let bottom = (rect.origin.y + rect.size.height).round() as i32;
    RectI::new(left, top, right, bottom)
}
