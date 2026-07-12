use crate::canvas::{Ellipse, GradientStop, VectorCanvas};
use crate::math::{ellipse_path, Color, Path, PathCmd, Vec2};

use lyon::math::{point, Point};
use lyon::path::Path as LyonPath;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};

#[cfg(target_os = "android")]
const PATH_TOLERANCE: f32 = 0.18;
#[cfg(not(target_os = "android"))]
const PATH_TOLERANCE: f32 = 0.05;
const ELLIPSE_SEGMENTS: usize = 28;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

pub struct GpuScene {
    pub width: u32,
    pub height: u32,
    pub vertices: Vec<GpuVertex>,
    pub indices: Vec<u32>,
}

impl GpuScene {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }
}

pub struct GpuCanvas {
    width: u32,
    height: u32,
    origin: Vec2,
    scale: f32,
    mesh: VertexBuffers<GpuVertex, u32>,
}

impl GpuCanvas {
    pub fn new(width: u32, height: u32, origin: Vec2, scale: f32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            origin,
            scale: scale.max(0.1),
            mesh: VertexBuffers::new(),
        }
    }

    pub fn into_scene(self) -> GpuScene {
        GpuScene {
            width: self.width,
            height: self.height,
            vertices: self.mesh.vertices,
            indices: self.mesh.indices,
        }
    }

    fn build_path(&self, path: &Path) -> Option<LyonPath> {
        let mut builder = LyonPath::builder().with_svg();
        let mut open = false;
        let mut has_geometry = false;

        for cmd in &path.commands {
            match *cmd {
                PathCmd::MoveTo(p) => {
                    builder.move_to(self.to_local_point(p));
                    open = true;
                }
                PathCmd::LineTo(p) if open => {
                    builder.line_to(self.to_local_point(p));
                    has_geometry = true;
                }
                PathCmd::CubicTo(c1, c2, p) if open => {
                    builder.cubic_bezier_to(
                        self.to_local_point(c1),
                        self.to_local_point(c2),
                        self.to_local_point(p),
                    );
                    has_geometry = true;
                }
                PathCmd::Close if open => {
                    builder.close();
                    open = false;
                }
                _ => {}
            }
        }

        if has_geometry {
            Some(builder.build())
        } else {
            None
        }
    }

    #[inline]
    fn to_local_point(&self, p: Vec2) -> Point {
        point(
            (p.x - self.origin.x) * self.scale,
            (p.y - self.origin.y) * self.scale,
        )
    }

    #[inline]
    fn clip_position(width: u32, height: u32, p: Point) -> [f32; 2] {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        [(p.x / w) * 2.0 - 1.0, 1.0 - (p.y / h) * 2.0]
    }

    #[inline]
    fn color_array(color: Color) -> [f32; 4] {
        [
            color.r.clamp(0.0, 1.0),
            color.g.clamp(0.0, 1.0),
            color.b.clamp(0.0, 1.0),
            color.a.clamp(0.0, 1.0),
        ]
    }

    #[inline]
    fn vertex(width: u32, height: u32, p: Point, color: [f32; 4]) -> GpuVertex {
        GpuVertex {
            position: Self::clip_position(width, height, p),
            color,
        }
    }

    fn fill_lyon_path(&mut self, path: &LyonPath, color: Color) {
        if color.a <= 0.0 {
            return;
        }

        let width = self.width;
        let height = self.height;
        let color = Self::color_array(color);
        let options = FillOptions::tolerance(PATH_TOLERANCE).with_fill_rule(FillRule::NonZero);
        let mut tessellator = FillTessellator::new();
        let _ = tessellator.tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut self.mesh, |vertex: FillVertex| {
                Self::vertex(width, height, vertex.position(), color)
            }),
        );
    }

    fn lerp_color(a: Color, b: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }

    fn sample_gradient(stops: &[GradientStop], t: f32) -> Color {
        if stops.is_empty() {
            return Color::default();
        }
        if stops.len() == 1 {
            return stops[0].color;
        }

        let t = t.clamp(0.0, 1.0);
        let mut prev = stops[0];
        for stop in &stops[1..] {
            if t <= stop.position {
                let denom = (stop.position - prev.position).abs();
                let local_t = if denom <= 0.0001 {
                    0.0
                } else {
                    (t - prev.position) / (stop.position - prev.position)
                };
                return Self::lerp_color(prev.color, stop.color, local_t);
            }
            prev = *stop;
        }
        prev.color
    }

    fn fill_lyon_path_linear_gradient(
        &mut self,
        path: &LyonPath,
        start: Vec2,
        end: Vec2,
        stops: &[GradientStop],
    ) {
        if stops.is_empty() {
            return;
        }

        let width = self.width;
        let height = self.height;
        let start = self.to_local_point(start);
        let end = self.to_local_point(end);
        let dir = end - start;
        let denom = dir.x * dir.x + dir.y * dir.y;
        let options = FillOptions::tolerance(PATH_TOLERANCE).with_fill_rule(FillRule::NonZero);
        let mut tessellator = FillTessellator::new();
        let _ = tessellator.tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut self.mesh, |vertex: FillVertex| {
                let pos = vertex.position();
                let t = if denom <= 0.0001 {
                    1.0
                } else {
                    (((pos.x - start.x) * dir.x + (pos.y - start.y) * dir.y) / denom).clamp(0.0, 1.0)
                };
                let color = Self::color_array(Self::sample_gradient(stops, t));
                Self::vertex(width, height, pos, color)
            }),
        );
    }

    fn stroke_lyon_path(&mut self, path: &LyonPath, color: Color, width_px: f32) {
        if color.a <= 0.0 || width_px <= 0.0 {
            return;
        }

        let width = self.width;
        let height = self.height;
        let color = Self::color_array(color);
        let options = StrokeOptions::tolerance(PATH_TOLERANCE)
            .with_line_width(width_px)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        let mut tessellator = StrokeTessellator::new();
        let _ = tessellator.tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut self.mesh, |vertex: StrokeVertex| {
                Self::vertex(width, height, vertex.position(), color)
            }),
        );
    }
}

