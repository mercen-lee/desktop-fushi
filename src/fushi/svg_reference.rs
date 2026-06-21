#![allow(dead_code)]

use crate::math::{PathCmd, Vec2};
// SVG-derived reference contours for Fushi appendages and the default face.
// Source: assets/fushi_ears_reference.svg, mapped from body_main_fill to renderer-local body coordinates.
// These are traced/simplified paths, not newly invented primitive shapes.

// Clean rear-ear outer silhouette.  This is the one contour used for both the fill and
// the final stroke, so the rim stays a uniform outline instead of becoming a thick
// painted patch on one side.
pub const REAR_EAR_OUTER_0: [Vec2; 17] = [
    Vec2::new(-131.5, -53.2),
    Vec2::new(-131.0, -61.2),
    Vec2::new(-128.6, -69.4),
    Vec2::new(-124.4, -76.2),
    Vec2::new(-119.8, -79.7),
    Vec2::new(-115.6, -78.8),
    Vec2::new(-112.4, -72.4),
    Vec2::new(-110.3, -61.4),
    Vec2::new(-110.1, -47.2),
    Vec2::new(-111.6, -41.5),
    Vec2::new(-115.4, -37.5),
    Vec2::new(-120.6, -35.8),
    Vec2::new(-125.6, -37.2),
    Vec2::new(-128.9, -41.5),
    Vec2::new(-130.7, -46.6),
    Vec2::new(-131.5, -50.4),
    Vec2::new(-131.6, -52.0),
];

pub static REAR_EAR_OUTER: &[&[Vec2]] = &[&REAR_EAR_OUTER_0];

pub const REAR_EAR_SHADOW_0: [Vec2; 19] = [
    Vec2::new(-1.31e+02, -55.4),
    Vec2::new(-1.3e+02, -63.3),
    Vec2::new(-1.28e+02, -68.6),
    Vec2::new(-1.25e+02, -73.8),
    Vec2::new(-1.21e+02, -77.5),
    Vec2::new(-1.2e+02, -78.1),
    Vec2::new(-1.18e+02, -71.7),
    Vec2::new(-1.18e+02, -65.2),
    Vec2::new(-1.2e+02, -65.2),
    Vec2::new(-1.21e+02, -64.6),
    Vec2::new(-1.24e+02, -61.7),
    Vec2::new(-1.25e+02, -57.6),
    Vec2::new(-1.27e+02, -51.8),
    Vec2::new(-1.27e+02, -48.1),
    Vec2::new(-1.26e+02, -43.4),
    Vec2::new(-1.23e+02, -34.2),
    Vec2::new(-1.25e+02, -33.4),
    Vec2::new(-1.29e+02, -41.1),
    Vec2::new(-1.31e+02, -49.8),
];

pub static REAR_EAR_SHADOW: &[&[Vec2]] = &[&REAR_EAR_SHADOW_0];

pub const REAR_EAR_MID_TONE_0: [Vec2; 17] = [
    Vec2::new(-1.23e+02, -40.4),
    Vec2::new(-1.22e+02, -45.1),
    Vec2::new(-1.2e+02, -51.9),
    Vec2::new(-1.18e+02, -62.1),
    Vec2::new(-1.17e+02, -69.7),
    Vec2::new(-1.18e+02, -73.2),
    Vec2::new(-1.19e+02, -78.3),
    Vec2::new(-1.18e+02, -78.3),
    Vec2::new(-1.15e+02, -77.4),
    Vec2::new(-1.14e+02, -75.3),
    Vec2::new(-1.12e+02, -71.5),
    Vec2::new(-1.11e+02, -66.9),
    Vec2::new(-1.1e+02, -59.1),
    Vec2::new(-1.11e+02, -47.2),
    Vec2::new(-1.1e+02, -39.3),
    Vec2::new(-1.22e+02, -34.7),
    Vec2::new(-1.23e+02, -37.5),
];

pub static REAR_EAR_MID_TONE: &[&[Vec2]] = &[&REAR_EAR_MID_TONE_0];

pub const REAR_EAR_BASE_EDGE_0: [Vec2; 5] = [
    Vec2::new(-1.21e+02, -35.1),
    Vec2::new(-1.13e+02, -38.5),
    Vec2::new(-1.1e+02, -39.2),
    Vec2::new(-1.1e+02, -38.3),
    Vec2::new(-1.21e+02, -34.1),
];

pub static REAR_EAR_BASE_EDGE: &[&[Vec2]] = &[&REAR_EAR_BASE_EDGE_0];

pub const REAR_EAR_INNER_FILL_0: [Vec2; 14] = [
    Vec2::new(-1.26e+02, -47.4),
    Vec2::new(-1.26e+02, -52.7),
    Vec2::new(-1.25e+02, -55.3),
    Vec2::new(-1.23e+02, -60.5),
    Vec2::new(-1.21e+02, -63.7),
    Vec2::new(-1.2e+02, -64.6),
    Vec2::new(-1.19e+02, -64.8),
    Vec2::new(-1.18e+02, -64.4),
    Vec2::new(-1.18e+02, -63.8),
    Vec2::new(-1.21e+02, -50.3),
    Vec2::new(-1.23e+02, -46.5),
    Vec2::new(-1.24e+02, -43.9),
    Vec2::new(-1.24e+02, -38.5),
    Vec2::new(-1.25e+02, -42.1),
];

pub static REAR_EAR_INNER_FILL: &[&[Vec2]] = &[&REAR_EAR_INNER_FILL_0];

pub const FRONT_EAR_OUTER_OUTLINE_0: [Vec2; 70] = [
    Vec2::new(-73.2, -37.0),
    Vec2::new(-72.8, -38.4),
    Vec2::new(-69.6, -43.7),
    Vec2::new(-68.0, -48.1),
    Vec2::new(-67.2, -48.0),
    Vec2::new(-69.4, -41.6),
    Vec2::new(-70.5, -40.0),
    Vec2::new(-70.2, -39.8),
    Vec2::new(-70.9, -35.9),
    Vec2::new(-71.5, -36.0),
    Vec2::new(-71.3, -38.8),
    Vec2::new(-72.0, -37.6),
    Vec2::new(-72.1, -36.5),
    Vec2::new(-71.7, -34.2),
    Vec2::new(-70.7, -33.6),
    Vec2::new(-70.1, -32.0),
    Vec2::new(-68.1, -29.2),
    Vec2::new(-65.4, -26.9),
    Vec2::new(-62.2, -25.6),
    Vec2::new(-60.6, -25.3),
    Vec2::new(-57.0, -25.0),
    Vec2::new(-56.3, -25.3),
    Vec2::new(-56.1, -31.5),
    Vec2::new(-60.8, -47.9),
    Vec2::new(-61.6, -52.0),
    Vec2::new(-61.7, -55.4),
    Vec2::new(-61.4, -57.1),
    Vec2::new(-60.5, -57.2),
    Vec2::new(-60.6, -55.2),
    Vec2::new(-58.7, -58.8),
    Vec2::new(-56.6, -61.7),
    Vec2::new(-54.5, -63.5),
    Vec2::new(-52.2, -64.5),
    Vec2::new(-50.0, -64.6),
    Vec2::new(-48.0, -63.8),
    Vec2::new(-45.2, -60.5),
    Vec2::new(-43.0, -55.7),
    Vec2::new(-41.7, -48.8),
    Vec2::new(-41.9, -42.0),
    Vec2::new(-43.6, -35.6),
    Vec2::new(-46.7, -30.3),
    Vec2::new(-51.5, -25.3),
    Vec2::new(-49.1, -23.7),
    Vec2::new(-48.0, -23.9),
    Vec2::new(-44.8, -26.7),
    Vec2::new(-42.1, -29.9),
    Vec2::new(-39.6, -33.8),
    Vec2::new(-37.0, -40.5),
    Vec2::new(-35.9, -45.8),
    Vec2::new(-35.7, -49.2),
    Vec2::new(-35.2, -48.7),
    Vec2::new(-35.3, -44.7),
    Vec2::new(-35.9, -43.5),
    Vec2::new(-35.7, -42.5),
    Vec2::new(-35.9, -40.9),
    Vec2::new(-36.8, -38.0),
    Vec2::new(-38.6, -33.8),
    Vec2::new(-42.6, -27.9),
    Vec2::new(-45.4, -24.9),
    Vec2::new(-48.4, -22.7),
    Vec2::new(-49.3, -22.7),
    Vec2::new(-51.0, -24.0),
    Vec2::new(-52.2, -24.4),
    Vec2::new(-58.4, -24.1),
    Vec2::new(-61.4, -24.4),
    Vec2::new(-64.2, -25.4),
    Vec2::new(-66.7, -26.8),
    Vec2::new(-68.9, -28.6),
    Vec2::new(-70.7, -30.8),
    Vec2::new(-72.1, -33.2),
];

pub const FRONT_EAR_OUTER_OUTLINE_1: [Vec2; 19] = [
    Vec2::new(-60.5, -50.8),
    Vec2::new(-55.2, -30.6),
    Vec2::new(-55.3, -25.2),
    Vec2::new(-53.5, -25.5),
    Vec2::new(-50.8, -26.9),
    Vec2::new(-47.4, -30.6),
    Vec2::new(-44.8, -34.9),
    Vec2::new(-43.1, -39.8),
    Vec2::new(-42.4, -44.7),
    Vec2::new(-42.6, -49.7),
    Vec2::new(-43.9, -55.8),
    Vec2::new(-45.5, -59.6),
    Vec2::new(-47.4, -62.1),
    Vec2::new(-48.9, -63.3),
    Vec2::new(-51.2, -63.9),
    Vec2::new(-52.9, -63.4),
    Vec2::new(-55.9, -60.9),
    Vec2::new(-58.5, -57.6),
    Vec2::new(-60.1, -54.4),
];

pub static FRONT_EAR_OUTER_OUTLINE: &[&[Vec2]] = &[&FRONT_EAR_OUTER_OUTLINE_0, &FRONT_EAR_OUTER_OUTLINE_1];

