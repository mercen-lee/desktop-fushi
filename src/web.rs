#![cfg(all(target_arch = "wasm32", feature = "web"))]

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::desktop::{DesktopEnvironment, SurfaceContact, SurfaceKind};
use crate::fushi::constants::BODY_HALF_LENGTH;
use crate::fushi::render::FushiRenderer;
use crate::fushi::{FushiBody, MotionMode};
use crate::gpu_canvas::GpuCanvas;
use crate::math::{clampf, smoothstep, RectI, Vec2};
use crate::wgpu_layer::{WgpuLayer, WgpuSurfaceSize};

const FAR_CURSOR: Vec2 = Vec2::new(-10000.0, -10000.0);
const DEFAULT_DPR: f32 = 1.0;
const WEB_DRAG_RADIUS_SCALE: f32 = 0.58;
const WEB_FUSHI_RENDER_WIDTH_RATIO: f32 = 2.55;
const WEB_EMBED_MIN_SCALE: f32 = 0.20;
const WEB_EMBED_MAX_SCALE: f32 = 1.38;

#[wasm_bindgen]
pub struct WebFushiEngine {
    env: DesktopEnvironment,
    fushi: FushiBody,
    renderer: FushiRenderer,
    wgpu: WgpuLayer,
    width: u32,
    height: u32,
    dpr: f32,
    awakened: bool,
    pointer_down: bool,
    ui_rects: Vec<(isize, RectI)>,
    showcase_anchor: Vec2,
    showcase_mode: bool,
}

#[wasm_bindgen]
impl WebFushiEngine {
    #[wasm_bindgen(js_name = create)]
    pub async fn create(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
        dpr: f32,
        _showcase_line: f32,
    ) -> Result<WebFushiEngine, JsValue> {
        Self::new(canvas, width, height, dpr, true).await
    }

    #[wasm_bindgen(js_name = createDesktop)]
    pub async fn create_desktop(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
        dpr: f32,
        _showcase_line: f32,
    ) -> Result<WebFushiEngine, JsValue> {
        Self::new(canvas, width, height, dpr, false).await
    }

    pub fn resize(&mut self, width: u32, height: u32, dpr: f32, _showcase_line: f32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.dpr = dpr.max(DEFAULT_DPR);
        self.wgpu.resize(self.width, self.height);
        self.showcase_anchor = self.compute_showcase_anchor();
        self.env = self.viewport_environment();
        self.set_responsive_scale();
        if self.showcase_mode {
            self.pin_to_showcase_anchor();
        } else if !self.env.virtual_bounds.inflate(900).contains(self.fushi.center) {
            self.fushi.reset_to_safe_surface(&self.env);
        }
    }

    pub fn pointer(&mut self, x: f32, y: f32, down: bool) -> bool {
        let world = Vec2::new(x, y);
        let hit = self.fushi.interactive_hit_test(world);

        if down {
            if !self.pointer_down && !hit {
                return false;
            }
            self.awaken();
            let target = self.interaction_point(world);
            if self.pointer_down {
                self.fushi.drag_to(target);
            } else if self.fushi.try_begin_drag(world) {
                self.fushi.drag_to(target);
            }
            self.fushi.set_cursor(target);
        } else if self.awakened {
            let target = self.interaction_point(world);
            self.fushi.drag_to(target);
            self.fushi.release_drag();
            self.fushi.set_cursor(target);
        }

        self.pointer_down = down;
        hit
    }

    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let world = Vec2::new(x, y);
        let hit = self.fushi.interactive_hit_test(world);
        if hit {
            self.awaken();
            self.fushi.set_cursor(world);
        } else if self.awakened && !self.pointer_down {
            self.fushi.set_cursor(FAR_CURSOR);
        }
        hit
    }

    #[wasm_bindgen(js_name = setUiRects)]
    pub fn set_ui_rects(&mut self, rects: js_sys::Float32Array) {
        let mut ui_rects = Vec::new();
        let values = rects.length();
        let count = (values / 4).min(24);
        for index in 0..count {
            let base = index * 4;
            let left = rects.get_index(base).round() as i32;
            let top = rects.get_index(base + 1).round() as i32;
            let right = rects.get_index(base + 2).round() as i32;
            let bottom = rects.get_index(base + 3).round() as i32;
            let rect = RectI::new(
                left.clamp(0, self.width as i32),
                top.clamp(0, self.height as i32),
                right.clamp(0, self.width as i32),
                bottom.clamp(0, self.height as i32),
            );
            if rect.width() >= 24 && rect.height() >= 16 {
                ui_rects.push((10_000 + index as isize, rect));
            }
        }

        self.ui_rects = ui_rects;
        if self.awakened {
            self.env = self.viewport_environment();
        }
    }

    pub fn wake(&mut self) {
        self.awaken();
    }

    #[wasm_bindgen(js_name = isAwakened)]
    pub fn is_awakened(&self) -> bool {
        self.awakened
    }

    pub fn shake(&mut self, ax: f32, ay: f32, dt: f32) {
        self.awaken();
        let dt = dt.clamp(0.001, 0.060);
        let accel = Vec2::new(ax, ay).clamp_len(5200.0);
        let intensity = smoothstep(900.0, 3900.0, accel.length());
        if intensity > 0.002 {
            self.fushi.apply_external_shake(accel, intensity, dt);
            if self.showcase_mode {
                self.pin_to_showcase_anchor();
            }
        }
    }

    pub fn tick(&mut self, dt: f32) {
        let _dt = dt.clamp(0.001, 0.050);
        self.fushi.step(_dt, &self.env);
        if self.showcase_mode {
            self.pin_to_showcase_anchor();
        }
        self.render();
    }
}

