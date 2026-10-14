//! The binary encoding of saved games: [`Writer`] / [`Reader`] and the [`Persist`] trait with
//! its implementations for primitives and containers.
//!
//! Encoding (all little-endian):
//! - `u8`, `bool` (0/1): one byte;
//! - `u16`, `u32`, `u64`, `usize`: LEB128 varint; `i32`, `i64`: zigzag + LEB128 (most game
//!   integers are small or zero, so arrays of counters stay compact);
//! - `String`: byte length (varint) + UTF-8 bytes;
//! - `Option<T>`: tag byte (0 = None, 1 = Some) + value;
//! - `Vec<T>`, `IdxVec<I, T>`, `BTreeMap<K, V>` (in key order): length (varint) + elements;
//! - `[T; N]`: the `N` elements (no length: `N` is fixed by the code; the save header's
//!   layout fingerprint covers the enum-sized constants);
//! - tuples: the fields in order; structs: the fields in declaration order ([`persist_struct!`]);
//! - typed indices and entity ids: their inner integer.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use ffh_data::{Idx, IdxVec};

use super::LoadError;
use crate::city::{CacheBool, CacheI32};
use crate::slab::Slab;

/// Appends the encoding of values to a byte buffer.
#[derive(Default)]
pub struct Writer {
    pub(crate) buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Writer {
        Writer::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    #[inline]
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }

    /// LEB128 varint.
    #[inline]
    pub fn varint(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.buf.push((v as u8) | 0x80);
            v >>= 7;
        }
        self.buf.push(v as u8);
    }

    /// Zigzag + LEB128 varint.
    #[inline]
    pub fn varint_signed(&mut self, v: i64) {
        self.varint(((v << 1) ^ (v >> 63)) as u64);
    }

    pub fn len(&mut self, n: usize) {
        self.varint(n as u64);
    }
}

/// Reads values back from a byte slice; every read fails cleanly (no panic) on truncated or
/// malformed input.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, pos: 0 }
    }

    /// Bytes not read yet.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    #[inline]
    pub fn u8(&mut self) -> Result<u8, LoadError> {
        let b = *self.buf.get(self.pos).ok_or(LoadError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], LoadError> {
        if n > self.remaining() {
            return Err(LoadError::Truncated);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    #[inline]
    pub fn varint(&mut self) -> Result<u64, LoadError> {
        let mut v: u64 = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            if shift == 63 && b > 1 {
                return Err(LoadError::Corrupt("varint overflow".into()));
            }
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 63 {
                return Err(LoadError::Corrupt("varint too long".into()));
            }
        }
    }

    #[inline]
    pub fn varint_signed(&mut self) -> Result<i64, LoadError> {
        let u = self.varint()?;
        Ok(((u >> 1) as i64) ^ -((u & 1) as i64))
    }

    /// A length prefix. Every encoded element takes at least one byte, so a length larger than
    /// the rest of the input is corrupt (this also bounds allocations).
    pub fn len(&mut self) -> Result<usize, LoadError> {
        let n = self.varint()?;
        if n > self.remaining() as u64 {
            return Err(LoadError::Corrupt(format!("length {n} exceeds the remaining {} bytes", self.remaining())));
        }
        Ok(n as usize)
    }
}

/// A value that can be written to and read back from a saved game.
///
/// Struct implementations (see [`persist_struct!`]) destructure the struct exhaustively, so
/// adding a field to a saved struct fails to compile until the save code handles it.
pub trait Persist: Sized {
    fn write(&self, w: &mut Writer);
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError>;
}

/// `Persist` for a struct with named fields, written in the order listed (keep it the
/// declaration order). `write` destructures the struct with no `..` and `read` builds it with
/// a struct literal, so a field missing from the list is a compile error.
macro_rules! persist_struct {
    ($t:ident { $($f:ident),* $(,)? }) => {
        impl $crate::save::Persist for $t {
            fn write(&self, w: &mut $crate::save::Writer) {
                let $t { $($f),* } = self;
                $( $crate::save::Persist::write($f, w); )*
            }
            fn read(r: &mut $crate::save::Reader<'_>) -> Result<Self, $crate::save::LoadError> {
                // Struct literal fields are evaluated in the order written.
                Ok($t { $($f: $crate::save::Persist::read(r)?),* })
            }
        }
    };
}
pub(crate) use persist_struct;

/// `Persist` for single-field tuple structs (`struct UnitId(pub u32)`): the inner value.
macro_rules! persist_newtype {
    ($($t:ident),* $(,)?) => {$(
        impl $crate::save::Persist for $t {
            #[inline]
            fn write(&self, w: &mut $crate::save::Writer) {
                let $t(v) = self;
                $crate::save::Persist::write(v, w);
            }
            #[inline]
            fn read(r: &mut $crate::save::Reader<'_>) -> Result<Self, $crate::save::LoadError> {
                Ok($t($crate::save::Persist::read(r)?))
            }
        }
    )*};
}
pub(crate) use persist_newtype;

// ---- primitives ----

impl Persist for u8 {
    #[inline]
    fn write(&self, w: &mut Writer) {
        w.u8(*self);
    }
    #[inline]
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        r.u8()
    }
}

