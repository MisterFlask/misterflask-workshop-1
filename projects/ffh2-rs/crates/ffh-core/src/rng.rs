//! Port of `CvRandom` (CvRandom.cpp): a 32-bit LCG, bit-exact with the C++.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rng {
    seed: u32,
}

const RANDOM_A: u32 = 1103515245;
const RANDOM_C: u32 = 12345;
const RANDOM_SHIFT: u32 = 16;
const MAX_UNSIGNED_SHORT: u32 = 0xFFFF;

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng { seed }
    }

    /// C++ `CvRandom::reset` / `init`.
    pub fn reset(&mut self, seed: u32) {
        self.seed = seed;
    }

    /// C++ `CvRandom::get(usNum)`: value in `0..num` (0 when `num == 0`).
    /// `log` mirrors the C++ log string argument (kept for parity tracing).
    #[inline]
    pub fn get(&mut self, num: u16, _log: &str) -> u16 {
        self.seed = RANDOM_A.wrapping_mul(self.seed).wrapping_add(RANDOM_C);
        ((((self.seed >> RANDOM_SHIFT) & MAX_UNSIGNED_SHORT) * num as u32) / (MAX_UNSIGNED_SHORT + 1)) as u16
    }

    /// Convenience: C++ code passes `int` arguments that are implicitly truncated to
    /// `unsigned short`; replicate that truncation and return `int`.
    #[inline]
    pub fn get_i(&mut self, num: i32, log: &str) -> i32 {
        self.get(num as u16, log) as i32
    }

    /// C++ `CvRandom::getFloat`.
    pub fn get_float(&mut self) -> f32 {
        (self.get(MAX_UNSIGNED_SHORT as u16, "getFloat") as f32) / (MAX_UNSIGNED_SHORT as f32)
    }

    /// C++ `CvRandom::reseed`.
    pub fn reseed(&mut self, seed: u32) {
        self.seed = seed;
    }

    /// C++ `CvRandom::getSeed`.
    pub fn seed(&self) -> u32 {
        self.seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_cpp_lcg() {
        // Reference values computed with the C++ formula (unsigned long = 32 bit on MSVC).
        let mut r = Rng::new(0);
        let v: Vec<u16> = (0..5).map(|_| r.get(100, "")).collect();
        let mut s: u32 = 0;
        let mut expect = Vec::new();
        for _ in 0..5 {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            expect.push(((((s >> 16) & 0xFFFF) * 100) / 65536) as u16);
        }
        assert_eq!(v, expect);
        assert_eq!(Rng::new(7).get(0, ""), 0);
    }
}
