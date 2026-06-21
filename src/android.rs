#![cfg(target_os = "android")]

use jni::objects::{JClass, JObject};
use jni::sys::{jboolean, jfloat, jfloatArray, jint, jlong};
use jni::JNIEnv;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::desktop::{DesktopEnvironment, SurfaceContact, SurfaceKind};
use crate::fushi::render::FushiRenderer;
use crate::fushi::{FushiBody, MotionMode};
use crate::gpu_canvas::{GpuCanvas, GpuScene, GpuVertex};
use crate::math::{clampf, smoothstep, Vec2};
use crate::wgpu_layer::LayeredFrame;

const MIN_SURFACE_SIZE: u32 = 96;
const FIXED_STEP: f32 = 1.0 / 60.0;
const FAR_CURSOR: Vec2 = Vec2::new(-10000.0, -10000.0);
const ANDROID_MIN_SCALE: f32 = 0.62;
const ANDROID_MAX_SCALE: f32 = 2.42;
const ANDROID_BITMAP_RESULT_SUCCESS: i32 = 0;
const ANDROID_BITMAP_FORMAT_RGBA_8888: i32 = 1;

#[repr(C)]
#[derive(Default)]
struct AndroidBitmapInfo {
    width: u32,
    height: u32,
    stride: u32,
    format: i32,
    flags: u32,
}

#[link(name = "jnigraphics")]
extern "C" {
    fn AndroidBitmap_getInfo(
        env: *mut jni::sys::JNIEnv,
        bitmap: jni::sys::jobject,
        info: *mut AndroidBitmapInfo,
    ) -> i32;
    fn AndroidBitmap_lockPixels(
        env: *mut jni::sys::JNIEnv,
        bitmap: jni::sys::jobject,
        pixels: *mut *mut c_void,
    ) -> i32;
    fn AndroidBitmap_unlockPixels(env: *mut jni::sys::JNIEnv, bitmap: jni::sys::jobject) -> i32;
}

pub struct AndroidFushiEngine {
    env: DesktopEnvironment,
    fushi: FushiBody,
    renderer: FushiRenderer,
    screen_width: i32,
    screen_height: i32,
    surface_width: u32,
    surface_height: u32,
    window_origin: Vec2,
    window_size: (u32, u32),
    last_frame: Option<LayeredFrame>,
    logged_frame_stats: bool,
    accumulator: f32,
    pointer_down: bool,
    density: f32,
}

impl AndroidFushiEngine {
    unsafe fn new(
        surface_width: u32,
        surface_height: u32,
        density: f32,
        screen_width: i32,
        screen_height: i32,
    ) -> Result<Self, String> {
        let surface_width = surface_width.max(MIN_SURFACE_SIZE);
        let surface_height = surface_height.max(MIN_SURFACE_SIZE);
        let density = density.max(0.5);

        let screen_width = screen_width.max(240);
        let screen_height = screen_height.max(160);
        let env_model = DesktopEnvironment::from_screen_size(screen_width, screen_height);
        let mut fushi = FushiBody::new(&env_model);
        let scale = android_fushi_scale(density);
        fushi.set_scale(scale, &env_model);
        fushi.snap_to_contact(
            SurfaceContact::monitor(0, SurfaceKind::Bottom),
            screen_width as f32 * 0.50,
            &env_model,
        );
        keep_android_fushi_visible(&mut fushi, screen_width, screen_height);
        fushi.set_cursor(FAR_CURSOR);

        let (origin, size) = android_window_rect_for_body(&fushi);
        let mut engine = Self {
            env: env_model,
            fushi,
            renderer: FushiRenderer::new(),
            screen_width,
            screen_height,
            surface_width,
            surface_height,
            window_origin: origin,
            window_size: size,
            last_frame: None,
            logged_frame_stats: false,
            accumulator: 0.0,
            pointer_down: false,
            density,
        };
        engine.render();
        Ok(engine)
    }

