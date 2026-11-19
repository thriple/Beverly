//! Pen curves: a curve is points with Bezier handles (what the pen tool draws, `editor/pen.rs`). It is kept as the
//! points and handles only; the line through them is worked out when needed, so it stays editable and exact.
use bevy::prelude::{Ray3d, Vec3};
use serde::{Deserialize, Serialize};

/// A point on a curve and the handles that bend the curve through it, as offsets from the point: `out` leads on
/// toward the next point, `into` back toward the one before. A zero handle makes a corner.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
pub struct PenPoint {
    pub at: [f32; 3],
    #[serde(default, skip_serializing_if = "is_zero")]
    pub into: [f32; 3],
    #[serde(default, skip_serializing_if = "is_zero")]
    pub out: [f32; 3],
}

fn is_zero(v: &[f32; 3]) -> bool { *v == [0.0; 3] }

impl PenPoint {
    pub fn corner(at: Vec3) -> PenPoint { PenPoint { at: at.to_array(), ..default() } }
    pub fn pos(&self) -> Vec3 { Vec3::from(self.at) }
    pub fn into_handle(&self) -> Vec3 { Vec3::from(self.into) }
    pub fn out_handle(&self) -> Vec3 { Vec3::from(self.out) }
}

fn default<T: Default>() -> T { T::default() }

/// A pen curve: open, or closed back on its first point.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Curve {
    pub id: u64,
    pub name: String,
    pub points: Vec<PenPoint>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
}

/// How many straight pieces a curved segment is drawn with.
pub const PIECES: usize = 16;

impl Curve {
    /// How many segments (a closed curve has one more than an open one).
    pub fn segments(&self) -> usize {
        let n = self.points.len();
        if n < 2 { 0 } else if self.closed { n } else { n - 1 }
    }

    /// Segment `i`'s Bezier: its two points and the two control points between them.
    pub fn bezier(&self, i: usize) -> [Vec3; 4] {
        let (a, b) = (&self.points[i], &self.points[(i + 1) % self.points.len()]);
        [a.pos(), a.pos() + a.out_handle(), b.pos() + b.into_handle(), b.pos()]
    }

    /// Whether segment `i` is a straight line (both of its handles are zero).
    pub fn straight(&self, i: usize) -> bool {
        let (a, b) = (&self.points[i], &self.points[(i + 1) % self.points.len()]);
        a.out_handle() == Vec3::ZERO && b.into_handle() == Vec3::ZERO
    }

    /// The line through the points, as straight pieces (a curved segment in [`PIECES`], a straight one in one).
    pub fn sample(&self) -> Vec<Vec3> {
        let mut v = vec![];
        for i in 0..self.segments() {
            let b = self.bezier(i);
            if i == 0 { v.push(b[0]); }
            let n = if self.straight(i) { 1 } else { PIECES };
            for k in 1..=n { v.push(at(&b, k as f32 / n as f32)); }
        }
        if v.is_empty() { v.extend(self.points.iter().map(PenPoint::pos)); }
        v
    }

    /// How long the line is (m).
    pub fn length(&self) -> f32 { self.sample().windows(2).map(|w| w[0].distance(w[1])).sum() }

    /// The place on the curve nearest to a ray: (segment, how far along it, how far the ray passes from it).
    pub fn nearest(&self, ray: Ray3d) -> Option<(usize, f32, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for i in 0..self.segments() {
            let b = self.bezier(i);
            let n = if self.straight(i) { 1 } else { PIECES };
            for k in 0..n {
                let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
                let (p, q) = (at(&b, t0), at(&b, t1));
                let (along, d) = ray_to_segment(ray, p, q);
                if best.is_none_or(|(_, _, bd)| d < bd) { best = Some((i, t0 + (t1 - t0) * along, d)); }
            }
        }
        best
    }

    /// Put a new point on segment `i`, `t` of the way along it, without changing the line: the segment's Bezier is
    /// cut in two there. Returns the new point's index.
    pub fn split(&mut self, i: usize, t: f32) -> usize {
        let [p0, p1, p2, p3] = self.bezier(i);
        let (q0, q1, q2) = (p0.lerp(p1, t), p1.lerp(p2, t), p2.lerp(p3, t));
        let (r0, r1) = (q0.lerp(q1, t), q1.lerp(q2, t));
        let s = r0.lerp(r1, t);
        let j = (i + 1) % self.points.len();
        self.points[i].out = (q0 - p0).to_array();
        self.points[j].into = (q2 - p3).to_array();
        let new = PenPoint { at: s.to_array(), into: (r0 - s).to_array(), out: (r1 - s).to_array() };
        // a segment from the last point back to the first goes on the end
        self.points.insert(i + 1, new);
        i + 1
    }

    /// Take point `i` out (the line closes over the gap).
    pub fn remove(&mut self, i: usize) {
        self.points.remove(i);
        if self.points.len() < 3 { self.closed = false; }
    }
}

