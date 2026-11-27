//! Guards (docs/orderliness.md, A1 kind 4): the tiny numbers that keep a division, a normalising or a ray test from
//! meeting zero. They decide nothing about the level (that is the project's precision, `level::Precision`), so
//! they are named here once and left alone: never use one to judge whether two things are the same or touch.

/// A squared length, or a direction's part, too small to divide by.
pub const LEN2: f32 = 1e-12;
/// A determinant, or a cosine's distance from 1, that is nothing (exact geometry: triangles, curves, turns).
pub const FINE: f32 = 1e-9;
/// A length, squared length or denominator too small to divide by.
pub const ZERO: f32 = 1e-6;
/// How far in front of its start a ray must meet a triangle (so it doesn't meet the face it leaves).
pub const AHEAD: f32 = 1e-5;
/// A size or a length too small to take a direction from (a part squashed flat, two points on top of each other).
pub const SHORT: f32 = 1e-4;
/// A width, radius or denominator too small to divide by where the numbers are coarse (screen, feathering).
pub const SMALL: f32 = 1e-3;
/// A squared length too small to divide by in the paint's sums, as `paint.wesl` has it.
pub const PAINT_LEN2: f32 = 1e-8;
