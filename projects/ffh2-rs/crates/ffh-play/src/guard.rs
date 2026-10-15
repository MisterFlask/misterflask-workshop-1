//! `QueryGuard`: every read of `&Game` outside a rules call goes through one (design §2.5).
//!
//! Some `&self` rules functions write saved state through interior mutability (F11): the six
//! path finders (`RefCell<PathFinder>`, filled by `group_generate_path`, `unit_get_upgrade_city`,
//! `player_count_unimproved_bonuses`, the step/area finders), `Player::ai_bonus_value`
//! (`Cell`, filled by `AI_baseBonusVal`) and the city rank caches (`CacheI32`/`CacheBool`,
//! filled by any happiness display through `city_largest_city_happiness`). A read-only UI
//! would otherwise change later rules results and the save bytes. The guard snapshots the
//! caches when created and writes them back when dropped, so a read leaves `Game` exactly as it
//! was.
//!
//! The finders are not copied (P3 review: cloning all six cost about 0.9 ms per guard on a huge
//! map, where each holds about 1.1 MB of nodes): the guard moves each one out of the game, puts
//! a fresh finder of the same kind in its place (it initializes itself if a read searches with
//! it), and moves the game's own back when dropped. That is O(1), and the game's finders come
//! back exactly as they were, whatever a read did with the stand-ins.
//!
//! The guard is the safety net, not a licence: queries still keep to the allow-list (§2.5).

use std::ops::Deref;

use ffh_core::city::City;
use ffh_core::game::Game;
use ffh_core::ids::CityId;
use ffh_core::path::PathFinder;
use ffh_data::{CommerceType, YieldType};

/// One city's rank caches (22 cells).
struct Ranks {
    city: CityId,
    population: (i32, bool),
    base_yield: [(i32, bool); YieldType::NUM],
    yield_: [(i32, bool); YieldType::NUM],
    commerce: [(i32, bool); CommerceType::NUM],
}

impl Ranks {
    fn take(city: CityId, c: &City) -> Ranks {
        Ranks {
            city,
            population: (c.population_rank.get(), c.population_rank_valid.get()),
            base_yield: std::array::from_fn(|i| (c.base_yield_rank[i].get(), c.base_yield_rank_valid[i].get())),
            yield_: std::array::from_fn(|i| (c.yield_rank[i].get(), c.yield_rank_valid[i].get())),
            commerce: std::array::from_fn(|i| (c.commerce_rank[i].get(), c.commerce_rank_valid[i].get())),
        }
    }

    fn restore(&self, c: &City) {
        c.population_rank.set(self.population.0);
        c.population_rank_valid.set(self.population.1);
        for i in 0..YieldType::NUM {
            c.base_yield_rank[i].set(self.base_yield[i].0);
            c.base_yield_rank_valid[i].set(self.base_yield[i].1);
            c.yield_rank[i].set(self.yield_[i].0);
            c.yield_rank_valid[i].set(self.yield_[i].1);
        }
        for i in 0..CommerceType::NUM {
            c.commerce_rank[i].set(self.commerce[i].0);
            c.commerce_rank_valid[i].set(self.commerce[i].1);
        }
    }
}

/// The game's six finders, in `Game` field order.
fn finders(g: &Game) -> [&std::cell::RefCell<PathFinder>; 6] {
    [&g.path_finder, &g.step_finder, &g.route_finder, &g.border_finder, &g.area_finder, &g.plot_group_finder]
}

/// What the guard did with one of the game's finders.
enum Finder {
    /// Moved out of the game (a fresh stand-in is there); moved back on drop.
    Saved(PathFinder),
    /// Borrowed by the rules call the read happens in, so left in place: shared (a read cannot
    /// change it either), or exclusive (`busy`: finder-using reads must not run).
    InUse { busy: bool },
}

/// A read-only view of a `Game` that undoes, when dropped, every interior-mutable change made
/// through it (see the module docs). Derefs to `&Game`.
pub struct QueryGuard<'a> {
    g: &'a Game,
    ranks: Vec<Ranks>,
    bonus: Vec<Vec<i32>>,
    finders: [Finder; 6],
}

impl<'a> QueryGuard<'a> {
    pub fn new(g: &'a Game) -> QueryGuard<'a> {
        let ranks = g.cities.iter().map(|(id, c)| Ranks::take(id, c)).collect();
        let bonus = g.player_ids().map(|p| g.players[p].ai_bonus_value.iter().map(|c| c.get()).collect()).collect();
        let finders = finders(g).map(|cell| match cell.try_borrow_mut() {
            Ok(mut f) => {
                let stand_in = PathFinder::new(f.kind);
                Finder::Saved(std::mem::replace(&mut *f, stand_in))
            }
            Err(_) => Finder::InUse { busy: cell.try_borrow().is_err() },
        });
        QueryGuard { g, ranks, bonus, finders }
    }

    /// Some finder was held exclusively by a rules call when the guard was made (a read inside
    /// that call): reads that search paths must not run.
    pub fn finders_busy(&self) -> bool {
        self.finders.iter().any(|f| matches!(f, Finder::InUse { busy: true }))
    }

    /// The guarded game (same as deref).
    pub fn game(&self) -> &'a Game {
        self.g
    }
}

