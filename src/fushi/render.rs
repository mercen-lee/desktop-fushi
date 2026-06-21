use crate::canvas::{Ellipse, GradientStop, VectorCanvas};
use crate::fushi::constants::*;
use crate::fushi::physics::{FushiBody, FushiExpression, MotionMode};
use crate::fushi::soft_body::BlobNode;
use crate::fushi::svg_reference as svg_ref;
use crate::math::{
    catmull_rom_path, catmull_rom_path_with_tension, lerp, smoothstep, Color, Path, PathCmd, Vec2,
};

const FACE_EYE_SCALE: f32 = 0.74;
const EAR_SETTLE_DROP_Y: f32 = 4.0;

// The face is authored as a flat 2D vector decal.  The whole decal is then
// fitted to one cheek-surface frame, so eyes/mouth keep their drawn proportions
// while still riding on Fushi's soft 3D-ish body.
const FACE_DECAL_CENTER_LOCAL: Vec2 = Vec2::new(-132.0, 19.0);
const FACE_DECAL_BASE_SHIFT: Vec2 = Vec2::new(5.0, -0.8);
const FACE_DECAL_X_SCALE: f32 = 0.995;
const FACE_DECAL_Y_SCALE: f32 = 0.990;
const FACE_DECAL_SHEAR_Y: f32 = 0.0008;
const FACE_DECAL_SURFACE_Z_NORM: f32 = 0.50;
const FACE_DECAL_AXIS_DELTA: f32 = 24.0;

const APPENDAGE_FINAL_DARK: Color = Color::rgba_u8(53, 51, 57, 255);
const APPENDAGE_FINAL_LIGHT: Color = Color::rgba_u8(203, 197, 188, 255);
const APPENDAGE_FINAL_SHADE: Color = Color::rgba_u8(88, 82, 93, 255);
const APPENDAGE_FINAL_EDGE: Color = Color::rgba_u8(35, 35, 36, 255);
const FINAL_EAR_STROKE_LOCAL: f32 = 1.643_777_8;
const FINAL_TAIL_STROKE_LOCAL: f32 = 1.859_722_3;

pub struct FushiRenderer;

impl FushiRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn draw<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody) {
        let body_points = self.body_outline_points(body);
        let body_path = self.body_path_from_points(body, &body_points);

        let [back_ear, front_ear] = if ear_render_depth(body, 0) <= ear_render_depth(body, 1) {
            [0_usize, 1_usize]
        } else {
            [1_usize, 0_usize]
        };
        self.draw_ear(canvas, body, back_ear);

        canvas.fill_path(&body_path, BODY_FILL);
        self.draw_spots(canvas, body, &body_points);
        canvas.stroke_path(&body_path, BODY_STROKE, 2.1 * body.scale.max(0.2));
        self.draw_tail(canvas, body);
        self.draw_ear(canvas, body, front_ear);