impl WebFushiEngine {
    async fn new(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
        dpr: f32,
        showcase_mode: bool,
    ) -> Result<WebFushiEngine, JsValue> {
        console_error_panic_hook::set_once();

        let width = width.max(1);
        let height = height.max(1);
        let dpr = dpr.max(DEFAULT_DPR);
        let env = DesktopEnvironment::from_screen_size(width as i32, height as i32);
        let mut fushi = FushiBody::new(&env);
        set_web_fushi_scale(&mut fushi, width, height, dpr, showcase_mode, &env);
        fushi.snap_to_contact(
            SurfaceContact::monitor(0, SurfaceKind::Bottom),
            width as f32 * 0.5,
            &env,
        );
        fushi.set_cursor(FAR_CURSOR);

        let wgpu = WgpuLayer::new_for_canvas(canvas, WgpuSurfaceSize::new(width, height))
            .await
            .map_err(|err| JsValue::from_str(&err))?;

        let mut engine = WebFushiEngine {
            env,
            fushi,
            renderer: FushiRenderer::new(),
            wgpu,
            width,
            height,
            dpr,
            awakened: true,
            pointer_down: false,
            ui_rects: Vec::new(),
            showcase_anchor: Vec2::ZERO,
            showcase_mode,
        };
        engine.showcase_anchor = engine.compute_showcase_anchor();
        if engine.showcase_mode {
            engine.pin_to_showcase_anchor();
        }
        engine.render();
        Ok(engine)
    }

    fn awaken(&mut self) {
        if self.awakened {
            return;
        }
        self.awakened = true;
        self.env = self.viewport_environment();
        self.fushi.set_cursor(FAR_CURSOR);
        if self.showcase_mode {
            self.pin_to_showcase_anchor();
        }
    }

    fn render(&mut self) {
        let mut canvas = GpuCanvas::new(self.width, self.height, Vec2::ZERO, 1.0);
        let mut render_fushi = self.fushi.clone();
        if self.showcase_mode && render_fushi.mode == MotionMode::Flying {
            render_fushi.mode = MotionMode::Attached;
        }
        self.renderer.draw(&mut canvas, &render_fushi);
        let scene = canvas.into_scene();
        self.wgpu.resize(self.width, self.height);
        if let Err(err) = self.wgpu.render(&scene) {
            web_log(&format!("Desktop Fushi web render failed: {err}"));
        }
    }

    fn viewport_environment(&self) -> DesktopEnvironment {
        DesktopEnvironment::from_screen_size(self.width as i32, self.height as i32)
            .with_window_rects(self.ui_rects.iter().copied())
    }

    fn compute_showcase_anchor(&self) -> Vec2 {
        Vec2::new(self.width as f32 * 0.5, self.height as f32 * 0.54)
    }

    fn set_responsive_scale(&mut self) {
        set_web_fushi_scale(
            &mut self.fushi,
            self.width,
            self.height,
            self.dpr,
            self.showcase_mode,
            &self.env,
        );
    }

    fn interaction_point(&self, world: Vec2) -> Vec2 {
        if self.showcase_mode {
            self.clamped_interaction_point(world)
        } else {
            world
        }
    }

    fn clamped_interaction_point(&self, world: Vec2) -> Vec2 {
        let max_radius = BODY_HALF_LENGTH * self.fushi.scale * WEB_DRAG_RADIUS_SCALE;
        self.fushi.center + (world - self.fushi.center).clamp_len(max_radius)
    }

    fn pin_to_showcase_anchor(&mut self) {
        let delta = self.showcase_anchor - self.fushi.center;
        if delta.length_sq() > 0.0001 {
            self.fushi.center += delta;
            for node in &mut self.fushi.mesh.nodes {
                node.pos += delta;
            }
        }
        self.fushi.velocity = Vec2::ZERO;
        if self.fushi.mode != MotionMode::Dragged {
            self.fushi.mode = MotionMode::Flying;
            self.fushi.surface = None;
        }
    }
}

fn web_fushi_scale(width: u32, height: u32, dpr: f32) -> f32 {
    web_fushi_scale_for_mode(width, height, dpr, true)
}

fn web_embed_fushi_scale(width: u32, height: u32, dpr: f32) -> f32 {
    web_fushi_scale_for_mode(width, height, dpr, false)
}

fn web_fushi_scale_for_mode(width: u32, height: u32, dpr: f32, showcase_mode: bool) -> f32 {
    let css_width = width as f32 / dpr.max(DEFAULT_DPR);
    let css_height = height as f32 / dpr.max(DEFAULT_DPR);
    let target_css_width = if showcase_mode {
        clampf(css_width.min(css_height * 1.62) * 0.72, 180.0, 360.0)
    } else {
        clampf(css_width.min(css_height * 1.90) * 0.36, 72.0, 560.0)
    };
    target_css_width * dpr / (BODY_HALF_LENGTH * WEB_FUSHI_RENDER_WIDTH_RATIO)
}

fn set_web_fushi_scale(
    fushi: &mut FushiBody,
    width: u32,
    height: u32,
    dpr: f32,
    showcase_mode: bool,
    env: &DesktopEnvironment,
) {
    if showcase_mode {
        fushi.set_scale(web_fushi_scale(width, height, dpr), env);
    } else {
        fushi.set_scale_with_limits(
            web_embed_fushi_scale(width, height, dpr),
            WEB_EMBED_MIN_SCALE,
            WEB_EMBED_MAX_SCALE,
            env,
        );
    }
}

fn web_log(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
}
