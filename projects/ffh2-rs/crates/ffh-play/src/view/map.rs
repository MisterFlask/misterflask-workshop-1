//! The map arrays and the plot query (design §4.4, §4.6), from the human team's side.

use ffh_core::ids::*;
use ffh_data::*;

use crate::json::Json;
use crate::query::{invalid, QueryErr};
use crate::view::{city_name, unit_json, Viewer};

/// `vis` values.
pub const UNREVEALED: i32 = 0;
pub const FOGGED: i32 = 1;
pub const VISIBLE: i32 = 2;

/// The `river` value of a plot: bit 0 = `n_of_river` (a river on the plot's south edge), bit 1 =
/// `w_of_river` (east edge), bits 2-4 = the north/south flow (`river_ns_direction` + 1, 0 =
/// none), bits 5-7 = the west/east flow (`river_we_direction` + 1).
pub fn river_bits(v: &Viewer, p: PlotIdx) -> i32 {
    let pl = &v.g.map.plots[p];
    let dir = |d: Option<CardinalDirectionType>| d.map_or(0, |d| d.idx() as i32 + 1);
    (pl.n_of_river as i32) | ((pl.w_of_river as i32) << 1) | (dir(pl.river_ns_direction) << 2) | (dir(pl.river_we_direction) << 5)
}

/// The plot's yields packed as `food | production << 8 | commerce << 16`, for plots the human
/// sees (a fogged plot's yield would show improvements built since it was seen).
pub fn packed_yield(v: &Viewer, p: PlotIdx) -> i32 {
    if !v.plot_visible(p) {
        return -1;
    }
    let y = &v.g.map.plots[p].yields;
    (y[0].clamp(0, 255)) | (y[1].clamp(0, 255) << 8) | (y[2].clamp(0, 255) << 16)
}

/// FNV-1a 64 over values (the `map_rev`).
fn fnv(values: impl Iterator<Item = i32>) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for v in values {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    format!("{h:016x}")
}

fn opt<I: Idx>(v: Option<I>) -> i32 {
    v.map_or(-1, |i| i.idx() as i32)
}

/// `map`: flat arrays indexed by `y * w + x`, -1 = none; `vis` 0 = unrevealed (every other array
/// is -1 there), 1 = fogged, 2 = visible.
pub fn map(v: &Viewer) -> Json {
    let g = v.g;
    let (w, h) = (g.map.grid_width, g.map.grid_height);
    let n = g.map.num_plots();
    let names = ["vis", "plot_type", "terrain", "feature", "bonus", "improvement", "route", "owner", "river", "city", "yield"];
    let mut arrays: Vec<Vec<i32>> = vec![Vec::with_capacity(n); names.len()];
    for p in g.map.plot_ids() {
        let pl = &g.map.plots[p];
        let row: [i32; 11] = if !v.plot_revealed(p) {
            let mut r = [-1; 11];
            r[0] = UNREVEALED;
            r
        } else {
            [
                if v.plot_visible(p) { VISIBLE } else { FOGGED },
                pl.plot_type.idx() as i32,
                opt(pl.terrain_type),
                opt(pl.feature_type),
                opt(v.plot_bonus(p)),
                opt(v.plot_improvement(p)),
                opt(v.plot_route(p)),
                v.plot_owner(p).map_or(-1, |o| o.0 as i32),
                river_bits(v, p),
                pl.plot_city.filter(|&c| v.city_revealed(c)).map_or(-1, |c| c.0 as i32),
                packed_yield(v, p),
            ]
        };
        for (a, x) in arrays.iter_mut().zip(row) {
            a.push(x);
        }
    }
    let rev = fnv(arrays.iter().flatten().copied());
    let mut j = crate::jobj! { "w" => w, "h" => h, "wrap_x" => g.map.wrap_x, "wrap_y" => g.map.wrap_y, "map_rev" => rev, "full" => true, "reveal" => v.reveal };
    for (name, a) in names.iter().zip(arrays) {
        j.set(name, a);
    }
    j
}

/// The map revision of [`map`] (an FNV hash of its arrays).
pub fn map_rev(v: &Viewer) -> String {
    map(v).get("map_rev").and_then(Json::as_str).unwrap_or_default().to_string()
}

