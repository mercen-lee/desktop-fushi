#![cfg(target_os = "android")]

use jni::objects::{JClass, JFloatArray, JObject};
use jni::sys::{jboolean, jfloat, jint, jlong};
use jni::JNIEnv;
use raw_window_handle::{
    AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    WindowHandle,
};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::desktop::{DesktopEnvironment, SurfaceContact, SurfaceKind};
use crate::fushi::constants::{HUGE_FUSHI_SCALE, LARGE_FUSHI_SCALE, NORMAL_FUSHI_SCALE, SMALL_FUSHI_SCALE};
use crate::fushi::motion_input::{FrameMotion, MotionInput, SensorAvailability, SensorKind};
use crate::fushi::render::FushiRenderer;
use crate::fushi::{FushiBody, MotionMode};
use crate::gpu_canvas::GpuCanvas;
use crate::math::{RectI, Vec2};
use crate::wgpu_layer::{android_backend_available, WgpuLayer, WgpuSurfaceSize};

const MAX_SIMULATION_STEP: f32 = 1.0 / 120.0;
const FAR_CURSOR: Vec2 = Vec2::new(-10000.0, -10000.0);
const DRAG_START_TIMEOUT: Duration = Duration::from_millis(80);
const SURFACE_ATTACH_TIMEOUT: Duration = Duration::from_secs(4);
const DRAG_REQUEST_PENDING: u8 = 0;
const DRAG_REQUEST_CLAIMED: u8 = 1;
const DRAG_REQUEST_CANCELLED: u8 = 2;
const DRAG_REQUEST_COMPLETED: u8 = 3;
const ANDROID_WINDOW_VISUAL_MARGIN: f32 = 32.0;
const ANDROID_WINDOW_SIZE_QUANTUM: u32 = 16;
const ANDROID_MIN_WINDOW_SIZE: u32 = 96;
const ANDROID_FUSHI_SCALE_FACTOR: f32 = 0.50;
const ANDROID_MONITOR_CORNER_PADDING: f32 = 1.08;

#[derive(Clone, Copy, Debug)]
enum AndroidGraphicsBackend {
    Vulkan,
    Gles,
}

impl AndroidGraphicsBackend {
    fn from_raw(value: jint) -> Result<Self, String> {
        match value {
            0 => Ok(Self::Vulkan),
            1 => Ok(Self::Gles),
            _ => Err(format!("unsupported Android graphics backend: {value}")),
        }
    }