// Clean front-ear outer silhouette.  Earlier traces mixed inner/rim fragments into
// the outline; this is a single smooth leaf contour used for both fill and stroke.
pub const FRONT_EAR_OUTER_0: [Vec2; 20] = [
    Vec2::new(-73.0, -36.8),
    Vec2::new(-71.5, -42.2),
    Vec2::new(-68.0, -49.5),
    Vec2::new(-64.0, -59.5),
    Vec2::new(-59.5, -68.2),
    Vec2::new(-54.0, -74.5),
    Vec2::new(-48.5, -76.8),
    Vec2::new(-44.5, -75.0),
    Vec2::new(-41.0, -69.0),
    Vec2::new(-37.6, -60.2),
    Vec2::new(-35.8, -49.4),
    Vec2::new(-36.2, -42.0),
    Vec2::new(-38.8, -34.0),
    Vec2::new(-43.8, -27.0),
    Vec2::new(-49.0, -22.8),
    Vec2::new(-55.8, -24.0),
    Vec2::new(-62.8, -25.2),
    Vec2::new(-68.2, -29.0),
    Vec2::new(-71.5, -33.0),
    Vec2::new(-72.6, -35.2),
];

pub static FRONT_EAR_OUTER: &[&[Vec2]] = &[&FRONT_EAR_OUTER_0];

pub const FRONT_EAR_OUTER_SHADOW_0: [Vec2; 35] = [
    Vec2::new(-60.6, -55.2),
    Vec2::new(-60.4, -57.2),
    Vec2::new(-59.1, -62.0),
    Vec2::new(-57.0, -66.4),
    Vec2::new(-55.2, -69.1),
    Vec2::new(-51.7, -73.0),
    Vec2::new(-47.6, -76.3),
    Vec2::new(-46.1, -76.6),
    Vec2::new(-44.7, -75.8),
    Vec2::new(-43.3, -74.3),
    Vec2::new(-41.3, -70.5),
    Vec2::new(-37.6, -60.4),
    Vec2::new(-36.3, -54.3),
    Vec2::new(-35.7, -49.3),
    Vec2::new(-35.8, -46.6),
    Vec2::new(-37.0, -40.5),
    Vec2::new(-39.6, -33.8),
    Vec2::new(-43.8, -27.7),
    Vec2::new(-48.3, -23.8),
    Vec2::new(-49.1, -23.7),
    Vec2::new(-51.5, -25.3),
    Vec2::new(-47.5, -29.3),
    Vec2::new(-44.7, -33.3),
    Vec2::new(-43.2, -36.7),
    Vec2::new(-41.9, -42.0),
    Vec2::new(-41.7, -48.8),
    Vec2::new(-42.6, -54.0),
    Vec2::new(-43.6, -57.3),
    Vec2::new(-45.2, -60.5),
    Vec2::new(-47.0, -63.0),
    Vec2::new(-49.0, -64.3),
    Vec2::new(-51.1, -64.7),
    Vec2::new(-53.3, -64.1),
    Vec2::new(-55.9, -62.4),
    Vec2::new(-58.2, -59.6),
];

pub static FRONT_EAR_OUTER_SHADOW: &[&[Vec2]] = &[&FRONT_EAR_OUTER_SHADOW_0];

pub const FRONT_EAR_MID_TONE_0: [Vec2; 24] = [
    Vec2::new(-70.8, -35.9),
    Vec2::new(-70.2, -40.1),
    Vec2::new(-67.2, -48.0),
    Vec2::new(-64.4, -58.6),
    Vec2::new(-61.2, -66.1),
    Vec2::new(-57.7, -71.2),
    Vec2::new(-55.2, -73.8),
    Vec2::new(-52.3, -75.7),
    Vec2::new(-48.6, -76.7),
    Vec2::new(-53.6, -72.3),
    Vec2::new(-56.7, -68.7),
    Vec2::new(-60.1, -62.5),
    Vec2::new(-61.4, -57.8),
    Vec2::new(-61.7, -53.4),
    Vec2::new(-61.1, -49.2),
    Vec2::new(-56.1, -31.5),
    Vec2::new(-56.3, -25.3),
    Vec2::new(-57.2, -25.0),
    Vec2::new(-62.2, -25.6),
    Vec2::new(-63.9, -26.1),
    Vec2::new(-66.8, -28.0),
    Vec2::new(-69.2, -30.6),
    Vec2::new(-70.0, -32.1),
    Vec2::new(-70.7, -33.7),
];

pub static FRONT_EAR_MID_TONE: &[&[Vec2]] = &[&FRONT_EAR_MID_TONE_0];

pub const FRONT_EAR_INNER_FILL_0: [Vec2; 19] = [
    Vec2::new(-60.6, -50.9),
    Vec2::new(-60.1, -54.5),
    Vec2::new(-57.8, -58.6),
    Vec2::new(-54.9, -62.0),
    Vec2::new(-52.1, -63.7),
    Vec2::new(-49.6, -63.6),
    Vec2::new(-47.4, -62.1),
    Vec2::new(-45.0, -58.7),
    Vec2::new(-43.6, -54.8),
    Vec2::new(-42.6, -49.7),
    Vec2::new(-42.4, -44.7),
    Vec2::new(-43.1, -39.8),
    Vec2::new(-43.8, -37.3),
    Vec2::new(-45.3, -33.8),
    Vec2::new(-48.2, -29.6),
    Vec2::new(-51.3, -26.5),
    Vec2::new(-53.5, -25.5),
    Vec2::new(-55.3, -25.2),
    Vec2::new(-55.2, -30.6),
];

pub static FRONT_EAR_INNER_FILL: &[&[Vec2]] = &[&FRONT_EAR_INNER_FILL_0];

pub const FRONT_EAR_TINY_EDGE_0: [Vec2; 3] = [
    Vec2::new(-35.9, -43.5),
    Vec2::new(-35.4, -44.4),
    Vec2::new(-35.6, -42.5),
];

pub static FRONT_EAR_TINY_EDGE: &[&[Vec2]] = &[&FRONT_EAR_TINY_EDGE_0];

pub const FRONT_EAR_TINY_SHADOW_1_0: [Vec2; 4] = [
    Vec2::new(-71.7, -34.3),
    Vec2::new(-71.5, -35.9),
    Vec2::new(-70.9, -35.8),
    Vec2::new(-70.7, -33.7),
];

pub static FRONT_EAR_TINY_SHADOW_1: &[&[Vec2]] = &[&FRONT_EAR_TINY_SHADOW_1_0];

pub const FRONT_EAR_TINY_SHADOW_2_0: [Vec2; 3] = [
    Vec2::new(-70.5, -40.0),
    Vec2::new(-70.2, -40.2),
    Vec2::new(-70.2, -39.9),
];

pub static FRONT_EAR_TINY_SHADOW_2: &[&[Vec2]] = &[&FRONT_EAR_TINY_SHADOW_2_0];

pub const FRONT_EAR_TINY_EDGE_2_0: [Vec2; 4] = [
    Vec2::new(-72.1, -36.5),
    Vec2::new(-72.0, -37.6),
    Vec2::new(-71.3, -38.8),
    Vec2::new(-71.7, -34.4),
];

pub static FRONT_EAR_TINY_EDGE_2: &[&[Vec2]] = &[&FRONT_EAR_TINY_EDGE_2_0];

// Single exterior tail silhouette.  The old renderer filled the upper tail and the lower
// rim as separate shapes, then stroked only one of them; this contour gives the
// fill and the final outside stroke the same geometry.
pub const TAIL_OUTER_0: [Vec2; 67] = [
    Vec2::new(48.6, -72.5),
    Vec2::new(48.8, -73.3),
    Vec2::new(50.0, -73.8),
    Vec2::new(61.4, -72.4),
    Vec2::new(63.2, -72.6),
    Vec2::new(64.3, -73.2),
    Vec2::new(65.3, -76.9),
    Vec2::new(65.8, -93.2),
    Vec2::new(66.8, -96.1),
    Vec2::new(67.9, -95.8),
    Vec2::new(83.7, -86.1),
    Vec2::new(85.4, -85.6),
    Vec2::new(86.3, -85.9),
    Vec2::new(87.4, -87.2),
    Vec2::new(88.9, -91.0),
    Vec2::new(90.9, -101.0),
    Vec2::new(93.0, -104.0),
    Vec2::new(95.2, -104.0),
    Vec2::new(97.7, -103.0),
    Vec2::new(99.9, -100.0),
    Vec2::new(106.0, -85.5),
    Vec2::new(109.0, -83.7),
    Vec2::new(112.0, -84.4),
    Vec2::new(131.0, -95.4),
    Vec2::new(134.0, -95.5),
    Vec2::new(137.0, -94.5),
    Vec2::new(138.0, -92.4),
    Vec2::new(133.0, -68.2),
    Vec2::new(133.0, -64.0),
    Vec2::new(135.0, -62.7),
    Vec2::new(152.0, -62.4),
    Vec2::new(154.0, -61.4),
    Vec2::new(154.0, -59.9),
    Vec2::new(153.0, -56.9),
    Vec2::new(151.0, -54.4),
    Vec2::new(138.0, -42.6),
    Vec2::new(138.0, -41.3),
    Vec2::new(139.0, -40.5),
    Vec2::new(145.0, -37.6),
    Vec2::new(146.0, -36.8),
    Vec2::new(146.0, -35.3),
    Vec2::new(144.0, -32.9),
    Vec2::new(137.0, -29.6),
    Vec2::new(134.0, -30.6),
    Vec2::new(131.0, -30.7),
    Vec2::new(132.4, -28.4),
    Vec2::new(134.0, -27.5),
    Vec2::new(133.4, -25.9),
    Vec2::new(130.0, -24.6),
    Vec2::new(125.2, -23.8),
    Vec2::new(120.0, -22.6),
    Vec2::new(114.6, -21.1),
    Vec2::new(102.0, -17.0),
    Vec2::new(102.0, -17.2),
    Vec2::new(97.8, -17.4),
    Vec2::new(93.6, -18.5),
    Vec2::new(83.3, -23.8),
    Vec2::new(65.0, -34.6),
    Vec2::new(63.5, -37.0),
    Vec2::new(59.8, -48.7),
    Vec2::new(59.4, -51.8),
    Vec2::new(59.8, -54.1),
    Vec2::new(61.6, -55.2),
    Vec2::new(62.2, -56.7),
    Vec2::new(60.4, -56.6),
    Vec2::new(58.1, -54.4),
    Vec2::new(57.6, -54.6),
];

