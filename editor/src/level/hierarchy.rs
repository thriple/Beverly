//! Parts attached to parts. In the editor every part's position and turn are the world's, as always; `Part::parent`
//! says which part carries it. When a parent moves or turns, its children (and theirs) move with it: [`Level::follow`]
//! does that against the level as of the last undo step. In the file a child's position and turn are offsets from its
//! parent (`to_file`, `from_file`), so moving a parent changes one line, and a region or a ship can be moved as a whole.
use super::*;
use bevy::math::Quat;
use std::collections::{HashMap, HashSet};

/// A part's place: position and turn (its size is not inherited).
pub fn frame(p: &Part) -> (Vec3, Quat) { (Vec3::from(p.pos), Quat::from_array(p.rot).normalize()) }

fn set_frame(p: &mut Part, f: (Vec3, Quat)) {
    p.pos = f.0.to_array();
    p.rot = f.1.to_array();
}

/// `inner` as seen from `outer` (inner's world frame to a frame within outer's).
fn within(outer: (Vec3, Quat), inner: (Vec3, Quat)) -> (Vec3, Quat) {
    let inv = outer.1.inverse();
    (inv * (inner.0 - outer.0), inv * inner.1)
}

/// The inverse of `within`: a frame given within `outer`, in the world.
fn beyond(outer: (Vec3, Quat), inner: (Vec3, Quat)) -> (Vec3, Quat) { (outer.0 + outer.1 * inner.0, outer.1 * inner.1) }

impl Level {
    pub fn children(&self, id: u64) -> Vec<u64> { self.parts.iter().filter(|p| p.parent == Some(id)).map(|p| p.id).collect() }

    /// Everything below a part, nearest first.
    pub fn descendants(&self, id: u64) -> Vec<u64> {
        let (mut out, mut seen) = (vec![], HashSet::from([id]));
        let mut next = vec![id];
        while !next.is_empty() {
            let mut level = vec![];
            for p in &next { for c in self.children(*p) { if seen.insert(c) { level.push(c); } } }
            out.extend(&level);
            next = level;
        }
        out
    }

    /// How many parents are above a part.
    pub fn depth(&self, id: u64) -> usize {
        let (mut n, mut at) = (0, id);
        while let Some(p) = self.get(at).and_then(|p| p.parent) {
            n += 1;
            at = p;
            if n > self.parts.len() { break; }
        }
        n
    }

    /// Attach `id` to `parent` (or let go of its parent), keeping it where it is. Refused when the parent is the part
    /// itself or one of its descendants.
    pub fn set_parent(&mut self, id: u64, parent: Option<u64>) -> Result<(), String> {
        if self.get(id).is_none() { return Err("No such part".into()); }
        if let Some(p) = parent {
            let name = |l: &Level, id| l.get(id).map(|p| p.name.clone()).unwrap_or_default();
            if self.get(p).is_none() { return Err("No such parent".into()); }
            if p == id || self.descendants(id).contains(&p) { return Err(format!("{} is part of {} already: a part can't carry its own carrier", name(self, p), name(self, id))); }
        }
        if self.get(id).is_some_and(|q| q.parent != parent) { self.get_mut(id).expect("checked").parent = parent; }
        Ok(())
    }

    /// Take a part out of the level, with its wires; its children are attached to its parent in its place (staying
    /// where they are).
    pub fn remove_part(&mut self, id: u64) {
        let up = self.get(id).and_then(|p| p.parent);
        for p in &mut self.parts { if p.parent == Some(id) { p.parent = up; } }
        self.parts.retain(|p| p.id != id);
        self.wires.retain(|w| w.from != id && w.to != id);
    }

    /// Copies of a part and everything below it, each attached to the copy of its parent; the copy of `id` keeps its
    /// parent. Returns the copy's id.
    pub fn copy_tree(&mut self, id: u64, shift: [f32; 3]) -> Option<u64> {
        let all: Vec<u64> = std::iter::once(id).chain(self.descendants(id)).collect();
        let mut made: HashMap<u64, u64> = HashMap::new();
        for old in all {
            let mut c = self.get(old)?.clone();
            c.name = self.fresh_name(c.shape.label());
            if old == id { for k in 0..3 { c.pos[k] += shift[k]; } }
            else { c.parent = c.parent.and_then(|p| made.get(&p).copied()); c.pos = (Vec3::from(c.pos) + Vec3::from(shift)).to_array(); }
            made.insert(old, self.add(c));
        }
        made.get(&id).copied()
    }