impl Deref for QueryGuard<'_> {
    type Target = Game;
    fn deref(&self) -> &Game {
        self.g
    }
}

impl Drop for QueryGuard<'_> {
    fn drop(&mut self) {
        let g = self.g;
        for r in &self.ranks {
            if let Some(c) = g.cities.get(r.city) {
                r.restore(c);
            }
        }
        for (p, values) in g.player_ids().zip(&self.bonus) {
            for (cell, &v) in g.players[p].ai_bonus_value.iter().zip(values) {
                cell.set(v);
            }
        }
        for (cell, saved) in finders(g).into_iter().zip(self.finders.iter_mut()) {
            if let Finder::Saved(saved) = std::mem::replace(saved, Finder::InUse { busy: false }) {
                if let Ok(mut f) = cell.try_borrow_mut() {
                    *f = saved;
                }
            }
        }
    }
}

/// Runs `f` on `g` under a [`QueryGuard`].
pub fn read<R>(g: &Game, f: impl FnOnce(&Game) -> R) -> R {
    let guard = QueryGuard::new(g);
    f(&guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::human::AdvisorAnswerer;
    use crate::session::{Session, SessionOptions};
    use crate::setup::NewGame;
    use ffh_core::ids::{PlayerId, PlotIdx};
    use ffh_data::Idx;
    use std::sync::Arc;

    /// Happiness displays fill the population-rank cache (F11); under a guard the save bytes are
    /// unchanged afterwards. Without the guard they change (the test would be vacuous otherwise).
    #[test]
    fn city_screen_reads_leave_save_bytes_unchanged() {
        let d = ffh_data::global();
        let mut ng = NewGame::new(d, 5);
        ng.opponents = 2;
        ng.world = d.t("WORLDSIZE_DUEL");
        let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
        // Let the AI play the human slot for a while so cities exist.
        let g = &mut s.g;
        g.set_ai_auto_play(100);
        while g.game_turn() < 12 || !g.at_turn_boundary() {
            g.update();
        }
        assert!(g.cities.len() > 1);
        let reads = |g: &Game| {
            for (c, _) in g.cities.iter() {
                let _ = g.city_happy_level(c);
                let _ = g.city_unhappy_level(c, 0);
                let _ = g.city_largest_city_happiness(c);
                for y in 0..YieldType::NUM {
                    let _ = g.city_find_base_yield_rate_rank(c, YieldType::from_usize(y));
                }
            }
        };
        // Invalidate the caches so the reads have something to fill.
        for (_, c) in g.cities.iter() {
            c.population_rank_valid.set(false);
            for v in &c.base_yield_rank_valid {
                v.set(false);
            }
        }
        let before = g.save();
        read(g, reads);
        assert_eq!(before, g.save(), "a guarded read changed the save");
        reads(g);
        assert_ne!(before, g.save(), "unguarded reads should fill the rank caches");
    }

    /// A read that searches with the game's path finder (off the allow-list) searches with the
    /// guard's stand-in: the path is found, and the game's finder comes back as it was. A finder
    /// a rules call holds exclusively makes the guard `finders_busy`.
    #[test]
    fn finders_are_swapped_out_and_back() {
        let d = ffh_data::global();
        let mut ng = NewGame::new(d, 3);
        ng.opponents = 1;
        ng.world = d.t("WORLDSIZE_DUEL");
        let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
        let g = &mut s.g;
        g.set_ai_auto_play(100);
        while g.game_turn() < 5 || !g.at_turn_boundary() {
            g.update();
        }
        let grp = g.players[PlayerId(0)].groups[0];
        let from = g.unit_plot(g.groups[grp].units[0]);
        let (fx, fy) = g.map.xy(from);
        let near: Vec<PlotIdx> = g.map.plot_ids().filter(|&p| g.plot_distance(fx, fy, g.map.xy(p).0, g.map.xy(p).1) == 2).collect();
        let to = read(g, |g| near.iter().copied().find(|&p| g.group_generate_path(grp, from, p, 0, false).is_some())).expect("a reachable plot");
        let before = g.save();
        let found = read(g, |g| {
            assert!(g.path_finder.borrow().nodes.is_empty(), "the game's finder is swapped out");
            g.group_generate_path(grp, from, to, 0, false)
        });
        assert!(found.is_some());
        assert_eq!(before, g.save(), "a guarded search changed the save");
        assert!(!g.path_finder.borrow().nodes.is_empty(), "the game's finder is back");
        // Unguarded, the same search changes the saved finder (the test would be vacuous
        // otherwise).
        g.group_generate_path(grp, from, to, 0, false);
        assert_ne!(before, g.save());
        // Inside a rules call that holds a finder: busy, and the finder is left alone.
        let held = g.border_finder.borrow_mut();
        let guard = QueryGuard::new(g);
        assert!(guard.finders_busy());
        drop(guard);
        drop(held);
        assert!(!QueryGuard::new(g).finders_busy());
    }
}