impl Persist for bool {
    #[inline]
    fn write(&self, w: &mut Writer) {
        w.u8(*self as u8);
    }
    #[inline]
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        match r.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(LoadError::Corrupt(format!("bool byte {b}"))),
        }
    }
}

macro_rules! persist_unsigned {
    ($($t:ty),*) => {$(
        impl Persist for $t {
            #[inline]
            fn write(&self, w: &mut Writer) {
                w.varint(*self as u64);
            }
            #[inline]
            fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
                let v = r.varint()?;
                <$t>::try_from(v).map_err(|_| LoadError::Corrupt(format!("{v} out of range for {}", stringify!($t))))
            }
        }
    )*};
}
persist_unsigned!(u16, u32, u64, usize);

macro_rules! persist_signed {
    ($($t:ty),*) => {$(
        impl Persist for $t {
            #[inline]
            fn write(&self, w: &mut Writer) {
                w.varint_signed(*self as i64);
            }
            #[inline]
            fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
                let v = r.varint_signed()?;
                <$t>::try_from(v).map_err(|_| LoadError::Corrupt(format!("{v} out of range for {}", stringify!($t))))
            }
        }
    )*};
}
persist_signed!(i32, i64);

impl Persist for String {
    fn write(&self, w: &mut Writer) {
        w.len(self.len());
        w.bytes(self.as_bytes());
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let n = r.len()?;
        let b = r.bytes(n)?;
        String::from_utf8(b.to_vec()).map_err(|_| LoadError::Corrupt("string is not UTF-8".into()))
    }
}

// ---- containers ----

impl<T: Persist> Persist for Option<T> {
    #[inline]
    fn write(&self, w: &mut Writer) {
        match self {
            None => w.u8(0),
            Some(v) => {
                w.u8(1);
                v.write(w);
            }
        }
    }
    #[inline]
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        match r.u8()? {
            0 => Ok(None),
            1 => Ok(Some(T::read(r)?)),
            b => Err(LoadError::Corrupt(format!("option tag {b}"))),
        }
    }
}

impl<T: Persist> Persist for Vec<T> {
    fn write(&self, w: &mut Writer) {
        w.len(self.len());
        for v in self {
            v.write(w);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let n = r.len()?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(T::read(r)?);
        }
        Ok(v)
    }
}