pub static TAIL_OUTER: &[&[Vec2]] = &[&TAIL_OUTER_0];

// Rear tail plane fill.  This is intentionally not the whole tail silhouette:
// the lower/front lobe is a separate foreground plane with its own fill and stroke.
pub const TAIL_REAR_PLANE_0: [Vec2; 60] = [
    Vec2::new(48.6, -72.5),
    Vec2::new(48.8, -73.3),
    Vec2::new(50.0, -73.8),
    Vec2::new(61.4, -72.4),
    Vec2::new(63.2, -72.6),
    Vec2::new(64.3, -73.2),
    Vec2::new(65.3, -76.9),
    Vec2::new(65.8, -93.2),
    Vec2::new(66.8, -96.1),
    Vec2::new(67.9, -95.8),
    Vec2::new(83.7, -86.1),
    Vec2::new(85.4, -85.6),
    Vec2::new(86.3, -85.9),
    Vec2::new(87.4, -87.2),
    Vec2::new(88.9, -91.0),
    Vec2::new(90.9, -101.0),
    Vec2::new(93.0, -104.0),
    Vec2::new(95.2, -104.0),
    Vec2::new(97.7, -103.0),
    Vec2::new(99.9, -100.0),
    Vec2::new(106.0, -85.5),
    Vec2::new(109.0, -83.7),
    Vec2::new(112.0, -84.4),
    Vec2::new(131.0, -95.4),
    Vec2::new(134.0, -95.5),
    Vec2::new(137.0, -94.5),
    Vec2::new(138.0, -92.4),
    Vec2::new(133.0, -68.2),
    Vec2::new(133.0, -64.0),
    Vec2::new(135.0, -62.7),
    Vec2::new(152.0, -62.4),
    Vec2::new(154.0, -61.4),
    Vec2::new(154.0, -59.9),
    Vec2::new(153.0, -56.9),
    Vec2::new(151.0, -54.4),
    Vec2::new(138.0, -42.6),
    Vec2::new(138.0, -41.3),
    Vec2::new(139.0, -40.5),
    Vec2::new(145.0, -37.6),
    Vec2::new(146.0, -36.8),
    Vec2::new(146.0, -35.3),
    Vec2::new(144.0, -32.9),
    Vec2::new(137.0, -29.6),
    Vec2::new(134.0, -30.6),
    Vec2::new(131.0, -30.7),
    Vec2::new(132.6, -28.7),
    Vec2::new(112.4, -27.1),
    Vec2::new(110.1, -27.4),
    Vec2::new(108.8, -28.9),
    Vec2::new(106.8, -36.0),
    Vec2::new(104.8, -37.3),
    Vec2::new(102.3, -37.2),
    Vec2::new(98.8, -35.8),
    Vec2::new(90.1, -29.6),
    Vec2::new(88.2, -29.2),
    Vec2::new(72.5, -48.2),
    Vec2::new(67.6, -52.8),
    Vec2::new(61.8, -55.9),
    Vec2::new(59.6, -54.6),
    Vec2::new(58.8, -52.0),
];

pub static TAIL_REAR_PLANE: &[&[Vec2]] = &[&TAIL_REAR_PLANE_0];

// Compatibility alias for the renderer; this now points at the rear plane, not TAIL_OUTER_0.
pub static TAIL_BACK_OUTLINE: &[&[Vec2]] = &[&TAIL_REAR_PLANE_0];

// Visible rear outline. This open contour belongs only to the rear/crown plane;
// the lower foreground lobe is stroked separately from its own closed fill path.
pub const TAIL_REAR_VISIBLE_EDGE_0: [Vec2; 46] = [
    Vec2::new(57.6, -54.6),
    Vec2::new(48.6, -72.5),
    Vec2::new(48.8, -73.3),
    Vec2::new(50.0, -73.8),
    Vec2::new(61.4, -72.4),
    Vec2::new(63.2, -72.6),
    Vec2::new(64.3, -73.2),
    Vec2::new(65.3, -76.9),
    Vec2::new(65.8, -93.2),
    Vec2::new(66.8, -96.1),
    Vec2::new(67.9, -95.8),
    Vec2::new(83.7, -86.1),
    Vec2::new(85.4, -85.6),
    Vec2::new(86.3, -85.9),
    Vec2::new(87.4, -87.2),
    Vec2::new(88.9, -91.0),
    Vec2::new(90.9, -101.0),
    Vec2::new(93.0, -104.0),
    Vec2::new(95.2, -104.0),
    Vec2::new(97.7, -103.0),
    Vec2::new(99.9, -100.0),
    Vec2::new(106.0, -85.5),
    Vec2::new(109.0, -83.7),
    Vec2::new(112.0, -84.4),
    Vec2::new(131.0, -95.4),
    Vec2::new(134.0, -95.5),
    Vec2::new(137.0, -94.5),
    Vec2::new(138.0, -92.4),
    Vec2::new(133.0, -68.2),
    Vec2::new(133.0, -64.0),
    Vec2::new(135.0, -62.7),
    Vec2::new(152.0, -62.4),
    Vec2::new(154.0, -61.4),
    Vec2::new(154.0, -59.9),
    Vec2::new(153.0, -56.9),
    Vec2::new(151.0, -54.4),
    Vec2::new(138.0, -42.6),
    Vec2::new(138.0, -41.3),
    Vec2::new(139.0, -40.5),
    Vec2::new(145.0, -37.6),
    Vec2::new(146.0, -36.8),
    Vec2::new(146.0, -35.3),
    Vec2::new(144.0, -32.9),
    Vec2::new(137.0, -29.6),
    Vec2::new(134.0, -30.6),
    Vec2::new(131.0, -30.7),
];

pub static TAIL_REAR_VISIBLE_EDGE: &[&[Vec2]] = &[&TAIL_REAR_VISIBLE_EDGE_0];

// Clean front lower lobe.  This deliberately replaces the older bottom-shadow trace for the
// visible front tail plane: fill and final stroke both use this exact contour.
pub const TAIL_FRONT_LOBE_0: [Vec2; 31] = [
    Vec2::new(58.8, -52.0),
    Vec2::new(59.6, -54.6),
    Vec2::new(61.8, -55.9),
    Vec2::new(67.6, -52.8),
    Vec2::new(72.5, -48.2),
    Vec2::new(88.2, -29.2),
    Vec2::new(90.1, -29.6),
    Vec2::new(98.8, -35.8),
    Vec2::new(102.3, -37.2),
    Vec2::new(104.8, -37.3),
    Vec2::new(106.8, -36.0),
    Vec2::new(108.8, -28.9),
    Vec2::new(110.1, -27.4),
    Vec2::new(112.4, -27.1),
    Vec2::new(132.6, -28.7),
    Vec2::new(135.1, -27.8),
    Vec2::new(134.9, -26.0),
    Vec2::new(132.6, -24.5),
    Vec2::new(128.0, -23.3),
    Vec2::new(122.4, -22.0),
    Vec2::new(116.2, -20.7),
    Vec2::new(109.0, -19.3),
    Vec2::new(101.2, -17.8),
    Vec2::new(97.1, -17.9),
    Vec2::new(93.0, -18.9),
    Vec2::new(83.0, -23.6),
    Vec2::new(65.0, -34.3),
    Vec2::new(63.2, -36.8),
    Vec2::new(59.8, -48.4),
    Vec2::new(58.9, -50.6),
    Vec2::new(58.7, -51.2),
];

pub static TAIL_FRONT_LOBE: &[&[Vec2]] = &[&TAIL_FRONT_LOBE_0];

// Visible stroke edge for the front lobe. It stops before the cream splash area,
// so the splash stays clean and does not need its own outline stroke.
pub const TAIL_FRONT_LOBE_VISIBLE_EDGE_0: [Vec2; 21] = [
    Vec2::new(132.6, -28.7),
    Vec2::new(135.1, -27.8),
    Vec2::new(134.9, -26.0),
    Vec2::new(132.6, -24.5),
    Vec2::new(128.0, -23.3),
    Vec2::new(122.4, -22.0),
    Vec2::new(116.2, -20.7),
    Vec2::new(109.0, -19.3),
    Vec2::new(101.2, -17.8),
    Vec2::new(97.1, -17.9),
    Vec2::new(93.0, -18.9),
    Vec2::new(83.0, -23.6),
    Vec2::new(65.0, -34.3),
    Vec2::new(63.2, -36.8),
    Vec2::new(59.8, -48.4),
    Vec2::new(58.9, -50.6),
    Vec2::new(58.7, -51.2),
    Vec2::new(58.8, -52.0),
    Vec2::new(59.6, -54.6),
    Vec2::new(61.8, -55.9),
    Vec2::new(67.6, -52.8),
];

pub static TAIL_FRONT_LOBE_VISIBLE_EDGE: &[&[Vec2]] = &[&TAIL_FRONT_LOBE_VISIBLE_EDGE_0];

