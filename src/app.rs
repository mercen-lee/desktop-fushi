use std::error::Error;
use std::io;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tao::dpi::{PhysicalPosition, PhysicalSize};
use tao::event::{ElementState, Event, MouseButton, StartCause, Touch, TouchPhase, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
#[cfg(target_os = "windows")]
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::window::{Window, WindowBuilder};
#[cfg(target_os = "windows")]
use tray_icon::Icon;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    TrayIcon, TrayIconBuilder,
};

use crate::desktop::{DesktopEnvironment, SurfaceContact, SurfaceKind};
use crate::fushi::render::FushiRenderer;
use crate::fushi::{FushiBody, MotionMode};
use crate::gpu_canvas::GpuCanvas;
use crate::math::Vec2;
use crate::settings::AppSettings;
use crate::wgpu_layer::{WgpuLayer, WgpuSurfaceSize};

const FIXED_STEP: f32 = 1.0 / 60.0;
const MIN_WINDOW_SIZE: u32 = 64;
const ACTIVE_ENV_REFRESH_INTERVAL: f32 = 0.05;
const IDLE_ENV_REFRESH_INTERVAL: f32 = 0.25;
const ACTIVE_RENDER_INTERVAL: f32 = FIXED_STEP;
const IDLE_RENDER_INTERVAL: f32 = 1.0 / 30.0;
const FRAME_TIMER_WAKE: f32 = 0.5;
const ACTIVE_EVENT_LOOP_INTERVAL: Duration = Duration::from_millis(16);
const IDLE_EVENT_LOOP_INTERVAL: Duration = Duration::from_millis(33);
#[cfg(target_os = "macos")]
const MACOS_CURSOR_CAPTURE_MARGIN: f32 = 24.0;
#[cfg(target_os = "macos")]
const MACOS_EVENT_TAP_CAPTURE_RADIUS: f32 = 32.0;

const DEFAULT_ALWAYS_ON_TOP: bool = true;
const DEFAULT_HIDE_WHEN_FULLSCREEN: bool = false;
const APP_DISPLAY_NAME: &str = "Desktop Fushi";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const APP_VERSION_TEXT: &str = concat!("Desktop Fushi v", env!("CARGO_PKG_VERSION"));
#[cfg(any(target_os = "windows", target_os = "macos"))]
const APP_WEBSITE_URL: &str = "https://desktopfushi.mercen.net";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const DEVELOPER_CREDIT_TEXT: &str = "Developed by Mercen && Rian";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const CHARACTER_COPYRIGHT_TEXT: &str = "Character copyright belongs to TWIN ENGINE";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const WEBSITE_MENU_ID: &str = "desktop-fushi.website";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const QUIT_MENU_ID: &str = "desktop-fushi.quit";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const SIZE_SMALL_MENU_ID: &str = "desktop-fushi.size.small";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const SIZE_NORMAL_MENU_ID: &str = "desktop-fushi.size.normal";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const SIZE_LARGE_MENU_ID: &str = "desktop-fushi.size.large";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const SIZE_HUGE_MENU_ID: &str = "desktop-fushi.size.huge";
#[cfg(any(target_os = "windows", target_os = "macos"))]
const WINDOW_INTERACTION_MENU_ID: &str = "desktop-fushi.windows.interaction";
#[cfg(target_os = "windows")]
const START_ON_LOGIN_MENU_ID: &str = "desktop-fushi.start-on-login";

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FushiSizePreset {
    Small,
    Normal,
    Large,
    Huge,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl FushiSizePreset {
    fn from_menu_id(id: &str) -> Option<Self> {
        match id {
            SIZE_SMALL_MENU_ID => Some(Self::Small),
            SIZE_NORMAL_MENU_ID => Some(Self::Normal),
            SIZE_LARGE_MENU_ID => Some(Self::Large),
            SIZE_HUGE_MENU_ID => Some(Self::Huge),
            _ => None,
        }
    }

    fn from_scale(scale: f32) -> Self {
        let mut best = Self::Normal;
        let mut best_delta = f32::MAX;
        for preset in [Self::Small, Self::Normal, Self::Large, Self::Huge] {
            let delta = (preset.scale() - scale).abs();
            if delta < best_delta {
                best = preset;
                best_delta = delta;
            }
        }
        best
    }

    fn scale(self) -> f32 {
        match self {
            Self::Small => crate::settings::SMALL_FUSHI_SCALE,
            Self::Normal => crate::settings::NORMAL_FUSHI_SCALE,
            Self::Large => crate::settings::LARGE_FUSHI_SCALE,
            Self::Huge => crate::settings::HUGE_FUSHI_SCALE,
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Debug, Clone)]
enum AppEvent {
    Menu(MenuEvent),
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[derive(Debug, Clone)]
enum AppEvent {}

pub fn run() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let proxy = event_loop.create_proxy();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(AppEvent::Menu(event));
        }));
    }

    let mut app = App::new(&event_loop).map_err(setup_error)?;

    event_loop.run(move |event, _, control_flow| {
        let mut should_exit = false;
        match event {
            #[cfg(target_os = "macos")]
            Event::NewEvents(StartCause::Init) => {
                app.ensure_macos_status_menu();
            }
            Event::WindowEvent { window_id, event, .. } if window_id == app.window.id() => match event {
                WindowEvent::CloseRequested => {
                    should_exit = true;
                }
                WindowEvent::Resized(size) => {
                    app.window_size = logical_size_from_physical(app.window.as_ref(), size);
                    app.render_timer = FRAME_TIMER_WAKE;
                }
                WindowEvent::Touch(touch) => {
                    app.handle_touch(touch);
                }
                #[cfg(target_os = "macos")]
                WindowEvent::CursorMoved { position, .. } => {
                    app.handle_native_cursor_moved(position);
                }
                #[cfg(target_os = "macos")]
                WindowEvent::MouseInput { state, button, .. } => {
                    app.handle_native_mouse_input(state, button);
                }
                _ => {}
            },
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            Event::UserEvent(AppEvent::Menu(event)) => {
                if app.handle_menu_event(&event) {
                    should_exit = true;
                }
            }
            Event::MainEventsCleared => {
                #[cfg(target_os = "windows")]
                if app.restart_requested() {
                    should_exit = true;
                } else {
                    app.tick();
                }

                #[cfg(not(target_os = "windows"))]
                app.tick();
            }
            _ => {}
        }

        *control_flow = if should_exit {
            ControlFlow::Exit
        } else {
            ControlFlow::WaitUntil(Instant::now() + app.next_event_loop_interval())
        };
    });
}

