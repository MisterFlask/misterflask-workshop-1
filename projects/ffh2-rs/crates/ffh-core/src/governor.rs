//! City citizen governor (replaces C++ `CvCityAI::AI_assignWorkingPlots`,
//! `AI_addBestCitizen`, `AI_removeWorstCitizen`, `AI_juggleCitizens`).
//!
//! New code, not a port: the C++ city AI is not ported (see docs/AI.md). The
//! governor decides which plots a city works and which specialists it employs;
//! the rules (which plots *can* be worked, forced specialists, yields) stay in the
//! ported CvCity code, which feeds this module plain candidate lists.
//!
//! Valuation (after C++ `CvCityAI::AI_yieldValue`): a hammer is worth 15, a point of
//! commerce about 7 (split by the owner's sliders, research weighted up), a great-people
//! point 3. Food is valued by the city's food *surplus*, as a concave function: food that
//! avoids starvation is worth 60 per point; while the city wants to grow (it has happy
//! faces to spare and good tiles to work) food up to a growth target (6 at size 1, falling
//! to 2 at size 5) is worth 23-38 depending on how much room there is, food past the target
//! a third of that; a city avoiding growth values extra food at 2, a city at its happy cap
//! at -3 (growing would only add an angry citizen). While the city builds a food-production unit (settlers,
//! workers) its surplus becomes hammers, so food is valued like production.
//!
//! Algorithm: always work the city centre; fill forced specialists; then add citizens
//! greedily by marginal value (recomputed against the running food surplus); finally a
//! bounded swap pass (any worked tile or specialist against any unworked one) improves the
//! total, which the greedy order can miss because of the food curve.

use ffh_data::*;

/// Relative weights for the three yields and the commerce types (per city emphasis).
#[derive(Clone, Copy, Debug)]
pub struct Weights {
    /// Food value multiplier, percent.
    pub food: i32,
    /// Value of one hammer.
    pub production: i32,
    /// Value of one point of (unsplit) commerce.
    pub commerce: i32,
    /// Per commerce type value of one point (gold, research, culture, espionage), percent of
    /// `commerce`.
    pub commerce_type: [i32; CommerceType::NUM],
    /// Value of one great-people point.
    pub great_people: i32,
}

impl Default for Weights {
    fn default() -> Self {
        Weights { food: 100, production: 15, commerce: 9, commerce_type: [100, 115, 60, 20], great_people: 3 }
    }
}

/// What the city wants from food, computed from its state.
#[derive(Clone, Copy, Debug)]
pub struct FoodContext {
    pub population: i32,
    /// Citizens the city may still add without angry citizens and with useful work for them.
    pub growth_room: i32,
    /// Good health minus bad health.
    pub health_room: i32,
    /// Building a unit whose cost is paid with food too (settlers, workers): the surplus is
    /// production.
    pub food_is_production: bool,
    /// Do not grow (emphasize avoid growth, disorder).
    pub avoid_growth: bool,
    /// The owner does not use food (FfH `isIgnoreFood`): no growth, no starvation.
    pub ignore_food: bool,
    /// Surplus beyond which more food is worth less.
    pub growth_target: i32,
    /// Hammers below this are worth double (a city must build something; after BBAI's
    /// "more than enough food but very little production" rule).
    pub production_floor: i32,
}

impl Default for FoodContext {
    fn default() -> Self {
        FoodContext {
            population: 1,
            growth_room: 3,
            health_room: 3,
            food_is_production: false,
            avoid_growth: false,
            ignore_food: false,
            growth_target: 4,
            production_floor: 0,
        }
    }
}