        // Draw every expression last as one decal clipped to the body hull.
        let face_clip = self.face_clip_polygon_from_points(body, &body_points);
        {
            let mut clipped = FaceClipCanvas::new(canvas, &face_clip);
            self.draw_blush(&mut clipped, body);
        }
        self.draw_face(canvas, body, &face_clip);
        {
            let mut clipped = FaceClipCanvas::new(canvas, &face_clip);
            self.draw_comic_effects(&mut clipped, body);
        }
    }

    fn body_outline_points(&self, body: &FushiBody) -> Vec<Vec2> {
        let yaw_amount = view_yaw(body).abs();
        let pitch_amount = view_pitch(body).abs();
        let shape_guard = render_drag_shape_guard(body);
        let projection_mix = body_outline_projection_mix(body);
        let deformation_mix = body_outline_deformation_mix(body);

        let mut pts: Vec<Vec2> = body
            .mesh
            .nodes
            .iter()
            .map(|n| {
                let projected = body_outline_local_to_world(body, n.rest, deformation_mix);
                // Crawling should still read like the original soft 2D motion: the live mesh leads
                // the silhouette, while pseudo-3D projection only adds gentle surface curvature.
                n.pos * (1.0 - projection_mix) + projected * projection_mix
            })
            .collect();

        if pts.len() >= 6 {
            // Smooth after the mesh/projection blend.  At the normal forward-facing crawl angle this
            // keeps the old stretch-gather movement instead of over-polishing it into a rigid 3D turntable.
            for _ in 0..2 {
                let prev = pts.clone();
                let len = prev.len();
                for (i, p) in pts.iter_mut().enumerate() {
                    let rest = body.mesh.nodes[i].rest;
                    let edge = (rest.x.abs() / BODY_HALF_LENGTH).clamp(0.0, 1.0);
                    let belly = smoothstep(BODY_HALF_HEIGHT * 0.22, BODY_CENTER_TO_BELLY + 4.0, rest.y);
                    let view_soften = 1.0 + yaw_amount * 0.18 + pitch_amount * 0.12;
                    let w = (0.062 + belly * 0.064 + shape_guard * 0.048) * (1.0 - edge * 0.36) * view_soften;
                    *p = prev[i] * (1.0 - w * 2.0) + (prev[(i + len - 1) % len] + prev[(i + 1) % len]) * w;
                }
            }
        }

        pts
    }

    fn body_path_from_points(&self, body: &FushiBody, roots: &[Vec2]) -> Path {
        if roots.len() < 6 {
            return catmull_rom_path_with_tension(roots, true, 0.105);
        }

        // Calm Fushi now uses a real furry silhouette. Surprise/panic does not draw
        // a separate ring: the same uneven tufts simply stand up and sharpen.
        let startled = self.startled_quill_alpha(body);
        let pts = self.body_fur_points(body, roots, startled);
        if pts.len() < 6 {
            return catmull_rom_path_with_tension(roots, true, 0.105);
        }

        let tension = lerp(0.022, 0.014, startled.clamp(0.0, 1.0));
        catmull_rom_path_with_tension(&pts, true, tension)
    }

    fn face_clip_polygon_from_points(&self, body: &FushiBody, roots: &[Vec2]) -> Vec<Vec2> {
        if roots.len() < 3 {
            return Vec::new();
        }

        // FaceClipCanvas clips against convex polygons only.  Build a real convex
        // hull from the smooth body roots instead of using the locally concave
        // furry/soft contour; that old clip could delete the whole expression in
        // tall or squashed poses.  The inset is tiny so it trims only escaped
        // pixels, not the face itself.
        let hull = convex_hull(roots);
        if hull.len() < 3 {
            return Vec::new();
        }
        let center = polygon_centroid(&hull);
        let inset = 0.35 * body.scale.max(0.2);
        hull.iter()
            .map(|p| {
                let inward = (center - *p).normalized_or(Vec2::ZERO);
                *p + inward * inset
            })
            .collect()
    }

    fn body_fur_points(&self, body: &FushiBody, roots: &[Vec2], startled: f32) -> Vec<Vec2> {
        let scale = body.scale.max(0.2);
        let shape_guard = render_drag_shape_guard(body);
        let calm_fur_alpha = (1.0 - shape_guard * 0.34).clamp(0.58, 1.0);
        let panic_boost = if body.expression == FushiExpression::Panic {
            1.22
        } else {
            1.0
        };
        let len = roots.len();
        let winding = signed_polygon_area(roots);
        let mut pts = Vec::with_capacity(len * 4);

        for i in 0..len {
            let root = roots[i];
            let next = roots[(i + 1) % len];
            let edge = next - root;
            let edge_len = edge.length();
            if edge_len <= scale * 1.8 {
                pts.push(root);
                continue;
            }

            let rest = (body.mesh.nodes[i].rest + body.mesh.nodes[(i + 1) % len].rest) * 0.5;
            let tangent = edge.normalized_or(Vec2::X);
            let mut outward = if winding < 0.0 {
                tangent.perp_left()
            } else {
                -tangent.perp_left()
            };
            let mid_for_normal = (root + next) * 0.5;
            if outward.dot(mid_for_normal - body.center) < 0.0 {
                outward = -outward;
            }
            let outward =
                outward.normalized_or((mid_for_normal - body.center).normalized_or(Vec2::new(0.0, -1.0)));

            // Tiny root offsets break the perfect ellipse without changing the physical mesh.
            // Keep the floor-contact belly shorter so Fushi still sits cleanly on the desktop.
            let root_rest = body.mesh.nodes[i].rest;
            let root_noise = quill_noise(i, 0x4B1D) - 0.5;
            let root_top = 1.0 - smoothstep(-BODY_HALF_HEIGHT * 0.60, BODY_HALF_HEIGHT * 0.05, root_rest.y);
            let root_belly = smoothstep(BODY_HALF_HEIGHT * 0.12, BODY_CENTER_TO_BELLY + 6.0, root_rest.y);
            let root_contact = if body.mode == MotionMode::Attached {
                smoothstep(33.0, BODY_CENTER_TO_BELLY + 5.0, root_rest.y) * 0.30
            } else {
                0.0
            };
            let root_lift = root_noise
                * (0.55 + root_top * 0.90 + root_belly * 0.52)
                * scale
                * calm_fur_alpha
                * (1.0 - root_contact);
            pts.push(root + outward * root_lift);

            let nx = (rest.x / BODY_HALF_LENGTH).abs().clamp(0.0, 1.0);
            let top = 1.0 - smoothstep(-BODY_HALF_HEIGHT * 0.60, BODY_HALF_HEIGHT * 0.08, rest.y);
            let side = smoothstep(0.64, 1.0, nx);
            let belly = smoothstep(BODY_HALF_HEIGHT * 0.10, BODY_CENTER_TO_BELLY + 7.0, rest.y);
            let attached_belly = if body.mode == MotionMode::Attached {
                smoothstep(34.0, BODY_CENTER_TO_BELLY + 6.0, rest.y)
            } else {
                0.0
            };
            let face_bottom_soften =
                1.0 - smoothstep(-150.0, -112.0, rest.x) * smoothstep(18.0, 54.0, rest.y) * 0.34;
            let tail_base_soften =
                1.0 - smoothstep(112.0, BODY_HALF_LENGTH, rest.x) * smoothstep(-24.0, 34.0, rest.y) * 0.26;
            let contact_soften = 1.0 - attached_belly * 0.30;
            let end_soften = 1.0 - smoothstep(0.92, 1.0, nx) * 0.24;
            let fur_region = (0.46 + top * 0.45 + side * 0.16 + belly * 0.36).clamp(0.38, 1.12)
                * face_bottom_soften
                * tail_base_soften
                * contact_soften
                * end_soften;

            let skip_chance = (0.13 - top * 0.06 - side * 0.03 - startled * 0.08).clamp(0.0, 0.16);
            if quill_noise(i, 0xF17E) < skip_chance && startled <= 0.18 {
                continue;
            }

            let secondary = edge_len > scale * 9.0 && quill_noise(i, 0x5EC0) > (0.70 - startled * 0.22);
            let tuft_count = if secondary { 2 } else { 1 };
            for tuft in 0..tuft_count {
                let k = i * 5 + tuft;
                let center_t = if tuft_count == 1 {
                    lerp(0.28, 0.72, quill_noise(k, 0x71F3))
                } else if tuft == 0 {
                    lerp(0.20, 0.44, quill_noise(k, 0x71F3))
                } else {
                    lerp(0.56, 0.82, quill_noise(k, 0x71F3))
                };
                let mid = root + edge * center_t;
                let base_variation = quill_noise(k, 0xA11CE);
                let micro_wave = (body.time * 2.15 + base_variation * std::f32::consts::TAU).sin() * 0.10;
                let calm_length =
                    (1.10 + base_variation * 3.25 + micro_wave) * scale * fur_region * calm_fur_alpha;

                let mut startle_length = 0.0;
                if startled > 0.02 {
                    let variation = quill_noise(k, 0xA1A1);
                    let flicker = quill_noise(k, 0xF05A);
                    let pulse_phase = quill_noise(k, 0x51A7E) * std::f32::consts::TAU;
                    let pulse_speed = lerp(7.5, 16.5, quill_noise(k, 0xB10B));
                    let belly_soften =
                        1.0 - smoothstep(BODY_HALF_HEIGHT * 0.22, BODY_CENTER_TO_BELLY, rest.y) * 0.24;
                    let end_length = 1.0 - smoothstep(124.0, BODY_HALF_LENGTH, rest.x.abs()) * 0.46;
                    let face_belly_length =
                        1.0 - smoothstep(-138.0, -104.0, rest.x) * smoothstep(18.0, 52.0, rest.y) * 0.42;
                    let shiver = (body.time * 24.0 + flicker * std::f32::consts::TAU).sin();
                    let pulse = (body.time * pulse_speed + pulse_phase).sin() * 0.5 + 0.5;
                    let flutter = (body.time * (pulse_speed * 1.9) + pulse_phase * 0.47).sin() * 0.5 + 0.5;
                    let animated_length = lerp(0.58, 1.58, pulse) + flutter * 0.15;
                    let startle_gate = lerp(0.34, 1.0, smoothstep(0.18, 0.82, quill_noise(k, 0xC0FFEE)));
                    startle_length = (5.0 + variation * 8.2 + shiver * 0.35)
                        * scale
                        * startled
                        * panic_boost
                        * animated_length
                        * belly_soften
                        * end_length
                        * face_belly_length
                        * startle_gate;
                }

                let mut length = calm_length + startle_length;
                if length <= scale * 0.35 {
                    continue;
                }

                let shoulder = edge_len.min(14.0 * scale)
                    * lerp(0.12, 0.28, quill_noise(k, 0xBEEF))
                    * lerp(1.0, 0.58, startled.clamp(0.0, 1.0));
                let root_lift = calm_length * lerp(0.10, 0.30, quill_noise(k, 0x9AA9));
                let jitter = (quill_noise(k, 0x5EED) - 0.5) * edge_len.min(10.0 * scale) * 0.40;
                let mut side_step = if startled > 0.02 {
                    (body.time * 24.0 + quill_noise(k, 0xF05A) * std::f32::consts::TAU).sin()
                        * 0.32
                        * scale
                        * startled
                } else {
                    0.0
                };

                if body.body_pet_amount > 0.01 && body.mode != MotionMode::Dragged {
                    let brush_distance = (mid - body.cursor_world).length();
                    let brush = body.body_pet_amount
                        * (1.0 - smoothstep(14.0 * scale, 76.0 * scale, brush_distance))
                        * (1.0 - startled * 0.42);
                    if brush > 0.001 {
                        let brush_dir = body.passive_mouse_velocity.normalized_or(tangent);
                        let along = brush_dir.dot(tangent).clamp(-1.0, 1.0);
                        let lift = brush_dir.dot(outward).max(0.0);
                        length += brush * scale * (0.65 + lift * 1.15 + quill_noise(k, 0xB405) * 0.80);
                        side_step += brush * along * 5.8 * scale;
                    }
                }

                pts.push(mid - tangent * shoulder + outward * (root_lift * 0.42));
                pts.push(mid + outward * length + tangent * (jitter + side_step));
                pts.push(mid + tangent * shoulder + outward * (root_lift * 0.25));
            }
        }

        pts
    }

    fn draw_spots<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, body_points: &[Vec2]) {
        let mut spots: Vec<(f32, crate::fushi::constants::SpotSpec, f32)> = SPOTS
            .iter()
            .copied()
            .map(|spot| {
                let z_norm = spot_surface_z_norm(spot.local);
                let z = body_surface_depth(spot.local) * z_norm;
                (projected_depth(body, spot.local, z), spot, z_norm)
            })
            .collect();
        // Far surface marks first. The dots are not merely re-ordered: their centers and axes are
        // projected from points on the rounded shell, so they slide, flatten and wrap with yaw/pitch.
        spots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        for (depth, spot, z_norm) in spots {
            // Spots are body markings, not animated fur clumps. Keep them visually stable so
            // they move with the projected body surface instead of seeming to hop around.
            let pulse = 1.0 + 0.006 * (body.time * 2.2 + spot.local.x * 0.018).sin();
            let shrink = if body.mode == MotionMode::Dragged {
                0.86
            } else {
                0.92
            };
            let radius = spot.radius * pulse * shrink * SPOT_RADIUS_SCALE * body.scale.max(0.2);
            let (center, axis_x, axis_y, surface_scale) =
                body_surface_frame(body, spot.local, z_norm, decoration_surface_deformation_mix(body));
            if !point_in_polygon(center, body_points) {
                continue;
            }
            let normal_visibility = surface_visibility(body, spot.local, z_norm).clamp(0.44, 1.0);
            let wrap_scale = (0.92 - view_yaw(body).abs() * 0.18 + depth * 0.00040).clamp(0.56, 1.16);
            let spot_alpha =
                (0.70 + normal_visibility * 0.30 - view_yaw(body).abs() * 0.045).clamp(0.38, 1.0);
            let rx = radius * spot.stretch * surface_scale.x * wrap_scale;
            let ry = radius * surface_scale.y * (0.88 + normal_visibility * 0.12);
            let keep_scale = ellipse_keep_scale_in_polygon(center, rx, ry, body_points);
            if keep_scale <= 0.12 {
                continue;
            }
            canvas.fill_ellipse(
                Ellipse::new(center, axis_x, axis_y, rx * keep_scale, ry * keep_scale),
                with_alpha(SPOT, spot_alpha * smoothstep(0.12, 0.52, keep_scale)),
            );
        }
    }

    fn draw_ear<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, i: usize) {
        let wobble = self.ear_wobble(body, i);
        let scale = body.scale.max(0.2);
        let alpha = ear_layer_alpha(body, i);
        let (outer_cmds, inner_cmds, shade_cmds) = if i == 0 {
            (
                &svg_ref::FINAL_REAR_EAR_OUTER_PATH[..],
                &svg_ref::FINAL_REAR_EAR_INNER_PATH[..],
                &svg_ref::FINAL_REAR_EAR_SHADE_PATH[..],
            )
        } else {
            (
                &svg_ref::FINAL_FRONT_EAR_OUTER_PATH[..],
                &svg_ref::FINAL_FRONT_EAR_INNER_PATH[..],
                &svg_ref::FINAL_FRONT_EAR_SHADE_PATH[..],
            )
        };

        let outer = self.svg_ear_command_path(body, i, outer_cmds, wobble, 0.0);
        let inner = self.svg_ear_command_path(body, i, inner_cmds, wobble, 2.4);
        let shade = self.svg_ear_command_path(body, i, shade_cmds, wobble, 0.0);
        let stroke_width = FINAL_EAR_STROKE_LOCAL * scale * ear_line_scale(body, i);
        let edge = with_alpha(APPENDAGE_FINAL_EDGE, 0.98 * alpha);

        // Imported directly from the user-edited SVG: outer fill, inner fill,
        // and the darker side plane are painted as separate closed Bezier shapes.
        canvas.fill_path(&outer, with_alpha(APPENDAGE_FINAL_DARK, alpha));
        canvas.stroke_path(&outer, edge, stroke_width);
        canvas.fill_path(&inner, with_alpha(APPENDAGE_FINAL_LIGHT, alpha));
        canvas.stroke_path(&inner, edge, stroke_width);
        canvas.fill_path(&shade, with_alpha(APPENDAGE_FINAL_SHADE, alpha));
        canvas.stroke_path(&shade, edge, stroke_width);
    }

    fn draw_svg_ear_inner<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        index: usize,
        subpaths: &[&[Vec2]],
        wobble_degrees: f32,
    ) {
        let path = self.svg_ear_path(body, index, subpaths, wobble_degrees, 2.4);
        let root = ear_root_world(body, index);
        let tip = self.svg_ear_reference_tip_world(body, index, subpaths, wobble_degrees);
        canvas.fill_path_linear_gradient(
            &path,
            root,
            tip,
            &[
                GradientStop::new(0.0, EAR_INNER_SHADOW),
                GradientStop::new(0.38, EAR_INNER_FILL),
                GradientStop::new(1.0, Color::rgba_u8(247, 242, 235, 255)),
            ],
        );
        canvas.stroke_path(&path, Color::rgba_u8(54, 52, 57, 228), 0.78 * body.scale.max(0.2));
    }

    fn svg_ear_command_path(
        &self,
        body: &FushiBody,
        index: usize,
        commands: &[PathCmd],
        wobble_degrees: f32,
        z_bias: f32,
    ) -> Path {
        let root = EARS[index].anchor;
        reference_path_from_commands(commands, |p| {
            let local = self.ear_reference_local_point(body, index, root, p, wobble_degrees, z_bias);
            ear_local_to_world(body, root, local, index)
        })
    }

    fn svg_ear_reference_tip_world(
        &self,
        body: &FushiBody,
        index: usize,
        subpaths: &[&[Vec2]],
        wobble_degrees: f32,
    ) -> Vec2 {
        let root = EARS[index].anchor;
        let mut best = root;
        let mut best_y = f32::INFINITY;
        for path in subpaths {
            for p in *path {
                if p.y < best_y {
                    best_y = p.y;
                    best = *p;
                }
            }
        }
        let local = self.ear_reference_local_point(body, index, root, best, wobble_degrees, 0.0);
        ear_local_to_world(body, root, local, index)
    }

    fn svg_ear_path(
        &self,
        body: &FushiBody,
        index: usize,
        subpaths: &[&[Vec2]],
        wobble_degrees: f32,
        z_bias: f32,
    ) -> Path {
        let root = EARS[index].anchor;
        reference_path_from_subpaths(subpaths, |p| {
            let local = self.ear_reference_local_point(body, index, root, p, wobble_degrees, z_bias);
            ear_local_to_world(body, root, local, index)
        })
    }

    fn ear_reference_local_point(
        &self,
        body: &FushiBody,
        index: usize,
        root: Vec2,
        p: Vec2,
        wobble_degrees: f32,
        _z_bias: f32,
    ) -> Vec2 {
        let scale = EARS[index].scale;
        let d = (p - root) * scale;
        let tip_weight = smoothstep(8.0, 58.0, d.length());
        let curl_weight = smoothstep(-26.0, -66.0, d.y);
        let wobble = wobble_degrees.to_radians() * (0.22 + tip_weight * 0.78);
        let breathing = (body.time * 3.3 + index as f32 * 1.7 + d.x * 0.035).sin()
            * (0.35 + body.stress * 1.15).to_radians()
            * tip_weight
            * 0.75;
        let view_spread = view_yaw(body) * if index == 0 { -1.8 } else { 1.35 } * curl_weight;
        let base_attach = (1.0 - smoothstep(8.0, 36.0, d.length())).clamp(0.0, 1.0);
        let attach_drop = if index == 0 { 11.0 } else { 7.0 };
        let mut local = root
            + d.rotate(wobble + breathing)
            + Vec2::new(
                view_spread,
                view_pitch(body) * 1.4 * tip_weight + attach_drop * base_attach + EAR_SETTLE_DROP_Y,
            );

        let kin = body.kinematics();
        let ear_grab = kin.ear_grab_amount();
        if ear_grab > 0.001 {
            let drag_pull = kin.guarded_drag_pull_local().clamp_len(170.0);
            local += drag_pull * (ear_grab * tip_weight * 0.18);
        }

        // Ears use a lighter material response than the body.  The root is
        // pinned by tip_weight, so brushing/grabbing never detaches the ear from
        // the body, but the leaf still flutters and stretches at the free end.
        let material = body.appendage_sway.clamp_len(58.0);
        let material_weight = tip_weight * (0.20 + curl_weight * 0.80);
        if material_weight > 0.001 {
            local += Vec2::new(material.x * 0.20, material.y * 0.14) * material_weight;
            local += Vec2::new(
                (body.time * 11.0 + index as f32 * 2.2 + d.y * 0.015).sin() * 1.7,
                (body.time * 13.5 + index as f32).sin() * 1.0,
            ) * (body.appendage_pet_amount * material_weight);
        }

        if body.appendage_pet_amount > 0.01 && body.mode != MotionMode::Dragged {
            let cursor_local = body.world_to_local(body.cursor_world);
            let brush_distance = (local - cursor_local).length();
            let brush =
                body.appendage_pet_amount * (1.0 - smoothstep(5.0, 54.0, brush_distance)) * tip_weight;
            if brush > 0.001 {
                let brush_delta = body
                    .world_to_local(body.cursor_world + body.passive_mouse_velocity * 0.016)
                    - cursor_local;
                let sway = brush_delta.clamp_len(24.0);
                local += sway * (0.15 * brush);
                local.y += (body.time * 13.0 + index as f32).sin() * 1.7 * brush;
            }
        }
        local
    }

    fn ear_wobble(&self, body: &FushiBody, _i: usize) -> f32 {
        let shape_guard = render_drag_shape_guard(body);
        let mut wobble = (body.bank_velocity * 5.8).clamp(-13.0, 13.0);
        wobble += body.view_pitch * 1.2;
        wobble += (body.time * 4.9).sin() * (1.0 + body.stress * 3.2);
        wobble += (body.time * 12.5 + _i as f32 * 1.7).sin() * 4.2 * body.appendage_pet_amount;
        match body.expression {
            FushiExpression::Angry => {
                wobble -= 7.0;
            }
            FushiExpression::Dizzy => wobble += (body.time * 18.0).sin() * 5.5,
            FushiExpression::Sad => wobble -= 5.5,
            FushiExpression::Surprised | FushiExpression::Panic | FushiExpression::Stare => wobble += 5.0,
            _ => {}
        }
        wobble * 0.78 * (1.0 - shape_guard * 0.54)
    }

    #[allow(dead_code)]
    fn ear_path(
        &self,
        body: &FushiBody,
        index: usize,
        ear: crate::fushi::constants::EarSpec,
        lean_degrees: f32,
        outer: bool,
    ) -> Path {
        let line_scale = ear_line_scale(body, index).clamp(0.84, 1.12);
        let scale = ear.scale * line_scale;
        let lean = lean_degrees.to_radians();
        let along = Vec2::new(0.0, -1.0)
            .rotate(lean)
            .normalized_or(Vec2::new(0.0, -1.0));
        let side = along.perp_left().normalized_or(Vec2::X);
        let root_settle = if index == 0 {
            Vec2::new(-0.8, 2.6) * scale
        } else {
            Vec2::new(0.8, 2.6) * scale
        };
        let anchor = ear.anchor + root_settle;
        let (length, width, tip_side, inner_shift) = if outer {
            (
                EAR_TIP_LENGTH * scale,
                EAR_LEAF_HALF_WIDTH * scale,
                -4.0 * scale,
                Vec2::ZERO,
            )
        } else if index == 0 {
            (
                50.0 * scale,
                10.8 * scale,
                -4.0 * scale,
                side * (-3.0 * scale) + along * (-2.0 * scale),
            )
        } else {
            (
                50.0 * scale,
                10.8 * scale,
                -4.0 * scale,
                side * (-3.0 * scale) + along * (-2.0 * scale),
            )
        };

        let base_tuck = if outer { -6.0 * scale } else { -8.0 * scale };
        let root = anchor + inner_shift;
        let local = |side_offset: f32, along_offset: f32| -> Vec2 {
            root + side * side_offset + along * along_offset
        };
        let to_world = |p: Vec2| ear_local_to_world(body, ear.anchor, p, index);

        let left_base = local(-width * if outer { 0.44 } else { 0.58 }, base_tuck);
        let right_base = local(
            width * if outer { 0.52 } else { 0.22 },
            base_tuck + if outer { 4.2 * scale } else { 2.0 * scale },
        );
        let top_left = local(-width * if outer { 0.82 } else { 0.62 }, length * 0.87);
        let top_mid = local(tip_side, length);
        let top_right = local(width * if outer { 0.72 } else { 0.58 }, length * 0.87);
        let left_mid = local(
            -width * if outer { 1.18 } else { 1.06 },
            length * if outer { 0.43 } else { 0.38 },
        );
        let right_mid = local(
            width * if outer { 0.98 } else { 0.76 },
            length * if outer { 0.39 } else { 0.34 },
        );
        let right_root = local(
            width * if outer { 0.28 } else { 0.16 },
            if outer { -13.0 } else { -4.2 } * scale,
        );
        let left_root = local(
            -width * if outer { 0.38 } else { 0.42 },
            if outer { -13.5 } else { -4.8 } * scale,
        );

        let mut p = Path::new();
        p.move_to(to_world(left_base));
        p.cubic_to(to_world(left_mid), to_world(top_left), to_world(top_mid));
        p.cubic_to(to_world(top_right), to_world(right_mid), to_world(right_base));
        p.cubic_to(to_world(right_root), to_world(left_root), to_world(left_base));
        p.close();
        p
    }

    #[allow(dead_code)]
    fn ear_rim_path(
        &self,
        body: &FushiBody,
        index: usize,
        ear: crate::fushi::constants::EarSpec,
        lean_degrees: f32,
    ) -> Path {
        let line_scale = ear_line_scale(body, index).clamp(0.84, 1.12);
        let scale = ear.scale * line_scale;
        let lean = lean_degrees.to_radians();
        let along = Vec2::new(0.0, -1.0)
            .rotate(lean)
            .normalized_or(Vec2::new(0.0, -1.0));
        let side = along.perp_left().normalized_or(Vec2::X);
        let root_settle = if index == 0 {
            Vec2::new(-0.8, 2.6) * scale
        } else {
            Vec2::new(0.8, 2.6) * scale
        };
        let root = ear.anchor + root_settle;
        let length = EAR_TIP_LENGTH * scale;
        let width = EAR_LEAF_HALF_WIDTH * scale;
        let tip_side = -4.0 * scale;
        let local = |side_offset: f32, along_offset: f32| -> Vec2 {
            root + side * side_offset + along * along_offset
        };
        let to_world = |p: Vec2| ear_local_to_world(body, ear.anchor, p, index);

        let base_tuck = -6.0 * scale;
        let left_base = local(-width * 0.44, base_tuck);
        let right_base = local(width * 0.52, base_tuck + 4.2 * scale);
        let top_left = local(-width * 0.82, length * 0.87);
        let top_mid = local(tip_side, length);
        let top_right = local(width * 0.72, length * 0.87);
        let left_mid = local(-width * 1.18, length * 0.43);
        let right_mid = local(width * 0.98, length * 0.39);

        let mut p = Path::new();
        p.move_to(to_world(left_base));
        p.cubic_to(to_world(left_mid), to_world(top_left), to_world(top_mid));
        p.cubic_to(to_world(top_right), to_world(right_mid), to_world(right_base));
        p
    }

    fn draw_tail<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody) {
        let wobble = self.tail_wobble(body);
        let scale = body.scale.max(0.2);

        let rear = self.tail_command_path(body, &svg_ref::FINAL_TAIL_REAR_PATH, wobble, false, Vec2::ZERO);
        let light = self.tail_command_path(body, &svg_ref::FINAL_TAIL_LIGHT_PATH, wobble, false, Vec2::ZERO);
        let front = self.tail_command_path(body, &svg_ref::FINAL_TAIL_FRONT_PATH, wobble, false, Vec2::ZERO);
        let stroke_width = FINAL_TAIL_STROKE_LOCAL * scale;

        // Match the user-edited SVG layer order exactly: rear dark plane,
        // light cutout plane, then the foreground lower/front plane.
        canvas.fill_path(&rear, APPENDAGE_FINAL_DARK);
        canvas.stroke_path(&rear, APPENDAGE_FINAL_EDGE, stroke_width);
        canvas.fill_path(&light, APPENDAGE_FINAL_LIGHT);
        canvas.stroke_path(&light, APPENDAGE_FINAL_EDGE, stroke_width);
        canvas.fill_path(&front, APPENDAGE_FINAL_SHADE);
        canvas.stroke_path(&front, APPENDAGE_FINAL_EDGE, stroke_width);
    }

    fn tail_wobble(&self, body: &FushiBody) -> f32 {
        let kin = body.kinematics();
        let x_dir = body.local_vec_to_world(Vec2::X).normalized_or(Vec2::X);
        let local_speed = kin.velocity.dot(x_dir) / body.scale.max(0.2);
        let motion_lag = (-local_speed / 1050.0).clamp(-0.13, 0.13);
        let bank_lag = (body.bank_velocity * 0.030).clamp(-0.28, 0.28);
        let breathing = (body.time * 3.7 + 0.4).sin() * (0.012 + body.stress * 0.036);
        let pet_wag = (body.time * 12.0 + 0.7).sin() * 0.052 * body.appendage_pet_amount;
        let crawl = if body.mode == MotionMode::Attached {
            (kin.crawl_phase + 0.55).sin() * 0.018 * kin.crawl_drive
        } else {
            0.0
        };
        let view_lag = body.view_yaw_velocity * 0.010 + body.view_pitch_velocity * 0.006;
        let angry_tuck = if body.expression == FushiExpression::Angry {
            0.055
        } else {
            0.0
        };
        bank_lag + motion_lag + breathing + crawl + view_lag + pet_wag - angry_tuck
    }

    fn tail_command_path(
        &self,
        body: &FushiBody,
        commands: &[PathCmd],
        extra_rotation: f32,
        inner_shape: bool,
        local_offset: Vec2,
    ) -> Path {
        reference_path_from_commands(commands, |p| {
            self.tail_reference_point_to_world(body, p, extra_rotation, inner_shape, local_offset)
        })
    }

    fn tail_reference_point_to_world(
        &self,
        body: &FushiBody,
        p: Vec2,
        extra_rotation: f32,
        inner_shape: bool,
        local_offset: Vec2,
    ) -> Vec2 {
        let kin = body.kinematics();
        let tail_grab = kin.tail_grab_amount();
        let drag_pull = kin.guarded_drag_pull_local();
        let drag_twist = if tail_grab > 0.001 {
            (drag_pull.y * 0.003 - drag_pull.x * 0.0016).clamp(-0.18, 0.18) * tail_grab
        } else {
            0.0
        };
        let flutter_amount = match body.mode {
            MotionMode::Attached => 0.016,
            MotionMode::Flying => 0.030,
            MotionMode::Dragged => 0.034,
        };
        let shape_scale = if inner_shape { 0.985 } else { 1.0 };
        let view_yaw =
            (view_yaw(body) + body.view_yaw_velocity * 0.012 + body.bank_velocity * 0.002).clamp(-0.32, 0.32);
        let view_pitch = (view_pitch(body) + body.view_pitch_velocity * 0.010).clamp(-0.18, 0.18);
        let view_abs = view_yaw.abs();
        let p = p - TAIL_ANCHOR;
        let radius = p.length();
        let tip_weight = smoothstep(18.0, 88.0, radius);
        let top_weight = 1.0 - smoothstep(-56.0, 12.0, p.y);
        let bottom_weight = smoothstep(8.0, 42.0, p.y);
        let rim_weight = if inner_shape { 0.62 } else { 1.0 };
        let flutter = (body.time * 4.15 + p.x * 0.027 - p.y * 0.018).sin()
            * flutter_amount
            * tip_weight
            * rim_weight
            * (0.42 + body.stress * 0.58);
        let bend =
            extra_rotation * (0.22 + tip_weight * 0.78) + drag_twist * (0.35 + tip_weight * 0.65) + flutter;
        let depth = tail_point_depth(p, inner_shape);
        let crown_weight = top_weight * tip_weight;
        let mut q = Vec2::new(p.x * shape_scale, p.y * shape_scale);
        let foreshorten = 1.0 - view_abs * (0.035 + tip_weight * 0.055);
        q.x *= (1.0 + bottom_weight * 0.032 - top_weight * 0.012) * foreshorten;
        q.y *= 1.0
            + top_weight * 0.012
            + view_pitch * 0.024 * (0.35 + tip_weight)
            + view_abs * 0.010 * crown_weight;
        if inner_shape {
            q.x += view_yaw * 1.6 * (0.60 + tip_weight * 0.40);
            q.y += view_pitch * 1.3;
        }
        let view_turn = if inner_shape { 0.20 } else { 0.34 };
        q = q.rotate(bend + view_yaw * 0.012 * tip_weight * view_turn);
        if tail_grab > 0.001 {
            q += drag_pull.clamp_len(156.0) * (tail_grab * tip_weight * 0.125 * rim_weight);
        }
        let material = body.appendage_sway.clamp_len(64.0);
        q += Vec2::new(material.x * 0.16, material.y * 0.11)
            * (tip_weight * rim_weight * (0.45 + bottom_weight * 0.55));
        if body.appendage_pet_amount > 0.01 && body.mode != MotionMode::Dragged {
            let cursor_local = body.world_to_local(body.cursor_world);
            let tail_local = TAIL_ANCHOR + q;
            let brush_distance = (tail_local - cursor_local).length();
            let brush =
                body.appendage_pet_amount * (1.0 - smoothstep(10.0, 70.0, brush_distance)) * tip_weight;
            if brush > 0.001 {
                let brush_delta = body
                    .world_to_local(body.cursor_world + body.passive_mouse_velocity * 0.016)
                    - cursor_local;
                q += brush_delta.clamp_len(22.0) * (0.045 * brush * rim_weight);
                q.y += (body.time * 11.5 + p.x * 0.015).sin() * 1.1 * brush;
            }
        }
        let local = TAIL_ANCHOR + q + local_offset;
        let depth_px = tail_depth_px(body, q, depth, inner_shape);
        tail_local_to_world(body, local, depth_px)
    }

    fn tail_path(
        &self,
        body: &FushiBody,
        subpaths: &[&[Vec2]],
        extra_rotation: f32,
        inner_shape: bool,
        local_offset: Vec2,
    ) -> Path {
        self.tail_path_impl(
            body,
            subpaths,
            extra_rotation,
            inner_shape,
            local_offset,
            true,
            if inner_shape { 0.092 } else { 0.078 },
        )
    }

    fn tail_edge_path(
        &self,
        body: &FushiBody,
        subpaths: &[&[Vec2]],
        extra_rotation: f32,
        inner_shape: bool,
        local_offset: Vec2,
    ) -> Path {
        self.tail_path_impl(
            body,
            subpaths,
            extra_rotation,
            inner_shape,
            local_offset,
            false,
            0.066,
        )
    }

    fn tail_path_impl(
        &self,
        body: &FushiBody,
        subpaths: &[&[Vec2]],
        extra_rotation: f32,
        inner_shape: bool,
        local_offset: Vec2,
        closed: bool,
        tension: f32,
    ) -> Path {
        let kin = body.kinematics();
        let tail_grab = kin.tail_grab_amount();
        let drag_pull = kin.guarded_drag_pull_local();
        let drag_twist = if tail_grab > 0.001 {
            (drag_pull.y * 0.003 - drag_pull.x * 0.0016).clamp(-0.18, 0.18) * tail_grab
        } else {
            0.0
        };
        let flutter_amount = match body.mode {
            MotionMode::Attached => 0.016,
            MotionMode::Flying => 0.030,
            MotionMode::Dragged => 0.034,
        };
        let shape_scale = if inner_shape { 0.985 } else { 1.0 };
        let view_yaw =
            (view_yaw(body) + body.view_yaw_velocity * 0.012 + body.bank_velocity * 0.002).clamp(-0.32, 0.32);
        let view_pitch = (view_pitch(body) + body.view_pitch_velocity * 0.010).clamp(-0.18, 0.18);
        let view_abs = view_yaw.abs();
        let mut path = Path::new();
        for points in subpaths {
            if (closed && points.len() < 3) || (!closed && points.len() < 2) {
                continue;
            }
            let mut local_points = Vec::with_capacity(points.len());
            for p in *points {
                let p = *p - TAIL_ANCHOR;
                let radius = p.length();
                let tip_weight = smoothstep(18.0, 88.0, radius);
                let top_weight = 1.0 - smoothstep(-56.0, 12.0, p.y);
                let bottom_weight = smoothstep(8.0, 42.0, p.y);
                let rim_weight = if inner_shape { 0.62 } else { 1.0 };
                let flutter = (body.time * 4.15 + p.x * 0.027 - p.y * 0.018).sin()
                    * flutter_amount
                    * tip_weight
                    * rim_weight
                    * (0.42 + body.stress * 0.58);
                let bend = extra_rotation * (0.22 + tip_weight * 0.78)
                    + drag_twist * (0.35 + tip_weight * 0.65)
                    + flutter;
                let depth = tail_point_depth(p, inner_shape);
                let crown_weight = top_weight * tip_weight;
                let mut q = Vec2::new(p.x * shape_scale, p.y * shape_scale);
                // Keep only material deformation here; every SVG tail layer shares the same
                // projected pivot so the stacked drawing stays aligned while turning.
                let foreshorten = 1.0 - view_abs * (0.035 + tip_weight * 0.055);
                q.x *= (1.0 + bottom_weight * 0.032 - top_weight * 0.012) * foreshorten;
                q.y *= 1.0
                    + top_weight * 0.012
                    + view_pitch * 0.024 * (0.35 + tip_weight)
                    + view_abs * 0.010 * crown_weight;
                if inner_shape {
                    q.x += view_yaw * 1.6 * (0.60 + tip_weight * 0.40);
                    q.y += view_pitch * 1.3;
                }
                let view_turn = if inner_shape { 0.20 } else { 0.34 };
                q = q.rotate(bend + view_yaw * 0.012 * tip_weight * view_turn);
                if tail_grab > 0.001 {
                    q += drag_pull.clamp_len(156.0) * (tail_grab * tip_weight * 0.125 * rim_weight);
                }
                let material = body.appendage_sway.clamp_len(64.0);
                q += Vec2::new(material.x * 0.16, material.y * 0.11)
                    * (tip_weight * rim_weight * (0.45 + bottom_weight * 0.55));
                if body.appendage_pet_amount > 0.01 && body.mode != MotionMode::Dragged {
                    let cursor_local = body.world_to_local(body.cursor_world);
                    let tail_local = TAIL_ANCHOR + q;
                    let brush_distance = (tail_local - cursor_local).length();
                    let brush = body.appendage_pet_amount
                        * (1.0 - smoothstep(10.0, 70.0, brush_distance))
                        * tip_weight;
                    if brush > 0.001 {
                        let brush_delta = body
                            .world_to_local(body.cursor_world + body.passive_mouse_velocity * 0.016)
                            - cursor_local;
                        q += brush_delta.clamp_len(22.0) * (0.045 * brush * rim_weight);
                        q.y += (body.time * 11.5 + p.x * 0.015).sin() * 1.1 * brush;
                    }
                }
                let local = TAIL_ANCHOR + q + local_offset;
                let depth_px = tail_depth_px(body, q, depth, inner_shape);
                local_points.push(tail_local_to_world(body, local, depth_px));
            }
            let mut subpath = catmull_rom_path_with_tension(&local_points, closed, tension);
            path.commands.append(&mut subpath.commands);
        }
        path
    }

    fn draw_face<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, face_clip: &[Vec2]) {
        let t = smootherstep(0.0, 1.0, body.expression_transition);
        let leaving_startled = matches!(
            body.previous_expression,
            FushiExpression::Surprised | FushiExpression::Panic
        ) && !matches!(
            body.expression,
            FushiExpression::Surprised | FushiExpression::Panic
        );
        if body.previous_expression != body.expression && t < 0.995 {
            let previous_alpha = if leaving_startled {
                1.0 - smoothstep(0.0, 0.86, t)
            } else {
                1.0 - t
            };
            self.draw_face_expression_with_clip_policy(
                canvas,
                body,
                body.previous_expression,
                previous_alpha,
                face_clip,
            );
        }
        let current_alpha = if body.previous_expression == body.expression {
            1.0
        } else if leaving_startled {
            smoothstep(0.20, 1.0, t)
        } else {
            t
        };
        self.draw_face_expression_with_clip_policy(canvas, body, body.expression, current_alpha, face_clip);
    }

    fn draw_face_expression_with_clip_policy<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        expression: FushiExpression,
        alpha: f32,
        face_clip: &[Vec2],
    ) {
        if alpha <= 0.01 {
            return;
        }
        if face_clip.len() < 3 {
            self.draw_face_expression(canvas, body, expression, alpha);
        } else {
            let mut clipped = FaceClipCanvas::new(canvas, face_clip);
            self.draw_face_expression(&mut clipped, body, expression, alpha);
        }
    }

    fn draw_face_expression<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        expression: FushiExpression,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }
        match expression {
            FushiExpression::Default => self.face_default(canvas, body, alpha),
            FushiExpression::Sleepy => self.face_sleepy(canvas, body, alpha),
            FushiExpression::Surprised => self.face_surprised(canvas, body, alpha),
            FushiExpression::Angry => self.face_angry(canvas, body, alpha),
            FushiExpression::Grumpy => self.face_grumpy(canvas, body, alpha),
            FushiExpression::Panic => self.face_panic(canvas, body, alpha),
            FushiExpression::Dizzy => self.face_dizzy(canvas, body, alpha),
            FushiExpression::Sad => self.face_sad(canvas, body, alpha),
            FushiExpression::Stare => self.face_stare(canvas, body, alpha),
        }
    }

    fn blink_lid_alpha(&self, blink: f32) -> f32 {
        // Keep the eyelid visible long enough to read as animation instead of a
        // one-frame sprite swap.  The input is a closing pulse that decays to 0.
        smoothstep(0.22, 0.86, blink)
    }

    fn blink_open_alpha(&self, blink: f32) -> f32 {
        1.0 - smoothstep(0.18, 0.76, blink)
    }

    #[allow(dead_code)]
    fn draw_eye<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        rx: f32,
        ry: f32,
        alpha: f32,
    ) {
        self.draw_eye_with_highlight(canvas, body, local, Vec2::new(rx, ry), alpha);
    }

    #[allow(dead_code)]
    fn draw_eye_with_highlight<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        radius: Vec2,
        alpha: f32,
    ) {
        let scale = body.scale.max(0.2);
        let (center, x, y) = face_frame(body, local);
        let blink = body.blink_amount;
        let lid_alpha = self.blink_lid_alpha(blink) * alpha;
        let open_alpha = self.blink_open_alpha(blink) * alpha;
        if open_alpha > 0.01 {
            let shape_pulse = self.eye_shape_pulse(body);
            let perspective = face_element_scale(body, local);
            let detail_scale = (radius.x / FACE_OPEN_EYE_RX).clamp(0.55, 1.0);
            let drawn_rx = radius.x * FACE_EYE_SCALE * scale * (1.0 + shape_pulse * 0.075) * perspective.x;
            let drawn_ry = radius.y * FACE_EYE_SCALE * scale * (1.0 - shape_pulse * 0.060) * perspective.y;
            canvas.fill_ellipse(
                Ellipse::new(center, x, y, drawn_rx, drawn_ry),
                with_alpha(DARK, open_alpha),
            );
            if blink < 0.74 && drawn_ry > 3.6 * scale {
                let sx = radius.x / 9.6;
                let sy = radius.y / 10.8;
                let (h1, hx1, hy1) = face_frame(body, local + Vec2::new(-3.2 * sx, -4.6 * sy));
                canvas.fill_ellipse(
                    Ellipse::new(
                        h1,
                        hx1,
                        hy1,
                        3.0 * scale * detail_scale * perspective.x,
                        2.25 * scale * detail_scale * perspective.y,
                    ),
                    with_alpha(WHITE_HILIGHT, open_alpha * 0.96),
                );
                let (h1b, hx1b, hy1b) = face_frame(body, local + Vec2::new(-0.9 * sx, -5.1 * sy));
                canvas.fill_ellipse(
                    Ellipse::new(
                        h1b,
                        hx1b,
                        hy1b,
                        1.45 * scale * detail_scale * perspective.x,
                        1.12 * scale * detail_scale * perspective.y,
                    ),
                    with_alpha(WHITE_HILIGHT, open_alpha * 0.88),
                );
                let (h2, hx2, hy2) = face_frame(body, local + Vec2::new(-5.1 * sx, 3.3 * sy));
                canvas.fill_ellipse(
                    Ellipse::new(
                        h2,
                        hx2,
                        hy2,
                        1.25 * scale * detail_scale * perspective.x,
                        0.96 * scale * detail_scale * perspective.y,
                    ),
                    with_alpha(WHITE_HILIGHT, open_alpha * 0.80),
                );
            }
        }
        if lid_alpha > 0.01 {
            self.draw_closed_eye(canvas, body, local, lid_alpha);
        }
    }

    fn draw_surprised_eye<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        radius: f32,
        alpha: f32,
    ) {
        let scale = body.scale.max(0.2);
        let blink = body.blink_amount;
        let lid_alpha = self.blink_lid_alpha(blink) * alpha;
        let open_alpha = self.blink_open_alpha(blink) * alpha;
        if open_alpha > 0.01 {
            let (center, x_axis, y_axis) = face_frame(body, local);
            let perspective = face_element_scale(body, local);
            let pulse = self.eye_shape_pulse(body);
            let r = radius * FACE_EYE_SCALE * scale * (1.0 + pulse * 0.10);
            let rx = r * perspective.x;
            let ry = r * perspective.y;
            canvas.fill_ellipse(
                Ellipse::new(center, x_axis, y_axis, rx, ry),
                with_alpha(Color::rgba_u8(255, 255, 255, 255), open_alpha),
            );
            canvas.stroke_ellipse(
                Ellipse::new(center, x_axis, y_axis, rx, ry),
                with_alpha(DARK, open_alpha),
                1.65 * scale,
            );
        }
        if lid_alpha > 0.01 {
            self.draw_closed_eye(canvas, body, local, lid_alpha);
        }
    }

    fn draw_dizzy_eye<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        phase: f32,
        alpha: f32,
    ) {
        let scale = body.scale.max(0.2);
        let blink = body.blink_amount;
        let lid_alpha = self.blink_lid_alpha(blink) * alpha;
        let open_alpha = self.blink_open_alpha(blink) * alpha;
        if open_alpha > 0.01 {
            let (center, x_axis, y_axis) = face_frame(body, local);
            let perspective = face_element_scale(body, local);
            let ax = x_axis.normalized_or(Vec2::X);
            let ay = y_axis.normalized_or(Vec2::Y);
            let pulse = self.eye_shape_pulse(body);
            let radius = FACE_OPEN_EYE_RY * FACE_EYE_SCALE * scale * (1.10 + pulse * 0.04);

            canvas.fill_ellipse(
                Ellipse::new(
                    center,
                    x_axis,
                    y_axis,
                    radius * 1.05 * perspective.x,
                    radius * 0.98 * perspective.y,
                ),
                with_alpha(DARK, open_alpha * 0.74),
            );
            canvas.fill_ellipse(
                Ellipse::new(
                    center,
                    x_axis,
                    y_axis,
                    radius * 0.88 * perspective.x,
                    radius * 0.82 * perspective.y,
                ),
                with_alpha(Color::rgba_u8(255, 253, 247, 235), open_alpha),
            );

            let mut points = Vec::with_capacity(34);
            for i in 0..34 {
                let t = i as f32 / 33.0;
                let a = phase + t * std::f32::consts::TAU * 2.05;
                let r = radius * (0.16 + t * 0.74);
                points.push(
                    center + ax * (a.cos() * r * perspective.x) + ay * (a.sin() * r * 0.86 * perspective.y),
                );
            }
            canvas.stroke_path(
                &catmull_rom_path(&points, false),
                with_alpha(DARK, open_alpha),
                1.32 * scale,
            );

            let (highlight, hx, hy) = face_frame(body, local + Vec2::new(-4.0, -4.4));
            canvas.fill_ellipse(
                Ellipse::new(
                    highlight,
                    hx,
                    hy,
                    1.25 * scale * perspective.x,
                    0.95 * scale * perspective.y,
                ),
                with_alpha(WHITE_HILIGHT, open_alpha * 0.70),
            );
        }
        if lid_alpha > 0.01 {
            self.draw_closed_eye(canvas, body, local, lid_alpha);
        }
    }

    #[allow(dead_code)]
    fn draw_cut_eye<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        rx: f32,
        ry: f32,
        inward_slant: f32,
        alpha: f32,
    ) {
        let blink = body.blink_amount;
        let lid_alpha = self.blink_lid_alpha(blink) * alpha;
        let open_alpha = self.blink_open_alpha(blink) * alpha;
        if open_alpha > 0.01 {
            let scale = body.scale.max(0.2);
            let (center, x_axis, y_axis) = face_frame(body, local);
            let cut = self.expression_shape_amount(body, FushiExpression::Angry);
            let pulse = self.eye_shape_pulse(body);
            let perspective = face_element_scale(body, local);
            let rx = rx * FACE_EYE_SCALE * scale * (1.0 + pulse * 0.024) * perspective.x;
            let ry = ry * FACE_EYE_SCALE * scale * (1.0 - pulse * 0.052) * perspective.y;
            let cut = smoothstep(0.0, 1.0, cut);
            let slope = inward_slant * lerp(0.04, 0.12, cut);
            let intercept = lerp(-ry * 0.10, -ry * 0.24, cut);
            let inv_rx2 = 1.0 / (rx * rx);
            let inv_ry2 = 1.0 / (ry * ry);
            let a = inv_rx2 + slope * slope * inv_ry2;
            let b = 2.0 * slope * intercept * inv_ry2;
            let c = intercept * intercept * inv_ry2 - 1.0;
            let disc = (b * b - 4.0 * a * c).max(0.0).sqrt();
            let x0 = ((-b - disc) / (2.0 * a)).clamp(-rx, rx);
            let x1 = ((-b + disc) / (2.0 * a)).clamp(-rx, rx);
            let left = Vec2::new(x0.min(x1), slope * x0.min(x1) + intercept);
            let right = Vec2::new(x0.max(x1), slope * x0.max(x1) + intercept);
            let start = (right.y / ry).atan2(right.x / rx);
            let mut end = (left.y / ry).atan2(left.x / rx);
            while end <= start {
                end += std::f32::consts::TAU;
            }
            let to_world = |p: Vec2| center + x_axis * p.x + y_axis * p.y;

            let mut path = Path::new();
            path.move_to(to_world(left));
            path.line_to(to_world(right));
            for i in 1..=18 {
                let t = i as f32 / 18.0;
                let angle = start + (end - start) * t;
                let p = Vec2::new(angle.cos() * rx, angle.sin() * ry);
                path.line_to(to_world(p));
            }
            path.close();
            canvas.fill_path(&path, with_alpha(DARK, open_alpha));
        }
        if lid_alpha > 0.01 {
            self.draw_closed_eye(canvas, body, local, lid_alpha);
        }
    }

    #[allow(dead_code)]
    fn draw_svg_face_layer<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        subpaths: &[&[Vec2]],
        color: Color,
    ) {
        let path = reference_path_from_subpaths(subpaths, |p| face_local_to_world(body, p));
        canvas.fill_path(&path, color);
    }

    fn draw_svg_face_layer_transformed<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        subpaths: &[&[Vec2]],
        origin: Vec2,
        offset: Vec2,
        scale: Vec2,
        rotation: f32,
        color: Color,
    ) {
        let path = reference_path_from_subpaths(subpaths, |p| {
            let d = p - origin;
            let scaled = Vec2::new(d.x * scale.x, d.y * scale.y).rotate(rotation);
            face_local_to_world(body, origin + scaled + offset)
        });
        canvas.fill_path(&path, color);
    }

    #[allow(dead_code)]
    fn draw_svg_face_layer_warped<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        subpaths: &[&[Vec2]],
        origin: Vec2,
        offset: Vec2,
        scale: Vec2,
        rotation: f32,
        shear_y: f32,
        color: Color,
    ) {
        let path = reference_path_from_subpaths(subpaths, |p| {
            let d = p - origin;
            let warped = Vec2::new(d.x * scale.x, d.y * scale.y + d.x * shear_y).rotate(rotation);
            face_local_to_world(body, origin + warped + offset)
        });
        canvas.fill_path(&path, color);
    }

    fn draw_reference_eye<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        left_eye: bool,
        offset: Vec2,
        scale: Vec2,
        rotation: f32,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }
        let blink = body.blink_amount;
        let lid_alpha = self.blink_lid_alpha(blink) * alpha;
        let open_alpha = self.blink_open_alpha(blink) * alpha;
        let (black, deep, highlight, origin) = if left_eye {
            (
                svg_ref::FACE_DEFAULT_LEFT_EYE_BLACK,
                svg_ref::FACE_DEFAULT_LEFT_EYE_DEEP_BLACK,
                svg_ref::FACE_DEFAULT_LEFT_EYE_HIGHLIGHT,
                FACE_LEFT_EYE,
            )
        } else {
            (
                svg_ref::FACE_DEFAULT_RIGHT_EYE_BLACK,
                svg_ref::FACE_DEFAULT_RIGHT_EYE_DEEP_BLACK,
                svg_ref::FACE_DEFAULT_RIGHT_EYE_HIGHLIGHT,
                FACE_RIGHT_EYE,
            )
        };
        if open_alpha > 0.01 {
            self.draw_svg_face_layer_transformed(
                canvas,
                body,
                black,
                origin,
                offset,
                scale,
                rotation,
                with_alpha(DARK, open_alpha),
            );
            self.draw_svg_face_layer_transformed(
                canvas,
                body,
                deep,
                origin,
                offset,
                scale,
                rotation,
                with_alpha(Color::rgba_u8(3, 3, 3, 255), open_alpha),
            );
            self.draw_svg_face_layer_transformed(
                canvas,
                body,
                highlight,
                origin,
                offset,
                scale,
                rotation,
                with_alpha(WHITE_HILIGHT, open_alpha * 0.96),
            );
        }
        if lid_alpha > 0.01 {
            self.draw_closed_eye(canvas, body, origin + offset, lid_alpha);
        }
    }

    #[allow(dead_code)]
    fn draw_reference_cut_eye<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        left_eye: bool,
        offset: Vec2,
        scale: Vec2,
        rotation: f32,
        shear_y: f32,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }
        let blink = body.blink_amount;
        let lid_alpha = self.blink_lid_alpha(blink) * alpha;
        let open_alpha = self.blink_open_alpha(blink) * alpha;
        let (black, deep, highlight, origin) = if left_eye {
            (
                svg_ref::FACE_DEFAULT_LEFT_EYE_BLACK,
                svg_ref::FACE_DEFAULT_LEFT_EYE_DEEP_BLACK,
                svg_ref::FACE_DEFAULT_LEFT_EYE_HIGHLIGHT,
                FACE_LEFT_EYE,
            )
        } else {
            (
                svg_ref::FACE_DEFAULT_RIGHT_EYE_BLACK,
                svg_ref::FACE_DEFAULT_RIGHT_EYE_DEEP_BLACK,
                svg_ref::FACE_DEFAULT_RIGHT_EYE_HIGHLIGHT,
                FACE_RIGHT_EYE,
            )
        };
        if open_alpha > 0.01 {
            self.draw_svg_face_layer_warped(
                canvas,
                body,
                black,
                origin,
                offset,
                scale,
                rotation,
                shear_y,
                with_alpha(DARK, open_alpha),
            );
            self.draw_svg_face_layer_warped(
                canvas,
                body,
                deep,
                origin,
                offset,
                scale,
                rotation,
                shear_y,
                with_alpha(Color::rgba_u8(3, 3, 3, 255), open_alpha),
            );
            self.draw_svg_face_layer_warped(
                canvas,
                body,
                highlight,
                origin,
                offset,
                scale,
                rotation,
                shear_y,
                with_alpha(WHITE_HILIGHT, open_alpha * 0.58),
            );
        }
        if lid_alpha > 0.01 {
            self.draw_closed_eye(canvas, body, origin + offset, lid_alpha);
        }
    }

    #[allow(dead_code)]
    fn draw_reference_cut_eye_pair<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        left_offset: Vec2,
        right_offset: Vec2,
        scale: Vec2,
        alpha: f32,
    ) {
        self.draw_reference_cut_eye(canvas, body, true, left_offset, scale, 0.05, 0.12, alpha);
        self.draw_reference_cut_eye(canvas, body, false, right_offset, scale, -0.05, -0.12, alpha);
    }

    fn draw_reference_eye_pair<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        left_offset: Vec2,
        right_offset: Vec2,
        scale: Vec2,
        left_rotation: f32,
        right_rotation: f32,
        alpha: f32,
    ) {
        self.draw_reference_eye(canvas, body, true, left_offset, scale, left_rotation, alpha);
        self.draw_reference_eye(canvas, body, false, right_offset, scale, right_rotation, alpha);
    }

    fn draw_unified_face_eyes<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        self.draw_reference_eye(
            canvas,
            body,
            true,
            Vec2::new(1.2, -0.2),
            Vec2::new(1.00, 1.02),
            -0.010,
            alpha * 0.98,
        );
        self.draw_reference_eye(
            canvas,
            body,
            false,
            Vec2::new(2.8, -0.4),
            Vec2::new(1.00, 1.02),
            -0.010,
            alpha,
        );
    }

    fn draw_reference_mouth<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        offset: Vec2,
        scale: Vec2,
        alpha: f32,
    ) {
        self.draw_reference_mouth_pose(canvas, body, offset, scale, 0.0, alpha);
    }

    fn draw_reference_mouth_pose<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        offset: Vec2,
        scale: Vec2,
        rotation: f32,
        alpha: f32,
    ) {
        let origin = FACE_MOUTH_MID;
        let offset = offset + Vec2::new(0.0, -0.45);
        // Draw the black mouth first and then a slightly inset tongue on top.
        // With clipped face decals the old order could bury the red tongue
        // under the filled traced mouth shape at small scales.
        self.draw_svg_face_layer_transformed(
            canvas,
            body,
            svg_ref::FACE_DEFAULT_MOUTH_LINE,
            origin,
            offset,
            scale,
            rotation,
            with_alpha(DARK, alpha),
        );
        self.draw_svg_face_layer_transformed(
            canvas,
            body,
            svg_ref::FACE_DEFAULT_TONGUE,
            origin,
            offset + Vec2::new(0.0, 0.3),
            Vec2::new(scale.x * 0.86, scale.y * 0.84),
            rotation,
            with_alpha(MOUTH_INNER, alpha * 0.96),
        );
    }

    fn draw_mouth_alpha<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        pts: &[Vec2],
        width: f32,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }
        let world: Vec<Vec2> = pts.iter().map(|p| face_local_to_world(body, *p)).collect();
        let path = catmull_rom_path(&world, false);
        canvas.stroke_path(&path, with_alpha(DARK, alpha), width * body.scale.max(0.2));
    }

    #[allow(dead_code)]
    fn draw_surprised_mouth<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        size_scale: Vec2,
        open: f32,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }
        let scale = body.scale.max(0.2);
        let open = smoothstep(0.0, 1.0, open);
        let width_scale = size_scale.x * lerp(0.42, 1.0, open);
        let height_scale = size_scale.y * lerp(0.16, 1.0, open);
        let local = local + Vec2::new(0.0, lerp(4.0, 0.0, open));
        let p =
            |x: f32, y: f32| face_local_to_world(body, local + Vec2::new(x * width_scale, y * height_scale));
        let mut path = Path::new();
        path.move_to(p(-16.5, -3.0));
        path.cubic_to(p(-15.2, -12.5), p(-6.2, -14.2), p(-0.6, -6.0));
        path.cubic_to(p(4.8, 1.7), p(11.8, -14.0), p(17.2, -8.0));
        path.cubic_to(p(22.2, -2.2), p(19.6, 10.8), p(14.8, 15.0));
        path.cubic_to(p(7.2, 11.8), p(-6.4, 11.8), p(-14.8, 15.0));
        path.cubic_to(p(-19.4, 9.8), p(-21.2, 1.5), p(-16.5, -3.0));
        path.close();

        canvas.fill_path(
            &path,
            with_alpha(MOUTH_INNER, alpha * smoothstep(0.16, 0.48, open)),
        );
        canvas.stroke_path(&path, with_alpha(DARK, alpha), 1.90 * scale);
    }

    fn surprise_mouth_open(&self, body: &FushiBody, expression: FushiExpression) -> f32 {
        if body.expression == expression {
            if matches!(
                body.previous_expression,
                FushiExpression::Surprised | FushiExpression::Panic
            ) {
                1.0
            } else {
                body.expression_transition
            }
        } else if body.previous_expression == expression {
            1.0 - body.expression_transition
        } else {
            1.0
        }
    }

    fn draw_closed_eye<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, local: Vec2, alpha: f32) {
        let closed_scale = 0.80;
        let pts = [
            local + Vec2::new(-9.1 * closed_scale, -0.4),
            local + Vec2::new(-4.2 * closed_scale, 1.0),
            local + Vec2::new(4.2 * closed_scale, 1.0),
            local + Vec2::new(9.1 * closed_scale, -0.4),
        ];
        let world: Vec<Vec2> = pts.iter().map(|p| face_local_to_world(body, *p)).collect();
        let mut path = Path::new();
        path.move_to(world[0]);
        path.cubic_to(world[1], world[2], world[3]);
        canvas.stroke_path(&path, with_alpha(DARK, alpha), 2.18 * body.scale.max(0.2));
    }

    fn draw_blush<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody) {
        let alpha = self.blush_alpha(body);
        if alpha <= 0.02 {
            return;
        }

        let scale = body.scale.max(0.2);
        // Soft layered blush: broad transparent peach haze + a warmer core.
        // The two cheeks are intentionally asymmetric so the blush reads like
        // color under fur instead of a stamped pair of circles.
        let cheeks = [
            (
                Vec2::new(-154.0, 18.4),
                Vec2::new(25.8, 17.4),
                Vec2::new(-1.8, 0.7),
                -0.08_f32,
            ),
            (
                Vec2::new(-96.0, 24.2),
                Vec2::new(24.2, 16.0),
                Vec2::new(1.5, -0.4),
                0.06_f32,
            ),
        ];
        for (center_local, radius, warm_offset, rotation) in cheeks {
            let perspective = face_element_scale(body, center_local);
            let rot = rotation + view_yaw(body) * 0.018;
            let (center, x0, y0) = face_frame(body, center_local);
            let x = x0.rotate(rot);
            let y = y0.rotate(rot);
            let soft = Color::rgba_u8(255, 190, 184, 255);
            let warm = Color::rgba_u8(255, 155, 160, 255);
            let line = Color::rgba_u8(222, 104, 116, 255);

            canvas.fill_ellipse(
                Ellipse::new(
                    center,
                    x,
                    y,
                    radius.x * 1.20 * scale * perspective.x,
                    radius.y * 1.10 * scale * perspective.y,
                ),
                with_alpha(soft, alpha * 0.075),
            );
            canvas.fill_ellipse(
                Ellipse::new(
                    center + x * (warm_offset.x * scale) + y * (warm_offset.y * scale),
                    x,
                    y,
                    radius.x * 0.82 * scale * perspective.x,
                    radius.y * 0.76 * scale * perspective.y,
                ),
                with_alpha(warm, alpha * 0.135),
            );
            canvas.fill_ellipse(
                Ellipse::new(
                    center + x * ((warm_offset.x + 1.1) * scale) + y * ((warm_offset.y - 0.5) * scale),
                    x,
                    y,
                    radius.x * 0.52 * scale * perspective.x,
                    radius.y * 0.46 * scale * perspective.y,
                ),
                with_alpha(warm, alpha * 0.075),
            );

            // Only a hint of diagonal blush texture remains; at low scale the old
            // lines looked like scratches and made the cheeks less natural.
            for (i, offset) in [-5.8_f32, 1.2, 7.4].iter().enumerate() {
                let len = if i == 1 { 5.0 } else { 3.8 };
                let a = center_local + Vec2::new(*offset - len * 0.30, -2.0 - i as f32 * 0.15);
                let b = center_local + Vec2::new(*offset + len * 0.30, 2.0 + i as f32 * 0.10);
                canvas.draw_line(
                    face_local_to_world(body, a),
                    face_local_to_world(body, b),
                    with_alpha(line, alpha * 0.045),
                    0.62 * scale,
                );
            }
        }
    }

    fn blush_alpha(&self, body: &FushiBody) -> f32 {
        let calm_gate = 1.0
            - smoothstep(
                0.16,
                0.58,
                body.anger
                    .max(body.stress * 0.9)
                    .max(body.sadness * 0.8)
                    .max(body.dizziness * 0.8),
            );
        let mode_gate = match body.mode {
            MotionMode::Attached => 1.0,
            MotionMode::Dragged => 0.34,
            MotionMode::Flying => 0.0,
        };
        let unresolved_upset = smoothstep(0.18, 0.55, body.anger.max(body.stress * 0.82));
        let hover_warmth = smoothstep(0.28, 0.92, body.hover_amount) * 0.42 * (1.0 - unresolved_upset * 0.72);
        let pet_warmth = smoothstep(0.14, 0.90, body.petting_amount) * 0.86 * (1.0 - unresolved_upset * 0.42);
        let happy_warmth = smoothstep(0.22, 0.86, body.happiness) * 0.58;
        let cozy_warmth = smoothstep(0.42, 0.92, body.sleepiness) * 0.24;
        let expression_warmth = self.expression_alpha(body, FushiExpression::Stare) * 0.72
            + self.expression_alpha(body, FushiExpression::Surprised) * 0.32
            + self.expression_alpha(body, FushiExpression::Sleepy) * 0.22
            + self.expression_alpha(body, FushiExpression::Default) * 0.10;

        ((hover_warmth + pet_warmth + happy_warmth + cozy_warmth).max(expression_warmth)
            * calm_gate
            * mode_gate)
            .clamp(0.0, 1.0)
    }

    fn face_default<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }

        self.draw_unified_face_eyes(canvas, body, alpha);
        self.draw_reference_mouth_pose(
            canvas,
            body,
            Vec2::new(1.4, -1.3),
            Vec2::new(1.02, 0.98),
            -0.010,
            alpha,
        );
    }

    fn draw_peak_mouth<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        left: Vec2,
        mid: Vec2,
        right: Vec2,
        width: f32,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }

        let mut path = Path::new();
        path.move_to(face_local_to_world(body, left));
        path.line_to(face_local_to_world(body, mid));
        path.line_to(face_local_to_world(body, right));
        canvas.stroke_path(&path, with_alpha(DARK, alpha), width * body.scale.max(0.2));
    }

    fn draw_soft_peak_mouth<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        center: Vec2,
        half_width: f32,
        lift: f32,
        drop: f32,
        width: f32,
        alpha: f32,
    ) {
        if alpha <= 0.01 {
            return;
        }

        let left = center + Vec2::new(-half_width, drop);
        let crest_half_width = (half_width * 0.28).clamp(2.4, 4.0);
        let crest_left = center + Vec2::new(-crest_half_width, -lift);
        let crest_right = center + Vec2::new(crest_half_width, -lift);
        let right = center + Vec2::new(half_width, drop);
        let mut path = Path::new();
        path.move_to(face_local_to_world(body, left));
        path.cubic_to(
            face_local_to_world(body, left + Vec2::new(half_width * 0.38, -lift * 0.42)),
            face_local_to_world(body, crest_left + Vec2::new(-half_width * 0.18, 0.12)),
            face_local_to_world(body, crest_left),
        );
        path.cubic_to(
            face_local_to_world(body, crest_right + Vec2::new(half_width * 0.18, 0.12)),
            face_local_to_world(body, right + Vec2::new(-half_width * 0.38, -lift * 0.42)),
            face_local_to_world(body, right),
        );
        canvas.stroke_path(&path, with_alpha(DARK, alpha), width * body.scale.max(0.2));
    }

    fn face_sleepy<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        self.draw_unified_face_eyes(canvas, body, alpha * 0.92);
        self.draw_soft_peak_mouth(
            canvas,
            body,
            FACE_MOUTH_MID + Vec2::new(0.0, -0.8),
            9.2,
            1.7,
            0.9,
            1.45,
            alpha,
        );
    }

    fn face_surprised<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        self.draw_surprised_eye(
            canvas,
            body,
            FACE_LEFT_EYE + Vec2::new(0.0, -1.0),
            FACE_OPEN_EYE_RY,
            alpha,
        );
        self.draw_surprised_eye(
            canvas,
            body,
            FACE_RIGHT_EYE + Vec2::new(0.0, -1.0),
            FACE_OPEN_EYE_RY,
            alpha,
        );
        self.draw_surprised_mouth(
            canvas,
            body,
            FACE_MOUTH_MID + Vec2::new(0.0, -1.4),
            Vec2::new(0.88, 0.82),
            self.surprise_mouth_open(body, FushiExpression::Surprised),
            alpha,
        );
    }

    fn face_angry<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        self.draw_cut_eye(
            canvas,
            body,
            FACE_LEFT_EYE + Vec2::new(0.0, 0.4),
            FACE_OPEN_EYE_RX,
            FACE_OPEN_EYE_RY,
            1.0,
            alpha,
        );
        self.draw_cut_eye(
            canvas,
            body,
            FACE_RIGHT_EYE + Vec2::new(0.0, 0.4),
            FACE_OPEN_EYE_RX,
            FACE_OPEN_EYE_RY,
            -1.0,
            alpha,
        );

        self.draw_peak_mouth(
            canvas,
            body,
            FACE_MOUTH_LEFT + Vec2::new(0.0, 1.9),
            FACE_MOUTH_MID + Vec2::new(0.0, -6.6),
            FACE_MOUTH_RIGHT + Vec2::new(0.0, 1.7),
            1.90,
            alpha,
        );
    }

    fn face_grumpy<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        self.draw_unified_face_eyes(canvas, body, alpha);
        self.draw_soft_peak_mouth(
            canvas,
            body,
            FACE_MOUTH_MID + Vec2::new(0.0, 0.2),
            12.2,
            2.8,
            1.3,
            1.58,
            alpha,
        );
    }

    fn face_panic<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        let wobble = (body.time * 28.0).sin() * 1.8;
        self.draw_surprised_eye(
            canvas,
            body,
            FACE_LEFT_EYE + Vec2::new(wobble, -2.0),
            FACE_OPEN_EYE_RY,
            alpha,
        );
        self.draw_surprised_eye(
            canvas,
            body,
            FACE_RIGHT_EYE + Vec2::new(-wobble, 0.0),
            FACE_OPEN_EYE_RY,
            alpha,
        );
        self.draw_surprised_mouth(
            canvas,
            body,
            FACE_MOUTH_MID + Vec2::new(0.0, -0.8),
            Vec2::new(1.02, 0.96),
            self.surprise_mouth_open(body, FushiExpression::Panic),
            alpha,
        );
    }

    fn face_dizzy<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        let phase = body.time * 12.0 + body.bank * 2.2;
        self.draw_dizzy_eye(canvas, body, FACE_LEFT_EYE + Vec2::new(0.0, -0.6), phase, alpha);
        self.draw_dizzy_eye(
            canvas,
            body,
            FACE_RIGHT_EYE + Vec2::new(0.0, -0.6),
            -phase * 0.94,
            alpha,
        );
        self.draw_surprised_mouth(
            canvas,
            body,
            FACE_MOUTH_MID + Vec2::new(0.0, -1.4),
            Vec2::new(0.88, 0.82),
            self.surprise_mouth_open(body, FushiExpression::Dizzy),
            alpha,
        );
    }

    fn face_sad<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        self.draw_unified_face_eyes(canvas, body, alpha);
        self.draw_soft_peak_mouth(
            canvas,
            body,
            FACE_MOUTH_MID + Vec2::new(0.0, 0.5),
            12.4,
            1.8,
            1.5,
            1.52,
            alpha,
        );
        self.draw_teardrop(canvas, body, FACE_LEFT_EYE + Vec2::new(-12.5, 12.0), 5.8, alpha);
        if body.sadness > 0.65 {
            self.draw_teardrop(canvas, body, FACE_RIGHT_EYE + Vec2::new(12.5, 12.0), 5.0, alpha);
        }
    }

    fn face_stare<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        self.draw_unified_face_eyes(canvas, body, alpha);
        let local_cursor = body.world_to_local(body.cursor_world);
        let aim = Vec2::new(
            (local_cursor.x + 130.0).clamp(-8.0, 8.0),
            (local_cursor.y - 22.0).clamp(-6.0, 6.0),
        ) * 0.25;
        let scale = body.scale.max(0.2);
        let (left_center, left_x, left_y) = face_frame(body, FACE_LEFT_EYE + Vec2::new(-3.0, -4.0) + aim);
        canvas.fill_ellipse(
            Ellipse::new(left_center, left_x, left_y, 1.85 * scale, 1.85 * scale),
            with_alpha(WHITE_HILIGHT, alpha),
        );
        let (right_center, right_x, right_y) = face_frame(body, FACE_RIGHT_EYE + Vec2::new(-3.0, -4.0) + aim);
        canvas.fill_ellipse(
            Ellipse::new(right_center, right_x, right_y, 1.85 * scale, 1.85 * scale),
            with_alpha(WHITE_HILIGHT, alpha),
        );
        // Petting/hovering should read as happy, not as a shy ^-mouth.
        self.draw_reference_mouth_pose(
            canvas,
            body,
            Vec2::new(1.4, -1.2),
            Vec2::new(1.05, 1.00),
            -0.008,
            alpha,
        );
    }

    fn draw_teardrop<C: VectorCanvas>(
        &self,
        canvas: &mut C,
        body: &FushiBody,
        local: Vec2,
        radius: f32,
        alpha: f32,
    ) {
        let scale = body.scale.max(0.2);
        let top = face_local_to_world(body, local + Vec2::new(0.0, -radius * 0.75));
        let left = face_local_to_world(body, local + Vec2::new(-radius * 0.62, radius * 0.08));
        let bottom = face_local_to_world(body, local + Vec2::new(0.0, radius * 1.08));
        let right = face_local_to_world(body, local + Vec2::new(radius * 0.62, radius * 0.08));
        let mut path = Path::new();
        path.move_to(top);
        path.cubic_to(
            face_local_to_world(body, local + Vec2::new(-radius * 0.70, -radius * 0.32)),
            left,
            bottom,
        );
        path.cubic_to(
            right,
            face_local_to_world(body, local + Vec2::new(radius * 0.70, -radius * 0.32)),
            top,
        );
        path.close();
        canvas.fill_path(&path, with_alpha(TEAR_BLUE, alpha));
        canvas.stroke_path(
            &path,
            with_alpha(Color::rgba_u8(72, 128, 210, 170), alpha),
            0.9 * scale,
        );
    }

    fn draw_comic_effects<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody) {
        let angry_alpha = self.expression_alpha(body, FushiExpression::Angry);
        if angry_alpha > 0.02 {
            self.draw_anger_mark(canvas, body, angry_alpha);
        }
    }

    fn startled_quill_alpha(&self, body: &FushiBody) -> f32 {
        let shape_alpha = 1.0 - render_drag_shape_guard(body) * 0.88;
        let current_startled = matches!(
            body.expression,
            FushiExpression::Surprised | FushiExpression::Panic
        );
        if current_startled {
            return shape_alpha;
        }

        let previous_startled = matches!(
            body.previous_expression,
            FushiExpression::Surprised | FushiExpression::Panic
        );
        if previous_startled {
            (1.0 - smoothstep(0.0, 1.0, body.expression_transition)) * shape_alpha
        } else {
            0.0
        }
    }

    fn expression_alpha(&self, body: &FushiBody, expression: FushiExpression) -> f32 {
        let t = smoothstep(0.0, 1.0, body.expression_transition);
        if body.expression == expression {
            if body.previous_expression == body.expression {
                1.0
            } else {
                t
            }
        } else if body.previous_expression == expression {
            1.0 - t
        } else {
            0.0
        }
    }

    fn expression_shape_amount(&self, body: &FushiBody, expression: FushiExpression) -> f32 {
        self.expression_alpha(body, expression)
    }

    fn eye_shape_pulse(&self, body: &FushiBody) -> f32 {
        if body.previous_expression == body.expression {
            return 0.0;
        }

        let t = smoothstep(0.0, 1.0, body.expression_transition);
        (t * std::f32::consts::PI).sin()
    }

    fn draw_anger_mark<C: VectorCanvas>(&self, canvas: &mut C, body: &FushiBody, alpha: f32) {
        let (base, x, y) = face_frame(body, FACE_RIGHT_EYE + Vec2::new(40.0, -8.0));
        let scale = body.scale.max(0.2);
        let to_world = |p: Vec2| {
            let p = (p * 0.78).rotate(10.0_f32.to_radians());
            base + x * (p.x * scale) + y * (p.y * scale)
        };
        let segments = [
            [Vec2::new(-19.0, -7.0), Vec2::new(-8.0, -7.0)],
            [Vec2::new(-7.0, -19.0), Vec2::new(-7.0, -8.0)],
            [Vec2::new(7.0, -19.0), Vec2::new(7.0, -8.0)],
            [Vec2::new(19.0, -7.0), Vec2::new(8.0, -7.0)],
            [Vec2::new(19.0, 7.0), Vec2::new(8.0, 7.0)],
            [Vec2::new(7.0, 19.0), Vec2::new(7.0, 8.0)],
            [Vec2::new(-7.0, 19.0), Vec2::new(-7.0, 8.0)],
            [Vec2::new(-19.0, 7.0), Vec2::new(-8.0, 7.0)],
        ];

        for segment in segments {
            canvas.draw_line(
                to_world(segment[0]),
                to_world(segment[1]),
                with_alpha(Color::rgba_u8(88, 28, 32, 135), alpha),
                4.0 * scale,
            );
        }
        for segment in segments {
            canvas.draw_line(
                to_world(segment[0]),
                to_world(segment[1]),
                with_alpha(ANGER_RED, alpha),
                2.35 * scale,
            );
        }
    }
}

