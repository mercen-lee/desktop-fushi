use std::collections::VecDeque;

use crate::math::Vec2;

const WARMUP_NS: i64 = 300_000_000;
const DISCONTINUITY_NS: i64 = 250_000_000;
const GATE_QUIET_NS: i64 = 250_000_000;
const PEAK_WINDOW_NS: i64 = 400_000_000;
const MIN_PEAK_SEPARATION_NS: i64 = 30_000_000;
const MIN_GESTURE_SPAN_NS: i64 = 110_000_000;
const BIAS_TIME_CONSTANT: f32 = 2.0;
const WARMUP_BIAS_TIME_CONSTANT: f32 = 0.15;
const RAW_GRAVITY_TIME_CONSTANT: f32 = 0.35;
const DIRECT_GRAVITY_TIME_CONSTANT: f32 = 0.10;
const DEAD_ZONE: f32 = 1.5;
const PEAK_THRESHOLD: f32 = 7.0;
// Once triggered, only another deliberate-shake-strength sample may extend the gate. Ordinary
// handling can easily reach 3-5 m/s² briefly and must not keep the tumbler state alive.
const GATE_KEEP_THRESHOLD: f32 = PEAK_THRESHOLD;
const OPPOSING_DOT_THRESHOLD: f32 = -0.35;
const MAX_SENSOR_MAGNITUDE: f32 = 48.0;
const MAX_FRAME_IMPULSE: f32 = 12.0;
const MAX_PRETRIGGER_SAMPLES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SensorKind {
    LinearAcceleration,
    Gravity,
    Accelerometer,
}

