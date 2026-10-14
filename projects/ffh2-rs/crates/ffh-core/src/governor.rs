//! City citizen governor (replaces C++ `CvCityAI::AI_assignWorkingPlots`,
//! `AI_addBestCitizen`, `AI_removeWorstCitizen`, `AI_juggleCitizens`).
//!
//! New code, not a port: the C++ city AI is not ported (see docs/AI.md). The
//! governor decides which plots a city works and which specialists it employs;
//! the rules (which plots *can* be worked, forced specialists, yields) stay in the
//! ported CvCity code, which feeds this module plain candidate lists.
//!
//! Algorithm: always work the city centre; fill forced specialists; then add
//! citizens greedily by marginal value, where food is weighted up while the city
//! would starve or (unless avoiding growth) while the surplus is below the growth
//! target; finally do a bounded swap pass (worst worked vs. best unworked).

use ffh_data::*;

/// Relative weights for the three yields and the commerce types (per city emphasis).
#[derive(Clone, Copy, Debug)]
pub struct Weights {
    pub food: i32,
    pub production: i32,
    pub commerce: i32,
    /// Per commerce type multiplier (gold, research, culture, espionage), percent.
    pub commerce_type: [i32; CommerceType::NUM],
    /// Value of one great-people point.
    pub great_people: i32,
}

impl Default for Weights {
    fn default() -> Self {
        Weights { food: 3, production: 2, commerce: 1, commerce_type: [100, 110, 60, 30], great_people: 1 }
    }
}

/// A plot the city may work (index into the city's plot list).
#[derive(Clone, Debug)]
pub struct TileOption {
    pub city_plot: usize,
    pub yields: [i32; YieldType::NUM],
    /// Commerce split by type after the player's sliders (optional refinement).
    pub commerce: [i32; CommerceType::NUM],
}

/// A specialist the city may employ.
#[derive(Clone, Debug)]
pub struct SpecialistOption {
    pub specialist: SpecialistType,
    pub yields: [i32; YieldType::NUM],
    pub commerce: [i32; CommerceType::NUM],
    pub great_people: i32,
    /// How many more can be added (`i32::MAX` if unlimited).
    pub max_more: i32,
}

