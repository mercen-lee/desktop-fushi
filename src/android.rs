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

pub struct AndroidFushiEngine {
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
        }
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
        self.render_timer = FRAME_TIMER_WAKE;
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
                let _ = self.fushi.try_begin_drag(world);
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
                self.restore_hover_cursor();
            }
            (false, false) => {
                self.restore_hover_cursor();
            }
        }
        self.pointer_down = down;
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
        let dt = dt.clamp(0.001, 0.050);
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

        [origin.x, origin.y, size.0 as f32, size.1 as f32]
    }

    fn high_activity(&self) -> bool {
        self.pointer_down || self.fushi.mode != MotionMode::Attached
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
        if let Err(err) = wgpu.render(&scene) {
            log::error!("Android wgpu render failed: {err}");
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
    let desired_width = bounds.width().ceil().max(MIN_SURFACE_SIZE as f32) as u32;
    let desired_height = bounds.height().ceil().max(MIN_SURFACE_SIZE as f32) as u32;
    let width = grow_surface_dimension(current_size.0, desired_width);
    let height = grow_surface_dimension(current_size.1, desired_height);

    let mut origin = Vec2::new(
        ((bounds.left + bounds.right) * 0.5 - width as f32 * 0.5).floor(),
        ((bounds.top + bounds.bottom) * 0.5 - height as f32 * 0.5).floor(),
    );
    let max_x = (screen_width.max(1) as f32 - width as f32).max(0.0);
    let max_y = (screen_height.max(1) as f32 - height as f32).max(0.0);
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

unsafe fn engine_mut<'a>(ptr: jlong) -> Option<&'a mut AndroidFushiEngine> {
    if ptr == 0 {
        None
    } else {
        Some(&mut *(ptr as *mut AndroidFushiEngine))
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
        AndroidFushiEngine::new(
            surface_width.max(1) as u32,
            surface_height.max(1) as u32,
            density.max(0.5),
            screen_width,
            screen_height,
        )
    })) {
        Ok(engine) => Box::into_raw(Box::new(engine)) as jlong,
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
                let mut engine = Box::from_raw(ptr as *mut AndroidFushiEngine);
                engine.detach_surface();
                drop(engine);
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
        let Some(engine) = engine_mut(ptr) else {
            return Err("native engine is null".to_string());
        };
        let Some(window) = AndroidNativeWindow::from_surface(&mut env, surface) else {
            return Err("ANativeWindow_fromSurface returned null".to_string());
        };
        engine.attach_surface(window, width.max(1) as u32, height.max(1) as u32)
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
        if let Some(engine) = engine_mut(ptr) {
            engine.detach_surface();
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
        if let Some(engine) = engine_mut(ptr) {
            engine.resize_surface(width.max(1) as u32, height.max(1) as u32, density.max(0.5));
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
        if let Some(engine) = engine_mut(ptr) {
            engine.pointer(x, y, down != 0);
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
        if let Some(engine) = engine_mut(ptr) {
            engine.hover(x, y, inside != 0);
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
        if let Some(engine) = engine_mut(ptr) {
            engine.shake(ax, ay, az, dt);
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeStep(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    dt: jfloat,
    screen_width: jint,
    screen_height: jint,
    layout: JFloatArray<'_>,
) {
    let frame = catch_unwind(AssertUnwindSafe(|| unsafe {
        engine_mut(ptr).map(|engine| engine.step(dt, screen_width, screen_height))
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
