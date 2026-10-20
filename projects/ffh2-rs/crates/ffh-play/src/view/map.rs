//! The map arrays and the plot query (design §4.4, §4.6), from the human team's side, and the
//! map tile deltas ([`MapTracker`], phase 6).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// The arrays of the `map` reply, in this order (one value per plot each).
pub const LAYERS: [&str; 11] = ["vis", "plot_type", "terrain", "feature", "bonus", "improvement", "route", "owner", "river", "city", "yield"];

/// One plot's values of the [`LAYERS`].
pub type Row = [i32; 11];

/// The map as the human sees it, one [`Row`] per plot (index `y * w + x`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapRows {
    pub w: i32,
    pub h: i32,
    pub wrap_x: bool,
    pub wrap_y: bool,
    pub reveal: bool,
    pub rows: Vec<Row>,
}

impl MapRows {
    /// `map_rev`: the FNV hash of the arrays, layer after layer (as they go over the wire).
    pub fn map_rev(&self) -> String {
        fnv((0..LAYERS.len()).flat_map(|l| self.rows.iter().map(move |r| r[l])))
    }

    /// The full `map` reply.
    pub fn to_json(&self) -> Json {
        let mut j = crate::jobj! { "w" => self.w, "h" => self.h, "wrap_x" => self.wrap_x, "wrap_y" => self.wrap_y, "map_rev" => self.map_rev(), "full" => true, "reveal" => self.reveal };
        for (l, name) in LAYERS.iter().enumerate() {
            j.set(name, self.rows.iter().map(|r| r[l]).collect::<Vec<i32>>());
        }
        j
    }

    /// The rows of a full `map` reply (`None` when it is not one).
    pub fn from_json(j: &Json) -> Option<MapRows> {
        let int = |k: &str| j.get(k).and_then(Json::as_i64);
        let (w, h) = (int("w")? as i32, int("h")? as i32);
        let n = (w.max(0) as usize) * (h.max(0) as usize);
        let mut rows = vec![[0; 11]; n];
        for (l, name) in LAYERS.iter().enumerate() {
            let a = j.get(name)?.as_arr()?;
            if a.len() != n {
                return None;
            }
            for (r, v) in rows.iter_mut().zip(a) {
                r[l] = v.as_i64()? as i32;
            }
        }
        let flag = |k: &str| j.get(k).and_then(Json::as_bool).unwrap_or(false);
        Some(MapRows { w, h, wrap_x: flag("wrap_x"), wrap_y: flag("wrap_y"), reveal: flag("reveal"), rows })
    }
}

/// The map rows from the human team's side: `vis` 0 = unrevealed (every other value is -1
/// there), 1 = fogged, 2 = visible.
pub fn rows(v: &Viewer) -> MapRows {
    let g = v.g;
    let mut rows = Vec::with_capacity(g.map.num_plots());
    for p in g.map.plot_ids() {
        let pl = &g.map.plots[p];
        rows.push(if !v.plot_revealed(p) {
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
        });
    }
    MapRows { w: g.map.grid_width, h: g.map.grid_height, wrap_x: g.map.wrap_x, wrap_y: g.map.wrap_y, reveal: v.reveal, rows }
}

/// `map`: flat arrays indexed by `y * w + x`, -1 = none (see [`rows`]).
pub fn map(v: &Viewer) -> Json {
    rows(v).to_json()
}

/// The map revision of [`map`] (an FNV hash of its arrays).
pub fn map_rev(v: &Viewer) -> String {
    rows(v).map_rev()
}

