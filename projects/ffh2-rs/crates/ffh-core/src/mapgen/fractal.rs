//! Port of `CvFractal` (CvFractal.h / CvFractal.cpp): the fractal height map used by map
//! scripts (Python `CyFractal`). Bit-exact with the C++: same integer arithmetic and the same
//! sequence of `CvRandom::get` draws (verified against the original code by
//! `tests/parity_fractal.rs`).

use crate::rng::Rng;
use crate::utils::range;

/// C++ `FLOAT_PRECISION` (fixed-point scale of the interpolation in `getHeight`).
const FLOAT_PRECISION: i32 = 1000;

// C++ `CvFractal::FracVals` (also available as `Fractal::FRAC_*`).
pub const FRAC_WRAP_X: i32 = 0x0001;
pub const FRAC_WRAP_Y: i32 = 0x0002;
/// Returns raw data as a percent.
pub const FRAC_PERCENT: i32 = 0x0004;
/// Sets polar regions to zero.
pub const FRAC_POLAR: i32 = 0x0008;
/// Draws rift in center of world, too.
pub const FRAC_CENTER_RIFT: i32 = 0x0010;
/// Inverts the heights.
pub const FRAC_INVERT_HEIGHTS: i32 = 0x0020;
pub const DEFAULT_FRAC_X_EXP: i32 = 7;
pub const DEFAULT_FRAC_Y_EXP: i32 = 6;

/// C++ `CvFractal`.
#[derive(Clone, Debug)]
pub struct Fractal {
    xs: i32,
    ys: i32,
    flags: i32,
    frac_x: i32,
    frac_y: i32,
    frac_x_exp: i32,
    frac_y_exp: i32,
    x_inc: i32,
    y_inc: i32,
    /// C++ `m_aaiFrac[m_iFracX + 1][m_iFracY + 1]`, x-major: index `x * (frac_y + 1) + y`.
    frac: Vec<i32>,
}

impl Default for Fractal {
    fn default() -> Self {
        Fractal::new()
    }
}

impl Fractal {
    pub const FRAC_WRAP_X: i32 = FRAC_WRAP_X;
    pub const FRAC_WRAP_Y: i32 = FRAC_WRAP_Y;
    pub const FRAC_PERCENT: i32 = FRAC_PERCENT;
    pub const FRAC_POLAR: i32 = FRAC_POLAR;
    pub const FRAC_CENTER_RIFT: i32 = FRAC_CENTER_RIFT;
    pub const FRAC_INVERT_HEIGHTS: i32 = FRAC_INVERT_HEIGHTS;
    pub const DEFAULT_FRAC_X_EXP: i32 = DEFAULT_FRAC_X_EXP;
    pub const DEFAULT_FRAC_Y_EXP: i32 = DEFAULT_FRAC_Y_EXP;