/// The point `t` (0 to 1) along a cubic Bezier.
pub fn at(b: &[Vec3; 4], t: f32) -> Vec3 {
    let u = 1.0 - t;
    b[0] * (u * u * u) + b[1] * (3.0 * u * u * t) + b[2] * (3.0 * u * t * t) + b[3] * (t * t * t)
}

/// Where along the segment `p`–`q` (0 to 1) it passes nearest to the ray, and how near.
pub fn ray_to_segment(ray: Ray3d, p: Vec3, q: Vec3) -> (f32, f32) {
    let (d, e, r) = (*ray.direction, q - p, ray.origin - p);
    let (a, b, c) = (d.dot(d), d.dot(e), e.dot(e));
    let (c1, f) = (d.dot(r), e.dot(r));
    let den = a * c - b * b;
    let t = if den.abs() < 1e-9 { 0.0 } else { ((a * f - b * c1) / den).clamp(0.0, 1.0) };
    (t, ray_to_point(ray, p + e * t))
}

/// How far a point is from a ray (in front of it).
pub fn ray_to_point(ray: Ray3d, p: Vec3) -> f32 {
    let s = (p - ray.origin).dot(*ray.direction).max(0.0);
    (ray.origin + *ray.direction * s).distance(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Dir3;

    fn two() -> Curve {
        Curve { id: 1, name: "C".into(), points: vec![PenPoint { at: [0.0, 0.0, 0.0], into: [0.0; 3], out: [1.0, 0.0, 1.0] }, PenPoint { at: [4.0, 0.0, 0.0], into: [-1.0, 0.0, 1.0], out: [0.0; 3] }], closed: false }
    }

    #[test]
    fn corners_are_straight_lines_and_handles_bend_the_curve() {
        let mut c = two();
        c.points[0].out = [0.0; 3];
        c.points[1].into = [0.0; 3];
        assert_eq!(c.sample(), vec![Vec3::ZERO, Vec3::new(4.0, 0.0, 0.0)]);
        let c = two();
        let s = c.sample();
        assert_eq!(s.len(), PIECES + 1);
        assert!(s[PIECES / 2].z > 0.5, "it bulges toward the handles: {:?}", s[PIECES / 2]);
        assert!((s[PIECES / 2].x - 2.0).abs() < 1e-4);
        assert!(c.length() > 4.0);
    }

    #[test]
    fn cutting_a_segment_keeps_the_line() {
        let mut c = two();
        let before = c.sample();
        let i = c.split(0, 0.5);
        assert_eq!((i, c.points.len()), (1, 3));
        // the new point sits on the old line, and the line through the three points is the same one
        let mid = before[PIECES / 2];
        assert!(c.points[1].pos().distance(mid) < 1e-4);
        for p in c.sample() {
            let d = before.windows(2).map(|w| point_to_segment(p, w[0], w[1])).fold(f32::MAX, f32::min);
            assert!(d < 0.06, "{p:?} is {d} off the old line");
        }
    }

    fn point_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
        p.distance(a + ab * t)
    }

    #[test]
    fn a_closed_curve_has_one_more_segment_and_points_come_out() {
        let mut c = Curve { id: 1, name: "L".into(), points: vec![PenPoint::corner(Vec3::ZERO), PenPoint::corner(Vec3::X), PenPoint::corner(Vec3::Z)], closed: true };
        assert_eq!(c.segments(), 3);
        assert_eq!(*c.sample().last().unwrap(), Vec3::ZERO);
        c.remove(2);
        assert!(!c.closed, "two points can't be a loop");
        assert_eq!(c.segments(), 1);
        // splitting the segment that closes the loop puts the new point at the end
        let mut c = Curve { id: 1, name: "L".into(), points: vec![PenPoint::corner(Vec3::ZERO), PenPoint::corner(Vec3::X), PenPoint::corner(Vec3::Z)], closed: true };
        let i = c.split(2, 0.5);
        assert_eq!(i, 3);
        assert_eq!(c.points[3].pos(), Vec3::new(0.0, 0.0, 0.5));
    }

    #[test]
    fn the_nearest_place_on_a_curve_to_a_ray() {
        let c = two();
        // a ray from above straight down at the curve's middle
        let ray = Ray3d::new(Vec3::new(2.0, 5.0, 0.75), Dir3::NEG_Y);
        let (seg, t, d) = c.nearest(ray).unwrap();
        assert_eq!(seg, 0);
        assert!((t - 0.5).abs() < 0.06, "{t}");
        assert!(d < 0.1, "{d}");
        let far = Ray3d::new(Vec3::new(2.0, 5.0, 9.0), Dir3::NEG_Y);
        assert!(c.nearest(far).unwrap().2 > 5.0);
        assert!((ray_to_point(ray, Vec3::new(2.0, 0.0, 1.75)) - 1.0).abs() < 1e-4);
    }
}