/// A plot on the map, or `invalid`.
pub fn plot_at(v: &Viewer, x: i32, y: i32) -> Result<PlotIdx, QueryErr> {
    let g = v.g;
    let (w, h) = (g.map.grid_width, g.map.grid_height);
    if x < 0 || y < 0 || x >= w || y >= h {
        return Err(invalid(format!("plot ({x}, {y}) is off the map ({w} x {h})")));
    }
    g.map.plot_xy(x, y).ok_or_else(|| invalid(format!("plot ({x}, {y}) is off the map")))
}

/// The base movement cost of a plot (C++ plot help): its feature's or terrain's cost, plus the
/// hills' extra; peaks are impassable to most units.
fn base_move_cost(v: &Viewer, p: PlotIdx) -> i32 {
    let g = v.g;
    let d = g.d;
    let pl = &g.map.plots[p];
    let mut cost = match (pl.feature_type, pl.terrain_type) {
        (Some(f), _) => d.features[f].movement_cost,
        (None, Some(t)) => d.terrains[t].movement_cost,
        _ => 1,
    };
    if g.plot_is_hills(p) {
        cost += d.defines.hills_extra_movement;
    }
    cost
}

/// `plot {x,y}`: the plot as the human knows it; units and live values only while visible.
pub fn plot(v: &Viewer, x: i32, y: i32) -> Result<Json, QueryErr> {
    let p = plot_at(v, x, y)?;
    let g = v.g;
    let d = g.d;
    if !v.plot_revealed(p) {
        return Ok(crate::jobj! { "x" => x, "y" => y, "vis" => UNREVEALED });
    }
    let visible = v.plot_visible(p);
    let pl = &g.map.plots[p];
    let mut j = crate::jobj! {
        "x" => x,
        "y" => y,
        "vis" => if visible { VISIBLE } else { FOGGED },
        "plot_type" => Json::idx(pl.plot_type),
        "terrain" => Json::opt_idx(pl.terrain_type),
        "feature" => Json::opt_idx(pl.feature_type),
        "hills" => g.plot_is_hills(p),
        "bonus" => Json::opt_idx(v.plot_bonus(p)),
        "improvement" => Json::opt_idx(v.plot_improvement(p)),
        "route" => Json::opt_idx(v.plot_route(p)),
        "owner" => v.plot_owner(p),
        "river" => river_bits(v, p),
        "river_side" => g.plot_is_river(p),
        "city" => pl.plot_city.filter(|&c| v.city_revealed(c)).map(|c| crate::jobj! { "id" => c, "name" => city_name(g, c), "owner" => g.cities[c].owner }),
        "move_cost" => base_move_cost(v, p),
    };
    if visible {
        let owner_team = pl.owner.map(|o| g.players[o].team);
        j.set("yields", pl.yields.to_vec());
        j.set("defense", g.plot_defense_modifier(p, owner_team, true));
        // Culture of the players the human knows (C++ plot help lists the culture shares).
        let culture: Vec<Json> = g
            .player_ids()
            .filter(|&q| pl.culture[q] > 0 && v.player_met(q))
            .map(|q| crate::jobj! { "player" => q, "culture" => pl.culture[q], "percent" => g.plot_calculate_culture_percent(p, q) })
            .collect();
        j.set("culture", Json::Arr(culture));
        if let Some(i) = v.plot_improvement(p) {
            if d.improvements[i].improvement_upgrade.is_some() {
                j.set("upgrade_turns_left", g.plot_get_upgrade_time_left(p, i, pl.owner));
            }
        }
        // Builds in progress (workers' turns so far).
        let builds: Vec<Json> =
            d.builds.keys().filter(|&b| pl.build_progress[b] > 0).map(|b| crate::jobj! { "build" => Json::idx(b), "progress" => pl.build_progress[b] }).collect();
        j.set("builds", Json::Arr(builds));
        let units: Vec<Json> = pl
            .units
            .iter()
            .filter(|&&u| v.unit_visible(u))
            .map(|&u| {
                let mut uj = unit_json(v, u);
                uj.set("strength", g.unit_base_combat_str(u));
                uj.set("defense_strength", g.unit_base_combat_str_defense(u));
                uj
            })
            .collect();
        j.set("units", Json::Arr(units));
    }
    Ok(j)
}
