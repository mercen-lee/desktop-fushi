use crate::fushi::constants::{BODY_CENTER_TO_BELLY, BODY_HALF_LENGTH};
use crate::math::{clampf, smoothstep, RectI, Vec2};

const WINDOW_OCCLUSION_EDGE_BAND: f32 = 10.0;
const WINDOW_VISIBLE_CONTACT_MIN_RATIO: f32 = 0.42;
const WINDOW_VISIBLE_CONTACT_MIN_PIXELS: f32 = 30.0;
const WINDOW_TOP_SUPPORT_HEADROOM_MIN: f32 = 112.0;
const WINDOW_TOP_SUPPORT_HEADROOM_FACTOR: f32 = 1.05;

#[cfg(target_os = "windows")]
use windows::core::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{LPARAM, RECT};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceKind {
    Bottom,
    Right,
    Top,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceContact {
    pub monitor_index: usize,
    pub kind: SurfaceKind,
    pub window_id: Option<isize>,
}

impl SurfaceContact {
    pub const fn monitor(monitor_index: usize, kind: SurfaceKind) -> Self {
        Self {
            monitor_index,
            kind,
            window_id: None,
        }
    }

    pub const fn window(monitor_index: usize, window_id: isize, kind: SurfaceKind) -> Self {
        Self {
            monitor_index,
            kind,
            window_id: Some(window_id),
        }
    }

    pub fn is_platform(self) -> bool {
        self.window_id.is_some()
    }
}

#[derive(Clone, Debug)]
pub struct MonitorArea {
    pub bounds: RectI,
    pub work: RectI,
    pub primary: bool,
}

#[derive(Clone, Debug)]
pub struct WindowSurface {
    pub id: isize,
    pub monitor_index: usize,
    pub rect: RectI,
    pub velocity: Vec2,
    pub edge_velocity: EdgeVelocity,
}

impl WindowSurface {
    fn contact(&self, kind: SurfaceKind) -> SurfaceContact {
        SurfaceContact::window(self.monitor_index, self.id, kind)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeVelocity {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Debug)]
pub struct DesktopEnvironment {
    pub monitors: Vec<MonitorArea>,
    pub virtual_bounds: RectI,
    pub windows: Vec<WindowSurface>,
    monitor_corner_padding: Option<f32>,
}

impl DesktopEnvironment {
    #[cfg(target_os = "windows")]
    pub fn capture() -> Self {
        let mut monitors: Vec<MonitorArea> = Vec::new();
        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            lparam: LPARAM,
        ) -> BOOL {
            let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorArea>) };
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(hmonitor, &mut info as *mut MONITORINFO) }.as_bool() {
                monitors.push(MonitorArea {
                    bounds: rect_from_win32(info.rcMonitor),
                    work: rect_from_win32(info.rcWork),
                    primary: (info.dwFlags & MONITORINFOF_PRIMARY) != 0,
                });
            }
            BOOL(1)
        }

        unsafe {
            let _ = EnumDisplayMonitors(
                None,
                None,
                Some(enum_proc),
                LPARAM((&mut monitors as *mut Vec<MonitorArea>) as isize),
            );
        }

        Self::from_monitor_areas(monitors)
    }

    #[cfg(target_os = "macos")]
    pub fn capture() -> Self {
        Self::from_monitor_areas(crate::macos::monitor_areas())
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    pub fn capture() -> Self {
        Self::fallback()
    }

    pub fn from_screen_size(width: i32, height: i32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let rect = RectI::new(0, 0, width, height);
        Self::from_screen_work_area(rect, rect)
    }

    pub fn from_screen_work_area(bounds: RectI, work: RectI) -> Self {
        let bounds = RectI::new(
            bounds.left,
            bounds.top,
            bounds.right.max(bounds.left + 1),
            bounds.bottom.max(bounds.top + 1),
        );
        let work_left = work.left.clamp(bounds.left, bounds.right - 1);
        let work_top = work.top.clamp(bounds.top, bounds.bottom - 1);
        let work = RectI::new(
            work_left,
            work_top,
            work.right.clamp(work_left + 1, bounds.right),
            work.bottom.clamp(work_top + 1, bounds.bottom),
        );
        Self::from_monitor_areas(vec![MonitorArea {
            bounds,
            work,
            primary: true,
        }])
    }

    pub fn with_window_rects(mut self, windows: impl IntoIterator<Item = (isize, RectI)>) -> Self {
        self.windows = windows
            .into_iter()
            .filter_map(|(id, rect)| {
                if rect.width() < 96 || rect.height() < 48 {
                    return None;
                }
                Some(WindowSurface {
                    id,
                    monitor_index: self.monitor_index_for_rect(rect),
                    rect,
                    velocity: Vec2::ZERO,
                    edge_velocity: EdgeVelocity::default(),
                })
            })
            .collect();
        self
    }

    pub fn with_monitor_corner_padding(mut self, padding: f32) -> Self {
        self.monitor_corner_padding = Some(padding.clamp(0.58, 1.20));
        self
    }

    pub fn apply_window_motion_from(&mut self, previous: &DesktopEnvironment, dt: f32) {
        let dt = dt.max(0.001);
        for window in &mut self.windows {
            if let Some(prev) = previous
                .windows
                .iter()
                .find(|candidate| candidate.id == window.id)
            {
                window.velocity = (rect_center(window.rect) - rect_center(prev.rect)) / dt;
                window.edge_velocity = EdgeVelocity {
                    left: (window.rect.left - prev.rect.left) as f32 / dt,
                    top: (window.rect.top - prev.rect.top) as f32 / dt,
                    right: (window.rect.right - prev.rect.right) as f32 / dt,
                    bottom: (window.rect.bottom - prev.rect.bottom) as f32 / dt,
                };
            }
        }
    }

    fn fallback() -> Self {
        Self::from_monitor_areas(vec![MonitorArea {
            bounds: RectI::new(0, 0, 1920, 1080),
            work: RectI::new(0, 0, 1920, 1040),
            primary: true,
        }])
    }

    fn from_monitor_areas(mut monitors: Vec<MonitorArea>) -> Self {
        if monitors.is_empty() {
            return Self::fallback();
        }
        if !monitors.iter().any(|m| m.primary) {
            if let Some(first) = monitors.first_mut() {
                first.primary = true;
            }
        }

        let mut vb = monitors[0].bounds;
        for m in &monitors[1..] {
            vb.left = vb.left.min(m.bounds.left);
            vb.top = vb.top.min(m.bounds.top);
            vb.right = vb.right.max(m.bounds.right);
            vb.bottom = vb.bottom.max(m.bounds.bottom);
        }

        Self {
            monitors,
            virtual_bounds: vb,
            windows: Vec::new(),
            monitor_corner_padding: None,
        }
    }

    pub fn primary_index(&self) -> usize {
        self.monitors.iter().position(|m| m.primary).unwrap_or(0)
    }

    pub fn monitor_index_for_point(&self, p: Vec2) -> usize {
        self.monitors
            .iter()
            .position(|monitor| monitor.bounds.inflate(32).contains(p))
            .unwrap_or_else(|| self.primary_index())
    }

    pub fn initial_contact(&self) -> SurfaceContact {
        SurfaceContact::monitor(self.primary_index(), SurfaceKind::Bottom)
    }

    pub fn initial_center(&self) -> Vec2 {
        let i = self.primary_index();
        let w = self.monitors[i].work;
        Vec2::new(
            w.left as f32 + BODY_HALF_LENGTH + 96.0,
            w.bottom as f32 - BODY_CENTER_TO_BELLY,
        )
    }

    pub fn monitor(&self, c: SurfaceContact) -> &MonitorArea {
        &self.monitors[c.monitor_index.min(self.monitors.len() - 1)]
    }

    pub fn contact_window(&self, c: SurfaceContact) -> Option<&WindowSurface> {
        let id = c.window_id?;
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn contact_velocity(&self, c: SurfaceContact) -> Vec2 {
        self.contact_window(c)
            .map(|window| Self::window_surface_velocity(window, c.kind))
            .unwrap_or(Vec2::ZERO)
    }

    pub fn screen_edge_pinch_amount(&self, c: SurfaceContact, center_offset: f32) -> f32 {
        let Some(window) = self.contact_window(c) else {
            return 0.0;
        };
        if self.monitors.is_empty() {
            return 0.0;
        }

        let monitor = &self.monitors[window.monitor_index.min(self.monitors.len() - 1)];
        let work = monitor.work;
        let gap = match c.kind {
            SurfaceKind::Bottom => window.rect.top - work.top,
            SurfaceKind::Top => work.bottom - window.rect.bottom,
            SurfaceKind::Left => work.right - window.rect.right,
            SurfaceKind::Right => window.rect.left - work.left,
        }
        .max(0) as f32;

        let full = (center_offset * 0.18).max(8.0);
        let fade = (center_offset * 1.42).max(46.0);
        clampf(1.0 - smoothstep(full, fade, gap), 0.0, 1.0)
    }

    pub fn platform_supports(&self, c: SurfaceContact, center: Vec2, half_len: f32) -> bool {
        let Some(index) = self.contact_window_index(c) else {
            return false;
        };
        if !self.window_contact_has_room(c, center, half_len) {
            return false;
        }
        let coord = Self::tangent_coord(c.kind, center);
        let (lo, hi) = self.tangent_extent(c);
        if coord < lo - half_len * 0.42 || coord > hi + half_len * 0.42 {
            return false;
        }

        // A window does not have to be the active/topmost one, but the part Fushi is
        // touching must actually be visible. Covered intervals are ignored as solid ground so
        // hidden windows cannot repeatedly catch a falling Fushi through transparent space.
        self.window_edge_contact_visible(index, c.kind, coord, half_len * 0.50)
    }

    pub fn replacement_platform_surface(
        &self,
        previous: SurfaceContact,
        center: Vec2,
        half_len: f32,
        center_offset: f32,
    ) -> Option<(SurfaceContact, Vec2, Vec2)> {
        if !previous.is_platform() {
            return None;
        }
        let coord = Self::tangent_coord(previous.kind, center);
        let max_snap = match previous.kind {
            SurfaceKind::Bottom => center_offset * 0.40 + 10.0,
            SurfaceKind::Top => center_offset * 0.32 + 8.0,
            SurfaceKind::Left | SurfaceKind::Right => center_offset * 0.62 + 10.0,
        };

        let mut best: Option<(SurfaceContact, Vec2, Vec2, f32)> = None;
        for (index, window) in self.windows.iter().enumerate() {
            if previous.window_id == Some(window.id) {
                continue;
            }
            let contact = window.contact(previous.kind);
            let (lo, hi) = self.tangent_extent(contact);
            if coord < lo - half_len * 0.44 || coord > hi + half_len * 0.44 {
                continue;
            }
            if !self.window_edge_contact_visible(index, previous.kind, coord, half_len * 0.50) {
                continue;
            }

            let snapped = self.point_from_tangent(contact, clampf(coord, lo, hi), center_offset);
            if !self.window_contact_has_room(contact, snapped, half_len) {
                continue;
            }
            let snap_delta = snapped - center;
            let snap_distance = snap_delta.length();
            if snap_distance > max_snap {
                continue;
            }
            let normal_shift = snap_delta.dot(Self::surface_normal(previous.kind));
            if normal_shift < -max_snap * 0.4 {
                continue;
            }

            let attachable = match previous.kind {
                SurfaceKind::Bottom => snapped.y >= center.y - center_offset * 0.25,
                SurfaceKind::Top => snapped.y <= center.y + center_offset * 0.25,
                SurfaceKind::Left | SurfaceKind::Right => {
                    (snapped.x - center.x).abs() <= center_offset * 1.15
                }
            };
            if !attachable {
                continue;
            }

            let velocity = Self::window_surface_velocity(window, previous.kind);
            let score = snap_distance + index as f32 * 4.0;
            let replace = best.as_ref().map(|current| score < current.3).unwrap_or(true);
            if replace {
                best = Some((contact, snapped, velocity, score));
            }
        }

        if let Some((contact, snapped, velocity, _)) = best {
            return Some((contact, snapped, velocity));
        }

        self.find_visible_window_surface_near(previous, previous.kind, center, half_len, center_offset)
            .map(|(contact, snapped)| (contact, snapped, self.contact_velocity(contact)))
    }

    pub fn surface_normal(kind: SurfaceKind) -> Vec2 {
        match kind {
            SurfaceKind::Bottom => Vec2::new(0.0, 1.0),
            SurfaceKind::Top => Vec2::new(0.0, -1.0),
            SurfaceKind::Left => Vec2::new(-1.0, 0.0),
            SurfaceKind::Right => Vec2::new(1.0, 0.0),
        }
    }

    pub fn surface_tangent(kind: SurfaceKind) -> Vec2 {
        match kind {
            SurfaceKind::Bottom => Vec2::new(1.0, 0.0),
            SurfaceKind::Right => Vec2::new(0.0, -1.0),
            SurfaceKind::Top => Vec2::new(-1.0, 0.0),
            SurfaceKind::Left => Vec2::new(0.0, 1.0),
        }
    }

    pub fn surface_line(&self, c: SurfaceContact) -> f32 {
        let w = self.surface_rect(c);
        if c.is_platform() {
            match c.kind {
                SurfaceKind::Bottom => w.top as f32,
                SurfaceKind::Top => -w.bottom as f32,
                SurfaceKind::Left => -w.right as f32,
                SurfaceKind::Right => w.left as f32,
            }
        } else {
            match c.kind {
                SurfaceKind::Bottom => w.bottom as f32,
                SurfaceKind::Top => -w.top as f32,
                SurfaceKind::Left => -w.left as f32,
                SurfaceKind::Right => w.right as f32,
            }
        }
    }

    pub fn tangent_extent(&self, c: SurfaceContact) -> (f32, f32) {
        let w = self.surface_rect(c);
        match c.kind {
            SurfaceKind::Bottom | SurfaceKind::Top => (w.left as f32, w.right as f32),
            SurfaceKind::Left | SurfaceKind::Right => (w.top as f32, w.bottom as f32),
        }
    }

    pub fn tangent_coord(kind: SurfaceKind, p: Vec2) -> f32 {
        match kind {
            SurfaceKind::Bottom | SurfaceKind::Top => p.x,
            SurfaceKind::Left | SurfaceKind::Right => p.y,
        }
    }

    pub fn point_from_tangent(&self, c: SurfaceContact, tangent_coord: f32, center_offset: f32) -> Vec2 {
        let w = self.surface_rect(c);
        if c.is_platform() {
            match c.kind {
                SurfaceKind::Bottom => Vec2::new(tangent_coord, w.top as f32 - center_offset),
                SurfaceKind::Top => Vec2::new(tangent_coord, w.bottom as f32 + center_offset),
                SurfaceKind::Left => Vec2::new(w.right as f32 + center_offset, tangent_coord),
                SurfaceKind::Right => Vec2::new(w.left as f32 - center_offset, tangent_coord),
            }
        } else {
            match c.kind {
                SurfaceKind::Bottom => Vec2::new(tangent_coord, w.bottom as f32 - center_offset),
                SurfaceKind::Top => Vec2::new(tangent_coord, w.top as f32 + center_offset),
                SurfaceKind::Left => Vec2::new(w.left as f32 + center_offset, tangent_coord),
                SurfaceKind::Right => Vec2::new(w.right as f32 - center_offset, tangent_coord),
            }
        }
    }

    pub fn constrain_to_surface(
        &self,
        c: SurfaceContact,
        center: Vec2,
        half_len: f32,
        center_offset: f32,
    ) -> (Vec2, bool, bool) {
        let (lo, hi) = self.tangent_extent(c);
        let coord = Self::tangent_coord(c.kind, center);
        let edge_pad = if c.is_platform() {
            0.36
        } else {
            self.monitor_corner_padding.unwrap_or(0.58)
        };
        let min = lo + half_len * edge_pad;
        let max = hi - half_len * edge_pad;
        let crossed_low = coord < min;
        let crossed_high = coord > max;
        let clamped = if min <= max {
            clampf(coord, min, max)
        } else {
            (lo + hi) * 0.5
        };
        (
            self.point_from_tangent(c, clamped, center_offset),
            crossed_low,
            crossed_high,
        )
    }

    pub fn transition_from_edge(
        &self,
        c: SurfaceContact,
        walk_sign: i32,
        from_high_edge: bool,
        half_len: f32,
        center_offset: f32,
    ) -> (SurfaceContact, Vec2) {
        if c.is_platform() {
            return self.transition_window_from_edge(c, walk_sign, from_high_edge, half_len, center_offset);
        }

        if let Some((neighbor, p)) = self.try_cross_to_neighbor(c, from_high_edge, half_len, center_offset) {
            return (neighbor, p);
        }

        let next_kind = Self::next_surface(c.kind, walk_sign);
        let next = SurfaceContact::monitor(c.monitor_index, next_kind);
        let w = self.monitor(c).work;
        let corner_padding = self.monitor_corner_padding.unwrap_or(0.60);
        let p = match (c.kind, from_high_edge) {
            (SurfaceKind::Bottom, true) => Vec2::new(
                w.right as f32 - center_offset,
                w.bottom as f32 - half_len * corner_padding,
            ),
            (SurfaceKind::Bottom, false) => Vec2::new(
                w.left as f32 + center_offset,
                w.bottom as f32 - half_len * corner_padding,
            ),
            (SurfaceKind::Top, true) => Vec2::new(
                w.left as f32 + center_offset,
                w.top as f32 + half_len * corner_padding,
            ),
            (SurfaceKind::Top, false) => Vec2::new(
                w.right as f32 - center_offset,
                w.top as f32 + half_len * corner_padding,
            ),
            (SurfaceKind::Right, true) => Vec2::new(
                w.right as f32 - half_len * corner_padding,
                w.top as f32 + center_offset,
            ),
            (SurfaceKind::Right, false) => Vec2::new(
                w.right as f32 - half_len * corner_padding,
                w.bottom as f32 - center_offset,
            ),
            (SurfaceKind::Left, true) => Vec2::new(
                w.left as f32 + half_len * corner_padding,
                w.bottom as f32 - center_offset,
            ),
            (SurfaceKind::Left, false) => Vec2::new(
                w.left as f32 + half_len * corner_padding,
                w.top as f32 + center_offset,
            ),
        };

        let p = match next.kind {
            SurfaceKind::Bottom | SurfaceKind::Top => self.point_from_tangent(next, p.x, center_offset),
            SurfaceKind::Left | SurfaceKind::Right => self.point_from_tangent(next, p.y, center_offset),
        };
        (next, p)
    }

    fn try_cross_to_neighbor(
        &self,
        c: SurfaceContact,
        from_high_edge: bool,
        half_len: f32,
        center_offset: f32,
    ) -> Option<(SurfaceContact, Vec2)> {
        if c.is_platform() {
            return None;
        }

        let current = self.monitor(c).work;
        let tolerance = 4;
        for (i, monitor) in self.monitors.iter().enumerate() {
            if i == c.monitor_index {
                continue;
            }
            let w = monitor.work;
            match c.kind {
                SurfaceKind::Bottom | SurfaceKind::Top => {
                    let same_line = if c.kind == SurfaceKind::Bottom {
                        (w.bottom - current.bottom).abs() <= tolerance
                    } else {
                        (w.top - current.top).abs() <= tolerance
                    };
                    if same_line {
                        if from_high_edge && (w.left - current.right).abs() <= tolerance {
                            let next = SurfaceContact::monitor(i, c.kind);
                            return Some((
                                next,
                                self.point_from_tangent(next, w.left as f32 + half_len * 0.7, center_offset),
                            ));
                        }
                        if !from_high_edge && (current.left - w.right).abs() <= tolerance {
                            let next = SurfaceContact::monitor(i, c.kind);
                            return Some((
                                next,
                                self.point_from_tangent(next, w.right as f32 - half_len * 0.7, center_offset),
                            ));
                        }
                    }
                }
                SurfaceKind::Left | SurfaceKind::Right => {
                    let same_line = if c.kind == SurfaceKind::Left {
                        (w.left - current.left).abs() <= tolerance
                    } else {
                        (w.right - current.right).abs() <= tolerance
                    };
                    if same_line {
                        if from_high_edge && (w.top - current.bottom).abs() <= tolerance {
                            let next = SurfaceContact::monitor(i, c.kind);
                            return Some((
                                next,
                                self.point_from_tangent(next, w.top as f32 + half_len * 0.7, center_offset),
                            ));
                        }
                        if !from_high_edge && (current.top - w.bottom).abs() <= tolerance {
                            let next = SurfaceContact::monitor(i, c.kind);
                            return Some((
                                next,
                                self.point_from_tangent(
                                    next,
                                    w.bottom as f32 - half_len * 0.7,
                                    center_offset,
                                ),
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    fn transition_window_from_edge(
        &self,
        c: SurfaceContact,
        walk_sign: i32,
        from_high_edge: bool,
        half_len: f32,
        center_offset: f32,
    ) -> (SurfaceContact, Vec2) {
        let Some(window) = self.contact_window(c) else {
            return (
                c,
                self.point_from_tangent(c, Self::tangent_coord(c.kind, Vec2::ZERO), center_offset),
            );
        };
        let next_kind = Self::next_window_surface(c.kind, walk_sign);
        let next = window.contact(next_kind);
        let pad = half_len * 0.50;
        let r = window.rect;
        let coord = match next_kind {
            SurfaceKind::Bottom | SurfaceKind::Top => match c.kind {
                SurfaceKind::Left => r.right as f32 - pad,
                SurfaceKind::Right => r.left as f32 + pad,
                SurfaceKind::Bottom | SurfaceKind::Top => {
                    if from_high_edge {
                        r.right as f32 - pad
                    } else {
                        r.left as f32 + pad
                    }
                }
            },
            SurfaceKind::Left | SurfaceKind::Right => match c.kind {
                SurfaceKind::Bottom => r.top as f32 + pad,
                SurfaceKind::Top => r.bottom as f32 - pad,
                SurfaceKind::Left | SurfaceKind::Right => {
                    if from_high_edge {
                        r.bottom as f32 - pad
                    } else {
                        r.top as f32 + pad
                    }
                }
            },
        };
        let point = self.point_from_tangent(next, coord, center_offset);
        if self.platform_supports(next, point, half_len) {
            return (next, point);
        }

        if let Some((bridge, bridge_point)) =
            self.find_visible_window_surface_near(c, next.kind, point, half_len, center_offset)
        {
            return (bridge, bridge_point);
        }

        (next, point)
    }

    fn find_visible_window_surface_near(
        &self,
        previous: SurfaceContact,
        preferred_kind: SurfaceKind,
        target: Vec2,
        half_len: f32,
        center_offset: f32,
    ) -> Option<(SurfaceContact, Vec2)> {
        let previous_index = self.contact_window_index(previous);
        let max_tangent_gap = half_len * 0.72 + center_offset * 0.22;
        let max_normal_gap = center_offset * 1.80 + half_len * 0.10 + 18.0;
        let kinds = [
            preferred_kind,
            previous.kind,
            SurfaceKind::Bottom,
            SurfaceKind::Top,
            SurfaceKind::Left,
            SurfaceKind::Right,
        ];

        let mut best: Option<(SurfaceContact, Vec2, f32)> = None;
        for (index, window) in self.windows.iter().enumerate() {
            for &kind in &kinds {
                let contact = window.contact(kind);
                if contact == previous {
                    continue;
                }
                let (lo, hi) = self.tangent_extent(contact);
                let wanted = Self::tangent_coord(kind, target);
                let tangent_gap = if wanted < lo {
                    lo - wanted
                } else if wanted > hi {
                    wanted - hi
                } else {
                    0.0
                };
                if tangent_gap > max_tangent_gap {
                    continue;
                }

                let coord = clampf(wanted, lo, hi);
                if !self.window_edge_contact_visible(index, kind, coord, half_len * 0.50) {
                    continue;
                }

                let snapped = self.point_from_tangent(contact, coord, center_offset);
                if !self.window_contact_has_room(contact, snapped, half_len) {
                    continue;
                }
                let delta = snapped - target;
                let normal_gap = delta.dot(Self::surface_normal(kind)).abs();
                if normal_gap > max_normal_gap {
                    continue;
                }

                let kind_penalty = if kind == preferred_kind {
                    0.0
                } else if kind == previous.kind {
                    14.0
                } else {
                    34.0
                };
                let z_penalty = match previous_index {
                    Some(previous_i) if index > previous_i => 32.0,
                    Some(previous_i) if index == previous_i => 8.0,
                    _ => 0.0,
                };
                let score =
                    delta.length() + tangent_gap * 0.55 + kind_penalty + z_penalty + index as f32 * 3.5;
                let replace = best.as_ref().map(|current| score < current.2).unwrap_or(true);
                if replace {
                    best = Some((contact, snapped, score));
                }
            }
        }

        best.map(|(contact, snapped, _)| (contact, snapped))
    }

    pub fn next_surface(kind: SurfaceKind, walk_sign: i32) -> SurfaceKind {
        match (kind, walk_sign >= 0) {
            (SurfaceKind::Bottom, true) => SurfaceKind::Right,
            (SurfaceKind::Right, true) => SurfaceKind::Top,
            (SurfaceKind::Top, true) => SurfaceKind::Left,
            (SurfaceKind::Left, true) => SurfaceKind::Bottom,
            (SurfaceKind::Bottom, false) => SurfaceKind::Left,
            (SurfaceKind::Left, false) => SurfaceKind::Top,
            (SurfaceKind::Top, false) => SurfaceKind::Right,
            (SurfaceKind::Right, false) => SurfaceKind::Bottom,
        }
    }

    fn next_window_surface(kind: SurfaceKind, walk_sign: i32) -> SurfaceKind {
        match (kind, walk_sign >= 0) {
            (SurfaceKind::Bottom, true) => SurfaceKind::Left,
            (SurfaceKind::Left, true) => SurfaceKind::Top,
            (SurfaceKind::Top, true) => SurfaceKind::Right,
            (SurfaceKind::Right, true) => SurfaceKind::Bottom,
            (SurfaceKind::Bottom, false) => SurfaceKind::Right,
            (SurfaceKind::Right, false) => SurfaceKind::Top,
            (SurfaceKind::Top, false) => SurfaceKind::Left,
            (SurfaceKind::Left, false) => SurfaceKind::Bottom,
        }
    }

    pub fn nearest_surface_with(&self, p: Vec2, half_len: f32, center_offset: f32) -> (SurfaceContact, Vec2) {
        let mut best = (
            SurfaceContact::monitor(0, SurfaceKind::Bottom),
            f32::MAX,
            Vec2::ZERO,
        );
        for (i, _m) in self.monitors.iter().enumerate() {
            let surfaces = [
                SurfaceKind::Bottom,
                SurfaceKind::Right,
                SurfaceKind::Top,
                SurfaceKind::Left,
            ];
            for kind in surfaces {
                let c = SurfaceContact::monitor(i, kind);
                let coord = Self::tangent_coord(kind, p);
                let (lo, hi) = self.tangent_extent(c);
                let clamped = clampf(coord, lo + half_len * 0.6, hi - half_len * 0.6);
                let snap = self.point_from_tangent(c, clamped, center_offset);
                let d = (snap - p).length_sq();
                if d < best.1 {
                    best = (c, d, snap);
                }
            }
        }

        (best.0, best.2)
    }

    pub fn try_find_collision_surface(
        &self,
        center: Vec2,
        velocity: Vec2,
        half_len: f32,
        center_offset: f32,
    ) -> Option<(SurfaceContact, Vec2, f32)> {
        let mut best_window: Option<(SurfaceContact, Vec2, f32, f32)> = None;
        for (index, window) in self.windows.iter().enumerate() {
            if let Some((contact, snapped, impact)) =
                self.try_find_window_collision(window, center, velocity, half_len, center_offset)
            {
                let coord = Self::tangent_coord(contact.kind, snapped);
                if !self.window_edge_contact_visible(index, contact.kind, coord, half_len * 0.46) {
                    continue;
                }
                if !self.window_contact_has_room(contact, snapped, half_len) {
                    continue;
                }
                let swat_speed = Self::window_surface_velocity(window, contact.kind).length();
                let score = impact + swat_speed * 0.28 - index as f32 * 7.0;
                let replace = best_window
                    .as_ref()
                    .map(|current| score > current.3)
                    .unwrap_or(true);
                if replace {
                    best_window = Some((contact, snapped, impact, score));
                }
            }
        }
        if let Some((contact, snapped, impact, _)) = best_window {
            return Some((contact, snapped, impact));
        }

        for (i, m) in self.monitors.iter().enumerate() {
            let w = m.work;
            if center.x >= w.left as f32 - 90.0
                && center.x <= w.right as f32 + 90.0
                && center.y >= w.top as f32 - 90.0
                && center.y <= w.bottom as f32 + 90.0
            {
                if center.y + center_offset >= w.bottom as f32 && velocity.y > 0.0 {
                    let c = SurfaceContact::monitor(i, SurfaceKind::Bottom);
                    return Some((
                        c,
                        self.point_from_tangent(c, center.x, center_offset),
                        velocity.y.abs(),
                    ));
                }
                if center.y - center_offset <= w.top as f32 && velocity.y < 0.0 {
                    let c = SurfaceContact::monitor(i, SurfaceKind::Top);
                    return Some((
                        c,
                        self.point_from_tangent(c, center.x, center_offset),
                        velocity.y.abs(),
                    ));
                }
                if center.x - center_offset <= w.left as f32 && velocity.x < 0.0 {
                    let c = SurfaceContact::monitor(i, SurfaceKind::Left);
                    return Some((
                        c,
                        self.point_from_tangent(c, center.y, center_offset),
                        velocity.x.abs(),
                    ));
                }
                if center.x + center_offset >= w.right as f32 && velocity.x > 0.0 {
                    let c = SurfaceContact::monitor(i, SurfaceKind::Right);
                    return Some((
                        c,
                        self.point_from_tangent(c, center.y, center_offset),
                        velocity.x.abs(),
                    ));
                }
            }
        }
        None
    }

    fn try_find_window_collision(
        &self,
        window: &WindowSurface,
        center: Vec2,
        velocity: Vec2,
        half_len: f32,
        center_offset: f32,
    ) -> Option<(SurfaceContact, Vec2, f32)> {
        let r = window.rect;
        let horizontal_pad = half_len * 0.55 + center_offset * 0.26;
        let vertical_pad = center_offset * 1.45;
        let mut best: Option<(SurfaceContact, Vec2, f32, f32)> = None;

        let mut candidate = |kind: SurfaceKind, penetration: f32, coord: f32, span_lo: f32, span_hi: f32| {
            if coord < span_lo - horizontal_pad || coord > span_hi + horizontal_pad {
                return;
            }
            let surface_velocity = Self::window_surface_velocity(window, kind);
            let relative = velocity - surface_velocity;
            let closing = relative.dot(Self::surface_normal(kind));
            if closing <= 0.0 {
                return;
            }
            let max_penetration = closing * (1.0 / 42.0) + center_offset * 0.45 + 8.0;
            if penetration < -4.0 || penetration > max_penetration {
                return;
            }

            let contact = window.contact(kind);
            let snapped = self.point_from_tangent(contact, clampf(coord, span_lo, span_hi), center_offset);
            let impact = closing.abs() + surface_velocity.length() * 0.16;
            let item = (contact, snapped, impact, penetration.abs());
            let replace = best.as_ref().map(|current| item.3 < current.3).unwrap_or(true);
            if replace {
                best = Some(item);
            }
        };

        candidate(
            SurfaceKind::Bottom,
            center.y + center_offset - r.top as f32,
            center.x,
            r.left as f32,
            r.right as f32,
        );
        candidate(
            SurfaceKind::Top,
            r.bottom as f32 - (center.y - center_offset),
            center.x,
            r.left as f32,
            r.right as f32,
        );

        let mut side_candidate = |kind: SurfaceKind, penetration: f32, coord: f32| {
            if coord < r.top as f32 - vertical_pad || coord > r.bottom as f32 + vertical_pad {
                return;
            }
            let surface_velocity = Self::window_surface_velocity(window, kind);
            let relative = velocity - surface_velocity;
            let closing = relative.dot(Self::surface_normal(kind));
            if closing <= 0.0 {
                return;
            }
            let max_penetration = closing * (1.0 / 42.0) + center_offset * 0.45 + 8.0;
            if penetration < -4.0 || penetration > max_penetration {
                return;
            }

            let contact = window.contact(kind);
            let snapped = self.point_from_tangent(
                contact,
                clampf(coord, r.top as f32, r.bottom as f32),
                center_offset,
            );
            let impact = closing.abs() + surface_velocity.length() * 0.16;
            let item = (contact, snapped, impact, penetration.abs());
            let replace = best.as_ref().map(|current| item.3 < current.3).unwrap_or(true);
            if replace {
                best = Some(item);
            }
        };

        side_candidate(
            SurfaceKind::Right,
            center.x + center_offset - r.left as f32,
            center.y,
        );
        side_candidate(
            SurfaceKind::Left,
            r.right as f32 - (center.x - center_offset),
            center.y,
        );

        best.map(|(contact, snapped, impact, _)| (contact, snapped, impact))
    }

    fn surface_rect(&self, c: SurfaceContact) -> RectI {
        if c.is_platform() {
            if let Some(window) = self.contact_window(c) {
                return window.rect;
            }
        }
        self.monitor(c).work
    }

    fn contact_window_index(&self, c: SurfaceContact) -> Option<usize> {
        let id = c.window_id?;
        self.windows.iter().position(|window| window.id == id)
    }

    fn window_contact_has_room(&self, c: SurfaceContact, center: Vec2, half_len: f32) -> bool {
        if !c.is_platform() || c.kind != SurfaceKind::Bottom {
            return true;
        }

        let Some(window) = self.contact_window(c) else {
            return false;
        };
        let monitor = &self.monitors[window.monitor_index.min(self.monitors.len() - 1)];
        let top = monitor.work.top as f32;
        let headroom = window.rect.top as f32 - top;
        let required = (half_len * WINDOW_TOP_SUPPORT_HEADROOM_FACTOR).max(WINDOW_TOP_SUPPORT_HEADROOM_MIN);
        headroom >= required && center.y - half_len * 0.72 >= top + 4.0
    }

    fn window_edge_contact_visible(
        &self,
        window_index: usize,
        kind: SurfaceKind,
        coord: f32,
        half_span: f32,
    ) -> bool {
        let Some(window) = self.windows.get(window_index) else {
            return false;
        };
        let contact = window.contact(kind);
        let (edge_lo, edge_hi) = self.tangent_extent(contact);
        if edge_hi - edge_lo <= 1.0 {
            return false;
        }

        let coord = coord.clamp(edge_lo, edge_hi);
        if !self.window_edge_point_visible(window_index, kind, coord) {
            return false;
        }

        if window_index == 0 {
            return true;
        }

        let span_radius = half_span.max(WINDOW_VISIBLE_CONTACT_MIN_PIXELS * 0.5);
        let span_lo = (coord - span_radius).max(edge_lo);
        let span_hi = (coord + span_radius).min(edge_hi);
        let span_len = span_hi - span_lo;
        if span_len <= 1.0 {
            return true;
        }

        let mut blocked: Vec<(f32, f32)> = Vec::new();
        for upper in self.windows.iter().take(window_index) {
            if let Some((lo, hi)) = Self::upper_window_edge_occlusion_interval(window, upper, kind) {
                let lo = lo.max(span_lo);
                let hi = hi.min(span_hi);
                if hi > lo {
                    blocked.push((lo, hi));
                }
            }
        }

        if blocked.is_empty() {
            return true;
        }

        blocked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut covered = 0.0;
        let mut cur_lo = blocked[0].0;
        let mut cur_hi = blocked[0].1;
        for &(lo, hi) in &blocked[1..] {
            if lo <= cur_hi {
                cur_hi = cur_hi.max(hi);
            } else {
                covered += cur_hi - cur_lo;
                cur_lo = lo;
                cur_hi = hi;
            }
        }
        covered += cur_hi - cur_lo;

        let visible = (span_len - covered).max(0.0);
        visible >= WINDOW_VISIBLE_CONTACT_MIN_PIXELS && visible / span_len >= WINDOW_VISIBLE_CONTACT_MIN_RATIO
    }

    fn window_edge_point_visible(&self, window_index: usize, kind: SurfaceKind, coord: f32) -> bool {
        if window_index == 0 {
            return true;
        }
        let Some(window) = self.windows.get(window_index) else {
            return false;
        };
        !self
            .windows
            .iter()
            .take(window_index)
            .any(|upper| Self::upper_window_occludes_edge_at(window, upper, kind, coord))
    }

    fn upper_window_occludes_edge_at(
        window: &WindowSurface,
        upper: &WindowSurface,
        kind: SurfaceKind,
        coord: f32,
    ) -> bool {
        let Some((lo, hi)) = Self::upper_window_edge_occlusion_interval(window, upper, kind) else {
            return false;
        };
        coord >= lo && coord <= hi
    }

    fn upper_window_edge_occlusion_interval(
        window: &WindowSurface,
        upper: &WindowSurface,
        kind: SurfaceKind,
    ) -> Option<(f32, f32)> {
        let r = window.rect;
        let u = upper.rect;
        let band = WINDOW_OCCLUSION_EDGE_BAND;

        match kind {
            SurfaceKind::Bottom => {
                let y = r.top as f32;
                if u.bottom as f32 >= y - band && u.top as f32 <= y + band {
                    Some((u.left as f32 - band, u.right as f32 + band))
                } else {
                    None
                }
            }
            SurfaceKind::Top => {
                let y = r.bottom as f32;
                if u.bottom as f32 >= y - band && u.top as f32 <= y + band {
                    Some((u.left as f32 - band, u.right as f32 + band))
                } else {
                    None
                }
            }
            SurfaceKind::Left => {
                let x = r.right as f32;
                if u.right as f32 >= x - band && u.left as f32 <= x + band {
                    Some((u.top as f32 - band, u.bottom as f32 + band))
                } else {
                    None
                }
            }
            SurfaceKind::Right => {
                let x = r.left as f32;
                if u.right as f32 >= x - band && u.left as f32 <= x + band {
                    Some((u.top as f32 - band, u.bottom as f32 + band))
                } else {
                    None
                }
            }
        }
    }

    fn monitor_index_for_rect(&self, rect: RectI) -> usize {
        let center = rect_center(rect);
        self.monitors
            .iter()
            .position(|monitor| monitor.bounds.inflate(32).contains(center))
            .unwrap_or_else(|| self.primary_index())
    }

    fn window_surface_velocity(window: &WindowSurface, kind: SurfaceKind) -> Vec2 {
        match kind {
            SurfaceKind::Bottom => Vec2::new(window.velocity.x, window.edge_velocity.top),
            SurfaceKind::Top => Vec2::new(window.velocity.x, window.edge_velocity.bottom),
            SurfaceKind::Left => Vec2::new(window.edge_velocity.right, window.velocity.y),
            SurfaceKind::Right => Vec2::new(window.edge_velocity.left, window.velocity.y),
        }
    }
}

fn rect_center(rect: RectI) -> Vec2 {
    Vec2::new(
        (rect.left + rect.right) as f32 * 0.5,
        (rect.top + rect.bottom) as f32 * 0.5,
    )
}

#[cfg(target_os = "windows")]
fn rect_from_win32(r: RECT) -> RectI {
    RectI::new(r.left, r.top, r.right, r.bottom)
}
