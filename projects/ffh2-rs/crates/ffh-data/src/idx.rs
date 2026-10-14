//! Typed indices and index-keyed vectors.
//!
//! Every Civ4 "info class" (UnitInfo, TechInfo, ...) gets a newtype index
//! (`UnitType`, `TechType`, ...). C++'s `NO_X == -1` becomes `Option<XType>`.
//! Dense per-type arrays (C++ `int* m_piFoo` sized `GC.getNumXInfos()`) become
//! `IdxVec<XType, T>`, indexable directly by the typed index.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

pub trait Idx: Copy + Eq + Ord + std::hash::Hash + fmt::Debug + 'static {
    fn from_usize(i: usize) -> Self;
    fn idx(self) -> usize;
    /// i32 view, matching the C++ enum value (for logging / parity checks).
    fn i(self) -> i32 {
        self.idx() as i32
    }
}

/// Converts the C++ `int` convention (-1 = none) to an `Option<I>`.
pub fn opt_from_i32<I: Idx>(v: i32) -> Option<I> {
    if v < 0 {
        None
    } else {
        Some(I::from_usize(v as usize))
    }
}

/// Converts `Option<I>` back to the C++ `int` convention (-1 = none).
pub fn opt_to_i32<I: Idx>(v: Option<I>) -> i32 {
    v.map(|x| x.i()).unwrap_or(-1)
}

#[macro_export]
macro_rules! define_idx {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub u16);
        impl $crate::idx::Idx for $name {
            #[inline]
            fn from_usize(i: usize) -> Self {
                debug_assert!(i <= u16::MAX as usize);
                $name(i as u16)
            }
            #[inline]
            fn idx(self) -> usize {
                self.0 as usize
            }
        }
        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

/// A vector keyed by a typed index.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct IdxVec<I: Idx, T> {
    pub raw: Vec<T>,
    _m: PhantomData<fn(I) -> I>,
}

impl<I: Idx, T: fmt::Debug> fmt::Debug for IdxVec<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.raw.fmt(f)
    }
}

impl<I: Idx, T> Default for IdxVec<I, T> {
    fn default() -> Self {
        IdxVec { raw: Vec::new(), _m: PhantomData }
    }
}

impl<I: Idx, T> IdxVec<I, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_vec(raw: Vec<T>) -> Self {
        IdxVec { raw, _m: PhantomData }
    }
    pub fn from_elem(v: T, n: usize) -> Self
    where
        T: Clone,
    {
        IdxVec { raw: vec![v; n], _m: PhantomData }
    }
    pub fn from_fn(n: usize, mut f: impl FnMut(I) -> T) -> Self {
        IdxVec { raw: (0..n).map(|i| f(I::from_usize(i))).collect(), _m: PhantomData }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
    pub fn push(&mut self, v: T) -> I {
        let i = I::from_usize(self.raw.len());
        self.raw.push(v);
        i
    }
    #[inline]
    pub fn get(&self, i: I) -> Option<&T> {
        self.raw.get(i.idx())
    }
    #[inline]
    pub fn get_mut(&mut self, i: I) -> Option<&mut T> {
        self.raw.get_mut(i.idx())
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.raw.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.raw.iter_mut()
    }
    /// Iterate `(index, &value)` pairs.
    pub fn iter_enumerated(&self) -> impl Iterator<Item = (I, &T)> + '_ {
        self.raw.iter().enumerate().map(|(i, v)| (I::from_usize(i), v))
    }
    /// All valid indices.
    pub fn keys(&self) -> impl Iterator<Item = I> + 'static {
        (0..self.raw.len()).map(I::from_usize)
    }
    pub fn fill(&mut self, v: T)
    where
        T: Clone,
    {
        for x in self.raw.iter_mut() {
            *x = v.clone();
        }
    }
}

impl<I: Idx, T> Index<I> for IdxVec<I, T> {
    type Output = T;
    #[inline]
    fn index(&self, i: I) -> &T {
        &self.raw[i.idx()]
    }
}

impl<I: Idx, T> IndexMut<I> for IdxVec<I, T> {
    #[inline]
    fn index_mut(&mut self, i: I) -> &mut T {
        &mut self.raw[i.idx()]
    }
}

impl<'a, I: Idx, T> IntoIterator for &'a IdxVec<I, T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.raw.iter()
    }
}