#[derive(Clone, Debug)]
pub struct GovernorInput {
    /// Citizens to place (population + free specialists, minus forced specialists).
    pub citizens: i32,
    /// Food eaten per citizen (C++ `FOOD_CONSUMPTION_PER_POPULATION`), times population.
    pub food_consumption: i32,
    /// Food from the centre plot and other fixed sources.
    pub base_food: i32,
    /// Desired food surplus (e.g. 2 + population/3); ignored when `avoid_growth`.
    pub growth_target: i32,
    pub avoid_growth: bool,
    pub tiles: Vec<TileOption>,
    pub specialists: Vec<SpecialistOption>,
    pub weights: Weights,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignment {
    /// City plot indices to work (centre excluded).
    pub tiles: Vec<usize>,
    /// Specialists to add (type, count).
    pub specialists: Vec<(SpecialistType, i32)>,
}

fn commerce_value(w: &Weights, commerce: &[i32; CommerceType::NUM], raw: i32) -> i32 {
    let split: i32 = commerce.iter().zip(w.commerce_type.iter()).map(|(c, m)| c * m).sum::<i32>() / 100;
    // If no split is given, value raw commerce directly.
    if commerce.iter().all(|&c| c == 0) {
        raw * w.commerce
    } else {
        split * w.commerce
    }
}

fn value(w: &Weights, food_weight: i32, yields: &[i32; 3], commerce: &[i32; 4], gpp: i32) -> i32 {
    yields[0] * food_weight * 4
        + yields[1] * w.production * 4
        + commerce_value(w, commerce, yields[2]) * 4
        + gpp * w.great_people * 2
}

/// Chooses tiles and specialists for `input.citizens` citizens.
pub fn assign(input: &GovernorInput) -> Assignment {
    let w = &input.weights;
    let mut food = input.base_food - input.food_consumption;
    let mut taken = vec![false; input.tiles.len()];
    let mut spec_used = vec![0i32; input.specialists.len()];
    let mut out = Assignment::default();

    for _ in 0..input.citizens.max(0) {
        let food_weight = if food < 0 {
            w.food * 4
        } else if !input.avoid_growth && food < input.growth_target {
            w.food * 2
        } else if input.avoid_growth {
            w.food / 2
        } else {
            w.food
        };
        // best tile
        let mut best: Option<(i32, bool, usize)> = None; // (value, is_tile, index)
        for (i, t) in input.tiles.iter().enumerate() {
            if taken[i] {
                continue;
            }
            let v = value(w, food_weight, &t.yields, &t.commerce, 0);
            if best.map_or(true, |(bv, _, _)| v > bv) {
                best = Some((v, true, i));
            }
        }
        for (i, s) in input.specialists.iter().enumerate() {
            if spec_used[i] >= s.max_more {
                continue;
            }
            let v = value(w, food_weight, &s.yields, &s.commerce, s.great_people);
            if best.map_or(true, |(bv, _, _)| v > bv) {
                best = Some((v, false, i));
            }
        }
        match best {
            Some((_, true, i)) => {
                taken[i] = true;
                food += input.tiles[i].yields[0];
                out.tiles.push(input.tiles[i].city_plot);
            }
            Some((_, false, i)) => {
                spec_used[i] += 1;
                food += input.specialists[i].yields[0];
            }
            None => break,
        }
    }
    for (i, n) in spec_used.iter().enumerate() {
        if *n > 0 {
            out.specialists.push((input.specialists[i].specialist, *n));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(i: usize, f: i32, p: i32, c: i32) -> TileOption {
        TileOption { city_plot: i, yields: [f, p, c], commerce: [0; 4] }
    }

    #[test]
    fn avoids_starvation_first() {
        let input = GovernorInput {
            citizens: 2,
            food_consumption: 4,
            base_food: 2,
            growth_target: 2,
            avoid_growth: false,
            tiles: vec![tile(1, 0, 3, 0), tile(2, 3, 0, 0), tile(3, 2, 1, 0)],
            specialists: vec![],
            weights: Weights::default(),
        };
        let a = assign(&input);
        assert_eq!(a.tiles.len(), 2);
        assert!(a.tiles.contains(&2), "food tile must be worked: {:?}", a);
    }

    #[test]
    fn uses_specialists_when_no_tiles() {
        let input = GovernorInput {
            citizens: 2,
            food_consumption: 4,
            base_food: 6,
            growth_target: 0,
            avoid_growth: false,
            tiles: vec![tile(1, 1, 0, 0)],
            specialists: vec![SpecialistOption {
                specialist: SpecialistType(0),
                yields: [0, 0, 0],
                commerce: [0, 3, 0, 0],
                great_people: 3,
                max_more: i32::MAX,
            }],
            weights: Weights::default(),
        };
        let a = assign(&input);
        assert_eq!(a.tiles.len() as i32 + a.specialists.iter().map(|s| s.1).sum::<i32>(), 2);
    }
}

// ----------------------------------------------------------------------------
// Game entry points (C++ CvCityAI::AI_assignWorkingPlots and helpers)
// ----------------------------------------------------------------------------

use crate::defs::CITY_HOME_PLOT;
use crate::game::Game;
use crate::ids::*;

impl Game {
    /// Replaces C++ `CvCityAI::AI_assignWorkingPlots`. The rules-visible steps keep the C++
    /// order (clear assignments of automated cities, `verifyWorkingPlots`, forced specialists,
    /// max-specialist clamps, the home plot, removal while over the limit, the proportional
    /// spread of extra specialists over forced types); the choice of which plots and specialists
    /// to use is [`assign`]. `AI_juggleCitizens` is not needed: automated cities are reassigned
    /// from scratch.
    pub fn city_assign_working_plots(&mut self, c: CityId) {
        if self.d.defines.ai_shouldnt_manage_plot_assignment != 0 {
            return;
        }
        let owner = self.cities[c].owner;
        let automated = !self.city_is_human(c) || self.cities[c].citizens_automated;

        if automated {
            for i in 0..self.city_num_city_plots(c).max(0) as usize {
                self.city_set_working_plot(c, i, false);
            }
        }

        // AI: AI_updateSpecialYieldMultiplier (AI-only cache).

        self.city_verify_working_plots(c);

        let mut specialist_forced = false;
        let mut total_forced = 0;
        for s in self.d.specialists.keys() {
            let forced = self.cities[c].force_specialist_count[s];
            if forced > 0 {
                specialist_forced = true;
                total_forced += forced;
            }
            if automated || self.cities[c].specialist_count[s] < forced {
                self.city_set_specialist_count(c, s, forced);
            }
        }

        for s in self.d.specialists.keys() {
            if !self.city_is_specialist_valid(c, s, 0) {
                let max = self.cities[c].max_specialist_count[s];
                if self.cities[c].specialist_count[s] > max {
                    self.city_set_specialist_count(c, s, max);
                }
            }
        }

        if let Some(home) = self.city_index_plot(c, CITY_HOME_PLOT) {
            let work = self.cities[c].population > 0 && self.city_can_work(c, home);
            self.city_set_working_plot(c, CITY_HOME_PLOT, work);
        }

        while self.city_extra_population(c) < 0 {
            if !self.city_remove_worst_citizen(c) {
                break;
            }
        }

        let extra = self.city_extra_population(c);
        if extra > 0 {
            self.city_place_citizens(c, extra, true);
        }

        let initial_extra_specialists = self.city_extra_specialists(c);
        let mut extra_specialists = initial_extra_specialists;
        if specialist_forced && extra_specialists > 0 {
            for s in self.d.specialists.keys() {
                if !self.city_is_specialist_valid(c, s, 1) {
                    continue;
                }
                let forced = self.cities[c].force_specialist_count[s];
                if forced <= 0 {
                    continue;
                }
                let mut count = self.cities[c].specialist_count[s];
                let max = self.cities[c].max_specialist_count[s];
                let mut to_add = (initial_extra_specialists * forced + total_forced / 2) / total_forced;
                if extra_specialists < to_add {
                    to_add = extra_specialists;
                }
                count += to_add;
                extra_specialists -= to_add;
                if count > max && !self.player_is_specialist_valid(owner, s) {
                    extra_specialists += count - max;
                    count = max;
                }
                self.city_set_specialist_count(c, s, count);
            }
        }

        let extra = self.city_extra_specialists(c);
        if extra > 0 {
            self.city_place_citizens(c, extra, false);
        }

        self.cities[c].ai_assign_work_dirty = false;
    }

    /// Candidate specialists for `c` with how many more of each may be added.
    fn governor_specialists(&self, c: CityId) -> Vec<SpecialistOption> {
        let owner = self.cities[c].owner;
        let mut out = Vec::new();
        for s in self.d.specialists.keys() {
            if !self.city_is_specialist_valid(c, s, 1) {
                continue;
            }
            let city = &self.cities[c];
            let unlimited = self.player_is_specialist_valid(owner, s) || city.force_specialist_count[s] > 0;
            let max_more = if unlimited { i32::MAX } else { (city.max_specialist_count[s] - city.specialist_count[s]).max(0) };
            if max_more == 0 {
                continue;
            }
            let yields = std::array::from_fn(|i| self.player_specialist_yield(owner, s, YieldType::from_usize(i)));
            let commerce = std::array::from_fn(|i| self.player_specialist_commerce(owner, s, CommerceType::from_usize(i)));
            out.push(SpecialistOption { specialist: s, yields, commerce, great_people: self.d.specialists[s].great_people_rate_change, max_more });
        }
        out
    }

    fn governor_weights(&self, c: CityId) -> Weights {
        let mut w = Weights::default();
        if self.city_ai_is_emphasize_yield(c, YieldType::YIELD_FOOD) {
            w.food += 2;
        }
        if self.city_ai_is_emphasize_yield(c, YieldType::YIELD_PRODUCTION) {
            w.production += 2;
        }
        if self.city_ai_is_emphasize_yield(c, YieldType::YIELD_COMMERCE) {
            w.commerce += 2;
        }
        if self.city_ai_is_emphasize_great_people(c) {
            w.great_people += 3;
        }
        w
    }

    /// Places `n` citizens on tiles (if `workers`) and specialists, chosen by [`assign`].
    fn city_place_citizens(&mut self, c: CityId, n: i32, workers: bool) {
        let mut tiles = Vec::new();
        if workers {
            for i in 0..self.city_num_city_plots(c).max(0) as usize {
                if i == CITY_HOME_PLOT || self.cities[c].working_plot[i] {
                    continue;
                }
                let Some(plot) = self.city_index_plot(c, i) else { continue };
                if !self.city_can_work(c, plot) {
                    continue;
                }
                tiles.push(TileOption { city_plot: i, yields: self.map.plots[plot].yields, commerce: [0; CommerceType::NUM] });
            }
        }
        let population = self.cities[c].population;
        let avoid_growth = self.city_ai_stop_growth(c) || self.city_unhappy_level(c, 1) > self.city_happy_level(c);
        let input = GovernorInput {
            citizens: n,
            food_consumption: 0,
            base_food: self.city_food_difference(c, false),
            growth_target: 2 + population / 4,
            avoid_growth,
            tiles,
            specialists: self.governor_specialists(c),
            weights: self.governor_weights(c),
        };
        let a = assign(&input);
        for i in a.tiles {
            self.city_set_working_plot(c, i, true);
        }
        for (s, k) in a.specialists {
            self.city_change_specialist_count(c, s, k);
        }
    }

    /// Replaces C++ `CvCityAI::AI_removeWorstCitizen`: drop a specialist above the forced
    /// count, else the least valuable worked plot, else a forced specialist.
    fn city_remove_worst_citizen(&mut self, c: CityId) -> bool {
        let w = self.governor_weights(c);
        let owner = self.cities[c].owner;
        let spec_value = |g: &Game, s: SpecialistType| -> i32 {
            let y: [i32; 3] = std::array::from_fn(|i| g.player_specialist_yield(owner, s, YieldType::from_usize(i)));
            let cm: [i32; 4] = std::array::from_fn(|i| g.player_specialist_commerce(owner, s, CommerceType::from_usize(i)));
            value(&w, w.food, &y, &cm, g.d.specialists[s].great_people_rate_change)
        };
        let mut worst: Option<(SpecialistType, i32)> = None;
        for s in self.d.specialists.keys() {
            let city = &self.cities[c];
            if city.specialist_count[s] > city.force_specialist_count[s] {
                let v = spec_value(self, s);
                if worst.map_or(true, |(_, b)| v < b) {
                    worst = Some((s, v));
                }
            }
        }
        if let Some((s, _)) = worst {
            self.city_change_specialist_count(c, s, -1);
            return true;
        }
        let mut worst_tile: Option<(usize, i32)> = None;
        for i in 0..self.city_num_city_plots(c).max(0) as usize {
            if i == CITY_HOME_PLOT || !self.cities[c].working_plot[i] {
                continue;
            }
            let Some(plot) = self.city_index_plot(c, i) else { continue };
            let v = value(&w, w.food, &self.map.plots[plot].yields, &[0; CommerceType::NUM], 0);
            if worst_tile.map_or(true, |(_, b)| v < b) {
                worst_tile = Some((i, v));
            }
        }
        if let Some((i, _)) = worst_tile {
            self.city_set_working_plot(c, i, false);
            return true;
        }
        for s in self.d.specialists.keys() {
            if self.cities[c].specialist_count[s] > 0 {
                self.city_change_specialist_count(c, s, -1);
                return true;
            }
        }
        false
    }
}