fn setup_error(message: String) -> Box<dyn Error> {
    Box::new(io::Error::other(message))
}

fn preferred_initial_cursor(env: &DesktopEnvironment) -> Vec2 {
    #[cfg(target_os = "windows")]
    unsafe {
        let cursor = crate::win32::cursor_pos();
        if env.virtual_bounds.inflate(16).contains(cursor) {
            return cursor;
        }
    }

    #[cfg(target_os = "macos")]
    if let Some(cursor) = crate::macos::cursor_pos() {
        if env.virtual_bounds.inflate(16).contains(cursor) {
            return cursor;
        }
    }

    env.initial_center()
}

struct App {
    window: Arc<Window>,
    wgpu: WgpuLayer,
    env: DesktopEnvironment,
    fushi: FushiBody,
    renderer: FushiRenderer,
    settings: AppSettings,
    hidden_for_fullscreen: bool,
    last_frame: Instant,
    accumulator: f32,
    fullscreen_timer: f32,
    env_timer: f32,
    render_timer: f32,
    window_origin: Vec2,
    window_size: PhysicalSize<u32>,
    cursor_world: Vec2,
    mouse_down: bool,
    cursor_hittest: bool,
    native_window_visible: bool,
    #[cfg(target_os = "windows")]
    touch_input_available: bool,
    #[cfg(target_os = "windows")]
    active_touch_id: Option<u64>,
    #[cfg(target_os = "windows")]
    active_touch_world: Option<Vec2>,
    #[cfg(target_os = "macos")]
    native_mouse_down: bool,
    #[cfg(target_os = "macos")]
    _mouse_event_tap: Option<crate::macos::MouseEventTap>,
    #[cfg(target_os = "macos")]
    mac_debug_timer: f32,
    #[cfg(target_os = "windows")]
    start_on_login_enabled: bool,
    #[cfg(target_os = "windows")]
    _tray_menu: TrayMenu,
    #[cfg(target_os = "macos")]
    mac_status_menu: Option<MacStatusMenu>,
    #[cfg(target_os = "windows")]
    layered_window_buffer: Option<crate::win32::LayeredWindowBuffer>,
    #[cfg(target_os = "windows")]
    restart_event: Option<crate::single_instance::RestartEvent>,
}

impl App {
    fn new(event_loop: &EventLoop<AppEvent>) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        crate::macos::ensure_app_bundle_launch()?;
        #[cfg(target_os = "macos")]
        crate::macos::ensure_accessibility_permission()?;
        #[cfg(target_os = "macos")]
        crate::macos::reset_debug_log();
        #[cfg(target_os = "macos")]
        let mouse_event_tap = match crate::macos::MouseEventTap::new() {
            Ok(tap) => Some(tap),
            Err(err) => {
                eprintln!("{err}");
                None
            }
        };

        let settings = AppSettings::load();
        let mut env = DesktopEnvironment::capture();
        let cursor_world = preferred_initial_cursor(&env);
        let monitor_index = env.monitor_index_for_point(cursor_world);
        let mut fushi = FushiBody::new(&env);
        fushi.set_scale(settings.fushi_scale, &env);
        fushi.snap_to_contact(
            SurfaceContact::monitor(monitor_index, SurfaceKind::Bottom),
            cursor_world.x,
            &env,
        );

        let (origin, size) = window_rect_for_body(&fushi);

        let window_builder = WindowBuilder::new()
            .with_title(APP_DISPLAY_NAME)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_focused(false)
            .with_always_on_top(DEFAULT_ALWAYS_ON_TOP)
            .with_visible(false)
            .with_position(PhysicalPosition::new(origin.x as i32, origin.y as i32))
            .with_inner_size(size);
        #[cfg(not(target_os = "macos"))]
        let window_builder = window_builder.with_focusable(false);
        #[cfg(target_os = "windows")]
        let window_builder = window_builder.with_skip_taskbar(true);

        let window = window_builder
            .build(event_loop)
            .map_err(|err| format!("failed to create fushi window: {err}"))?;
        let window = Arc::new(window);
        window.set_title(APP_DISPLAY_NAME);
        window.set_outer_position(physical_position_for_window(window.as_ref(), origin));
        window.set_inner_size(physical_size_for_window(window.as_ref(), size));

        #[cfg(target_os = "windows")]
        if let Some(hwnd) = crate::win32::hwnd_from_window(window.as_ref()) {
            unsafe { crate::win32::configure_pet_window(hwnd) };
        }
        #[cfg(target_os = "macos")]
        crate::macos::configure_pet_window(window.as_ref());
        #[cfg(target_os = "windows")]
        let touch_input_available = unsafe { crate::win32::touch_input_available() };
        #[cfg(not(target_os = "windows"))]
        let touch_input_available = false;
        set_window_click_through(window.as_ref(), !touch_input_available);