    /// Make the descendants of every part that moved or turned since `before` (the level as of the last undo step) move
    /// and turn with it: each child goes to where it was, carried by its parent's change. Parts that are not in `before`
    /// are left alone. Does nothing (and borrows nothing for change) when there is nothing to do.
    pub fn follow(&mut self, before: &Level) {
        if !self.parts.iter().any(|p| p.parent.is_some()) { return; }
        let was: HashMap<u64, &Part> = before.parts.iter().map(|p| (p.id, p)).collect();
        let mut order: Vec<(usize, u64)> = self.parts.iter().filter(|p| p.parent.is_some()).map(|p| (self.depth(p.id), p.id)).collect();
        order.sort();
        for (_, id) in order {
            let Some(parent) = self.get(id).and_then(|p| p.parent).and_then(|pid| self.get(pid)).map(frame) else { continue };
            let (Some(old_child), Some(old_parent)) = (was.get(&id), self.get(id).and_then(|p| p.parent).and_then(|pid| was.get(&pid))) else { continue };
            let (old_child, old_parent) = (frame(old_child), frame(old_parent));
            if old_parent == parent { continue; }
            let to = beyond(parent, within(old_parent, old_child));
            let now = frame(self.get(id).expect("listed"));
            if to.0.distance_squared(now.0) > 1e-12 || to.1.dot(now.1).abs() < 1.0 - 1e-9 { set_frame(self.get_mut(id).expect("listed"), to); }
        }
    }

    /// The level as the file keeps it: a child's position and turn as offsets from its parent.
    pub fn to_file(&self) -> Level {
        let mut out = self.clone();
        for p in &mut out.parts {
            if let Some(par) = p.parent.and_then(|pid| self.get(pid)) { set_frame(p, within(frame(par), frame(p))); }
        }
        out
    }

