use crate::desktop::{DesktopEnvironment, SurfaceContact, SurfaceKind};
use crate::fushi::constants::*;
use crate::fushi::soft_body::{BodyKinematics, ContactProjection, SoftBody};
use crate::math::{approach, clampf, exp_decay, lerp, smoothstep, vlerp, wrap_angle, RectF, Vec2};

const HOVER_PAUSE_DURATION: f32 = 0.68;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionMode {
    Attached,
    Flying,
    Dragged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FushiExpression {
    Default,
    Sleepy,
    Surprised,
    Angry,
    Grumpy,
    Panic,
    Dizzy,
    Sad,
    Stare,
}

#[derive(Clone, Copy, Debug)]
struct TurnState {
    target_sign: i32,
    t: f32,
    swapped: bool,
}

#[derive(Clone, Copy, Debug)]
struct TumblerState {
    shaking: bool,
}

#[derive(Clone, Copy, Debug)]
struct TinyRng {
    state: u64,
}

impl TinyRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }
}

fn is_rear_svg_ear(ear: EarSpec) -> bool {
    // The renderer now uses traced SVG ears instead of the old upright rabbit-ear primitive.
    // Hit testing follows broad ellipses around those traced, side-splayed curled silhouettes.
    ear.anchor.x < -90.0
}

fn ear_tip(ear: EarSpec) -> Vec2 {
    if is_rear_svg_ear(ear) {
        Vec2::new(-121.0, -78.0)
    } else {
        Vec2::new(-46.0, -75.0)
    }
}

fn ear_leaf_distance(local: Vec2, ear: EarSpec) -> f32 {
    let (center, rx, ry, angle) = if is_rear_svg_ear(ear) {
        (Vec2::new(-124.5, -51.5), 22.0_f32, 33.0_f32, -0.10_f32)
    } else {
        (Vec2::new(-54.5, -49.5), 29.0_f32, 36.0_f32, -0.22_f32)
    };

    let q = (local - center).rotate(-angle);
    let k = (q.x / rx).powi(2) + (q.y / ry).powi(2);
    if k <= 1.0 {
        0.0
    } else {
        (k.sqrt() - 1.0) * rx.min(ry)
    }
}

fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let denom = ab.length_sq();
    if denom <= 0.001 {
        return (p - a).length();
    }

    let t = ((p - a).dot(ab) / denom).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

fn crawl_reach_collapse_per_phase(phase: f32) -> f32 {
    let reach_base = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let reach_base_velocity = phase.cos() * 0.5;
    let reach_velocity =
        CRAWL_REACH_EXPONENT * reach_base.powf(CRAWL_REACH_EXPONENT - 1.0) * reach_base_velocity;
    (-reach_velocity).max(0.0)
}

fn approach_angle(current: f32, target: f32, max_delta: f32) -> f32 {
    wrap_angle(current + wrap_angle(target - current).clamp(-max_delta, max_delta))
}

fn finite_vec_or(value: Vec2, fallback: Vec2) -> Vec2 {
    if value.x.is_finite() && value.y.is_finite() {
        value
    } else {
        fallback
    }
}

fn finite_scalar_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn ordered_container_limits(min: f32, max: f32) -> (f32, f32) {
    if min <= max {
        (min, max)
    } else {
        let midpoint = (min + max) * 0.5;
        (midpoint, midpoint)
    }
}

fn projected_sensor_gravity(gravity_world: Vec2) -> (Vec2, Option<Vec2>) {
    let gravity_world = finite_vec_or(gravity_world, Vec2::ZERO);
    let magnitude = gravity_world.length();
    if !magnitude.is_finite() || magnitude <= TUMBLER_GRAVITY_DEAD_ZONE {
        return (Vec2::ZERO, None);
    }

    let direction = gravity_world * (1.0 / magnitude);
    let projected_strength = ((magnitude - TUMBLER_GRAVITY_DEAD_ZONE)
        / (TUMBLER_STANDARD_GRAVITY - TUMBLER_GRAVITY_DEAD_ZONE))
        .clamp(0.0, 1.0);
    (direction * (FREE_GRAVITY * projected_strength), Some(direction))
}

fn content_bound_anchors() -> [Vec2; 9] {
    [
        Vec2::new(-130.0, -166.0),
        Vec2::new(-78.0, -172.0),
        Vec2::new(-6.0, -166.0),
        TAIL_ANCHOR + Vec2::new(108.0, -78.0),
        TAIL_ANCHOR + Vec2::new(-66.0, -70.0),
        TAIL_ANCHOR + Vec2::new(100.0, 58.0),
        TAIL_ANCHOR + Vec2::new(-56.0, 48.0),
        Vec2::new(-155.0, 80.0),
        Vec2::new(155.0, 80.0),
    ]
}

#[derive(Clone)]
pub struct FushiBody {
    pub mesh: SoftBody,
    pub center: Vec2,
    pub velocity: Vec2,
    pub mode: MotionMode,
    pub surface: Option<SurfaceContact>,
    pub move_sign: i32,
    pub facing_sign: i32,
    pub yaw_scale: f32,
    pub view_yaw: f32,
    pub view_pitch: f32,
    pub view_yaw_velocity: f32,
    pub view_pitch_velocity: f32,
    pub normal: Vec2,
    pub tangent: Vec2,
    pub bank: f32,
    pub bank_velocity: f32,
    pub cursor_world: Vec2,
    pub drag_world: Vec2,
    pub mouse_velocity: Vec2,
    pub grab_local: Vec2,
    pub grab_node: Option<usize>,
    pub time: f32,
    pub anger: f32,
    pub stress: f32,
    pub impact_squash: f32,
    pub edge_squash: f32,
    pub hover_amount: f32,
    pub petting_amount: f32,
    pub appendage_pet_amount: f32,
    pub body_pet_amount: f32,
    pub happiness: f32,
    pub passive_mouse_velocity: Vec2,
    pub appendage_sway: Vec2,
    pub appendage_sway_velocity: Vec2,
    pub throw_anger_timer: f32,
    pub dizzy_reaction_timer: f32,
    pub sleepiness: f32,
    pub blink_amount: f32,
    pub dizziness: f32,
    pub sadness: f32,
    pub scale: f32,
    pub previous_expression: FushiExpression,
    pub expression_transition: f32,
    pub expression: FushiExpression,
    last_mouse: Vec2,
    last_mouse_velocity: Vec2,
    last_cursor_world: Vec2,
    last_cursor_valid: bool,
    drag_follow_world: Vec2,
    drag_lift: f32,
    crawl_drive: f32,
    crawl_phase: f32,
    idle_timer: f32,
    hover_pause_timer: f32,
    hover_pause_hovering: bool,
    direction_timer: f32,
    look_timer: f32,
    look_target_yaw: f32,
    look_target_pitch: f32,
    surprise_timer: f32,
    panic_timer: f32,
    blink_timer: f32,
    surface_transition: f32,
    surface_transition_from_normal: Vec2,
    surface_transition_from_tangent: Vec2,
    platform_lost_timer: f32,
    sensor_gravity_world: Option<Vec2>,
    tumbler: Option<TumblerState>,
    turn: Option<TurnState>,
    rng: TinyRng,
}

impl FushiBody {
    pub fn new(env: &DesktopEnvironment) -> Self {
        let contact = env.initial_contact();
        let normal = DesktopEnvironment::surface_normal(contact.kind);
        let tangent = DesktopEnvironment::surface_tangent(contact.kind);
        let mut this = Self {
            mesh: SoftBody::new(),
            center: env.initial_center(),
            velocity: Vec2::ZERO,
            mode: MotionMode::Attached,
            surface: Some(contact),
            move_sign: 1,
            facing_sign: 1,
            yaw_scale: 1.0,
            view_yaw: 0.0,
            view_pitch: 0.0,
            view_yaw_velocity: 0.0,
            view_pitch_velocity: 0.0,
            normal,
            tangent,
            bank: 0.0,
            bank_velocity: 0.0,
            cursor_world: Vec2::ZERO,
            drag_world: Vec2::ZERO,
            mouse_velocity: Vec2::ZERO,
            grab_local: Vec2::ZERO,
            grab_node: None,
            time: 0.0,
            anger: 0.0,
            stress: 0.0,
            impact_squash: 0.0,
            edge_squash: 0.0,
            hover_amount: 0.0,
            petting_amount: 0.0,
            appendage_pet_amount: 0.0,
            body_pet_amount: 0.0,
            happiness: 0.0,
            passive_mouse_velocity: Vec2::ZERO,
            appendage_sway: Vec2::ZERO,
            appendage_sway_velocity: Vec2::ZERO,
            throw_anger_timer: 0.0,
            dizzy_reaction_timer: 0.0,
            sleepiness: 0.0,
            blink_amount: 0.0,
            dizziness: 0.0,
            sadness: 0.0,
            scale: 1.0,
            previous_expression: FushiExpression::Default,
            expression_transition: 1.0,
            expression: FushiExpression::Default,
            last_mouse: Vec2::ZERO,
            last_mouse_velocity: Vec2::ZERO,
            last_cursor_world: Vec2::ZERO,
            last_cursor_valid: false,
            drag_follow_world: Vec2::ZERO,
            drag_lift: 0.0,
            crawl_drive: 1.0,
            crawl_phase: CRAWL_REST_PHASE,
            idle_timer: 0.0,
            hover_pause_timer: 0.0,
            hover_pause_hovering: false,
            direction_timer: 2.5,
            look_timer: 1.1,
            look_target_yaw: 0.0,
            look_target_pitch: 0.0,
            surprise_timer: 0.0,
            panic_timer: 0.0,
            blink_timer: 1.4,
            surface_transition: 0.0,
            surface_transition_from_normal: normal,
            surface_transition_from_tangent: tangent,
            platform_lost_timer: 0.0,
            sensor_gravity_world: None,
            tumbler: None,
            turn: None,
            rng: TinyRng::new(0x4655_5348_4932_4432),
        };
        let kin = this.kinematics();
        this.mesh.reset(&kin);
        this
    }

    pub fn set_cursor(&mut self, cursor: Vec2) {
        self.cursor_world = cursor;
    }

    pub fn reset_to_safe_surface(&mut self, env: &DesktopEnvironment) {
        let (contact, center) =
            env.nearest_surface_with(self.center, self.body_half_length(), self.body_center_to_belly());
        self.surface = Some(contact);
        self.mode = MotionMode::Attached;
        self.center = center;
        self.velocity *= 0.1;
        self.bank = 0.0;
        self.bank_velocity = 0.0;
        self.surface_transition = 0.0;
        self.platform_lost_timer = 0.0;
        self.tumbler = None;
        self.edge_squash = 0.0;
        self.drag_lift = 0.0;
        self.crawl_drive = 1.0;
        self.petting_amount = 0.0;
        self.body_pet_amount = 0.0;
        self.appendage_pet_amount = 0.0;
        self.happiness = 0.0;
        self.passive_mouse_velocity = Vec2::ZERO;
        self.appendage_sway = Vec2::ZERO;
        self.appendage_sway_velocity = Vec2::ZERO;
        self.hover_pause_timer = 0.0;
        self.hover_pause_hovering = false;
        self.throw_anger_timer = 0.0;
        self.dizzy_reaction_timer = 0.0;
        self.last_cursor_valid = false;
        self.crawl_phase = CRAWL_REST_PHASE;
        self.turn = None;
        self.yaw_scale = 1.0;
        self.view_yaw = 0.0;
        self.view_pitch = 0.0;
        self.view_yaw_velocity = 0.0;
        self.view_pitch_velocity = 0.0;
        self.look_target_yaw = 0.0;
        self.look_target_pitch = 0.0;
        self.look_timer = 0.8 + self.rng.f32() * 1.8;
        if self.surface_transition <= 0.0 {
            self.update_surface_frame(env);
        }
        let kin = self.kinematics();
        self.mesh.reset(&kin);
    }

    pub fn set_scale(&mut self, scale: f32, env: &DesktopEnvironment) {
        self.set_scale_with_limits(scale, 0.48, 2.80, env);
    }

    pub fn set_scale_with_limits(
        &mut self,
        scale: f32,
        min_scale: f32,
        max_scale: f32,
        env: &DesktopEnvironment,
    ) {
        let min_scale = min_scale.clamp(0.20, 2.80);
        let max_scale = max_scale.max(min_scale).min(2.80);
        let scale = clampf(scale, min_scale, max_scale);
        if (self.scale - scale).abs() <= 0.001 {
            return;
        }
        self.scale = scale;
        if let Some(surface) = self.surface {
            let (lo, hi) = env.tangent_extent(surface);
            let coord = DesktopEnvironment::tangent_coord(surface.kind, self.center);
            let half_len = self.body_half_length();
            let clamped = clampf(coord, lo + half_len * 0.6, hi - half_len * 0.6);
            self.center = env.point_from_tangent(surface, clamped, self.body_center_to_belly());
        }
        self.grab_local = self.clamp_grab_local(self.grab_local);
        let kin = self.kinematics();
        self.mesh.reset(&kin);
    }