struct FaceClipCanvas<'a, C: VectorCanvas> {
    inner: &'a mut C,
    clip: &'a [Vec2],
}

impl<'a, C: VectorCanvas> FaceClipCanvas<'a, C> {
    fn new(inner: &'a mut C, clip: &'a [Vec2]) -> Self {
        Self { inner, clip }
    }
}

impl<'a, C: VectorCanvas> VectorCanvas for FaceClipCanvas<'a, C> {
    fn fill_path(&mut self, path: &Path, color: Color) {
        if color.a <= 0.0 || self.clip.len() < 3 {
            return;
        }
        for contour in flatten_path_contours(path, 12) {
            if !contour.closed || contour.points.len() < 3 {
                continue;
            }
            let clipped = clip_polygon_to_convex(&contour.points, self.clip);
            if clipped.len() >= 3 {
                self.inner.fill_path(&polygon_path(&clipped, true), color);
            }
        }
    }

    fn fill_path_linear_gradient(&mut self, path: &Path, start: Vec2, end: Vec2, stops: &[GradientStop]) {
        if self.clip.len() < 3 {
            return;
        }
        for contour in flatten_path_contours(path, 12) {
            if !contour.closed || contour.points.len() < 3 {
                continue;
            }
            let clipped = clip_polygon_to_convex(&contour.points, self.clip);
            if clipped.len() >= 3 {
                self.inner
                    .fill_path_linear_gradient(&polygon_path(&clipped, true), start, end, stops);
            }
        }
    }