impl SensorKind {
    fn index(self) -> usize {
        match self {
            Self::LinearAcceleration => 0,
            Self::Gravity => 1,
            Self::Accelerometer => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SensorAvailability {
    linear_acceleration: bool,
    gravity: bool,
    accelerometer: bool,
}

impl SensorAvailability {
    pub(crate) const fn new(linear_acceleration: bool, gravity: bool, accelerometer: bool) -> Self {
        Self {
            linear_acceleration,
            gravity,
            accelerometer,
        }
    }

    fn supports(self, kind: SensorKind) -> bool {
        match kind {
            SensorKind::LinearAcceleration => self.linear_acceleration,
            SensorKind::Gravity => self.gravity,
            SensorKind::Accelerometer => self.accelerometer,
        }
    }

    pub(crate) const fn none() -> Self {
        Self::new(false, false, false)
    }

    pub(crate) const fn direct_pair() -> Self {
        Self::new(true, true, false)
    }

    pub(crate) const fn raw_accelerometer() -> Self {
        Self::new(false, false, true)
    }

    fn any(self) -> bool {
        self.linear_acceleration || self.gravity || self.accelerometer
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameMotion {
    /// Apparent change in content velocity accumulated since the previous frame (m/s).
    pub impulse: Vec2,
    /// Apparent gravity projected into screen coordinates (+X right, +Y down), in m/s².
    pub gravity: Vec2,
    /// Whether Java successfully registered a coherent direct pair or raw fallback sensor.
    pub sensor_available: bool,
    /// Distinguishes a valid flat-phone zero projection from a sensor that has no reading yet.
    pub gravity_valid: bool,
    pub intensity: f32,
    pub gate_open: bool,
    pub triggered: bool,
}

impl Default for FrameMotion {
    fn default() -> Self {
        Self {
            impulse: Vec2::ZERO,
            gravity: Vec2::ZERO,
            sensor_available: false,
            gravity_valid: false,
            intensity: 0.0,
            gate_open: false,
            triggered: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn normalized(self) -> Self {
        let length = self.length();
        if length > 0.0001 {
            self * (1.0 / length)
        } else {
            Self::ZERO
        }
    }

    fn clamp_len(self, maximum: f32) -> Self {
        let length = self.length();
        if length > maximum && length > 0.0001 {
            self * (maximum / length)
        } else {
            self
        }
    }

    fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

#[derive(Clone, Copy, Debug)]
struct Peak {
    timestamp_ns: i64,
    direction: Vec3,
    magnitude: f32,
}

#[derive(Clone, Copy, Debug)]
struct BufferedImpulse {
    timestamp_ns: i64,
    impulse: Vec2,
}

pub(crate) struct MotionInput {
    availability: SensorAvailability,
    last_rotation: Option<u8>,
    latest_timestamp_ns: Option<i64>,
    last_kind_timestamp_ns: [Option<i64>; 3],
    warmup_started_ns: Option<i64>,
    linear_bias: Vec3,
    bias_initialized: bool,
    raw_gravity: Vec3,
    raw_gravity_initialized: bool,
    latest_gravity: Vec2,
    direct_linear_seen: bool,
    direct_gravity_seen: bool,
    peaks: VecDeque<Peak>,
    pretrigger_impulses: VecDeque<BufferedImpulse>,
    gate_open: bool,
    last_gate_activity_ns: Option<i64>,
    gate_activity_since_frame: bool,
    frame_quiet_seconds: f32,
    mailbox_impulse: Vec2,
    mailbox_intensity: f32,
    mailbox_triggered: bool,
}

impl MotionInput {
    pub(crate) fn new(availability: SensorAvailability) -> Self {
        Self {
            availability,
            last_rotation: None,
            latest_timestamp_ns: None,
            last_kind_timestamp_ns: [None; 3],
            warmup_started_ns: None,
            linear_bias: Vec3::ZERO,
            bias_initialized: false,
            raw_gravity: Vec3::ZERO,
            raw_gravity_initialized: false,
            latest_gravity: Vec2::ZERO,
            direct_linear_seen: false,
            direct_gravity_seen: false,
            peaks: VecDeque::with_capacity(4),
            pretrigger_impulses: VecDeque::with_capacity(24),
            gate_open: false,
            last_gate_activity_ns: None,
            gate_activity_since_frame: false,
            frame_quiet_seconds: 0.0,
            mailbox_impulse: Vec2::ZERO,
            mailbox_intensity: 0.0,
            mailbox_triggered: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        let availability = self.availability;
        *self = Self::new(availability);
    }

    pub(crate) fn set_availability(&mut self, availability: SensorAvailability) {
        if self.availability != availability {
            *self = Self::new(availability);
        }
    }

    pub(crate) fn push_sample(
        &mut self,
        kind: SensorKind,
        timestamp_ns: i64,
        rotation: u8,
        values: [f32; 3],
    ) -> bool {
        if !self.availability.supports(kind) || timestamp_ns < 0 || rotation > 3 {
            return false;
        }
        let raw = Vec3::new(values[0], values[1], values[2]);
        if !raw.is_finite() {
            return false;
        }

        let Some(dt) = self.prepare_timestamp(kind, timestamp_ns, rotation) else {
            return false;
        };
        let apparent = apparent_screen_vector(raw, rotation).clamp_len(MAX_SENSOR_MAGNITUDE);

        match kind {
            SensorKind::LinearAcceleration => {
                self.direct_linear_seen = true;
                self.process_linear(apparent, timestamp_ns, dt);
            }
            SensorKind::Gravity => {
                let gravity = apparent.xy().clamp_len(14.0);
                if self.direct_gravity_seen {
                    let alpha = time_response(DIRECT_GRAVITY_TIME_CONSTANT, dt);
                    self.latest_gravity += (gravity - self.latest_gravity) * alpha;
                } else {
                    // Do not blend the first real reading with a synthetic screen-down value.
                    // The detector has a 300 ms warm-up, so gravity is initialized well before
                    // an intentional shake can open the physics gate.
                    self.latest_gravity = gravity;
                    self.direct_gravity_seen = true;
                }
            }
            SensorKind::Accelerometer => self.process_accelerometer(apparent, timestamp_ns, dt),
        }
        true
    }

    pub(crate) fn take_frame(&mut self) -> FrameMotion {
        let frame = FrameMotion {
            impulse: self.mailbox_impulse,
            gravity: self.latest_gravity,
            sensor_available: self.availability.any(),
            gravity_valid: self.direct_gravity_seen || self.raw_gravity_initialized,
            intensity: self.mailbox_intensity.clamp(0.0, 1.0),
            gate_open: self.gate_open,
            triggered: self.mailbox_triggered,
        };
        self.mailbox_impulse = Vec2::ZERO;
        self.mailbox_intensity = 0.0;
        self.mailbox_triggered = false;
        frame
    }

    /// Advances the gate even if a sensor driver temporarily stops delivering callbacks while
    /// Choreographer continues. Normal sensor timestamps remain the primary clock.
    pub(crate) fn advance_frame(&mut self, dt: f32) {
        if !self.gate_open {
            self.gate_activity_since_frame = false;
            self.frame_quiet_seconds = 0.0;
            return;
        }
        if self.gate_activity_since_frame {
            self.frame_quiet_seconds = 0.0;
        } else if dt.is_finite() {
            self.frame_quiet_seconds = (self.frame_quiet_seconds + dt.clamp(0.0, 0.12)).min(0.5);
        }
        self.gate_activity_since_frame = false;
        if self.frame_quiet_seconds >= GATE_QUIET_NS as f32 * 1.0e-9 {
            self.close_gate();
        }
    }

    fn prepare_timestamp(&mut self, kind: SensorKind, timestamp_ns: i64, rotation: u8) -> Option<f32> {
        let rotation_changed = self
            .last_rotation
            .map(|previous| previous != rotation)
            .unwrap_or(false);
        let discontinuous = self
            .latest_timestamp_ns
            .map(|latest| timestamp_ns > latest.saturating_add(DISCONTINUITY_NS))
            .unwrap_or(false);
        let kind_index = kind.index();
        let kind_reversed = self.last_kind_timestamp_ns[kind_index]
            .map(|previous| timestamp_ns < previous)
            .unwrap_or(false);
        if rotation_changed || discontinuous || kind_reversed {
            self.reset();
        }

        // Gravity and linear-acceleration callbacks may legitimately share a timestamp. Only a
        // duplicate from the same stream is ignored; cross-stream equality never resets warm-up.
        if self.last_kind_timestamp_ns[kind_index] == Some(timestamp_ns) {
            return None;
        }

        let previous_kind_timestamp = self.last_kind_timestamp_ns[kind_index];
        self.last_kind_timestamp_ns[kind_index] = Some(timestamp_ns);
        self.latest_timestamp_ns = Some(
            self.latest_timestamp_ns
                .map(|latest| latest.max(timestamp_ns))
                .unwrap_or(timestamp_ns),
        );
        self.last_rotation = Some(rotation);
        self.warmup_started_ns.get_or_insert(timestamp_ns);
        self.expire_gate(timestamp_ns);

        Some(
            previous_kind_timestamp
                .map(|previous| ((timestamp_ns - previous) as f32 * 1.0e-9).clamp(0.001, 0.060))
                .unwrap_or(0.020),
        )
    }

    fn process_accelerometer(&mut self, apparent: Vec3, timestamp_ns: i64, dt: f32) {
        if !self.raw_gravity_initialized {
            self.raw_gravity = apparent;
            self.raw_gravity_initialized = true;
        } else if !self.direct_gravity_seen {
            let alpha = time_response(RAW_GRAVITY_TIME_CONSTANT, dt);
            self.raw_gravity = self.raw_gravity + (apparent - self.raw_gravity) * alpha;
        }

        if !self.direct_gravity_seen {
            self.latest_gravity = self.raw_gravity.xy().clamp_len(14.0);
        }
        if !self.direct_linear_seen {
            let gravity = if self.direct_gravity_seen {
                Vec3::new(self.latest_gravity.x, self.latest_gravity.y, self.raw_gravity.z)
            } else {
                self.raw_gravity
            };
            self.process_linear(apparent - gravity, timestamp_ns, dt);
        }
    }

    fn process_linear(&mut self, linear: Vec3, timestamp_ns: i64, dt: f32) {
        if !self.bias_initialized {
            self.linear_bias = linear;
            self.bias_initialized = true;
        }

        let warming_up =
            timestamp_ns.saturating_sub(self.warmup_started_ns.unwrap_or(timestamp_ns)) < WARMUP_NS;
        if warming_up {
            let alpha = time_response(WARMUP_BIAS_TIME_CONSTANT, dt);
            self.linear_bias = self.linear_bias + (linear - self.linear_bias) * alpha;
            self.peaks.clear();
            self.pretrigger_impulses.clear();
            return;
        }

        let mut centered = linear - self.linear_bias;
        let centered_magnitude = centered.length();
        if !self.gate_open && centered_magnitude < GATE_KEEP_THRESHOLD {
            let alpha = time_response(BIAS_TIME_CONSTANT, dt);
            self.linear_bias = self.linear_bias + (linear - self.linear_bias) * alpha;
            centered = linear - self.linear_bias;
        }

        let magnitude = centered.length();
        let filtered = apply_dead_zone(centered, DEAD_ZONE);
        let planar_impulse = filtered.xy() * dt;

        if self.gate_open && magnitude >= GATE_KEEP_THRESHOLD {
            self.last_gate_activity_ns = Some(timestamp_ns);
            self.gate_activity_since_frame = true;
        }

        let trigger_magnitude = self.observe_peak(centered, magnitude, timestamp_ns);
        if !self.gate_open {
            self.buffer_pretrigger(timestamp_ns, planar_impulse);
        }

        if let Some(trigger_magnitude) = trigger_magnitude {
            let trigger_intensity = intensity_for_magnitude(trigger_magnitude);
            self.gate_open = true;
            self.last_gate_activity_ns = Some(timestamp_ns);
            self.gate_activity_since_frame = true;
            self.frame_quiet_seconds = 0.0;
            self.mailbox_triggered = true;
            self.mailbox_intensity = self.mailbox_intensity.max(trigger_intensity);
            let buffered = self.take_pretrigger_impulse(filtered.xy());
            self.add_mailbox_impulse(buffered);
        } else if self.gate_open {
            self.add_mailbox_impulse(planar_impulse);
            self.mailbox_intensity = self.mailbox_intensity.max(intensity_for_magnitude(magnitude));
        }
    }

    fn observe_peak(&mut self, centered: Vec3, magnitude: f32, timestamp_ns: i64) -> Option<f32> {
        while self
            .peaks
            .front()
            .map(|peak| timestamp_ns.saturating_sub(peak.timestamp_ns) > PEAK_WINDOW_NS)
            .unwrap_or(false)
        {
            self.peaks.pop_front();
        }
        if magnitude < PEAK_THRESHOLD {
            return None;
        }

        let candidate = Peak {
            timestamp_ns,
            direction: centered.normalized(),
            magnitude,
        };
        if let Some(previous) = self.peaks.back_mut() {
            let separation = timestamp_ns.saturating_sub(previous.timestamp_ns);
            let dot = previous.direction.dot(candidate.direction);
            if dot > OPPOSING_DOT_THRESHOLD {
                if candidate.magnitude > previous.magnitude {
                    *previous = candidate;
                }
                return None;
            }
            if separation < MIN_PEAK_SEPARATION_NS {
                return None;
            }
        }
        self.peaks.push_back(candidate);

        if self.peaks.len() < 3 {
            return None;
        }
        let first = self.peaks[self.peaks.len() - 3];
        let second = self.peaks[self.peaks.len() - 2];
        let third = self.peaks[self.peaks.len() - 1];
        let gesture_span = third.timestamp_ns.saturating_sub(first.timestamp_ns);
        let within_window = (MIN_GESTURE_SPAN_NS..=PEAK_WINDOW_NS).contains(&gesture_span);
        let alternating = first.direction.dot(second.direction) <= OPPOSING_DOT_THRESHOLD
            && second.direction.dot(third.direction) <= OPPOSING_DOT_THRESHOLD;
        if within_window && alternating {
            let trigger_magnitude = first.magnitude.max(second.magnitude).max(third.magnitude);
            self.peaks.clear();
            Some(trigger_magnitude)
        } else {
            None
        }
    }

    fn buffer_pretrigger(&mut self, timestamp_ns: i64, impulse: Vec2) {
        self.pretrigger_impulses.push_back(BufferedImpulse {
            timestamp_ns,
            impulse,
        });
        while self.pretrigger_impulses.len() > MAX_PRETRIGGER_SAMPLES {
            self.pretrigger_impulses.pop_front();
        }
        while self
            .pretrigger_impulses
            .front()
            .map(|sample| timestamp_ns.saturating_sub(sample.timestamp_ns) > PEAK_WINDOW_NS)
            .unwrap_or(false)
        {
            self.pretrigger_impulses.pop_front();
        }
    }

    fn take_pretrigger_impulse(&mut self, latest_direction: Vec2) -> Vec2 {
        let mut total = Vec2::ZERO;
        let mut effort = 0.0;
        for sample in self.pretrigger_impulses.drain(..) {
            total += sample.impulse;
            effort += sample.impulse.length();
        }
        if total.length() < effort * 0.18 && latest_direction.length() > 0.001 {
            total = latest_direction.normalized_or(Vec2::X) * (effort * 0.35);
        }
        total.clamp_len(MAX_FRAME_IMPULSE)
    }

    fn add_mailbox_impulse(&mut self, impulse: Vec2) {
        if impulse.x.is_finite() && impulse.y.is_finite() {
            self.mailbox_impulse = (self.mailbox_impulse + impulse).clamp_len(MAX_FRAME_IMPULSE);
        }
    }

    fn expire_gate(&mut self, timestamp_ns: i64) {
        if self.gate_open
            && self
                .last_gate_activity_ns
                .map(|last| timestamp_ns.saturating_sub(last) >= GATE_QUIET_NS)
                .unwrap_or(true)
        {
            self.close_gate();
        }
    }

    fn close_gate(&mut self) {
        self.gate_open = false;
        self.last_gate_activity_ns = None;
        self.gate_activity_since_frame = false;
        self.frame_quiet_seconds = 0.0;
        self.peaks.clear();
        self.pretrigger_impulses.clear();
    }
}

fn time_response(time_constant: f32, dt: f32) -> f32 {
    1.0 - (-dt.max(0.0) / time_constant.max(0.001)).exp()
}

fn apply_dead_zone(value: Vec3, dead_zone: f32) -> Vec3 {
    let magnitude = value.length();
    if magnitude <= dead_zone || magnitude <= 0.0001 {
        Vec3::ZERO
    } else {
        value * ((magnitude - dead_zone) / magnitude)
    }
}

fn intensity_for_magnitude(magnitude: f32) -> f32 {
    if !magnitude.is_finite() || magnitude < GATE_KEEP_THRESHOLD {
        0.0
    } else {
        ((magnitude - PEAK_THRESHOLD) / 11.0)
            .clamp(0.0, 1.0)
            .mul_add(0.55, 0.45)
    }
}

/// Converts Android's device-natural sensor axes into apparent acceleration of contents in
/// current screen coordinates. Android reports proper acceleration/gravity, so the result is
/// negated after rotation. Screen coordinates use +X right and +Y down.
fn apparent_screen_vector(value: Vec3, rotation: u8) -> Vec3 {
    match rotation {
        1 => Vec3::new(value.y, value.x, -value.z),
        2 => Vec3::new(value.x, -value.y, -value.z),
        3 => Vec3::new(-value.y, -value.x, -value.z),
        _ => Vec3::new(-value.x, value.y, -value.z),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_NS: i64 = 20_000_000;

    fn direct_input() -> MotionInput {
        MotionInput::new(SensorAvailability::new(true, true, true))
    }

    fn warm_up(input: &mut MotionInput, start_ns: i64) -> i64 {
        let mut timestamp = start_ns;
        for _ in 0..20 {
            assert!(input.push_sample(SensorKind::Gravity, timestamp, 0, [0.0, 9.81, 0.0],));
            assert!(input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [0.0, 0.0, 0.0],));
            timestamp += SAMPLE_NS;
        }
        input.take_frame();
        timestamp
    }

    #[test]
    fn remaps_all_display_rotations_to_apparent_screen_axes() {
        let value = Vec3::new(2.0, 3.0, 4.0);
        let expected = [
            Vec3::new(-2.0, 3.0, -4.0),
            Vec3::new(3.0, 2.0, -4.0),
            Vec3::new(2.0, -3.0, -4.0),
            Vec3::new(-3.0, -2.0, -4.0),
        ];
        for (rotation, expected) in expected.into_iter().enumerate() {
            let actual = apparent_screen_vector(value, rotation as u8);
            assert!((actual.x - expected.x).abs() < 0.0001);
            assert!((actual.y - expected.y).abs() < 0.0001);
            assert!((actual.z - expected.z).abs() < 0.0001);
        }
    }

    #[test]
    fn physical_screen_down_gravity_is_consistent_in_every_display_rotation() {
        const G: f32 = 9.80665;
        let raw_screen_down = [
            Vec3::new(0.0, G, 0.0),
            Vec3::new(G, 0.0, 0.0),
            Vec3::new(0.0, -G, 0.0),
            Vec3::new(-G, 0.0, 0.0),
        ];

        for (rotation, raw) in raw_screen_down.into_iter().enumerate() {
            let actual = apparent_screen_vector(raw, rotation as u8);
            assert!(actual.x.abs() < 0.0001, "rotation={rotation} x={}", actual.x);
            assert!(
                (actual.y - G).abs() < 0.0001,
                "rotation={rotation} y={}",
                actual.y
            );
        }
    }

    #[test]
    fn direct_gravity_is_smoothed_without_becoming_a_shake_impulse() {
        const G: f32 = 9.80665;
        let mut input = direct_input();
        assert!(input.push_sample(SensorKind::Gravity, 0, 0, [0.0, G, 0.0]));
        let initial = input.take_frame();
        assert!((initial.gravity.y - G).abs() < 0.0001);
        assert_eq!(initial.impulse, Vec2::ZERO);

        // A sudden orientation estimate is blended over a short interval. Gravity remains a
        // separate continuous signal and can never manufacture a detector impulse.
        assert!(input.push_sample(SensorKind::Gravity, SAMPLE_NS, 0, [-G, 0.0, 0.0]));
        let changed = input.take_frame();
        assert!(changed.gravity.x > 0.0 && changed.gravity.x < G);
        assert!(changed.gravity.y > 0.0 && changed.gravity.y < G);
        assert_eq!(changed.impulse, Vec2::ZERO);
        assert!(!changed.triggered);
    }

    #[test]
    fn unavailable_sensor_mode_is_fully_inert() {
        let mut input = MotionInput::new(SensorAvailability::none());
        assert!(!input.push_sample(SensorKind::Gravity, 0, 0, [0.0, 9.81, 0.0]));
        assert!(!input.push_sample(SensorKind::LinearAcceleration, 0, 0, [20.0, 0.0, 0.0]));
        assert!(!input.push_sample(SensorKind::Accelerometer, 0, 0, [0.0, 9.81, 0.0]));

        let frame = input.take_frame();
        assert!(!frame.sensor_available);
        assert!(!frame.gravity_valid);
        assert!(!frame.triggered && !frame.gate_open);
        assert_eq!(frame.impulse, Vec2::ZERO);
        assert_eq!(frame.gravity, Vec2::ZERO);
    }

    #[test]
    fn configured_sensor_modes_accept_only_their_coherent_streams() {
        let mut input = MotionInput::new(SensorAvailability::direct_pair());
        assert!(input.take_frame().sensor_available);
        assert!(!input.take_frame().gravity_valid);
        assert!(!input.push_sample(SensorKind::Accelerometer, 0, 0, [0.0, 9.81, 0.0]));
        assert!(input.push_sample(SensorKind::Gravity, 0, 0, [0.0, 9.81, 0.0]));
        assert!(input.take_frame().gravity_valid);

        input.set_availability(SensorAvailability::none());
        let disabled = input.take_frame();
        assert!(!disabled.sensor_available && !disabled.gravity_valid);
        assert_eq!(disabled.gravity, Vec2::ZERO);

        input.set_availability(SensorAvailability::raw_accelerometer());
        assert!(!input.push_sample(SensorKind::Gravity, 0, 0, [0.0, 9.81, 0.0]));
        assert!(input.push_sample(SensorKind::Accelerometer, 0, 0, [0.0, 9.81, 0.0]));
        let raw = input.take_frame();
        assert!(raw.sensor_available && raw.gravity_valid);
        assert!((raw.gravity.y - 9.81).abs() < 0.01);
    }

    #[test]
    fn sixty_seconds_of_bias_and_hand_tremor_never_triggers() {
        let mut input = direct_input();
        let mut timestamp = 0;
        for frame in 0..(60 * 50) {
            let phase = frame as f32 * 0.17;
            let sample = [0.55 + phase.sin() * 0.62, phase.cos() * 0.48, phase.sin() * 0.31];
            assert!(input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, sample,));
            let motion = input.take_frame();
            assert!(!motion.triggered);
            assert_eq!(motion.impulse, Vec2::ZERO);
            timestamp += SAMPLE_NS;
        }
        assert!(!input.take_frame().gate_open);
    }

    #[test]
    fn one_large_bump_does_not_trigger() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [-12.0, 0.0, 0.0]);
        timestamp += SAMPLE_NS;
        for _ in 0..30 {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [0.0, 0.0, 0.0]);
            timestamp += SAMPLE_NS;
        }
        let motion = input.take_frame();
        assert!(!motion.triggered);
        assert!(!motion.gate_open);
        assert_eq!(motion.impulse, Vec2::ZERO);
    }

    #[test]
    fn short_impact_ringing_does_not_look_like_a_deliberate_shake() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for x in [-16.0, 15.0, -13.0] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 40_000_000;
        }
        let motion = input.take_frame();
        assert!(!motion.triggered);
        assert!(!motion.gate_open);
        assert_eq!(motion.impulse, Vec2::ZERO);
    }

    #[test]
    fn three_alternating_strong_peaks_trigger_once() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for x in [-10.0, 10.0, -11.0] {
            assert!(input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0],));
            timestamp += 70_000_000;
        }

        let motion = input.take_frame();
        assert!(motion.triggered);
        assert!(motion.gate_open);
        assert!(motion.intensity >= 0.45);
        assert!(motion.impulse.length() > 0.0);
        assert!(!input.take_frame().triggered);
    }