/// A tracker's identity: unique in the process and, through the start time, across processes
/// (a client that kept a revision from a server that was restarted gets the full map).
fn new_epoch() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    static START: OnceLock<u64> = OnceLock::new();
    let start = *START.get_or_init(|| {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        // No process ids on wasm32-wasip1 (`std::process::id` panics; the static site's engine,
        // docs/WEB.md): the start time alone tells its restarts apart.
        let pid = if cfg!(target_arch = "wasm32") { 0 } else { std::process::id() };
        nanos ^ ((pid as u64) << 40)
    });
    format!("{start:x}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The map tile deltas (design §4.4, phase 6): the map as last read, with the revision at
/// which each tile last changed (`tile_rev`), so that a client holding the map of revision `R`
/// gets only the tiles changed since (`map {since: R, epoch}`, `GET /api/map?since=R&epoch=E`).
/// Revisions count from 1 within an `epoch` (a tracker's identity); a `since` of another epoch
/// (another tracker: a new game, the other transport's tracker, a restarted server) or out of
/// range gets the full map, as does a delta that would not be much smaller.
#[derive(Clone, Debug)]
pub struct MapTracker {
    epoch: String,
    rev: i64,
    cur: Option<MapRows>,
    tile_rev: Vec<i64>,
}

impl Default for MapTracker {
    fn default() -> MapTracker {
        MapTracker { epoch: new_epoch(), rev: 0, cur: None, tile_rev: Vec::new() }
    }
}

impl MapTracker {
    /// Takes the current map. Returns the indices of the tiles that changed since the last one
    /// (`None`: every tile, the first map or one of another shape, which starts a new epoch).
    pub fn update(&mut self, m: MapRows) -> Option<Vec<usize>> {
        let same_shape = self.cur.as_ref().is_some_and(|c| (c.w, c.h, c.wrap_x, c.wrap_y) == (m.w, m.h, m.wrap_x, m.wrap_y) && c.rows.len() == m.rows.len());
        let Some(cur) = self.cur.as_mut().filter(|_| same_shape) else {
            self.epoch = new_epoch();
            self.rev = 1;
            self.tile_rev = vec![1; m.rows.len()];
            self.cur = Some(m);
            return None;
        };
        let changed: Vec<usize> = (0..m.rows.len()).filter(|&i| cur.rows[i] != m.rows[i]).collect();
        if !changed.is_empty() {
            self.rev += 1;
            for &i in &changed {
                self.tile_rev[i] = self.rev;
            }
        }
        *cur = m;
        Some(changed)
    }

    pub fn epoch(&self) -> &str {
        &self.epoch
    }

    /// The current revision (0 before the first map).
    pub fn rev(&self) -> i64 {
        self.rev
    }

    /// The current map's `map_rev`.
    pub fn map_rev(&self) -> Option<String> {
        self.cur.as_ref().map(MapRows::map_rev)
    }

    /// The full map of the current revision (with `epoch` and `rev`; `null` before the first).
    pub fn full(&self) -> Json {
        match &self.cur {
            Some(m) => m.to_json().with("epoch", self.epoch.as_str()).with("rev", self.rev),
            None => Json::Null,
        }
    }

    /// The reply to a client holding revision `since` of `epoch`: the tiles changed since
    /// (`full: false`, `since`, `tiles` = their indices, and each layer's values for those
    /// tiles in the same order), or the full map.
    pub fn reply(&self, since: Option<i64>, epoch: Option<&str>) -> Json {
        let Some(m) = &self.cur else { return Json::Null };
        let Some(since) = since.filter(|&s| epoch == Some(self.epoch.as_str()) && (1..=self.rev).contains(&s)) else { return self.full() };
        let tiles: Vec<usize> = (0..m.rows.len()).filter(|&i| self.tile_rev[i] > since).collect();
        if tiles.len() * 3 > m.rows.len() {
            return self.full();
        }
        let mut j = crate::jobj! {
            "w" => m.w, "h" => m.h, "wrap_x" => m.wrap_x, "wrap_y" => m.wrap_y, "map_rev" => m.map_rev(), "full" => false,
            "reveal" => m.reveal, "epoch" => self.epoch.as_str(), "rev" => self.rev, "since" => since,
        };
        for (l, name) in LAYERS.iter().enumerate() {
            j.set(name, tiles.iter().map(|&i| m.rows[i][l]).collect::<Vec<i32>>());
        }
        j.set("tiles", tiles);
        j
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(w: i32, h: i32, f: impl Fn(usize) -> i32) -> MapRows {
        let n = (w * h) as usize;
        MapRows { w, h, wrap_x: true, wrap_y: false, reveal: false, rows: (0..n).map(|i| [f(i); 11]).collect() }
    }

    /// Applies a reply to a client's map (the client's own logic, `tiles.js applyMapDelta`).
    fn apply(client: &Json, reply: &Json) -> Json {
        if reply.get("full").and_then(Json::as_bool) == Some(true) {
            return reply.clone();
        }
        let mut out = reply.clone();
        let tiles: Vec<usize> = reply.get("tiles").unwrap().as_arr().unwrap().iter().map(|t| t.as_i64().unwrap() as usize).collect();
        for name in LAYERS {
            let mut a: Vec<Json> = client.get(name).unwrap().as_arr().unwrap().to_vec();
            for (k, &i) in tiles.iter().enumerate() {
                a[i] = reply.get(name).unwrap().at(k).unwrap().clone();
            }
            out.set(name, Json::Arr(a));
        }
        out
    }

    fn same_map(a: &Json, b: &Json) {
        for k in LAYERS.iter().chain(&["w", "h", "map_rev", "epoch", "rev"]) {
            assert_eq!(a.get(k), b.get(k), "{k}");
        }
    }

    #[test]
    fn deltas_carry_the_tiles_changed_since_a_revision() {
        let mut t = MapTracker::default();
        assert!(t.reply(None, None).is_null());
        assert_eq!(t.update(rows(8, 4, |_| 0)), None);
        let r1 = t.full();
        assert_eq!((r1.req::<i64>("rev"), r1.req::<bool>("full")), (Ok(1), Ok(true)));
        let epoch = t.epoch().to_string();
        // No change: no new revision, an empty delta.
        assert_eq!(t.update(rows(8, 4, |_| 0)), Some(vec![]));
        let d = t.reply(Some(1), Some(&epoch));
        assert_eq!((d.req::<bool>("full"), d.req::<i64>("rev")), (Ok(false), Ok(1)));
        assert_eq!(d.get("tiles").unwrap().as_arr().unwrap().len(), 0);
        // Two tiles change, then one more.
        assert_eq!(t.update(rows(8, 4, |i| (i == 3 || i == 17) as i32)), Some(vec![3, 17]));
        let r2 = t.full();
        assert_eq!(t.update(rows(8, 4, |i| (i == 3 || i == 17 || i == 30) as i32 * 2)), Some(vec![3, 17, 30]));
        assert_eq!(t.update(rows(8, 4, |i| (i == 3 || i == 17 || i == 30) as i32 * 2 + (i == 5) as i32)), Some(vec![5]));
        let now = t.full();
        assert_eq!(now.req::<i64>("rev"), Ok(4));
        // From revision 2 the tiles changed in 3 and 4; from 3 only tile 5.
        let d2 = t.reply(Some(2), Some(&epoch));
        assert_eq!(d2.get("tiles"), Some(&Json::from(vec![3usize, 5, 17, 30])));
        same_map(&apply(&r2, &d2), &now);
        let d3 = t.reply(Some(3), Some(&epoch));
        assert_eq!(d3.get("tiles"), Some(&Json::from(vec![5usize])));
        assert_eq!(d3.get("vis"), Some(&Json::from(vec![1])));
        same_map(&apply(&r1, &t.reply(Some(1), Some(&epoch))), &now);
        // Another epoch, an unknown revision or none: the full map.
        for (since, ep) in [(Some(2), Some("other")), (Some(2), None), (Some(0), Some(epoch.as_str())), (Some(5), Some(epoch.as_str())), (None, Some(epoch.as_str()))] {
            assert_eq!(t.reply(since, ep).req::<bool>("full"), Ok(true), "{since:?} {ep:?}");
        }
        // More than a third of the map changed: the full map.
        t.update(rows(8, 4, |i| 7 + (i % 2) as i32));
        assert_eq!(t.reply(Some(4), Some(&epoch)).req::<bool>("full"), Ok(true));
        // Another shape: a new epoch.
        assert_eq!(t.update(rows(4, 4, |_| 0)), None);
        assert_ne!(t.epoch(), epoch);
        assert_eq!(t.rev(), 1);
        // The rows survive the full reply's JSON.
        let full = t.full();
        assert_eq!(MapRows::from_json(&full), Some(rows(4, 4, |_| 0)));
        assert_eq!(t.map_rev().as_deref(), full.get("map_rev").and_then(Json::as_str));
    }
}
