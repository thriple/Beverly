//! An undo step: what changed between two states of a level (the parts that changed, came or went, and the other
//! things that differ), not a copy of the level. Applied one way it takes the first state to the second, the other way
//! back.
use super::*;
use std::collections::{HashMap, HashSet};

/// A value before and after, when it changed.
type Pair<T> = Option<(T, T)>;

/// The value to go to: after when going forward, before when going back.
fn pick<T>(p: &Pair<T>, forward: bool) -> Option<&T> { p.as_ref().map(|(before, after)| if forward { after } else { before }) }

fn pair<T: Clone + PartialEq>(a: &T, b: &T) -> Pair<T> { (a != b).then(|| (a.clone(), b.clone())) }

/// A map's entries that differ: (key, before, after).
type Entries<T> = Vec<(String, Option<T>, Option<T>)>;

fn entries<T: Clone + PartialEq>(a: &std::collections::BTreeMap<String, T>, b: &std::collections::BTreeMap<String, T>) -> Entries<T> {
    let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    keys.into_iter().filter(|k| a.get(*k) != b.get(*k)).map(|k| (k.clone(), a.get(k).cloned(), b.get(k).cloned())).collect()
}

fn set<T: Clone>(map: &mut std::collections::BTreeMap<String, T>, from: &Entries<T>, forward: bool) {
    for (k, before, after) in from {
        match if forward { after } else { before } {
            Some(v) => { map.insert(k.clone(), v.clone()); }
            None => { map.remove(k); }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Delta {
    name: Pair<String>,
    next_id: Pair<u64>,
    /// Parts in both states whose contents differ: (id, before, after)
    changed: Vec<(u64, Part, Part)>,
    /// Parts only in the first state, and only in the second, with their place in that state
    removed: Vec<(usize, Part)>,
    added: Vec<(usize, Part)>,
    /// The order of the part ids before and after, when parts in both states changed places
    order: Pair<Vec<u64>>,
    wires: Pair<Vec<Wire>>,
    curves: Pair<Vec<crate::curve::Curve>>,
    buckets: Pair<Vec<super::Bucket>>,
    ranges: Pair<Ranges>,
    lighting: Pair<Lighting>,
    navigation: Pair<Navigation>,
    walker: Pair<WalkerSounds>,
    materials: Entries<Material>,
    sounds: Entries<Sound>,
}

impl Delta {
    /// What differs between two states of a level. Quick when little differs: parts that were not borrowed for change
    /// since a copy was made (equal stamps) are not looked at.
    pub fn between(a: &Level, b: &Level) -> Delta {
        let mut d = Delta {
            name: pair(&a.name, &b.name),
            next_id: pair(&a.next_id, &b.next_id),
            wires: if a.wires.stamp() == b.wires.stamp() { None } else { pair(&*a.wires, &*b.wires) },
            curves: if a.curves.stamp() == b.curves.stamp() { None } else { pair(&*a.curves, &*b.curves) },
            buckets: if a.buckets.stamp() == b.buckets.stamp() { None } else { pair(&*a.buckets, &*b.buckets) },
            ranges: if a.ranges.stamp() == b.ranges.stamp() { None } else { pair(&*a.ranges, &*b.ranges) },
            lighting: if a.lighting.stamp() == b.lighting.stamp() { None } else { pair(&*a.lighting, &*b.lighting) },
            navigation: if a.navigation.stamp() == b.navigation.stamp() { None } else { pair(&*a.navigation, &*b.navigation) },
            walker: if a.walker.stamp() == b.walker.stamp() { None } else { pair(&*a.walker, &*b.walker) },
            ..Delta::default()
        };
        if a.materials.stamp() != b.materials.stamp() { d.materials = entries(&a.materials, &b.materials); }
        if a.sounds.stamp() != b.sounds.stamp() { d.sounds = entries(&a.sounds, &b.sounds); }
        if a.parts.stamp() != b.parts.stamp() {
            let (ia, ib): (HashMap<u64, &Part>, HashMap<u64, &Part>) = (a.parts.iter().map(|p| (p.id, p)).collect(), b.parts.iter().map(|p| (p.id, p)).collect());
            d.removed = a.parts.iter().enumerate().filter(|(_, p)| !ib.contains_key(&p.id)).map(|(i, p)| (i, p.clone())).collect();
            d.added = b.parts.iter().enumerate().filter(|(_, p)| !ia.contains_key(&p.id)).map(|(i, p)| (i, p.clone())).collect();
            d.changed = b.parts.iter().filter_map(|p| ia.get(&p.id).filter(|q| ***q != *p).map(|q| (p.id, (*q).clone(), p.clone()))).collect();
            // parts in both states in the same order? (an add or a remove keeps the rest where it was)
            let shared = |l: &Level, other: &HashMap<u64, &Part>| l.parts.iter().map(|p| p.id).filter(|id| other.contains_key(id)).collect::<Vec<_>>();
            if shared(a, &ib) != shared(b, &ia) { d.order = Some((a.parts.iter().map(|p| p.id).collect(), b.parts.iter().map(|p| p.id).collect())); }
        }
        d
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.next_id.is_none() && self.changed.is_empty() && self.removed.is_empty() && self.added.is_empty() && self.order.is_none()
            && self.wires.is_none() && self.curves.is_none() && self.buckets.is_none() && self.ranges.is_none() && self.lighting.is_none() && self.navigation.is_none() && self.walker.is_none()
            && self.materials.is_empty() && self.sounds.is_empty()
    }

    /// The ids of the parts this step touched.
    pub fn parts_touched(&self) -> Vec<u64> {
        self.changed.iter().map(|c| c.0).chain(self.removed.iter().chain(&self.added).map(|(_, p)| p.id)).collect()
    }

    /// Take `level` from the first state to the second (`forward`) or back.
    pub fn apply(&self, level: &mut Level, forward: bool) {
        if let Some(v) = pick(&self.name, forward) { level.name = v.clone(); }
        if let Some(v) = pick(&self.next_id, forward) { level.next_id = *v; }
        if let Some(v) = pick(&self.wires, forward) { *level.wires = v.clone(); }
        if let Some(v) = pick(&self.curves, forward) { *level.curves = v.clone(); }
        if let Some(v) = pick(&self.buckets, forward) { *level.buckets = v.clone(); }
        if let Some(v) = pick(&self.ranges, forward) { *level.ranges = v.clone(); }
        if let Some(v) = pick(&self.lighting, forward) { *level.lighting = v.clone(); }
        if let Some(v) = pick(&self.navigation, forward) { *level.navigation = v.clone(); }
        if let Some(v) = pick(&self.walker, forward) { *level.walker = v.clone(); }
        if !self.materials.is_empty() { set(&mut level.materials, &self.materials, forward); }
        if !self.sounds.is_empty() { set(&mut level.sounds, &self.sounds, forward); }
        let (gone, come) = if forward { (&self.removed, &self.added) } else { (&self.added, &self.removed) };
        if !gone.is_empty() {
            let ids: HashSet<u64> = gone.iter().map(|(_, p)| p.id).collect();
            level.parts.retain(|p| !ids.contains(&p.id));
        }
        if !self.changed.is_empty() {
            let at: HashMap<u64, usize> = level.parts.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
            for (id, before, after) in &self.changed {
                if let Some(&i) = at.get(id) { level.parts[i] = if forward { after.clone() } else { before.clone() }; }
            }
        }
        // each in its place, lowest place first, so the places they had are the places they get
        for (i, p) in come { let at = (*i).min(level.parts.len()); level.parts.insert(at, p.clone()); }
        if let Some(order) = pick(&self.order, forward) {
            let place: HashMap<u64, usize> = order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
            level.parts.sort_by_key(|p| place.get(&p.id).copied().unwrap_or(usize::MAX));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(n: u64) -> Level {
        let mut l = Level { name: "main".into(), ..Default::default() };
        for i in 0..n { l.add(Part::placed(Shape::Box, None, format!("Box {i}"), [i as f32, 0.5, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])); }
        l
    }

    /// Every step from `a` to `b` and back gives the same level.
    fn round_trip(a: &Level, b: &Level) -> Delta {
        let d = Delta::between(a, b);
        let (mut x, mut y) = (a.clone(), b.clone());
        d.apply(&mut x, true);
        d.apply(&mut y, false);
        assert_eq!(&x, b, "forward");
        assert_eq!(&y, a, "back");
        d
    }

    #[test]
    fn nothing_changed_is_an_empty_step_and_only_what_changed_is_kept() {
        let a = level(50);
        assert!(Delta::between(&a, &a.clone()).is_empty());
        let mut b = a.clone();
        b.parts[7].pos[0] = 99.0;
        let d = round_trip(&a, &b);
        assert!(!d.is_empty() && d.parts_touched() == vec![b.parts[7].id] && d.order.is_none());
        // borrowed for change but not changed: still empty
        let mut c = a.clone();
        c.parts[3].name = c.parts[3].name.clone();
        assert!(Delta::between(&a, &c).is_empty());
    }

    #[test]
    fn parts_added_removed_changed_and_moved_in_the_list_undo_and_redo_exactly() {
        let a = level(20);
        // removed from the middle, one added at the end, one changed
        let mut b = a.clone();
        let gone = b.parts[5].id;
        b.parts.retain(|p| p.id != gone);
        b.add(Part::placed(Shape::Sphere, None, "New".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3]));
        b.parts[0].size = [3.0; 3];
        let d = round_trip(&a, &b);
        assert!(d.order.is_none() && d.removed.len() == 1 && d.added.len() == 1 && d.changed.len() == 1);
        // several removed and several inserted in the middle
        let mut c = a.clone();
        c.parts.retain(|p| p.id % 4 != 0);
        let p = Part { id: 900, ..c.parts[2].clone() };
        c.parts.insert(2, p);
        let q = Part { id: 901, ..c.parts[9].clone() };
        c.parts.insert(9, q);
        round_trip(&a, &c);
        // the list put in another order
        let mut e = a.clone();
        e.parts.reverse();
        let d = round_trip(&a, &e);
        assert!(d.order.is_some());
    }

    #[test]
    fn the_other_things_in_a_level_come_back_too() {
        let mut a = level(3);
        a.wires.push(Wire { from: 1, out: "pressed".into(), to: 2, input: "go".into() });
        a.materials.insert("assets/materials/a.bev".into(), Material::default());
        let mut b = a.clone();
        b.name = "other".into();
        b.next_id += 5;
        b.wires.clear();
        b.lighting.interior = true;
        b.materials.insert("assets/materials/b.bev".into(), Material::default());
        b.materials.remove("assets/materials/a.bev");
        b.walker = Default::default();
        let d = round_trip(&a, &b);
        assert!(d.parts_touched().is_empty());
    }
}
