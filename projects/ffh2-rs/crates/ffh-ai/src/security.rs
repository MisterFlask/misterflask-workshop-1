//! Worker safety against barbarians and raiders (`barbarian-security`).
//!
//! Once per view of a civ player, a *secure-zone* map:
//! - **threat2**: per plot, the attack power of visible hostile units that can reach it within
//!   two turns (2 x moves + 1 plots; animals only outside borders, which they cannot enter),
//!   plus a nominal threat within two plots of a revealed lair (an improvement that spawns
//!   units) or enemy city;
//! - **refuge**: plots holding our land fighters (garrisoned cities included); **covered**:
//!   plots within one step of a refuge.
//!
//! A plot is *secure* when nothing threatens it within two turns, or it is covered (a worker
//! there steps into the refuge when a raider closes in).
//!
//! Workers (`workers.rs`) use it at the end of the turn: while an enemy can reach a plot the
//! worker could step on this turn, a job the worker does not reach this turn is taken only if
//! the plot it stops on tonight is out of the enemy's reach and secure. Otherwise the worker,
//! sent out of its city by the job search, stopped next to the raider on the way to a job the
//! raider could not reach. A worker whose own plot one of our fighters holds (at least as
//! strong as what can reach it) does not flee.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::view::{attack_power, defense_power, View};

const REFUGE: u8 = 1;
const COVERED: u8 = 2;
/// Threat counted near a spawn site (lair, enemy city) that shows no unit yet.
const SPAWN_THREAT: i32 = 1;

#[derive(Default)]
pub struct Security {
    /// Per plot: hostile attack power that can reach it within two turns (see module doc).
    threat2: Vec<i32>,
    /// Per plot: `REFUGE` / `COVERED` bits.
    cover: Vec<u8>,
}

/// A land unit of ours that can hold a plot against a raider (not a worker or settler).
fn is_fighter(g: &Game, u: UnitId) -> bool {
    let info = &g.d.units[g.units[u].unit_type];
    !info.found && (info.combat > 0 || info.combat_defense > 0) && defense_power(g, u) > 0 && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND && !g.unit_is_cargo(u)
}

/// Our fighters' defense power on `plot`.
pub fn fighters_at(g: &Game, p: PlayerId, plot: PlotIdx) -> i32 {
    g.map.plots[plot].units.iter().filter(|&&u| g.units[u].owner == p && is_fighter(g, u)).map(|&u| defense_power(g, u)).sum()
}