impl VectorCanvas for GpuCanvas {
    fn fill_path(&mut self, path: &Path, color: Color) {
        if let Some(path) = self.build_path(path) {
            self.fill_lyon_path(&path, color);
        }
    }

    fn fill_path_linear_gradient(&mut self, path: &Path, start: Vec2, end: Vec2, stops: &[GradientStop]) {
        if let Some(path) = self.build_path(path) {
            self.fill_lyon_path_linear_gradient(&path, start, end, stops);
        }
    }

    fn stroke_path(&mut self, path: &Path, color: Color, width: f32) {
        if let Some(path) = self.build_path(path) {
            self.stroke_lyon_path(&path, color, width * self.scale);
        }
    }

    fn fill_ellipse(&mut self, ellipse: Ellipse, color: Color) {
        let path = ellipse_path(
            ellipse.center,
            ellipse.axis_x,
            ellipse.axis_y,
            ellipse.rx,
            ellipse.ry,
            ELLIPSE_SEGMENTS,
        );
        self.fill_path(&path, color);
    }

    fn stroke_ellipse(&mut self, ellipse: Ellipse, color: Color, width: f32) {
        let path = ellipse_path(
            ellipse.center,
            ellipse.axis_x,
            ellipse.axis_y,
            ellipse.rx,
            ellipse.ry,
            ELLIPSE_SEGMENTS,
        );
        self.stroke_path(&path, color, width);
    }

    fn draw_line(&mut self, a: Vec2, b: Vec2, color: Color, width: f32) {
        let mut path = Path::new();
        path.move_to(a);
        path.line_to(b);
        self.stroke_path(&path, color, width);
    }
}