    /// The level as the editor keeps it, from a file's offsets. A parent that is missing, or a loop of parents, is let go
    /// (the part stays where its offset puts it, from the world's origin).
    pub fn from_file(&mut self) {
        let ids: HashSet<u64> = self.parts.iter().map(|p| p.id).collect();
        for p in &mut self.parts { if p.parent.is_some_and(|q| !ids.contains(&q) || q == p.id) { p.parent = None; } }
        let mut order: Vec<(usize, u64)> = self.parts.iter().map(|p| (self.depth(p.id), p.id)).collect();
        // a loop never reaches the top: let its parts go
        let cap = self.parts.len();
        for (d, id) in &order { if *d >= cap { self.get_mut(*id).expect("listed").parent = None; } }
        order = self.parts.iter().map(|p| (self.depth(p.id), p.id)).collect();
        order.sort();
        for (_, id) in order {
            let Some(par) = self.get(id).and_then(|p| p.parent).and_then(|pid| self.get(pid)).map(frame) else { continue };
            let p = self.get_mut(id).expect("listed");
            let world = beyond(par, frame(p));
            set_frame(p, world);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(l: &mut Level, name: &str, at: [f32; 3]) -> u64 {
        l.add(Part::placed(Shape::Box, None, name.into(), at, [0.0, 0.0, 0.0, 1.0], [1.0; 3]))
    }

    fn near(a: [f32; 3], b: [f32; 3]) -> bool { Vec3::from(a).distance(Vec3::from(b)) < 1e-4 }

    #[test]
    fn a_part_cant_carry_itself_or_what_it_carries() {
        let mut l = Level::default();
        let (a, b, c) = (part(&mut l, "A", [0.0; 3]), part(&mut l, "B", [1.0, 0.0, 0.0]), part(&mut l, "C", [2.0, 0.0, 0.0]));
        l.set_parent(b, Some(a)).unwrap();
        l.set_parent(c, Some(b)).unwrap();
        assert!(l.set_parent(a, Some(c)).is_err() && l.set_parent(a, Some(a)).is_err() && l.set_parent(a, Some(99)).is_err());
        assert_eq!(l.descendants(a), vec![b, c]);
        assert_eq!((l.depth(a), l.depth(c)), (0, 2));
        l.set_parent(c, None).unwrap();
        assert!(l.set_parent(a, Some(c)).is_ok());
    }

    #[test]
    fn children_follow_a_parent_that_moves_and_turns_and_it_stays_one_undo_step() {
        let mut l = Level::default();
        let (a, b, c) = (part(&mut l, "A", [0.0; 3]), part(&mut l, "B", [2.0, 0.0, 0.0]), part(&mut l, "C", [2.0, 3.0, 0.0]));
        l.set_parent(b, Some(a)).unwrap();
        l.set_parent(c, Some(b)).unwrap();
        let before = l.clone();
        // slide A, then turn it a quarter about Y: B (2 m out along x) goes round to -z, C (above B) stays above it
        l.get_mut(a).unwrap().pos = [10.0, 0.0, 0.0];
        l.follow(&before);
        assert!(near(l.get(b).unwrap().pos, [12.0, 0.0, 0.0]) && near(l.get(c).unwrap().pos, [12.0, 3.0, 0.0]));
        l.get_mut(a).unwrap().rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array();
        l.follow(&before);
        assert!(near(l.get(b).unwrap().pos, [10.0, 0.0, -2.0]), "{:?}", l.get(b).unwrap().pos);
        assert!(near(l.get(c).unwrap().pos, [10.0, 3.0, -2.0]), "{:?}", l.get(c).unwrap().pos);
        // B turned with it
        assert!(Quat::from_array(l.get(b).unwrap().rot).dot(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)).abs() > 0.9999);
        // the same call again changes nothing (so it can run every frame), and nothing moved means nothing is borrowed
        let (s, now) = (l.parts.stamp(), l.clone());
        l.follow(&before);
        assert_eq!((l.parts.stamp(), &l), (s, &now));
        let mut quiet = before.clone();
        let s = quiet.parts.stamp();
        quiet.follow(&before);
        assert_eq!(quiet.parts.stamp(), s);
    }

    #[test]
    fn deleting_a_part_hands_its_children_to_its_parent_and_a_copy_takes_its_tree() {
        let mut l = Level::default();
        let (a, b, c) = (part(&mut l, "A", [0.0; 3]), part(&mut l, "B", [1.0, 0.0, 0.0]), part(&mut l, "C", [2.0, 0.0, 0.0]));
        l.set_parent(b, Some(a)).unwrap();
        l.set_parent(c, Some(b)).unwrap();
        let copy = l.copy_tree(a, [0.0, 0.0, 5.0]).unwrap();
        assert_eq!(l.parts.len(), 6);
        let kids = l.descendants(copy);
        assert_eq!(kids.len(), 2);
        assert!(kids.iter().all(|k| *k != b && *k != c) && near(l.get(kids[1]).unwrap().pos, [2.0, 0.0, 5.0]) && l.get(kids[0]).unwrap().parent == Some(copy));
        l.remove_part(b);
        assert_eq!(l.get(c).unwrap().parent, Some(a));
        l.remove_part(a);
        assert_eq!(l.get(c).unwrap().parent, None);
    }

    #[test]
    fn the_file_keeps_offsets_from_the_parent_and_loads_back_to_the_same_world() {
        let mut l = Level::default();
        let (a, b, c) = (part(&mut l, "A", [5.0, 1.0, -3.0]), part(&mut l, "B", [7.0, 1.0, -3.0]), part(&mut l, "C", [7.0, 4.0, -3.0]));
        l.get_mut(a).unwrap().rot = Quat::from_rotation_y(0.8).to_array();
        l.set_parent(b, Some(a)).unwrap();
        l.set_parent(c, Some(b)).unwrap();
        let file = l.to_file();
        // B is 2 m from A in A's own frame, and the file says so
        assert!(near(file.get(b).unwrap().pos, [(0.8f32).cos() * 2.0, 0.0, (0.8f32).sin() * 2.0]), "{:?}", file.get(b).unwrap().pos);
        assert_eq!(file.get(a).unwrap().pos, [5.0, 1.0, -3.0]);
        let mut back = file.clone();
        back.from_file();
        for id in [a, b, c] { assert!(near(back.get(id).unwrap().pos, l.get(id).unwrap().pos), "{id}"); }
        // a missing parent and a loop are let go
        let mut bad = file.clone();
        bad.get_mut(a).unwrap().parent = Some(c);
        bad.get_mut(c).unwrap().parent = Some(99);
        bad.from_file();
        assert!(bad.parts.iter().all(|p| p.parent != Some(99)));
        assert!(bad.parts.iter().filter(|p| p.parent.is_some()).count() < 3);
    }
}