    fn stroke_path(&mut self, path: &Path, color: Color, width: f32) {
        if color.a <= 0.0 || width <= 0.0 || self.clip.len() < 3 {
            return;
        }
        for contour in flatten_path_contours(path, 12) {
            if contour.points.len() < 2 {
                continue;
            }
            for pair in contour.points.windows(2) {
                if let Some((a, b)) = clip_segment_to_convex(pair[0], pair[1], self.clip) {
                    if (b - a).length_sq() > 0.01 {
                        self.inner.draw_line(a, b, color, width);
                    }
                }
            }
            if contour.closed && contour.points.len() > 2 {
                let a = contour.points[contour.points.len() - 1];
                let b = contour.points[0];
                if let Some((a, b)) = clip_segment_to_convex(a, b, self.clip) {
                    if (b - a).length_sq() > 0.01 {
                        self.inner.draw_line(a, b, color, width);
                    }
                }
            }
        }
    }

    fn fill_ellipse(&mut self, ellipse: Ellipse, color: Color) {
        if color.a <= 0.0 || self.clip.len() < 3 {
            return;
        }
        let subject = ellipse_points(ellipse, 36);
        let clipped = clip_polygon_to_convex(&subject, self.clip);
        if clipped.len() >= 3 {
            self.inner.fill_path(&polygon_path(&clipped, true), color);
        }
    }

