#![cfg(target_os = "android")]

use jni::objects::{JClass, JFloatArray, JObject};
use jni::sys::{jboolean, jfloat, jint, jlong};
use jni::JNIEnv;
use raw_window_handle::{
    AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawWindowHandle, WindowHandle,
};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use crate::desktop::{DesktopEnvironment, SurfaceContact, SurfaceKind};
use crate::fushi::render::FushiRenderer;
use crate::fushi::{FushiBody, MotionMode};
use crate::gpu_canvas::GpuCanvas;
use crate::math::{clampf, smoothstep, Vec2};
use crate::wgpu_layer::{WgpuLayer, WgpuSurfaceSize};

const MIN_SURFACE_SIZE: u32 = 96;
const FIXED_STEP: f32 = 1.0 / 60.0;
const ACTIVE_RENDER_INTERVAL: f32 = FIXED_STEP;
const IDLE_RENDER_INTERVAL: f32 = 1.0 / 30.0;
const FRAME_TIMER_WAKE: f32 = 0.5;
const FAR_CURSOR: Vec2 = Vec2::new(-10000.0, -10000.0);
const ANDROID_MIN_SCALE: f32 = 0.62;
const ANDROID_MAX_SCALE: f32 = 2.42;
const WINDOW_PADDING: f32 = 28.0;
const WINDOW_GROW_CHUNK: u32 = 32;

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
    screen_width: i32,
    screen_height: i32,
    pending: bool,
}

impl PendingFrame {
    fn new(screen_width: i32, screen_height: i32) -> Self {
        Self {
            dt: 0.0,
            screen_width,
            screen_height,
            pending: false,
        }
    }

    fn merge(&mut self, dt: f32, screen_width: i32, screen_height: i32) {
        self.dt = (self.dt + dt.clamp(0.001, 0.050)).min(0.12);
        self.screen_width = screen_width;
        self.screen_height = screen_height;
        self.pending = true;
    }

    fn take(&mut self) -> Option<(f32, i32, i32)> {
        if !self.pending {
            return None;
        }
        let frame = (self.dt.max(0.001), self.screen_width, self.screen_height);
        self.dt = 0.0;
        self.pending = false;
        Some(frame)
    }
}

enum WorkerSignal {
    Attach {
        window: AndroidNativeWindow,
        width: u32,
        height: u32,
    },
    Detach {
        completed: mpsc::SyncSender<()>,
    },
    Resize {
        width: u32,
        height: u32,
        density: f32,
    },
    Pointer {
        x: f32,
        y: f32,
        down: bool,
    },
    Hover {
        x: f32,
        y: f32,
        inside: bool,
    },
    Shake {
        ax: f32,
        ay: f32,
        az: f32,
        dt: f32,
    },
    Frame,
    Stop {
        completed: mpsc::SyncSender<()>,
    },
}

pub struct AndroidFushiController {
    signal_tx: mpsc::Sender<WorkerSignal>,
    pending_frame: Arc<Mutex<PendingFrame>>,
    frame_queued: Arc<AtomicBool>,
    latest_layout: Arc<Mutex<[f32; 4]>>,
    worker: Option<JoinHandle<()>>,
}