    pub fn snap_to_contact(&mut self, contact: SurfaceContact, tangent_coord: f32, env: &DesktopEnvironment) {
        let (lo, hi) = env.tangent_extent(contact);
        let half_len = self.body_half_length();
        let min = lo + half_len * 0.36;
        let max = hi - half_len * 0.36;
        let coord = if min <= max {
            clampf(tangent_coord, min, max)
        } else {
            (lo + hi) * 0.5
        };

        self.surface = Some(contact);
        self.mode = MotionMode::Attached;
        self.center = env.point_from_tangent(contact, coord, self.body_center_to_belly());
        self.velocity = Vec2::ZERO;
        self.bank = 0.0;
        self.bank_velocity = 0.0;
        self.surface_transition = 0.0;
        self.platform_lost_timer = 0.0;
        self.tumbler = None;
        self.edge_squash = env.screen_edge_pinch_amount(contact, self.body_center_to_belly());
        self.drag_lift = 0.0;
        self.grab_node = None;
        self.petting_amount = 0.0;
        self.body_pet_amount = 0.0;
        self.appendage_pet_amount = 0.0;
        self.happiness = 0.0;
        self.passive_mouse_velocity = Vec2::ZERO;
        self.appendage_sway = Vec2::ZERO;
        self.appendage_sway_velocity = Vec2::ZERO;
        self.hover_pause_timer = 0.0;
        self.hover_pause_hovering = false;
        self.throw_anger_timer = 0.0;
        self.dizzy_reaction_timer = 0.0;
        self.last_cursor_valid = false;
        self.crawl_drive = 1.0;
        self.crawl_phase = CRAWL_REST_PHASE;
        self.turn = None;
        self.yaw_scale = 1.0;
        self.view_yaw = 0.0;
        self.view_pitch = 0.0;
        self.view_yaw_velocity = 0.0;
        self.view_pitch_velocity = 0.0;
        self.look_target_yaw = 0.0;
        self.look_target_pitch = 0.0;
        self.look_timer = 0.8 + self.rng.f32() * 1.8;
        self.update_surface_frame(env);
        let kin = self.kinematics();
        self.mesh.reset(&kin);
    }

    pub fn try_begin_drag(&mut self, world: Vec2) -> bool {
        self.try_begin_drag_with_margin(world, INTERACTION_HIT_MARGIN)
    }

    pub fn try_begin_drag_with_margin(&mut self, world: Vec2, extra: f32) -> bool {
        if !self.hit_test(world, extra) {
            return false;
        }
        self.begin_drag_at(world)
    }

    pub fn begin_drag_unchecked(&mut self, world: Vec2) -> bool {
        self.begin_drag_at(world)
    }

    fn begin_drag_at(&mut self, world: Vec2) -> bool {
        self.tumbler = None;
        let grab_local = self.clamp_drag_grab_local(self.world_to_local(world));
        let grab_node = self.mesh.closest_node(world);
        self.mode = MotionMode::Dragged;
        self.drag_world = world;
        self.drag_follow_world = world;
        self.last_mouse = world;
        self.last_mouse_velocity = Vec2::ZERO;
        self.mouse_velocity = Vec2::ZERO;
        self.grab_local = grab_local;
        self.grab_node = grab_node;
        self.edge_squash = 0.0;
        self.drag_lift = 0.0;
        self.petting_amount = approach(self.petting_amount, 0.0, 0.34);
        self.body_pet_amount = approach(self.body_pet_amount, 0.0, 0.34);
        self.appendage_pet_amount = approach(self.appendage_pet_amount, 0.0, 0.34);
        let detach_normal = self.normal.normalized_or(Vec2::Y);
        let kin = self.kinematics();
        let lower_grab = smoothstep(BODY_HALF_HEIGHT * 0.18, BODY_HALF_HEIGHT * 1.05, grab_local.y);
        let detach_amount = DRAG_DETACH_LIFT * (0.26 + (1.0 - lower_grab) * 0.18);
        self.mesh
            .detach_from_surface(&kin, detach_normal, detach_amount * self.scale.max(0.2));
        self.stress = clampf(self.stress + 0.08, 0.0, 1.0);
        self.anger = clampf(self.anger + 0.05, 0.0, 1.0);
        self.sleepiness = 0.0;
        if self.hover_amount < 0.42 {
            self.surprise_timer = self.surprise_timer.max(0.24);
        }
        true
    }

    pub fn drag_to(&mut self, world: Vec2) {
        self.drag_world = world;
    }