    fn stroke_ellipse(&mut self, ellipse: Ellipse, color: Color, width: f32) {
        if color.a <= 0.0 || width <= 0.0 || self.clip.len() < 3 {
            return;
        }
        let pts = ellipse_points(ellipse, 36);
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            if let Some((a, b)) = clip_segment_to_convex(a, b, self.clip) {
                if (b - a).length_sq() > 0.01 {
                    self.inner.draw_line(a, b, color, width);
                }
            }
        }
    }

    fn draw_line(&mut self, a: Vec2, b: Vec2, color: Color, width: f32) {
        if color.a <= 0.0 || width <= 0.0 || self.clip.len() < 3 {
            return;
        }
        if let Some((a, b)) = clip_segment_to_convex(a, b, self.clip) {
            if (b - a).length_sq() > 0.01 {
                self.inner.draw_line(a, b, color, width);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct FlattenedContour {
    points: Vec<Vec2>,
    closed: bool,
}

fn reference_path_from_commands<F>(commands: &[PathCmd], mut transform: F) -> Path
where
    F: FnMut(Vec2) -> Vec2,
{
    let mut path = Path::new();
    for cmd in commands {
        match *cmd {
            PathCmd::MoveTo(p) => path.move_to(transform(p)),
            PathCmd::LineTo(p) => path.line_to(transform(p)),
            PathCmd::CubicTo(c1, c2, p) => {
                path.cubic_to(transform(c1), transform(c2), transform(p));
            }
            PathCmd::Close => path.close(),
        }
    }
    path
}

fn reference_path_from_subpaths<F>(subpaths: &[&[Vec2]], mut transform: F) -> Path
where
    F: FnMut(Vec2) -> Vec2,
{
    let mut path = Path::new();
    for points in subpaths {
        if points.len() < 3 {
            continue;
        }
        path.move_to(transform(points[0]));
        for p in &points[1..] {
            path.line_to(transform(*p));
        }
        path.close();
    }
    path
}

fn dedupe_polygon(points: Vec<Vec2>) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(points.len());
    for p in points {
        if out
            .last()
            .map(|last: &Vec2| (*last - p).length_sq() < 0.05)
            .unwrap_or(false)
        {
            continue;
        }
        out.push(p);
    }
    if out.len() > 1 && (out[0] - out[out.len() - 1]).length_sq() < 0.05 {
        out.pop();
    }
    out
}

fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() <= 3 {
        return points.to_vec();
    }

    let mut pts = points.to_vec();
    pts.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    pts.dedup_by(|a, b| (*a - *b).length_sq() < 0.01);
    if pts.len() <= 3 {
        return pts;
    }

    let mut lower: Vec<Vec2> = Vec::new();
    for p in &pts {
        while lower.len() >= 2 {
            let n = lower.len();
            if (lower[n - 1] - lower[n - 2]).cross(*p - lower[n - 1]) > 0.0 {
                break;
            }
            lower.pop();
        }
        lower.push(*p);
    }

    let mut upper: Vec<Vec2> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 {
            let n = upper.len();
            if (upper[n - 1] - upper[n - 2]).cross(*p - upper[n - 1]) > 0.0 {
                break;
            }
            upper.pop();
        }
        upper.push(*p);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    dedupe_polygon(lower)
}

fn polygon_path(points: &[Vec2], closed: bool) -> Path {
    let mut path = Path::new();
    if let Some(first) = points.first().copied() {
        path.move_to(first);
        for p in &points[1..] {
            path.line_to(*p);
        }
        if closed {
            path.close();
        }
    }
    path
}

fn ellipse_points(ellipse: Ellipse, segments: usize) -> Vec<Vec2> {
    let n = segments.max(8);
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        pts.push(
            ellipse.center
                + ellipse.axis_x * (a.cos() * ellipse.rx)
                + ellipse.axis_y * (a.sin() * ellipse.ry),
        );
    }
    pts
}