pub const TAIL_DARK_MAIN_0: [Vec2; 102] = [
    Vec2::new(48.6, -72.5),
    Vec2::new(48.8, -73.3),
    Vec2::new(50.0, -73.8),
    Vec2::new(61.4, -72.4),
    Vec2::new(63.2, -72.6),
    Vec2::new(64.3, -73.2),
    Vec2::new(65.3, -76.9),
    Vec2::new(65.8, -93.2),
    Vec2::new(66.8, -96.1),
    Vec2::new(67.9, -95.8),
    Vec2::new(83.7, -86.1),
    Vec2::new(85.4, -85.6),
    Vec2::new(86.3, -85.9),
    Vec2::new(87.4, -87.2),
    Vec2::new(88.9, -91.0),
    Vec2::new(90.9, -1.01e+02),
    Vec2::new(93.0, -1.04e+02),
    Vec2::new(95.2, -1.04e+02),
    Vec2::new(97.7, -1.03e+02),
    Vec2::new(99.9, -1e+02),
    Vec2::new(1.06e+02, -85.5),
    Vec2::new(1.09e+02, -83.7),
    Vec2::new(1.12e+02, -84.4),
    Vec2::new(1.31e+02, -95.4),
    Vec2::new(1.34e+02, -95.5),
    Vec2::new(1.37e+02, -94.5),
    Vec2::new(1.38e+02, -92.4),
    Vec2::new(1.33e+02, -68.2),
    Vec2::new(1.33e+02, -64.0),
    Vec2::new(1.35e+02, -62.7),
    Vec2::new(1.52e+02, -62.4),
    Vec2::new(1.54e+02, -61.4),
    Vec2::new(1.54e+02, -59.9),
    Vec2::new(1.53e+02, -56.9),
    Vec2::new(1.51e+02, -54.4),
    Vec2::new(1.38e+02, -42.6),
    Vec2::new(1.38e+02, -41.3),
    Vec2::new(1.39e+02, -40.5),
    Vec2::new(1.45e+02, -37.6),
    Vec2::new(1.46e+02, -36.8),
    Vec2::new(1.46e+02, -35.3),
    Vec2::new(1.44e+02, -32.9),
    Vec2::new(1.37e+02, -29.6),
    Vec2::new(1.34e+02, -30.6),
    Vec2::new(1.31e+02, -30.7),
    Vec2::new(1.11e+02, -28.4),
    Vec2::new(1.1e+02, -28.9),
    Vec2::new(1.1e+02, -29.7),
    Vec2::new(1.13e+02, -30.3),
    Vec2::new(1.14e+02, -31.2),
    Vec2::new(1.17e+02, -31.3),
    Vec2::new(1.19e+02, -32.4),
    Vec2::new(1.21e+02, -32.4),
    Vec2::new(1.21e+02, -33.1),
    Vec2::new(1.21e+02, -34.1),
    Vec2::new(1.21e+02, -35.2),
    Vec2::new(1.16e+02, -38.3),
    Vec2::new(1.16e+02, -39.3),
    Vec2::new(1.16e+02, -40.1),
    Vec2::new(1.25e+02, -48.9),
    Vec2::new(1.26e+02, -50.3),
    Vec2::new(1.25e+02, -51.2),
    Vec2::new(1.22e+02, -52.6),
    Vec2::new(1.21e+02, -52.2),
    Vec2::new(1.19e+02, -52.7),
    Vec2::new(1.14e+02, -52.6),
    Vec2::new(1.13e+02, -53.1),
    Vec2::new(1.14e+02, -54.3),
    Vec2::new(1.13e+02, -54.9),
    Vec2::new(1.15e+02, -58.7),
    Vec2::new(1.17e+02, -65.4),
    Vec2::new(1.18e+02, -65.8),
    Vec2::new(1.16e+02, -67.6),
    Vec2::new(1.14e+02, -68.3),
    Vec2::new(1.13e+02, -68.0),
    Vec2::new(1.07e+02, -63.6),
    Vec2::new(1.05e+02, -63.2),
    Vec2::new(1.04e+02, -61.7),
    Vec2::new(1.02e+02, -61.7),
    Vec2::new(1.02e+02, -60.6),
    Vec2::new(1.02e+02, -60.4),
    Vec2::new(1e+02, -61.4),
    Vec2::new(95.9, -72.6),
    Vec2::new(93.5, -74.4),
    Vec2::new(91.4, -74.6),
    Vec2::new(89.8, -73.5),
    Vec2::new(88.3, -71.3),
    Vec2::new(85.9, -60.1),
    Vec2::new(77.5, -65.0),
    Vec2::new(75.7, -65.3),
    Vec2::new(73.6, -64.4),
    Vec2::new(72.8, -63.1),
    Vec2::new(72.8, -61.3),
    Vec2::new(74.8, -53.8),
    Vec2::new(74.4, -53.2),
    Vec2::new(72.2, -52.5),
    Vec2::new(71.4, -50.8),
    Vec2::new(66.7, -54.6),
    Vec2::new(62.2, -56.7),
    Vec2::new(60.4, -56.6),
    Vec2::new(58.1, -54.4),
    Vec2::new(57.6, -54.6),
];

pub static TAIL_DARK_MAIN: &[&[Vec2]] = &[&TAIL_DARK_MAIN_0];

pub const TAIL_WHITE_SPLASH_0: [Vec2; 45] = [
    Vec2::new(72.4, -50.9),
    Vec2::new(72.8, -51.8),
    Vec2::new(75.1, -52.8),
    Vec2::new(75.4, -53.6),
    Vec2::new(73.5, -61.7),
    Vec2::new(73.6, -63.2),
    Vec2::new(74.1, -64.1),
    Vec2::new(75.1, -64.7),
    Vec2::new(76.6, -64.6),
    Vec2::new(84.5, -59.8),
    Vec2::new(86.1, -59.4),
    Vec2::new(87.0, -60.9),
    Vec2::new(89.3, -71.0),
    Vec2::new(90.7, -73.5),
    Vec2::new(92.4, -74.2),
    Vec2::new(93.5, -73.8),
    Vec2::new(95.4, -71.4),
    Vec2::new(99.8, -60.8),
    Vec2::new(1.01e+02, -59.7),
    Vec2::new(1.02e+02, -59.6),
    Vec2::new(1.04e+02, -60.3),
    Vec2::new(1.14e+02, -67.6),
    Vec2::new(1.16e+02, -67.5),
    Vec2::new(1.16e+02, -65.9),
    Vec2::new(1.12e+02, -54.5),
    Vec2::new(1.12e+02, -53.2),
    Vec2::new(1.13e+02, -52.3),
    Vec2::new(1.24e+02, -51.2),
    Vec2::new(1.25e+02, -50.0),
    Vec2::new(1.14e+02, -39.2),
    Vec2::new(1.15e+02, -38.0),
    Vec2::new(1.19e+02, -35.4),
    Vec2::new(1.21e+02, -33.8),
    Vec2::new(1.19e+02, -32.4),
    Vec2::new(1.1e+02, -30.6),
    Vec2::new(1.09e+02, -35.7),
    Vec2::new(1.08e+02, -37.6),
    Vec2::new(1.06e+02, -38.7),
    Vec2::new(1.04e+02, -39.1),
    Vec2::new(99.2, -38.1),
    Vec2::new(95.4, -35.8),
    Vec2::new(89.7, -31.4),
    Vec2::new(88.3, -31.3),
    Vec2::new(77.6, -43.7),
    Vec2::new(72.9, -49.5),
];

pub static TAIL_WHITE_SPLASH: &[&[Vec2]] = &[&TAIL_WHITE_SPLASH_0];

pub const TAIL_BOTTOM_SHADOW_0: [Vec2; 31] = [
    Vec2::new(58.8, -52.0),
    Vec2::new(59.6, -54.6),
    Vec2::new(61.8, -55.9),
    Vec2::new(67.6, -52.8),
    Vec2::new(72.5, -48.2),
    Vec2::new(88.2, -29.2),
    Vec2::new(90.1, -29.6),
    Vec2::new(98.8, -35.8),
    Vec2::new(102.3, -37.2),
    Vec2::new(104.8, -37.3),
    Vec2::new(106.8, -36.0),
    Vec2::new(108.8, -28.9),
    Vec2::new(110.1, -27.4),
    Vec2::new(112.4, -27.1),
    Vec2::new(132.6, -28.7),
    Vec2::new(135.1, -27.8),
    Vec2::new(134.9, -26.0),
    Vec2::new(132.6, -24.5),
    Vec2::new(128.0, -23.3),
    Vec2::new(122.4, -22.0),
    Vec2::new(116.2, -20.7),
    Vec2::new(109.0, -19.3),
    Vec2::new(101.2, -17.8),
    Vec2::new(97.1, -17.9),
    Vec2::new(93.0, -18.9),
    Vec2::new(83.0, -23.6),
    Vec2::new(65.0, -34.3),
    Vec2::new(63.2, -36.8),
    Vec2::new(59.8, -48.4),
    Vec2::new(58.9, -50.6),
    Vec2::new(58.7, -51.2),
];

pub static TAIL_BOTTOM_SHADOW: &[&[Vec2]] = &[&TAIL_BOTTOM_SHADOW_0];

pub const TAIL_UPPER_DARK_ACCENT_0: [Vec2; 28] = [
    Vec2::new(89.9, -73.5),
    Vec2::new(90.9, -74.4),
    Vec2::new(92.4, -74.7),
    Vec2::new(95.4, -73.2),
    Vec2::new(96.6, -71.3),
    Vec2::new(1e+02, -61.4),
    Vec2::new(1.02e+02, -60.4),
    Vec2::new(1.02e+02, -60.6),
    Vec2::new(1.02e+02, -61.7),
    Vec2::new(1.04e+02, -61.7),
    Vec2::new(1.05e+02, -63.2),
    Vec2::new(1.07e+02, -63.6),
    Vec2::new(1.13e+02, -68.0),
    Vec2::new(1.14e+02, -68.3),
    Vec2::new(1.16e+02, -68.0),
    Vec2::new(1.17e+02, -66.6),
    Vec2::new(1.15e+02, -59.8),
    Vec2::new(1.14e+02, -59.0),
    Vec2::new(1.16e+02, -65.9),
    Vec2::new(1.16e+02, -66.9),
    Vec2::new(1.16e+02, -67.7),
    Vec2::new(1.14e+02, -67.5),
    Vec2::new(1.04e+02, -60.5),
    Vec2::new(1.01e+02, -59.5),
    Vec2::new(99.8, -60.8),
    Vec2::new(96.1, -70.0),
    Vec2::new(94.2, -73.0),
    Vec2::new(92.4, -74.2),
];

pub static TAIL_UPPER_DARK_ACCENT: &[&[Vec2]] = &[&TAIL_UPPER_DARK_ACCENT_0];

pub const TAIL_ROOT_SHADOW_0: [Vec2; 9] = [
    Vec2::new(50.3, -53.0),
    Vec2::new(50.7, -54.7),
    Vec2::new(51.6, -55.6),
    Vec2::new(53.6, -55.5),
    Vec2::new(55.3, -54.5),
    Vec2::new(57.8, -52.0),
    Vec2::new(58.7, -46.2),
    Vec2::new(53.6, -50.6),
    Vec2::new(50.7, -52.1),
];

pub static TAIL_ROOT_SHADOW: &[&[Vec2]] = &[&TAIL_ROOT_SHADOW_0];

