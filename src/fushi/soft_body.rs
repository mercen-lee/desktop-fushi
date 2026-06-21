use crate::desktop::{SurfaceContact, SurfaceKind};
use crate::fushi::constants::*;
use crate::math::{clampf, lerp, smoothstep, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct BodyKinematics {
    pub center: Vec2,
    pub x_axis: Vec2,
    pub y_axis: Vec2,
    pub velocity: Vec2,
    pub bank_velocity: f32,
    pub time: f32,
    pub crawl_phase: f32,
    pub scale: f32,
    pub attached: bool,
    pub dragged: bool,
    pub stress: f32,
    pub impact_squash: f32,
    pub edge_squash: f32,
    pub grab_local: Vec2,
    pub mouse_velocity: Vec2,
    pub drag_pull_local: Vec2,
    pub drag_handle_radius: f32,
    pub drag_lift: f32,
    pub crawl_drive: f32,
    pub corner_bend: f32,
    pub corner_bend_sign: f32,
}

impl BodyKinematics {
    pub fn drag_shape_guard(&self) -> f32 {
        if !self.dragged {
            return 0.0;
        }

        let spin_guard = smoothstep(
            DRAG_SHAPE_GUARD_SPIN_START,
            DRAG_SHAPE_GUARD_SPIN_FULL,
            self.bank_velocity.abs(),
        );
        let pull_guard = smoothstep(
            DRAG_SHAPE_GUARD_PULL_START,
            DRAG_SHAPE_GUARD_PULL_FULL,
            self.drag_pull_local.length(),
        );
        let speed_guard = smoothstep(
            DRAG_SHAPE_GUARD_SPEED_START,
            DRAG_SHAPE_GUARD_SPEED_FULL,
            self.mouse_velocity.length(),
        );
        let edge_grab = smoothstep(48.0, 112.0, self.grab_local.length());
        let close_guard =
            (1.0 - smoothstep(
                DRAG_SHAPE_GUARD_CLOSE_RADIUS_FULL,
                DRAG_SHAPE_GUARD_CLOSE_RADIUS_START,
                self.drag_handle_radius,
            )) * edge_grab;
        let end_compression_guard = self.drag_end_compression_guard();

        spin_guard
            .max(pull_guard)
            .max(speed_guard)
            .max(close_guard)
            .max(end_compression_guard)
    }

    pub fn drag_end_compression_guard(&self) -> f32 {
        if !self.dragged {
            return 0.0;
        }

        let end_grab = smoothstep(
            BODY_HALF_LENGTH * 0.58,
            BODY_HALF_LENGTH * 0.94,
            self.grab_local.x.abs(),
        );
        let grab_sign = if self.grab_local.x < 0.0 { -1.0 } else { 1.0 };
        let inward_pull = (-grab_sign * self.drag_pull_local.x).max(0.0);
        end_grab * smoothstep(DRAG_END_COMPRESSION_START, DRAG_END_COMPRESSION_FULL, inward_pull)
    }

    pub fn tail_grab_amount(&self) -> f32 {
        if !self.dragged {
            return 0.0;
        }

        let tail_center = TAIL_ANCHOR;
        1.0 - smoothstep(
            TAIL_GRAB_RADIUS * 0.58,
            TAIL_GRAB_RADIUS + 18.0,
            (self.grab_local - tail_center).length(),
        )
    }

    pub fn ear_grab_amount(&self) -> f32 {
        if !self.dragged {
            return 0.0;
        }

        let mut amount: f32 = 0.0;
        for ear in EARS {
            let distance = ear_leaf_distance(self.grab_local, ear);
            amount = amount.max(1.0 - smoothstep(0.0, EAR_GRAB_RADIUS * 0.74, distance));
        }
        amount
    }

    pub fn appendage_grab_amount(&self) -> f32 {
        self.tail_grab_amount().max(self.ear_grab_amount())
    }

    pub fn guarded_drag_pull_local(&self) -> Vec2 {
        let compression_guard = self.drag_end_compression_guard();
        if compression_guard <= 0.001 {
            return self.drag_pull_local;
        }

        let grab_sign = if self.grab_local.x < 0.0 { -1.0 } else { 1.0 };
        let inward_pull = (-grab_sign * self.drag_pull_local.x).max(0.0);
        Vec2::new(
            self.drag_pull_local.x
                + grab_sign * inward_pull * DRAG_END_COMPRESSION_RELIEF * compression_guard,
            self.drag_pull_local.y,
        )
    }

    fn guarded_mouse_local(&self, mouse_local: Vec2) -> Vec2 {
        let compression_guard = self.drag_end_compression_guard();
        if compression_guard <= 0.001 {
            return mouse_local;
        }

        let grab_sign = if self.grab_local.x < 0.0 { -1.0 } else { 1.0 };
        let inward_velocity = (-grab_sign * mouse_local.x).max(0.0);
        Vec2::new(
            mouse_local.x + grab_sign * inward_velocity * 0.72 * compression_guard,
            mouse_local.y,
        )
    }

    #[inline]
    pub fn local_to_world(&self, local: Vec2) -> Vec2 {
        let d = self.deform_local(local);
        self.center + self.x_axis * d.x + self.y_axis * d.y
    }

    #[inline]
    pub fn local_vec_to_world(&self, local: Vec2) -> Vec2 {
        self.x_axis * local.x + self.y_axis * local.y
    }

    pub fn deform_local(&self, local: Vec2) -> Vec2 {
        let x_dir = self.x_axis.normalized_or(Vec2::X);
        let y_dir = self.y_axis.normalized_or(Vec2::Y);
        let shape_guard = self.drag_shape_guard();
        let along = self.velocity.dot(x_dir);
        let normal = self.velocity.dot(y_dir);
        let stretch = clampf(along.abs() / 1700.0, 0.0, 0.14);
        let mut squash = clampf(
            self.impact_squash * 0.42 + self.stress * 0.05 + normal.abs() / 5200.0,
            0.0,
            0.24,
        );
        if self.dragged {
            squash *= lerp(0.62, 0.30, shape_guard);
        }

        let mut x = local.x * (1.0 + stretch - squash * 0.05);
        let mut y = local.y * (1.0 - squash * 0.72 + stretch * 0.04);

        if !self.attached {
            // Once Fushi is lifted, the lower outline tucks inward so the belly no longer looks
            // pressed onto the floor/wall. The effect is kept away from the face and ears.
            let belly = smoothstep(BODY_HALF_HEIGHT * 0.44, BODY_CENTER_TO_BELLY + 5.5, local.y);
            let corner = smoothstep(0.54, 1.0, local.x.abs() / BODY_HALF_LENGTH);
            let lower_grab = smoothstep(
                BODY_HALF_HEIGHT * 0.18,
                BODY_HALF_HEIGHT * 1.05,
                self.grab_local.y,
            );
            let mode_scale = if self.dragged {
                (1.0 - lower_grab * 0.48) * self.drag_lift
            } else {
                0.70
            };
            y -= AIRBORNE_BELLY_TUCK * mode_scale * belly * (0.78 + corner * 0.34);
        }

        if self.attached {
            let edge_squash = clampf(self.edge_squash, 0.0, 1.0);
            if edge_squash > 0.001 {
                let center_fade = 1.0 - smoothstep(0.54, 1.06, local.x.abs() / BODY_HALF_LENGTH);
                let belly_fade = 1.0 - smoothstep(0.20, 1.0, local.y.abs() / BODY_HALF_HEIGHT);
                let amount = edge_squash * (0.66 + center_fade * 0.34) * (0.72 + belly_fade * 0.28);
                x *= 1.0 + amount * 0.16;
                y *= 1.0 - amount * 0.32;
            }

            let corner_bend = clampf(self.corner_bend, 0.0, 1.0);
            if corner_bend > 0.001 {
                let nx = clampf(local.x / BODY_HALF_LENGTH, -1.0, 1.0);
                let body_center_fade = 1.0 - smoothstep(0.36, 1.02, nx.abs());
                let belly_fade = 1.0 - smoothstep(0.24, 1.0, local.y.abs() / BODY_HALF_HEIGHT);
                let soft_side = 0.62 + belly_fade * 0.38;
                let bend = self.corner_bend_sign * corner_bend;

                // During a 90-degree window/screen corner change the world axes rotate smoothly,
                // but a perfectly rigid slug still looks as if it popped into a new drawing.
                // This small local S-curve lets the front and tail lag in opposite directions,
                // so Fushi appears to wrap around the ㄴ corner instead of swapping pose at once.
                y += nx * bend * 13.5 * soft_side;
                x += bend
                    * body_center_fade
                    * (1.0 - smoothstep(0.12, 1.0, local.y.abs() / BODY_HALF_HEIGHT))
                    * 4.0;
                x *= 1.0 - corner_bend * body_center_fade * 0.026;
                y *= 1.0 + corner_bend * body_center_fade * 0.018;
            }

            let belly_fade = 1.0 - clampf(local.y.abs() / BODY_HALF_HEIGHT, 0.0, 1.0);
            let crawl_amount = self.crawl_drive
                * smoothstep(CRAWL_STRETCH_SPEED_START, CRAWL_STRETCH_SPEED_FULL, CRAWL_SPEED);
            if crawl_amount > 0.001 {
                let forward_sign = -1.0;
                let forward_x = local.x * forward_sign;
                let front_weight = smoothstep(-BODY_HALF_LENGTH * 0.10, BODY_HALF_LENGTH * 0.90, forward_x);
                let center_weight =
                    1.0 - smoothstep(BODY_HALF_LENGTH * 0.10, BODY_HALF_LENGTH * 0.92, local.x.abs());
                let phase = self.crawl_phase;
                let reach = (phase.sin() * 0.5 + 0.5).powf(CRAWL_REACH_EXPONENT);
                let gather = ((phase + std::f32::consts::PI).sin() * 0.5 + 0.5).powf(CRAWL_GATHER_EXPONENT);
                let side_fade = 0.72 + belly_fade * 0.28;
                let forward_stretch = CRAWL_FORWARD_STRETCH * crawl_amount * reach * side_fade;

                x += forward_sign * forward_stretch * front_weight;

                let gather_scale =
                    CRAWL_GATHER_SQUASH * crawl_amount * gather * (0.62 + center_weight * 0.38);
                x *= 1.0 - gather_scale;
                y *= 1.0 + gather_scale * CRAWL_GATHER_BELLY_EXPAND;
            }

            let phase = self.time * 5.0 + local.x * 0.034;
            y += phase.sin() * CRAWL_WIGGLE_AMPLITUDE * belly_fade;
            x += (self.time * 3.25 + local.y * 0.052).sin() * 1.15 * belly_fade;
        }

        if self.dragged {
            let tail_grab = self.tail_grab_amount();
            if tail_grab > 0.001 {
                let tail_center = TAIL_ANCHOR;
                let sag_weight = smoothstep(68.0, BODY_HALF_LENGTH * 1.48, (local - tail_center).length());
                let gravity_local = Vec2::new(Vec2::Y.dot(x_dir), Vec2::Y.dot(y_dir));
                let sag = DRAG_TAIL_HANG_SAG * tail_grab * self.drag_lift * sag_weight;
                x += gravity_local.x * sag;
                y += gravity_local.y * sag;
            }

            let dist = (local - self.grab_local).length();
            let pull = (-(dist * dist) / (160.0 * 160.0)).exp();
            let dough_pull = (-(dist * dist) / (218.0 * 218.0)).exp() * self.drag_lift;
            let dough_stretch = 1.0 - shape_guard * 0.46;
            let guarded_pull = self.guarded_drag_pull_local();
            x += guarded_pull.x * dough_pull * 0.24 * dough_stretch;
            y += guarded_pull.y * dough_pull * 0.22 * dough_stretch;

            let mouse_local = self.guarded_mouse_local(Vec2::new(
                self.mouse_velocity.dot(x_dir),
                self.mouse_velocity.dot(y_dir),
            ));
            x += clampf(mouse_local.x / 42.0, -32.0, 32.0) * pull * 0.08 * self.drag_lift;
            y += clampf(mouse_local.y / 42.0, -32.0, 32.0) * pull * 0.08 * self.drag_lift;

            let spin_limit = lerp(12.0, 8.0, shape_guard);
            let spin = clampf(self.bank_velocity, -spin_limit, spin_limit);
            let shear = DRAG_SOFT_SHEAR * (1.0 - shape_guard * 0.48);
            let edge_fade = 1.0 - smoothstep(0.58, 1.0, local.x.abs() / BODY_HALF_LENGTH);
            let belly_fade = 1.0 - smoothstep(0.20, 1.0, local.y.abs() / BODY_HALF_HEIGHT);
            let twist_wave = (self.time * 16.0 + local.x * 0.046 + local.y * 0.023).sin();
            let towel_wave = (self.time * 12.0 - local.x * 0.031 + local.y * 0.041).cos();
            x += twist_wave * spin * shear * (0.46 + belly_fade * 0.40) * self.drag_lift;
            y += towel_wave * spin * shear * 0.58 * (0.35 + edge_fade * 0.60) * self.drag_lift;

            let grab_side = clampf((local.x - self.grab_local.x) / BODY_HALF_LENGTH, -1.0, 1.0);
            y += grab_side * spin * 0.34 * pull * self.drag_lift * (1.0 - shape_guard * 0.36);
        }

        Vec2::new(x, y)
    }
}

#[derive(Clone, Debug)]
pub struct BlobNode {
    pub rest: Vec2,
    pub pos: Vec2,
    pub vel: Vec2,
    pub belly_weight: f32,
}

#[derive(Clone, Debug)]
pub struct SoftBody {
    pub nodes: Vec<BlobNode>,
    rest_neighbor_lengths: Vec<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct ContactProjection {
    pub contact: SurfaceContact,
    pub normal: Vec2,
    pub plane: f32,
    pub scale: f32,
}

impl SoftBody {
    pub fn new() -> Self {
        let rest = generate_rest_outline();
        let mut nodes = Vec::with_capacity(rest.len());
        for p in rest {
            let belly_weight = smoothstep(BODY_HALF_HEIGHT * 0.14, BODY_HALF_HEIGHT * 0.82, p.y);
            nodes.push(BlobNode {
                rest: p,
                pos: Vec2::ZERO,
                vel: Vec2::ZERO,
                belly_weight,
            });
        }

        let mut rest_neighbor_lengths = Vec::with_capacity(nodes.len());
        for i in 0..nodes.len() {
            let j = (i + 1) % nodes.len();
            rest_neighbor_lengths.push((nodes[j].rest - nodes[i].rest).length());
        }

        Self {
            nodes,
            rest_neighbor_lengths,
        }
    }

    pub fn reset(&mut self, kin: &BodyKinematics) {
        for n in &mut self.nodes {
            n.pos = kin.local_to_world(n.rest);
            n.vel = Vec2::ZERO;
        }
    }

    pub fn detach_from_surface(&mut self, kin: &BodyKinematics, normal: Vec2, amount: f32) {
        let nrm = normal.normalized_or(Vec2::Y);
        let scale = kin.scale.max(0.2);
        for n in &mut self.nodes {
            if n.belly_weight <= 0.01 {
                continue;
            }
            let edge = (n.rest.x.abs() / BODY_HALF_LENGTH).clamp(0.0, 1.0);
            let corner = smoothstep(0.58, 1.0, edge);
            let belly = (n.belly_weight * (0.74 + corner * 0.26)).clamp(0.0, 1.0);
            let target = kin.local_to_world(n.rest) - nrm * (amount * belly);
            n.pos += (target - n.pos) * (0.58 * belly);
            n.vel = n.vel * (1.0 - 0.28 * belly) - nrm * (44.0 * scale * belly);
        }
    }

    pub fn closest_node(&self, world: Vec2) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (a.pos - world).length_sq();
                let db = (b.pos - world).length_sq();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        let first = self.nodes.first()?;
        let mut min = first.pos;
        let mut max = first.pos;
        for n in &self.nodes[1..] {
            min = min.min(n.pos);
            max = max.max(n.pos);
        }
        Some((min, max))
    }

    pub fn step(
        &mut self,
        dt: f32,
        kin: &BodyKinematics,
        grab: Option<(usize, Vec2)>,
        contact: Option<ContactProjection>,
    ) {
        let dt = clampf(dt, 0.001, 0.035);
        let shape_guard = kin.drag_shape_guard();
        let appendage_grab = kin.appendage_grab_amount();
        let drag_stiffness_scale = lerp(
            DRAG_MESH_STIFFNESS_SCALE,
            DRAG_MESH_GUARD_STIFFNESS_SCALE,
            shape_guard,
        );
        let stiffness = MESH_STIFFNESS * if kin.dragged { drag_stiffness_scale } else { 1.0 };
        let damping = MESH_DAMPING
            * if kin.dragged {
                DRAG_MESH_DAMPING_SCALE * (1.0 + shape_guard * 0.24)
            } else {
                1.0
            };
        let dragging = kin.dragged;

        for n in &mut self.nodes {
            let target = kin.local_to_world(n.rest);
            let mut force = (target - n.pos) * stiffness;
            force += (kin.velocity - n.vel) * (damping * 0.72);
            n.vel += force * dt;

            if dragging {
                let dist = (n.rest - kin.grab_local).length();
                let drag_radius = lerp(DRAG_GRAB_RADIUS, DRAG_GRAB_RADIUS * 0.56, appendage_grab);
                let w = (-(dist * dist) / (drag_radius * drag_radius)).exp();
                let x_dir = kin.x_axis.normalized_or(Vec2::X);
                let y_dir = kin.y_axis.normalized_or(Vec2::Y);
                let mouse_local = kin.guarded_mouse_local(Vec2::new(
                    kin.mouse_velocity.dot(x_dir),
                    kin.mouse_velocity.dot(y_dir),
                ));
                let mouse_world = x_dir * mouse_local.x + y_dir * mouse_local.y;
                let target_grabbed = target + (mouse_world * lerp(0.014, 0.008, shape_guard));
                n.vel += (target_grabbed - n.pos)
                    * (w * (1.0 - appendage_grab * 0.52)
                        * (lerp(0.82, 0.62, shape_guard) + kin.drag_lift * 0.36)
                        * dt);
            }

            n.pos += n.vel * dt;
        }

        if let Some((_grab_i, _grab_world)) = grab {
            let lower_grab = smoothstep(BODY_HALF_HEIGHT * 0.18, BODY_HALF_HEIGHT * 1.05, kin.grab_local.y);
            let lift_response = 0.14 + kin.drag_lift * lerp(0.50, 0.34, shape_guard);
            let max_pull = DRAG_NODE_PULL_LIMIT
                * lerp(1.0, 0.72, shape_guard)
                * lerp(1.0, 0.52, appendage_grab)
                * (1.0 + lower_grab * 0.52)
                * lift_response
                * kin.scale.max(0.2);
            let grab_world =
                kin.center + kin.local_vec_to_world(kin.grab_local + kin.guarded_drag_pull_local());

            for n in &mut self.nodes {
                let dist = (n.rest - kin.grab_local).length();
                let drag_radius = lerp(DRAG_GRAB_RADIUS, DRAG_GRAB_RADIUS * 0.56, appendage_grab);
                let grab_weight = (-(dist * dist) / (drag_radius * drag_radius)).exp();
                if grab_weight <= 0.025 {
                    continue;
                }

                let target = kin.local_to_world(n.rest);
                let pull = (grab_world - target).clamp_len(max_pull * (0.48 + grab_weight * 0.52));
                let target_grabbed = target + pull;
                let follow = grab_weight
                    * (0.018 + kin.drag_lift * (0.046 + lower_grab * 0.020))
                    * (1.0 - shape_guard * 0.18);
                n.pos = n.pos + (target_grabbed - n.pos) * follow;
                n.vel = n.vel * (1.0 - grab_weight * (0.045 + lower_grab * 0.025))
                    + (target_grabbed - n.pos)
                        * (grab_weight * (lerp(0.70, 0.54, shape_guard) + kin.drag_lift * 0.82));
            }
        }

        self.limit_shape_deviation(kin, shape_guard);

        if let Some(cp) = contact {
            self.project_belly_to_surface(cp);
        }

        for _ in 0..MESH_CONSTRAINT_ITERATIONS {
            self.solve_neighbor_constraints(kin, shape_guard);
            if let Some(cp) = contact {
                self.project_belly_to_surface(cp);
            }
            self.limit_shape_deviation(kin, shape_guard);
        }
    }

    fn solve_neighbor_constraints(&mut self, kin: &BodyKinematics, shape_guard: f32) {
        let len = self.nodes.len();
        if len < 3 {
            return;
        }
        let drag_neighbor_scale = lerp(
            DRAG_NEIGHBOR_STIFFNESS_SCALE,
            DRAG_NEIGHBOR_GUARD_STIFFNESS_SCALE,
            shape_guard,
        );
        let stiffness = MESH_NEIGHBOR_STIFFNESS * if kin.dragged { drag_neighbor_scale } else { 1.0 };
        for i in 0..len {
            let j = (i + 1) % len;
            let (a, b) = get_two_mut(&mut self.nodes, i, j);
            let delta = b.pos - a.pos;
            let d = delta.length();
            if d <= 0.001 {
                continue;
            }
            let rest = self.rest_neighbor_lengths[i] * kin.scale.max(0.2);
            let diff = (d - rest) / d;
            let correction = delta * (diff * stiffness * 0.5);
            a.pos += correction;
            b.pos -= correction;
        }
    }

    fn project_belly_to_surface(&mut self, cp: ContactProjection) {
        let scale = cp.scale.max(0.2);
        for n in &mut self.nodes {
            if n.belly_weight <= 0.01 {
                continue;
            }
            let edge = (n.rest.x.abs() / BODY_HALF_LENGTH).clamp(0.0, 1.0);
            let corner = smoothstep(0.66, 1.0, edge);
            let edge_contact_fade = 1.0 - corner * BELLY_CORNER_CONTACT_FADE;
            let belly = (n.belly_weight * edge_contact_fade).clamp(0.0, 1.0);
            if belly <= 0.01 {
                continue;
            }

            let side_corner_lift =
                if cp.contact.kind == SurfaceKind::Bottom || cp.contact.kind == SurfaceKind::Top {
                    corner * BELLY_CONTACT_CORNER_LIFT * scale
                } else {
                    corner * (BELLY_CONTACT_CORNER_LIFT * 0.72) * scale
                };
            let airy_gap = (1.0 - n.belly_weight) * 1.4 * scale;
            let contact_sink = 0.35 * scale;
            let desired = cp.plane + contact_sink - airy_gap - side_corner_lift;
            let current = n.pos.dot(cp.normal);
            let correction = (current - desired) * belly * BELLY_CONTACT_STRENGTH;
            n.pos -= cp.normal * correction;
            let vn = n.vel.dot(cp.normal);
            if vn > 0.0 {
                n.vel -= cp.normal * (vn * belly);
            }
        }
    }

    fn limit_shape_deviation(&mut self, kin: &BodyKinematics, shape_guard: f32) {
        let drag_max_deviation = lerp(
            MESH_DRAG_SHAPE_MAX_DEVIATION,
            MESH_DRAG_GUARD_SHAPE_MAX_DEVIATION,
            shape_guard,
        );
        let max_deviation = if kin.dragged {
            drag_max_deviation
        } else {
            MESH_SHAPE_MAX_DEVIATION
        } * kin.scale.max(0.2);
        for n in &mut self.nodes {
            let target = kin.local_to_world(n.rest);
            let delta = n.pos - target;
            let dist = delta.length();
            if dist <= max_deviation || dist <= 0.001 {
                continue;
            }
            let dir = delta / dist;
            let corrected = target + dir * max_deviation;
            n.pos = corrected;
            let outward = n.vel.dot(dir);
            if outward > 0.0 {
                n.vel -= dir * (outward * 0.88);
            }
        }
    }
}

fn get_two_mut<T>(items: &mut [T], i: usize, j: usize) -> (&mut T, &mut T) {
    assert!(i != j);
    if i < j {
        let (left, right) = items.split_at_mut(j);
        (&mut left[i], &mut right[0])
    } else {
        let (left, right) = items.split_at_mut(i);
        (&mut right[0], &mut left[j])
    }
}

fn is_rear_svg_ear(ear: EarSpec) -> bool {
    ear.anchor.x < -90.0
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

fn generate_rest_outline() -> Vec<Vec2> {
    let mut pts = Vec::with_capacity(BODY_NODE_COUNT);
    let top_count = BODY_NODE_COUNT / 2;
    let bottom_count = BODY_NODE_COUNT - top_count;

    // Top: right -> left, high and fluffy.
    for i in 0..top_count {
        let u = i as f32 / (top_count - 1) as f32;
        let x = BODY_HALF_LENGTH * (1.0 - 2.0 * u);
        let nx = (x / BODY_HALF_LENGTH).abs();
        let arch = (1.0 - nx.powf(2.35)).max(0.0).powf(0.54);
        let rough = ((i as f32 * 1.91).sin() * 1.2 + (i as f32 * 0.73).cos() * 0.9) * BODY_OUTLINE_ROUGHNESS;
        let head_tuck = if x < -92.0 {
            3.6 * smoothstep(92.0, BODY_HALF_LENGTH, -x)
        } else {
            0.0
        };
        let tail_slope = if x > 95.0 { 4.0 * ((x - 95.0) / 63.0) } else { 0.0 };
        let y = -BODY_HALF_HEIGHT * arch * 0.92 + rough + head_tuck + tail_slope;
        pts.push(Vec2::new(x, y));
    }

    // Bottom: left -> right. The lower corners are lifted in the rest pose, so when Fushi is
    // airborne they curl naturally instead of forming flat paper-like corners. Contact projection
    // only presses the soft belly area back to the active surface.
    for i in 0..bottom_count {
        let u = i as f32 / (bottom_count - 1) as f32;
        let x = -BODY_HALF_LENGTH + 5.5 + 2.0 * (BODY_HALF_LENGTH - 5.5) * u;
        let nx = (x / BODY_HALF_LENGTH).abs();
        let center_sag = 2.4 * (1.0 - nx.powf(2.0)).max(0.0).powf(0.72);
        let corner_lift = BELLY_AIR_CORNER_LIFT * smoothstep(0.62, 1.0, nx);
        let edge_fade = (1.0 - nx.powf(4.0)).max(0.0);
        let rough =
            ((i as f32 * 1.37 + 1.8).sin() * 0.55 + (i as f32 * 0.59).cos() * 0.35) * 0.30 * edge_fade;
        let y = BODY_CENTER_TO_BELLY + center_sag - corner_lift + rough;
        pts.push(Vec2::new(x, y));
    }
    pts
}