impl<T: Persist, const N: usize> Persist for [T; N] {
    fn write(&self, w: &mut Writer) {
        for v in self {
            v.write(w);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let mut v = Vec::with_capacity(N);
        for _ in 0..N {
            v.push(T::read(r)?);
        }
        v.try_into().map_err(|_| LoadError::Corrupt("array length".into()))
    }
}

impl<I: Idx, T: Persist> Persist for IdxVec<I, T> {
    fn write(&self, w: &mut Writer) {
        self.raw.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(IdxVec::from_vec(Vec::read(r)?))
    }
}

impl<K: Persist + Ord, V: Persist> Persist for BTreeMap<K, V> {
    fn write(&self, w: &mut Writer) {
        w.len(self.len());
        for (k, v) in self {
            k.write(w);
            v.write(w);
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let n = r.len()?;
        let mut m = BTreeMap::new();
        for _ in 0..n {
            let k = K::read(r)?;
            let v = V::read(r)?;
            if m.insert(k, v).is_some() {
                return Err(LoadError::Corrupt("duplicate map key".into()));
            }
        }
        Ok(m)
    }
}

impl<A: Persist, B: Persist> Persist for (A, B) {
    fn write(&self, w: &mut Writer) {
        self.0.write(w);
        self.1.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let a = A::read(r)?;
        let b = B::read(r)?;
        Ok((a, b))
    }
}

impl<A: Persist, B: Persist, C: Persist> Persist for (A, B, C) {
    fn write(&self, w: &mut Writer) {
        self.0.write(w);
        self.1.write(w);
        self.2.write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let a = A::read(r)?;
        let b = B::read(r)?;
        let c = C::read(r)?;
        Ok((a, b, c))
    }
}

/// A slab keeps its ids: every slot ever allocated is written (removed ones as `None`), so the
/// loaded slab has the same live ids and hands out the same next id.
impl<I: Idx, T: Persist> Persist for Slab<I, T> {
    fn write(&self, w: &mut Writer) {
        let slots: Vec<Option<&T>> = self.slots().collect();
        w.len(slots.len());
        for s in slots {
            match s {
                None => w.u8(0),
                Some(v) => {
                    w.u8(1);
                    v.write(w);
                }
            }
        }
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(Slab::from_slots(Vec::<Option<T>>::read(r)?))
    }
}

// ---- interior-mutable caches (C++ `mutable` members): saved like plain values ----

impl<T: Persist + Copy> Persist for Cell<T> {
    fn write(&self, w: &mut Writer) {
        self.get().write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(Cell::new(T::read(r)?))
    }
}

impl<T: Persist> Persist for RefCell<T> {
    fn write(&self, w: &mut Writer) {
        self.borrow().write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(RefCell::new(T::read(r)?))
    }
}

impl Persist for CacheI32 {
    fn write(&self, w: &mut Writer) {
        self.get().write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(CacheI32::new(i32::read(r)?))
    }
}

impl Persist for CacheBool {
    fn write(&self, w: &mut Writer) {
        self.get().write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(CacheBool::new(bool::read(r)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Persist + PartialEq + std::fmt::Debug>(v: T) {
        let mut w = Writer::new();
        v.write(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(T::read(&mut r).unwrap(), v);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn primitives_round_trip() {
        for v in [0i32, 1, -1, 63, -64, 64, i32::MAX, i32::MIN] {
            round_trip(v);
        }
        for v in [0u32, 127, 128, u32::MAX] {
            round_trip(v);
        }
        round_trip(u64::MAX);
        round_trip(i64::MIN);
        round_trip(true);
        round_trip(String::from("Kael's \u{2603}"));
        round_trip(Some(vec![(1u8, -5i32), (2, 7)]));
        round_trip([3i32, -4, 5]);
        round_trip(BTreeMap::from([(String::from("a"), 1i32), (String::from("b"), 2)]));
    }

    #[test]
    fn small_values_are_compact() {
        let mut w = Writer::new();
        0i32.write(&mut w);
        (-1i32).write(&mut w);
        63i32.write(&mut w);
        assert_eq!(w.into_bytes().len(), 3);
    }

    #[test]
    fn malformed_input_fails_cleanly() {
        assert!(matches!(i32::read(&mut Reader::new(&[0x80])), Err(LoadError::Truncated)));
        assert!(bool::read(&mut Reader::new(&[2])).is_err());
        assert!(Option::<u8>::read(&mut Reader::new(&[7])).is_err());
        // A length longer than the input.
        assert!(Vec::<u8>::read(&mut Reader::new(&[0xff, 0xff, 0x03])).is_err());
        assert!(u64::read(&mut Reader::new(&[0xff; 11])).is_err());
        assert!(u16::read(&mut Reader::new(&[0xff, 0xff, 0x7f])).is_err());
    }

    #[test]
    fn slab_keeps_ids() {
        use crate::ids::UnitId;
        let mut s: Slab<UnitId, i32> = Slab::new();
        let a = s.insert(10);
        let b = s.insert(20);
        let c = s.insert(30);
        s.remove(b);
        let mut w = Writer::new();
        s.write(&mut w);
        let bytes = w.into_bytes();
        let t = Slab::<UnitId, i32>::read(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!((t[a], t[c]), (10, 30));
        assert!(!t.contains(b));
        assert_eq!(t.next_id(), s.next_id());
    }
}