pub const TAIL_SPLASH_RIGHT_SHADOW_0: [Vec2; 20] = [
    Vec2::new(1.14e+02, -38.9),
    Vec2::new(1.15e+02, -40.3),
    Vec2::new(1.25e+02, -49.7),
    Vec2::new(1.25e+02, -50.4),
    Vec2::new(1.24e+02, -51.1),
    Vec2::new(1.22e+02, -51.4),
    Vec2::new(1.24e+02, -52.0),
    Vec2::new(1.25e+02, -51.0),
    Vec2::new(1.26e+02, -50.3),
    Vec2::new(1.25e+02, -49.1),
    Vec2::new(1.16e+02, -40.1),
    Vec2::new(1.16e+02, -39.3),
    Vec2::new(1.16e+02, -38.3),
    Vec2::new(1.21e+02, -35.0),
    Vec2::new(1.22e+02, -33.8),
    Vec2::new(1.21e+02, -32.9),
    Vec2::new(1.19e+02, -32.4),
    Vec2::new(1.21e+02, -33.8),
    Vec2::new(1.19e+02, -35.4),
    Vec2::new(1.15e+02, -38.0),
];

pub static TAIL_SPLASH_RIGHT_SHADOW: &[&[Vec2]] = &[&TAIL_SPLASH_RIGHT_SHADOW_0];

pub const TAIL_SPLASH_UPPER_EDGE_0: [Vec2; 9] = [
    Vec2::new(1.13e+02, -52.4),
    Vec2::new(1.14e+02, -52.6),
    Vec2::new(1.17e+02, -52.2),
    Vec2::new(1.17e+02, -52.6),
    Vec2::new(1.19e+02, -52.7),
    Vec2::new(1.21e+02, -52.2),
    Vec2::new(1.22e+02, -52.6),
    Vec2::new(1.23e+02, -52.0),
    Vec2::new(1.22e+02, -51.4),
];

pub static TAIL_SPLASH_UPPER_EDGE: &[&[Vec2]] = &[&TAIL_SPLASH_UPPER_EDGE_0];

pub const TAIL_SPLASH_LEFT_EDGE_0: [Vec2; 9] = [
    Vec2::new(1.13e+02, -53.6),
    Vec2::new(1.14e+02, -58.9),
    Vec2::new(1.15e+02, -60.0),
    Vec2::new(1.17e+02, -66.5),
    Vec2::new(1.17e+02, -65.4),
    Vec2::new(1.13e+02, -54.8),
    Vec2::new(1.13e+02, -53.0),
    Vec2::new(1.14e+02, -52.7),
    Vec2::new(1.13e+02, -52.5),
];

pub static TAIL_SPLASH_LEFT_EDGE: &[&[Vec2]] = &[&TAIL_SPLASH_LEFT_EDGE_0];

pub const TAIL_SPLASH_LOWER_EDGE_0: [Vec2; 3] = [
    Vec2::new(1.15e+02, -31.2),
    Vec2::new(1.19e+02, -32.3),
    Vec2::new(1.17e+02, -31.3),
];

pub static TAIL_SPLASH_LOWER_EDGE: &[&[Vec2]] = &[&TAIL_SPLASH_LOWER_EDGE_0];

pub const TAIL_SPLASH_LEFT_CORNER_SHADOW_0: [Vec2; 5] = [
    Vec2::new(1.12e+02, -53.7),
    Vec2::new(1.14e+02, -58.7),
    Vec2::new(1.13e+02, -54.0),
    Vec2::new(1.13e+02, -52.7),
    Vec2::new(1.12e+02, -53.0),
];

pub static TAIL_SPLASH_LEFT_CORNER_SHADOW: &[&[Vec2]] = &[&TAIL_SPLASH_LEFT_CORNER_SHADOW_0];

pub const FACE_DEFAULT_MOUTH_LINE_0: [Vec2; 28] = [
    Vec2::new(-1.5e+02, 28.1),
    Vec2::new(-1.5e+02, 27.0),
    Vec2::new(-1.49e+02, 26.6),
    Vec2::new(-1.46e+02, 28.1),
    Vec2::new(-1.45e+02, 28.5),
    Vec2::new(-1.44e+02, 28.1),
    Vec2::new(-1.41e+02, 26.4),
    Vec2::new(-1.39e+02, 26.1),
    Vec2::new(-1.35e+02, 26.9),
    Vec2::new(-1.27e+02, 29.7),
    Vec2::new(-1.25e+02, 29.6),
    Vec2::new(-1.22e+02, 28.4),
    Vec2::new(-1.21e+02, 28.8),
    Vec2::new(-1.21e+02, 29.6),
    Vec2::new(-1.22e+02, 31.1),
    Vec2::new(-1.23e+02, 31.8),
    Vec2::new(-1.25e+02, 31.8),
    Vec2::new(-1.29e+02, 31.1),
    Vec2::new(-1.3e+02, 33.5),
    Vec2::new(-1.31e+02, 34.0),
    Vec2::new(-1.33e+02, 34.3),
    Vec2::new(-1.36e+02, 34.0),
    Vec2::new(-1.39e+02, 33.0),
    Vec2::new(-1.41e+02, 31.3),
    Vec2::new(-1.43e+02, 29.7),
    Vec2::new(-1.46e+02, 30.4),
    Vec2::new(-1.48e+02, 30.2),
    Vec2::new(-1.5e+02, 29.2),
];

pub const FACE_DEFAULT_MOUTH_LINE_1: [Vec2; 15] = [
    Vec2::new(-1.42e+02, 28.7),
    Vec2::new(-1.41e+02, 30.0),
    Vec2::new(-1.4e+02, 30.8),
    Vec2::new(-1.38e+02, 31.9),
    Vec2::new(-1.35e+02, 32.6),
    Vec2::new(-1.33e+02, 32.9),
    Vec2::new(-1.31e+02, 32.8),
    Vec2::new(-1.31e+02, 32.2),
    Vec2::new(-1.3e+02, 31.2),
    Vec2::new(-1.3e+02, 30.5),
    Vec2::new(-1.31e+02, 29.8),
    Vec2::new(-1.34e+02, 28.4),
    Vec2::new(-1.36e+02, 27.7),
    Vec2::new(-1.4e+02, 27.4),
    Vec2::new(-1.41e+02, 27.8),
];

pub static FACE_DEFAULT_MOUTH_LINE: &[&[Vec2]] = &[&FACE_DEFAULT_MOUTH_LINE_0, &FACE_DEFAULT_MOUTH_LINE_1];

pub const FACE_DEFAULT_TONGUE_0: [Vec2; 16] = [
    Vec2::new(-1.42e+02, 28.7),
    Vec2::new(-1.41e+02, 27.8),
    Vec2::new(-1.39e+02, 27.4),
    Vec2::new(-1.39e+02, 27.3),
    Vec2::new(-1.36e+02, 27.7),
    Vec2::new(-1.34e+02, 28.4),
    Vec2::new(-1.31e+02, 29.8),
    Vec2::new(-1.3e+02, 30.5),
    Vec2::new(-1.3e+02, 31.2),
    Vec2::new(-1.31e+02, 32.2),
    Vec2::new(-1.31e+02, 32.8),
    Vec2::new(-1.33e+02, 32.9),
    Vec2::new(-1.35e+02, 32.6),
    Vec2::new(-1.38e+02, 31.9),
    Vec2::new(-1.4e+02, 30.8),
    Vec2::new(-1.41e+02, 30.0),
];

pub static FACE_DEFAULT_TONGUE: &[&[Vec2]] = &[&FACE_DEFAULT_TONGUE_0];

pub const FACE_DEFAULT_LEFT_EYE_BLACK_0: [Vec2; 16] = [
    Vec2::new(-1.52e+02, 9.79),
    Vec2::new(-1.52e+02, 6.19),
    Vec2::new(-1.51e+02, 4.03),
    Vec2::new(-1.5e+02, 2.06),
    Vec2::new(-1.49e+02, 0.767),
    Vec2::new(-1.48e+02, -0.0999),
    Vec2::new(-1.46e+02, -0.255),
    Vec2::new(-1.45e+02, 0.21),
    Vec2::new(-1.44e+02, 0.95),
    Vec2::new(-1.44e+02, 1.86),
    Vec2::new(-1.44e+02, 4.55),
    Vec2::new(-1.46e+02, 7.89),
    Vec2::new(-1.47e+02, 9.88),
    Vec2::new(-1.48e+02, 11.8),
    Vec2::new(-1.5e+02, 12.5),
    Vec2::new(-1.52e+02, 11.5),
];

pub const FACE_DEFAULT_LEFT_EYE_BLACK_1: [Vec2; 15] = [
    Vec2::new(-1.52e+02, 9.09),
    Vec2::new(-1.51e+02, 11.1),
    Vec2::new(-1.5e+02, 11.8),
    Vec2::new(-1.48e+02, 11.0),
    Vec2::new(-1.47e+02, 9.57),
    Vec2::new(-1.46e+02, 7.82),
    Vec2::new(-1.46e+02, 6.43),
    Vec2::new(-1.45e+02, 4.34),
    Vec2::new(-1.45e+02, 2.03),
    Vec2::new(-1.46e+02, 0.844),
    Vec2::new(-1.47e+02, 0.603),
    Vec2::new(-1.48e+02, 0.845),
    Vec2::new(-1.49e+02, 2.92),
    Vec2::new(-1.5e+02, 3.17),
    Vec2::new(-1.51e+02, 6.45),
];

pub static FACE_DEFAULT_LEFT_EYE_BLACK: &[&[Vec2]] =
    &[&FACE_DEFAULT_LEFT_EYE_BLACK_0, &FACE_DEFAULT_LEFT_EYE_BLACK_1];