impl Security {
    pub fn new(g: &Game, v: &View) -> Security {
        let n = g.map.num_plots();
        let mut s = Security { threat2: vec![0; n], cover: vec![0; n] };
        let team = v.team;
        // Visible hostile units: two turns of movement (one more step for the attack itself).
        for pl in g.players.iter() {
            if !pl.alive || !v.enemies.contains(&pl.team) {
                continue;
            }
            for &u in &pl.units {
                if !g.units.contains(u) {
                    continue;
                }
                let unit = &g.units[u];
                let Some(plot) = g.map.plot_xy(unit.x, unit.y) else { continue };
                if !g.plot_is_visible(plot, team) {
                    continue;
                }
                let power = attack_power(g, u);
                if power == 0 {
                    continue;
                }
                if g.map.plots[plot].plot_city.is_some() && unit.ai_type == Some(UnitAIType::UNITAI_CITY_DEFENSE) {
                    continue;
                }
                let info = &g.d.units[unit.unit_type];
                let range = (2 * info.moves.max(1) + 1).min(5);
                s.add(g, unit.x, unit.y, range, power, info.animal);
            }
            // Their cities train raiders.
            for &c in &pl.cities {
                let cp = g.city_plot(c);
                if g.map.plots[cp].revealed[team] {
                    s.add(g, g.cities[c].x, g.cities[c].y, 2, SPAWN_THREAT, false);
                }
            }
        }
        // Lairs spawn barbarians (animal dens: animals, which stay out of owned land).
        let barb_team = g.players[g.barbarian_player()].team;
        if v.enemies.contains(&barb_team) {
            for (i, pl) in g.map.plots.iter().enumerate() {
                let Some(imp) = pl.improvement_type else { continue };
                let Some(su) = g.d.improvements[imp].spawn_unit_type else { continue };
                if !pl.revealed[team] {
                    continue;
                }
                let (x, y) = g.map.xy(PlotIdx::from_usize(i));
                s.add(g, x, y, 2, SPAWN_THREAT, g.d.units[su].animal);
            }
        }
        // Refuges: our defended cities and our fighters in the field.
        for &u in &g.players[v.p].units {
            if !g.units.contains(u) || !is_fighter(g, u) {
                continue;
            }
            let unit = &g.units[u];
            let Some(plot) = g.map.plot_xy(unit.x, unit.y) else { continue };
            if s.cover[plot.idx()] & REFUGE != 0 {
                continue;
            }
            if g.map.plots[plot].plot_city.is_some_and(|c| g.cities[c].owner != v.p) {
                continue;
            }
            s.cover[plot.idx()] |= REFUGE;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(q) = g.plot_xy(unit.x, unit.y, dx, dy) {
                        if !g.plot_is_water(q) {
                            s.cover[q.idx()] |= COVERED;
                        }
                    }
                }
            }
        }
        s
    }

    fn add(&mut self, g: &Game, x: i32, y: i32, range: i32, power: i32, animal: bool) {
        for dx in -range..=range {
            for dy in -range..=range {
                if let Some(q) = g.plot_xy(x, y, dx, dy) {
                    if animal && g.map.plots[q].owner.is_some() {
                        continue;
                    }
                    self.threat2[q.idx()] += power;
                }
            }
        }
    }

    pub fn threat2(&self, q: PlotIdx) -> i32 {
        self.threat2.get(q.idx()).copied().unwrap_or(0)
    }

    /// A refuge here or one step away.
    pub fn covered(&self, q: PlotIdx) -> bool {
        self.cover.get(q.idx()).is_some_and(|&b| b & (REFUGE | COVERED) != 0)
    }

    /// Nothing threatens the plot within two turns, or a refuge is one step away.
    pub fn secure(&self, q: PlotIdx) -> bool {
        self.threat2(q) == 0 || self.covered(q)
    }
}

/// An enemy can reach a worker on this plot next turn and nothing here protects it (no city,
/// no fighter of ours at least as strong as what can reach it).
pub fn worker_threatened(g: &Game, v: &View, plot: PlotIdx) -> bool {
    v.danger_at(plot) > 0 && g.map.plots[plot].plot_city.is_none() && !holds(g, v, plot)
}

/// Our fighters on `plot` are at least as strong as what can reach it next turn.
fn holds(g: &Game, v: &View, plot: PlotIdx) -> bool {
    let f = fighters_at(g, v.p, plot);
    f > 0 && f >= v.danger_at(plot)
}

/// When an enemy can reach a plot the worker could step on this turn: the plots it reaches
/// this turn. None otherwise (no restriction on its jobs, see [`job_ok`]).
pub fn tonight(g: &Game, v: &mut View, u: UnitId) -> Option<Vec<PlotIdx>> {
    let reach = v.flood.run(g, u, 0, false);
    if reach.iter().any(|r| v.danger_at(r.plot) > 0) {
        Some(reach.iter().map(|r| r.plot).collect())
    } else {
        None
    }
}

/// May a worker take a job on `q`? While a raider is close (`tonight`, see [`tonight`]), only
/// if the worker gets there this turn, or stops tonight (estimated: on the plot it reaches this
/// turn nearest to `q`) where no enemy can reach it next turn and that is secure.
pub fn job_ok(g: &Game, v: &View, q: PlotIdx, tonight: Option<&[PlotIdx]>) -> bool {
    if v.danger_at(q) > 0 {
        return holds(g, v, q);
    }
    match tonight {
        Some(t) if !t.contains(&q) => overnight(g, q, t).is_some_and(|o| v.danger_at(o) == 0 && v.security.secure(o)),
        _ => true,
    }
}

/// Where a worker heading for `q` stops tonight, estimated: the plot it reaches this turn
/// (`tonight`) nearest to `q`.
fn overnight(g: &Game, q: PlotIdx, tonight: &[PlotIdx]) -> Option<PlotIdx> {
    let (qx, qy) = g.map.xy(q);
    tonight.iter().copied().min_by_key(|&o| {
        let (ox, oy) = g.map.xy(o);
        (g.plot_distance(ox, oy, qx, qy), o)
    })
}