fn flatten_path_contours(path: &Path, cubic_steps: usize) -> Vec<FlattenedContour> {
    let mut contours = Vec::new();
    let mut points: Vec<Vec2> = Vec::new();
    let mut current = Vec2::ZERO;
    let mut has_current = false;
    let mut closed = false;
    let steps = cubic_steps.max(3);

    fn finish_contour(contours: &mut Vec<FlattenedContour>, points: &mut Vec<Vec2>, closed: bool) {
        let pts = dedupe_polygon(std::mem::take(points));
        let min_len = if closed { 3 } else { 2 };
        if pts.len() >= min_len {
            contours.push(FlattenedContour { points: pts, closed });
        }
    }

    for command in &path.commands {
        match *command {
            PathCmd::MoveTo(p) => {
                if !points.is_empty() {
                    finish_contour(&mut contours, &mut points, closed);
                }
                points.push(p);
                current = p;
                has_current = true;
                closed = false;
            }
            PathCmd::LineTo(p) => {
                if has_current {
                    points.push(p);
                    current = p;
                }
            }
            PathCmd::CubicTo(c1, c2, p) => {
                if has_current {
                    let from = current;
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        points.push(cubic_point(from, c1, c2, p, t));
                    }
                    current = p;
                }
            }
            PathCmd::Close => {
                if has_current {
                    closed = true;
                    finish_contour(&mut contours, &mut points, true);
                    has_current = false;
                    closed = false;
                }
            }
        }
    }
    if !points.is_empty() {
        finish_contour(&mut contours, &mut points, closed);
    }
    contours
}

fn clip_polygon_to_convex(subject: &[Vec2], clip: &[Vec2]) -> Vec<Vec2> {
    if subject.len() < 3 || clip.len() < 3 {
        return Vec::new();
    }
    let orientation = if signed_polygon_area(clip) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut output = subject.to_vec();
    for i in 0..clip.len() {
        if output.is_empty() {
            break;
        }
        let a = clip[i];
        let b = clip[(i + 1) % clip.len()];
        let input = output;
        output = Vec::new();
        let mut prev = input[input.len() - 1];
        let mut prev_signed = clip_edge_signed(a, b, prev, orientation);
        let mut prev_inside = prev_signed >= -0.01;
        for current in input {
            let current_signed = clip_edge_signed(a, b, current, orientation);
            let current_inside = current_signed >= -0.01;
            if current_inside {
                if !prev_inside {
                    output.push(line_halfplane_intersection(
                        prev,
                        current,
                        prev_signed,
                        current_signed,
                    ));
                }
                output.push(current);
            } else if prev_inside {
                output.push(line_halfplane_intersection(
                    prev,
                    current,
                    prev_signed,
                    current_signed,
                ));
            }
            prev = current;
            prev_signed = current_signed;
            prev_inside = current_inside;
        }
        output = dedupe_polygon(output);
    }
    dedupe_polygon(output)
}

