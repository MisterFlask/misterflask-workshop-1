//! `QueryGuard`: every read of `&Game` outside a rules call goes through one (design §2.5).
//!
//! Some `&self` rules functions write saved state through interior mutability (F11): the six
//! path finders (`RefCell<PathFinder>`, filled by `group_generate_path`, `unit_get_upgrade_city`,
//! `player_count_unimproved_bonuses`, the step/area finders), `Player::ai_bonus_value`
//! (`Cell`, filled by `AI_baseBonusVal`) and the city rank caches (`CacheI32`/`CacheBool`,
//! filled by any happiness display through `city_largest_city_happiness`). A read-only UI
//! would otherwise change later rules results and the save bytes. The guard snapshots all of
//! it when created and writes it back when dropped, so a read leaves `Game` exactly as it was.
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

/// A read-only view of a `Game` that undoes, when dropped, every interior-mutable change made
/// through it (see the module docs). Derefs to `&Game`.
pub struct QueryGuard<'a> {
    g: &'a Game,
    ranks: Vec<Ranks>,
    bonus: Vec<Vec<i32>>,
    /// A clone of each finder, or `None` when it was borrowed at creation (a read inside a rules
    /// call that holds the finder): finder-using reads must then not run (`finders_busy`).
    finders: [Option<PathFinder>; 6],
}

impl<'a> QueryGuard<'a> {
    pub fn new(g: &'a Game) -> QueryGuard<'a> {
        let ranks = g.cities.iter().map(|(id, c)| Ranks::take(id, c)).collect();
        let bonus = g.player_ids().map(|p| g.players[p].ai_bonus_value.iter().map(|c| c.get()).collect()).collect();
        let finders = finders(g).map(|f| f.try_borrow().ok().map(|f| f.clone()));
        QueryGuard { g, ranks, bonus, finders }
    }

    /// Some finder was borrowed when the guard was made: reads that search paths must not run.
    pub fn finders_busy(&self) -> bool {
        self.finders.iter().any(Option::is_none)
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
            if let Some(saved) = saved.take() {
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
}