pub const FACE_DEFAULT_LEFT_EYE_DEEP_BLACK_0: [Vec2; 15] = [
    Vec2::new(-1.52e+02, 9.09),
    Vec2::new(-1.52e+02, 7.31),
    Vec2::new(-1.51e+02, 5.61),
    Vec2::new(-1.5e+02, 3.17),
    Vec2::new(-1.49e+02, 3.01),
    Vec2::new(-1.49e+02, 4.79),
    Vec2::new(-1.49e+02, 5.96),
    Vec2::new(-1.48e+02, 6.77),
    Vec2::new(-1.47e+02, 6.94),
    Vec2::new(-1.46e+02, 6.56),
    Vec2::new(-1.46e+02, 8.48),
    Vec2::new(-1.48e+02, 10.6),
    Vec2::new(-1.5e+02, 11.7),
    Vec2::new(-1.51e+02, 11.7),
    Vec2::new(-1.51e+02, 11.1),
];

pub static FACE_DEFAULT_LEFT_EYE_DEEP_BLACK: &[&[Vec2]] = &[&FACE_DEFAULT_LEFT_EYE_DEEP_BLACK_0];

pub const FACE_DEFAULT_LEFT_EYE_HIGHLIGHT_0: [Vec2; 15] = [
    Vec2::new(-1.49e+02, 4.18),
    Vec2::new(-1.49e+02, 3.02),
    Vec2::new(-1.48e+02, 1.16),
    Vec2::new(-1.48e+02, 0.845),
    Vec2::new(-1.47e+02, 0.603),
    Vec2::new(-1.46e+02, 0.844),
    Vec2::new(-1.46e+02, 1.13),
    Vec2::new(-1.45e+02, 2.03),
    Vec2::new(-1.45e+02, 3.79),
    Vec2::new(-1.45e+02, 5.45),
    Vec2::new(-1.46e+02, 6.49),
    Vec2::new(-1.47e+02, 6.84),
    Vec2::new(-1.47e+02, 6.91),
    Vec2::new(-1.48e+02, 6.42),
    Vec2::new(-1.49e+02, 5.4),
];

pub static FACE_DEFAULT_LEFT_EYE_HIGHLIGHT: &[&[Vec2]] = &[&FACE_DEFAULT_LEFT_EYE_HIGHLIGHT_0];

pub const FACE_DEFAULT_RIGHT_EYE_BLACK_0: [Vec2; 22] = [
    Vec2::new(-1.17e+02, 8.66),
    Vec2::new(-1.17e+02, 6.25),
    Vec2::new(-1.16e+02, 5.0),
    Vec2::new(-1.15e+02, 3.14),
    Vec2::new(-1.13e+02, 2.23),
    Vec2::new(-1.11e+02, 2.17),
    Vec2::new(-1.09e+02, 2.92),
    Vec2::new(-1.08e+02, 3.64),
    Vec2::new(-1.06e+02, 5.37),
    Vec2::new(-1.05e+02, 8.54),
    Vec2::new(-1.04e+02, 11.1),
    Vec2::new(-1.04e+02, 13.0),
    Vec2::new(-1.04e+02, 14.8),
    Vec2::new(-1.04e+02, 16.4),
    Vec2::new(-1.05e+02, 17.5),
    Vec2::new(-1.07e+02, 18.6),
    Vec2::new(-1.09e+02, 18.5),
    Vec2::new(-1.11e+02, 17.8),
    Vec2::new(-1.13e+02, 16.6),
    Vec2::new(-1.15e+02, 15.0),
    Vec2::new(-1.16e+02, 13.1),
    Vec2::new(-1.17e+02, 11.0),
];

pub const FACE_DEFAULT_RIGHT_EYE_BLACK_1: [Vec2; 22] = [
    Vec2::new(-1.16e+02, 7.97),
    Vec2::new(-1.16e+02, 9.88),
    Vec2::new(-1.16e+02, 11.7),
    Vec2::new(-1.15e+02, 13.3),
    Vec2::new(-1.14e+02, 14.8),
    Vec2::new(-1.13e+02, 16.1),
    Vec2::new(-1.11e+02, 17.1),
    Vec2::new(-1.09e+02, 17.9),
    Vec2::new(-1.07e+02, 17.4),
    Vec2::new(-1.05e+02, 15.9),
    Vec2::new(-1.05e+02, 15.1),
    Vec2::new(-1.04e+02, 13.3),
    Vec2::new(-1.05e+02, 10.6),
    Vec2::new(-1.05e+02, 9.69),
    Vec2::new(-1.06e+02, 8.89),
    Vec2::new(-1.07e+02, 7.27),
    Vec2::new(-1.07e+02, 5.92),
    Vec2::new(-1.09e+02, 4.31),
    Vec2::new(-1.11e+02, 3.18),
    Vec2::new(-1.12e+02, 3.11),
    Vec2::new(-1.14e+02, 3.59),
    Vec2::new(-1.16e+02, 5.32),
];

pub static FACE_DEFAULT_RIGHT_EYE_BLACK: &[&[Vec2]] =
    &[&FACE_DEFAULT_RIGHT_EYE_BLACK_0, &FACE_DEFAULT_RIGHT_EYE_BLACK_1];

pub const FACE_DEFAULT_RIGHT_EYE_DEEP_BLACK_0: [Vec2; 32] = [
    Vec2::new(-1.16e+02, 7.97),
    Vec2::new(-1.16e+02, 5.32),
    Vec2::new(-1.14e+02, 3.59),
    Vec2::new(-1.12e+02, 3.11),
    Vec2::new(-1.11e+02, 3.18),
    Vec2::new(-1.1e+02, 3.82),
    Vec2::new(-1.1e+02, 3.96),
    Vec2::new(-1.12e+02, 4.51),
    Vec2::new(-1.13e+02, 5.43),
    Vec2::new(-1.13e+02, 6.69),
    Vec2::new(-1.12e+02, 8.4),
    Vec2::new(-1.11e+02, 10.4),
    Vec2::new(-1.09e+02, 11.5),
    Vec2::new(-1.08e+02, 11.5),
    Vec2::new(-1.07e+02, 11.1),
    Vec2::new(-1.06e+02, 9.9),
    Vec2::new(-1.06e+02, 9.05),
    Vec2::new(-1.05e+02, 9.69),
    Vec2::new(-1.04e+02, 11.5),
    Vec2::new(-1.04e+02, 13.3),
    Vec2::new(-1.05e+02, 15.1),
    Vec2::new(-1.05e+02, 15.9),
    Vec2::new(-1.07e+02, 17.4),
    Vec2::new(-1.08e+02, 17.8),
    Vec2::new(-1.09e+02, 17.9),
    Vec2::new(-1.1e+02, 17.7),
    Vec2::new(-1.11e+02, 17.1),
    Vec2::new(-1.13e+02, 16.1),
    Vec2::new(-1.14e+02, 14.8),
    Vec2::new(-1.15e+02, 13.3),
    Vec2::new(-1.16e+02, 11.7),
    Vec2::new(-1.16e+02, 9.88),
];

pub static FACE_DEFAULT_RIGHT_EYE_DEEP_BLACK: &[&[Vec2]] = &[&FACE_DEFAULT_RIGHT_EYE_DEEP_BLACK_0];

pub const FACE_DEFAULT_RIGHT_EYE_HIGHLIGHT_0: [Vec2; 16] = [
    Vec2::new(-1.13e+02, 6.69),
    Vec2::new(-1.13e+02, 5.44),
    Vec2::new(-1.12e+02, 4.53),
    Vec2::new(-1.11e+02, 4.07),
    Vec2::new(-1.1e+02, 4.04),
    Vec2::new(-1.09e+02, 4.42),
    Vec2::new(-1.07e+02, 5.95),
    Vec2::new(-1.07e+02, 7.31),
    Vec2::new(-1.06e+02, 8.97),
    Vec2::new(-1.07e+02, 10.6),
    Vec2::new(-1.07e+02, 11.1),
    Vec2::new(-1.08e+02, 11.5),
    Vec2::new(-1.1e+02, 11.2),
    Vec2::new(-1.11e+02, 10.7),
    Vec2::new(-1.12e+02, 9.69),
    Vec2::new(-1.12e+02, 8.86),
];

pub static FACE_DEFAULT_RIGHT_EYE_HIGHLIGHT: &[&[Vec2]] = &[&FACE_DEFAULT_RIGHT_EYE_HIGHLIGHT_0];

// Final user-edited appendage Bezier paths imported from fushi_ears_final.svg and fushi_tail_final.svg.

// Coordinates are converted back into the renderer-local SVG reference coordinate system.
// Ear paths are authored at their final shortened height; the renderer does not
// apply an extra y-axis height multiplier.

// These arrays preserve the SVG cubic curves directly instead of retracing them as Catmull-Rom point clouds.

pub const FINAL_REAR_EAR_OUTER_PATH: [PathCmd; 11] = [
    PathCmd::MoveTo(Vec2::new(-131.496, -51.772)),
    PathCmd::CubicTo(
        Vec2::new(-131.370, -57.210),
        Vec2::new(-130.358, -62.413),
        Vec2::new(-128.588, -67.023),
    ),
    PathCmd::CubicTo(
        Vec2::new(-127.197, -70.570),
        Vec2::new(-123.909, -74.471),
        Vec2::new(-119.863, -76.600),
    ),
    PathCmd::CubicTo(
        Vec2::new(-118.093, -76.363),
        Vec2::new(-116.576, -76.126),
        Vec2::new(-115.564, -75.772),
    ),
    PathCmd::CubicTo(
        Vec2::new(-112.403, -72.107),
        Vec2::new(-110.633, -65.132),
        Vec2::new(-110.253, -59.457),
    ),
    PathCmd::CubicTo(
        Vec2::new(-109.748, -53.782),
        Vec2::new(-109.621, -48.935),
        Vec2::new(-110.127, -46.216),
    ),
    PathCmd::CubicTo(
        Vec2::new(-110.506, -42.905),
        Vec2::new(-112.403, -39.004),
        Vec2::new(-115.438, -37.113),
    ),
    PathCmd::CubicTo(
        Vec2::new(-118.219, -35.575),
        Vec2::new(-122.266, -34.985),
        Vec2::new(-125.553, -36.876),
    ),
    PathCmd::CubicTo(
        Vec2::new(-128.082, -38.768),
        Vec2::new(-129.852, -42.077),
        Vec2::new(-130.737, -45.625),
    ),
    PathCmd::CubicTo(
        Vec2::new(-131.243, -47.516),
        Vec2::new(-131.749, -49.762),
        Vec2::new(-131.496, -51.772),
    ),
    PathCmd::Close,
];

