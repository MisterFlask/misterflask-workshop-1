//! Default map script ("Fractal"): a Rust re-implementation of the BTS Python
//! map script `Fractal.py` and the parts of `CvMapGeneratorUtil.py` it uses
//! (`FractalWorld`, `TerrainGenerator`, `FeatureGenerator`).
//!
//! Civ4 map scripts are Python files outside the DLL and are not part of FFH2's
//! rules; the original BTS scripts are not available in this environment, so this
//! follows their published algorithm (fractal heightmaps with percentile
//! thresholds) rather than being a line-by-line port. The DLL-side steps
//! (rivers, lakes, bonuses, goodies, start plots) are ports of CvMapGenerator /
//! CvGame and live elsewhere.

use crate::mapgen::fractal::{Fractal, FRAC_POLAR, FRAC_WRAP_X, FRAC_WRAP_Y};
use crate::rng::Rng;
use ffh_data::*;

/// Parameters of `FractalWorld.generatePlotTypes` (BTS defaults).
#[derive(Clone, Debug)]
pub struct FractalWorldParams {
    pub continent_grain: i32,
    /// `-1` = no rifts (Fractal.py uses no rifts).
    pub rift_grain: i32,
    pub has_center_rift: bool,
    pub polar: bool,
    pub water_percent: i32,
    pub shift_plot_types: bool,
    pub grain_amount: i32,
    pub hill_group_one_base: i32,
    pub hill_group_one_range: i32,
    pub hill_group_two_base: i32,
    pub hill_group_two_range: i32,
    pub peak_percent: i32,
    pub frac_x_exp: i32,
    pub frac_y_exp: i32,
}

impl Default for FractalWorldParams {
    fn default() -> Self {
        FractalWorldParams {
            continent_grain: 2,
            rift_grain: -1,
            has_center_rift: false,
            polar: true,
            water_percent: 78,
            shift_plot_types: true,
            grain_amount: 3,
            hill_group_one_base: 25,
            hill_group_one_range: 5,
            hill_group_two_base: 75,
            hill_group_two_range: 5,
            peak_percent: 12,
            frac_x_exp: 7,
            frac_y_exp: 6,
        }
    }
}

fn wrap_flags(wrap_x: bool, wrap_y: bool) -> i32 {
    (if wrap_x { FRAC_WRAP_X } else { 0 }) | (if wrap_y { FRAC_WRAP_Y } else { 0 })
}

/// `FractalWorld.generatePlotTypes`: returns one `PlotType` per plot (row-major, y*w+x).
pub fn generate_plot_types(
    w: i32,
    h: i32,
    wrap_x: bool,
    wrap_y: bool,
    sea_level_change: i32,
    rng: &mut Rng,
    p: &FractalWorldParams,
) -> Vec<PlotType> {
    let mut flags = wrap_flags(wrap_x, wrap_y);
    if p.polar {
        flags |= FRAC_POLAR;
    }
    // initFractal
    let continents = if p.rift_grain >= 0 {
        let rifts = Fractal::init(w, h, p.rift_grain, rng, flags, None, p.frac_x_exp, p.frac_y_exp);
        let mut f2 = flags;
        if p.has_center_rift {
            f2 |= crate::mapgen::fractal::FRAC_CENTER_RIFT;
        }
        Fractal::init(w, h, p.continent_grain, rng, f2, Some(&rifts), p.frac_x_exp, p.frac_y_exp)
    } else {
        Fractal::init(w, h, p.continent_grain, rng, flags, None, p.frac_x_exp, p.frac_y_exp)
    };
    // generatePlotTypes
    let hills = Fractal::init(w, h, p.grain_amount, rng, flags, None, p.frac_x_exp, p.frac_y_exp);
    let peaks = Fractal::init(w, h, p.grain_amount + 1, rng, flags, None, p.frac_x_exp, p.frac_y_exp);
    let water_percent = (p.water_percent + sea_level_change).clamp(0, 100);
    let water_threshold = continents.height_from_percent(water_percent);
    let hills_bottom1 = hills.height_from_percent((p.hill_group_one_base - p.hill_group_one_range).max(0));
    let hills_top1 = hills.height_from_percent((p.hill_group_one_base + p.hill_group_one_range).min(100));
    let hills_bottom2 = hills.height_from_percent((p.hill_group_two_base - p.hill_group_two_range).max(0));
    let hills_top2 = hills.height_from_percent((p.hill_group_two_base + p.hill_group_two_range).min(100));
    let peak_threshold = peaks.height_from_percent(p.peak_percent);

    let mut out = vec![PlotType::PLOT_OCEAN; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let val = continents.height(x, y);
            if val <= water_threshold {
                out[i] = PlotType::PLOT_OCEAN;
            } else {
                let hv = hills.height(x, y);
                if (hv >= hills_bottom1 && hv <= hills_top1) || (hv >= hills_bottom2 && hv <= hills_top2) {
                    out[i] = if peaks.height(x, y) <= peak_threshold { PlotType::PLOT_PEAK } else { PlotType::PLOT_HILLS };
                } else {
                    out[i] = PlotType::PLOT_LAND;
                }
            }
        }
    }
    if p.shift_plot_types {
        shift_plot_types(&mut out, w, h, wrap_x, wrap_y);
    }
    out
}