    fn resize_surface(&mut self, width: u32, height: u32, density: f32) {
        self.surface_width = width.max(MIN_SURFACE_SIZE);
        self.surface_height = height.max(MIN_SURFACE_SIZE);
        self.density = density.max(0.5);
        let scale = android_fushi_scale(self.density);
        self.fushi.set_scale(scale, &self.env);
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
        let scale = android_fushi_scale(self.density);
        self.fushi.set_scale(scale, &self.env);
        if !self.env.virtual_bounds.inflate(900).contains(self.fushi.center) {
            self.fushi.reset_to_safe_surface(&self.env);
        }
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
                self.fushi.set_cursor(FAR_CURSOR);
            }
            (false, false) => {
                self.fushi.set_cursor(FAR_CURSOR);
            }
        }
        self.pointer_down = down;
    }

    fn shake(&mut self, ax: f32, ay: f32, az: f32, dt: f32) {
        let dt = dt.clamp(0.001, 0.060);
        let total = (ax * ax + ay * ay + az * az).sqrt();
        let planar = (ax * ax + ay * ay).sqrt();
        let intensity = smoothstep(2.2, 13.5, total).max(smoothstep(3.0, 10.0, planar));
        if intensity <= 0.002 {
            return;
        }
        // Treat the phone as a transparent physical container: the same Rust Fushi body
        // receives acceleration impulses instead of using a separate Android-only pet.
        let pixels_per_mps2 = 90.0 * self.density.clamp(0.75, 4.0);
        let local_accel = Vec2::new(-ax * pixels_per_mps2, ay * pixels_per_mps2).clamp_len(2200.0);
        self.fushi.apply_external_shake(local_accel, intensity, dt);
    }

    fn step(&mut self, dt: f32, screen_width: i32, screen_height: i32) -> [f32; 4] {
        self.set_screen(screen_width, screen_height);
        let dt = dt.clamp(0.001, 0.050);
        self.accumulator = (self.accumulator + dt).min(0.12);
        while self.accumulator >= FIXED_STEP {
            self.fushi.step(FIXED_STEP, &self.env);
            self.accumulator -= FIXED_STEP;
        }
        if self.accumulator > 0.030 {
            let partial = self.accumulator;
            self.fushi.step(partial, &self.env);
            self.accumulator = 0.0;
        }
        keep_android_fushi_visible(&mut self.fushi, self.screen_width, self.screen_height);
        let (origin, size) = android_window_rect_for_body(&self.fushi);
        self.window_origin = origin;
        self.window_size = size;
        self.render();
        [origin.x, origin.y, size.0 as f32, size.1 as f32]
    }

    fn render(&mut self) {
        let width = self.window_size.0.max(MIN_SURFACE_SIZE);
        let height = self.window_size.1.max(MIN_SURFACE_SIZE);
        let mut canvas = GpuCanvas::new(width, height, self.window_origin, 1.0);
        self.renderer.draw(&mut canvas, &self.fushi);
        let scene = canvas.into_scene();
        let frame = rasterize_scene(&scene);
        if !self.logged_frame_stats {
            let alpha_max = frame.bgra.chunks_exact(4).map(|px| px[3]).max().unwrap_or(0);
            let alpha_pixels = frame.bgra.chunks_exact(4).filter(|px| px[3] != 0).count();
            log::info!(
                "software frame {}x{} alpha_max={} alpha_pixels={}",
                frame.width,
                frame.height,
                alpha_max,
                alpha_pixels
            );
            self.logged_frame_stats = true;
        }
        self.last_frame = Some(frame);
    }
}

fn rasterize_scene(scene: &GpuScene) -> LayeredFrame {
    let width = scene.width.max(1);
    let height = scene.height.max(1);
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    if scene.is_empty() {
        return LayeredFrame { width, height, bgra };
    }

    for triangle in scene.indices.chunks_exact(3) {
        let Some(v0) = scene.vertices.get(triangle[0] as usize).copied() else {
            continue;
        };
        let Some(v1) = scene.vertices.get(triangle[1] as usize).copied() else {
            continue;
        };
        let Some(v2) = scene.vertices.get(triangle[2] as usize).copied() else {
            continue;
        };
        rasterize_triangle(&mut bgra, width, height, v0, v1, v2);
    }

    LayeredFrame { width, height, bgra }
}

