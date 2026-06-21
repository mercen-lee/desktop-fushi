use crate::math::{Color, Path, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct GradientStop {
    pub position: f32,
    pub color: Color,
}

impl GradientStop {
    #[inline]
    pub const fn new(position: f32, color: Color) -> Self {
        Self { position, color }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    pub center: Vec2,
    pub axis_x: Vec2,
    pub axis_y: Vec2,
    pub rx: f32,
    pub ry: f32,
}

impl Ellipse {
    #[inline]
    pub const fn new(center: Vec2, axis_x: Vec2, axis_y: Vec2, rx: f32, ry: f32) -> Self {
        Self {
            center,
            axis_x,
            axis_y,
            rx,
            ry,
        }
    }
}

pub trait VectorCanvas {
    fn fill_path(&mut self, path: &Path, color: Color);
    fn fill_path_linear_gradient(&mut self, path: &Path, start: Vec2, end: Vec2, stops: &[GradientStop]);
    fn stroke_path(&mut self, path: &Path, color: Color, width: f32);
    fn fill_ellipse(&mut self, ellipse: Ellipse, color: Color);
    #[allow(dead_code)]
    fn stroke_ellipse(&mut self, ellipse: Ellipse, color: Color, width: f32);
    fn draw_line(&mut self, a: Vec2, b: Vec2, color: Color, width: f32);
}