fn clip_segment_to_convex(a: Vec2, b: Vec2, clip: &[Vec2]) -> Option<(Vec2, Vec2)> {
    if clip.len() < 3 {
        return None;
    }
    let orientation = if signed_polygon_area(clip) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut t0: f32 = 0.0;
    let mut t1: f32 = 1.0;
    for i in 0..clip.len() {
        let e0 = clip[i];
        let e1 = clip[(i + 1) % clip.len()];
        let s0 = clip_edge_signed(e0, e1, a, orientation);
        let s1 = clip_edge_signed(e0, e1, b, orientation);
        let inside0 = s0 >= -0.01;
        let inside1 = s1 >= -0.01;
        if inside0 && inside1 {
            continue;
        }
        if !inside0 && !inside1 {
            return None;
        }
        let t = (s0 / (s0 - s1)).clamp(0.0, 1.0);
        if !inside0 && inside1 {
            t0 = t0.max(t);
        } else if inside0 && !inside1 {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    let d = b - a;
    Some((a + d * t0, a + d * t1))
}

fn clip_edge_signed(a: Vec2, b: Vec2, p: Vec2, orientation: f32) -> f32 {
    orientation * (b - a).cross(p - a)
}

fn line_halfplane_intersection(a: Vec2, b: Vec2, signed_a: f32, signed_b: f32) -> Vec2 {
    let denom = signed_a - signed_b;
    if denom.abs() <= 0.0001 {
        return a;
    }
    a + (b - a) * (signed_a / denom).clamp(0.0, 1.0)
}

fn flatten_closed_path(path: &Path, cubic_steps: usize) -> Vec<Vec2> {
    let mut points = Vec::new();
    let mut current = Vec2::ZERO;
    let mut start = Vec2::ZERO;
    let mut has_current = false;
    let steps = cubic_steps.max(3);

    for command in &path.commands {
        match *command {
            PathCmd::MoveTo(p) => {
                current = p;
                start = p;
                has_current = true;
                points.push(p);
            }
            PathCmd::LineTo(p) => {
                if has_current {
                    points.push(p);
                    current = p;
                }
            }
            PathCmd::CubicTo(c1, c2, p) => {
                if has_current {
                    let from = current;
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        points.push(cubic_point(from, c1, c2, p, t));
                    }
                    current = p;
                }
            }
            PathCmd::Close => {
                if has_current && (current - start).length_sq() > 0.05 {
                    points.push(start);
                }
            }
        }
    }
    dedupe_polygon(points)
}

fn cubic_point(a: Vec2, b: Vec2, c: Vec2, d: Vec2, t: f32) -> Vec2 {
    let mt = 1.0 - t;
    a * (mt * mt * mt) + b * (3.0 * mt * mt * t) + c * (3.0 * mt * t * t) + d * (t * t * t)
}

fn ellipse_keep_scale_in_polygon(center: Vec2, rx: f32, ry: f32, polygon: &[Vec2]) -> f32 {
    if polygon.len() < 3 {
        return 1.0;
    }

    // Do not recenter the spot every frame. Re-centering was the reason the markings appeared
    // to jump around while crawling/turning. Keep the projected center stable and only shrink/fade
    // the spot as it approaches or crosses the visible body silhouette.
    let radius = rx.max(ry).max(1.0);
    let edge_distance = min_distance_to_polygon_edges(center, polygon);
    let signed_distance = if point_in_polygon(center, polygon) {
        edge_distance
    } else {
        -edge_distance
    };

    smoothstep(-radius * 0.20, radius * 0.82, signed_distance)
}

fn point_in_polygon(p: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[j];
        if (a.y > p.y) != (b.y > p.y) {
            let x = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn min_distance_to_polygon_edges(p: Vec2, polygon: &[Vec2]) -> f32 {
    if polygon.len() < 2 {
        return 0.0;
    }
    let mut min_d = f32::MAX;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        min_d = min_d.min(point_segment_distance(p, a, b));
    }
    min_d
}

fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let denom = ab.length_sq();
    if denom <= 0.0001 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / denom).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

fn polygon_centroid(polygon: &[Vec2]) -> Vec2 {
    if polygon.is_empty() {
        return Vec2::ZERO;
    }
    let mut sum = Vec2::ZERO;
    for p in polygon {
        sum += *p;
    }
    sum / polygon.len() as f32
}

fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha.clamp(0.0, 1.0),
        ..color
    }
}

fn quill_noise(index: usize, salt: u32) -> f32 {
    let mut x = (index as u32).wrapping_mul(747_796_405).wrapping_add(salt);
    x = (x ^ (x >> 16)).wrapping_mul(2_246_822_519);
    x = (x ^ (x >> 13)).wrapping_mul(3_266_489_917);
    let x = x ^ (x >> 16);
    x as f32 / u32::MAX as f32
}

fn signed_polygon_area(points: &[Vec2]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a.cross(b);
    }
    area * 0.5
}

fn mesh_frame(body: &FushiBody, local: Vec2, _z_hint: f32) -> (Vec2, Vec2, Vec2) {
    let kin = body.kinematics();
    let stable = kin.local_to_world(local);
    let mut displacement = Vec2::ZERO;
    let mut weight_sum = 0.0;

    for node in &body.mesh.nodes {
        let rest_world = kin.local_to_world(node.rest);
        let d2 = (node.rest - local).length_sq();
        let weight = 1.0 / (d2 + 1450.0);
        displacement += (node.pos - rest_world) * weight;
        weight_sum += weight;
    }

    let soft_weight = match body.mode {
        MotionMode::Attached => 0.78,
        MotionMode::Flying => 0.62,
        MotionMode::Dragged => 0.68,
    } * (1.0 - render_drag_shape_guard(body) * 0.20);
    let mesh_displacement = if weight_sum > 0.0001 {
        displacement / weight_sum
    } else {
        Vec2::ZERO
    };

    (
        stable + mesh_displacement * soft_weight,
        kin.local_vec_to_world(Vec2::X),
        kin.local_vec_to_world(Vec2::Y),
    )
}

const BODY_DEPTH_RADIUS: f32 = 62.0;
const BODY_CAMERA_DISTANCE: f32 = 1040.0;
const SURFACE_AXIS_DELTA: f32 = 1.8;

fn view_yaw(body: &FushiBody) -> f32 {
    (body.view_yaw + body.view_yaw_velocity * 0.006).clamp(-0.36, 0.36)
}

fn view_pitch(body: &FushiBody) -> f32 {
    (body.view_pitch + body.view_pitch_velocity * 0.005).clamp(-0.22, 0.22)
}

fn project_local_3d_to_2d(body: &FushiBody, local: Vec2, z: f32) -> Vec2 {
    let yaw = view_yaw(body);
    let pitch = view_pitch(body);
    let (sy, cy) = yaw.sin_cos();
    let x1 = local.x * cy + z * sy;
    let z1 = z * cy - local.x * sy;
    let (sp, cp) = pitch.sin_cos();
    let y2 = local.y * cp - z1 * sp;
    let z2 = local.y * sp + z1 * cp;
    let perspective = (BODY_CAMERA_DISTANCE / (BODY_CAMERA_DISTANCE - z2)).clamp(0.82, 1.22);
    Vec2::new(x1 * perspective, y2 * perspective)
}

fn projected_depth(body: &FushiBody, local: Vec2, z: f32) -> f32 {
    let yaw = view_yaw(body);
    let pitch = view_pitch(body);
    let (sy, cy) = yaw.sin_cos();
    let z1 = z * cy - local.x * sy;
    let (sp, cp) = pitch.sin_cos();
    local.y * sp + z1 * cp
}

fn body_surface_depth(local: Vec2) -> f32 {
    let nx = (local.x / BODY_HALF_LENGTH).abs().clamp(0.0, 1.0);
    let center_y = (BODY_CENTER_TO_BELLY - BODY_HALF_HEIGHT) * 0.5;
    let half_y = (BODY_CENTER_TO_BELLY + BODY_HALF_HEIGHT) * 0.58;
    let ny = ((local.y - center_y) / half_y).abs().clamp(0.0, 1.25);
    let length_dome = (1.0 - nx.powf(2.05)).max(0.0).powf(0.52);
    let vertical_dome = (1.0 - (ny * 0.82).powf(2.25)).max(0.0).powf(0.40);
    let top = 1.0 - smoothstep(-32.0, BODY_CENTER_TO_BELLY + 2.0, local.y);
    let belly = smoothstep(BODY_HALF_HEIGHT * 0.36, BODY_CENTER_TO_BELLY + 8.0, local.y);
    BODY_DEPTH_RADIUS * length_dome * vertical_dome * (0.92 + top * 0.16 - belly * 0.18).clamp(0.70, 1.16)
}

fn body_outline_projection_mix(body: &FushiBody) -> f32 {
    let shape_guard = render_drag_shape_guard(body);
    let yaw_amount = view_yaw(body).abs();
    let pitch_amount = view_pitch(body).abs();
    (match body.mode {
        MotionMode::Attached => (0.10 + yaw_amount * 0.32 + pitch_amount * 0.18).clamp(0.08, 0.32),
        MotionMode::Flying => (0.22 + yaw_amount * 0.28 + pitch_amount * 0.18).clamp(0.16, 0.48),
        MotionMode::Dragged => {
            (0.26 + yaw_amount * 0.24 + pitch_amount * 0.16 + shape_guard * 0.10).clamp(0.20, 0.54)
        }
    }) * (1.0 - shape_guard * 0.20)
}

fn body_outline_deformation_mix(body: &FushiBody) -> f32 {
    let shape_guard = render_drag_shape_guard(body);
    match body.mode {
        MotionMode::Attached => 0.88,
        MotionMode::Flying => 0.74,
        MotionMode::Dragged => (0.76 - shape_guard * 0.12).clamp(0.62, 0.80),
    }
}

fn body_outline_z_norm(local: Vec2) -> f32 {
    let top = 1.0 - smoothstep(-34.0, 24.0, local.y);
    let belly = smoothstep(22.0, BODY_CENTER_TO_BELLY + 8.0, local.y);
    let end = smoothstep(0.78, 1.0, (local.x / BODY_HALF_LENGTH).abs());
    (0.74 + top * 0.18 - belly * 0.20 - end * 0.10).clamp(0.42, 1.04)
}

fn body_outline_local_to_world(body: &FushiBody, local: Vec2, deformation_mix: f32) -> Vec2 {
    let z = body_surface_depth(local) * body_outline_z_norm(local);
    body_surface_local_to_world(body, local, z, deformation_mix)
}

fn body_surface_local_to_world(body: &FushiBody, local: Vec2, z: f32, deformation_mix: f32) -> Vec2 {
    let projected = project_local_3d_to_2d(body, local, z);
    let mesh_world = mesh_frame(body, projected, 0.0).0;
    let stable_world = body.kinematics().local_to_world(projected);
    mesh_world * deformation_mix + stable_world * (1.0 - deformation_mix)
}

fn decoration_surface_deformation_mix(body: &FushiBody) -> f32 {
    let shape_guard = render_drag_shape_guard(body);
    match body.mode {
        MotionMode::Attached => 0.92,
        MotionMode::Flying => 0.88,
        MotionMode::Dragged => (0.94 + shape_guard * 0.04).clamp(0.92, 1.0),
    }
}

fn spot_surface_z_norm(local: Vec2) -> f32 {
    let top = 1.0 - smoothstep(-34.0, 28.0, local.y);
    let belly = smoothstep(28.0, BODY_CENTER_TO_BELLY + 12.0, local.y);
    let edge = smoothstep(0.70, 1.0, (local.x / BODY_HALF_LENGTH).abs());
    (0.78 + top * 0.20 - belly * 0.18 - edge * 0.08).clamp(0.50, 1.08)
}

fn body_surface_frame(
    body: &FushiBody,
    local: Vec2,
    z_norm: f32,
    deformation_mix: f32,
) -> (Vec2, Vec2, Vec2, Vec2) {
    let scale = body.scale.max(0.2);
    let z = body_surface_depth(local) * z_norm;
    let center = body_surface_local_to_world(body, local, z, deformation_mix);

    let lx = local + Vec2::new(SURFACE_AXIS_DELTA, 0.0);
    let ly = local + Vec2::new(0.0, SURFACE_AXIS_DELTA);
    let px = body_surface_local_to_world(body, lx, body_surface_depth(lx) * z_norm, deformation_mix);
    let py = body_surface_local_to_world(body, ly, body_surface_depth(ly) * z_norm, deformation_mix);

    let vx = px - center;
    let vy = py - center;
    let fallback_x = body.local_vec_to_world(Vec2::X).normalized_or(Vec2::X);
    let fallback_y = body.local_vec_to_world(Vec2::Y).normalized_or(Vec2::Y);
    let axis_x = vx.normalized_or(fallback_x);
    let mut axis_y = vy.normalized_or(fallback_y);
    if axis_x.cross(axis_y).abs() < 0.08 {
        axis_y = axis_x.perp_left().normalized_or(fallback_y);
    }

    let sx = (vx.length() / (SURFACE_AXIS_DELTA * scale)).clamp(0.42, 1.30);
    let sy = (vy.length() / (SURFACE_AXIS_DELTA * scale)).clamp(0.48, 1.28);
    (center, axis_x, axis_y, Vec2::new(sx, sy))
}

fn surface_visibility(body: &FushiBody, local: Vec2, z_norm: f32) -> f32 {
    let z = body_surface_depth(local) * z_norm;
    let depth = projected_depth(body, local, z);
    smoothstep(-76.0, 86.0, depth)
}

fn face_decal_center_local(body: &FushiBody) -> Vec2 {
    let mut center = FACE_DECAL_CENTER_LOCAL + FACE_DECAL_BASE_SHIFT;

    // Do not drive the face forward/back from instantaneous velocity.  When Fushi
    // gathers its head before crawling, velocity-based offsets made the decal move
    // before the head silhouette and then get clipped away.  Keep the authored 2D
    // face in one local cheek position; the frame below moves that whole decal with
    // the head mesh instead.
    let drag_guard = render_drag_shape_guard(body);
    center.x += drag_guard * 0.05;
    center.y -= drag_guard * 0.03;
    center
}

fn face_decal_depth_px(local: Vec2) -> f32 {
    (body_surface_depth(local) * FACE_DECAL_SURFACE_Z_NORM + 2.0).clamp(10.0, BODY_DEPTH_RADIUS * 1.02)
}