const STRIP_RADIUS: i32 = 15;

fn calc_weights(strip_radius: i32) -> Vec<i32> {
    let strip_size = 2 * strip_radius;
    (0..strip_size)
        .map(|i| {
            let from_start = i + 1;
            let from_end = strip_size - i;
            let from_edge = from_start.min(from_end);
            let mut weight = from_edge;
            let from_center = strip_radius - from_edge;
            if from_center <= 1 {
                weight *= strip_radius;
            }
            if from_center == 0 {
                weight *= 2;
            }
            weight
        })
        .collect()
}

/// Finds the column (or row) through which to "cut" a wrapping map so that land
/// masses are not split by the visible map edge (`FractalWorld.shiftPlotTypes`).
fn best_split(plot_types: &[PlotType], w: i32, h: i32, along_x: bool) -> i32 {
    let (n_lines, line_len) = if along_x { (w, h) } else { (h, w) };
    let strip_size = 2 * STRIP_RADIUS;
    if strip_size > line_len.max(n_lines) {
        return 0;
    }
    let weights = calc_weights(STRIP_RADIUS);
    let mut scores = vec![0i64; n_lines as usize];
    for line in 0..n_lines {
        let mut land = 0i64;
        let mut found = false;
        for k in 0..line_len {
            let (x, y) = if along_x { (line, k) } else { (k, line) };
            if plot_types[(y * w + x) as usize] == PlotType::PLOT_LAND {
                land += 1;
                found = true;
            }
        }
        if found {
            land += 30;
        }
        for (i, wgt) in weights.iter().enumerate() {
            let idx = (line + i as i32 - STRIP_RADIUS).rem_euclid(n_lines);
            scores[idx as usize] += land * *wgt as i64;
        }
    }
    let mut best = 0;
    for (i, s) in scores.iter().enumerate() {
        if *s < scores[best] {
            best = i;
        }
    }
    best as i32
}

fn shift_plot_types(pt: &mut [PlotType], w: i32, h: i32, wrap_x: bool, wrap_y: bool) {
    let sx = if wrap_x { best_split(pt, w, h, true) } else { 0 };
    let sy = if wrap_y { best_split(pt, w, h, false) } else { 0 };
    if sx == 0 && sy == 0 {
        return;
    }
    let buf = pt.to_vec();
    for y in 0..h {
        for x in 0..w {
            let src_x = (x + sx).rem_euclid(w);
            let src_y = (y + sy).rem_euclid(h);
            pt[(y * w + x) as usize] = buf[(src_y * w + src_x) as usize];
        }
    }
}

/// Terrain choices used by [`generate_terrain`] (FFH2 type ids).
#[derive(Clone, Debug)]
pub struct TerrainSet {
    pub grass: TerrainType,
    pub plains: TerrainType,
    pub desert: TerrainType,
    pub tundra: TerrainType,
    pub snow: TerrainType,
}

