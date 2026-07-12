use crate::math::{Color, Vec2};

pub const BODY_HALF_LENGTH: f32 = 158.0;
pub const BODY_HALF_HEIGHT: f32 = 54.0;
pub const BODY_CENTER_TO_BELLY: f32 = 51.5;
pub const BODY_NODE_COUNT: usize = 44;
pub const SMALL_FUSHI_SCALE: f32 = 0.48;
pub const NORMAL_FUSHI_SCALE: f32 = 0.62;
pub const LARGE_FUSHI_SCALE: f32 = 0.80;
pub const HUGE_FUSHI_SCALE: f32 = 0.98;
pub const BODY_OUTLINE_ROUGHNESS: f32 = 2.5;
pub const BELLY_AIR_CORNER_LIFT: f32 = 17.0;
pub const BELLY_CONTACT_CORNER_LIFT: f32 = 2.5;
pub const BELLY_CORNER_CONTACT_FADE: f32 = 0.38;

pub const DRAG_DETACH_LIFT: f32 = 17.0;
pub const AIRBORNE_BELLY_TUCK: f32 = 10.5;

pub const CRAWL_SPEED: f32 = 38.0;
pub const CRAWL_WIGGLE_AMPLITUDE: f32 = 3.8;
pub const INTERACTION_HIT_MARGIN: f32 = 7.0;
pub const CRAWL_STRETCH_SPEED_START: f32 = 6.0;
pub const CRAWL_STRETCH_SPEED_FULL: f32 = CRAWL_SPEED * 0.72;
pub const CRAWL_STRETCH_FREQUENCY: f32 = 6.0;
pub const CRAWL_REST_PHASE: f32 = -1.5707964;
pub const CRAWL_PHASE_SETTLE_RATE: f32 = 8.0;
pub const CRAWL_REACH_EXPONENT: f32 = 1.35;
pub const CRAWL_GATHER_EXPONENT: f32 = 1.18;
pub const CRAWL_FORWARD_STRETCH: f32 = 42.0;
pub const CRAWL_GATHER_SQUASH: f32 = 0.045;
pub const CRAWL_GATHER_BELLY_EXPAND: f32 = 0.62;
pub const CRAWL_DRIVE_RESPONSE: f32 = 6.0;
pub const ATTACHED_VELOCITY_RESPONSE: f32 = 5.5;

pub const WINDOW_SHAKE_DROP_SPEED: f32 = 1180.0;
pub const WINDOW_SHAKE_SLIDE_DROP_SPEED: f32 = 1020.0;
pub const TURN_DURATION: f32 = 0.76;
pub const TURN_MIN_YAW_SCALE: f32 = 0.56;

pub const FREE_GRAVITY: f32 = 1550.0;
pub const THROW_VELOCITY_SCALE: f32 = 0.70;
pub const MAX_THROW_VELOCITY: f32 = 2050.0;