pub const FINAL_REAR_EAR_INNER_PATH: [PathCmd; 10] = [
    PathCmd::MoveTo(Vec2::new(-126.628, -47.339)),
    PathCmd::CubicTo(
        Vec2::new(-126.438, -49.762),
        Vec2::new(-125.806, -52.245),
        Vec2::new(-125.047, -53.782),
    ),
    PathCmd::CubicTo(
        Vec2::new(-124.036, -56.265),
        Vec2::new(-122.645, -59.929),
        Vec2::new(-121.001, -61.585),
    ),
    PathCmd::CubicTo(
        Vec2::new(-120.116, -62.413),
        Vec2::new(-119.104, -63.004),
        Vec2::new(-117.966, -62.294),
    ),
    PathCmd::CubicTo(
        Vec2::new(-117.840, -57.565),
        Vec2::new(-117.840, -52.600),
        Vec2::new(-118.599, -45.506),
    ),
    PathCmd::CubicTo(
        Vec2::new(-119.357, -44.088),
        Vec2::new(-120.748, -42.551),
        Vec2::new(-121.507, -42.077),
    ),
    PathCmd::CubicTo(
        Vec2::new(-122.518, -40.422),
        Vec2::new(-123.530, -38.531),
        Vec2::new(-124.289, -37.230),
    ),
    PathCmd::CubicTo(
        Vec2::new(-124.921, -38.885),
        Vec2::new(-125.553, -41.250),
        Vec2::new(-126.059, -43.851),
    ),
    PathCmd::CubicTo(
        Vec2::new(-126.312, -45.033),
        Vec2::new(-126.565, -46.452),
        Vec2::new(-126.628, -47.339),
    ),
    PathCmd::Close,
];

pub const FINAL_REAR_EAR_SHADE_PATH: [PathCmd; 11] = [
    PathCmd::MoveTo(Vec2::new(-122.898, -46.216)),
    PathCmd::CubicTo(
        Vec2::new(-122.392, -47.456),
        Vec2::new(-120.875, -49.644),
        Vec2::new(-119.863, -52.481),
    ),
    PathCmd::CubicTo(
        Vec2::new(-118.852, -56.383),
        Vec2::new(-117.966, -60.166),
        Vec2::new(-117.966, -62.176),
    ),
    PathCmd::CubicTo(
        Vec2::new(-117.840, -65.013),
        Vec2::new(-118.346, -71.279),
        Vec2::new(-119.863, -76.600),
    ),
    PathCmd::CubicTo(
        Vec2::new(-118.346, -76.363),
        Vec2::new(-116.576, -76.126),
        Vec2::new(-115.564, -75.772),
    ),
    PathCmd::CubicTo(
        Vec2::new(-112.403, -72.107),
        Vec2::new(-110.633, -65.132),
        Vec2::new(-110.253, -59.457),
    ),
    PathCmd::CubicTo(
        Vec2::new(-109.748, -53.782),
        Vec2::new(-109.621, -48.817),
        Vec2::new(-110.127, -46.216),
    ),
    PathCmd::CubicTo(
        Vec2::new(-110.506, -42.905),
        Vec2::new(-112.403, -39.004),
        Vec2::new(-115.438, -37.113),
    ),
    PathCmd::CubicTo(
        Vec2::new(-118.093, -35.693),
        Vec2::new(-121.760, -35.339),
        Vec2::new(-124.542, -36.639),
    ),
    PathCmd::CubicTo(
        Vec2::new(-124.415, -39.949),
        Vec2::new(-123.360, -45.081),
        Vec2::new(-122.898, -46.216),
    ),
    PathCmd::Close,
];

pub const FINAL_FRONT_EAR_OUTER_PATH: [PathCmd; 14] = [
    PathCmd::MoveTo(Vec2::new(-73.003, -36.128)),
    PathCmd::CubicTo(
        Vec2::new(-71.814, -40.183),
        Vec2::new(-69.791, -44.675),
        Vec2::new(-68.021, -47.986),
    ),
    PathCmd::CubicTo(
        Vec2::new(-66.630, -51.533),
        Vec2::new(-65.239, -54.844),
        Vec2::new(-63.975, -57.326),
    ),
    PathCmd::CubicTo(
        Vec2::new(-61.952, -61.700),
        Vec2::new(-58.664, -67.848),
        Vec2::new(-53.986, -71.395),
    ),
    PathCmd::CubicTo(
        Vec2::new(-51.962, -72.932),
        Vec2::new(-48.801, -74.114),
        Vec2::new(-48.548, -73.523),
    ),
    PathCmd::CubicTo(
        Vec2::new(-45.514, -72.932),
        Vec2::new(-42.606, -69.267),
        Vec2::new(-40.962, -66.193),
    ),
    PathCmd::CubicTo(
        Vec2::new(-38.306, -61.346),
        Vec2::new(-36.663, -53.306),
        Vec2::new(-35.778, -47.868),
    ),
    PathCmd::CubicTo(
        Vec2::new(-35.525, -43.021),
        Vec2::new(-36.536, -37.583),
        Vec2::new(-38.812, -33.563),
    ),
    PathCmd::CubicTo(
        Vec2::new(-41.341, -28.834),
        Vec2::new(-45.767, -24.222),
        Vec2::new(-50.129, -21.149),
    ),
    PathCmd::CubicTo(
        Vec2::new(-51.583, -22.923),
        Vec2::new(-52.468, -23.631),
        Vec2::new(-52.342, -23.631),
    ),
    PathCmd::CubicTo(
        Vec2::new(-54.365, -23.868),
        Vec2::new(-58.664, -24.459),
        Vec2::new(-62.837, -25.286),
    ),
    PathCmd::CubicTo(
        Vec2::new(-66.251, -25.996),
        Vec2::new(-70.170, -29.425),
        Vec2::new(-71.561, -32.617),
    ),
    PathCmd::CubicTo(
        Vec2::new(-72.446, -33.917),
        Vec2::new(-72.826, -35.218),
        Vec2::new(-73.003, -36.128),
    ),
    PathCmd::Close,
];

pub const FINAL_FRONT_EAR_INNER_PATH: [PathCmd; 13] = [
    PathCmd::MoveTo(Vec2::new(-62.204, -48.577)),
    PathCmd::CubicTo(
        Vec2::new(-61.193, -50.587),
        Vec2::new(-59.549, -53.543),
        Vec2::new(-57.779, -56.499),
    ),
    PathCmd::CubicTo(
        Vec2::new(-56.135, -58.627),
        Vec2::new(-53.859, -60.636),
        Vec2::new(-52.089, -61.228),
    ),
    PathCmd::CubicTo(
        Vec2::new(-50.066, -61.582),
        Vec2::new(-48.548, -60.991),
        Vec2::new(-47.410, -59.808),
    ),
    PathCmd::CubicTo(
        Vec2::new(-45.008, -57.680),
        Vec2::new(-43.744, -53.543),
        Vec2::new(-42.606, -48.223),
    ),
    PathCmd::CubicTo(
        Vec2::new(-42.100, -44.912),
        Vec2::new(-42.226, -41.483),
        Vec2::new(-43.111, -38.883),
    ),
    PathCmd::CubicTo(
        Vec2::new(-43.617, -36.636),
        Vec2::new(-44.755, -34.272),
        Vec2::new(-45.261, -33.326),
    ),
    PathCmd::CubicTo(
        Vec2::new(-46.652, -30.725),
        Vec2::new(-48.485, -27.946),
        Vec2::new(-50.761, -26.174),
    ),
    PathCmd::CubicTo(
        Vec2::new(-51.773, -25.286),
        Vec2::new(-52.342, -24.695),
        Vec2::new(-53.417, -23.868),
    ),
    PathCmd::CubicTo(
        Vec2::new(-55.440, -24.105),
        Vec2::new(-57.273, -24.341),
        Vec2::new(-59.043, -24.695),
    ),
    PathCmd::CubicTo(
        Vec2::new(-58.790, -26.469),
        Vec2::new(-58.411, -28.597),
        Vec2::new(-58.285, -30.134),
    ),
    PathCmd::CubicTo(
        Vec2::new(-59.549, -36.163),
        Vec2::new(-60.814, -42.193),
        Vec2::new(-62.204, -48.577),
    ),
    PathCmd::Close,
];

pub const FINAL_FRONT_EAR_SHADE_PATH: [PathCmd; 15] = [
    PathCmd::MoveTo(Vec2::new(-73.774, -36.519)),
    PathCmd::CubicTo(
        Vec2::new(-72.067, -40.183),
        Vec2::new(-69.918, -44.794),
        Vec2::new(-68.021, -47.986),
    ),
    PathCmd::CubicTo(
        Vec2::new(-65.618, -53.424),
        Vec2::new(-63.342, -59.335),
        Vec2::new(-59.549, -65.484),
    ),
    PathCmd::CubicTo(
        Vec2::new(-57.779, -68.084),
        Vec2::new(-55.376, -70.331),
        Vec2::new(-53.986, -71.395),
    ),
    PathCmd::CubicTo(
        Vec2::new(-52.468, -72.340),
        Vec2::new(-50.192, -73.286),
        Vec2::new(-48.548, -73.523),
    ),
    PathCmd::CubicTo(
        Vec2::new(-50.951, -71.276),
        Vec2::new(-53.353, -68.676),
        Vec2::new(-55.756, -64.774),
    ),
    PathCmd::CubicTo(
        Vec2::new(-57.526, -61.819),
        Vec2::new(-58.917, -58.271),
        Vec2::new(-59.802, -53.661),
    ),
    PathCmd::CubicTo(
        Vec2::new(-59.296, -50.114),
        Vec2::new(-58.032, -45.149),
        Vec2::new(-57.400, -43.375),
    ),
    PathCmd::CubicTo(
        Vec2::new(-56.641, -39.947),
        Vec2::new(-55.882, -36.163),
        Vec2::new(-55.503, -33.326),
    ),
    PathCmd::CubicTo(
        Vec2::new(-55.124, -30.134),
        Vec2::new(-55.376, -27.414),
        Vec2::new(-55.756, -26.350),
    ),
    PathCmd::CubicTo(
        Vec2::new(-56.262, -25.523),
        Vec2::new(-56.894, -24.814),
        Vec2::new(-57.526, -24.341),
    ),
    PathCmd::CubicTo(
        Vec2::new(-59.423, -24.578),
        Vec2::new(-61.193, -24.932),
        Vec2::new(-62.837, -25.286),
    ),
    PathCmd::CubicTo(
        Vec2::new(-66.251, -26.115),
        Vec2::new(-70.044, -29.425),
        Vec2::new(-71.561, -32.617),
    ),
    PathCmd::CubicTo(
        Vec2::new(-72.573, -33.917),
        Vec2::new(-73.332, -35.454),
        Vec2::new(-73.774, -36.519),
    ),
    PathCmd::Close,
];