fn rasterize_triangle(bgra: &mut [u8], width: u32, height: u32, v0: GpuVertex, v1: GpuVertex, v2: GpuVertex) {
    let p0 = ndc_to_pixel(v0.position, width, height);
    let p1 = ndc_to_pixel(v1.position, width, height);
    let p2 = ndc_to_pixel(v2.position, width, height);
    let area = edge(p0, p1, p2);
    if area.abs() <= 0.0001 {
        return;
    }

    let min_x = p0.0.min(p1.0).min(p2.0).floor().max(0.0) as u32;
    let min_y = p0.1.min(p1.1).min(p2.1).floor().max(0.0) as u32;
    let max_x = p0.0.max(p1.0).max(p2.0).ceil().min(width as f32) as u32;
    let max_y = p0.1.max(p1.1).max(p2.1).ceil().min(height as f32) as u32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(p1, p2, p) / area;
            let w1 = edge(p2, p0, p) / area;
            let w2 = edge(p0, p1, p) / area;
            if w0 < -0.0001 || w1 < -0.0001 || w2 < -0.0001 {
                continue;
            }
            let color = interpolate_color(v0.color, v1.color, v2.color, w0, w1, w2);
            blend_pixel(bgra, width, x, y, color);
        }
    }
}

fn ndc_to_pixel(position: [f32; 2], width: u32, height: u32) -> (f32, f32) {
    (
        (position[0] + 1.0) * 0.5 * width as f32,
        (1.0 - position[1]) * 0.5 * height as f32,
    )
}

fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)
}

fn interpolate_color(c0: [f32; 4], c1: [f32; 4], c2: [f32; 4], w0: f32, w1: f32, w2: f32) -> [f32; 4] {
    [
        (c0[0] * w0 + c1[0] * w1 + c2[0] * w2).clamp(0.0, 1.0),
        (c0[1] * w0 + c1[1] * w1 + c2[1] * w2).clamp(0.0, 1.0),
        (c0[2] * w0 + c1[2] * w1 + c2[2] * w2).clamp(0.0, 1.0),
        (c0[3] * w0 + c1[3] * w1 + c2[3] * w2).clamp(0.0, 1.0),
    ]
}

fn blend_pixel(bgra: &mut [u8], width: u32, x: u32, y: u32, color: [f32; 4]) {
    let src_a = color[3].clamp(0.0, 1.0);
    if src_a <= 0.0 {
        return;
    }
    let src_r = color[0].clamp(0.0, 1.0) * src_a;
    let src_g = color[1].clamp(0.0, 1.0) * src_a;
    let src_b = color[2].clamp(0.0, 1.0) * src_a;
    let index = ((y * width + x) * 4) as usize;
    if index + 3 >= bgra.len() {
        return;
    }

    let dst_b = bgra[index] as f32 / 255.0;
    let dst_g = bgra[index + 1] as f32 / 255.0;
    let dst_r = bgra[index + 2] as f32 / 255.0;
    let dst_a = bgra[index + 3] as f32 / 255.0;
    let inv_a = 1.0 - src_a;

    bgra[index] = to_u8(src_b + dst_b * inv_a);
    bgra[index + 1] = to_u8(src_g + dst_g * inv_a);
    bgra[index + 2] = to_u8(src_r + dst_r * inv_a);
    bgra[index + 3] = to_u8(src_a + dst_a * inv_a);
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn android_fushi_scale(density: f32) -> f32 {
    clampf(density * 0.62, ANDROID_MIN_SCALE, ANDROID_MAX_SCALE)
}

fn android_window_rect_for_body(fushi: &FushiBody) -> (Vec2, (u32, u32)) {
    let bounds = fushi.render_bounds().inflate(18.0);
    let origin = Vec2::new(bounds.left.floor(), bounds.top.floor());
    let width = bounds.width().ceil().max(MIN_SURFACE_SIZE as f32) as u32;
    let height = bounds.height().ceil().max(MIN_SURFACE_SIZE as f32) as u32;
    (origin, (width, height))
}

fn keep_android_fushi_visible(fushi: &mut FushiBody, screen_width: i32, screen_height: i32) {
    let screen_width = screen_width.max(240) as f32;
    let screen_height = screen_height.max(160) as f32;
    let margin = 8.0;
    let bounds = fushi.render_bounds().inflate(18.0);
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

fn empty_float_array(env: &mut JNIEnv<'_>) -> jfloatArray {
    match env.new_float_array(0) {
        Ok(array) => array.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn frame_to_jarray(env: &mut JNIEnv<'_>, frame: [f32; 4]) -> jfloatArray {
    match env.new_float_array(4) {
        Ok(array) => {
            if env.set_float_array_region(&array, 0, &frame).is_ok() {
                array.into_raw()
            } else {
                empty_float_array(env)
            }
        }
        Err(_) => empty_float_array(env),
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
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        AndroidFushiEngine::new(
            surface_width.max(1) as u32,
            surface_height.max(1) as u32,
            density.max(0.5),
            screen_width,
            screen_height,
        )
    })) {
        Ok(Ok(engine)) => Box::into_raw(Box::new(engine)) as jlong,
        Ok(Err(err)) => {
            eprintln!("Desktop Fushi Android init failed: {err}");
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
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut AndroidFushiEngine)) };
    }
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
) -> jfloatArray {
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        engine_mut(ptr).map(|engine| engine.step(dt, screen_width, screen_height))
    })) {
        Ok(Some(frame)) => frame_to_jarray(&mut env, frame),
        _ => empty_float_array(&mut env),
    }
}