pub const DRAG_CURSOR_FOLLOW_RESPONSE: f32 = 9.6;
pub const DRAG_CURSOR_MAX_LAG: f32 = 112.0;
pub const DRAG_ANCHOR_STIFFNESS: f32 = 15.8;
pub const DRAG_ANCHOR_DAMPING: f32 = 7.8;
pub const DRAG_MAX_ANCHOR_ERROR: f32 = 124.0;
pub const DRAG_NODE_PULL_LIMIT: f32 = 62.0;
pub const DRAG_GRAB_RADIUS: f32 = 150.0;
pub const DRAG_SHAPE_GUARD_SPIN_START: f32 = 8.5;
pub const DRAG_SHAPE_GUARD_SPIN_FULL: f32 = 19.0;
pub const DRAG_SHAPE_GUARD_PULL_START: f32 = 72.0;
pub const DRAG_SHAPE_GUARD_PULL_FULL: f32 = 154.0;
pub const DRAG_SHAPE_GUARD_SPEED_START: f32 = 1250.0;
pub const DRAG_SHAPE_GUARD_SPEED_FULL: f32 = 3100.0;
pub const DRAG_SHAPE_GUARD_CLOSE_RADIUS_START: f32 = 122.0;
pub const DRAG_SHAPE_GUARD_CLOSE_RADIUS_FULL: f32 = 38.0;
pub const DRAG_END_COMPRESSION_START: f32 = 10.0;
pub const DRAG_END_COMPRESSION_FULL: f32 = 78.0;
pub const DRAG_END_COMPRESSION_RELIEF: f32 = 0.82;
pub const DRAG_TAIL_HANG_GRAVITY_SCALE: f32 = 0.34;
pub const DRAG_TAIL_HANG_ROTATION_RESPONSE: f32 = 8.5;
pub const DRAG_TAIL_HANG_SAG: f32 = 30.0;
pub const DRAG_STRESS_GAIN: f32 = 0.000022;
pub const DRAG_BANK_TORQUE: f32 = 0.00028;
pub const DRAG_ROTATION_RESPONSE: f32 = 3.6;
pub const DRAG_ORBIT_SPIN_GAIN: f32 = 2.15;
pub const DRAG_PIN_SPIN_RESPONSE: f32 = 9.5;
pub const DRAG_PIN_SPIN_MAX: f32 = 17.0;
pub const DRAG_SPIN_DAMPING: f32 = 1.85;
pub const DRAG_MESH_STIFFNESS_SCALE: f32 = 0.28;
pub const DRAG_MESH_GUARD_STIFFNESS_SCALE: f32 = 0.52;
pub const DRAG_MESH_DAMPING_SCALE: f32 = 0.88;
pub const DRAG_NEIGHBOR_STIFFNESS_SCALE: f32 = 0.32;
pub const DRAG_NEIGHBOR_GUARD_STIFFNESS_SCALE: f32 = 0.58;
pub const DRAG_SOFT_SHEAR: f32 = 0.28;

pub const MAX_BANK_RADIANS: f32 = 0.54;
pub const BANK_RESTORE: f32 = 5.0;
pub const BANK_DAMPING: f32 = 5.7;

pub const MESH_STIFFNESS: f32 = 32.0;
pub const MESH_DAMPING: f32 = 8.6;
pub const MESH_NEIGHBOR_STIFFNESS: f32 = 0.36;
pub const MESH_CONSTRAINT_ITERATIONS: usize = 4;
pub const MESH_SHAPE_MAX_DEVIATION: f32 = 25.0;
pub const MESH_DRAG_SHAPE_MAX_DEVIATION: f32 = 82.0;
pub const MESH_DRAG_GUARD_SHAPE_MAX_DEVIATION: f32 = 48.0;
pub const BELLY_CONTACT_STRENGTH: f32 = 0.98;
pub const SURFACE_TRANSITION_DURATION: f32 = 0.52;

pub const HOVER_RANGE: f32 = 40.0;
pub const RENDER_WINDOW_MARGIN: f32 = 156.0;
pub const TAIL_GRAB_RADIUS: f32 = 76.0;
pub const EAR_GRAB_RADIUS: f32 = 32.0;

pub const BODY_FILL: Color = Color::rgba_u8(255, 253, 247, 255);
pub const BODY_STROKE: Color = Color::rgba_u8(176, 171, 155, 230);
pub const BODY_SHADOW: Color = Color::rgba_u8(214, 201, 191, 232);
pub const BODY_DEEP_SHADOW: Color = Color::rgba_u8(184, 170, 161, 224);
pub const DARK: Color = Color::rgba_u8(38, 37, 47, 255);
pub const DARK_SHADOW: Color = Color::rgba_u8(10, 10, 18, 210);
pub const EAR_INNER_FILL: Color = Color::rgba_u8(232, 225, 216, 255);
pub const SPOT: Color = Color::rgba_u8(118, 122, 138, 118);
pub const SPOT_RADIUS_SCALE: f32 = 0.58;
pub const ANGER_RED: Color = Color::rgba_u8(210, 62, 58, 235);
pub const TEAR_BLUE: Color = Color::rgba_u8(99, 164, 238, 205);
pub const WHITE_HILIGHT: Color = Color::rgba_u8(255, 255, 255, 245);
pub const BLUSH_PINK: Color = Color::rgba_u8(255, 148, 154, 210);
pub const BLUSH_LINE: Color = Color::rgba_u8(210, 82, 96, 180);
pub const MOUTH_INNER: Color = Color::rgba_u8(246, 180, 174, 255);