    /// A new fractal initialized with [`Fractal::frac_init`] (Python `CyFractal()` +
    /// `fracInit(...)`).
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        new_xs: i32,
        new_ys: i32,
        grain: i32,
        rng: &mut Rng,
        flags: i32,
        rifts: Option<&Fractal>,
        frac_x_exp: i32,
        frac_y_exp: i32,
    ) -> Fractal {
        let mut f = Fractal::new();
        f.frac_init(new_xs, new_ys, grain, rng, flags, rifts, frac_x_exp, frac_y_exp);
        f
    }

    /// C++ `CvFractal::CvFractal` (+ `reset`).
    pub fn new() -> Fractal {
        Fractal {
            xs: -1,
            ys: -1,
            flags: 0,
            frac_x: -1,
            frac_y: -1,
            frac_x_exp: -1,
            frac_y_exp: -1,
            x_inc: 0,
            y_inc: 0,
            frac: Vec::new(),
        }
    }

    /// C++ `CvFractal::reset` (frees the grid; `m_iXInc`/`m_iYInc` are left untouched as in C++).
    pub fn reset(&mut self) {
        self.frac = Vec::new();
        self.frac_x_exp = -1;
        self.frac_y_exp = -1;
        self.xs = -1;
        self.ys = -1;
        self.flags = 0;
        self.frac_x = -1;
        self.frac_y = -1;
    }

    /// C++ `m_aaiFrac[x][y]`.
    #[inline]
    fn at(&self, x: i32, y: i32) -> i32 {
        self.frac[(x * (self.frac_y + 1) + y) as usize]
    }

    #[inline]
    fn at_mut(&mut self, x: i32, y: i32) -> &mut i32 {
        let i = (x * (self.frac_y + 1) + y) as usize;
        &mut self.frac[i]
    }

    /// C++ `fracInit(iNewXs, iNewYs, iGrain, random, iFlags, pRifts, iFracXExp, iFracYExp)`.
    ///
    /// Smoothness varies with 2^grain: totally random is `grain = min(x_exp, y_exp)`, as smooth
    /// as possible is `grain = 0`. Exponents < 0 select the defaults (7, 6).
    #[allow(clippy::too_many_arguments)]
    pub fn frac_init(
        &mut self,
        new_xs: i32,
        new_ys: i32,
        grain: i32,
        rng: &mut Rng,
        flags: i32,
        rifts: Option<&Fractal>,
        frac_x_exp: i32,
        frac_y_exp: i32,
    ) {
        self.frac_init_internal(new_xs, new_ys, grain, rng, None, flags, rifts, frac_x_exp, frac_y_exp);
    }

    /// C++ `fracInitHinted(..., pbyHints, iHintsLength, iFlags, pRifts, ...)`: the initial
    /// random grid is replaced by `hints`, a row-major `width * height` byte grid with
    /// `width = 2^(x_exp - smooth) + (wrap_x ? 0 : 1)` and likewise for the height
    /// (`smooth = range(min_exp - grain, 0, min_exp)`). `FRAC_POLAR` is ignored.
    #[allow(clippy::too_many_arguments)]
    pub fn frac_init_hinted(
        &mut self,
        new_xs: i32,
        new_ys: i32,
        grain: i32,
        rng: &mut Rng,
        hints: &[u8],
        flags: i32,
        rifts: Option<&Fractal>,
        frac_x_exp: i32,
        frac_y_exp: i32,
    ) {
        let flags_non_polar = flags & !Self::FRAC_POLAR;
        self.frac_init_internal(new_xs, new_ys, grain, rng, Some(hints), flags_non_polar, rifts, frac_x_exp, frac_y_exp);
    }

    /// C++ `fracInitInternal`.
    #[allow(clippy::too_many_arguments)]
    fn frac_init_internal(
        &mut self,
        new_xs: i32,
        new_ys: i32,
        grain: i32,
        rng: &mut Rng,
        hints: Option<&[u8]>,
        flags: i32,
        rifts: Option<&Fractal>,
        mut frac_x_exp: i32,
        mut frac_y_exp: i32,
    ) {
        self.reset();

        if frac_x_exp < 0 {
            frac_x_exp = Self::DEFAULT_FRAC_X_EXP;
        }
        if frac_y_exp < 0 {
            frac_y_exp = Self::DEFAULT_FRAC_Y_EXP;
        }

        self.frac_x_exp = frac_x_exp;
        self.frac_y_exp = frac_y_exp;
        self.frac_x = 1 << frac_x_exp;
        self.frac_y = 1 << frac_y_exp;

        // Init m_aaiFrac to all zeroes.
        self.frac = vec![0; ((self.frac_x + 1) * (self.frac_y + 1)) as usize];

        self.xs = new_xs;
        self.ys = new_ys;
        self.flags = flags;
        self.x_inc = (self.frac_x * FLOAT_PRECISION) / self.xs;
        self.y_inc = (self.frac_y * FLOAT_PRECISION) / self.ys;

        let frac_x = self.frac_x;
        let frac_y = self.frac_y;
        let wrap_x = (flags & Self::FRAC_WRAP_X) != 0;
        let wrap_y = (flags & Self::FRAC_WRAP_Y) != 0;

        let min_exp = frac_x_exp.min(frac_y_exp);
        let smooth = range(min_exp - grain, 0, min_exp);

        let hints_width = (1 << (frac_x_exp - smooth)) + if wrap_x { 0 } else { 1 };
        let hints_height = (1 << (frac_y_exp - smooth)) + if wrap_y { 0 } else { 1 };
        if let Some(h) = hints {
            debug_assert_eq!(h.len() as i32, hints_width * hints_height, "pbyHints is the wrong size!");
        }

        let mut pass = smooth;
        while pass >= 0 {
            // This screens out already marked spots in m_aaiFrac[][].
            let mut screen: i32 = 0;
            for i in 0..=pass {
                screen |= 1i32.wrapping_shl(i as u32);
            }

            if wrap_y {
                for x in 0..frac_x + 1 {
                    *self.at_mut(x, frac_y) = self.at(x, 0);
                }
            } else if (flags & Self::FRAC_POLAR) != 0 {
                for x in 0..frac_x + 1 {
                    *self.at_mut(x, 0) = 0;
                    *self.at_mut(x, frac_y) = 0;
                }
            }

            if wrap_x {
                for y in 0..frac_y + 1 {
                    *self.at_mut(frac_x, y) = self.at(0, y);
                }
            } else if (flags & Self::FRAC_POLAR) != 0 {
                for y in 0..frac_y + 1 {
                    *self.at_mut(0, y) = 0;
                    *self.at_mut(frac_x, y) = 0;
                }
            }

            if (flags & Self::FRAC_CENTER_RIFT) != 0 {
                if wrap_y {
                    for x in 0..frac_x + 1 {
                        for y in 0..frac_y / 6 {
                            let div = ((frac_y / 12) - y).abs() + 1;
                            *self.at_mut(x, y) /= div;
                            *self.at_mut(x, (frac_y / 2) + y) /= div;
                        }
                    }
                }

                if wrap_x {
                    for y in 0..frac_y + 1 {
                        for x in 0..frac_x / 6 {
                            let div = ((frac_x / 12) - x).abs() + 1;
                            *self.at_mut(x, y) /= div;
                            *self.at_mut((frac_x / 2) + x, y) /= div;
                        }
                    }
                }
            }

            // Random amplitude of this pass (C++ `1 << (8 - iSmooth + iPass)` truncated to the
            // `unsigned short` argument of CvRandom::get; shifts wrap like x86 `shl`).
            let amplitude = 1i32.wrapping_shl((8 - smooth + pass) as u32);
            let half_amplitude = 1i32.wrapping_shl((7 - smooth + pass) as u32);

            let x_end = (frac_x >> pass) + if wrap_x { 0 } else { 1 };
            let y_end = (frac_y >> pass) + if wrap_y { 0 } else { 1 };
            for x in 0..x_end {
                for y in 0..y_end {
                    let (px, py) = (x << pass, y << pass);
                    if pass == smooth {
                        // If this is the first pass, set the initial random spots.
                        match hints {
                            None => *self.at_mut(px, py) = rng.get(256, "Fractal Gen") as i32,
                            Some(h) => {
                                let xx = x % hints_width; // wrap
                                let yy = y % hints_height; // wrap
                                let hints_i = yy * hints_width + xx;
                                *self.at_mut(px, py) = h[hints_i as usize] as i32;
                            }
                        }
                    } else {
                        // Interpolate.
                        let mut sum: i32 = 0;
                        let log;
                        if (px & screen) != 0 {
                            if (py & screen) != 0 {
                                // (center)
                                sum += self.at((x - 1) << pass, (y - 1) << pass);
                                sum += self.at((x + 1) << pass, (y - 1) << pass);
                                sum += self.at((x - 1) << pass, (y + 1) << pass);
                                sum += self.at((x + 1) << pass, (y + 1) << pass);
                                sum >>= 2;
                                log = "Fractal Gen 2";
                            } else {
                                // (horizontal)
                                sum += self.at((x - 1) << pass, py);
                                sum += self.at((x + 1) << pass, py);
                                sum >>= 1;
                                log = "Fractal Gen 3";
                            }
                        } else if (py & screen) != 0 {
                            // (vertical)
                            sum += self.at(px, (y - 1) << pass);
                            sum += self.at(px, (y + 1) << pass);
                            sum >>= 1;
                            log = "Fractal Gen 4";
                        } else {
                            // (corner) This was already set in an earlier pass.
                            continue;
                        }
                        sum = sum.wrapping_add(rng.get(amplitude as u16, log) as i32);
                        sum = sum.wrapping_sub(half_amplitude);
                        sum = range(sum, 0, 255);
                        *self.at_mut(px, py) = sum;
                    }
                }
            }
            pass -= 1;
        }

        if let Some(r) = rifts {
            self.tectonic_action(r); // Assumes FRAC_WRAP_X is on.
        }

        if (flags & Self::FRAC_INVERT_HEIGHTS) != 0 {
            // C++ quirk kept: the last row and column (x == frac_x, y == frac_y) are not inverted.
            for x in 0..frac_x {
                for y in 0..frac_y {
                    let v = self.at(x, y);
                    *self.at_mut(x, y) = 255 - v;
                }
            }
        }
    }

    /// C++ `getHeight(iX, iY)`: bilinear interpolation of the grid at map plot (x, y),
    /// 0..=255 (0..=99 with `FRAC_PERCENT`).
    pub fn height(&self, x: i32, y: i32) -> i32 {
        debug_assert!(0 <= x && x < self.xs, "iX out of range");
        debug_assert!(0 <= y && y < self.ys, "iY out of range");
        let mut low_x = (self.x_inc * x) / FLOAT_PRECISION;
        if low_x > self.frac_x - 1 {
            low_x = self.frac_x - 1; // clamp so that low_x + 1 doesn't overrun the array
        }
        let mut low_y = (self.y_inc * y) / FLOAT_PRECISION;
        if low_y > self.frac_y - 1 {
            low_y = self.frac_y - 1; // clamp so that low_y + 1 doesn't overrun the array
        }
        let err_x = (self.x_inc * x) - (low_x * FLOAT_PRECISION);
        let err_y = (self.y_inc * y) - (low_y * FLOAT_PRECISION);

        let mut sum = 0;
        sum += (FLOAT_PRECISION - err_x) * (FLOAT_PRECISION - err_y) * self.at(low_x, low_y);
        sum += err_x * (FLOAT_PRECISION - err_y) * self.at(low_x + 1, low_y);
        sum += (FLOAT_PRECISION - err_x) * err_y * self.at(low_x, low_y + 1);
        sum += err_x * err_y * self.at(low_x + 1, low_y + 1);

        sum /= FLOAT_PRECISION * FLOAT_PRECISION;

        let height = range(sum, 0, 255);

        if (self.flags & Self::FRAC_PERCENT) != 0 {
            (height * 100) >> 8
        } else {
            height
        }
    }

    /// C++ `getHeightFromPercent(iPercent)`: binary search for the height below which
    /// `percent`% of the grid lies (grid row/column `frac_x`/`frac_y` excluded).
    pub fn height_from_percent(&self, percent: i32) -> i32 {
        let mut lower_bound = 0;
        let mut upper_bound = 255;

        let percent = range(percent, 0, 100);
        let mut estimate = 255 * percent / 100;

        while estimate != lower_bound {
            let mut sum = 0;

            for x in 0..self.frac_x {
                for y in 0..self.frac_y {
                    if self.at(x, y) < estimate {
                        sum += 1;
                    }
                }
            }
            if (100 * sum / self.frac_x / self.frac_y) > percent {
                upper_bound = estimate;
                estimate = (upper_bound + lower_bound) / 2;
            } else {
                lower_bound = estimate;
                estimate = (upper_bound + lower_bound) / 2;
            }
        }

        estimate
    }

    /// C++ `tectonicAction(pRifts)`: carves a rift along the right quarter line of `rifts`.
    /// Assumes `FRAC_WRAP_X`.
    fn tectonic_action(&mut self, rifts: &Fractal) {
        let rift2x = (self.frac_x / 4) * 3; // C++ also computes an unused iRift1x = m_iFracX / 4.
        let width = 16;

        for y in 0..self.frac_y + 1 {
            for x in 0..width {
                // Rift along edge of map.
                let deep = 0;
                let offset = (((rifts.at(rift2x, y) - 128) * self.frac_x) / 128) / 8;
                let rx = self.yield_x(offset + x);
                let lx = self.yield_x(offset - x);
                let v = self.at(rx, y);
                *self.at_mut(rx, y) = ((v * x) + deep * (width - x)) / width;
                let v = self.at(lx, y);
                *self.at_mut(lx, y) = ((v * x) + deep * (width - x)) / width;
            }
        }

        for y in 0..self.frac_y + 1 {
            *self.at_mut(self.frac_x, y) = self.at(0, y);
        }
    }

    /// C++ `yieldX(iBadX)`: wraps an x index once. Assumes `FRAC_WRAP_X`.
    fn yield_x(&self, bad_x: i32) -> i32 {
        if bad_x < 0 {
            return bad_x + self.frac_x;
        }
        if bad_x >= self.frac_x {
            return bad_x - self.frac_x;
        }
        bad_x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heights(f: &Fractal, xs: i32, ys: i32) -> Vec<i32> {
        (0..ys).flat_map(|y| (0..xs).map(move |x| (x, y))).map(|(x, y)| f.height(x, y)).collect()
    }

    #[test]
    fn deterministic_for_a_seed() {
        let make = |seed| {
            let mut r = Rng::new(seed);
            let mut f = Fractal::new();
            f.frac_init(60, 40, 3, &mut r, Fractal::FRAC_WRAP_X | Fractal::FRAC_POLAR, None, 7, 6);
            (heights(&f, 60, 40), r.seed())
        };
        let (a, sa) = make(4242);
        let (b, sb) = make(4242);
        assert_eq!(a, b);
        assert_eq!(sa, sb);
        let (c, _) = make(4243);
        assert_ne!(a, c);
        assert!(a.iter().all(|&h| (0..=255).contains(&h)));
    }

    #[test]
    fn percent_flag_and_height_from_percent() {
        let mut r = Rng::new(7);
        let mut f = Fractal::new();
        f.frac_init(32, 20, 2, &mut r, Fractal::FRAC_WRAP_X | Fractal::FRAC_PERCENT, None, 6, 5);
        assert!(heights(&f, 32, 20).iter().all(|&h| (0..100).contains(&h)));
        assert_eq!(f.height_from_percent(0), 0);
        assert_eq!(f.height_from_percent(100), 255);
        let mut last = 0;
        for p in (0..=100).step_by(10) {
            let h = f.height_from_percent(p);
            assert!(h >= last);
            last = h;
        }
    }

    #[test]
    fn hinted_grid_with_full_grain_uses_no_randomness() {
        // grain = min_exp -> smooth = 0: the hints are the grid, no RNG draws.
        let (xe, ye) = (4, 3);
        let w = (1 << xe) + 1; // no wrap: one extra column/row
        let h = (1 << ye) + 1;
        let hints: Vec<u8> = (0..w * h).map(|i| ((i * 37 + 11) % 256) as u8).collect();
        let mut r = Rng::new(99);
        let mut f = Fractal::new();
        f.frac_init_hinted(16, 8, 3, &mut r, &hints, 0, None, xe, ye);
        assert_eq!(r.seed(), 99);
        // xs == frac_x: x_inc = 1000, so plots sample grid points exactly.
        for y in 0..8 {
            for x in 0..16 {
                assert_eq!(f.height(x, y), hints[(y * w + x) as usize] as i32);
            }
        }
    }
}