#[no_mangle]
pub extern "system" fn Java_net_mercen_desktopfushi_FushiOverlayView_nativeCopyFrame(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
    bitmap: JObject<'_>,
) -> jboolean {
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        let Some(engine) = engine_mut(ptr) else {
            return false;
        };
        let Some(frame) = engine.last_frame.as_ref() else {
            return false;
        };
        copy_frame_to_bitmap(&mut env, bitmap, frame).is_ok()
    })) {
        Ok(true) => 1,
        _ => 0,
    }
}

fn init_android_logging() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("DesktopFushiRust")
            .with_max_level(log::LevelFilter::Info),
    );
}

unsafe fn copy_frame_to_bitmap(
    env: &mut JNIEnv<'_>,
    bitmap: JObject<'_>,
    frame: &LayeredFrame,
) -> Result<(), String> {
    if bitmap.is_null() {
        return Err("bitmap is null".to_string());
    }
    if frame.bgra.is_empty() {
        return Err("frame is empty".to_string());
    }

    let mut info = AndroidBitmapInfo::default();
    let bitmap_obj = bitmap.as_raw();
    let env_ptr = env.get_native_interface();
    let info_result = AndroidBitmap_getInfo(env_ptr, bitmap_obj, &mut info);
    if info_result != ANDROID_BITMAP_RESULT_SUCCESS {
        return Err(format!("AndroidBitmap_getInfo failed: {info_result}"));
    }
    if info.format != ANDROID_BITMAP_FORMAT_RGBA_8888 {
        return Err(format!("unsupported bitmap format: {}", info.format));
    }
    if info.width != frame.width || info.height != frame.height {
        return Err(format!(
            "bitmap/frame size mismatch: bitmap={}x{} frame={}x{}",
            info.width, info.height, frame.width, frame.height
        ));
    }

    let mut pixels: *mut c_void = std::ptr::null_mut();
    let lock_result = AndroidBitmap_lockPixels(env_ptr, bitmap_obj, &mut pixels);
    if lock_result != ANDROID_BITMAP_RESULT_SUCCESS || pixels.is_null() {
        return Err(format!("AndroidBitmap_lockPixels failed: {lock_result}"));
    }

    let result = copy_bgra_to_rgba_pixels(pixels.cast::<u8>(), &info, frame);
    let _ = AndroidBitmap_unlockPixels(env_ptr, bitmap_obj);
    result
}

unsafe fn copy_bgra_to_rgba_pixels(
    pixels: *mut u8,
    info: &AndroidBitmapInfo,
    frame: &LayeredFrame,
) -> Result<(), String> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let stride = info.stride as usize;
    let row_bytes = width * 4;
    if frame.bgra.len() < row_bytes * height {
        return Err("frame buffer is shorter than expected".to_string());
    }
    if stride < row_bytes {
        return Err("bitmap stride is shorter than a frame row".to_string());
    }

    for y in 0..height {
        let src_row = &frame.bgra[y * row_bytes..y * row_bytes + row_bytes];
        let dst_row = std::slice::from_raw_parts_mut(pixels.add(y * stride), row_bytes);
        for x in 0..width {
            let i = x * 4;
            dst_row[i] = src_row[i + 2];
            dst_row[i + 1] = src_row[i + 1];
            dst_row[i + 2] = src_row[i];
            dst_row[i + 3] = src_row[i + 3];
        }
    }

    Ok(())
}