impl AndroidFushiController {
    fn new(
        surface_width: u32,
        surface_height: u32,
        density: f32,
        screen_width: i32,
        screen_height: i32,
    ) -> Result<Self, String> {
        let engine = AndroidFushiEngine::new(
            surface_width,
            surface_height,
            density,
            screen_width,
            screen_height,
        );
        let initial_layout = engine.layout();
        let (signal_tx, signal_rx) = mpsc::channel();
        let pending_frame = Arc::new(Mutex::new(PendingFrame::new(
            screen_width,
            screen_height,
        )));
        let frame_queued = Arc::new(AtomicBool::new(false));
        let latest_layout = Arc::new(Mutex::new(initial_layout));

        let worker_pending_frame = pending_frame.clone();
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
            frame_queued,
            latest_layout,
            worker: Some(worker),
        })
    }

    fn attach_surface(
        &self,
        window: AndroidNativeWindow,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.signal_tx
            .send(WorkerSignal::Attach {
                window,
                width: width.max(1),
                height: height.max(1),
            })
            .map_err(|_| "Android render thread is not available".to_string())
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

    fn pointer(&self, x: f32, y: f32, down: bool) {
        let _ = self.signal_tx.send(WorkerSignal::Pointer { x, y, down });
    }

    fn hover(&self, x: f32, y: f32, inside: bool) {
        let _ = self.signal_tx.send(WorkerSignal::Hover { x, y, inside });
    }

    fn shake(&self, ax: f32, ay: f32, az: f32, dt: f32) {
        let _ = self.signal_tx.send(WorkerSignal::Shake {
            ax,
            ay,
            az,
            dt: dt.clamp(0.001, 0.060),
        });
    }

    fn request_frame(&self, dt: f32, screen_width: i32, screen_height: i32) {
        lock_unpoisoned(&self.pending_frame).merge(dt, screen_width, screen_height);
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
            } => {
                if let Err(err) = engine.attach_surface(window, width, height) {
                    log::error!("Android surface attach failed: {err}");
                }
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
            WorkerSignal::Pointer { x, y, down } => engine.pointer(x, y, down),
            WorkerSignal::Hover { x, y, inside } => engine.hover(x, y, inside),
            WorkerSignal::Shake { ax, ay, az, dt } => engine.shake(ax, ay, az, dt),
            WorkerSignal::Frame => {
                // Clear the queued marker before taking the merged frame. A concurrently arriving
                // vsync may enqueue one harmless extra wake-up, but no frame request can be lost.
                frame_queued.store(false, Ordering::Release);
                let frame = lock_unpoisoned(&pending_frame).take();
                if let Some((dt, screen_width, screen_height)) = frame {
                    let layout = engine.step(dt, screen_width, screen_height);
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
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct AndroidFushiEngine {
    env: DesktopEnvironment,
    fushi: FushiBody,
    renderer: FushiRenderer,
    wgpu: Option<WgpuLayer>,
    screen_width: i32,
    screen_height: i32,
    surface_width: u32,
    surface_height: u32,
    window_origin: Vec2,
    window_size: (u32, u32),
    accumulator: f32,
    render_timer: f32,
    pointer_down: bool,
    hovering: bool,
    hover_world: Vec2,
    density: f32,
    render_error_logged: bool,
}

impl AndroidFushiEngine {
    fn new(
        surface_width: u32,
        surface_height: u32,
        density: f32,
        screen_width: i32,
        screen_height: i32,
    ) -> Self {
        let surface_width = surface_width.max(MIN_SURFACE_SIZE);
        let surface_height = surface_height.max(MIN_SURFACE_SIZE);
        let density = density.max(0.5);
        let screen_width = screen_width.max(240);
        let screen_height = screen_height.max(160);
        let env = DesktopEnvironment::from_screen_size(screen_width, screen_height);
        let mut fushi = FushiBody::new(&env);
        fushi.set_scale(android_fushi_scale(density), &env);
        fushi.snap_to_contact(
            SurfaceContact::monitor(0, SurfaceKind::Bottom),
            screen_width as f32 * 0.50,
            &env,
        );
        keep_android_fushi_visible(&mut fushi, screen_width, screen_height);
        fushi.set_cursor(FAR_CURSOR);

        let (window_origin, window_size) = android_window_rect_for_body(
            &fushi,
            (MIN_SURFACE_SIZE, MIN_SURFACE_SIZE),
            screen_width,
            screen_height,
        );

        Self {
            env,
            fushi,
            renderer: FushiRenderer::new(),
            wgpu: None,
            screen_width,
            screen_height,
            surface_width,
            surface_height,
            window_origin,
            window_size,
            accumulator: 0.0,
            render_timer: FRAME_TIMER_WAKE,
            pointer_down: false,
            hovering: false,
            hover_world: FAR_CURSOR,
            density,
            render_error_logged: false,
        }
    }

    fn layout(&self) -> [f32; 4] {
        [
            self.window_origin.x,
            self.window_origin.y,
            self.window_size.0 as f32,
            self.window_size.1 as f32,
        ]
    }

    fn attach_surface(
        &mut self,
        window: AndroidNativeWindow,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.wgpu = None;
        self.surface_width = width.max(1);
        self.surface_height = height.max(1);
        let size = WgpuSurfaceSize::new(self.surface_width, self.surface_height);
        let layer = pollster::block_on(WgpuLayer::new(window, size))?;
        self.wgpu = Some(layer);
        self.render_error_logged = false;
        self.render_timer = FRAME_TIMER_WAKE;
        self.render();
        self.render_timer = 0.0;
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
            self.fushi
                .set_scale(android_fushi_scale(self.density), &self.env);
        }
        if let Some(wgpu) = self.wgpu.as_mut() {
            wgpu.resize(width, height);
        }
        self.render_timer = FRAME_TIMER_WAKE;
    }

    fn set_screen(&mut self, screen_width: i32, screen_height: i32) {
        let screen_width = screen_width.max(240);
        let screen_height = screen_height.max(160);
        if self.screen_width == screen_width && self.screen_height == screen_height {
            return;
        }
        self.screen_width = screen_width;
        self.screen_height = screen_height;
        self.env = DesktopEnvironment::from_screen_size(screen_width, screen_height);
        self.fushi
            .set_scale(android_fushi_scale(self.density), &self.env);
        if !self
            .env
            .virtual_bounds
            .inflate(900)
            .contains(self.fushi.center)
        {
            self.fushi.reset_to_safe_surface(&self.env);
        }
        self.render_timer = FRAME_TIMER_WAKE;
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
        self.render_timer = FRAME_TIMER_WAKE;
    }

    fn hover(&mut self, raw_x: f32, raw_y: f32, inside: bool) {
        self.hovering = inside;
        self.hover_world = Vec2::new(raw_x, raw_y);
        if !self.pointer_down {
            self.restore_hover_cursor();
        }
        self.render_timer = FRAME_TIMER_WAKE;
    }

    fn restore_hover_cursor(&mut self) {
        self.fushi.set_cursor(if self.hovering {
            self.hover_world
        } else {
            FAR_CURSOR
        });
    }

    fn shake(&mut self, ax: f32, ay: f32, az: f32, dt: f32) {
        let dt = dt.clamp(0.001, 0.060);
        let total = (ax * ax + ay * ay + az * az).sqrt();
        let planar = (ax * ax + ay * ay).sqrt();
        let intensity = smoothstep(2.2, 13.5, total).max(smoothstep(3.0, 10.0, planar));
        if intensity <= 0.002 {
            return;
        }
        let pixels_per_mps2 = 90.0 * self.density.clamp(0.75, 4.0);
        let local_accel =
            Vec2::new(-ax * pixels_per_mps2, ay * pixels_per_mps2).clamp_len(2200.0);
        self.fushi.apply_external_shake(local_accel, intensity, dt);
        self.render_timer = FRAME_TIMER_WAKE;
    }

    fn step(&mut self, dt: f32, screen_width: i32, screen_height: i32) -> [f32; 4] {
        self.set_screen(screen_width, screen_height);
        let dt = dt.clamp(0.001, 0.12);
        self.accumulator = (self.accumulator + dt).min(0.12);
        self.render_timer = (self.render_timer + dt).min(FRAME_TIMER_WAKE);

        let mut updated = false;
        while self.accumulator >= FIXED_STEP {
            self.fushi.step(FIXED_STEP, &self.env);
            self.accumulator -= FIXED_STEP;
            updated = true;
        }
        if self.accumulator > 0.030 {
            let partial = self.accumulator;
            self.fushi.step(partial, &self.env);
            self.accumulator = 0.0;
            updated = true;
        }

        keep_android_fushi_visible(&mut self.fushi, self.screen_width, self.screen_height);
        let (origin, size) = android_window_rect_for_body(
            &self.fushi,
            self.window_size,
            self.screen_width,
            self.screen_height,
        );
        let layout_changed = (origin.x - self.window_origin.x).abs() >= 1.0
            || (origin.y - self.window_origin.y).abs() >= 1.0
            || size != self.window_size;
        self.window_origin = origin;
        self.window_size = size;

        let render_interval = if self.high_activity() {
            ACTIVE_RENDER_INTERVAL
        } else {
            IDLE_RENDER_INTERVAL
        };
        if self.wgpu.is_some()
            && (updated || layout_changed)
            && self.render_timer >= render_interval
        {
            self.render_timer = 0.0;
            self.render();
        }

        self.layout()
    }

    fn high_activity(&self) -> bool {
        self.pointer_down
            || self.fushi.mode != MotionMode::Attached
            || self
                .fushi
                .surface
                .map(|surface| surface.is_platform())
                .unwrap_or(false)
            || self.fushi.stress > 0.03
            || self.fushi.hover_amount > 0.05
            || self.fushi.petting_amount > 0.02
            || self.fushi.happiness > 0.03
    }

    fn render(&mut self) {
        let width = self.surface_width.max(1);
        let height = self.surface_height.max(1);
        let mut canvas = GpuCanvas::new(width, height, self.window_origin, 1.0);
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

fn android_fushi_scale(density: f32) -> f32 {
    clampf(density * 0.62, ANDROID_MIN_SCALE, ANDROID_MAX_SCALE)
}

fn android_window_rect_for_body(
    fushi: &FushiBody,
    current_size: (u32, u32),
    screen_width: i32,
    screen_height: i32,
) -> (Vec2, (u32, u32)) {
    let bounds = fushi.render_bounds().inflate(WINDOW_PADDING);
    let screen_width = screen_width.max(MIN_SURFACE_SIZE as i32) as u32;
    let screen_height = screen_height.max(MIN_SURFACE_SIZE as i32) as u32;
    let desired_width = bounds.width().ceil().max(MIN_SURFACE_SIZE as f32) as u32;
    let desired_height = bounds.height().ceil().max(MIN_SURFACE_SIZE as f32) as u32;
    let width = grow_surface_dimension(current_size.0, desired_width).min(screen_width);
    let height = grow_surface_dimension(current_size.1, desired_height).min(screen_height);

    let mut origin = Vec2::new(
        ((bounds.left + bounds.right) * 0.5 - width as f32 * 0.5).floor(),
        ((bounds.top + bounds.bottom) * 0.5 - height as f32 * 0.5).floor(),
    );
    let max_x = (screen_width as f32 - width as f32).max(0.0);
    let max_y = (screen_height as f32 - height as f32).max(0.0);
    origin.x = clampf(origin.x, 0.0, max_x);
    origin.y = clampf(origin.y, 0.0, max_y);
    (origin, (width, height))
}

fn grow_surface_dimension(current: u32, desired: u32) -> u32 {
    let current = current.max(MIN_SURFACE_SIZE);
    if desired <= current {
        return current;
    }
    desired
        .div_ceil(WINDOW_GROW_CHUNK)
        .saturating_mul(WINDOW_GROW_CHUNK)
}

fn keep_android_fushi_visible(fushi: &mut FushiBody, screen_width: i32, screen_height: i32) {
    let screen_width = screen_width.max(240) as f32;
    let screen_height = screen_height.max(160) as f32;
    let margin = 8.0;
    let bounds = fushi.render_bounds().inflate(WINDOW_PADDING);
    let mut delta = Vec2::ZERO;

    if bounds.left < margin {
        delta.x += margin - bounds.left;
    } else if bounds.right > screen_width - margin {
        delta.x -= bounds.right - (screen_width - margin);
    }

    if bounds.top < margin {
        delta.y += margin - bounds.top;
    } else if bounds.bottom > screen_height - margin {
        delta.y -= bounds.bottom - (screen_height - margin);
    }

    fushi.translate_world(delta);
}

unsafe fn controller_ref<'a>(ptr: jlong) -> Option<&'a AndroidFushiController> {
    if ptr == 0 {
        None
    } else {
        Some(&*(ptr as *const AndroidFushiController))
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
) -> jlong {
    init_android_logging();
    match catch_unwind(AssertUnwindSafe(|| {
        AndroidFushiController::new(
            surface_width.max(1) as u32,
            surface_height.max(1) as u32,
            density.max(0.5),
            screen_width,
            screen_height,
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
            controller.resize_surface(
                width.max(1) as u32,
                height.max(1) as u32,
                density.max(0.5),
            );
        }
    }));
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
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeShake(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    ax: jfloat,
    ay: jfloat,
    az: jfloat,
    dt: jfloat,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if let Some(controller) = controller_ref(ptr) {
            controller.shake(ax, ay, az, dt);
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
    layout: JFloatArray<'_>,
) {
    let frame = catch_unwind(AssertUnwindSafe(|| unsafe {
        controller_ref(ptr).map(|controller| {
            controller.request_frame(dt, screen_width, screen_height);
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