    #[test]
    fn trigger_intensity_uses_the_strongest_peak_not_only_the_last_peak() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for x in [-20.0, 20.0, -7.2] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 70_000_000;
        }
        let motion = input.take_frame();
        assert!(motion.triggered);
        assert!(motion.intensity > 0.9, "intensity={}", motion.intensity);
    }

    #[test]
    fn faster_deliberate_shake_produces_a_larger_velocity_impulse() {
        fn shake_with_peak(peak: f32) -> FrameMotion {
            let mut input = direct_input();
            let mut timestamp = warm_up(&mut input, 0);
            for x in [-peak, peak, -peak] {
                input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
                timestamp += 70_000_000;
            }
            input.take_frame()
        }

        let slow = shake_with_peak(8.0);
        let fast = shake_with_peak(18.0);
        assert!(slow.triggered && fast.triggered);
        assert!(
            fast.impulse.length() > slow.impulse.length() * 1.8,
            "slow={} fast={}",
            slow.impulse.length(),
            fast.impulse.length()
        );
        assert!(fast.intensity > slow.intensity);
    }

    #[test]
    fn tilt_never_creates_a_shake_impulse_and_gate_closes_after_quiet_time() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for step in 0..20 {
            let amount = step as f32 / 19.0;
            input.push_sample(
                SensorKind::Gravity,
                timestamp,
                0,
                [9.81 * amount, 9.81 * (1.0 - amount), 0.0],
            );
            timestamp += SAMPLE_NS;
            let tilt_only = input.take_frame();
            assert!(!tilt_only.triggered);
            assert!(!tilt_only.gate_open);
            assert_eq!(tilt_only.impulse, Vec2::ZERO);
        }

        for x in [-11.0, 11.0, -12.0] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 60_000_000;
        }
        assert!(input.take_frame().gate_open);

        for _ in 0..15 {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [0.0, 0.0, 0.0]);
            timestamp += SAMPLE_NS;
        }
        assert!(!input.take_frame().gate_open);
    }

    #[test]
    fn ordinary_after_motion_does_not_extend_the_deliberate_shake_gate() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for x in [-11.0, 11.0, -12.0] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 60_000_000;
        }
        assert!(input.take_frame().gate_open);

        // This is noticeable phone motion, but it is below the deliberate 7 m/s² shake
        // threshold and therefore must not keep resetting the 250 ms activity timer.
        for index in 0..14 {
            let x = if index % 2 == 0 { 4.5 } else { -4.5 };
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += SAMPLE_NS;
        }

        assert!(!input.take_frame().gate_open);
    }

    #[test]
    fn display_rotation_change_restarts_warmup_and_discards_peaks() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for x in [-12.0, 12.0] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 70_000_000;
        }
        input.push_sample(SensorKind::LinearAcceleration, timestamp, 1, [-12.0, 0.0, 0.0]);
        let motion = input.take_frame();
        assert!(!motion.triggered);
        assert!(!motion.gate_open);
        assert_eq!(motion.impulse, Vec2::ZERO);
    }

    #[test]
    fn raw_accelerometer_dynamically_supplies_gravity_and_linear_fallback() {
        let mut input = direct_input();
        let mut timestamp = 0;
        for _ in 0..24 {
            assert!(input.push_sample(SensorKind::Accelerometer, timestamp, 0, [0.0, 9.81, 0.0],));
            timestamp += SAMPLE_NS;
        }
        let resting = input.take_frame();
        assert!((resting.gravity.y - 9.81).abs() < 0.05);
        assert_eq!(resting.impulse, Vec2::ZERO);

        for x in [-14.0, 14.0, -15.0] {
            input.push_sample(SensorKind::Accelerometer, timestamp, 0, [x, 9.81, 0.0]);
            timestamp += 70_000_000;
        }
        let shaken = input.take_frame();
        assert!(shaken.triggered);
        assert!(shaken.gate_open);
    }

    #[test]
    fn raw_fallback_slow_tilt_and_ordinary_pickup_stay_inert() {
        let mut input = direct_input();
        let mut timestamp = 0;
        for _ in 0..24 {
            input.push_sample(SensorKind::Accelerometer, timestamp, 0, [0.0, 9.81, 0.0]);
            timestamp += SAMPLE_NS;
        }
        input.take_frame();

        for step in 0..100 {
            let angle = std::f32::consts::FRAC_PI_2 * step as f32 / 99.0;
            input.push_sample(
                SensorKind::Accelerometer,
                timestamp,
                0,
                [angle.sin() * 9.81, angle.cos() * 9.81, 0.0],
            );
            timestamp += SAMPLE_NS;
            let motion = input.take_frame();
            assert!(!motion.triggered);
            assert_eq!(motion.impulse, Vec2::ZERO);
        }
        for x in [0.0, 5.8, 2.0, 0.0] {
            input.push_sample(SensorKind::Accelerometer, timestamp, 0, [9.81 + x, 0.0, 0.0]);
            timestamp += SAMPLE_NS;
        }
        let motion = input.take_frame();
        assert!(!motion.triggered);
        assert!(!motion.gate_open);
        assert_eq!(motion.impulse, Vec2::ZERO);
    }

    #[test]
    fn equal_cross_stream_timestamps_are_valid_but_gaps_and_invalid_values_reset() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        assert!(input.push_sample(SensorKind::Gravity, timestamp, 0, [0.0, 9.81, 0.0],));
        assert!(input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [-10.0, 0.0, 0.0],));
        assert!(!input.push_sample(
            SensorKind::LinearAcceleration,
            timestamp + SAMPLE_NS,
            0,
            [f32::NAN, 0.0, 0.0],
        ));

        timestamp += DISCONTINUITY_NS + SAMPLE_NS;
        assert!(input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [10.0, 0.0, 0.0],));
        for x in [-10.0, 10.0] {
            timestamp += 70_000_000;
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
        }
        let motion = input.take_frame();
        assert!(!motion.triggered, "timestamp gap must restart warm-up");
        assert_eq!(motion.impulse, Vec2::ZERO);
    }

    #[test]
    fn mailbox_is_bounded_and_coalesces_until_taken() {
        let mut input = direct_input();
        let mut timestamp = warm_up(&mut input, 0);
        for x in [-18.0, 18.0, -18.0] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 60_000_000;
        }
        for index in 0..200 {
            let x = if index % 2 == 0 { -40.0 } else { 40.0 };
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += SAMPLE_NS;
        }
        let motion = input.take_frame();
        assert!(motion.triggered);
        assert!(motion.impulse.length() <= MAX_FRAME_IMPULSE + 0.001);
        let drained = input.take_frame();
        assert_eq!(drained.impulse, Vec2::ZERO);
        assert!(!drained.triggered);
    }

    #[test]
    fn pretrigger_sample_count_and_gate_without_callbacks_are_bounded() {
        let mut input = direct_input();
        for index in 0..(MAX_PRETRIGGER_SAMPLES * 4) {
            input.buffer_pretrigger(index as i64 * 1_000_000, Vec2::new(0.01, -0.01));
        }
        assert_eq!(input.pretrigger_impulses.len(), MAX_PRETRIGGER_SAMPLES);

        let mut timestamp = warm_up(&mut input, 1_000_000_000);
        for x in [-12.0, 12.0, -13.0] {
            input.push_sample(SensorKind::LinearAcceleration, timestamp, 0, [x, 0.0, 0.0]);
            timestamp += 60_000_000;
        }
        assert!(input.take_frame().gate_open);
        input.advance_frame(0.10);
        input.advance_frame(0.10);
        input.advance_frame(0.10);
        input.advance_frame(0.10);
        assert!(!input.take_frame().gate_open);
    }
}