impl TerrainSet {
    pub fn from_data(d: &GameData) -> TerrainSet {
        TerrainSet {
            grass: d.t("TERRAIN_GRASS"),
            plains: d.t("TERRAIN_PLAINS"),
            desert: d.t("TERRAIN_DESERT"),
            tundra: d.t("TERRAIN_TUNDRA"),
            snow: d.t("TERRAIN_SNOW"),
        }
    }
}

/// Climate-adjusted parameters of `TerrainGenerator`.
#[derive(Clone, Debug)]
pub struct TerrainParams {
    pub desert_percent: i32,
    pub plains_percent: i32,
    pub snow_latitude: f32,
    pub tundra_latitude: f32,
    pub grass_latitude: f32,
    pub desert_bottom_latitude: f32,
    pub desert_top_latitude: f32,
    pub grain_amount: i32,
}

impl TerrainParams {
    /// BTS defaults adjusted by the climate and world size, as `TerrainGenerator.__init__` does.
    pub fn new(d: &GameData, climate: ClimateType, world: WorldSizeType) -> TerrainParams {
        let c = &d.climates[climate];
        let wi = &d.worlds[world];
        let clamp01 = |v: f32| v.clamp(0.0, 1.0);
        TerrainParams {
            desert_percent: (32 + c.desert_percent_change).clamp(0, 100),
            plains_percent: 18,
            snow_latitude: clamp01(0.7 + c.snow_latitude_change),
            tundra_latitude: clamp01(0.6 + c.tundra_latitude_change),
            grass_latitude: clamp01(0.1 + c.grass_latitude_change),
            desert_bottom_latitude: clamp01(0.2 + c.desert_bottom_latitude_change),
            desert_top_latitude: clamp01(0.5 + c.desert_top_latitude_change),
            grain_amount: 4 + wi.terrain_grain_change,
        }
    }
}

/// `TerrainGenerator.generateTerrain`: terrain for land plots (water plots return `None`,
/// keeping the terrain the map assigned from plot type).
pub fn generate_terrain(
    w: i32,
    h: i32,
    wrap_x: bool,
    wrap_y: bool,
    is_water: &[bool],
    rng: &mut Rng,
    tp: &TerrainParams,
    ts: &TerrainSet,
) -> Vec<Option<TerrainType>> {
    let flags = wrap_flags(wrap_x, wrap_y); // FRAC_POLAR deliberately not used
    let deserts = Fractal::init(w, h, tp.grain_amount, rng, flags, None, -1, -1);
    let desert_top = deserts.height_from_percent(100);
    let desert_bottom = deserts.height_from_percent((100 - tp.desert_percent).max(0));
    let plains = Fractal::init(w, h, tp.grain_amount + 1, rng, flags, None, -1, -1);
    let plains_top = plains.height_from_percent(100);
    let plains_bottom = plains.height_from_percent((100 - tp.desert_percent - tp.plains_percent).max(0));
    let variation = Fractal::init(w, h, tp.grain_amount, rng, flags, None, -1, -1);

    let half = (h / 2) as f32;
    let mut out = vec![None; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if is_water[i] {
                continue;
            }
            let mut lat = ((h / 2) - y).abs() as f32 / half;
            lat += (128 - variation.height(x, y)) as f32 / (255.0 * 5.0);
            let lat = lat.clamp(0.0, 1.0);
            let t = if lat >= tp.snow_latitude {
                ts.snow
            } else if lat >= tp.tundra_latitude {
                ts.tundra
            } else if lat < tp.grass_latitude {
                ts.grass
            } else {
                let dv = deserts.height(x, y);
                let pv = plains.height(x, y);
                if dv >= desert_bottom && dv <= desert_top && lat >= tp.desert_bottom_latitude && lat < tp.desert_top_latitude {
                    ts.desert
                } else if pv >= plains_bottom && pv <= plains_top {
                    ts.plains
                } else {
                    ts.grass
                }
            };
            out[i] = Some(t);
        }
    }
    out
}