pub const EAR_MID_TONE: Color = Color::rgba_u8(95, 89, 99, 255);
pub const EAR_SIDE_SHADOW: Color = Color::rgba_u8(62, 61, 67, 255);
pub const EAR_DEEP_EDGE: Color = Color::rgba_u8(34, 34, 35, 255);
pub const EAR_INNER_SHADOW: Color = Color::rgba_u8(216, 207, 198, 255);
pub const TAIL_DARK_MAIN: Color = Color::rgba_u8(62, 61, 67, 255);
pub const TAIL_DARK_LIT: Color = Color::rgba_u8(95, 89, 99, 255);
pub const TAIL_DARK_SHADE: Color = Color::rgba_u8(34, 34, 35, 255);
pub const TAIL_LIGHT_FILL: Color = Color::rgba_u8(232, 225, 216, 255);
pub const TAIL_LIGHT_SHADE: Color = Color::rgba_u8(216, 207, 198, 255);

pub const FACE_LEFT_EYE: Vec2 = Vec2::new(-148.0, 7.0);
pub const FACE_RIGHT_EYE: Vec2 = Vec2::new(-111.0, 10.5);
pub const FACE_OPEN_EYE_RX: f32 = 11.2;
pub const FACE_OPEN_EYE_RY: f32 = 13.0;
pub const FACE_MOUTH_LEFT: Vec2 = Vec2::new(-147.0, 27.0);
pub const FACE_MOUTH_MID: Vec2 = Vec2::new(-135.5, 29.0);
pub const FACE_MOUTH_RIGHT: Vec2 = Vec2::new(-124.0, 27.0);

#[derive(Clone, Copy, Debug)]
pub struct SpotSpec {
    pub local: Vec2,
    pub radius: f32,
    pub stretch: f32,
}

impl SpotSpec {
    pub const fn new(local: Vec2, radius: f32, stretch: f32) -> Self {
        Self {
            local,
            radius,
            stretch,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EarSpec {
    pub anchor: Vec2,
    pub lean_degrees: f32,
    pub scale: f32,
}

impl EarSpec {
    pub const fn new(anchor: Vec2, lean_degrees: f32, scale: f32) -> Self {
        Self {
            anchor,
            lean_degrees,
            scale,
        }
    }
}

pub const SPOTS: [SpotSpec; 12] = [
    SpotSpec::new(Vec2::new(-128.0, -25.0), 7.4, 1.02),
    SpotSpec::new(Vec2::new(-92.0, -3.0), 8.8, 1.00),
    SpotSpec::new(Vec2::new(-54.0, 28.0), 9.8, 1.02),
    SpotSpec::new(Vec2::new(-31.0, -19.0), 7.4, 1.00),
    SpotSpec::new(Vec2::new(8.0, 12.0), 6.3, 1.00),
    SpotSpec::new(Vec2::new(35.0, -28.0), 8.9, 1.00),
    SpotSpec::new(Vec2::new(60.0, -4.0), 7.8, 1.00),
    SpotSpec::new(Vec2::new(89.0, 28.0), 9.5, 1.00),
    SpotSpec::new(Vec2::new(118.0, -25.0), 7.2, 1.00),
    SpotSpec::new(Vec2::new(134.0, 6.0), 7.8, 1.00),
    SpotSpec::new(Vec2::new(-15.0, 49.0), 7.3, 1.00),
    SpotSpec::new(Vec2::new(64.0, 43.0), 8.7, 1.00),
];

pub const EARS: [EarSpec; 2] = [
    EarSpec::new(Vec2::new(-112.0, -32.0), -23.0, 1.16),
    EarSpec::new(Vec2::new(-78.0, -26.5), 18.0, 1.20),
];

pub const EAR_TIP_LENGTH: f32 = 64.0;
pub const EAR_LEAF_HALF_WIDTH: f32 = 17.0;

pub const TAIL_ANCHOR: Vec2 = Vec2::new(100.0, -48.0);
