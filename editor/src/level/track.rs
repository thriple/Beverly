//! Change tracking: a part of the level that notes when it was borrowed to be changed, so the editor can tell at a
//! glance whether the level may have changed (and compare only then), and an undo step can hold what changed
//! (`delta.rs`) instead of a copy of the level.
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Every borrow for change takes the next number, so a number is never given twice.
static STAMP: AtomicU64 = AtomicU64::new(1);

/// A value that stamps itself when it is borrowed to change (reading through `&` costs nothing). A copy has the
/// stamp of what it copies, so equal stamps mean the same contents. Not saved; equality looks at the value only.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tracked<T> {
    v: T,
    #[serde(skip, default = "fresh")]
    stamp: u64,
}

fn fresh() -> u64 { STAMP.fetch_add(1, Ordering::Relaxed) }

impl<T: Default> Default for Tracked<T> {
    fn default() -> Self { Tracked { v: T::default(), stamp: fresh() } }
}

impl<T> Tracked<T> {
    pub fn stamp(&self) -> u64 { self.stamp }
}

impl<T> From<T> for Tracked<T> {
    fn from(v: T) -> Self { Tracked { v, stamp: fresh() } }
}

impl<T> std::ops::Deref for Tracked<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.v }
}

impl<T> std::ops::DerefMut for Tracked<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.stamp = fresh();
        &mut self.v
    }
}

impl<'a, T> IntoIterator for &'a Tracked<Vec<T>> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter { self.v.iter() }
}

impl<'a, T> IntoIterator for &'a mut Tracked<Vec<T>> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter { self.stamp = fresh(); self.v.iter_mut() }
}

impl<'a, K, V> IntoIterator for &'a Tracked<std::collections::BTreeMap<K, V>> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::collections::btree_map::Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter { self.v.iter() }
}

impl<A, T: FromIterator<A>> FromIterator<A> for Tracked<T> {
    fn from_iter<I: IntoIterator<Item = A>>(it: I) -> Self { T::from_iter(it).into() }
}

impl<T: PartialEq> PartialEq for Tracked<T> {
    fn eq(&self, other: &Self) -> bool { self.v == other.v }
}

/// The stamps of everything in a level that can change: two levels with equal stamps (one a copy of the other, or the
/// same level not borrowed for change in between) hold the same things.
pub type Stamps = [u64; 12];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_leaves_the_stamp_and_borrowing_to_change_gives_a_new_one() {
        let mut t: Tracked<Vec<u32>> = vec![1, 2].into();
        let s = t.stamp();
        assert_eq!(t.len(), 2);
        assert_eq!(t.stamp(), s);
        t.push(3);
        assert_ne!(t.stamp(), s);
        let c = t.clone();
        assert_eq!(c.stamp(), t.stamp());
        assert_eq!(c, t);
        t.push(4);
        assert_ne!(c, t);
    }
}