pub const FINAL_TAIL_REAR_PATH: [PathCmd; 25] = [
    PathCmd::MoveTo(Vec2::new(49.085, -75.092)),
    PathCmd::CubicTo(
        Vec2::new(53.677, -75.482),
        Vec2::new(63.462, -71.011),
        Vec2::new(64.3, -73.2),
    ),
    PathCmd::CubicTo(
        Vec2::new(66.656, -79.355),
        Vec2::new(64.639, -86.206),
        Vec2::new(65.322, -92.616),
    ),
    PathCmd::CubicTo(
        Vec2::new(65.404, -93.677),
        Vec2::new(65.965, -95.334),
        Vec2::new(68.111, -95.048),
    ),
    PathCmd::CubicTo(
        Vec2::new(74.039, -92.384),
        Vec2::new(82.989, -85.249),
        Vec2::new(85.401, -85.601),
    ),
    PathCmd::CubicTo(
        Vec2::new(92.282, -89.084),
        Vec2::new(86.6, -102.524),
        Vec2::new(95.2, -104.0),
    ),
    PathCmd::CubicTo(
        Vec2::new(102.656, -101.597),
        Vec2::new(101.961, -91.015),
        Vec2::new(106.001, -85.5),
    ),
    PathCmd::CubicTo(
        Vec2::new(107.277, -83.608),
        Vec2::new(110.117, -83.081),
        Vec2::new(112.001, -84.4),
    ),
    PathCmd::CubicTo(
        Vec2::new(119.541, -87.672),
        Vec2::new(125.963, -94.516),
        Vec2::new(134.06, -96.121),
    ),
    PathCmd::CubicTo(
        Vec2::new(134.528, -96.051),
        Vec2::new(137.0, -95.5),
        Vec2::new(137.0, -92.688),
    ),
    PathCmd::CubicTo(
        Vec2::new(136.161, -83.113),
        Vec2::new(132.632, -73.607),
        Vec2::new(133.0, -64.001),
    ),
    PathCmd::CubicTo(
        Vec2::new(133.156, -63.571),
        Vec2::new(133.518, -62.825),
        Vec2::new(135.0, -62.7),
    ),
    PathCmd::CubicTo(
        Vec2::new(140.925, -61.443),
        Vec2::new(148.636, -64.471),
        Vec2::new(153.801, -61.358),
    ),
    PathCmd::CubicTo(
        Vec2::new(154.555, -58.78),
        Vec2::new(152.75, -56.143),
        Vec2::new(151.0, -54.4),
    ),
    PathCmd::CubicTo(
        Vec2::new(149.83, -53.284),
        Vec2::new(139.014, -43.622),
        Vec2::new(137.999, -42.599),
    ),
    PathCmd::CubicTo(
        Vec2::new(136.858, -41.76),
        Vec2::new(138.337, -40.961),
        Vec2::new(138.999, -40.499),
    ),
    PathCmd::CubicTo(
        Vec2::new(140.875, -38.633),
        Vec2::new(146.621, -38.873),
        Vec2::new(146.001, -35.301),
    ),
    PathCmd::CubicTo(
        Vec2::new(137.924, -25.566),
        Vec2::new(123.445, -28.341),
        Vec2::new(112.4, -27.099),
    ),
    PathCmd::CubicTo(
        Vec2::new(105.587, -26.261),
        Vec2::new(109.921, -39.575),
        Vec2::new(102.3, -37.201),
    ),
    PathCmd::CubicTo(
        Vec2::new(97.552, -35.972),
        Vec2::new(94.184, -32.078),
        Vec2::new(90.1, -29.6),
    ),
    PathCmd::CubicTo(
        Vec2::new(89.273, -29.085),
        Vec2::new(89.572, -27.749),
        Vec2::new(88.2, -29.199),
    ),
    PathCmd::CubicTo(
        Vec2::new(79.401, -37.76),
        Vec2::new(74.072, -51.051),
        Vec2::new(61.96, -55.636),
    ),
    PathCmd::CubicTo(
        Vec2::new(61.727, -55.333),
        Vec2::new(60.958, -54.383),
        Vec2::new(60.1, -55.779),
    ),
    PathCmd::CubicTo(
        Vec2::new(59.242, -57.175),
        Vec2::new(47.01, -74.915),
        Vec2::new(49.085, -75.092),
    ),
    PathCmd::Close,
];

pub const FINAL_TAIL_LIGHT_PATH: [PathCmd; 18] = [
    PathCmd::MoveTo(Vec2::new(66.323, -50.415)),
    PathCmd::CubicTo(
        Vec2::new(66.57, -50.589),
        Vec2::new(74.86, -52.633),
        Vec2::new(75.099, -52.801),
    ),
    PathCmd::CubicTo(
        Vec2::new(76.667, -55.979),
        Vec2::new(70.626, -62.292),
        Vec2::new(75.099, -64.7),
    ),
    PathCmd::CubicTo(
        Vec2::new(79.188, -64.826),
        Vec2::new(82.039, -59.743),
        Vec2::new(86.1, -59.4),
    ),
    PathCmd::CubicTo(
        Vec2::new(89.512, -63.387),
        Vec2::new(86.617, -72.155),
        Vec2::new(92.401, -74.2),
    ),
    PathCmd::CubicTo(
        Vec2::new(97.944, -71.894),
        Vec2::new(96.652, -63.075),
        Vec2::new(101.0, -59.7),
    ),
    PathCmd::CubicTo(
        Vec2::new(106.23, -59.417),
        Vec2::new(109.487, -65.704),
        Vec2::new(114.0, -67.6),
    ),
    PathCmd::CubicTo(
        Vec2::new(115.171, -68.551),
        Vec2::new(116.962, -67.274),
        Vec2::new(116.0, -65.9),
    ),
    PathCmd::CubicTo(
        Vec2::new(114.739, -61.592),
        Vec2::new(112.62, -57.411),
        Vec2::new(111.672, -53.061),
    ),
    PathCmd::CubicTo(
        Vec2::new(115.046, -50.865),
        Vec2::new(120.119, -52.31),
        Vec2::new(124.0, -51.2),
    ),
    PathCmd::CubicTo(
        Vec2::new(125.104, -50.988),
        Vec2::new(125.92, -51.104),
        Vec2::new(125.0, -50.0),
    ),
    PathCmd::CubicTo(
        Vec2::new(124.08, -48.895),
        Vec2::new(114.92, -40.303),
        Vec2::new(114.0, -39.2),
    ),
    PathCmd::CubicTo(
        Vec2::new(115.417, -36.744),
        Vec2::new(119.123, -36.036),
        Vec2::new(121.0, -33.8),
    ),
    PathCmd::CubicTo(
        Vec2::new(117.006, -30.079),
        Vec2::new(110.032, -30.687),
        Vec2::new(106.235, -27.168),
    ),
    PathCmd::CubicTo(
        Vec2::new(106.64, -29.999),
        Vec2::new(104.399, -34.467),
        Vec2::new(101.0, -32.4),
    ),
    PathCmd::CubicTo(
        Vec2::new(96.027, -31.102),
        Vec2::new(92.642, -24.148),
        Vec2::new(87.0, -26.882),
    ),
    PathCmd::CubicTo(
        Vec2::new(79.97, -34.603),
        Vec2::new(73.177, -42.538),
        Vec2::new(66.323, -50.415),
    ),
    PathCmd::Close,
];

pub const FINAL_TAIL_FRONT_PATH: [PathCmd; 13] = [
    PathCmd::MoveTo(Vec2::new(58.812, -52.489)),
    PathCmd::CubicTo(
        Vec2::new(59.101, -54.054),
        Vec2::new(60.063, -55.738),
        Vec2::new(61.799, -55.901),
    ),
    PathCmd::CubicTo(
        Vec2::new(73.646, -51.151),
        Vec2::new(79.116, -38.386),
        Vec2::new(87.71, -29.815),
    ),
    PathCmd::CubicTo(
        Vec2::new(89.083, -28.364),
        Vec2::new(89.273, -29.085),
        Vec2::new(90.1, -29.6),
    ),
    PathCmd::CubicTo(
        Vec2::new(94.748, -31.958),
        Vec2::new(101.101, -40.479),
        Vec2::new(106.8, -36.0),
    ),
    PathCmd::CubicTo(
        Vec2::new(108.244, -33.129),
        Vec2::new(107.536, -26.378),
        Vec2::new(112.4, -27.099),
    ),
    PathCmd::CubicTo(
        Vec2::new(114.155, -27.201),
        Vec2::new(130.83, -28.644),
        Vec2::new(132.601, -28.7),
    ),
    PathCmd::CubicTo(
        Vec2::new(136.277, -29.33),
        Vec2::new(135.482, -24.786),
        Vec2::new(132.601, -24.5),
    ),
    PathCmd::CubicTo(
        Vec2::new(120.907, -21.849),
        Vec2::new(109.092, -18.236),
        Vec2::new(97.1, -17.899),
    ),
    PathCmd::CubicTo(
        Vec2::new(85.275, -20.585),
        Vec2::new(75.45, -28.57),
        Vec2::new(65.0, -34.301),
    ),
    PathCmd::CubicTo(
        Vec2::new(61.046, -38.489),
        Vec2::new(61.093, -45.325),
        Vec2::new(58.9, -50.601),
    ),
    PathCmd::CubicTo(
        Vec2::new(58.824, -51.176),
        Vec2::new(58.782, -52.311),
        Vec2::new(58.812, -52.489),
    ),
    PathCmd::Close,
];