fn face_head_mesh_offset(body: &FushiBody, center_local: Vec2) -> Vec2 {
    let kin = body.kinematics();
    let mut displacement = Vec2::ZERO;
    let mut weight_sum = 0.0;

    for node in &body.mesh.nodes {
        // Weighted toward the head/cheek region, not the entire slug.  This is
        // the important lock: during the crawl gather/reach cycle the facial
        // decal follows the same soft head displacement as the snout outline,
        // instead of leading it from a separate projection.
        let head = smoothstep(82.0, BODY_HALF_LENGTH + 4.0, -node.rest.x);
        let cheek_band = 1.0 - smoothstep(36.0, BODY_CENTER_TO_BELLY + 12.0, node.rest.y);
        let upper_guard = 1.0 - smoothstep(56.0, 88.0, (node.rest.y + 12.0).abs());
        let proximity = 1.0 / ((node.rest - center_local).length_sq() + 620.0);
        let w = head * cheek_band.max(0.22) * upper_guard.max(0.20) * proximity;
        if w <= 0.0 {
            continue;
        }
        let rest_world = kin.local_to_world(node.rest);
        displacement += (node.pos - rest_world) * w;
        weight_sum += w;
    }

    if weight_sum <= 0.0001 {
        return Vec2::ZERO;
    }

    let follow = match body.mode {
        MotionMode::Attached => 0.92,
        MotionMode::Dragged => 0.74,
        MotionMode::Flying => 0.66,
    } * (1.0 - render_drag_shape_guard(body) * 0.16);
    (displacement / weight_sum * follow).clamp_len(
        match body.mode {
            MotionMode::Attached => 18.0,
            MotionMode::Dragged => 26.0,
            MotionMode::Flying => 24.0,
        } * body.scale.max(0.2),
    )
}

fn face_decal_frame(body: &FushiBody) -> (Vec2, Vec2, Vec2) {
    let center_local = face_decal_center_local(body);
    let z = face_decal_depth_px(center_local);
    let deformation_mix = face_surface_deformation_mix(body);
    let stable_center = body
        .kinematics()
        .local_to_world(project_local_3d_to_2d(body, center_local, z));
    let head_offset = face_head_mesh_offset(body, center_local);

    // A tiny surface projection keeps the decal seated on the cheek, but the
    // head mesh offset is the dominant movement.  This prevents the face from
    // swinging ahead of the head during crawl contraction.
    let surface_center = body_surface_local_to_world(body, center_local, z, deformation_mix);
    let surface_follow = match body.mode {
        MotionMode::Attached => 0.006,
        MotionMode::Dragged => 0.050,
        MotionMode::Flying => 0.040,
    } * (1.0 - render_drag_shape_guard(body) * 0.35);
    let center = stable_center + head_offset + (surface_center - stable_center) * surface_follow;

    let dx = FACE_DECAL_AXIS_DELTA;
    let dy = FACE_DECAL_AXIS_DELTA;
    let px0 = body_surface_local_to_world(body, center_local - Vec2::new(dx, 0.0), z, deformation_mix);
    let px1 = body_surface_local_to_world(body, center_local + Vec2::new(dx, 0.0), z, deformation_mix);
    let py0 = body_surface_local_to_world(body, center_local - Vec2::new(0.0, dy), z, deformation_mix);
    let py1 = body_surface_local_to_world(body, center_local + Vec2::new(0.0, dy), z, deformation_mix);

    let scale = body.scale.max(0.2);
    let stable_x = body.local_vec_to_world(Vec2::X).normalized_or(Vec2::X);
    let stable_y = body.local_vec_to_world(Vec2::Y).normalized_or(Vec2::Y);
    let raw_x = (px1 - px0) / (dx * 2.0);
    let raw_y = (py1 - py0) / (dy * 2.0);
    let raw_x_dir = raw_x.normalized_or(stable_x);
    let raw_y_dir = raw_y.normalized_or(stable_y);

    // Axes follow even less than the center.  Most of the crawl read should come
    // from the head translating with the body; rotating/shearing the face itself
    // is what made the expression look detached and disappear behind the snout.
    let axis_follow = match body.mode {
        MotionMode::Attached => 0.004,
        MotionMode::Dragged => 0.070,
        MotionMode::Flying => 0.055,
    } * (1.0 - render_drag_shape_guard(body) * 0.45);
    let axis_x_dir = (stable_x * (1.0 - axis_follow) + raw_x_dir * axis_follow).normalized_or(stable_x);
    let mut axis_y_dir = (stable_y * (1.0 - axis_follow) + raw_y_dir * axis_follow).normalized_or(stable_y);
    if axis_x_dir.cross(axis_y_dir).abs() < 0.08 {
        axis_y_dir = axis_x_dir.perp_left().normalized_or(stable_y);
    }

    let yaw_amount = view_yaw(body).abs();
    let pitch_amount = view_pitch(body).abs();
    let length_follow = match body.mode {
        MotionMode::Attached => 0.008,
        MotionMode::Dragged => 0.095,
        MotionMode::Flying => 0.075,
    } * (1.0 - render_drag_shape_guard(body) * 0.35);
    let surface_x_len = raw_x.length().clamp(0.86 * scale, 1.03 * scale);
    let surface_y_len = raw_y.length().clamp(0.88 * scale, 1.03 * scale);
    let x_len = lerp(scale, surface_x_len, length_follow) * FACE_DECAL_X_SCALE * (1.0 - yaw_amount * 0.010);
    let y_len = lerp(scale, surface_y_len, length_follow) * FACE_DECAL_Y_SCALE * (1.0 + pitch_amount * 0.006);

    (center, axis_x_dir * x_len, axis_y_dir * y_len)
}
fn face_local_to_world(body: &FushiBody, local: Vec2) -> Vec2 {
    let (center, x_axis, y_axis) = face_decal_frame(body);
    let mut d = local - FACE_DECAL_CENTER_LOCAL;

    // Apply 3D only at the decal-fitting stage.  The original 2D vector face is
    // preserved; this tiny shear just sells the cheek angle without warping the
    // eyes and mouth separately.
    d.y += d.x * FACE_DECAL_SHEAR_Y;
    center + x_axis * d.x + y_axis * d.y
}

fn face_frame(body: &FushiBody, local: Vec2) -> (Vec2, Vec2, Vec2) {
    let center = face_local_to_world(body, local);
    let (_, x_axis, y_axis) = face_decal_frame(body);
    (
        center,
        x_axis.normalized_or(Vec2::X),
        y_axis.normalized_or(Vec2::Y),
    )
}

fn face_element_scale(body: &FushiBody, _local: Vec2) -> Vec2 {
    let scale = body.scale.max(0.2);
    let (_, x_axis, y_axis) = face_decal_frame(body);
    Vec2::new(
        (x_axis.length() / scale).clamp(0.68, 1.08),
        (y_axis.length() / scale).clamp(0.70, 1.08),
    )
}

fn face_surface_deformation_mix(body: &FushiBody) -> f32 {
    let shape_guard = render_drag_shape_guard(body);
    match body.mode {
        MotionMode::Attached => 0.012,
        MotionMode::Flying => 0.070,
        MotionMode::Dragged => (0.090 - shape_guard * 0.03).clamp(0.055, 0.090),
    }
}

fn appendage_motion_lock(body: &FushiBody) -> f32 {
    let shake = smoothstep(5.5, 13.5, body.bank_velocity.abs()).max(smoothstep(
        700.0,
        2100.0,
        body.mouse_velocity.length(),
    ));
    render_drag_shape_guard(body).max(shake * 0.72).clamp(0.0, 1.0)
}

fn ear_depth(index: usize) -> f32 {
    if index == 0 {
        -0.52
    } else {
        0.52
    }
}

fn ear_root_depth_px(index: usize) -> f32 {
    let root = EARS[index].anchor;
    body_surface_depth(root) * 0.76 + ear_depth(index) * 18.0
}

fn ear_render_depth(body: &FushiBody, index: usize) -> f32 {
    let root = EARS[index].anchor;
    projected_depth(body, root, ear_root_depth_px(index))
}

fn ear_depth_px(root: Vec2, local: Vec2, index: usize) -> f32 {
    let d = local - root;
    let tip_weight = smoothstep(4.0, 44.0, d.length());
    ear_root_depth_px(index)
        + ear_depth(index) * (18.0 + tip_weight * 28.0)
        + (-d.y).max(0.0) * 0.10 * tip_weight
}

fn ear_layer_alpha(_body: &FushiBody, _index: usize) -> f32 {
    1.0
}

fn ear_line_scale(body: &FushiBody, index: usize) -> f32 {
    let root = EARS[index].anchor;
    let depth = projected_depth(body, root, ear_root_depth_px(index));
    (0.86 + smoothstep(-100.0, 130.0, depth) * 0.22 + view_pitch(body) * 0.025).clamp(0.76, 1.14)
}

fn tail_point_depth(p: Vec2, inner_shape: bool) -> f32 {
    let tip = smoothstep(18.0, 88.0, p.length());
    let crown = 1.0 - smoothstep(-55.0, 10.0, p.y);
    let lower = smoothstep(8.0, 42.0, p.y);
    let side = (p.x / 70.0).clamp(-1.0, 1.0);
    let base = crown * (0.58 + tip * 0.28) + side * 0.18 - lower * 0.30;
    if inner_shape {
        (base + 0.38).clamp(-0.20, 1.0)
    } else {
        base.clamp(-0.42, 0.92)
    }
}

fn tail_depth_px(_body: &FushiBody, q: Vec2, depth: f32, inner_shape: bool) -> f32 {
    let root_z = body_surface_depth(TAIL_ANCHOR) * 0.76 + 10.0;
    let tip_weight = smoothstep(18.0, 94.0, q.length());
    let crown = 1.0 - smoothstep(-54.0, 12.0, q.y);
    let lower = smoothstep(8.0, 42.0, q.y);
    let side = (q.x / 70.0).clamp(-1.0, 1.0);
    let cutout = if inner_shape { 0.78 } else { 1.0 };
    root_z + cutout * (26.0 + depth * 34.0 + crown * (14.0 + tip_weight * 11.0) + side * 9.0 - lower * 8.0)
}

fn tail_local_to_world(body: &FushiBody, local: Vec2, z_px: f32) -> Vec2 {
    let root = TAIL_ANCHOR;
    let root_z = body_surface_depth(root) * 0.76 + 10.0;
    let root_world = tail_root_world(body);
    let kin = body.kinematics();
    let tail_grab = kin.tail_grab_amount();
    let tip_weight = smoothstep(14.0, 108.0, (local - root).length());
    let flex = kin.guarded_drag_pull_local() * (tail_grab * tip_weight * 0.34);
    let projected_root = project_local_3d_to_2d(body, root, root_z);
    let projected = project_local_3d_to_2d(body, local + flex, z_px);
    root_world + body.local_vec_to_world(projected - projected_root)
}

fn tail_root_world(body: &FushiBody) -> Vec2 {
    let root = TAIL_ANCHOR;
    let root_z = body_surface_depth(root) * 0.76 + 10.0;
    let surface_root =
        body_surface_local_to_world(body, root, root_z, decoration_surface_deformation_mix(body));
    attached_appendage_root_world(body, root, surface_root, 0.56)
}

fn ear_local_to_world(body: &FushiBody, root: Vec2, local: Vec2, index: usize) -> Vec2 {
    let root_z = ear_root_depth_px(index);
    let root_world = ear_root_world(body, index);
    let kin = body.kinematics();
    let ear_grab = kin.ear_grab_amount();
    let root_delta = local - root;
    let tip_weight = smoothstep(5.0, 43.0, root_delta.length());
    let flex = kin.guarded_drag_pull_local() * (ear_grab * tip_weight * 0.30);
    let projected_root = project_local_3d_to_2d(body, root, root_z);
    let projected = project_local_3d_to_2d(body, local + flex, ear_depth_px(root, local + flex, index));
    let projected_world = root_world + body.local_vec_to_world(projected - projected_root);
    let root_locked_world = root_world + body.local_vec_to_world(root_delta);
    let root_lock =
        (1.0 - smoothstep(16.0, 42.0, root_delta.length())) * (0.68 + render_drag_shape_guard(body) * 0.20);
    projected_world * (1.0 - root_lock) + root_locked_world * root_lock
}

fn ear_root_world(body: &FushiBody, index: usize) -> Vec2 {
    let root = EARS[index].anchor;
    let root_z = ear_root_depth_px(index);
    let surface_root =
        body_surface_local_to_world(body, root, root_z, decoration_surface_deformation_mix(body));
    let base_offset_scale = if index == 0 { 0.26 } else { 0.36 };
    attached_appendage_root_world(body, root, surface_root, base_offset_scale)
}

fn attached_appendage_root_world(
    body: &FushiBody,
    root: Vec2,
    surface_root: Vec2,
    base_offset_scale: f32,
) -> Vec2 {
    let Some((outline_y, outline_pos)) = sample_outline_near(&body.mesh.nodes, root) else {
        return surface_root;
    };

    let outline_local = Vec2::new(root.x, outline_y);
    let projected_outline =
        body_outline_local_to_world(body, outline_local, body_outline_deformation_mix(body));
    let projection_mix = body_outline_projection_mix(body);
    let rendered_outline = outline_pos * (1.0 - projection_mix) + projected_outline * projection_mix;

    // The outline is the visible seam. Let perspective influence the root, but never enough to
    // pull the ear away from that seam under squash, yaw, or rapid dragging.
    let shape_guard = render_drag_shape_guard(body);
    let lock = appendage_motion_lock(body);
    let compression = smoothstep(0.04, 0.26, body.impact_squash + shape_guard * 0.36 + lock * 0.16);
    let offset_scale = lerp(
        base_offset_scale,
        base_offset_scale * 0.42,
        compression.max(lock * 0.78),
    );
    let outline_root = rendered_outline + body.local_vec_to_world(root - outline_local) * offset_scale;
    let max_perspective_gap = lerp(3.0, 0.8, compression.max(lock)) * body.scale.max(0.2);

    outline_root + (surface_root - outline_root).clamp_len(max_perspective_gap)
}

fn sample_outline_near(nodes: &[BlobNode], local: Vec2) -> Option<(f32, Vec2)> {
    let mut best: Option<(f32, Vec2, f32)> = None;

    for pair in nodes.windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let x = local.x;
        let min_x = a.rest.x.min(b.rest.x);
        let max_x = a.rest.x.max(b.rest.x);
        if x < min_x || x > max_x {
            continue;
        }
        let denom = b.rest.x - a.rest.x;
        let t = if denom.abs() > 0.001 {
            ((x - a.rest.x) / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let rest = a.rest + (b.rest - a.rest) * t;
        let pos = a.pos + (b.pos - a.pos) * t;
        let score = (rest - local).length_sq();
        if best.map(|(_, _, best_score)| score < best_score).unwrap_or(true) {
            best = Some((rest.y, pos, score));
        }
    }

    if let Some((outline_y, outline_pos, _)) = best {
        return Some((outline_y, outline_pos));
    }

    nodes
        .iter()
        .min_by(|a, b| {
            (a.rest - local)
                .length_sq()
                .partial_cmp(&(b.rest - local).length_sq())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|n| (n.rest.y, n.pos))
}

fn render_drag_shape_guard(body: &FushiBody) -> f32 {
    if body.mode == MotionMode::Dragged {
        body.kinematics().drag_shape_guard()
    } else {
        0.0
    }
}