        let wgpu = pollster::block_on(WgpuLayer::new(
            window.clone(),
            WgpuSurfaceSize::new(
                physical_size_for_window(window.as_ref(), size).width,
                physical_size_for_window(window.as_ref(), size).height,
            ),
        ))?;

        env = capture_environment(Some(window.as_ref()), None, 0.0, settings.interact_with_windows);
        #[cfg(target_os = "windows")]
        let start_on_login_enabled = crate::win32::start_on_login_enabled().unwrap_or_else(|err| {
            eprintln!("{err}");
            false
        });
        let mut app = Self {
            window,
            wgpu,
            env,
            fushi,
            renderer: FushiRenderer::new(),
            settings,
            hidden_for_fullscreen: false,
            last_frame: Instant::now(),
            accumulator: 0.0,
            fullscreen_timer: 0.0,
            env_timer: 0.0,
            render_timer: FRAME_TIMER_WAKE,
            window_origin: origin,
            window_size: size,
            cursor_world,
            mouse_down: false,
            cursor_hittest: false,
            native_window_visible: false,
            #[cfg(target_os = "windows")]
            touch_input_available,
            #[cfg(target_os = "windows")]
            active_touch_id: None,
            #[cfg(target_os = "windows")]
            active_touch_world: None,
            #[cfg(target_os = "macos")]
            native_mouse_down: false,
            #[cfg(target_os = "macos")]
            _mouse_event_tap: mouse_event_tap,
            #[cfg(target_os = "macos")]
            mac_debug_timer: 0.0,
            #[cfg(target_os = "windows")]
            start_on_login_enabled,
            #[cfg(target_os = "windows")]
            _tray_menu: TrayMenu::new(&settings, start_on_login_enabled)?,
            #[cfg(target_os = "macos")]
            mac_status_menu: None,
            #[cfg(target_os = "windows")]
            layered_window_buffer: None,
            #[cfg(target_os = "windows")]
            restart_event: crate::single_instance::RestartEvent::new(),
        };
        app.hide_native_frame();
        app.render();
        #[cfg(target_os = "macos")]
        app.write_macos_debug_snapshot("startup");
        Ok(app)
    }

    #[cfg(target_os = "macos")]
    fn ensure_macos_status_menu(&mut self) {
        if self.mac_status_menu.is_some() {
            return;
        }

        match MacStatusMenu::new(&self.settings) {
            Ok(menu) => {
                self.mac_status_menu = Some(menu);
            }
            Err(err) => {
                eprintln!("{err}");
            }
        }
    }

    fn tick(&mut self) {
        if self.advance() {
            self.render();
        }
    }

    fn advance(&mut self) -> bool {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        self.accumulator = (self.accumulator + dt).min(0.09);
        self.render_timer = (self.render_timer + dt).min(FRAME_TIMER_WAKE);
        #[cfg(target_os = "macos")]
        {
            self.mac_debug_timer += dt;
            if self.mac_debug_timer >= 0.5 {
                self.mac_debug_timer = 0.0;
                self.write_macos_debug_snapshot("tick");
            }
        }

        let cursor = self.current_cursor_world();
        self.cursor_world = cursor;
        self.fushi.set_cursor(cursor);
        self.update_mouse_drag(cursor);

        let mut updated = false;
        while self.accumulator >= FIXED_STEP {
            self.update_fixed(FIXED_STEP);
            self.accumulator -= FIXED_STEP;
            updated = true;
        }

        self.update_cursor_hittest();

        let render_interval = if self.high_activity() {
            ACTIVE_RENDER_INTERVAL
        } else {
            IDLE_RENDER_INTERVAL
        };
        let should_render = updated && !self.hidden_for_fullscreen && self.render_timer >= render_interval;
        if should_render {
            self.render_timer = 0.0;
        }
        should_render
    }

    fn update_fixed(&mut self, dt: f32) {
        self.env_timer += dt;
        if self.env_timer > self.environment_refresh_interval() {
            let elapsed = self.env_timer;
            self.env_timer = 0.0;
            self.refresh_environment(elapsed);
        }

        self.fushi.step(dt, &self.env);

        if DEFAULT_HIDE_WHEN_FULLSCREEN {
            self.fullscreen_timer += dt;
            if self.fullscreen_timer > 0.35 {
                self.fullscreen_timer = 0.0;
                self.update_fullscreen_visibility();
            }
        }
    }

    fn refresh_environment(&mut self, dt: f32) {
        self.env = capture_environment(
            Some(self.window.as_ref()),
            Some(&self.env),
            dt,
            self.settings.interact_with_windows,
        );
    }

    fn high_activity(&self) -> bool {
        self.mouse_down
            || self.touch_down()
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

    fn environment_refresh_interval(&self) -> f32 {
        if self.high_activity() {
            ACTIVE_ENV_REFRESH_INTERVAL
        } else {
            IDLE_ENV_REFRESH_INTERVAL
        }
    }

    fn next_event_loop_interval(&self) -> Duration {
        if self.high_activity() {
            ACTIVE_EVENT_LOOP_INTERVAL
        } else {
            IDLE_EVENT_LOOP_INTERVAL
        }
    }

    #[cfg(target_os = "windows")]
    fn restart_requested(&self) -> bool {
        self.restart_event
            .as_ref()
            .map(|event| event.requested())
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    fn touch_down(&self) -> bool {
        self.active_touch_id.is_some()
    }

    #[cfg(not(target_os = "windows"))]
    fn touch_down(&self) -> bool {
        false
    }

    fn handle_touch(&mut self, touch: Touch) {
        let world = self.touch_world(touch.location);
        match touch.phase {
            TouchPhase::Started => self.start_touch_drag(touch.id, world),
            TouchPhase::Moved => self.move_touch_drag(touch.id, world),
            TouchPhase::Ended | TouchPhase::Cancelled => self.end_touch_drag(touch.id, world),
            _ => {}
        }
    }

    #[cfg(target_os = "macos")]
    fn handle_native_cursor_moved(&mut self, local: PhysicalPosition<f64>) {
        let world = self.touch_world(local);
        self.cursor_world = world;
        self.fushi.set_cursor(world);
        if self.native_mouse_down || self.mouse_down {
            self.drag_to(world);
            self.render_timer = FRAME_TIMER_WAKE;
        }
    }

    #[cfg(target_os = "macos")]
    fn handle_native_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if button != MouseButton::Left {
            return;
        }

        match state {
            ElementState::Pressed => {
                self.native_mouse_down = true;
                if self.cursor_capture_hit_test_at(self.cursor_world) {
                    self.mouse_down = self.begin_drag(self.cursor_world);
                    self.render_timer = FRAME_TIMER_WAKE;
                }
            }
            ElementState::Released => {
                self.native_mouse_down = false;
                if self.mouse_down {
                    self.release_drag(self.cursor_world);
                    self.render_timer = FRAME_TIMER_WAKE;
                }
                self.mouse_down = false;
            }
            _ => {}
        }
    }

    fn touch_world(&self, local: PhysicalPosition<f64>) -> Vec2 {
        self.window_origin + Vec2::new(local.x as f32, local.y as f32) / window_scale(self.window.as_ref())
    }

    #[cfg(target_os = "windows")]
    fn start_touch_drag(&mut self, id: u64, world: Vec2) {
        if self.active_touch_id.is_some() || !self.fushi.interactive_hit_test(world) {
            return;
        }
        self.active_touch_id = Some(id);
        self.active_touch_world = Some(world);
        self.begin_drag(world);
        self.render_timer = FRAME_TIMER_WAKE;
    }

    #[cfg(not(target_os = "windows"))]
    fn start_touch_drag(&mut self, _id: u64, world: Vec2) {
        if self.fushi.interactive_hit_test(world) {
            self.begin_drag(world);
            self.render_timer = FRAME_TIMER_WAKE;
        }
    }

    #[cfg(target_os = "windows")]
    fn move_touch_drag(&mut self, id: u64, world: Vec2) {
        if self.active_touch_id != Some(id) {
            return;
        }
        self.active_touch_world = Some(world);
        self.drag_to(world);
        self.render_timer = FRAME_TIMER_WAKE;
    }

    #[cfg(not(target_os = "windows"))]
    fn move_touch_drag(&mut self, _id: u64, world: Vec2) {
        self.drag_to(world);
        self.render_timer = FRAME_TIMER_WAKE;
    }

    #[cfg(target_os = "windows")]
    fn end_touch_drag(&mut self, id: u64, world: Vec2) {
        if self.active_touch_id != Some(id) {
            return;
        }
        self.active_touch_id = None;
        self.active_touch_world = None;
        self.release_drag(world);
        self.render_timer = FRAME_TIMER_WAKE;
    }

    #[cfg(not(target_os = "windows"))]
    fn end_touch_drag(&mut self, _id: u64, world: Vec2) {
        self.release_drag(world);
        self.render_timer = FRAME_TIMER_WAKE;
    }

    #[cfg(target_os = "windows")]
    fn handle_menu_event(&mut self, event: &MenuEvent) -> bool {
        let id = event.id.as_ref();
        if id == QUIT_MENU_ID {
            return true;
        }

        if let Some(preset) = FushiSizePreset::from_menu_id(id) {
            self.set_fushi_scale(preset.scale());
            return false;
        }

        match id {
            WEBSITE_MENU_ID => {
                open_website();
            }
            WINDOW_INTERACTION_MENU_ID => {
                self.set_window_interaction(!self.settings.interact_with_windows);
            }
            START_ON_LOGIN_MENU_ID => {
                self.set_start_on_login(!self.start_on_login_enabled);
            }
            _ => {}
        }
        false
    }

    #[cfg(target_os = "macos")]
    fn handle_menu_event(&mut self, event: &MenuEvent) -> bool {
        let id = event.id.as_ref();
        if id == QUIT_MENU_ID {
            return true;
        }

        if let Some(preset) = FushiSizePreset::from_menu_id(id) {
            self.set_fushi_scale(preset.scale());
            return false;
        }

        match id {
            WEBSITE_MENU_ID => {
                open_website();
            }
            WINDOW_INTERACTION_MENU_ID => {
                self.set_window_interaction(!self.settings.interact_with_windows);
            }
            _ => {}
        }
        false
    }

    fn set_fushi_scale(&mut self, scale: f32) {
        self.settings.fushi_scale = crate::settings::clamp_scale(scale);
        self.fushi.set_scale(self.settings.fushi_scale, &self.env);
        self.persist_settings();
        self.update_menu_state();
        self.render_timer = FRAME_TIMER_WAKE;
    }

    fn set_window_interaction(&mut self, enabled: bool) {
        if self.settings.interact_with_windows == enabled {
            return;
        }
        self.settings.interact_with_windows = enabled;
        self.refresh_environment(0.0);
        self.env_timer = ACTIVE_ENV_REFRESH_INTERVAL;
        self.persist_settings();
        self.update_menu_state();
        self.render_timer = FRAME_TIMER_WAKE;
    }

    #[cfg(target_os = "windows")]
    fn set_start_on_login(&mut self, enabled: bool) {
        match crate::win32::set_start_on_login(enabled) {
            Ok(()) => {
                self.start_on_login_enabled = enabled;
            }
            Err(err) => {
                eprintln!("{err}");
            }
        }
        self.update_menu_state();
    }

    fn persist_settings(&self) {
        if let Err(err) = self.settings.save() {
            eprintln!("{err}");
        }
    }

    #[cfg(target_os = "windows")]
    fn update_menu_state(&self) {
        self._tray_menu
            .set_state(&self.settings, self.start_on_login_enabled);
    }

    #[cfg(target_os = "macos")]
    fn update_menu_state(&self) {
        if let Some(menu) = &self.mac_status_menu {
            menu.set_state(&self.settings);
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn update_menu_state(&self) {}

    fn render(&mut self) {
        if self.hidden_for_fullscreen {
            return;
        }

        let (origin, size) = self.target_window_rect();
        let moved =
            (origin.x - self.window_origin.x).abs() >= 1.0 || (origin.y - self.window_origin.y).abs() >= 1.0;
        let resized = size != self.window_size;
        if moved {
            self.window_origin = origin;
            #[cfg(not(target_os = "windows"))]
            self.window
                .set_outer_position(physical_position_for_window(self.window.as_ref(), origin));
        }
        if resized {
            self.window_size = size;
            #[cfg(not(target_os = "windows"))]
            self.window
                .set_inner_size(physical_size_for_window(self.window.as_ref(), size));
        }

        let scale = window_scale(self.window.as_ref());
        let physical_size = physical_size_for_window(self.window.as_ref(), size);
        let mut canvas = GpuCanvas::new(physical_size.width, physical_size.height, origin, scale);
        self.renderer.draw(&mut canvas, &self.fushi);
        let scene = canvas.into_scene();

        self.wgpu.resize(physical_size.width, physical_size.height);
        match self.wgpu.render(&scene) {
            Ok(_frame) => {
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = crate::win32::hwnd_from_window(self.window.as_ref()) {
                    unsafe { crate::win32::ensure_pet_window_styles(hwnd) };
                    if self
                        .layered_window_buffer
                        .as_ref()
                        .map(|buffer| !buffer.matches(_frame.width, _frame.height))
                        .unwrap_or(true)
                    {
                        self.layered_window_buffer = match unsafe {
                            crate::win32::LayeredWindowBuffer::new(_frame.width, _frame.height)
                        } {
                            Ok(buffer) => Some(buffer),
                            Err(err) => {
                                eprintln!("{err}");
                                None
                            }
                        };
                    }
                    if let Some(buffer) = self.layered_window_buffer.as_mut() {
                        if let Err(err) =
                            unsafe { buffer.update(hwnd, origin.x as i32, origin.y as i32, &_frame.bgra) }
                        {
                            eprintln!("{err}");
                        }
                    }
                    if !self.native_window_visible {
                        unsafe { crate::win32::show_pet_window(hwnd) };
                        self.native_window_visible = true;
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    if !self.native_window_visible {
                        self.window.set_visible(true);
                        self.native_window_visible = true;
                    }
                }
            }
            Err(err) => {
                eprintln!("{err}");
            }
        }
    }

    fn target_window_rect(&self) -> (Vec2, PhysicalSize<u32>) {
        window_rect_for_body(&self.fushi)
    }

    fn current_cursor_world(&self) -> Vec2 {
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(world) = self.active_touch_world {
                return world;
            }
            crate::win32::cursor_pos()
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(cursor) = crate::macos::event_tap_cursor_pos() {
                return cursor;
            }
            crate::macos::cursor_pos().unwrap_or(self.cursor_world)
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            self.cursor_world
        }
    }

    fn update_mouse_drag(&mut self, cursor: Vec2) {
        #[cfg(target_os = "windows")]
        if self.active_touch_id.is_some() {
            self.mouse_down = false;
            return;
        }

        #[cfg(target_os = "windows")]
        let down = unsafe { crate::win32::left_mouse_down() };

        #[cfg(target_os = "macos")]
        let down = {
            let system_down = crate::macos::left_mouse_down();
            if !system_down {
                self.native_mouse_down = false;
            }
            system_down || self.native_mouse_down
        };

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let down = false;

        let hit = self.cursor_capture_hit_test_at(cursor);
        if down && self.mouse_down {
            self.drag_to(cursor);
        } else if down {
            #[cfg(target_os = "macos")]
            let captured_origin = crate::macos::event_tap_drag_origin();
            #[cfg(not(target_os = "macos"))]
            let captured_origin: Option<Vec2> = None;

            if let Some(origin) = captured_origin {
                self.mouse_down = self.begin_captured_drag(origin, cursor);
            } else if hit || self.cursor_hittest {
                self.mouse_down = self.begin_drag(cursor);
            }
        } else if self.mouse_down {
            self.release_drag(cursor);
            self.mouse_down = false;
        } else {
            self.mouse_down = false;
        }
    }

    #[cfg(target_os = "macos")]
    fn write_macos_debug_snapshot(&self, label: &str) {
        if !crate::macos::debug_enabled() {
            return;
        }

        let windows = self
            .env
            .windows
            .iter()
            .take(10)
            .map(|window| {
                format!(
                    "{}:m{} rect=({},{} {}x{}) vel=({:.1},{:.1})",
                    window.id,
                    window.monitor_index,
                    window.rect.left,
                    window.rect.top,
                    window.rect.width(),
                    window.rect.height(),
                    window.velocity.x,
                    window.velocity.y
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let monitors = self
            .env
            .monitors
            .iter()
            .enumerate()
            .map(|(index, monitor)| {
                format!(
                    "{} bounds=({},{} {}x{}) work=({},{} {}x{}) primary={}",
                    index,
                    monitor.bounds.left,
                    monitor.bounds.top,
                    monitor.bounds.width(),
                    monitor.bounds.height(),
                    monitor.work.left,
                    monitor.work.top,
                    monitor.work.width(),
                    monitor.work.height(),
                    monitor.primary
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        crate::macos::append_debug_log(&format!(
            "{label} cursor=({:.1},{:.1}) hit={} down={} native_down={} tap_down={} mode={:?} surface={:?} center=({:.1},{:.1}) velocity=({:.1},{:.1}) window=({:.1},{:.1} {}x{}) virtual=({},{} {}x{}) monitors=[{}] windows=[{}]",
            self.cursor_world.x,
            self.cursor_world.y,
            self.cursor_hittest,
            self.mouse_down,
            self.native_mouse_down,
            crate::macos::event_tap_left_mouse_down(),
            self.fushi.mode,
            self.fushi.surface,
            self.fushi.center.x,
            self.fushi.center.y,
            self.fushi.velocity.x,
            self.fushi.velocity.y,
            self.window_origin.x,
            self.window_origin.y,
            self.window_size.width,
            self.window_size.height,
            self.env.virtual_bounds.left,
            self.env.virtual_bounds.top,
            self.env.virtual_bounds.width(),
            self.env.virtual_bounds.height(),
            monitors,
            windows
        ));
    }

    fn begin_drag(&mut self, world: Vec2) -> bool {
        self.cursor_world = world;
        self.fushi.set_cursor(world);
        #[cfg(target_os = "macos")]
        {
            self.fushi
                .try_begin_drag_with_margin(world, MACOS_CURSOR_CAPTURE_MARGIN)
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.fushi.try_begin_drag(world)
        }
    }

    fn begin_captured_drag(&mut self, origin: Vec2, cursor: Vec2) -> bool {
        self.cursor_world = cursor;
        self.fushi.set_cursor(origin);
        if !self.fushi.begin_drag_unchecked(origin) {
            self.fushi.set_cursor(cursor);
            return false;
        }
        self.fushi.set_cursor(cursor);
        self.fushi.drag_to(cursor);
        true
    }

    fn drag_to(&mut self, world: Vec2) {
        self.cursor_world = world;
        self.fushi.set_cursor(world);
        if self.fushi.mode == MotionMode::Dragged {
            self.fushi.drag_to(world);
        }
    }

    fn release_drag(&mut self, world: Vec2) {
        self.drag_to(world);
        if self.fushi.mode == MotionMode::Dragged {
            self.fushi.release_drag();
            self.env_timer = ACTIVE_ENV_REFRESH_INTERVAL;
        }
    }

    fn update_fullscreen_visibility(&mut self) {
        let should_hide = self.should_hide_for_fullscreen();
        if should_hide == self.hidden_for_fullscreen {
            return;
        }
        self.hidden_for_fullscreen = should_hide;
        if should_hide {
            self.hide_native_window();
            self.native_window_visible = false;
        } else {
            self.window.set_always_on_top(DEFAULT_ALWAYS_ON_TOP);
            set_window_click_through(self.window.as_ref(), self.should_click_through());
            self.render_timer = FRAME_TIMER_WAKE;
        }
    }

    #[cfg(target_os = "windows")]
    fn should_hide_for_fullscreen(&self) -> bool {
        if !DEFAULT_HIDE_WHEN_FULLSCREEN {
            return false;
        }
        let Some(hwnd) = crate::win32::hwnd_from_window(self.window.as_ref()) else {
            return false;
        };
        unsafe { crate::win32::foreground_fullscreen_except(hwnd) }
    }

    #[cfg(not(target_os = "windows"))]
    fn should_hide_for_fullscreen(&self) -> bool {
        false
    }

    fn update_cursor_hittest(&mut self) {
        let hit = self.cursor_capture_hit_test();
        #[cfg(target_os = "macos")]
        crate::macos::set_event_tap_capture_region(hit, self.cursor_world, MACOS_EVENT_TAP_CAPTURE_RADIUS);
        if hit == self.cursor_hittest {
            return;
        }
        self.cursor_hittest = hit;
        set_window_click_through(self.window.as_ref(), self.should_click_through());
    }

    fn cursor_capture_hit_test(&self) -> bool {
        self.cursor_capture_hit_test_at(self.cursor_world)
    }

    fn cursor_capture_hit_test_at(&self, world: Vec2) -> bool {
        if self.fushi.mode == MotionMode::Dragged {
            return true;
        }

        #[cfg(target_os = "macos")]
        {
            return self.fushi.hit_test(world, MACOS_CURSOR_CAPTURE_MARGIN);
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.fushi.interactive_hit_test(world)
        }
    }

    fn should_click_through(&self) -> bool {
        !self.cursor_hittest && !self.touch_capture_ready()
    }

    #[cfg(target_os = "windows")]
    fn touch_capture_ready(&self) -> bool {
        self.touch_input_available
    }

    #[cfg(not(target_os = "windows"))]
    fn touch_capture_ready(&self) -> bool {
        false
    }

    fn hide_native_frame(&self) {
        self.window.set_decorations(false);
        #[cfg(target_os = "windows")]
        self.window.set_undecorated_shadow(false);
    }

    fn hide_native_window(&self) {
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = crate::win32::hwnd_from_window(self.window.as_ref()) {
            unsafe { crate::win32::hide_pet_window(hwnd) };
        }

        #[cfg(not(target_os = "windows"))]
        self.window.set_visible(false);
    }
}

#[cfg(target_os = "windows")]
fn open_website() {
    if let Err(err) = Command::new("cmd")
        .args(["/C", "start", "", APP_WEBSITE_URL])
        .spawn()
    {
        eprintln!("failed to open Desktop Fushi website: {err}");
    }
}

#[cfg(target_os = "macos")]
fn open_website() {
    if let Err(err) = Command::new("open").arg(APP_WEBSITE_URL).spawn() {
        eprintln!("failed to open Desktop Fushi website: {err}");
    }
}

#[cfg(target_os = "windows")]
struct TrayMenu {
    _tray_icon: TrayIcon,
    size_small: CheckMenuItem,
    size_normal: CheckMenuItem,
    size_large: CheckMenuItem,
    size_huge: CheckMenuItem,
    interact_with_windows: CheckMenuItem,
    start_on_login: CheckMenuItem,
}

#[cfg(target_os = "windows")]
impl TrayMenu {
    fn new(settings: &AppSettings, start_on_login_enabled: bool) -> Result<Self, String> {
        let menu = Menu::new();
        let settings_menu = Submenu::new("Settings", true);
        let credits_menu = Submenu::new("Credits", true);
        let size_menu = Submenu::new("Fushi size", true);

        let size_small = CheckMenuItem::with_id(SIZE_SMALL_MENU_ID, "Small", true, false, None);
        let size_normal = CheckMenuItem::with_id(SIZE_NORMAL_MENU_ID, "Normal", true, false, None);
        let size_large = CheckMenuItem::with_id(SIZE_LARGE_MENU_ID, "Large", true, false, None);
        let size_huge = CheckMenuItem::with_id(SIZE_HUGE_MENU_ID, "Huge", true, false, None);
        size_menu
            .append_items(&[&size_small, &size_normal, &size_large, &size_huge])
            .map_err(|err| format!("failed to create tray menu: {err}"))?;

        let interact_with_windows = CheckMenuItem::with_id(
            WINDOW_INTERACTION_MENU_ID,
            "Walk on windows",
            true,
            settings.interact_with_windows,
            None,
        );
        let start_on_login = CheckMenuItem::with_id(
            START_ON_LOGIN_MENU_ID,
            "Start on login",
            true,
            start_on_login_enabled,
            None,
        );
        settings_menu
            .append_items(&[&size_menu, &interact_with_windows, &start_on_login])
            .map_err(|err| format!("failed to create tray menu: {err}"))?;

        let credits_version = MenuItem::with_id(WEBSITE_MENU_ID, APP_VERSION_TEXT, true, None);
        let developer_credit = MenuItem::new(DEVELOPER_CREDIT_TEXT, false, None);
        let character_copyright = MenuItem::new(CHARACTER_COPYRIGHT_TEXT, false, None);
        credits_menu
            .append_items(&[&credits_version, &developer_credit, &character_copyright])
            .map_err(|err| format!("failed to create tray menu: {err}"))?;

        let separator = PredefinedMenuItem::separator();
        let quit = MenuItem::with_id(QUIT_MENU_ID, "Quit", true, None);
        menu.append_items(&[&settings_menu, &credits_menu, &separator, &quit])
            .map_err(|err| format!("failed to create tray menu: {err}"))?;

        let icon = Icon::from_resource(1, Some((32, 32)))
            .or_else(|_| Icon::from_path("assets/desktop-fushi.ico", Some((32, 32))))
            .map_err(|err| format!("failed to load tray icon: {err}"))?;
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(APP_DISPLAY_NAME)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(true)
            .with_menu_on_right_click(true)
            .build()
            .map_err(|err| format!("failed to create tray icon: {err}"))?;

        let tray_menu = Self {
            _tray_icon: tray_icon,
            size_small,
            size_normal,
            size_large,
            size_huge,
            interact_with_windows,
            start_on_login,
        };
        tray_menu.set_state(settings, start_on_login_enabled);
        Ok(tray_menu)
    }

    fn set_state(&self, settings: &AppSettings, start_on_login_enabled: bool) {
        let size = FushiSizePreset::from_scale(settings.fushi_scale);
        self.size_small.set_checked(size == FushiSizePreset::Small);
        self.size_normal.set_checked(size == FushiSizePreset::Normal);
        self.size_large.set_checked(size == FushiSizePreset::Large);
        self.size_huge.set_checked(size == FushiSizePreset::Huge);
        self.interact_with_windows
            .set_checked(settings.interact_with_windows);
        self.start_on_login.set_checked(start_on_login_enabled);
    }
}

#[cfg(target_os = "macos")]
struct MacStatusMenu {
    _tray_icon: TrayIcon,
    size_small: CheckMenuItem,
    size_normal: CheckMenuItem,
    size_large: CheckMenuItem,
    size_huge: CheckMenuItem,
    interact_with_windows: CheckMenuItem,
}

#[cfg(target_os = "macos")]
impl MacStatusMenu {
    fn new(settings: &AppSettings) -> Result<Self, String> {
        let menu = Menu::new();
        let settings_menu = Submenu::new("Settings", true);
        let credits_menu = Submenu::new("Credits", true);
        let size_menu = Submenu::new("Fushi size", true);

        let size_small = CheckMenuItem::with_id(SIZE_SMALL_MENU_ID, "Small", true, false, None);
        let size_normal = CheckMenuItem::with_id(SIZE_NORMAL_MENU_ID, "Normal", true, false, None);
        let size_large = CheckMenuItem::with_id(SIZE_LARGE_MENU_ID, "Large", true, false, None);
        let size_huge = CheckMenuItem::with_id(SIZE_HUGE_MENU_ID, "Huge", true, false, None);
        size_menu
            .append_items(&[&size_small, &size_normal, &size_large, &size_huge])
            .map_err(|err| format!("failed to create macOS status menu: {err}"))?;

        let interact_with_windows = CheckMenuItem::with_id(
            WINDOW_INTERACTION_MENU_ID,
            "Walk on windows",
            true,
            settings.interact_with_windows,
            None,
        );
        settings_menu
            .append_items(&[&size_menu, &interact_with_windows])
            .map_err(|err| format!("failed to create macOS status menu: {err}"))?;

        let credits_version = MenuItem::with_id(WEBSITE_MENU_ID, APP_VERSION_TEXT, true, None);
        let developer_credit = MenuItem::new(DEVELOPER_CREDIT_TEXT, false, None);
        let character_copyright = MenuItem::new(CHARACTER_COPYRIGHT_TEXT, false, None);
        credits_menu
            .append_items(&[&credits_version, &developer_credit, &character_copyright])
            .map_err(|err| format!("failed to create macOS status menu: {err}"))?;

        let separator = PredefinedMenuItem::separator();
        let quit = MenuItem::with_id(QUIT_MENU_ID, "Quit", true, None);
        menu.append_items(&[&settings_menu, &credits_menu, &separator, &quit])
            .map_err(|err| format!("failed to create macOS status menu: {err}"))?;

        let icon = crate::macos::load_status_icon(include_bytes!("../assets/desktop-fushi.png"))?;
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(APP_DISPLAY_NAME)
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(true)
            .with_menu_on_right_click(true)
            .build()
            .map_err(|err| format!("failed to create macOS status icon: {err}"))?;

        let status_menu = Self {
            _tray_icon: tray_icon,
            size_small,
            size_normal,
            size_large,
            size_huge,
            interact_with_windows,
        };
        status_menu.set_state(settings);
        Ok(status_menu)
    }

    fn set_state(&self, settings: &AppSettings) {
        let size = FushiSizePreset::from_scale(settings.fushi_scale);
        self.size_small.set_checked(size == FushiSizePreset::Small);
        self.size_normal.set_checked(size == FushiSizePreset::Normal);
        self.size_large.set_checked(size == FushiSizePreset::Large);
        self.size_huge.set_checked(size == FushiSizePreset::Huge);
        self.interact_with_windows
            .set_checked(settings.interact_with_windows);
    }
}

fn window_rect_for_body(fushi: &FushiBody) -> (Vec2, PhysicalSize<u32>) {
    let bounds = fushi.render_bounds();
    let origin = Vec2::new(bounds.left.floor(), bounds.top.floor());
    let width = bounds.width().ceil().max(MIN_WINDOW_SIZE as f32) as u32;
    let height = bounds.height().ceil().max(MIN_WINDOW_SIZE as f32) as u32;
    (origin, PhysicalSize::new(width, height))
}

fn window_scale(window: &Window) -> f32 {
    #[cfg(target_os = "macos")]
    {
        return (window.scale_factor() as f32).max(1.0);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        1.0
    }
}

fn physical_position_for_window(window: &Window, origin: Vec2) -> PhysicalPosition<i32> {
    let scale = window_scale(window);
    PhysicalPosition::new(
        (origin.x * scale).round() as i32,
        (origin.y * scale).round() as i32,
    )
}

fn physical_size_for_window(window: &Window, size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    let scale = window_scale(window);
    PhysicalSize::new(
        ((size.width as f32) * scale).round().max(1.0) as u32,
        ((size.height as f32) * scale).round().max(1.0) as u32,
    )
}

fn logical_size_from_physical(window: &Window, size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    let scale = window_scale(window);
    PhysicalSize::new(
        ((size.width as f32) / scale).round().max(1.0) as u32,
        ((size.height as f32) / scale).round().max(1.0) as u32,
    )
}

fn set_window_click_through(window: &Window, enabled: bool) {
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = window.set_ignore_cursor_events(enabled);
    #[cfg(target_os = "macos")]
    crate::macos::set_click_through(window, enabled);
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = crate::win32::hwnd_from_window(window) {
        unsafe { crate::win32::set_click_through(hwnd, enabled) };
    }
}

#[cfg(target_os = "windows")]
fn capture_environment(
    window: Option<&Window>,
    previous: Option<&DesktopEnvironment>,
    dt: f32,
    interact_with_windows: bool,
) -> DesktopEnvironment {
    let mut env = DesktopEnvironment::capture();
    if interact_with_windows {
        let excluded: Vec<_> = window
            .and_then(crate::win32::hwnd_from_window)
            .into_iter()
            .collect();
        let windows = unsafe { crate::win32::visible_window_rects_excluding(&excluded) };
        env = env.with_window_rects(windows);
    }
    if let Some(previous) = previous {
        env.apply_window_motion_from(previous, dt);
    }
    env
}

#[cfg(target_os = "macos")]
fn capture_environment(
    _window: Option<&Window>,
    previous: Option<&DesktopEnvironment>,
    dt: f32,
    interact_with_windows: bool,
) -> DesktopEnvironment {
    let mut env = DesktopEnvironment::capture();
    if interact_with_windows {
        env = env.with_window_rects(crate::macos::visible_window_rects());
    }
    if let Some(previous) = previous {
        env.apply_window_motion_from(previous, dt);
    }
    env
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn capture_environment(
    _window: Option<&Window>,
    previous: Option<&DesktopEnvironment>,
    dt: f32,
    _interact_with_windows: bool,
) -> DesktopEnvironment {
    let mut env = DesktopEnvironment::capture();
    if let Some(previous) = previous {
        env.apply_window_motion_from(previous, dt);
    }
    env
}