/// A plot the city may work (index into the city's plot list).
#[derive(Clone, Debug)]
pub struct TileOption {
    pub city_plot: usize,
    pub yields: [i32; YieldType::NUM],
    /// Commerce split by type (optional refinement; zeros = use the raw commerce yield).
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
    /// Citizens to place.
    pub citizens: i32,
    /// Food surplus before placing them (current yield minus consumption).
    pub base_surplus: i32,
    /// Hammers before placing them.
    pub base_production: i32,
    pub food: FoodContext,
    pub tiles: Vec<TileOption>,
    pub specialists: Vec<SpecialistOption>,
    pub weights: Weights,
    /// Specialists may be used (false when only tiles may be worked).
    pub allow_specialists: bool,
    /// Tiles may be worked (false for free specialists).
    pub allow_tiles: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignment {
    /// City plot indices to work (centre excluded).
    pub tiles: Vec<usize>,
    /// Specialists to add (type, count).
    pub specialists: Vec<(SpecialistType, i32)>,
}

/// Value of the step from surplus `s` to `s + 1`.
fn food_step_value(w: &Weights, f: &FoodContext, s: i32) -> i32 {
    if f.ignore_food {
        return if f.food_is_production { w.production } else { 0 };
    }
    if s < 0 {
        return 60;
    }
    if f.food_is_production {
        return w.production;
    }
    if f.avoid_growth {
        return 2;
    }
    if f.growth_room <= 0 {
        // At the happy cap a new citizen would be angry (eats, does not work): a surplus only
        // grows the city into disorder, so it is slightly unwelcome.
        return -3;
    }
    let mut g = 20 + 3 * f.growth_room.min(6);
    if f.health_room <= 0 {
        g -= 6;
    }
    g = g * w.food / 100;
    // A city that can grow should: the first two points of surplus (a city at +0 never grows)
    // count double.
    if s < 2 {
        g * 2
    } else if s < f.growth_target {
        g
    } else {
        g / 3
    }
}

/// Value of changing the surplus from `s` by `delta` (negative = losing food).
pub fn food_value(w: &Weights, f: &FoodContext, s: i32, delta: i32) -> i32 {
    let mut v = 0;
    if delta >= 0 {
        for k in 0..delta {
            v += food_step_value(w, f, s + k);
        }
    } else {
        for k in 1..=-delta {
            v -= food_step_value(w, f, s - k);
        }
    }
    v
}

/// Value of adding `delta` hammers to `total`.
pub fn production_value(w: &Weights, f: &FoodContext, total: i32, delta: i32) -> i32 {
    let mut v = delta * w.production;
    // The part below the floor counts twice.
    let below = (f.production_floor - total).clamp(0, delta.max(0));
    v += below * w.production;
    v
}

/// Value of everything but food in an item.
pub fn non_food_value(w: &Weights, yields: &[i32; YieldType::NUM], commerce: &[i32; CommerceType::NUM], great_people: i32) -> i32 {
    let mut v = yields[YieldType::YIELD_PRODUCTION.idx()] * w.production;
    if commerce.iter().all(|&c| c == 0) {
        v += yields[YieldType::YIELD_COMMERCE.idx()] * w.commerce;
    } else {
        v += commerce.iter().zip(w.commerce_type.iter()).map(|(c, m)| c * m).sum::<i32>() * w.commerce / 100;
    }
    v + great_people * w.great_people
}

/// One candidate item (tile or one specialist slot).
#[derive(Clone, Copy)]
struct Item {
    food: i32,
    prod: i32,
    /// Value of everything but food and hammers.
    rest: i32,
    /// Index into tiles (`is_tile`) or specialists.
    index: usize,
    is_tile: bool,
}

/// Chooses tiles and specialists for `input.citizens` citizens.
pub fn assign(input: &GovernorInput) -> Assignment {
    let w = &input.weights;
    let f = &input.food;
    let mut items: Vec<Item> = Vec::new();
    if input.allow_tiles {
        for (i, t) in input.tiles.iter().enumerate() {
            items.push(Item { food: t.yields[0], prod: t.yields[1], rest: non_food_value(w, &t.yields, &t.commerce, 0) - t.yields[1] * w.production, index: i, is_tile: true });
        }
    }
    let mut spec_items_start = items.len();
    if input.allow_specialists {
        spec_items_start = items.len();
        let n = input.citizens.max(0);
        for (i, s) in input.specialists.iter().enumerate() {
            // One item per slot this assignment could use.
            let slots = s.max_more.min(n);
            for _ in 0..slots {
                items.push(Item { food: s.yields[0], prod: s.yields[1], rest: non_food_value(w, &s.yields, &s.commerce, s.great_people) - s.yields[1] * w.production, index: i, is_tile: false });
            }
        }
    }
    let _ = spec_items_start;

    let mut used = vec![false; items.len()];
    let mut surplus = input.base_surplus;
    let mut production = input.base_production;
    let value = |it: &Item, surplus: i32, production: i32| it.rest + food_value(w, f, surplus, it.food) + production_value(w, f, production, it.prod);
    let mut chosen: Vec<usize> = Vec::new();
    for _ in 0..input.citizens.max(0) {
        let mut best: Option<(i32, usize)> = None;
        for (k, it) in items.iter().enumerate() {
            if used[k] {
                continue;
            }
            let v = value(it, surplus, production);
            // Ties: first candidate (tiles before specialists, lower plot index first).
            if best.map_or(true, |(bv, _)| v > bv) {
                best = Some((v, k));
            }
        }
        let Some((_, k)) = best else { break };
        used[k] = true;
        surplus += items[k].food;
        production += items[k].prod;
        chosen.push(k);
    }

    // Swap pass: replace a chosen item by an unused one when the total improves.
    for _ in 0..4 {
        let mut improved = false;
        for ci in 0..chosen.len() {
            let a = chosen[ci];
            let without = surplus - items[a].food;
            let without_prod = production - items[a].prod;
            let base = value(&items[a], without, without_prod);
            let mut best: Option<(i32, usize)> = None;
            for (k, it) in items.iter().enumerate() {
                if used[k] {
                    continue;
                }
                let v = value(it, without, without_prod);
                if v > base && best.map_or(true, |(bv, _)| v > bv) {
                    best = Some((v, k));
                }
            }
            if let Some((_, k)) = best {
                used[a] = false;
                used[k] = true;
                surplus = without + items[k].food;
                production = without_prod + items[k].prod;
                chosen[ci] = k;
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }

    let mut out = Assignment::default();
    let mut spec_count = vec![0i32; input.specialists.len()];
    for &k in &chosen {
        let it = items[k];
        if it.is_tile {
            out.tiles.push(input.tiles[it.index].city_plot);
        } else {
            spec_count[it.index] += 1;
        }
    }
    out.tiles.sort_unstable();
    for (i, n) in spec_count.iter().enumerate() {
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

    fn input(citizens: i32, base_surplus: i32, tiles: Vec<TileOption>) -> GovernorInput {
        GovernorInput {
            citizens,
            base_surplus,
            base_production: 1,
            food: FoodContext::default(),
            tiles,
            specialists: vec![],
            weights: Weights::default(),
            allow_specialists: true,
            allow_tiles: true,
        }
    }

    #[test]
    fn avoids_starvation_first() {
        // Two citizens eat 4; the centre gives 2.
        let a = assign(&input(2, -2, vec![tile(1, 0, 3, 0), tile(2, 3, 0, 0), tile(3, 2, 1, 0)]));
        assert_eq!(a.tiles.len(), 2);
        assert!(a.tiles.contains(&2), "food tile must be worked: {:?}", a);
    }

    #[test]
    fn uses_specialists_when_no_tiles() {
        let mut inp = input(2, 2, vec![tile(1, 1, 0, 0)]);
        inp.specialists = vec![SpecialistOption {
            specialist: SpecialistType(0),
            yields: [0, 0, 0],
            commerce: [0, 3, 0, 0],
            great_people: 3,
            max_more: i32::MAX,
        }];
        let a = assign(&inp);
        assert_eq!(a.tiles.len() as i32 + a.specialists.iter().map(|s| s.1).sum::<i32>(), 2);
    }

    #[test]
    fn growing_city_prefers_food_then_production() {
        // Size 1 with room to grow: the 4-food tile beats the 0/3/0 mine.
        let a = assign(&input(1, 1, vec![tile(1, 0, 3, 0), tile(2, 4, 0, 0)]));
        assert_eq!(a.tiles, vec![2]);
        // At the happy cap, the mine wins.
        let mut inp = input(1, 1, vec![tile(1, 0, 3, 0), tile(2, 4, 0, 0)]);
        inp.food.growth_room = 0;
        assert_eq!(assign(&inp).tiles, vec![1]);
    }

    #[test]
    fn food_production_counts_food_as_hammers() {
        // Building a settler: 3/1/0 (4 total) beats 0/3/1.
        let mut inp = input(1, 0, vec![tile(1, 0, 3, 1), tile(2, 3, 1, 0)]);
        inp.food.food_is_production = true;
        assert_eq!(assign(&inp).tiles, vec![2]);
    }

    #[test]
    fn swap_pass_fixes_greedy_order() {
        // Surplus -1: greedy takes the 2-food tile first to stop starving, then the hammers.
        let a = assign(&input(2, -1, vec![tile(1, 1, 0, 0), tile(2, 2, 0, 0), tile(3, 0, 4, 0)]));
        assert_eq!(a.tiles, vec![2, 3]);
    }
}

// ----------------------------------------------------------------------------
// Game entry points (C++ CvCityAI::AI_assignWorkingPlots and helpers)
// ----------------------------------------------------------------------------

use crate::defs::{CITY_HOME_PLOT, MAX_INT, NUM_CITY_PLOTS};
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

    /// The governor's weights for city `c`: the default values, commerce valued by the owner's
    /// sliders (research a little above gold) and by its shortfall of gold, plus the player-set
    /// emphasize flags.
    fn governor_weights(&self, c: CityId) -> Weights {
        let mut w = Weights::default();
        let owner = self.cities[c].owner;
        let pct = self.players[owner].commerce_percent;
        // Raw commerce is split by the sliders: its value is the slider-weighted type value.
        let blended: i32 = (0..CommerceType::NUM).map(|k| pct[k].max(0) * w.commerce_type[k]).sum::<i32>() / 100;
        if blended > 0 {
            w.commerce = w.commerce * blended / 100;
        }
        // An owner short of gold needs commerce, up to twice as much: by how far the research
        // slider has been pushed down to pay the bills, or by the measured shortfall (costs that
        // eat the commerce before the slider has fallen).
        let research_pct = pct[CommerceType::COMMERCE_RESEARCH.idx()];
        if self.player_is_commerce_flexible(owner, CommerceType::COMMERCE_RESEARCH) {
            let boost = (100 - 2 * research_pct).max(0).max(self.governor_shortfall_percent(owner));
            w.commerce = w.commerce * (100 + boost) / 100;
        }
        if self.city_ai_is_emphasize_yield(c, YieldType::YIELD_FOOD) {
            w.food = w.food * 160 / 100;
        }
        if self.city_ai_is_emphasize_yield(c, YieldType::YIELD_PRODUCTION) {
            w.production = w.production * 150 / 100;
        }
        if self.city_ai_is_emphasize_yield(c, YieldType::YIELD_COMMERCE) {
            w.commerce = w.commerce * 160 / 100;
        }
        if self.city_ai_is_emphasize_great_people(c) {
            w.great_people *= 4;
        }
        // A culture-starved new city (no border growth yet) values culture more.
        if self.cities[c].culture_level.map_or(true, |l| l.i() <= 1) {
            w.commerce_type[CommerceType::COMMERCE_CULTURE.idx()] = 100;
        }
        w
    }

    /// How short of gold the owner is (0-100): the gold it would lack with research at 60%
    /// (costs, inflated, against the 40% of its gold + research commerce left for gold), three
    /// times as a percent of that commerce.
    fn governor_shortfall_percent(&self, p: PlayerId) -> i32 {
        let commerce = self.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD) + self.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH);
        if commerce <= 0 {
            return 0;
        }
        let short = self.player_calculate_inflated_costs(p) - self.players[p].gold_per_turn - commerce * 40 / 100;
        (short * 300 / commerce).clamp(0, 100)
    }

    /// What city `c` wants from food (see [`FoodContext`]). `placing` citizens are about to be
    /// placed; growth room is limited by the good tiles left for them.
    fn governor_food_context(&self, c: CityId, placing: i32) -> FoodContext {
        let city = &self.cities[c];
        let owner = city.owner;
        let population = city.population;
        let happy_room = if self.city_is_no_unhappiness(c) { 5 } else { self.city_happy_level(c) - self.city_unhappy_level(c, 0) };
        let health_room = self.city_good_health(c) - self.city_bad_health(c, false, 0);
        // Good tiles still unworked (food to sustain themselves, or a strong yield).
        let mut good_tiles = 0;
        for i in 0..self.city_num_city_plots(c).max(0) as usize {
            if i == CITY_HOME_PLOT || city.working_plot[i] {
                continue;
            }
            let Some(plot) = self.city_index_plot(c, i) else { continue };
            if !self.city_can_work(c, plot) {
                continue;
            }
            let y = self.map.plots[plot].yields;
            if y[0] >= 2 && y[0] + y[1] + y[2] >= 3 || y[0] + y[1] + y[2] >= 4 {
                good_tiles += 1;
            }
        }
        let tile_room = (good_tiles - placing + 1).max(1);
        let growth_room = happy_room.min(tile_room);
        let avoid_growth = self.city_ai_is_emphasize_avoid_growth(c)
            || self.city_is_disorder(c)
            || (self.city_ai_is_emphasize_avoid_angry_citizens(c) && happy_room <= 0)
            || (self.city_ai_is_emphasize_avoid_unhealthy_citizens(c) && health_room <= 0);
        FoodContext {
            population,
            growth_room,
            health_room,
            food_is_production: self.city_is_food_production(c),
            avoid_growth,
            ignore_food: self.players[owner].ignore_food,
            growth_target: 2 + (5 - population).max(0) + i32::from(growth_room >= 4 && population >= 5),
            production_floor: if self.city_is_production_process(c) { 0 } else { 2 + population / 2 },
        }
    }

    /// Current food surplus of `c` from what it works now (production-food included).
    fn governor_surplus(&self, c: CityId) -> i32 {
        self.city_yield_rate(c, YieldType::YIELD_FOOD) - self.city_food_consumption(c, false, 0)
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
        let input = GovernorInput {
            citizens: n,
            base_surplus: self.governor_surplus(c),
            base_production: self.cities[c].base_yield_rate[YieldType::YIELD_PRODUCTION.idx()],
            food: self.governor_food_context(c, n),
            tiles,
            specialists: self.governor_specialists(c),
            weights: self.governor_weights(c),
            allow_specialists: true,
            allow_tiles: workers,
        };
        let a = assign(&input);
        for i in a.tiles {
            self.city_set_working_plot(c, i, true);
        }
        for (s, k) in a.specialists {
            self.city_change_specialist_count(c, s, k);
        }
    }

    /// Value lost by removing one citizen with these outputs at the current surplus.
    fn governor_item_value(&self, w: &Weights, f: &FoodContext, surplus: i32, yields: &[i32; YieldType::NUM], commerce: &[i32; CommerceType::NUM], gpp: i32) -> i32 {
        non_food_value(w, yields, commerce, gpp) + food_value(w, f, surplus - yields[0], yields[0])
    }

    /// Replaces C++ `CvCityAI::AI_removeWorstCitizen`: drop a specialist above the forced
    /// count, else the least valuable worked plot, else a forced specialist.
    fn city_remove_worst_citizen(&mut self, c: CityId) -> bool {
        let w = self.governor_weights(c);
        let f = self.governor_food_context(c, 0);
        let surplus = self.governor_surplus(c);
        let owner = self.cities[c].owner;
        let mut worst: Option<(SpecialistType, i32)> = None;
        for s in self.d.specialists.keys() {
            let city = &self.cities[c];
            if city.specialist_count[s] > city.force_specialist_count[s] {
                let y: [i32; 3] = std::array::from_fn(|i| self.player_specialist_yield(owner, s, YieldType::from_usize(i)));
                let cm: [i32; 4] = std::array::from_fn(|i| self.player_specialist_commerce(owner, s, CommerceType::from_usize(i)));
                let v = self.governor_item_value(&w, &f, surplus, &y, &cm, self.d.specialists[s].great_people_rate_change);
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
            let v = self.governor_item_value(&w, &f, surplus, &self.map.plots[plot].yields, &[0; CommerceType::NUM], 0);
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

    /// Stand-in for C++ `CvCityAI::AI_addBestCitizen(bWorkers, bSpecialists)`
    /// (CvCityAI.cpp:6733-6879): adds one citizen to the best workable plot (`workers`) or as
    /// the best valid specialist (`specialists`); false if there is no candidate.
    pub(crate) fn city_ai_add_best_citizen(&mut self, c: CityId, workers: bool, specialists: bool) -> bool {
        let w = self.city_citizen_weights(c);
        let f = self.governor_food_context(c, 1);
        let surplus = self.governor_surplus(c);

        // AI: the C++ first prefers a forced specialist by its forced/total ratio; the player's
        // buttons only reach here for cities that are not automated, which force none.
        let mut best_specialist_value = 0;
        let mut best_specialist = None;
        if specialists {
            for specialist in self.d.specialists.keys() {
                if self.city_is_specialist_valid(c, specialist, 1) {
                    let food = self.player_specialist_yield(self.cities[c].owner, specialist, YieldType::YIELD_FOOD);
                    let value = (self.city_specialist_citizen_value(c, &w, specialist) - food * 20 + food_value(&w, &f, surplus, food)).max(0);
                    if value >= best_specialist_value {
                        best_specialist_value = value;
                        best_specialist = Some(specialist);
                    }
                }
            }
        }

        let mut best_plot_value = 0;
        let mut best_plot = None;
        if workers {
            for i in 0..NUM_CITY_PLOTS {
                if i == CITY_HOME_PLOT || self.cities[c].working_plot[i] {
                    continue;
                }
                if let Some(loop_plot) = self.city_index_plot(c, i) {
                    if self.city_can_work(c, loop_plot) {
                        let y = self.map.plots[loop_plot].yields;
                        let value = (non_food_value(&w, &y, &[0; CommerceType::NUM], 0) + food_value(&w, &f, surplus, y[0])).max(1);
                        if value > best_plot_value {
                            best_plot_value = value;
                            best_plot = Some(i);
                        }
                    }
                }
            }
        }

        // if the best plot value is better than the best specialist, do not work the specialist
        if best_plot.is_some() && best_plot_value > best_specialist_value {
            best_specialist = None;
        }

        if let Some(specialist) = best_specialist {
            self.city_change_specialist_count(c, specialist, 1);
            true
        } else if let Some(i) = best_plot {
            self.city_set_working_plot(c, i, true);
            true
        } else {
            false
        }
    }

    /// Stand-in for C++ `CvCityAI::AI_removeWorstCitizen(eIgnoreSpecialist)`
    /// (CvCityAI.cpp:6882-7002): frees one citizen (an extra default specialist, else the
    /// worst specialist that uses population or worked plot, else any specialist); false if
    /// none could be removed.
    pub(crate) fn city_ai_remove_worst_citizen(&mut self, c: CityId, ignore_specialist: Option<SpecialistType>) -> bool {
        let default_specialist = opt_from_i32::<SpecialistType>(self.d.defines.default_specialist);

        // if we are using more specialists than the free ones we get
        if self.city_extra_free_specialists(c) < 0 {
            // does generic 'citizen' specialist exist? is ignore something other than it?
            if let Some(default) = default_specialist.filter(|&s| Some(s) != ignore_specialist) {
                // do we have at least one more generic citizen than we are forcing?
                let city = &self.cities[c];
                if city.specialist_count[default] > city.force_specialist_count[default] {
                    // remove the extra generic citizen
                    self.city_change_specialist_count(c, default, -1);
                    return true;
                }
            }
        }

        let w = self.city_citizen_weights(c);
        let mut worst_value = MAX_INT;
        let mut worst_specialist = None;
        let mut worst_plot = None;

        // if we are using more specialists than the free ones we get
        if self.city_extra_free_specialists(c) < 0 {
            for specialist in self.d.specialists.keys() {
                let city = &self.cities[c];
                if Some(specialist) != ignore_specialist && city.specialist_count[specialist] > city.force_specialist_count[specialist] {
                    let value = self.city_specialist_citizen_value(c, &w, specialist);
                    if value < worst_value {
                        worst_value = value;
                        worst_specialist = Some(specialist);
                        worst_plot = None;
                    }
                }
            }
        }

        // check all the plots we are working
        for i in 0..NUM_CITY_PLOTS {
            if i == CITY_HOME_PLOT || !self.cities[c].working_plot[i] {
                continue;
            }
            if let Some(loop_plot) = self.city_index_plot(c, i) {
                let value = citizen_value(&w, &self.map.plots[loop_plot].yields, &[0; CommerceType::NUM], 0);
                if value < worst_value {
                    worst_value = value;
                    worst_specialist = None;
                    worst_plot = Some(i);
                }
            }
        }

        if let Some(specialist) = worst_specialist {
            self.city_change_specialist_count(c, specialist, -1);
            return true;
        } else if let Some(i) = worst_plot {
            self.city_set_working_plot(c, i, false);
            return true;
        }

        // if we still have not removed one, then try again, but do not ignore the one we were
        // told to ignore
        if self.city_extra_free_specialists(c) < 0 {
            for specialist in self.d.specialists.keys() {
                if self.cities[c].specialist_count[specialist] > 0 {
                    let value = self.city_specialist_citizen_value(c, &w, specialist);
                    if value < worst_value {
                        worst_value = value;
                        worst_specialist = Some(specialist);
                    }
                }
            }
        }

        if let Some(specialist) = worst_specialist {
            self.city_change_specialist_count(c, specialist, -1);
            return true;
        }

        false
    }

    /// The governor's weights for city `c` (used by the citizen-button stand-ins).
    fn city_citizen_weights(&self, c: CityId) -> Weights {
        self.governor_weights(c)
    }

    /// [`citizen_value`] of one more specialist of type `specialist` in city `c`.
    fn city_specialist_citizen_value(&self, c: CityId, w: &Weights, specialist: SpecialistType) -> i32 {
        let owner = self.cities[c].owner;
        let yields: [i32; YieldType::NUM] = std::array::from_fn(|i| self.player_specialist_yield(owner, specialist, YieldType::from_usize(i)));
        let commerce: [i32; CommerceType::NUM] =
            std::array::from_fn(|i| self.player_specialist_commerce(owner, specialist, CommerceType::from_usize(i)));
        citizen_value(w, &yields, &commerce, self.d.specialists[specialist].great_people_rate_change)
    }
}

/// The governor's value of one citizen's output with food at a flat 20 per point (for the
/// citizen-button stand-ins, which compare candidates without the food curve).
fn citizen_value(w: &Weights, yields: &[i32; YieldType::NUM], commerce: &[i32; CommerceType::NUM], great_people: i32) -> i32 {
    yields[YieldType::YIELD_FOOD.idx()] * 20 + non_food_value(w, yields, commerce, great_people)
}