    fn wgpu_backends(self) -> wgpu::Backends {
        match self {
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Gles => wgpu::Backends::GL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AndroidFushiSizePreset {
    Small,
    Normal,
    Large,
    Huge,
}

impl AndroidFushiSizePreset {
    fn from_raw(value: jint) -> Self {
        match value {
            0 => Self::Small,
            2 => Self::Large,
            3 => Self::Huge,
            _ => Self::Normal,
        }
    }

    fn desktop_scale(self) -> f32 {
        match self {
            Self::Small => SMALL_FUSHI_SCALE,
            Self::Normal => NORMAL_FUSHI_SCALE,
            Self::Large => LARGE_FUSHI_SCALE,
            Self::Huge => HUGE_FUSHI_SCALE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AndroidDisplayGeometry {
    width: i32,
    height: i32,
    work_left: i32,
    work_top: i32,
    work_right: i32,
    work_bottom: i32,
}

impl AndroidDisplayGeometry {
    fn new(
        width: i32,
        height: i32,
        work_left: i32,
        work_top: i32,
        work_right: i32,
        work_bottom: i32,
    ) -> Self {
        let width = width.max(240);
        let height = height.max(160);
        let work_left = work_left.clamp(0, width - 1);
        let work_top = work_top.clamp(0, height - 1);
        Self {
            width,
            height,
            work_left,
            work_top,
            work_right: work_right.clamp(work_left + 1, width),
            work_bottom: work_bottom.clamp(work_top + 1, height),
        }
    }

    fn environment(self) -> DesktopEnvironment {
        // The work-area edges are the physical contact planes. Insetting this rect for visual
        // padding makes the rendered body hover by that amount on every wall.
        DesktopEnvironment::from_screen_work_area(
            RectI::new(0, 0, self.width, self.height),
            RectI::new(self.work_left, self.work_top, self.work_right, self.work_bottom),
        )
        .with_monitor_corner_padding(ANDROID_MONITOR_CORNER_PADDING)
    }
}

#[repr(C)]
struct ANativeWindow {
    _private: [u8; 0],
}

#[link(name = "android")]
extern "C" {
    fn ANativeWindow_fromSurface(
        env: *mut jni::sys::JNIEnv,
        surface: jni::sys::jobject,
    ) -> *mut ANativeWindow;
    fn ANativeWindow_release(window: *mut ANativeWindow);
}

#[derive(Debug)]
struct AndroidNativeWindow {
    ptr: NonNull<ANativeWindow>,
}

unsafe impl Send for AndroidNativeWindow {}
unsafe impl Sync for AndroidNativeWindow {}

impl AndroidNativeWindow {
    unsafe fn from_surface(env: &mut JNIEnv<'_>, surface: JObject<'_>) -> Option<Self> {
        if surface.is_null() {
            return None;
        }
        let ptr = ANativeWindow_fromSurface(env.get_native_interface(), surface.as_raw());
        NonNull::new(ptr).map(|ptr| Self { ptr })
    }
}

impl Drop for AndroidNativeWindow {
    fn drop(&mut self) {
        unsafe { ANativeWindow_release(self.ptr.as_ptr()) };
    }
}

impl HasWindowHandle for AndroidNativeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = AndroidNdkWindowHandle::new(self.ptr.cast::<c_void>());
        let raw = RawWindowHandle::AndroidNdk(handle);
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for AndroidNativeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}

struct PendingFrame {
    dt: f32,
    geometry: AndroidDisplayGeometry,
    pending: bool,
}

impl PendingFrame {
    fn new(geometry: AndroidDisplayGeometry) -> Self {
        Self {
            dt: 0.0,
            geometry,
            pending: false,
        }
    }

    fn merge(&mut self, dt: f32, geometry: AndroidDisplayGeometry) {
        self.dt = (self.dt + dt.clamp(0.001, 0.050)).min(0.12);
        self.geometry = geometry;
        self.pending = true;
    }

    fn take(&mut self) -> Option<(f32, AndroidDisplayGeometry)> {
        if !self.pending {
            return None;
        }
        let frame = (self.dt.max(0.001), self.geometry);
        self.dt = 0.0;
        self.pending = false;
        Some(frame)
    }
}

struct DragStartRequest {
    state: AtomicU8,
    completed: mpsc::SyncSender<bool>,
}

impl DragStartRequest {
    fn new(completed: mpsc::SyncSender<bool>) -> Self {
        Self {
            state: AtomicU8::new(DRAG_REQUEST_PENDING),
            completed,
        }
    }

    /// Claims the request before touching the simulation. A timed-out caller can
    /// cancel only while the request is still queued, so a cancelled request can
    /// never begin a drag later on the render thread.
    fn try_claim(&self) -> bool {
        self.state
            .compare_exchange(
                DRAG_REQUEST_PENDING,
                DRAG_REQUEST_CLAIMED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn cancel_if_pending(&self) -> bool {
        self.state
            .compare_exchange(
                DRAG_REQUEST_PENDING,
                DRAG_REQUEST_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn complete(&self, started: bool) {
        self.state.store(DRAG_REQUEST_COMPLETED, Ordering::Release);
        let _ = self.completed.send(started);
    }
}

enum WorkerSignal {
    Attach {
        window: AndroidNativeWindow,
        width: u32,
        height: u32,
        completed: mpsc::SyncSender<Result<(), String>>,
    },
    Detach {
        completed: mpsc::SyncSender<()>,
    },
    Resize {
        width: u32,
        height: u32,
        density: f32,
    },
    SetSizePreset(AndroidFushiSizePreset),
    TryBeginDrag {
        x: f32,
        y: f32,
        request: Arc<DragStartRequest>,
    },
    Pointer {
        x: f32,
        y: f32,
        down: bool,
    },
    CancelPointer,
    Hover {
        x: f32,
        y: f32,
        inside: bool,
    },
    Frame,
    Stop {
        completed: mpsc::SyncSender<()>,
    },
}

pub struct AndroidFushiController {
    signal_tx: mpsc::Sender<WorkerSignal>,
    pending_frame: Arc<Mutex<PendingFrame>>,
    motion_input: Arc<Mutex<MotionInput>>,
    frame_queued: Arc<AtomicBool>,
    drag_active: AtomicBool,
    latest_layout: Arc<Mutex<[f32; 4]>>,
    worker: Option<JoinHandle<()>>,
}

impl AndroidFushiController {
    fn new(
        surface_width: u32,
        surface_height: u32,
        density: f32,
        geometry: AndroidDisplayGeometry,
        graphics_backend: AndroidGraphicsBackend,
        size_preset: AndroidFushiSizePreset,
    ) -> Result<Self, String> {
        let engine = AndroidFushiEngine::new(
            surface_width,
            surface_height,
            density,
            geometry,
            graphics_backend,
            size_preset,
        );
        let initial_layout = engine.layout();
        let (signal_tx, signal_rx) = mpsc::channel();
        let pending_frame = Arc::new(Mutex::new(PendingFrame::new(geometry)));
        // Fail closed until Java confirms one coherent sensor path. This keeps devices without
        // motion sensors on the original screen-down physics and rejects stale queued callbacks.
        let motion_input = Arc::new(Mutex::new(MotionInput::new(SensorAvailability::none())));
        let frame_queued = Arc::new(AtomicBool::new(false));
        let latest_layout = Arc::new(Mutex::new(initial_layout));

        let worker_pending_frame = pending_frame.clone();
        let worker_motion_input = motion_input.clone();
        let worker_frame_queued = frame_queued.clone();
        let worker_latest_layout = latest_layout.clone();
        let worker = thread::Builder::new()
            .name("Desktop Fushi wgpu".to_string())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run_render_worker(
                        engine,
                        signal_rx,
                        worker_pending_frame,
                        worker_motion_input,
                        worker_frame_queued,
                        worker_latest_layout,
                    );
                }));
                if result.is_err() {
                    log::error!("Android render thread panicked");
                }
            })
            .map_err(|err| format!("failed to start Android render thread: {err}"))?;

        Ok(Self {
            signal_tx,
            pending_frame,
            motion_input,
            frame_queued,
            drag_active: AtomicBool::new(false),
            latest_layout,
            worker: Some(worker),
        })
    }

    fn attach_surface(&self, window: AndroidNativeWindow, width: u32, height: u32) -> Result<(), String> {
        // Capacity one lets the worker report completion without blocking if JNI has already
        // returned after a timeout. In the normal path recv_timeout consumes the result directly.
        let (completed_tx, completed_rx) = mpsc::sync_channel(1);
        self.signal_tx
            .send(WorkerSignal::Attach {
                window,
                width: width.max(1),
                height: height.max(1),
                completed: completed_tx,
            })
            .map_err(|_| "Android render thread is not available".to_string())?;

        match completed_rx.recv_timeout(SURFACE_ATTACH_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err("timed out while attaching the Android render surface".to_string())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("Android render thread stopped while attaching the surface".to_string())
            }
        }
    }

    fn detach_surface(&self) {
        let (completed_tx, completed_rx) = mpsc::sync_channel(0);
        if self
            .signal_tx
            .send(WorkerSignal::Detach {
                completed: completed_tx,
            })
            .is_ok()
        {
            let _ = completed_rx.recv();
        }
    }

    fn resize_surface(&self, width: u32, height: u32, density: f32) {
        let _ = self.signal_tx.send(WorkerSignal::Resize {
            width: width.max(1),
            height: height.max(1),
            density: density.max(0.5),
        });
    }

    fn set_size_preset(&self, preset: AndroidFushiSizePreset) {
        let _ = self.signal_tx.send(WorkerSignal::SetSizePreset(preset));
    }

    fn try_begin_drag(&self, x: f32, y: f32) -> bool {
        let (completed_tx, completed_rx) = mpsc::sync_channel(0);
        let request = Arc::new(DragStartRequest::new(completed_tx));
        if self
            .signal_tx
            .send(WorkerSignal::TryBeginDrag {
                x,
                y,
                request: request.clone(),
            })
            .is_err()
        {
            return false;
        }
        let started = match completed_rx.recv_timeout(DRAG_START_TIMEOUT) {
            Ok(started) => started,
            Err(mpsc::RecvTimeoutError::Disconnected) => false,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if request.cancel_if_pending() {
                    false
                } else {
                    // The worker already claimed the command before the deadline.
                    // Wait for that in-progress hit test instead of returning false
                    // while allowing it to mutate the body asynchronously afterward.
                    completed_rx.recv().unwrap_or(false)
                }
            }
        };
        if started {
            self.drag_active.store(true, Ordering::Release);
            self.reset_motion();
        }
        started
    }

    fn pointer(&self, x: f32, y: f32, down: bool) {
        if !down {
            self.drag_active.store(false, Ordering::Release);
            self.reset_motion();
        }
        let _ = self.signal_tx.send(WorkerSignal::Pointer { x, y, down });
    }

    fn cancel_pointer(&self) {
        self.drag_active.store(false, Ordering::Release);
        self.reset_motion();
        let _ = self.signal_tx.send(WorkerSignal::CancelPointer);
    }

    fn hover(&self, x: f32, y: f32, inside: bool) {
        let _ = self.signal_tx.send(WorkerSignal::Hover { x, y, inside });
    }

    fn motion_sample(
        &self,
        sensor_kind: jint,
        x: f32,
        y: f32,
        z: f32,
        timestamp_ns: jlong,
        display_rotation: jint,
    ) {
        if self.drag_active.load(Ordering::Acquire) {
            return;
        }
        let sensor_kind = match sensor_kind {
            1 => SensorKind::LinearAcceleration,
            2 => SensorKind::Gravity,
            3 => SensorKind::Accelerometer,
            _ => return,
        };
        let rotation = display_rotation.clamp(0, 3) as u8;
        let _ =
            lock_unpoisoned(&self.motion_input).push_sample(sensor_kind, timestamp_ns, rotation, [x, y, z]);
    }

    fn reset_motion(&self) {
        lock_unpoisoned(&self.motion_input).reset();
    }

    fn set_motion_sensor_mode(&self, mode: jint) {
        let availability = match mode {
            1 => SensorAvailability::direct_pair(),
            2 => SensorAvailability::raw_accelerometer(),
            _ => SensorAvailability::none(),
        };
        lock_unpoisoned(&self.motion_input).set_availability(availability);
    }

    fn request_frame(&self, dt: f32, geometry: AndroidDisplayGeometry) {
        lock_unpoisoned(&self.pending_frame).merge(dt, geometry);
        if !self.frame_queued.swap(true, Ordering::AcqRel)
            && self.signal_tx.send(WorkerSignal::Frame).is_err()
        {
            self.frame_queued.store(false, Ordering::Release);
        }
    }

    fn latest_layout(&self) -> [f32; 4] {
        *lock_unpoisoned(&self.latest_layout)
    }

    fn stop(&mut self) {
        self.drag_active.store(false, Ordering::Release);
        self.reset_motion();
        let Some(worker) = self.worker.take() else {
            return;
        };
        let (completed_tx, completed_rx) = mpsc::sync_channel(0);
        if self
            .signal_tx
            .send(WorkerSignal::Stop {
                completed: completed_tx,
            })
            .is_ok()
        {
            let _ = completed_rx.recv();
        }
        let _ = worker.join();
    }
}

impl Drop for AndroidFushiController {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_render_worker(
    mut engine: AndroidFushiEngine,
    signal_rx: mpsc::Receiver<WorkerSignal>,
    pending_frame: Arc<Mutex<PendingFrame>>,
    motion_input: Arc<Mutex<MotionInput>>,
    frame_queued: Arc<AtomicBool>,
    latest_layout: Arc<Mutex<[f32; 4]>>,
) {
    log::info!("Android render thread started");
    while let Ok(signal) = signal_rx.recv() {
        match signal {
            WorkerSignal::Attach {
                window,
                width,
                height,
                completed,
            } => {
                let result = engine.attach_surface(window, width, height);
                if let Err(err) = &result {
                    log::error!("Android surface attach failed: {err}");
                }
                let _ = completed.send(result);
            }
            WorkerSignal::Detach { completed } => {
                engine.detach_surface();
                let _ = completed.send(());
            }
            WorkerSignal::Resize {
                width,
                height,
                density,
            } => engine.resize_surface(width, height, density),
            WorkerSignal::SetSizePreset(preset) => engine.set_size_preset(preset),
            WorkerSignal::TryBeginDrag { x, y, request } => {
                if request.try_claim() {
                    let started = engine.try_begin_drag(x, y);
                    request.complete(started);
                }
            }
            WorkerSignal::Pointer { x, y, down } => engine.pointer(x, y, down),
            WorkerSignal::CancelPointer => engine.cancel_pointer(),
            WorkerSignal::Hover { x, y, inside } => engine.hover(x, y, inside),
            WorkerSignal::Frame => {
                // Clear the queued marker before taking the merged frame. A concurrently arriving
                // vsync may enqueue one harmless extra wake-up, but no frame request can be lost.
                frame_queued.store(false, Ordering::Release);
                let frame = lock_unpoisoned(&pending_frame).take();
                if let Some((dt, geometry)) = frame {
                    let motion = {
                        let mut input = lock_unpoisoned(&motion_input);
                        input.advance_frame(dt);
                        input.take_frame()
                    };
                    let layout = engine.step(dt, geometry, motion);
                    *lock_unpoisoned(&latest_layout) = layout;
                }
            }
            WorkerSignal::Stop { completed } => {
                engine.detach_surface();
                let _ = completed.send(());
                break;
            }
        }
    }
    engine.detach_surface();
    log::info!("Android render thread stopped");
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct AndroidFushiEngine {
    env: DesktopEnvironment,
    fushi: FushiBody,
    renderer: FushiRenderer,
    wgpu: Option<WgpuLayer>,
    geometry: AndroidDisplayGeometry,
    surface_width: u32,
    surface_height: u32,
    window_size: (u32, u32),
    pointer_down: bool,
    hovering: bool,
    hover_world: Vec2,
    density: f32,
    graphics_backend: AndroidGraphicsBackend,
    size_preset: AndroidFushiSizePreset,
    render_error_logged: bool,
}

impl AndroidFushiEngine {
    fn new(
        surface_width: u32,
        surface_height: u32,
        density: f32,
        geometry: AndroidDisplayGeometry,
        graphics_backend: AndroidGraphicsBackend,
        size_preset: AndroidFushiSizePreset,
    ) -> Self {
        let surface_width = surface_width.max(1);
        let surface_height = surface_height.max(1);
        let density = density.max(0.5);
        let scale = android_fushi_scale(density, size_preset);
        let env = geometry.environment();
        let mut fushi = FushiBody::new(&env);
        fushi.set_scale(scale, &env);
        fushi.snap_to_contact(
            SurfaceContact::monitor(0, SurfaceKind::Bottom),
            (geometry.work_left + geometry.work_right) as f32 * 0.50,
            &env,
        );
        fushi.set_cursor(FAR_CURSOR);
        let window_size = fixed_android_window_size(&fushi);
        log::info!(
            "Android renderer settings backend={graphics_backend:?} size={size_preset:?} scale={:.2} window={}x{}",
            fushi.scale,
            window_size.0,
            window_size.1,
        );

        Self {
            env,
            fushi,
            renderer: FushiRenderer::new(),
            wgpu: None,
            geometry,
            surface_width,
            surface_height,
            window_size,
            pointer_down: false,
            hovering: false,
            hover_world: FAR_CURSOR,
            density,
            graphics_backend,
            size_preset,
            render_error_logged: false,
        }
    }

    fn layout(&self) -> [f32; 4] {
        let origin = android_window_origin(&self.fushi, self.window_size);
        [
            origin.x,
            origin.y,
            self.window_size.0 as f32,
            self.window_size.1 as f32,
        ]
    }

    fn attach_surface(&mut self, window: AndroidNativeWindow, width: u32, height: u32) -> Result<(), String> {
        self.wgpu = None;
        self.surface_width = width.max(1);
        self.surface_height = height.max(1);
        if (self.surface_width, self.surface_height) != self.window_size {
            log::warn!(
                "Android surface {}x{} differs from fixed window {}x{}",
                self.surface_width,
                self.surface_height,
                self.window_size.0,
                self.window_size.1,
            );
        }
        let size = WgpuSurfaceSize::new(self.surface_width, self.surface_height);
        let layer = pollster::block_on(WgpuLayer::new_with_backends(
            window,
            size,
            self.graphics_backend.wgpu_backends(),
        ))?;
        self.wgpu = Some(layer);
        self.render_error_logged = false;
        let origin = android_window_origin(&self.fushi, self.window_size);
        self.render_at(origin);
        log::info!(
            "attached Android wgpu surface {}x{}",
            self.surface_width,
            self.surface_height
        );
        Ok(())
    }

    fn detach_surface(&mut self) {
        if self.wgpu.take().is_some() {
            log::info!("detached Android wgpu surface");
        }
    }

    fn resize_surface(&mut self, width: u32, height: u32, density: f32) {
        let width = width.max(1);
        let height = height.max(1);
        let density = density.max(0.5);
        let density_changed = (self.density - density).abs() > 0.001;
        self.surface_width = width;
        self.surface_height = height;
        self.density = density;
        if density_changed {
            let scale = android_fushi_scale(self.density, self.size_preset);
            self.env = self.geometry.environment();
            self.fushi.set_scale(scale, &self.env);
            self.window_size = fixed_android_window_size(&self.fushi);
        }
        if let Some(wgpu) = self.wgpu.as_mut() {
            wgpu.resize(width, height);
        }
    }

    fn set_size_preset(&mut self, preset: AndroidFushiSizePreset) {
        if self.size_preset == preset {
            return;
        }
        self.size_preset = preset;
        let scale = android_fushi_scale(self.density, preset);
        self.env = self.geometry.environment();
        self.fushi.set_scale(scale, &self.env);
        self.window_size = fixed_android_window_size(&self.fushi);
        log::info!(
            "Android Fushi size changed to {preset:?} scale={:.2} window={}x{}",
            self.fushi.scale,
            self.window_size.0,
            self.window_size.1,
        );
    }

    fn set_display(&mut self, geometry: AndroidDisplayGeometry) {
        if self.geometry == geometry {
            return;
        }
        self.geometry = geometry;
        let scale = android_fushi_scale(self.density, self.size_preset);
        let scale_changed = (self.fushi.scale - scale).abs() > 0.001;
        let attached = self
            .fushi
            .surface
            .filter(|_| self.fushi.mode == MotionMode::Attached);
        let tangent = attached.map(|contact| {
            (
                contact,
                DesktopEnvironment::tangent_coord(contact.kind, self.fushi.center),
            )
        });
        self.env = geometry.environment();
        if scale_changed {
            self.fushi.set_scale(scale, &self.env);
            self.window_size = fixed_android_window_size(&self.fushi);
        } else if let Some((contact, tangent)) = tangent {
            self.fushi.snap_to_contact(contact, tangent, &self.env);
        }
        if !self.env.virtual_bounds.inflate(900).contains(self.fushi.center) {
            self.fushi.reset_to_safe_surface(&self.env);
        }
    }

    fn try_begin_drag(&mut self, raw_x: f32, raw_y: f32) -> bool {
        let world = Vec2::new(raw_x, raw_y);
        self.fushi.set_cursor(world);
        self.pointer_down = self.fushi.try_begin_drag(world);
        if !self.pointer_down {
            self.restore_hover_cursor();
        }
        self.pointer_down
    }

    fn pointer(&mut self, raw_x: f32, raw_y: f32, down: bool) {
        let world = Vec2::new(raw_x, raw_y);
        if down {
            self.fushi.set_cursor(world);
        }
        match (down, self.pointer_down) {
            (true, false) => {
                self.pointer_down = self.fushi.try_begin_drag(world);
                if !self.pointer_down {
                    self.restore_hover_cursor();
                }
            }
            (true, true) => {
                if self.fushi.mode == MotionMode::Dragged {
                    self.fushi.drag_to(world);
                }
            }
            (false, true) => {
                if self.fushi.mode == MotionMode::Dragged {
                    self.fushi.drag_to(world);
                    self.fushi.release_drag();
                }
                self.pointer_down = false;
                self.restore_hover_cursor();
            }
            (false, false) => {
                self.restore_hover_cursor();
            }
        }
    }

    fn cancel_pointer(&mut self) {
        if self.fushi.mode == MotionMode::Dragged {
            self.fushi.release_drag();
        }
        self.pointer_down = false;
        self.restore_hover_cursor();
    }

    fn hover(&mut self, raw_x: f32, raw_y: f32, inside: bool) {
        self.hovering = inside;
        self.hover_world = Vec2::new(raw_x, raw_y);
        if !self.pointer_down {
            self.restore_hover_cursor();
        }
    }

    fn restore_hover_cursor(&mut self) {
        self.fushi.set_cursor(if self.hovering {
            self.hover_world
        } else {
            FAR_CURSOR
        });
    }

    fn step(&mut self, dt: f32, geometry: AndroidDisplayGeometry, motion: FrameMotion) -> [f32; 4] {
        self.set_display(geometry);
        let dt = dt.clamp(0.001, 0.12);
        self.fushi.apply_container_motion(
            motion.impulse,
            motion.gravity,
            motion.intensity,
            motion.sensor_available,
            motion.gravity_valid,
            motion.triggered,
            motion.gate_open,
        );
        let step_count = ((dt / MAX_SIMULATION_STEP).ceil() as usize).clamp(1, 16);
        let simulation_step = dt / step_count as f32;
        for _ in 0..step_count {
            self.fushi.step(simulation_step, &self.env);
        }

        let layout = self.layout();
        if self.wgpu.is_some() {
            self.render_at(Vec2::new(layout[0], layout[1]));
        }

        layout
    }

    fn render_at(&mut self, origin: Vec2) {
        let width = self.surface_width.max(1);
        let height = self.surface_height.max(1);
        let mut canvas = GpuCanvas::new(width, height, origin, 1.0);
        self.renderer.draw(&mut canvas, &self.fushi);
        let scene = canvas.into_scene();
        let Some(wgpu) = self.wgpu.as_mut() else {
            return;
        };
        wgpu.resize(width, height);
        match wgpu.render(&scene) {
            Ok(_) => self.render_error_logged = false,
            Err(err) => {
                if !self.render_error_logged {
                    log::error!("Android wgpu render failed: {err}");
                    self.render_error_logged = true;
                }
            }
        }
    }
}

fn android_fushi_scale(density: f32, preset: AndroidFushiSizePreset) -> f32 {
    density.max(0.5) * preset.desktop_scale() * ANDROID_FUSHI_SCALE_FACTOR
}

fn fixed_android_window_size(fushi: &FushiBody) -> (u32, u32) {
    // The body center is the window's invariant anchor. Use the farthest real conservative
    // body/appendage point rather than the diagonal of its AABB: the latter combines unrelated
    // x/y extrema into an empty corner and made the touch-blocking square needlessly large.
    // A true radius remains rotation-safe, then the physical-pixel margin covers outlines, fur
    // and bounded soft-body deformation.
    let radial_extent = fushi.content_radius();
    let desired_side = (radial_extent * 2.0 + ANDROID_WINDOW_VISUAL_MARGIN * 2.0)
        .ceil()
        .max(ANDROID_MIN_WINDOW_SIZE as f32) as u32;
    let side = desired_side
        .div_ceil(ANDROID_WINDOW_SIZE_QUANTUM)
        .saturating_mul(ANDROID_WINDOW_SIZE_QUANTUM)
        .max(ANDROID_MIN_WINDOW_SIZE);
    (side, side)
}

fn android_window_origin(fushi: &FushiBody, window_size: (u32, u32)) -> Vec2 {
    Vec2::new(
        (fushi.center.x - window_size.0 as f32 * 0.5).round(),
        (fushi.center.y - window_size.1 as f32 * 0.5).round(),
    )
}

unsafe fn controller_ref<'a>(ptr: jlong) -> Option<&'a AndroidFushiController> {
    if ptr == 0 {
        None
    } else {
        Some(&*(ptr as *const AndroidFushiController))
    }
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeIsVulkanSupported(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jboolean {
    init_android_logging();
    match catch_unwind(AssertUnwindSafe(|| {
        android_backend_available(wgpu::Backends::VULKAN)
    })) {
        Ok(true) => 1,
        Ok(false) | Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeCreate(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    surface_width: jint,
    surface_height: jint,
    density: jfloat,
    screen_width: jint,
    screen_height: jint,
    work_left: jint,
    work_top: jint,
    work_right: jint,
    work_bottom: jint,
    graphics_backend: jint,
    size_preset: jint,
) -> jlong {
    init_android_logging();
    match catch_unwind(AssertUnwindSafe(|| {
        let graphics_backend = AndroidGraphicsBackend::from_raw(graphics_backend)?;
        AndroidFushiController::new(
            surface_width.max(1) as u32,
            surface_height.max(1) as u32,
            density.max(0.5),
            AndroidDisplayGeometry::new(
                screen_width,
                screen_height,
                work_left,
                work_top,
                work_right,
                work_bottom,
            ),
            graphics_backend,
            AndroidFushiSizePreset::from_raw(size_preset),
        )
    })) {
        Ok(Ok(controller)) => Box::into_raw(Box::new(controller)) as jlong,
        Ok(Err(err)) => {
            log::error!("Desktop Fushi Android init failed: {err}");
            0
        }
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeDestroy(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if ptr != 0 {
            unsafe {
                let mut controller = Box::from_raw(ptr as *mut AndroidFushiController);
                controller.stop();
                drop(controller);
            }
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeAttachSurface(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    surface: JObject<'_>,
    width: jint,
    height: jint,
) -> jboolean {
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(controller) = controller_ref(ptr) else {
            return Err("native controller is null".to_string());
        };
        let Some(window) = AndroidNativeWindow::from_surface(&mut env, surface) else {
            return Err("ANativeWindow_fromSurface returned null".to_string());
        };
        controller.attach_surface(window, width.max(1) as u32, height.max(1) as u32)
    })) {
        Ok(Ok(())) => 1,
        Ok(Err(err)) => {
            log::error!("Android surface attach failed: {err}");
            0
        }
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeDetachSurface(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.detach_surface();
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeResize(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    width: jint,
    height: jint,
    density: jfloat,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.resize_surface(width.max(1) as u32, height.max(1) as u32, density.max(0.5));
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeSetSizePreset(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    size_preset: jint,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.set_size_preset(AndroidFushiSizePreset::from_raw(size_preset));
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeTryBeginDrag(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        controller_ref(ptr)
            .map(|controller| controller.try_begin_drag(x, y))
            .unwrap_or(false)
    })) {
        Ok(true) => 1,
        Ok(false) | Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativePointer(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    x: jfloat,
    y: jfloat,
    down: jboolean,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.pointer(x, y, down != 0);
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeCancelPointer(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.cancel_pointer();
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeHover(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    x: jfloat,
    y: jfloat,
    inside: jboolean,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.hover(x, y, inside != 0);
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeMotionSample(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    sensor_kind: jint,
    x: jfloat,
    y: jfloat,
    z: jfloat,
    timestamp_ns: jlong,
    display_rotation: jint,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.motion_sample(sensor_kind, x, y, z, timestamp_ns, display_rotation);
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeResetMotion(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.reset_motion();
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeSetMotionSensorMode(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    mode: jint,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.set_motion_sensor_mode(mode);
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeStep(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    dt: jfloat,
    screen_width: jint,
    screen_height: jint,
    work_left: jint,
    work_top: jint,
    work_right: jint,
    work_bottom: jint,
    layout: JFloatArray<'_>,
) {
    let frame = catch_unwind(AssertUnwindSafe(|| unsafe {
        controller_ref(ptr).map(|controller| {
            controller.request_frame(
                dt,
                AndroidDisplayGeometry::new(
                    screen_width,
                    screen_height,
                    work_left,
                    work_top,
                    work_right,
                    work_bottom,
                ),
            );
            controller.latest_layout()
        })
    }));
    if let Ok(Some(frame)) = frame {
        let _ = env.set_float_array_region(&layout, 0, &frame);
    }
}

fn init_android_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("DesktopFushiRust")
            .with_max_level(log::LevelFilter::Info),
    );
}
