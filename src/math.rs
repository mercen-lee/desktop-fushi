use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub const X: Self = Self { x: 1.0, y: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    #[inline]
    pub fn cross(self, other: Self) -> f32 {
        self.x * other.y - self.y * other.x
    }

    #[inline]
    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    #[inline]
    pub fn normalized_or(self, fallback: Self) -> Self {
        let len = self.length();
        if len > 0.0001 {
            self / len
        } else {
            fallback
        }
    }

    #[inline]
    pub fn perp_left(self) -> Self {
        Self::new(-self.y, self.x)
    }

    #[inline]
    pub fn rotate(self, radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    #[inline]
    pub fn clamp_len(self, max_len: f32) -> Self {
        let len = self.length();
        if len > max_len && len > 0.0001 {
            self * (max_len / len)
        } else {
            self
        }
    }

    #[inline]
    pub fn min(self, other: Self) -> Self {
        Self::new(self.x.min(other.x), self.y.min(other.y))
    }

    #[inline]
    pub fn max(self, other: Self) -> Self {
        Self::new(self.x.max(other.x), self.y.max(other.y))
    }
}

impl Add for Vec2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl MulAssign<f32> for Vec2 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl Neg for Vec2 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    #[inline]
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn width(self) -> f32 {
        self.right - self.left
    }

    #[inline]
    pub fn height(self) -> f32 {
        self.bottom - self.top
    }

    #[inline]
    pub fn inflate(self, px: f32) -> Self {
        Self::new(self.left - px, self.top - px, self.right + px, self.bottom + px)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RectI {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectI {
    #[inline]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    #[inline]
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }

    #[inline]
    pub fn contains(self, p: Vec2) -> bool {
        p.x >= self.left as f32
            && p.x <= self.right as f32
            && p.y >= self.top as f32
            && p.y <= self.bottom as f32
    }

    #[inline]
    pub fn inflate(self, px: i32) -> Self {
        Self::new(self.left - px, self.top - px, self.right + px, self.bottom + px)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    #[inline]
    pub const fn rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PathCmd {
    MoveTo(Vec2),
    LineTo(Vec2),
    CubicTo(Vec2, Vec2, Vec2),
    Close,
}

#[derive(Clone, Debug, Default)]
pub struct Path {
    pub commands: Vec<PathCmd>,
}

impl Path {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    pub fn move_to(&mut self, p: Vec2) {
        self.commands.push(PathCmd::MoveTo(p));
    }

    pub fn line_to(&mut self, p: Vec2) {
        self.commands.push(PathCmd::LineTo(p));
    }

    pub fn cubic_to(&mut self, c1: Vec2, c2: Vec2, p: Vec2) {
        self.commands.push(PathCmd::CubicTo(c1, c2, p));
    }

    pub fn close(&mut self) {
        self.commands.push(PathCmd::Close);
    }
}

#[inline]
pub fn clampf(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn vlerp(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    a + (b - a) * t
}

#[inline]
pub fn exp_decay(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

#[inline]
pub fn approach(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clampf((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn wrap_angle(mut a: f32) -> f32 {
    const PI: f32 = std::f32::consts::PI;
    const TAU: f32 = std::f32::consts::TAU;
    while a > PI {
        a -= TAU;
    }
    while a < -PI {
        a += TAU;
    }
    a
}

pub fn catmull_rom_path(points: &[Vec2], closed: bool) -> Path {
    catmull_rom_path_with_tension(points, closed, 1.0 / 6.0)
}

pub fn catmull_rom_path_with_tension(points: &[Vec2], closed: bool, tension: f32) -> Path {
    let mut path = Path::new();
    if points.is_empty() {
        return path;
    }
    if points.len() < 3 {
        path.move_to(points[0]);
        for p in &points[1..] {
            path.line_to(*p);
        }
        if closed {
            path.close();
        }
        return path;
    }

    let tension = clampf(tension, 0.0, 0.5);
    path.move_to(points[0]);
    let n = points.len();
    let segment_count = if closed { n } else { n - 1 };
    for i in 0..segment_count {
        let p0 = if i == 0 {
            if closed {
                points[n - 1]
            } else {
                points[0]
            }
        } else {
            points[i - 1]
        };
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = if i + 2 >= n {
            if closed {
                points[(i + 2) % n]
            } else {
                points[n - 1]
            }
        } else {
            points[i + 2]
        };
        let c1 = p1 + (p2 - p0) * tension;
        let c2 = p2 - (p3 - p1) * tension;
        path.cubic_to(c1, c2, p2);
    }
    if closed {
        path.close();
    }
    path
}

pub fn ellipse_path(center: Vec2, axis_x: Vec2, axis_y: Vec2, rx: f32, ry: f32, segments: usize) -> Path {
    let mut pts = Vec::with_capacity(segments.max(8));
    let n = segments.max(8);
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        pts.push(center + axis_x * (a.cos() * rx) + axis_y * (a.sin() * ry));
    }
    catmull_rom_path(&pts, true)
}