    pub fn release_drag(&mut self) {
        if self.mode != MotionMode::Dragged {
            return;
        }
        self.velocity = (self.mouse_velocity * THROW_VELOCITY_SCALE).clamp_len(MAX_THROW_VELOCITY);
        let grab_vec = self.local_vec_to_world(self.grab_local);
        self.bank_velocity += grab_vec.cross(self.velocity) * 0.000012;
        self.bank_velocity = clampf(self.bank_velocity, -4.3, 4.3);
        self.mode = MotionMode::Flying;
        self.surface = None;
        self.grab_node = None;
        self.drag_follow_world = self.drag_world;
        self.edge_squash = 0.0;
        self.drag_lift = 0.0;
        let throw_speed = self.velocity.length();
        if throw_speed > 680.0 {
            self.panic_timer = self.panic_timer.max(clampf(throw_speed / 1050.0, 0.42, 1.35));
        } else if throw_speed > 180.0 {
            self.surprise_timer = self.surprise_timer.max(clampf(throw_speed / 1350.0, 0.14, 0.36));
        }
        let throw_annoyance = smoothstep(240.0, 980.0, throw_speed);
        self.anger = clampf(
            self.anger + clampf(throw_speed / 3500.0, 0.0, 0.28) + throw_annoyance * 0.44,
            0.0,
            1.0,
        );
        if throw_annoyance > 0.02 {
            self.throw_anger_timer = self.throw_anger_timer.max(0.36 + throw_annoyance * 1.10);
        }
        self.stress = clampf(self.stress + clampf(throw_speed / 3000.0, 0.0, 0.38), 0.0, 1.0);
        let throw_dizzy = clampf(self.bank_velocity.abs() / 9.2 + throw_speed / 5000.0, 0.0, 1.0);
        self.dizziness = self.dizziness.max(throw_dizzy);
        if throw_dizzy > 0.28 {
            self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.38 + throw_dizzy * 0.58);
        }
        self.sadness = self
            .sadness
            .max(clampf(self.stress * 0.42 + throw_speed / 4200.0, 0.0, 0.78));
    }

    pub fn apply_external_shake(&mut self, acceleration_local: Vec2, intensity: f32, dt: f32) {
        // Existing local-space impulse hook used by the web front-end. Android's
        // conservative, gated sensor behavior has a separate screen-space path.
        let dt = dt.clamp(0.001, 0.060);
        let intensity = intensity.clamp(0.0, 1.0);
        if intensity <= 0.001 {
            return;
        }

        let kick = acceleration_local.clamp_len(980.0) * (0.22 + intensity * 0.52);
        self.velocity += self.local_vec_to_world(kick) * dt;
        self.bank_velocity += acceleration_local.x.clamp(-1300.0, 1300.0) * 0.0018 * intensity;
        self.impact_squash = self.impact_squash.max(0.08 + intensity * 0.22);
        self.stress = clampf(self.stress + intensity * dt * 0.95, 0.0, 1.0);
        self.dizziness = clampf(
            self.dizziness + intensity * dt * 2.55 + intensity * 0.045,
            0.0,
            1.0,
        );
        self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.42 + intensity * 1.10);
        if intensity > 0.68 {
            self.panic_timer = self.panic_timer.max(0.10 + intensity * 0.20);
        }
    }

    fn motion_gravity(&self) -> (Vec2, Option<Vec2>) {
        self.sensor_gravity_world
            .map(projected_sensor_gravity)
            .unwrap_or((Vec2::Y * FREE_GRAVITY, Some(Vec2::Y)))
    }

    /// Applies a phone/container motion frame in screen coordinates.
    ///
    /// `impulse_world` is the detector-integrated apparent velocity change (m/s); gravity is the
    /// projected content acceleration (m/s²). Positive Y points down the screen. Valid gravity
    /// updates continuously; only shake impulses and tumbler entry require the detector gate.
    pub(crate) fn apply_container_motion(
        &mut self,
        impulse_world: Vec2,
        gravity_world: Vec2,
        intensity: f32,
        sensor_available: bool,
        gravity_valid: bool,
        triggered: bool,
        gate_open: bool,
    ) {
        if !sensor_available {
            self.sensor_gravity_world = None;
            self.tumbler = None;
            return;
        }
        if gravity_valid {
            self.sensor_gravity_world = Some(finite_vec_or(gravity_world, Vec2::ZERO).clamp_len(14.0));
        } else {
            self.sensor_gravity_world = None;
        }

        if self.mode == MotionMode::Dragged {
            self.tumbler = None;
            return;
        }

        let impulse_world = finite_vec_or(impulse_world, Vec2::ZERO).clamp_len(TUMBLER_MAX_SENSOR_IMPULSE);
        let intensity = if intensity.is_finite() {
            intensity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if triggered && self.sensor_gravity_world.is_some() {
            self.begin_tumbler(impulse_world, intensity, gate_open);
        }

        let Some(tumbler) = self.tumbler.as_mut() else {
            return;
        };

        tumbler.shaking = gate_open;
        if gate_open {
            let inertial_impulse =
                impulse_world * (TUMBLER_SENSOR_IMPULSE_SCALE * lerp(0.35, 1.0, intensity));
            self.velocity += inertial_impulse;
            self.velocity = finite_vec_or(self.velocity, Vec2::ZERO).clamp_len(MAX_THROW_VELOCITY);
            self.bank_velocity += impulse_world.x * 0.12 * intensity;
            self.bank_velocity = clampf(self.bank_velocity, -10.0, 10.0);
        }
    }

    fn begin_tumbler(&mut self, impulse_world: Vec2, intensity: f32, gate_open: bool) {
        let already_active = self.tumbler.is_some();
        let outward = self.normal.normalized_or(Vec2::Y);
        let inward = -outward;
        let inertial_direction = impulse_world.normalized_or(inward);
        let kick_speed =
            (impulse_world.length() * TUMBLER_INITIAL_IMPULSE_SCALE * lerp(0.78, 1.15, intensity))
                .clamp(TUMBLER_INITIAL_KICK_MIN, TUMBLER_INITIAL_KICK_MAX);
        let inward_speed = lerp(220.0, 430.0, intensity);
        let mut kick = inertial_direction * kick_speed + inward * inward_speed;
        let minimum_inward = lerp(190.0, 370.0, intensity);
        let inward_component = kick.dot(inward);
        if inward_component < minimum_inward {
            kick += inward * (minimum_inward - inward_component);
        }

        if !already_active {
            let kin = self.kinematics();
            self.mesh
                .detach_from_surface(&kin, outward, (DRAG_DETACH_LIFT * 1.35) * self.scale.max(0.2));
            self.velocity *= 0.22;
        }
        self.velocity += kick * if already_active { 0.58 } else { 1.0 };
        self.velocity = finite_vec_or(self.velocity, inward * minimum_inward).clamp_len(MAX_THROW_VELOCITY);
        self.mode = MotionMode::Flying;
        self.surface = None;
        self.platform_lost_timer = 0.0;
        self.surface_transition = 0.0;
        self.edge_squash = 0.0;
        self.turn = None;

        let spin_direction = outward.cross(inertial_direction) + inertial_direction.x * 0.45;
        self.bank_velocity = clampf(
            self.bank_velocity + spin_direction * lerp(3.2, 7.0, intensity),
            -10.0,
            10.0,
        );
        self.impact_squash = self.impact_squash.max(lerp(0.20, 0.42, intensity));
        self.stress = self.stress.max(lerp(0.55, 0.92, intensity));
        self.dizziness = self.dizziness.max(lerp(0.62, 1.0, intensity));
        self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(lerp(0.82, 1.52, intensity));
        self.panic_timer = self.panic_timer.max(lerp(0.32, 0.78, intensity));
        self.sleepiness = 0.0;

        self.tumbler = Some(TumblerState { shaking: gate_open });
    }

    pub fn step(&mut self, dt: f32, env: &DesktopEnvironment) {
        let dt = clampf(dt, 0.001, 0.035);
        self.time += dt;
        self.update_hover(dt);
        self.update_blink(dt);

        match self.mode {
            MotionMode::Attached => self.step_attached(dt, env),
            MotionMode::Dragged => self.step_dragged(dt),
            MotionMode::Flying => self.step_flying(dt, env),
        }
        self.advance_surface_transition(dt);
        self.update_shake_dizziness(dt);

        self.anger = (self.anger
            - dt * if self.mode == MotionMode::Dragged {
                0.01
            } else {
                0.036
            })
        .max(0.0);
        self.stress = (self.stress - dt * 0.43).max(0.0);
        self.dizziness = (self.dizziness
            - dt * if self.mode == MotionMode::Dragged {
                0.18
            } else {
                0.62
            })
        .max(0.0);
        self.sadness = (self.sadness - dt * 0.070).max(0.0);
        self.impact_squash = (self.impact_squash - dt * 1.9).max(0.0);
        self.surprise_timer = (self.surprise_timer - dt).max(0.0);
        self.panic_timer = (self.panic_timer - dt).max(0.0);
        self.dizzy_reaction_timer = (self.dizzy_reaction_timer - dt).max(0.0);
        self.throw_anger_timer = (self.throw_anger_timer - dt).max(0.0);
        self.update_expression(dt);
        self.update_view_pose(dt);

        let kin = self.kinematics();
        let contact = self.surface.map(|surface| ContactProjection {
            contact: surface,
            normal: DesktopEnvironment::surface_normal(surface.kind),
            plane: env.surface_line(surface),
            scale: self.scale,
        });
        let grab = if self.mode == MotionMode::Dragged {
            self.grab_node.map(|i| (i, self.drag_world))
        } else {
            None
        };
        self.mesh.step(dt, &kin, grab, contact);
        if self.tumbler.is_some() {
            self.constrain_tumbler_body(env);
        }
    }

    pub fn hit_test(&self, world: Vec2, extra: f32) -> bool {
        if self.body_hit_test(world, extra) {
            return true;
        }

        let local = self.world_to_local(world);
        let extra_local = extra / self.scale.max(0.2);

        for ear in EARS {
            if self.ear_contains(local, extra_local, ear) {
                return true;
            }
        }

        if self.tail_contains(local, extra_local) {
            return true;
        }
        false
    }

    pub fn interactive_hit_test(&self, world: Vec2) -> bool {
        self.hit_test(world, INTERACTION_HIT_MARGIN)
    }

    pub fn local_to_world(&self, local: Vec2) -> Vec2 {
        self.kinematics().local_to_world(local)
    }

    pub fn local_vec_to_world(&self, local: Vec2) -> Vec2 {
        let kin = self.kinematics();
        kin.local_vec_to_world(local)
    }

    pub fn world_to_local(&self, world: Vec2) -> Vec2 {
        let x_axis = self.raw_x_axis();
        let y_axis = self.raw_y_axis();
        let d = world - self.center;
        Self::world_vec_to_local_axes(d, x_axis, y_axis)
    }

    pub fn kinematics(&self) -> BodyKinematics {
        let x_axis = self.raw_x_axis();
        let y_axis = self.raw_y_axis();
        let raw_grab_world = self.center + x_axis * self.grab_local.x + y_axis * self.grab_local.y;
        let scale = self.scale.max(0.2);
        let drag_pull_local = if self.mode == MotionMode::Dragged {
            Self::world_vec_to_local_axes(self.drag_follow_world - raw_grab_world, x_axis, y_axis)
                .clamp_len(DRAG_MAX_ANCHOR_ERROR)
        } else {
            Vec2::ZERO
        };
        let drag_handle_radius = if self.mode == MotionMode::Dragged {
            (self.drag_follow_world - self.center).length() / scale
        } else {
            BODY_HALF_LENGTH
        };
        let (corner_bend, corner_bend_sign) =
            if self.mode == MotionMode::Attached && self.surface_transition > 0.0 {
                let phase = (1.0 - self.surface_transition / SURFACE_TRANSITION_DURATION).clamp(0.0, 1.0);
                let bend = (std::f32::consts::PI * phase).sin().max(0.0);
                let target_tangent = self
                    .surface
                    .map(|contact| DesktopEnvironment::surface_tangent(contact.kind))
                    .unwrap_or_else(|| self.tangent.normalized_or(Vec2::X));
                let cross = self
                    .surface_transition_from_tangent
                    .normalized_or(Vec2::X)
                    .cross(target_tangent.normalized_or(Vec2::X));
                let sign = if cross.abs() > 0.001 {
                    cross.signum()
                } else if self.bank_velocity.abs() > 0.001 {
                    self.bank_velocity.signum()
                } else {
                    1.0
                };
                (bend, sign)
            } else {
                (0.0, 1.0)
            };

        BodyKinematics {
            center: self.center,
            x_axis,
            y_axis,
            velocity: self.velocity,
            bank_velocity: self.bank_velocity,
            time: self.time,
            crawl_phase: self.crawl_phase,
            scale: self.scale,
            attached: self.mode == MotionMode::Attached,
            dragged: self.mode == MotionMode::Dragged,
            stress: self.stress,
            impact_squash: self.impact_squash,
            edge_squash: self.edge_squash,
            grab_local: self.grab_local,
            mouse_velocity: self.mouse_velocity,
            drag_pull_local,
            drag_handle_radius,
            drag_lift: self.drag_lift,
            crawl_drive: self.crawl_drive,
            corner_bend,
            corner_bend_sign,
        }
    }

    /// Returns the bounds occupied by the body and its appendages without any
    /// platform/window-management safety margin.
    pub fn content_bounds(&self) -> RectF {
        let mut min = self.center;
        let mut max = self.center;
        if let Some((a, b)) = self.mesh.bounds() {
            min = min.min(a);
            max = max.max(b);
        }
        for a in content_bound_anchors() {
            let p = self.local_to_world(a);
            min = min.min(p);
            max = max.max(p);
        }
        RectF::new(min.x, min.y, max.x, max.y)
    }

    /// Returns the farthest conservative body/appendage point from the rigid center.
    /// Unlike the diagonal of `content_bounds`, this does not invent an empty corner by
    /// combining the x extent of one appendage with the y extent of another.
    pub fn content_radius(&self) -> f32 {
        let mut radius_sq: f32 = 0.0;
        for node in &self.mesh.nodes {
            radius_sq = radius_sq.max((node.pos - self.center).length_sq());
        }
        for anchor in content_bound_anchors() {
            radius_sq = radius_sq.max((self.local_to_world(anchor) - self.center).length_sq());
        }
        radius_sq.sqrt()
    }

    pub fn render_bounds(&self) -> RectF {
        self.content_bounds().inflate(RENDER_WINDOW_MARGIN)
    }

    pub fn translate_world(&mut self, delta: Vec2) {
        if delta.length_sq() <= 0.0001 {
            return;
        }
        self.center += delta;
        self.translate_mesh(delta);
        self.drag_world += delta;
        self.drag_follow_world += delta;
        self.last_mouse += delta;
        self.last_cursor_world += delta;
    }

    fn step_attached(&mut self, dt: f32, env: &DesktopEnvironment) {
        let Some(mut contact) = self.surface else {
            self.reset_to_safe_surface(env);
            return;
        };
        let half_len = self.body_half_length() * (0.72 + self.yaw_scale * 0.28);
        let center_offset = self.body_center_to_belly();
        let mut platform_velocity = if contact.is_platform() {
            env.contact_velocity(contact)
        } else {
            Vec2::ZERO
        };

        if contact.is_platform() && env.contact_window(contact).is_none() {
            self.fall_from_platform(platform_velocity, 150.0);
            return;
        }

        if contact.is_platform() && !env.platform_supports(contact, self.center, half_len) {
            if let Some((next, snapped, support_velocity)) =
                env.replacement_platform_surface(contact, self.center, half_len, center_offset)
            {
                self.surface = Some(next);
                contact = next;
                platform_velocity = support_velocity;
                self.platform_lost_timer = 0.0;
                self.center = snapped;
                self.velocity = vlerp(self.velocity, support_velocity * 0.34, 0.45);
                self.impact_squash = self.impact_squash.max(0.16);
                self.begin_surface_transition(next);
            } else {
                self.platform_lost_timer += dt;
                if self.platform_lost_timer > 0.18 {
                    self.fall_from_platform(platform_velocity, 150.0);
                    return;
                }
            }
        } else {
            self.platform_lost_timer = 0.0;
        }

        self.update_surface_frame(env);
        let target_edge_squash = if contact.is_platform() {
            env.screen_edge_pinch_amount(contact, center_offset)
        } else {
            0.0
        };
        self.edge_squash = approach(self.edge_squash, target_edge_squash, dt * 7.5);
        if target_edge_squash > 0.02 {
            self.stress = clampf(self.stress + target_edge_squash * dt * 0.12, 0.0, 1.0);
        }
        self.advance_turn(dt);

        self.direction_timer -= dt;
        if self.direction_timer <= 0.0 && self.turn.is_none() {
            self.direction_timer = 3.0 + self.rng.f32() * 5.2;
            if self.rng.f32() < 0.16 {
                self.request_turn(-self.move_sign);
            }
            if self.rng.f32() < 0.34 {
                self.idle_timer = 0.35 + self.rng.f32() * 1.3;
            }
        }

        let mut speed_scale: f32 = 1.0;
        if self.idle_timer > 0.0 {
            self.idle_timer -= dt;
            speed_scale = 0.0;
        }
        if self.hover_pause_timer > 0.0 {
            self.hover_pause_timer = (self.hover_pause_timer - dt).max(0.0);
            speed_scale = 0.0;
        }
        if self.turn.is_some() {
            speed_scale *= 0.20;
        }
        if self.surface_transition > 0.0 {
            let phase = (1.0 - self.surface_transition / SURFACE_TRANSITION_DURATION).clamp(0.0, 1.0);
            let corner_ease = phase * phase * (3.0 - 2.0 * phase);
            speed_scale *= lerp(0.52, 1.0, corner_ease);
        }
        let target_drive = speed_scale.clamp(0.0, 1.0);
        self.crawl_drive = approach(self.crawl_drive, target_drive, dt * CRAWL_DRIVE_RESPONSE);

        let base_tangent = DesktopEnvironment::surface_tangent(contact.kind);
        let phase_speed = if target_drive > 0.001 {
            CRAWL_STRETCH_FREQUENCY * (0.38 + self.crawl_drive * 0.62)
        } else {
            self.crawl_phase =
                approach_angle(self.crawl_phase, CRAWL_REST_PHASE, dt * CRAWL_PHASE_SETTLE_RATE);
            0.0
        };
        if phase_speed > 0.0 {
            self.crawl_phase = wrap_angle(self.crawl_phase + phase_speed * dt);
        }
        let stride_speed =
            CRAWL_FORWARD_STRETCH * crawl_reach_collapse_per_phase(self.crawl_phase) * phase_speed;
        let mut desired = base_tangent * (self.move_sign as f32 * speed_scale * stride_speed);
        if contact.is_platform() {
            let normal = DesktopEnvironment::surface_normal(contact.kind);
            let platform_tangent_speed = platform_velocity.dot(base_tangent);
            let platform_normal_speed = platform_velocity.dot(normal);
            desired += base_tangent * (platform_tangent_speed * 0.06);
            self.center += platform_velocity * (dt * 0.86);

            let slide = platform_tangent_speed.abs();
            let shake_speed = platform_velocity.length();
            if shake_speed > WINDOW_SHAKE_DROP_SPEED || slide > WINDOW_SHAKE_SLIDE_DROP_SPEED {
                let shake_excess = (shake_speed - WINDOW_SHAKE_DROP_SPEED).max(0.0);
                let slide_excess = (slide - WINDOW_SHAKE_SLIDE_DROP_SPEED).max(0.0);
                self.panic_timer = self.panic_timer.max(0.34);
                self.fall_from_platform(
                    platform_velocity,
                    110.0 + shake_excess * 0.07 + slide_excess * 0.05,
                );
                return;
            }

            if platform_normal_speed > 620.0 {
                self.fall_from_platform(platform_velocity, 120.0 + platform_normal_speed * 0.12);
                return;
            }
            if platform_normal_speed < -28.0 {
                let lift = (-platform_normal_speed / 760.0).clamp(0.0, 0.36);
                self.impact_squash = self.impact_squash.max(0.12 + lift);
                self.stress = clampf(self.stress + lift * 0.48, 0.0, 1.0);
                self.dizziness = self.dizziness.max(lift * 0.72);
                if lift > 0.08 {
                    self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.26 + lift * 0.95);
                }
                self.bank_velocity += platform_normal_speed * -0.00018 * self.move_sign as f32;
            }
            if slide > 70.0 {
                let wobble = clampf((slide - 70.0) / 860.0, 0.0, 0.58);
                self.dizziness = clampf(self.dizziness + wobble * dt * 4.9, 0.0, 1.0);
                if wobble > 0.10 {
                    self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.24 + wobble * 0.92);
                }
                self.stress = clampf(self.stress + wobble * dt * 1.9, 0.0, 1.0);
                self.bank_velocity += platform_tangent_speed.signum() * wobble * dt * 4.2;
            }
        }
        self.velocity = vlerp(self.velocity, desired, exp_decay(ATTACHED_VELOCITY_RESPONSE, dt));
        self.center += self.velocity * dt;

        if !contact.is_platform() {
            if let Some((window_contact, snapped, impact)) =
                env.try_find_collision_surface(self.center, self.velocity, half_len, center_offset)
            {
                if window_contact.is_platform() {
                    self.land_on_surface(window_contact, snapped, impact, env);
                    return;
                }
            }
        }

        let (constrained, low, high) =
            env.constrain_to_surface(contact, self.center, half_len, center_offset);
        self.center = constrained;

        let base = DesktopEnvironment::surface_tangent(contact.kind);
        let from_high_edge = if contact.kind == SurfaceKind::Bottom || contact.kind == SurfaceKind::Top {
            if base.x * self.move_sign as f32 > 0.0 {
                high
            } else {
                low
            }
        } else if base.y * self.move_sign as f32 > 0.0 {
            high
        } else {
            low
        };

        let should_transition = from_high_edge || (contact.is_platform() && (low || high));
        if should_transition {
            let (next, p) = env.transition_from_edge(contact, self.move_sign, high, half_len, center_offset);
            if next.is_platform() && !env.platform_supports(next, p, half_len) {
                self.fall_from_platform(platform_velocity, 150.0);
                return;
            }
            self.surface = Some(next);
            self.center = p;
            let next_tangent = DesktopEnvironment::surface_tangent(next.kind);
            let carried_speed = self.velocity.dot(base).abs().max(CRAWL_SPEED * 0.18) * self.move_sign as f32;
            self.velocity = vlerp(self.velocity, next_tangent * carried_speed, 0.38);
            self.crawl_drive *= 0.74;
            self.impact_squash = self.impact_squash.max(0.055);
            self.begin_surface_transition(next);
            self.bank_velocity += 0.52 * self.move_sign as f32;
        }

        self.bank_velocity += -self.bank * BANK_RESTORE * dt;
        self.bank_velocity *= (-BANK_DAMPING * dt).exp();
        self.bank = clampf(
            self.bank + self.bank_velocity * dt,
            -MAX_BANK_RADIANS * 0.28,
            MAX_BANK_RADIANS * 0.28,
        );
    }

    fn fall_from_platform(&mut self, platform_velocity: Vec2, downward_kick: f32) {
        self.mode = MotionMode::Flying;
        self.surface = None;
        self.platform_lost_timer = 0.0;
        self.edge_squash = 0.0;
        self.velocity += Vec2::new(
            platform_velocity.x * 0.42,
            platform_velocity.y.max(0.0) * 0.34 + downward_kick,
        );
        self.velocity = self.velocity.clamp_len(MAX_THROW_VELOCITY * 1.05);
        self.bank_velocity += platform_velocity.x * 0.0022;
        self.stress = clampf(self.stress + 0.18, 0.0, 1.0);
        let platform_dizzy = clampf(platform_velocity.x.abs() / 1500.0 + 0.12, 0.0, 1.0);
        self.dizziness = clampf(self.dizziness + platform_dizzy, 0.0, 1.0);
        self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.20 + platform_dizzy * 0.72);
        self.surprise_timer = self.surprise_timer.max(0.20);
        self.turn = None;
    }

    fn step_dragged(&mut self, dt: f32) {
        self.drag_lift = approach(self.drag_lift, 1.0, dt * 4.0);
        if self.drag_lift >= 0.72 {
            self.surface = None;
        }

        let instantaneous = (self.drag_world - self.last_mouse) / dt;
        self.mouse_velocity = vlerp(
            self.mouse_velocity,
            instantaneous.clamp_len(2500.0),
            exp_decay(11.5, dt),
        );
        let mouse_accel = (self.mouse_velocity - self.last_mouse_velocity) / dt;
        self.last_mouse_velocity = self.mouse_velocity;
        self.last_mouse = self.drag_world;

        self.update_drag_follow(dt);
        self.keep_drag_anchor_near_target(self.drag_follow_world, dt);

        let kin = self.kinematics();
        let tail_grab = kin.tail_grab_amount();
        let appendage_grab = kin.appendage_grab_amount();
        let shape_guard = kin.drag_shape_guard();
        let anchor_world = self.local_to_world(self.grab_local);
        let error =
            (self.drag_follow_world - anchor_world).clamp_len(DRAG_MAX_ANCHOR_ERROR * self.scale.max(0.2));
        let lift_response = 0.20 + self.drag_lift * 0.62;
        let drag_force = error * (DRAG_ANCHOR_STIFFNESS * lift_response * lerp(1.0, 0.82, shape_guard))
            - self.velocity * (DRAG_ANCHOR_DAMPING * lerp(0.88, 1.16, shape_guard));
        self.velocity += drag_force * dt;
        if tail_grab > 0.001 {
            self.velocity += Vec2::Y * (FREE_GRAVITY * DRAG_TAIL_HANG_GRAVITY_SCALE * tail_grab * dt);
        }
        self.velocity = self.velocity.clamp_len(2450.0);
        self.center += self.velocity * dt;
        self.keep_drag_anchor_near_target(self.drag_follow_world, dt);

        let grab_vec = self.local_vec_to_world(self.grab_local);
        let pin_spin = self.drag_pin_angular_velocity();
        let pin_spin_target = pin_spin.clamp(-DRAG_PIN_SPIN_MAX, DRAG_PIN_SPIN_MAX);
        let pin_spin_response = exp_decay(DRAG_PIN_SPIN_RESPONSE, dt)
            * (0.34 + self.drag_lift * 0.66)
            * lerp(1.0, 0.62, shape_guard)
            * lerp(1.0, 0.50, appendage_grab);
        self.bank_velocity = lerp(self.bank_velocity, pin_spin_target, pin_spin_response);

        let torque = (grab_vec.cross(error) * DRAG_BANK_TORQUE
            + grab_vec.cross(self.mouse_velocity) * 0.000003)
            * (1.0 - shape_guard * 0.34);
        self.bank_velocity += torque * dt;
        self.bank_velocity += self.drag_rotation_error()
            * DRAG_ROTATION_RESPONSE
            * (1.0 - shape_guard * 0.48)
            * (1.0 - appendage_grab * 0.78)
            * dt;
        self.bank_velocity += pin_spin * DRAG_ORBIT_SPIN_GAIN * (1.0 - shape_guard * 0.22) * dt;
        if tail_grab > 0.001 {
            let hang_vec = (self.center - self.drag_follow_world).normalized_or(Vec2::Y);
            let hang_error = hang_vec.cross(Vec2::Y).atan2(hang_vec.dot(Vec2::Y));
            self.bank_velocity += hang_error * DRAG_TAIL_HANG_ROTATION_RESPONSE * tail_grab * dt;
        }
        let max_bank_velocity = lerp(14.0, 9.0, shape_guard);
        self.bank_velocity = clampf(self.bank_velocity, -max_bank_velocity, max_bank_velocity);
        self.bank_velocity *= (-(DRAG_SPIN_DAMPING * (1.0 + shape_guard * 0.55)) * dt).exp();
        let bank_before = self.bank;
        self.bank = wrap_angle(self.bank + self.bank_velocity * dt);
        let bank_delta = wrap_angle(self.bank - bank_before);
        self.pin_drag_body_to_cursor(self.drag_world, bank_delta, dt, shape_guard, appendage_grab);

        let shake = mouse_accel.length() * DRAG_STRESS_GAIN
            + error.length() * 0.00078
            + (self.drag_world - self.drag_follow_world).length() * 0.00036
            + pin_spin.abs() * 0.010
            + self.bank_velocity.abs() * 0.006;
        if shake > 0.035 {
            self.stress = clampf(self.stress + shake * dt * 8.8, 0.0, 1.0);
            self.anger = clampf(self.anger + shake * dt * 1.9, 0.0, 1.0);
            self.dizziness = clampf(self.dizziness + shake * dt * 4.8, 0.0, 1.0);
            self.dizzy_reaction_timer = self
                .dizzy_reaction_timer
                .max((0.24 + shake * 1.35).clamp(0.24, 1.10));
        }
    }

    fn drag_rotation_error(&self) -> f32 {
        let grab_world = self.local_to_world(self.grab_local);
        let from_center = (grab_world - self.center).normalized_or(Vec2::X);
        let to_cursor = (self.drag_follow_world - self.center).normalized_or(from_center);
        from_center
            .cross(to_cursor)
            .atan2(from_center.dot(to_cursor))
            .clamp(-1.25, 1.25)
    }

    fn drag_pin_angular_velocity(&self) -> f32 {
        let r = self.drag_world - self.center;
        let len_sq = r.length_sq();
        if len_sq < 900.0 {
            0.0
        } else {
            (r.cross(self.mouse_velocity) / len_sq).clamp(-24.0, 24.0)
        }
    }

    fn update_drag_follow(&mut self, dt: f32) {
        let follow_response = DRAG_CURSOR_FOLLOW_RESPONSE * (0.54 + self.drag_lift * 0.30);
        self.drag_follow_world = vlerp(
            self.drag_follow_world,
            self.drag_world,
            exp_decay(follow_response, dt),
        );

        let lag = self.drag_world - self.drag_follow_world;
        let distance = lag.length();
        let max_lag = DRAG_CURSOR_MAX_LAG * self.scale.max(0.2);
        if distance > max_lag && distance > 0.001 {
            self.drag_follow_world = self.drag_world - lag * (max_lag / distance);
        }
    }

    fn keep_drag_anchor_near_target(&mut self, target_world: Vec2, dt: f32) {
        let anchor_world = self.local_to_world(self.grab_local);
        let error = target_world - anchor_world;
        let distance = error.length();
        let max_error = DRAG_MAX_ANCHOR_ERROR * self.scale.max(0.2);
        if distance <= max_error || distance <= 0.001 {
            return;
        }
        let correction = error * (((distance - max_error) / distance) * 0.15);
        let correction = correction * (0.18 + self.drag_lift * 0.46);
        self.center += correction;
        self.translate_mesh(correction);
        self.velocity += correction * (0.05 / dt.max(0.001));
        self.velocity = self.velocity.clamp_len(2450.0);
    }

    fn pin_drag_body_to_cursor(
        &mut self,
        target_world: Vec2,
        bank_delta: f32,
        dt: f32,
        shape_guard: f32,
        appendage_grab: f32,
    ) {
        let center_before = self.center;
        let pivot_to_center = self.center - target_world;
        if bank_delta.abs() > 0.00001 && pivot_to_center.length_sq() > 16.0 {
            let orbit_amount = (0.36 + self.drag_lift * 0.64)
                * lerp(1.0, 0.74, shape_guard)
                * lerp(1.0, 0.42, appendage_grab);
            self.center = target_world + pivot_to_center.rotate(bank_delta * orbit_amount);
        }

        let pinned_center = target_world - self.local_vec_to_world(self.grab_local);
        let pin_response = exp_decay(24.0, dt)
            * (0.50 + self.drag_lift * 0.46)
            * lerp(1.0, 0.76, shape_guard)
            * lerp(1.0, 0.48, appendage_grab);
        self.center = vlerp(self.center, pinned_center, pin_response);

        let correction = self.center - center_before;
        self.transform_mesh(center_before, self.center, bank_delta);
        self.velocity += correction * (0.06 / dt.max(0.001));
        self.velocity = self.velocity.clamp_len(2450.0);
    }

    fn translate_mesh(&mut self, delta: Vec2) {
        if delta.length_sq() <= 0.0001 {
            return;
        }

        for node in &mut self.mesh.nodes {
            node.pos += delta;
        }
    }

    fn transform_mesh(&mut self, center_before: Vec2, center_after: Vec2, rotation: f32) {
        let delta = center_after - center_before;
        if delta.length_sq() <= 0.0001 && rotation.abs() <= 0.00001 {
            return;
        }

        for node in &mut self.mesh.nodes {
            node.pos = center_after + (node.pos - center_before).rotate(rotation);
            node.vel = node.vel.rotate(rotation);
        }
    }

    fn step_flying(&mut self, dt: f32, env: &DesktopEnvironment) {
        if self.tumbler.is_some() {
            self.step_tumbler(dt, env);
            return;
        }

        let (gravity_acceleration, gravity_direction) = self.motion_gravity();
        self.velocity += gravity_acceleration * dt;
        self.center += self.velocity * dt;
        if let Some(gravity_direction) = gravity_direction {
            self.normal =
                vlerp(self.normal, gravity_direction, exp_decay(0.9, dt)).normalized_or(gravity_direction);
            let tangent_candidate = self.normal.perp_left().normalized_or(Vec2::X);
            let tangent_target = if tangent_candidate.dot(self.tangent) < 0.0 {
                -tangent_candidate
            } else {
                tangent_candidate
            };
            self.tangent =
                vlerp(self.tangent, tangent_target, exp_decay(1.2, dt)).normalized_or(tangent_target);
        }
        self.bank_velocity += -self.bank * (BANK_RESTORE * 0.72) * dt;
        self.bank_velocity *= (-1.2 * dt).exp();
        self.bank = wrap_angle(self.bank + self.bank_velocity * dt);

        if let Some((contact, snapped, impact)) = env.try_find_collision_surface(
            self.center,
            self.velocity,
            self.body_half_length() * (0.72 + self.yaw_scale * 0.28),
            self.body_center_to_belly(),
        ) {
            self.land_on_surface(contact, snapped, impact, env);
        } else if !env.virtual_bounds.inflate(900).contains(self.center) {
            self.reset_to_safe_surface(env);
        }
    }

    fn step_tumbler(&mut self, dt: f32, env: &DesktopEnvironment) {
        let shaking = self.tumbler.expect("tumbler state checked").shaking;
        let (gravity_acceleration, gravity_direction) = self.motion_gravity();

        self.velocity += gravity_acceleration * dt;
        self.velocity = finite_vec_or(self.velocity, Vec2::ZERO).clamp_len(MAX_THROW_VELOCITY);

        if let Some(gravity_direction) = gravity_direction {
            self.normal =
                vlerp(self.normal, gravity_direction, exp_decay(1.7, dt)).normalized_or(gravity_direction);
            let tangent_candidate = self.normal.perp_left().normalized_or(Vec2::X);
            let tangent_target = if tangent_candidate.dot(self.tangent) < 0.0 {
                -tangent_candidate
            } else {
                tangent_candidate
            };
            self.tangent =
                vlerp(self.tangent, tangent_target, exp_decay(2.0, dt)).normalized_or(tangent_target);
        }
        self.bank_velocity += -self.bank.sin() * (BANK_RESTORE * 0.16) * dt;
        self.bank_velocity *= (-0.46 * dt).exp();
        self.bank_velocity = clampf(self.bank_velocity, -10.0, 10.0);
        self.bank = wrap_angle(self.bank + self.bank_velocity * dt);

        // Capture the current pose's real occupied extents before advancing the rigid center.
        // The mesh is intentionally one soft-body solve behind that center, so measuring after
        // translation would count ordinary frame motion as extra body width.
        let (left_extent, top_extent, right_extent, bottom_extent) = self.tumbler_extents();
        let monitor_index = env.monitor_index_for_point(self.center);
        let work = env.monitors[monitor_index].work;
        let raw_min_x = work.left as f32 + left_extent;
        let raw_max_x = work.right as f32 - right_extent;
        let raw_min_y = work.top as f32 + top_extent;
        let raw_max_y = work.bottom as f32 - bottom_extent;
        let (min_x, max_x) = ordered_container_limits(raw_min_x, raw_max_x);
        let (min_y, max_y) = ordered_container_limits(raw_min_y, raw_max_y);

        let old_center = self.center;
        let incoming = self.velocity;
        self.center += self.velocity * dt;

        let mut hit_x = false;
        let mut hit_y = false;
        let mut bounce_x = false;
        let mut bounce_y = false;
        let mut inward_normal = Vec2::ZERO;
        let mut settle_kind = SurfaceKind::Bottom;
        let mut strongest_impact = 0.0_f32;

        if self.center.x < min_x {
            self.center.x = min_x;
            hit_x = true;
            bounce_x = incoming.x < 0.0;
            inward_normal += Vec2::X;
            settle_kind = SurfaceKind::Left;
            strongest_impact = incoming.x.abs();
        } else if self.center.x > max_x {
            self.center.x = max_x;
            hit_x = true;
            bounce_x = incoming.x > 0.0;
            inward_normal -= Vec2::X;
            settle_kind = SurfaceKind::Right;
            strongest_impact = incoming.x.abs();
        }

        if self.center.y < min_y {
            self.center.y = min_y;
            hit_y = true;
            bounce_y = incoming.y < 0.0;
            inward_normal += Vec2::Y;
            if incoming.y.abs() >= strongest_impact {
                settle_kind = SurfaceKind::Top;
                strongest_impact = incoming.y.abs();
            }
        } else if self.center.y > max_y {
            self.center.y = max_y;
            hit_y = true;
            bounce_y = incoming.y > 0.0;
            inward_normal -= Vec2::Y;
            if incoming.y.abs() >= strongest_impact {
                settle_kind = SurfaceKind::Bottom;
                strongest_impact = incoming.y.abs();
            }
        }

        let correction = self.center - (old_center + incoming * dt);
        self.translate_mesh(correction);

        if hit_x || hit_y {
            // The deliberate-shake gate is the exact boundary between the two behaviours:
            // while it is open, the phone is an energetic tumbler; once it closes, the next
            // real wall contact is a landing. Never feed a low-speed reflection back into the
            // body after shaking stops, otherwise it can visibly chatter in place.
            if !shaking {
                self.settle_tumbler_on_wall(monitor_index, settle_kind, strongest_impact, env);
                return;
            }

            let reflected_x = if bounce_x {
                -incoming.x * TUMBLER_SHAKING_RESTITUTION
            } else if bounce_y {
                incoming.x * TUMBLER_SHAKING_TANGENT_RETENTION
            } else {
                incoming.x
            };
            let reflected_y = if bounce_y {
                -incoming.y * TUMBLER_SHAKING_RESTITUTION
            } else if bounce_x {
                incoming.y * TUMBLER_SHAKING_TANGENT_RETENTION
            } else {
                incoming.y
            };
            self.velocity =
                finite_vec_or(Vec2::new(reflected_x, reflected_y), Vec2::ZERO).clamp_len(MAX_THROW_VELOCITY);

            if bounce_x || bounce_y {
                let reaction_fallback = gravity_direction
                    .map(|direction| -direction)
                    .unwrap_or_else(|| -self.normal.normalized_or(Vec2::Y));
                self.react_to_tumbler_collision(
                    inward_normal.normalized_or(reaction_fallback),
                    strongest_impact,
                );
            }
        }

        if !env.virtual_bounds.inflate(900).contains(self.center)
            || !self.center.x.is_finite()
            || !self.center.y.is_finite()
        {
            self.reset_to_safe_surface(env);
        }
    }

    fn tumbler_extents(&self) -> (f32, f32, f32, f32) {
        // Ears and the long tail are secondary soft appendages. Using their conservative render
        // anchors as a rigid collision hull makes the body rebound from visibly empty space.
        // Collide the actual soft-body outline instead, with only a small fur/stroke allowance.
        let bounds = self.tumbler_collision_bounds();
        let fallback_x = self.body_half_length().max(8.0);
        let fallback_y = (BODY_HALF_HEIGHT * self.scale.max(0.2)).max(8.0);
        let left = (self.center.x - bounds.left).max(8.0);
        let top = (self.center.y - bounds.top).max(8.0);
        let right = (bounds.right - self.center.x).max(8.0);
        let bottom = (bounds.bottom - self.center.y).max(8.0);
        (
            finite_scalar_or(left, fallback_x),
            finite_scalar_or(top, fallback_y),
            finite_scalar_or(right, fallback_x),
            finite_scalar_or(bottom, fallback_y),
        )
    }

    fn tumbler_collision_bounds(&self) -> RectF {
        let visual_margin = (BODY_OUTLINE_ROUGHNESS + 3.0) * self.scale.max(0.2);
        if let Some((min, max)) = self.mesh.bounds() {
            return RectF::new(
                min.x - visual_margin,
                min.y - visual_margin,
                max.x + visual_margin,
                max.y + visual_margin,
            );
        }

        let mut min = self.center;
        let mut max = self.center;
        for local in [
            Vec2::new(-BODY_HALF_LENGTH, -BODY_HALF_HEIGHT),
            Vec2::new(BODY_HALF_LENGTH, -BODY_HALF_HEIGHT),
            Vec2::new(-BODY_HALF_LENGTH, BODY_CENTER_TO_BELLY),
            Vec2::new(BODY_HALF_LENGTH, BODY_CENTER_TO_BELLY),
        ] {
            let point = self.local_to_world(local);
            min = min.min(point);
            max = max.max(point);
        }
        RectF::new(
            min.x - visual_margin,
            min.y - visual_margin,
            max.x + visual_margin,
            max.y + visual_margin,
        )
    }

    fn constrain_tumbler_body(&mut self, env: &DesktopEnvironment) {
        let monitor_index = env.monitor_index_for_point(self.center);
        let work = env.monitors[monitor_index].work;
        let bounds = self.tumbler_collision_bounds();
        let work_width = work.width() as f32;
        let work_height = work.height() as f32;

        let dx = if bounds.width() > work_width {
            (work.left + work.right) as f32 * 0.5 - (bounds.left + bounds.right) * 0.5
        } else if bounds.left < work.left as f32 {
            work.left as f32 - bounds.left
        } else if bounds.right > work.right as f32 {
            work.right as f32 - bounds.right
        } else {
            0.0
        };
        let dy = if bounds.height() > work_height {
            (work.top + work.bottom) as f32 * 0.5 - (bounds.top + bounds.bottom) * 0.5
        } else if bounds.top < work.top as f32 {
            work.top as f32 - bounds.top
        } else if bounds.bottom > work.bottom as f32 {
            work.bottom as f32 - bounds.bottom
        } else {
            0.0
        };
        let correction = finite_vec_or(Vec2::new(dx, dy), Vec2::ZERO);
        self.center += correction;
        self.translate_mesh(correction);
    }

    fn react_to_tumbler_collision(&mut self, inward_normal: Vec2, impact: f32) {
        let impact = if impact.is_finite() {
            impact.clamp(0.0, MAX_THROW_VELOCITY)
        } else {
            0.0
        };
        let amount = smoothstep(180.0, 1750.0, impact);
        self.bank_velocity += inward_normal.cross(self.velocity) * 0.0042;
        self.bank_velocity += self.velocity.normalized_or(Vec2::X).x * amount * 1.4;
        self.bank_velocity = clampf(self.bank_velocity, -10.0, 10.0);
        self.impact_squash = self.impact_squash.max(0.12 + amount * 0.36);
        self.stress = clampf(self.stress + 0.10 + amount * 0.34, 0.0, 1.0);
        self.anger = clampf(self.anger + amount * 0.12, 0.0, 1.0);
        self.dizziness = clampf(self.dizziness + 0.12 + amount * 0.38, 0.0, 1.0);
        self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.48 + amount * 0.92);
        if impact > 920.0 {
            self.panic_timer = self.panic_timer.max(0.30 + amount * 0.42);
        } else {
            self.surprise_timer = self.surprise_timer.max(0.16 + amount * 0.22);
        }

        let radius = self.content_radius().max(1.0);
        for node in &mut self.mesh.nodes {
            let wallward = (-(node.pos - self.center).dot(inward_normal) / radius).clamp(0.0, 1.0);
            let contact_weight = smoothstep(0.12, 0.88, wallward);
            node.vel += inward_normal * (impact * (0.035 + contact_weight * 0.13));
        }
    }

    fn settle_tumbler_on_wall(
        &mut self,
        monitor_index: usize,
        kind: SurfaceKind,
        impact: f32,
        env: &DesktopEnvironment,
    ) {
        let contact = SurfaceContact::monitor(monitor_index, kind);
        let (lo, hi) = env.tangent_extent(contact);
        let half_len = self.body_half_length();
        let center_offset = self.body_center_to_belly();
        let (normal_snap, _, _) = env.constrain_to_surface(contact, self.center, half_len, center_offset);
        let normal_coordinate = DesktopEnvironment::tangent_coord(kind, normal_snap);
        // The body rotates through a surface transition after landing. Reserve its true radial
        // content extent, not only the current pose's tangent AABB, so ears and tail cannot sweep
        // through a corner while that transition completes.
        let tangent_radius = finite_scalar_or(self.content_radius(), half_len).max(half_len);
        let min = lo + tangent_radius;
        let max = hi - tangent_radius;
        let coordinate = normal_coordinate;
        let coordinate = if min <= max {
            clampf(coordinate, min, max)
        } else {
            (lo + hi) * 0.5
        };
        let snapped = env.point_from_tangent(contact, coordinate, center_offset);
        self.tumbler = None;
        self.land_on_surface(contact, snapped, impact, env);
    }

    fn land_on_surface(
        &mut self,
        contact: SurfaceContact,
        snapped: Vec2,
        impact: f32,
        env: &DesktopEnvironment,
    ) {
        self.tumbler = None;
        self.surface = Some(contact);
        self.mode = MotionMode::Attached;
        self.platform_lost_timer = 0.0;
        self.edge_squash = env.screen_edge_pinch_amount(contact, self.body_center_to_belly());
        self.center = snapped;
        self.crawl_drive = 0.0;
        self.crawl_phase = CRAWL_REST_PHASE;
        let tangent = DesktopEnvironment::surface_tangent(contact.kind);
        let support_velocity = if contact.is_platform() {
            env.contact_velocity(contact)
        } else {
            Vec2::ZERO
        };
        let tangent_velocity = (self.velocity - support_velocity).dot(tangent);
        self.move_sign = if tangent_velocity >= 0.0 { 1 } else { -1 };
        if tangent_velocity.abs() < 24.0 {
            self.move_sign = if self.rng.f32() < 0.5 { -1 } else { 1 };
        }
        self.facing_sign = self.move_sign;
        self.velocity = tangent * (tangent_velocity * 0.17) + support_velocity * 0.24;
        self.bank_velocity *= 0.18;
        self.bank *= 0.42;
        self.impact_squash = clampf(impact / 1900.0, 0.08, 0.42);
        self.stress = clampf(self.stress + self.impact_squash * 0.82, 0.0, 1.0);
        self.anger = clampf(self.anger + self.impact_squash * 0.56, 0.0, 1.0);
        if impact > 520.0 || self.throw_anger_timer > 0.02 {
            self.throw_anger_timer = self.throw_anger_timer.max(clampf(impact / 1500.0, 0.28, 0.88));
        }
        self.dizziness = self.dizziness.max(clampf(impact / 2400.0, 0.0, 0.72));
        if impact > 520.0 {
            self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.30 + self.dizziness * 0.54);
        }
        if contact.is_platform() {
            let support_dizzy = clampf(support_velocity.length() / 1250.0, 0.0, 0.58);
            self.dizziness = self.dizziness.max(support_dizzy);
            if support_dizzy > 0.18 {
                self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.18 + support_dizzy * 0.72);
            }
        }
        self.sadness = self.sadness.max(self.impact_squash * 0.72);
        if impact > 920.0 {
            self.panic_timer = self.panic_timer.max(clampf(impact / 2600.0, 0.16, 0.42));
        } else if impact > 420.0 {
            self.surprise_timer = self.surprise_timer.max(clampf(impact / 2200.0, 0.08, 0.22));
        }
        self.begin_surface_transition(contact);
    }

    fn request_turn(&mut self, target_sign: i32) {
        if target_sign == self.move_sign || self.turn.is_some() {
            return;
        }
        self.turn = Some(TurnState {
            target_sign,
            t: 0.0,
            swapped: false,
        });
    }

    fn advance_turn(&mut self, dt: f32) {
        if let Some(mut turn) = self.turn {
            turn.t += dt / TURN_DURATION;
            let phase = clampf(turn.t, 0.0, 1.0);
            let narrow = (std::f32::consts::PI * phase).sin();
            self.yaw_scale = lerp(1.0, TURN_MIN_YAW_SCALE, narrow);
            if !turn.swapped && phase >= 0.5 {
                self.move_sign = turn.target_sign;
                self.facing_sign = turn.target_sign;
                turn.swapped = true;
            }
            if phase >= 1.0 {
                self.turn = None;
                self.yaw_scale = 1.0;
            } else {
                self.turn = Some(turn);
            }
        } else {
            self.yaw_scale = approach(self.yaw_scale, 1.0, dt * 3.0);
        }
    }

    fn update_surface_frame(&mut self, _env: &DesktopEnvironment) {
        if let Some(contact) = self.surface {
            self.normal = DesktopEnvironment::surface_normal(contact.kind);
            self.tangent = DesktopEnvironment::surface_tangent(contact.kind);
        }
    }

    fn begin_surface_transition(&mut self, contact: SurfaceContact) {
        self.surface_transition = SURFACE_TRANSITION_DURATION;
        self.surface_transition_from_normal = self.normal.normalized_or(Vec2::Y);
        self.surface_transition_from_tangent = self.tangent.normalized_or(Vec2::X);
        self.normal = DesktopEnvironment::surface_normal(contact.kind);
        self.tangent = DesktopEnvironment::surface_tangent(contact.kind);
    }

    fn advance_surface_transition(&mut self, dt: f32) {
        if let Some(contact) = self.surface {
            let target_normal = DesktopEnvironment::surface_normal(contact.kind);
            let target_tangent = DesktopEnvironment::surface_tangent(contact.kind);
            if self.surface_transition > 0.0 {
                self.surface_transition = (self.surface_transition - dt).max(0.0);
                let t = 1.0 - self.surface_transition / SURFACE_TRANSITION_DURATION;
                let smooth = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
                self.normal = vlerp(self.surface_transition_from_normal, target_normal, smooth)
                    .normalized_or(target_normal);
                self.tangent = vlerp(self.surface_transition_from_tangent, target_tangent, smooth)
                    .normalized_or(target_tangent);
            } else {
                self.normal = target_normal;
                self.tangent = target_tangent;
            }
        }
    }

    fn body_half_length(&self) -> f32 {
        BODY_HALF_LENGTH * self.scale.max(0.2)
    }

    fn body_center_to_belly(&self) -> f32 {
        BODY_CENTER_TO_BELLY * self.scale.max(0.2)
    }

    fn raw_x_axis(&self) -> Vec2 {
        let head_direction = self.tangent * self.facing_sign as f32;
        let base_x = -head_direction.normalized_or(Vec2::X) * self.yaw_scale.max(0.05) * self.scale.max(0.2);
        base_x.rotate(self.bank)
    }

    fn raw_y_axis(&self) -> Vec2 {
        (self.normal.normalized_or(Vec2::Y) * self.scale.max(0.2)).rotate(self.bank)
    }

    fn clamp_grab_local(&self, local: Vec2) -> Vec2 {
        let x = clampf(local.x, -BODY_HALF_LENGTH * 1.08, BODY_HALF_LENGTH * 1.08);
        let y = clampf(local.y, -BODY_HALF_HEIGHT * 1.24, BODY_HALF_HEIGHT * 1.18);
        Vec2::new(x, y)
    }

    fn clamp_drag_grab_local(&self, local: Vec2) -> Vec2 {
        let extra = HOVER_RANGE / self.scale.max(0.2);
        for ear in EARS {
            if self.ear_contains(local, extra, ear) {
                let tip = ear_tip(ear);
                let leaf_margin = (EAR_LEAF_HALF_WIDTH + 10.0) * ear.scale;
                let min = ear.anchor.min(tip) - Vec2::new(leaf_margin, 18.0 * ear.scale);
                let max = ear.anchor.max(tip) + Vec2::new(leaf_margin, 14.0 * ear.scale);
                return Vec2::new(clampf(local.x, min.x, max.x), clampf(local.y, min.y, max.y));
            }
        }

        if self.tail_contains(local, extra) {
            let x = clampf(local.x, TAIL_ANCHOR.x - 62.0, TAIL_ANCHOR.x + 98.0);
            let y = clampf(local.y, TAIL_ANCHOR.y - 86.0, TAIL_ANCHOR.y + 58.0);
            Vec2::new(x, y)
        } else {
            self.clamp_grab_local(local)
        }
    }

    fn tail_contains(&self, local: Vec2, extra: f32) -> bool {
        (local - TAIL_ANCHOR).length() <= TAIL_GRAB_RADIUS + extra
    }

    fn body_hit_test(&self, world: Vec2, extra: f32) -> bool {
        let nodes = &self.mesh.nodes;
        if nodes.len() < 3 {
            return self.body_slug_contains(self.world_to_local(world), extra / self.scale.max(0.2));
        }

        let mut inside = false;
        let mut previous = nodes.len() - 1;
        for i in 0..nodes.len() {
            let a = nodes[i].pos;
            let b = nodes[previous].pos;
            if (a.y > world.y) != (b.y > world.y) {
                let cross_x = (b.x - a.x) * (world.y - a.y) / (b.y - a.y) + a.x;
                if world.x < cross_x {
                    inside = !inside;
                }
            }
            previous = i;
        }
        if inside {
            return true;
        }

        if extra <= 0.0 {
            return false;
        }

        for i in 0..nodes.len() {
            let a = nodes[i].pos;
            let b = nodes[(i + 1) % nodes.len()].pos;
            if point_segment_distance(world, a, b) <= extra {
                return true;
            }
        }
        false
    }

    fn ear_contains(&self, local: Vec2, extra: f32, ear: EarSpec) -> bool {
        ear_leaf_distance(local, ear) <= 5.0 * ear.scale + extra
    }

    fn world_vec_to_local_axes(vec: Vec2, x_axis: Vec2, y_axis: Vec2) -> Vec2 {
        let x_len = x_axis.length().max(0.001);
        let y_len = y_axis.length().max(0.001);
        Vec2::new(vec.dot(x_axis / x_len) / x_len, vec.dot(y_axis / y_len) / y_len)
    }

    fn body_slug_contains(&self, local: Vec2, extra: f32) -> bool {
        let hx = BODY_HALF_LENGTH + extra;
        if local.x.abs() > hx {
            return false;
        }
        let nx = local.x.abs() / hx;
        let top = -BODY_HALF_HEIGHT * (1.0 - nx.powf(2.35)).max(0.0).powf(0.54) - extra * 0.6;
        let corner_lift = BELLY_AIR_CORNER_LIFT * smoothstep(0.68, 1.0, nx);
        let bottom = BODY_CENTER_TO_BELLY + 2.0 * (1.0 - nx.powf(2.0)).max(0.0) - corner_lift + extra;
        local.y >= top && local.y <= bottom
    }

    fn update_hover(&mut self, dt: f32) {
        let cursor_delta = if self.last_cursor_valid {
            self.cursor_world - self.last_cursor_world
        } else {
            Vec2::ZERO
        };
        self.last_cursor_world = self.cursor_world;
        self.last_cursor_valid = true;

        let instantaneous = if dt > 0.0001 {
            (cursor_delta / dt).clamp_len(2300.0)
        } else {
            Vec2::ZERO
        };
        self.passive_mouse_velocity = vlerp(
            self.passive_mouse_velocity,
            instantaneous,
            exp_decay(
                if self.mode == MotionMode::Dragged {
                    18.0
                } else {
                    10.5
                },
                dt,
            ),
        );

        let local = self.world_to_local(self.cursor_world);
        let appendage_extra_local = (HOVER_RANGE + 34.0) / self.scale.max(0.2);
        let body_hover = self.body_hit_test(self.cursor_world, HOVER_RANGE);
        let ear_hover = EARS
            .iter()
            .copied()
            .any(|ear| self.ear_contains(local, appendage_extra_local, ear));
        let tail_hover = self.tail_contains(local, appendage_extra_local);
        let appendage_hover = ear_hover || tail_hover;
        let hovering = body_hover || appendage_hover;
        let hover_pauses_movement = hovering && self.mode == MotionMode::Attached;
        if hover_pauses_movement && !self.hover_pause_hovering {
            self.hover_pause_timer = HOVER_PAUSE_DURATION;
        }
        self.hover_pause_hovering = hover_pauses_movement;

        self.hover_amount = approach(self.hover_amount, if hovering { 1.0 } else { 0.0 }, dt * 5.0);

        // Passive petting: moving the cursor over the fur/ears/tail without clicking warms Fushi up.
        // It is disabled while dragging so grabs still read as a stronger interaction.
        let speed = self.passive_mouse_velocity.length();
        let stroke_speed = smoothstep(10.0, 180.0, speed) * (1.0 - smoothstep(1650.0, 2500.0, speed));
        let pettable = self.mode != MotionMode::Dragged && hovering;
        let pet_target = if pettable {
            let base = if appendage_hover { 0.42 } else { 0.16 };
            let gain = if appendage_hover { 0.58 } else { 0.84 };
            (base + stroke_speed * gain).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let body_target = if pettable && body_hover { pet_target } else { 0.0 };
        let appendage_target = if pettable && appendage_hover {
            pet_target.max(0.52)
        } else {
            0.0
        };
        self.petting_amount = approach(
            self.petting_amount,
            pet_target,
            dt * if pet_target > self.petting_amount {
                8.0
            } else {
                3.4
            },
        );
        self.body_pet_amount = approach(
            self.body_pet_amount,
            body_target,
            dt * if body_target > self.body_pet_amount {
                9.0
            } else {
                3.2
            },
        );
        self.appendage_pet_amount = approach(
            self.appendage_pet_amount,
            appendage_target,
            dt * if appendage_target > self.appendage_pet_amount {
                14.0
            } else {
                3.2
            },
        );
        let upset_lock = smoothstep(0.16, 0.56, self.anger.max(self.stress * 0.82)).max(smoothstep(
            0.02,
            0.38,
            self.throw_anger_timer,
        ));
        let friendly_pet = self.petting_amount * (1.0 - upset_lock * 0.72);
        let happiness_target = if friendly_pet > 0.12 {
            (friendly_pet * 1.10).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.happiness = approach(
            self.happiness,
            happiness_target,
            dt * if happiness_target > self.happiness {
                1.35
            } else {
                0.46
            },
        );

        if self.petting_amount > 0.02 {
            let soothe = self.petting_amount * dt;
            let anger_guard = 1.0 - smoothstep(0.02, 0.72, self.throw_anger_timer) * 0.78;
            self.stress = (self.stress - soothe * 0.48).max(0.0);
            self.anger = (self.anger - soothe * 0.26 * anger_guard).max(0.0);
            self.sadness = (self.sadness - soothe * 0.40).max(0.0);
            self.dizziness = (self.dizziness - soothe * 0.22).max(0.0);
        }

        // Ears and tail are lighter material than the body.  Hover-brushing
        // injects spring velocity even when the mouse is not clicked, while
        // roots stay locked in the renderer by tip-weighted deformation.
        let local_passive =
            Self::world_vec_to_local_axes(self.passive_mouse_velocity, self.raw_x_axis(), self.raw_y_axis())
                .clamp_len(1800.0);
        if self.mode != MotionMode::Dragged && appendage_hover {
            let brush = appendage_target * stroke_speed;
            if brush > 0.001 {
                self.appendage_sway_velocity += local_passive * (0.070 * brush * dt);
                self.appendage_sway_velocity.y += (self.time * 18.0).sin() * 42.0 * brush * dt;
            }
        }
        let local_body_velocity =
            Self::world_vec_to_local_axes(self.velocity, self.raw_x_axis(), self.raw_y_axis())
                .clamp_len(1200.0);
        let flutter_drive = match self.mode {
            MotionMode::Attached => smoothstep(12.0, 145.0, local_body_velocity.length()) * 0.42,
            MotionMode::Dragged => smoothstep(70.0, 780.0, self.mouse_velocity.length()) * 0.82,
            MotionMode::Flying => smoothstep(130.0, 1120.0, self.velocity.length()) * 0.88,
        };
        if flutter_drive > 0.001 {
            self.appendage_sway_velocity += Vec2::new(
                -local_body_velocity.x * 0.018,
                local_body_velocity.y * 0.007 + (self.time * 16.0).sin() * 34.0,
            ) * (flutter_drive * dt);
        }
        let spring = match self.mode {
            MotionMode::Attached => 14.5,
            MotionMode::Dragged => 10.0,
            MotionMode::Flying => 8.8,
        };
        let damping = match self.mode {
            MotionMode::Attached => 5.8,
            MotionMode::Dragged => 3.8,
            MotionMode::Flying => 3.4,
        };
        self.appendage_sway_velocity += -self.appendage_sway * spring * dt;
        self.appendage_sway_velocity *= (-damping * dt).exp();
        self.appendage_sway += self.appendage_sway_velocity * dt;
        self.appendage_sway = self
            .appendage_sway
            .clamp_len(44.0 + self.appendage_pet_amount * 20.0);
    }

    fn update_blink(&mut self, dt: f32) {
        // blink_timer > 0: waiting for the next blink.
        // blink_timer < 0: currently playing a close/open eyelid curve.
        // This makes blinking read as a small animation instead of a one-frame
        // sprite swap, and avoids losing the blink when the face decal moves.
        if self.blink_timer > 0.0 {
            self.blink_timer -= dt;
            self.blink_amount = approach(self.blink_amount, 0.0, dt * 12.0);
            if self.blink_timer <= 0.0 {
                let duration = 0.145 + self.rng.f32() * 0.050;
                self.blink_timer = -duration;
            }
            return;
        }

        let duration = (-self.blink_timer).max(0.085);
        let next_timer = self.blink_timer + dt;
        let phase = ((duration + next_timer) / duration).clamp(0.0, 1.0);
        self.blink_amount = (phase * std::f32::consts::PI).sin().max(0.0).powf(0.72);
        self.blink_timer = next_timer;

        if self.blink_timer >= 0.0 {
            self.blink_amount = 0.0;
            let sleepy_bias = smoothstep(0.42, 0.92, self.sleepiness);
            self.blink_timer = lerp(2.8, 1.35, sleepy_bias) + self.rng.f32() * lerp(4.2, 1.6, sleepy_bias);
        }
    }

    fn update_view_pose(&mut self, dt: f32) {
        // Do not make Fushi deliberately look left/right while crawling.  The side-to-side
        // "3D showcase" motion made the character feel like a turntable instead of a pet
        // that simply crawls forward.  View yaw is now a small physical residue only for
        // throws/drags/turn compression, while attached movement stays forward-facing.
        self.look_timer = (self.look_timer - dt).max(0.0);
        self.look_target_yaw = 0.0;
        self.look_target_pitch = 0.0;

        let x_dir = self.raw_x_axis().normalized_or(Vec2::X);
        let y_dir = self.raw_y_axis().normalized_or(Vec2::Y);
        let local_velocity = Vec2::new(self.velocity.dot(x_dir), self.velocity.dot(y_dir));
        let local_mouse_velocity = Vec2::new(self.mouse_velocity.dot(x_dir), self.mouse_velocity.dot(y_dir));
        let turn_denominator = (1.0_f32 - TURN_MIN_YAW_SCALE).max(0.001);
        let turn_amount = ((1.0 - self.yaw_scale) / turn_denominator).clamp(0.0, 1.0);

        let crawl_pitch = if self.mode == MotionMode::Attached {
            (self.crawl_phase + 2.15).sin() * 0.008 * self.crawl_drive
        } else {
            0.0
        };
        let turn_yaw = self.facing_sign as f32 * turn_amount * 0.026;
        let attached_bank_yaw = if self.mode == MotionMode::Attached {
            clampf(self.bank_velocity * 0.0014, -0.012, 0.012)
        } else {
            0.0
        };
        let motion_yaw = match self.mode {
            MotionMode::Attached => 0.0,
            MotionMode::Dragged => clampf(-local_mouse_velocity.x / 8800.0, -0.060, 0.060),
            MotionMode::Flying => clampf(-local_velocity.x / 9200.0, -0.052, 0.052),
        };
        let motion_pitch = match self.mode {
            MotionMode::Attached => 0.0,
            MotionMode::Dragged => clampf(local_mouse_velocity.y / 9000.0, -0.048, 0.048),
            MotionMode::Flying => clampf(local_velocity.y / 9400.0, -0.050, 0.062),
        };

        let mut target_yaw = match self.mode {
            MotionMode::Attached => turn_yaw + attached_bank_yaw,
            MotionMode::Dragged | MotionMode::Flying => {
                motion_yaw + clampf(self.bank_velocity * 0.0027, -0.038, 0.038)
            }
        };
        let mut target_pitch = match self.mode {
            MotionMode::Attached => crawl_pitch - self.impact_squash * 0.030,
            MotionMode::Dragged | MotionMode::Flying => motion_pitch - self.impact_squash * 0.040,
        };

        if self.expression == FushiExpression::Sleepy {
            target_yaw *= 0.45;
            target_pitch *= 0.45;
        }

        target_yaw = target_yaw.clamp(-0.095, 0.095);
        target_pitch = target_pitch.clamp(-0.070, 0.080);

        let response = match self.mode {
            MotionMode::Dragged => 16.0,
            MotionMode::Flying => 14.0,
            MotionMode::Attached => 10.5,
        };
        let damping = match self.mode {
            MotionMode::Dragged => 7.6,
            MotionMode::Flying => 7.0,
            MotionMode::Attached => 6.4,
        };
        self.view_yaw_velocity += (target_yaw - self.view_yaw) * response * dt;
        self.view_pitch_velocity += (target_pitch - self.view_pitch) * (response * 0.86) * dt;
        self.view_yaw_velocity *= (-damping * dt).exp();
        self.view_pitch_velocity *= (-(damping * 1.05) * dt).exp();
        self.view_yaw += self.view_yaw_velocity * dt;
        self.view_pitch += self.view_pitch_velocity * dt;

        if self.view_yaw < -0.11 {
            self.view_yaw = -0.11;
            self.view_yaw_velocity = self.view_yaw_velocity.max(0.0);
        } else if self.view_yaw > 0.11 {
            self.view_yaw = 0.11;
            self.view_yaw_velocity = self.view_yaw_velocity.min(0.0);
        }
        if self.view_pitch < -0.085 {
            self.view_pitch = -0.085;
            self.view_pitch_velocity = self.view_pitch_velocity.max(0.0);
        } else if self.view_pitch > 0.090 {
            self.view_pitch = 0.090;
            self.view_pitch_velocity = self.view_pitch_velocity.min(0.0);
        }
    }

    fn update_shake_dizziness(&mut self, dt: f32) {
        // Dizziness is a visible reaction, so keep a brief expression latch when
        // the body is whipped/spun by drag, platform shake, phone shake, or impact.
        let drag_whip = if self.mode == MotionMode::Dragged {
            smoothstep(560.0, 1800.0, self.mouse_velocity.length())
        } else {
            0.0
        };
        let spin = smoothstep(2.1, 7.2, self.bank_velocity.abs());
        let impact = smoothstep(0.12, 0.46, self.impact_squash);
        let airborne_tumble = if self.mode == MotionMode::Flying {
            smoothstep(450.0, 1450.0, self.velocity.length()) * smoothstep(1.0, 5.2, self.bank_velocity.abs())
        } else {
            0.0
        };
        let shake_drive = spin.max(drag_whip).max(impact).max(airborne_tumble);
        if shake_drive > 0.04 {
            self.dizziness = self.dizziness.max(0.30 + shake_drive * 0.50);
            self.dizzy_reaction_timer = self
                .dizzy_reaction_timer
                .max(0.30 + shake_drive * 0.72 + self.dizziness * 0.16);
        }

        if self.dizziness > 0.58 && (spin > 0.28 || drag_whip > 0.22) {
            self.dizzy_reaction_timer = self.dizzy_reaction_timer.max(0.55 + dt * 2.0);
        }
    }

    fn update_expression(&mut self, dt: f32) {
        let upset = self
            .anger
            .max(self.stress * 0.82)
            .max(self.throw_anger_timer * 0.70);
        let calm = self.mode == MotionMode::Attached
            && self.hover_amount < 0.2
            && self.anger < 0.15
            && self.stress < 0.12
            && self.petting_amount < 0.18;
        self.sleepiness = approach(
            self.sleepiness,
            if calm { 1.0 } else { 0.0 },
            if calm { dt * 0.03 } else { dt * 2.5 },
        );

        let flying_speed = self.velocity.length();
        let thrown_angry = self.throw_anger_timer > 0.05 && self.anger > 0.12;
        let rotational_dizzy = smoothstep(2.6, 7.6, self.bank_velocity.abs());
        let active_dizzy = self.dizzy_reaction_timer > 0.02
            || self.dizziness > 0.50
            || (self.dizziness > 0.28
                && (self.mode == MotionMode::Dragged
                    || self.mode == MotionMode::Flying
                    || self.stress > 0.16
                    || rotational_dizzy > 0.20));
        let hard_angry = (self.throw_anger_timer > 0.28 && self.anger > 0.24) || self.anger > 0.76;
        let strong_dizzy = active_dizzy && (self.dizziness > 0.58 || self.dizzy_reaction_timer > 0.22);
        let friendly_attention =
            (self.hover_amount * 0.42 + self.petting_amount * 0.78 + self.happiness * 0.64)
                * (1.0 - smoothstep(0.16, 0.58, upset.max(self.dizziness * 0.45)));
        let ready_to_smile = friendly_attention > 0.68
            && self.mode == MotionMode::Attached
            && self.throw_anger_timer <= 0.02
            && self.anger < 0.22
            && self.stress < 0.30
            && self.dizziness < 0.24
            && self.sadness < 0.42;

        let next_expression = if strong_dizzy {
            FushiExpression::Dizzy
        } else if self.panic_timer > 0.1
            || (self.mode == MotionMode::Flying && (flying_speed > 1120.0 || self.stress > 0.72))
        {
            FushiExpression::Panic
        } else if hard_angry {
            FushiExpression::Angry
        } else if active_dizzy {
            // Shaking should not be swallowed by ordinary angry/grumpy logic.
            FushiExpression::Dizzy
        } else if thrown_angry || self.anger > 0.66 {
            FushiExpression::Angry
        } else if self.mode == MotionMode::Flying && flying_speed > 280.0 {
            FushiExpression::Surprised
        } else if self.mode == MotionMode::Dragged && self.anger > 0.50 {
            FushiExpression::Angry
        } else if self.surprise_timer > 0.03 {
            FushiExpression::Surprised
        } else if self.anger > 0.30 || self.stress > 0.38 || self.throw_anger_timer > 0.02 {
            FushiExpression::Grumpy
        } else if self.sadness > 0.48 && self.anger < 0.38 {
            FushiExpression::Sad
        } else if ready_to_smile {
            FushiExpression::Stare
        } else if self.sleepiness > 0.74 && upset < 0.16 {
            FushiExpression::Sleepy
        } else {
            FushiExpression::Default
        };

        if next_expression != self.expression {
            self.previous_expression = self.expression;
            self.expression = next_expression;
            self.expression_transition = 0.0;
        } else {
            let leaving_big_reaction = matches!(
                self.previous_expression,
                FushiExpression::Surprised
                    | FushiExpression::Panic
                    | FushiExpression::Angry
                    | FushiExpression::Dizzy
            ) && matches!(
                self.expression,
                FushiExpression::Default
                    | FushiExpression::Sleepy
                    | FushiExpression::Stare
                    | FushiExpression::Grumpy
            );
            let entering_big_reaction = matches!(
                self.expression,
                FushiExpression::Surprised
                    | FushiExpression::Panic
                    | FushiExpression::Angry
                    | FushiExpression::Dizzy
            );
            let entering_smile = matches!(self.expression, FushiExpression::Stare);
            let rate = if leaving_big_reaction {
                1.90
            } else if entering_big_reaction {
                6.6
            } else if entering_smile {
                2.45
            } else {
                4.85
            };
            self.expression_transition = approach(self.expression_transition, 1.0, dt * rate);
            if self.expression_transition >= 0.995 {
                self.previous_expression = self.expression;
                self.expression_transition = 1.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::RectI;

    const TEST_DT: f32 = 1.0 / 60.0;

    fn test_environment() -> DesktopEnvironment {
        DesktopEnvironment::from_screen_work_area(RectI::new(0, 0, 1400, 1000), RectI::new(30, 40, 1370, 960))
    }

    fn trigger_tumbler(body: &mut FushiBody) {
        body.apply_container_motion(
            Vec2::new(0.55, -0.08),
            Vec2::new(0.0, 9.81),
            0.82,
            true,
            true,
            true,
            true,
        );
    }

    fn centered_tumbler(env: &DesktopEnvironment) -> FushiBody {
        let mut body = FushiBody::new(env);
        trigger_tumbler(&mut body);
        let work = env.monitors[0].work;
        let target = Vec2::new(
            (work.left + work.right) as f32 * 0.5,
            (work.top + work.bottom) as f32 * 0.5,
        );
        body.translate_world(target - body.center);
        body.velocity = Vec2::ZERO;
        body
    }

    #[test]
    fn weak_container_motion_has_no_effect_before_trigger() {
        let env = test_environment();
        let mut candidate = FushiBody::new(&env);
        let mut control = candidate.clone();

        for frame in 0..(60 * 60) {
            let phase = frame as f32 * 0.13;
            candidate.apply_container_motion(
                Vec2::new(phase.sin() * 1.1, phase.cos() * 0.9),
                Vec2::new(3.0, 9.2),
                0.24,
                true,
                true,
                false,
                true,
            );
            candidate.step(TEST_DT, &env);
            control.step(TEST_DT, &env);
        }

        assert_eq!(candidate.mode, control.mode);
        assert_eq!(candidate.center, control.center);
        assert_eq!(candidate.velocity, control.velocity);
        assert_eq!(candidate.bank, control.bank);
        assert!(candidate.tumbler.is_none());
    }

    #[test]
    fn strong_trigger_detaches_from_surface() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);

        trigger_tumbler(&mut body);

        assert_eq!(body.mode, MotionMode::Flying);
        assert!(body.surface.is_none());
        assert!(body.tumbler.is_some());
        assert!(body.velocity.length() >= TUMBLER_INITIAL_KICK_MIN * 0.75);
        assert!(body.velocity.y < 0.0, "bottom-surface kick must point inward");
        assert!(body.stress >= 0.55);
        assert!(body.dizziness >= 0.62);
    }

    #[test]
    fn faster_shake_creates_a_larger_body_velocity_change() {
        let env = test_environment();
        let mut slow = FushiBody::new(&env);
        let mut fast = slow.clone();

        slow.apply_container_motion(
            Vec2::X * 0.25,
            Vec2::Y * TUMBLER_STANDARD_GRAVITY,
            0.70,
            true,
            true,
            true,
            true,
        );
        fast.apply_container_motion(
            Vec2::X,
            Vec2::Y * TUMBLER_STANDARD_GRAVITY,
            0.70,
            true,
            true,
            true,
            true,
        );

        assert!(
            fast.velocity.length() > slow.velocity.length() * 1.8,
            "slow={} fast={}",
            slow.velocity.length(),
            fast.velocity.length()
        );
    }

    #[test]
    fn drag_cancels_and_ignores_tumbler_input() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        assert!(body.tumbler.is_some());

        let drag_point = body.center;
        assert!(body.begin_drag_unchecked(drag_point));
        let velocity_before = body.velocity;
        assert_eq!(body.mode, MotionMode::Dragged);
        assert!(body.tumbler.is_none());

        trigger_tumbler(&mut body);

        assert_eq!(body.mode, MotionMode::Dragged);
        assert_eq!(body.velocity, velocity_before);
        assert!(body.tumbler.is_none());
    }

    #[test]
    fn active_tumbler_bounces_without_landing() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let (left_extent, _, _, _) = body.tumbler_extents();
        let left_limit = env.monitors[0].work.left as f32 + left_extent;
        body.translate_world(Vec2::new(left_limit + 2.0 - body.center.x, -180.0));
        body.velocity = Vec2::new(-1100.0, -90.0);

        body.step(TEST_DT, &env);

        assert_eq!(body.mode, MotionMode::Flying);
        assert!(body.surface.is_none());
        assert!(body.tumbler.is_some());
        assert!(body.velocity.x > 0.0);
        assert!(body.impact_squash >= 0.1);
    }

    #[test]
    fn tumbler_wall_bounce_uses_configured_normal_and_tangent_retention() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let (left_extent, _, _, _) = body.tumbler_extents();
        let left_limit = env.monitors[0].work.left as f32 + left_extent;
        body.translate_world(Vec2::new(left_limit + 1.0 - body.center.x, -180.0));
        body.velocity = Vec2::new(-1000.0, 300.0);

        body.step(TEST_DT, &env);

        let incoming_tangent = 300.0 + FREE_GRAVITY * TEST_DT;
        assert!((body.velocity.x - 1000.0 * TUMBLER_SHAKING_RESTITUTION).abs() < 0.1);
        assert!((body.velocity.y - incoming_tangent * TUMBLER_SHAKING_TANGENT_RETENTION).abs() < 0.1);
        assert_eq!(body.mode, MotionMode::Flying);
    }

    #[test]
    fn open_shake_gate_keeps_even_a_low_speed_wall_contact_bouncing() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let (left_extent, _, _, _) = body.tumbler_extents();
        let left_limit = env.monitors[0].work.left as f32 + left_extent;
        body.translate_world(Vec2::new(left_limit + 0.5 - body.center.x, -180.0));
        body.velocity = Vec2::new(-80.0, 15.0);

        body.step(TEST_DT, &env);

        assert_eq!(body.mode, MotionMode::Flying);
        assert!(body.surface.is_none());
        assert!(body.tumbler.expect("open gate").shaking);
        assert!(body.velocity.x > 0.0, "vx={}", body.velocity.x);
    }

    #[test]
    fn appendage_render_anchors_do_not_create_an_invisible_tumbler_wall() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let content = body.content_bounds();
        let collision = body.tumbler_collision_bounds();
        assert!(
            collision.top - content.top > 40.0 * body.scale,
            "ear/tail bounds must not pad the rigid wall: content={} collision={}",
            content.top,
            collision.top
        );

        let (_, top_extent, _, _) = body.tumbler_extents();
        let top_limit = env.monitors[0].work.top as f32 + top_extent;
        body.translate_world(Vec2::new(0.0, top_limit + 1.0 - body.center.y));
        body.velocity = Vec2::new(80.0, -900.0);
        body.step(TEST_DT, &env);

        assert!(body.velocity.y > 0.0);
        assert_eq!(body.mode, MotionMode::Flying);
    }

    #[test]
    fn closed_shake_gate_attaches_on_next_wall_without_reflection() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let (left_extent, _, _, _) = body.tumbler_extents();
        let left_limit = env.monitors[0].work.left as f32 + left_extent;
        body.translate_world(Vec2::new(left_limit + 1.0 - body.center.x, -180.0));
        body.velocity = Vec2::new(-1100.0, 120.0);

        body.apply_container_motion(Vec2::ZERO, Vec2::Y * 9.81, 0.0, true, true, false, false);
        body.step(TEST_DT, &env);

        assert_eq!(body.mode, MotionMode::Attached);
        assert_eq!(body.surface.expect("wall attachment").kind, SurfaceKind::Left);
        assert!(body.tumbler.is_none());
    }

    #[test]
    fn closed_shake_gate_remains_flying_until_an_actual_wall_contact() {
        let env = test_environment();
        let mut body = centered_tumbler(&env);
        body.velocity = Vec2::new(120.0, 0.0);
        body.apply_container_motion(Vec2::ZERO, Vec2::Y * 9.81, 0.0, true, true, false, false);

        body.step(TEST_DT, &env);

        assert_eq!(body.mode, MotionMode::Flying);
        assert!(body.surface.is_none());
        assert!(!body.tumbler.expect("closed gate in midair").shaking);
    }

    #[test]
    fn continuously_open_shake_gate_keeps_wall_reflection_active() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);

        for _ in 0..(60 * 3) {
            body.apply_container_motion(
                Vec2::new(0.12, -0.03),
                Vec2::Y * 9.81,
                0.55,
                true,
                true,
                false,
                true,
            );
            body.step(TEST_DT, &env);
            assert_eq!(body.mode, MotionMode::Flying);
            assert!(body.tumbler.is_some());
        }
        assert!(body.tumbler.expect("gate-held tumbler").shaking);
    }

    #[test]
    fn ordinary_flying_uses_phone_gravity_without_a_shake() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        let work = env.monitors[0].work;
        body.mode = MotionMode::Flying;
        body.surface = None;
        body.translate_world(
            Vec2::new(
                (work.left + work.right) as f32 * 0.5,
                (work.top + work.bottom) as f32 * 0.5,
            ) - body.center,
        );
        body.velocity = Vec2::ZERO;

        body.apply_container_motion(
            Vec2::ZERO,
            -Vec2::X * TUMBLER_STANDARD_GRAVITY,
            0.0,
            true,
            true,
            false,
            false,
        );
        body.step(TEST_DT, &env);

        assert!(body.velocity.x < -20.0, "vx={}", body.velocity.x);
        assert!(body.velocity.y.abs() < 0.1, "vy={}", body.velocity.y);
        assert!(body.tumbler.is_none());
    }

    #[test]
    fn unavailable_sensor_disables_container_motion_and_keeps_default_gravity() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        let work = env.monitors[0].work;
        body.mode = MotionMode::Flying;
        body.surface = None;
        body.translate_world(
            Vec2::new(
                (work.left + work.right) as f32 * 0.5,
                (work.top + work.bottom) as f32 * 0.5,
            ) - body.center,
        );
        body.velocity = Vec2::ZERO;

        body.apply_container_motion(
            Vec2::new(30.0, 0.0),
            -Vec2::X * TUMBLER_STANDARD_GRAVITY,
            1.0,
            false,
            false,
            true,
            true,
        );
        body.step(TEST_DT, &env);

        assert!(body.velocity.x.abs() < 0.1, "vx={}", body.velocity.x);
        assert!(body.velocity.y > 20.0, "vy={}", body.velocity.y);
        assert!(body.tumbler.is_none());
        assert!(body.sensor_gravity_world.is_none());
    }

    #[test]
    fn invalidated_gravity_reading_clears_the_previous_screen_axis() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        let work = env.monitors[0].work;
        body.mode = MotionMode::Flying;
        body.surface = None;
        body.translate_world(
            Vec2::new(
                (work.left + work.right) as f32 * 0.5,
                (work.top + work.bottom) as f32 * 0.5,
            ) - body.center,
        );
        body.velocity = Vec2::ZERO;
        body.apply_container_motion(
            Vec2::ZERO,
            -Vec2::X * TUMBLER_STANDARD_GRAVITY,
            0.0,
            true,
            true,
            false,
            false,
        );
        body.apply_container_motion(Vec2::ZERO, Vec2::ZERO, 0.0, true, false, false, false);

        body.step(TEST_DT, &env);

        assert!(body.velocity.x.abs() < 0.1, "vx={}", body.velocity.x);
        assert!(body.velocity.y > 20.0, "vy={}", body.velocity.y);
        assert!(body.sensor_gravity_world.is_none());
    }

    #[test]
    fn active_tumbler_falls_toward_the_phone_gravity_projection() {
        let env = test_environment();
        let base = centered_tumbler(&env);
        let mut left = base.clone();
        let mut right = base;

        for _ in 0..12 {
            left.apply_container_motion(
                Vec2::ZERO,
                -Vec2::X * TUMBLER_STANDARD_GRAVITY,
                0.0,
                true,
                true,
                false,
                true,
            );
            right.apply_container_motion(
                Vec2::ZERO,
                Vec2::X * TUMBLER_STANDARD_GRAVITY,
                0.0,
                true,
                true,
                false,
                true,
            );
            left.step(TEST_DT, &env);
            right.step(TEST_DT, &env);
        }

        assert!(left.velocity.x < -300.0, "left vx={}", left.velocity.x);
        assert!(right.velocity.x > 300.0, "right vx={}", right.velocity.x);
        assert!((left.velocity.x + right.velocity.x).abs() < 0.1);
        assert!(left.velocity.y.abs() < 0.1 && right.velocity.y.abs() < 0.1);
    }

    #[test]
    fn active_tumbler_preserves_projected_gravity_strength() {
        let env = test_environment();
        let base = centered_tumbler(&env);
        let mut full = base.clone();
        let mut half = base;
        let half_gravity = TUMBLER_STANDARD_GRAVITY * 0.5;

        for _ in 0..12 {
            full.apply_container_motion(
                Vec2::ZERO,
                Vec2::X * TUMBLER_STANDARD_GRAVITY,
                0.0,
                true,
                true,
                false,
                true,
            );
            half.apply_container_motion(Vec2::ZERO, Vec2::X * half_gravity, 0.0, true, true, false, true);
            full.step(TEST_DT, &env);
            half.step(TEST_DT, &env);
        }

        let expected_ratio = (half_gravity - TUMBLER_GRAVITY_DEAD_ZONE)
            / (TUMBLER_STANDARD_GRAVITY - TUMBLER_GRAVITY_DEAD_ZONE);
        let actual_ratio = half.velocity.x / full.velocity.x;
        assert!(
            (actual_ratio - expected_ratio).abs() < 0.01,
            "ratio={actual_ratio}"
        );
    }

    #[test]
    fn flat_phone_does_not_invent_a_screen_down_gravity() {
        let env = test_environment();
        let mut body = centered_tumbler(&env);
        let start = body.center;

        for _ in 0..12 {
            body.apply_container_motion(Vec2::ZERO, Vec2::ZERO, 0.0, true, true, false, true);
            body.step(TEST_DT, &env);
        }

        assert_eq!(body.velocity, Vec2::ZERO);
        assert!((body.center - start).length() < 0.001);
    }

    #[test]
    fn gravity_direction_updates_continuously_after_shaking_stops() {
        let env = test_environment();
        let mut body = centered_tumbler(&env);

        for _ in 0..6 {
            body.apply_container_motion(
                Vec2::ZERO,
                Vec2::X * TUMBLER_STANDARD_GRAVITY,
                0.0,
                true,
                true,
                false,
                false,
            );
            body.step(TEST_DT, &env);
        }
        assert!(body.velocity.x > 140.0);

        for _ in 0..12 {
            body.apply_container_motion(
                Vec2::ZERO,
                -Vec2::X * TUMBLER_STANDARD_GRAVITY,
                0.0,
                true,
                true,
                false,
                false,
            );
            body.step(TEST_DT, &env);
        }
        assert!(body.velocity.x < -140.0, "vx={}", body.velocity.x);
    }

    #[test]
    fn stored_gravity_does_not_expire_between_render_frames() {
        let env = test_environment();
        let mut body = centered_tumbler(&env);
        body.apply_container_motion(
            Vec2::ZERO,
            -Vec2::X * TUMBLER_STANDARD_GRAVITY,
            0.0,
            true,
            true,
            false,
            false,
        );

        for _ in 0..4 {
            body.step(0.035, &env);
        }

        assert!(body.velocity.x < -200.0, "vx={}", body.velocity.x);
        assert!(body.velocity.y.abs() < 0.1, "vy={}", body.velocity.y);
    }

    #[test]
    fn corner_settle_keeps_rotating_content_away_from_tangent_edges() {
        let env = test_environment();
        let work = env.monitors[0].work;
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let (left_extent, top_extent, _, _) = body.tumbler_extents();
        let target = Vec2::new(
            work.left as f32 + left_extent + 1.0,
            work.top as f32 + top_extent + 1.0,
        );
        body.translate_world(target - body.center);
        let diagonal_gravity = Vec2::new(-1.0, -1.0).normalized_or(-Vec2::Y) * TUMBLER_STANDARD_GRAVITY;
        body.apply_container_motion(Vec2::ZERO, diagonal_gravity, 0.0, true, true, false, false);
        body.velocity = Vec2::new(-80.0, -80.0);

        body.step(TEST_DT, &env);

        assert_eq!(
            body.mode,
            MotionMode::Attached,
            "velocity={:?} center={:?}",
            body.velocity,
            body.center
        );
        let contact = body.surface.expect("settled contact");
        body.velocity = Vec2::ZERO;
        body.idle_timer = 2.0;
        for _ in 0..16 {
            body.step(TEST_DT, &env);
            let bounds = body.content_bounds();
            match contact.kind {
                SurfaceKind::Bottom | SurfaceKind::Top => {
                    assert!(bounds.left >= work.left as f32 - 0.05, "left={}", bounds.left);
                    assert!(bounds.right <= work.right as f32 + 0.05, "right={}", bounds.right);
                }
                SurfaceKind::Left | SurfaceKind::Right => {
                    assert!(bounds.top >= work.top as f32 - 0.05, "top={}", bounds.top);
                    assert!(
                        bounds.bottom <= work.bottom as f32 + 0.05,
                        "bottom={}",
                        bounds.bottom
                    );
                }
            }
        }
    }

    #[test]
    fn repeated_trigger_adds_another_bounded_impulse() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        let velocity_before = body.velocity;

        trigger_tumbler(&mut body);

        assert_ne!(body.velocity, velocity_before);
        assert!(body.tumbler.expect("retriggered").shaking);
        assert_eq!(body.mode, MotionMode::Flying);
        assert!(body.velocity.length() <= MAX_THROW_VELOCITY + 0.001);
    }

    #[test]
    fn tumbler_eventually_settles_and_stays_inside_work_area() {
        let env = test_environment();
        let work = env.monitors[0].work;
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        body.apply_container_motion(
            Vec2::ZERO,
            Vec2::Y * TUMBLER_STANDARD_GRAVITY,
            0.0,
            true,
            true,
            false,
            false,
        );

        for _ in 0..(60 * 5) {
            body.step(TEST_DT, &env);
            assert!(body.center.x.is_finite() && body.center.y.is_finite());
            assert!(body.velocity.x.is_finite() && body.velocity.y.is_finite());
            assert!(body.bank.is_finite() && body.bank_velocity.is_finite());
            if body.tumbler.is_some() {
                let bounds = body.tumbler_collision_bounds();
                assert!(bounds.left >= work.left as f32 - 0.05, "left={}", bounds.left);
                assert!(bounds.top >= work.top as f32 - 0.05, "top={}", bounds.top);
                assert!(bounds.right <= work.right as f32 + 0.05, "right={}", bounds.right);
                assert!(
                    bounds.bottom <= work.bottom as f32 + 0.05,
                    "bottom={}",
                    bounds.bottom
                );
            }
            if body.mode == MotionMode::Attached {
                break;
            }
        }

        assert_eq!(
            body.mode,
            MotionMode::Attached,
            "velocity={:?} center={:?}",
            body.velocity,
            body.center
        );
        assert!(body.surface.is_some());
        assert!(body.tumbler.is_none());
    }

    #[test]
    fn flat_phone_bounce_also_settles_on_an_actual_wall() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        trigger_tumbler(&mut body);
        body.apply_container_motion(Vec2::ZERO, Vec2::ZERO, 0.0, true, true, false, false);

        for _ in 0..(60 * 5) {
            body.step(TEST_DT, &env);
            if body.mode == MotionMode::Attached {
                break;
            }
        }

        assert_eq!(
            body.mode,
            MotionMode::Attached,
            "velocity={:?} center={:?}",
            body.velocity,
            body.center
        );
        assert!(body.surface.is_some());
        assert!(body.tumbler.is_none());
    }

    #[test]
    fn invalid_and_extreme_container_input_remains_finite_and_bounded() {
        let env = test_environment();
        let mut body = FushiBody::new(&env);
        body.apply_container_motion(
            Vec2::new(f32::NAN, f32::INFINITY),
            Vec2::new(f32::NEG_INFINITY, f32::NAN),
            f32::NAN,
            true,
            true,
            true,
            true,
        );
        body.apply_container_motion(
            Vec2::new(f32::MAX, -f32::MAX),
            Vec2::new(f32::MAX, f32::MAX),
            1.0,
            true,
            true,
            true,
            true,
        );

        for _ in 0..180 {
            body.step(TEST_DT, &env);
            assert!(body.center.x.is_finite() && body.center.y.is_finite());
            assert!(body.velocity.x.is_finite() && body.velocity.y.is_finite());
            assert!(body.velocity.length() <= MAX_THROW_VELOCITY + 0.01);
            assert!(body.bank.is_finite() && body.bank_velocity.is_finite());
        }
    }
}
