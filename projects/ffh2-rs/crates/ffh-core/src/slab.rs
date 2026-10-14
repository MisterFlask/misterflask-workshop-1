//! Arena of game objects keyed by never-reused ids (replaces C++ `FFreeListTrashArray`).

use ffh_data::Idx;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

#[derive(Clone, Debug)]
pub struct Slab<I: Idx, T> {
    items: Vec<Option<Box<T>>>,
    live: usize,
    _m: PhantomData<fn(I) -> I>,
}

impl<I: Idx, T> Default for Slab<I, T> {
    fn default() -> Self {
        Slab { items: Vec::new(), live: 0, _m: PhantomData }
    }
}

impl<I: Idx, T> Slab<I, T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Id the next `insert` will return.
    pub fn next_id(&self) -> I {
        I::from_usize(self.items.len())
    }

    pub fn insert(&mut self, v: T) -> I {
        let id = I::from_usize(self.items.len());
        self.items.push(Some(Box::new(v)));
        self.live += 1;
        id
    }

    /// Inserts a value built from its own id.
    pub fn insert_with(&mut self, f: impl FnOnce(I) -> T) -> I {
        let id = self.next_id();
        let v = f(id);
        self.insert(v)
    }

    pub fn remove(&mut self, id: I) -> Option<T> {
        let slot = self.items.get_mut(id.idx())?;
        let v = slot.take()?;
        self.live -= 1;
        Some(*v)
    }

    #[inline]
    pub fn get(&self, id: I) -> Option<&T> {
        self.items.get(id.idx()).and_then(|s| s.as_deref())
    }

    #[inline]
    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.items.get_mut(id.idx()).and_then(|s| s.as_deref_mut())
    }

    #[inline]
    pub fn contains(&self, id: I) -> bool {
        self.get(id).is_some()
    }

    /// Number of live objects.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Live objects in id (= creation) order.
    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> + '_ {
        self.items.iter().enumerate().filter_map(|(i, s)| s.as_deref().map(|v| (I::from_usize(i), v)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (I, &mut T)> + '_ {
        self.items.iter_mut().enumerate().filter_map(|(i, s)| s.as_deref_mut().map(|v| (I::from_usize(i), v)))
    }

    /// Ids of live objects in id order (snapshot; safe to mutate the slab while iterating it).
    pub fn ids(&self) -> Vec<I> {
        self.iter().map(|(i, _)| i).collect()
    }

    /// Every slot ever allocated, in id order (`None` = removed). With [`Slab::from_slots`] this
    /// round-trips a slab exactly (same ids, same next id): used by saved games.
    pub fn slots(&self) -> impl Iterator<Item = Option<&T>> + '_ {
        self.items.iter().map(|s| s.as_deref())
    }

    /// The slab whose slots are `slots` (see [`Slab::slots`]).
    pub fn from_slots(slots: Vec<Option<T>>) -> Self {
        let live = slots.iter().filter(|s| s.is_some()).count();
        Slab { items: slots.into_iter().map(|s| s.map(Box::new)).collect(), live, _m: PhantomData }
    }
}

impl<I: Idx, T> Index<I> for Slab<I, T> {
    type Output = T;
    #[inline]
    #[track_caller]
    fn index(&self, id: I) -> &T {
        match self.get(id) {
            Some(v) => v,
            None => panic!("dangling id {:?}", id),
        }
    }
}

impl<I: Idx, T> IndexMut<I> for Slab<I, T> {
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, id: I) -> &mut T {
        match self.get_mut(id) {
            Some(v) => v,
            None => panic!("dangling id {:?}", id),
        }
    }
}
