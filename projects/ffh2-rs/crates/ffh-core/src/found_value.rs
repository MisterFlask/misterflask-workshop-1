//! C++ `CvPlayerAI::AI_foundValue` (CvPlayerAI.cpp:1847-2993) and the bonus valuation it uses
//! (`AI_bonusVal` 7690-7716, `AI_baseBonusVal` 7719-8038, `AI_corporationBonusVal` 8040-8086),
//! plus `AI_getMinFoundValue` (18519-18545) and, as a private helper,
//! `AI_countDeadlockedBonuses` (16767-16825).
//!
//! These are AI functions in C++, but rules code depends on them: starting plots
//! (`CvPlot::getFoundValue` -> `CvPlayer::findStartingPlot`, `CvGame::normalizeStartingPlotLocations`
//! and the other `CvGame` starting-plot passes) and barbarian city placement
//! (`CvGame::createBarbarianCities`, FfH's barbarian city spawning). So they are ported
//! faithfully here; the new AI (`ffh-ai`) also uses them to score city sites.
//!
//! C++ `AI_isPlotCitySite`/`AI_getNumCitySites`/`AI_getCitySite` read the AI's cached city-site
//! list (`m_aiAICitySites`, filled by `AI_updateCitySites` for non-barbarian AI players). Rules
//! callers (barbarians, starting locations) never see a non-empty list, so the list is an
//! explicit `city_sites` argument: rules code passes `&[]`, the new AI passes its own sites.
//!
//! `AI_bonusVal` default argument: `CvPlayerAI.h` declares `iChange = 1` while the `CvPlayer`
//! virtual declares `iChange = 0`. C++ picks the default from the static type of the call, and
//! every DLL caller holds a `CvPlayerAI` (`GET_PLAYER` is `CvPlayerAI::getPlayer`;
//! `AI_foundValue` calls it on `this`), so the effective default is 1.
//!
//! `AI_baseBonusVal` is cached per player as in C++ (`Player::ai_bonus_value`, C++
//! `m_aiBonusValue`): filled on first use and reset by `player_ai_update_bonus_value` (each
//! turn in `AI_doTurnPre`'s place, on team merge and empire split), so a value can lag behind
//! changes to the player's cities or techs exactly as in C++.
//!
//! Integer arithmetic is the C++ `int` arithmetic: truncating division, and wrapping on
//! overflow like the MSVC build (the starting-location distance scaling multiplies the value
//! by 100 and then by up to ~3000, which overflows for good sites). Wrapping operations are
//! used wherever the found value or a term multiplied into it is scaled.

use std::cmp::{max, min};

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::player::Player;
use crate::utils::{is_limited_unit_class, is_limited_wonder_class};
use ffh_data::*;

/// C++ `GREATER_FOUND_RANGE` (CvPlayerAI.cpp:29).
const GREATER_FOUND_RANGE: i32 = 5;

/// Yield array indices (C++ `YIELD_FOOD`, ...).
const FOOD: usize = YieldType::YIELD_FOOD.0 as usize;
const PRODUCTION: usize = YieldType::YIELD_PRODUCTION.0 as usize;
const COMMERCE: usize = YieldType::YIELD_COMMERCE.0 as usize;

impl Game {
    /// C++ `CvPlayerAI::AI_foundValue(iX, iY, iMinRivalRange = -1, bStartingLoc = false)`
    /// (FfH 0.41 with Sephi's "BETTER AI" city placement): how good a city at (x, y) would be for
    /// `p`; 0 = cannot or should not found there, otherwise at least 1. `min_rival_range != -1`
    /// rejects sites with a unit of another team (`PUF_isOtherTeam`) within that many plots
    /// (barbarian cities); `starting_loc` scores a starting location. `city_sites` is the AI's
    /// city-site list (see the module docs; `&[]` for rules callers).
    pub fn player_ai_found_value(&self, p: PlayerId, x: i32, y: i32, min_rival_range: i32, starting_loc: bool, city_sites: &[PlotIdx]) -> i32 {
        let d = self.d;
        let pl = &self.players[p];
        let team = pl.team;
        let food_per_pop = d.defines.food_consumption_per_population;

        let mut resource_value: i32 = 0;
        let mut special_food = 0;
        let mut special_food_plus = 0;
        let mut special_food_minus = 0;
        let mut special_production = 0;
        let mut special_commerce = 0;

        // BETTER AI (Better City Placement) Sephi: adjust for Kuriotates
        let num_city_plots = found_value_num_city_plots(pl);
        let city_plots = num_city_plots as usize;

        let mut neutral_territory = true;

        let plot = self.map.plot_xy(x, y);

        if !self.player_can_found(p, x, y, false) {
            return 0;
        }
        // C++ dereferences pPlot from here on (canFound already did).
        let plot = plot.expect("AI_foundValue: canFound accepted an off-map plot");
        let (plot_x, plot_y) = self.map.xy(plot);

        // BETTER AI (Better City Placement) Sephi: Pirates love the open sea
        if pl.num_cities() == 0 {
            if self.player_is_pirate(p) {
                if !self.plot_is_coastal_land(plot, 5) {
                    return 0;
                }
            }
        }

        // stop AI from founding Cities on Bonus
        if self.plot_get_bonus_type(plot, None).is_some() {
            return 0;
        }

        let is_coastal = self.plot_is_coastal_land(plot, d.defines.min_water_size_for_ocean);
        let area = area_of(self, plot);
        let num_area_cities = self.map.areas[area].cities_per_player[p];

        let mut advanced_start = pl.advanced_start_points >= 0;
        // BETTER AI (Better City Placement) Sephi: adjust free first City selection
        if !starting_loc && !advanced_start && !(pl.num_cities() == 0) {
            if !is_coastal && num_area_cities == 0 {
                return 0;
            }
        }

        if advanced_start {
            debug_assert!(self.is_option(GameOptionType::GAMEOPTION_ADVANCED_START));
            if starting_loc {
                advanced_start = false;
            }
        }

        // Explaination of city site adjustment:
        // Any plot which is otherwise within the radius of a city site
        // is basically treated as if it's within an existing city radius
        // (BETTER AI, Sephi: sized iNumCityPlots instead of NUM_CITY_PLOTS)
        let mut city_site_radius = vec![false; city_plots];

        if !starting_loc {
            if !is_plot_city_site(city_sites, plot) {
                // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
                for (i, site_radius) in city_site_radius.iter_mut().enumerate() {
                    if let Some(loop_plot) = self.plot_city(x, y, i) {
                        let (lx, ly) = self.map.xy(loop_plot);
                        // AI_getNumCitySites() / AI_getCitySite(iJ)
                        for &city_site_plot in city_sites {
                            if city_site_plot != plot {
                                let (cx, cy) = self.map.xy(city_site_plot);
                                if self.plot_distance(lx, ly, cx, cy) <= CITY_PLOTS_RADIUS {
                                    // Plot is inside the radius of a city site
                                    *site_radius = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut bonus_count: IdxVec<BonusType, i32> = IdxVec::from_elem(0, d.bonuses.len());

        if min_rival_range != -1 {
            for dx in -min_rival_range..=min_rival_range {
                for dy in -min_rival_range..=min_rival_range {
                    if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                        if self.plot_plot_check(loop_plot, Game::puf_is_other_team, p.i(), -1, None, None, None, -1, -1).is_some() {
                            return 0;
                        }
                    }
                }
            }
        }

        if starting_loc {
            if self.plot_is_goody(plot, None) {
                return 0;
            }

            // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
            for i in 0..city_plots {
                if self.plot_city(x, y, i).is_none() {
                    return 0;
                }
            }
        }

        let mut owned_tiles = 0;

        // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
        for i in 0..city_plots {
            match self.plot_city(x, y, i) {
                None => owned_tiles += 1,
                Some(loop_plot) => {
                    if self.plot_is_owned(loop_plot) {
                        if self.plot_get_team(loop_plot) != Some(team) {
                            owned_tiles += 1;
                        }
                    }
                }
            }
        }

        // FfH: Modified by Kael 11/18/2007
        if owned_tiles > num_city_plots / 3 {
            return 0;
        }

        let mut bad_tile = 0;

        // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
        for (i, &site_radius) in city_site_radius.iter().enumerate() {
            let loop_plot = self.plot_city(x, y, i);

            if i != CITY_HOME_PLOT {
                match loop_plot {
                    None => bad_tile += 2,
                    Some(lp) if self.plot_is_impassable(lp) => bad_tile += 2,
                    Some(lp) => {
                        if !self.plot_is_fresh_water(lp) && !self.plot_is_hills(lp) {
                            // FfH: Modified by Kael 09/19/2009 (made the AI a little more picky
                            // about where to place cities: total best nature yield <= 2, was <= 1)
                            if self.plot_calculate_best_nature_yield(lp, YieldType::YIELD_FOOD, Some(team)) == 0
                                || self.plot_calculate_total_best_nature_yield(lp, Some(team)) <= 2
                            {
                                bad_tile += 2;
                            } else if self.plot_is_water(lp)
                                && !is_coastal
                                && self.plot_calculate_best_nature_yield(lp, YieldType::YIELD_FOOD, Some(team)) <= 1
                            {
                                bad_tile += 1;
                            }
                        } else if self.plot_is_owned(lp) {
                            // (As in BtS this `else` branch is only reached by fresh-water or
                            // hills plots.)
                            if self.plot_get_team(lp) == Some(team) {
                                if self.plot_is_city_radius(lp) || site_radius {
                                    bad_tile += if advanced_start { 2 } else { 1 };
                                }
                            }
                        }
                    }
                }
            }
        }

        bad_tile /= 2;

        if !starting_loc {
            // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
            if bad_tile > num_city_plots / 2 || self.map.areas[area].num_tiles <= 2 {
                let mut has_good_bonus = false;

                // FfH: Modified by Kael 11/18/2007
                for i in 0..city_plots {
                    let Some(loop_plot) = self.plot_city(x, y, i) else {
                        continue;
                    };
                    if !self.plot_is_owned(loop_plot) {
                        if self.plot_is_water(loop_plot)
                            || self.plot_area(loop_plot) == Some(area)
                            || self.map.areas[area_of(self, loop_plot)].cities_per_player[p] > 0
                        {
                            if let Some(bonus) = self.plot_get_bonus_type(loop_plot, Some(team)) {
                                if self.player_get_num_tradeable_bonuses(p, bonus) == 0
                                    || ai_bonus_val(self, p, bonus, 1) > 10
                                    || d.bonuses[bonus].yield_change[FOOD] > 0
                                {
                                    has_good_bonus = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                if !has_good_bonus {
                    return 0;
                }
            }
        }

        let mut taken_tiles = 0;
        let mut teammate_taken_tiles = 0;
        let mut health = 0;
        let mut value: i32 = 1000;

        let mut greed = 100;

        if advanced_start {
            greed = 150;
        } else if !starting_loc {
            for t in d.traits.keys() {
                if pl.traits[t] {
                    // Greedy founding means getting the best possible sites - fitting maximum
                    // resources into the fat cross.
                    greed += d.traits[t].upkeep_modifier / 2;
                    greed += 20 * d.traits[t].commerce_change[CommerceType::COMMERCE_CULTURE.idx()];
                }
            }
        }
        // BETTER AI (Better City Placement) Sephi: adjust free first City selection
        if pl.num_cities() == 0 {
            greed = 150;
        }
        // iClaimThreshold is the culture required to pop the 2nd borders.
        let claim_level = min(2, d.culture_levels.len() as i32 - 1);
        let mut claim_threshold = self.culture_threshold(CultureLevelType::from_usize(claim_level as usize));
        claim_threshold = max(1, claim_threshold);
        claim_threshold = claim_threshold.wrapping_mul(max(100, greed));

        // (C++ also declares `int iYieldLostHere = 0;` here; it is never used.)

        // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
        for (i, &site_radius) in city_site_radius.iter().enumerate() {
            let Some(loop_plot) = self.plot_city(x, y, i) else {
                taken_tiles += 1;
                continue;
            };

            if self.plot_is_city_radius(loop_plot) || site_radius {
                taken_tiles += 1;

                if site_radius {
                    teammate_taken_tiles += 1;
                }
                continue;
            }

            let lp = &self.map.plots[loop_plot];
            let (lx, ly) = self.map.xy(loop_plot);
            let mut temp_value: i32 = 0;

            let feature = lp.feature_type;
            let bonus = self.plot_get_bonus_type(loop_plot, if starting_loc { None } else { Some(team) });
            let mut bonus_improvement: Option<ImprovementType> = None;

            let culture_multiplier = match lp.owner {
                // !isOwned() || getOwnerINLINE() == getID()
                None => 100,
                Some(owner) if owner == p => 100,
                Some(owner) => {
                    neutral_territory = false;
                    let our_culture = lp.culture[p];
                    let other_culture = max(1, lp.culture[owner]);
                    let mut multiplier = 100i32.wrapping_mul(our_culture.wrapping_add(claim_threshold));
                    multiplier /= other_culture.wrapping_add(claim_threshold);
                    // The multiplier is basically normalized...
                    // 100% means we own (or rightfully own) the tile.
                    // 50% means the hostile culture is fairly firmly entrenched.
                    min(100, multiplier)
                }
            };

            if culture_multiplier < (if num_area_cities > 0 { 25 } else { 50 }) {
                // discourage hopeless cases, especially on other continents.
                taken_tiles += if num_area_cities > 0 { 1 } else { 2 };
            }

            if let Some(b) = bonus {
                for improvement in d.improvements.keys() {
                    if d.improvements[improvement].improvement_bonus[b].bonus_makes_valid {
                        bonus_improvement = Some(improvement);
                        break;
                    }
                }
            }

            let mut ai_yield = [0i32; YieldType::NUM];

            for (yi, ai) in ai_yield.iter_mut().enumerate() {
                let yield_info = &d.yields[YieldType::from_usize(yi)];
                *ai = lp.yields[yi];

                if i == CITY_HOME_PLOT {
                    let mut base_plot_yield = *ai;
                    *ai += yield_info.city_change;

                    if let Some(f) = feature {
                        *ai -= d.features[f].yield_change[yi];
                        base_plot_yield = max(base_plot_yield, *ai);
                    }

                    match bonus {
                        None => {
                            *ai = max(*ai, yield_info.min_city);
                        }
                        Some(b) => {
                            let bonus_yield_change = d.bonuses[b].yield_change[yi];
                            *ai += bonus_yield_change;
                            base_plot_yield += bonus_yield_change;

                            *ai = max(*ai, yield_info.min_city);
                        }
                    }

                    // (eBonusImprovement is only ever set when eBonus is.)
                    if let (Some(bi), Some(b)) = (bonus_improvement, bonus) {
                        base_plot_yield += d.improvements[bi].improvement_bonus[b].yield_change[yi];

                        if base_plot_yield > *ai {
                            *ai -= 2 * (base_plot_yield - *ai);
                        } else {
                            *ai += *ai - base_plot_yield;
                        }
                    }
                }
            }

            if i == CITY_HOME_PLOT {
                temp_value += ai_yield[FOOD] * 60;
                temp_value += ai_yield[PRODUCTION] * 60;
                temp_value += ai_yield[COMMERCE] * 40;
            } else if ai_yield[FOOD] >= food_per_pop {
                temp_value += ai_yield[FOOD] * 40;
                temp_value += ai_yield[PRODUCTION] * 40;
                temp_value += ai_yield[COMMERCE] * 30;

                if starting_loc {
                    temp_value *= 2;
                }
            } else if ai_yield[FOOD] == food_per_pop - 1 {
                temp_value += ai_yield[FOOD] * 25;
                temp_value += ai_yield[PRODUCTION] * 25;
                temp_value += ai_yield[COMMERCE] * 20;
            } else {
                temp_value += ai_yield[FOOD] * 15;
                temp_value += ai_yield[PRODUCTION] * 15;
                temp_value += ai_yield[COMMERCE] * 10;
            }

            if self.plot_is_water(loop_plot) {
                if ai_yield[COMMERCE] > 1 {
                    temp_value += if is_coastal { 30 } else { -20 };
                    if is_coastal && ai_yield[FOOD] >= food_per_pop {
                        special_food_plus += 1;
                    }
                    if starting_loc && !self.map.plots[plot].starting_plot {
                        // I'm pretty much forbidding starting 1 tile inland non-coastal.
                        // with more than a few coast tiles.
                        temp_value += if is_coastal { 0 } else { -400 };
                    }
                }
                // BETTER AI (Better City Placement) Sephi: Pirates like Water
                if self.player_is_pirate(p) {
                    temp_value += 100;
                }
            }

            if self.plot_is_river(loop_plot) {
                temp_value += 10;
            }

            if i == CITY_HOME_PLOT {
                temp_value = temp_value.wrapping_mul(2);
            } else if lp.owner == Some(p) || self.step_distance(x, y, lx, ly) == 1 {
                temp_value = temp_value.wrapping_mul(3);
                temp_value /= 2;
            } else {
                temp_value = temp_value.wrapping_mul(greed);
                temp_value /= 100;
            }

            temp_value = temp_value.wrapping_mul(culture_multiplier);
            temp_value /= 100;

            value = value.wrapping_add(temp_value);

            if culture_multiplier > 33 {
                // ignore hopelessly entrenched tiles.
                if let Some(f) = feature {
                    if i != CITY_HOME_PLOT {
                        health += d.features[f].health_percent;

                        special_food_plus += max(0, ai_yield[FOOD] - food_per_pop);
                    }
                }

                if let Some(b) = bonus {
                    if self.plot_area(loop_plot) == Some(area) || self.map.areas[area_of(self, loop_plot)].cities_per_player[p] > 0 {
                        bonus_count[b] += 1;
                        debug_assert!(bonus_count[b] > 0);

                        let first_of_kind = !starting_loc && self.player_get_num_tradeable_bonuses(p, b) == 0 && bonus_count[b] == 1;
                        temp_value = ai_bonus_val(self, p, b, 1).wrapping_mul(if first_of_kind { 80 } else { 20 });
                        temp_value = temp_value.wrapping_mul(if starting_loc { 100 } else { greed });
                        temp_value /= 100;

                        if i != CITY_HOME_PLOT && !starting_loc {
                            if lp.owner != Some(p) && self.step_distance(plot_x, plot_y, lx, ly) > 1 {
                                temp_value = temp_value.wrapping_mul(2);
                                temp_value /= 3;

                                temp_value = temp_value.wrapping_mul(min(150, greed));
                                temp_value /= 100;
                            }
                        }

                        value = value.wrapping_add(temp_value.wrapping_add(10));

                        if i != CITY_HOME_PLOT {
                            if let Some(bi) = bonus_improvement {
                                let bonus_yield = &d.improvements[bi].improvement_bonus[b].yield_change;
                                let mut special_food_temp =
                                    self.plot_calculate_best_nature_yield(loop_plot, YieldType::YIELD_FOOD, Some(team)) + bonus_yield[FOOD];

                                special_food += special_food_temp;

                                special_food_temp -= food_per_pop;

                                special_food_plus += max(0, special_food_temp);
                                special_food_minus -= min(0, special_food_temp);
                                special_production +=
                                    self.plot_calculate_best_nature_yield(loop_plot, YieldType::YIELD_PRODUCTION, Some(team)) + bonus_yield[PRODUCTION];
                                special_commerce +=
                                    self.plot_calculate_best_nature_yield(loop_plot, YieldType::YIELD_COMMERCE, Some(team)) + bonus_yield[COMMERCE];
                            }

                            // BETTER AI (Better City Placement) Sephi: grassland farms are really good
                            if self.plot_is_fresh_water(loop_plot) {
                                // C++ indexes the terrain info unchecked: every map plot has a terrain.
                                let terrain = lp.terrain_type.expect("AI_foundValue: plot without a terrain");
                                if d.terrains[terrain].yield_[FOOD] > 1 {
                                    // Grasslands
                                    special_food += 1;
                                }
                                if let Some(f) = feature {
                                    if d.features[f].yield_change[FOOD] > 2 {
                                        // Flood Plains and Oasis
                                        special_food += 1;
                                    }
                                }
                            }

                            if let Some(f) = feature {
                                if d.features[f].yield_change[FOOD] < 0 {
                                    resource_value -= 30;
                                }

                                // BETTER AI (Better City Placement) Sephi: Elves like Forests
                                // (rewritten by Kael 09/19/2009):
                                // `if (GC.getCivilizationInfo(getCivilizationType()).isMaintainFeatures(eFeature)) iTempValue += 50;`
                                // C++ quirk: a dead store -- iTempValue is not read again before
                                // its next assignment, so this has no effect and is not ported.
                            }
                            if self.plot_is_water(loop_plot) {
                                // BETTER AI (Better City Placement) Sephi: Pirates like Water
                                // (orig: `iValue += (bIsCoastal ? 100 : -800);` for everyone)
                                if !self.player_is_pirate(p) {
                                    value = value.wrapping_add(if is_coastal { 100 } else { -800 });
                                }
                            }
                        }
                    }
                }
            }
        }

        resource_value += special_food * 50;
        resource_value += special_production * 50;
        resource_value += special_commerce * 50;
        if starting_loc {
            resource_value /= 2;
        }

        value = value.wrapping_add(max(0, resource_value));

        // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
        if taken_tiles > num_city_plots / 3 && resource_value < 250 {
            return 0;
        }

        if teammate_taken_tiles > 1 {
            return 0;
        }

        value = value.wrapping_add(health / 5);

        if is_coastal {
            if !starting_loc {
                if self.map.areas[area].cities_per_player[p] == 0 {
                    if neutral_territory {
                        value = value.wrapping_add(if resource_value > 0 { 800 } else { 100 });
                    }
                } else {
                    value = value.wrapping_add(400);
                }
            } else {
                // let other penalties bring this down.
                value = value.wrapping_add(600);
                if !self.map.plots[plot].starting_plot {
                    if self.map.areas[area].num_starting_plots == 0 {
                        value = value.wrapping_add(1000);
                    }
                }
            }
        }

        if self.plot_is_hills(plot) {
            value = value.wrapping_add(200);
        }

        if self.plot_is_river(plot) {
            value = value.wrapping_add(40);
        }

        if self.plot_is_fresh_water(plot) {
            value = value.wrapping_add(40);
            value = value.wrapping_add(d.defines.fresh_water_health_change * 30);
        }

        if starting_loc {
            let range = GREATER_FOUND_RANGE;
            let mut greater_bad_tile = 0;

            for dx in -range..=range {
                for dy in -range..=range {
                    let Some(loop_plot) = self.plot_xy(x, y, dx, dy) else {
                        continue;
                    };
                    if self.plot_is_water(loop_plot) || self.plot_area(loop_plot) == Some(area) {
                        let (lx, ly) = self.map.xy(loop_plot);
                        if self.plot_distance(x, y, lx, ly) <= range {
                            let loop_yields = &self.map.plots[loop_plot].yields;
                            let mut temp_value = 0;
                            temp_value += loop_yields[FOOD] * 15;
                            temp_value += loop_yields[PRODUCTION] * 11;
                            temp_value += loop_yields[COMMERCE] * 5;
                            value = value.wrapping_add(temp_value);
                            if temp_value < 21 {
                                greater_bad_tile += 2;
                                if self.map.plots[loop_plot].feature_type.is_some() {
                                    if self.plot_calculate_best_nature_yield(loop_plot, YieldType::YIELD_FOOD, Some(team)) > 1 {
                                        greater_bad_tile -= 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !self.map.plots[plot].starting_plot {
                greater_bad_tile /= 2;
                if greater_bad_tile > 12 {
                    value = value.wrapping_mul(11);
                    value /= greater_bad_tile;
                }
            }

            let mut water_count = 0;

            // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
            for i in 0..city_plots {
                if let Some(loop_plot) = self.plot_city(x, y, i) {
                    if self.plot_is_water(loop_plot) {
                        water_count += 1;
                        if self.map.plots[loop_plot].yields[FOOD] <= 1 {
                            water_count += 1;
                        }
                    }
                }
            }
            water_count /= 2;

            // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS; the
            // "discourage very water-heavy starts" comment was dropped with the old code)
            let land_count = num_city_plots - water_count;
            if land_count < num_city_plots / 2 {
                value = value.wrapping_mul(1 + land_count);
                value /= 1 + num_city_plots / 2;
            }
        }

        if starting_loc {
            let min_original_start_dist = self.map.plots[plot].min_original_start_dist;
            if min_original_start_dist == -1 {
                value = value.wrapping_add(self.map.max_step_distance() * 100);
            } else {
                value = value.wrapping_mul(1 + 4 * min_original_start_dist);
                value /= 1 + 2 * self.map.max_step_distance();
            }

            // nice hacky way to avoid this messing with normalizer, use elsewhere?
            if !self.map.plots[plot].starting_plot {
                let mut min_distance_factor = MAX_INT;
                let min_range = self.player_starting_plot_range(p);

                // C++ quirk: these products overflow `int` for good sites (MSVC wraps them).
                value = value.wrapping_mul(100);
                for j in self.civ_player_ids() {
                    if self.players[j].alive {
                        if j != p {
                            let closeness_factor = self.player_starting_plot_distance_factor(j, plot, p, min_range);
                            min_distance_factor = min(closeness_factor, min_distance_factor);

                            if closeness_factor < 1000 {
                                value = value.wrapping_mul(2000 + closeness_factor);
                                value /= 3000;
                            }
                        }
                    }
                }

                if min_distance_factor > 1000 {
                    // give a maximum boost of 25% for somewhat distant locations, don't go overboard.
                    min_distance_factor = min(1500, min_distance_factor);
                    value = value.wrapping_mul(1000 + min_distance_factor);
                    value /= 2000;
                } else if min_distance_factor < 1000 {
                    // this is too close so penalize again.
                    value = value.wrapping_mul(min_distance_factor);
                    value /= 1000;
                    value = value.wrapping_mul(min_distance_factor);
                    value /= 1000;
                }

                value /= 10;

                if self.plot_get_bonus_type(plot, None).is_some() {
                    value /= 2;
                }
            }
        }

        if advanced_start {
            if self.plot_get_bonus_type(plot, None).is_some() {
                value = value.wrapping_mul(70);
                value /= 100;
            }
        }

        let barbarian = self.player_is_barbarian(p);
        let nearest_city = self.map_find_city(x, y, if barbarian { None } else { Some(p) }, None, true, false, None, None, None);

        if let Some(nearest_city) = nearest_city {
            let (cx, cy) = (self.cities[nearest_city].x, self.cities[nearest_city].y);
            if barbarian {
                value = value.wrapping_sub(max(0, 8 - self.plot_distance(x, y, cx, cy)) * 200);
            } else {
                let distance = self.plot_distance(x, y, cx, cy);
                let num_cities = pl.num_cities();
                if distance > 5 {
                    value = value.wrapping_sub((distance - 5) * 500);
                } else if distance < 4 {
                    value = value.wrapping_sub((4 - distance) * 2000);
                }
                value = value.wrapping_mul(8 + num_cities * 4);
                value /= 2 + (num_cities * 4) + distance;
                if self.city_is_capital(nearest_city) {
                    value = value.wrapping_mul(150);
                    value /= 100;
                } else if let Some(capital) = self.player_get_capital_city(p) {
                    // Provide up to a 50% boost to value (80% for adv.start)
                    // for city sites which are relatively close to the core
                    // compared with the most distance city from the core
                    // (having a boost rather than distance penalty avoids some distortion)

                    // This is not primarly about maitenance but more about empire
                    // shape as such forbidden palace/state property are not big deal.
                    let mut max_distance_from_capital = 0;

                    let (capital_x, capital_y) = (self.cities[capital].x, self.cities[capital].y);

                    // firstCity / nextCity
                    for &loop_city in &pl.cities {
                        let (lcx, lcy) = (self.cities[loop_city].x, self.cities[loop_city].y);
                        max_distance_from_capital = max(max_distance_from_capital, self.plot_distance(capital_x, capital_y, lcx, lcy));
                    }

                    // C++ quirk: `iDistanceToCapital = plotDistance(iCapitalX, iCapitalY, iX, iY)`
                    // is computed here but never used; the boost uses the nearest city's distance.

                    debug_assert!(max_distance_from_capital > 0);
                    let boost = if advanced_start { 80 } else { 50 };
                    value = value.wrapping_mul(100 + (boost * max(0, max_distance_from_capital - distance)) / max_distance_from_capital);
                    value /= 100;
                }
            }
        } else {
            let nearest_city = self.map_find_city(
                x,
                y,
                if barbarian { None } else { Some(p) },
                if barbarian { None } else { Some(team) },
                false,
                false,
                None,
                None,
                None,
            );
            if let Some(nearest_city) = nearest_city {
                let (cx, cy) = (self.cities[nearest_city].x, self.cities[nearest_city].y);
                let distance = self.plot_distance(x, y, cx, cy);
                value = value.wrapping_sub(min(500 * distance, (8000 * distance) / self.map.max_plot_distance()));
            }
        }

        if value <= 0 {
            return 1;
        }

        let area_num_cities = self.map.areas[area].num_cities;
        if area_num_cities == 0 {
            value = value.wrapping_mul(2);
        } else {
            let team_area_cities = self.team_count_num_cities_by_area(team, area);

            if area_num_cities == team_area_cities {
                value = value.wrapping_mul(3);
                value /= 2;
            } else if area_num_cities == team_area_cities + self.team_count_num_cities_by_area(self.barbarian_team(), area) {
                value = value.wrapping_mul(4);
                value /= 3;
            } else if team_area_cities > 0 {
                value = value.wrapping_mul(5);
                value /= 4;
            }
        }

        if !starting_loc {
            let food_surplus = max(0, special_food_plus - special_food_minus);
            let food_deficit = max(0, special_food_minus - special_food_plus);

            value = value.wrapping_mul(100 + 20 * max(0, min(food_surplus, 2 * food_per_pop)));
            value /= 100 + 20 * max(0, food_deficit);
        }

        if !starting_loc && pl.num_cities() > 0 {
            let mut total_bonus_count = 0;
            let mut unique_bonus_count = 0;
            for &count in &bonus_count {
                total_bonus_count += count;
                unique_bonus_count += if count > 0 { 1 } else { 0 };
            }
            if total_bonus_count > 4 {
                value = value.wrapping_mul(5);
                value /= 1 + total_bonus_count;
            } else if unique_bonus_count > 2 {
                value = value.wrapping_mul(5);
                value /= 3 + unique_bonus_count;
            }
        }

        if !starting_loc {
            let mut dead_lock_count = count_deadlocked_bonuses(self, p, plot);
            if advanced_start && dead_lock_count > 0 {
                dead_lock_count += 2;
            }
            value /= 1 + dead_lock_count;
        }

        // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
        value /= max(0, bad_tile - (num_city_plots / 4)) + 3;

        if starting_loc {
            let mut different_area_tile = 0;

            // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
            for i in 0..city_plots {
                let different = match self.plot_city(x, y, i) {
                    None => true,
                    Some(loop_plot) => !(self.plot_is_water(loop_plot) || self.plot_area(loop_plot) == Some(area)),
                };
                if different {
                    different_area_tile += 1;
                }
            }

            // FfH: Modified by Kael 11/18/2007 (iNumCityPlots instead of NUM_CITY_PLOTS)
            value /= max(0, different_area_tile - ((num_city_plots * 2) / 3)) + 2;
        }

        // BETTER AI (Better City Placement) Sephi: adjust for Kuriotates
        if pl.sprawling {
            let range = 4;
            let mut other_city = false;

            for dx in -range..=range {
                for dy in -range..=range {
                    if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                        if self.plot_is_city(loop_plot, false, None) && self.map.plots[loop_plot].owner == Some(p) {
                            other_city = true;
                            // C++ quirk: this `break` only leaves the inner (iDY) loop; the
                            // outer loop keeps scanning, which cannot change the result.
                            break;
                        }
                    }
                }
            }
            if !other_city {
                value = value.wrapping_add(30000);
            }
        }

        max(1, value)
    }

    /// C++ `CvPlayerAI::AI_bonusVal(eBonus, iChange)`: the value of one more (`change = 1`), one
    /// less (`-1`) or any (`0`) instance of `bonus` for `p` (FfH: plus the leader's bad-attitude
    /// term and +100 for mana). C++ default `iChange = 1` (see the module docs).
    pub fn player_ai_bonus_val(&self, p: PlayerId, bonus: BonusType, change: i32) -> i32 {
        ai_bonus_val(self, p, bonus, change)
    }

    /// C++ `CvPlayerAI::AI_baseBonusVal(eBonus)`: the value of a bonus without corporations,
    /// cached per player in `ai_bonus_value` (C++ `m_aiBonusValue`) until the next
    /// `player_ai_update_bonus_value`.
    pub fn player_ai_base_bonus_val(&self, p: PlayerId, bonus: BonusType) -> i32 {
        match self.players[p].ai_bonus_value.get(bonus.idx()) {
            Some(cached) if cached.get() != -1 => cached.get(),
            Some(cached) => {
                let value = self.player_ai_base_bonus_val_uncached(p, bonus);
                cached.set(value);
                value
            }
            None => self.player_ai_base_bonus_val_uncached(p, bonus),
        }
    }

    /// C++ `CvPlayerAI::AI_updateBonusValue()`: forget the cached base bonus values (C++ calls it
    /// in `AI_doTurnPre` for every player each turn, in `CvTeam::addTeam` and `splitEmpire`).
    pub fn player_ai_update_bonus_value(&self, p: PlayerId) {
        for cached in &self.players[p].ai_bonus_value {
            cached.set(-1);
        }
    }

    /// C++ `CvPlayerAI::AI_baseBonusVal` (value of a bonus without corporations), computed fresh
    /// (C++ caches it in `m_aiBonusValue`, see the module docs).
    fn player_ai_base_bonus_val_uncached(&self, p: PlayerId, bonus: BonusType) -> i32 {
        let d = self.d;
        let pl = &self.players[p];
        let team = pl.team;
        let current_era = pl.current_era.i();

        // `iDiff = abs(GC.getTechInfo(eTech).getEra() - getCurrentEra())` scaling, shared by
        // units, buildings and projects.
        let era_scale = |temp_value: &mut i32, tech: Option<TechType>| {
            if let Some(tech) = tech {
                let diff = (opt_to_i32(d.techs[tech].era) - current_era).abs();

                if diff == 0 {
                    *temp_value *= 3;
                    *temp_value /= 2;
                } else {
                    *temp_value /= diff;
                }
            }
        };

        let mut value = 0;

        if !self.team_is_bonus_obsolete(team, bonus) {
            value += d.bonuses[bonus].happiness * 100;
            value += d.bonuses[bonus].health * 100;

            let capital = self.player_get_capital_city(p);
            let city_count = pl.num_cities();
            let coastal_city_count = self.player_count_num_coastal_cities(p);

            // find the first coastal city
            let mut coastal_city: Option<CityId> = None;
            let mut unconnected_coastal_city: Option<CityId> = None;
            if coastal_city_count > 0 {
                // firstCity / nextCity
                for &loop_city in &pl.cities {
                    if self.city_is_coastal(loop_city, d.defines.min_water_size_for_ocean) {
                        if self.city_is_connected_to_capital(loop_city, Some(p)) {
                            coastal_city = Some(loop_city);
                            break;
                        } else if unconnected_coastal_city.is_none() {
                            unconnected_coastal_city = Some(loop_city);
                        }
                    }
                }
            }
            if coastal_city.is_none() && unconnected_coastal_city.is_some() {
                coastal_city = unconnected_coastal_city;
            }

            // C++ indexes `GC.getCivilizationInfo(getCivilizationType())` unchecked.
            let civ = self.player_get_civilization_type(p).expect("AI_baseBonusVal: player without a civilization");

            for unit_class in d.unit_classes.keys() {
                let Some(loop_unit) = d.civ_unit(civ, unit_class) else {
                    continue;
                };
                let unit_info = &d.units[loop_unit];

                let mut temp_value = 0;

                if unit_info.prereq_and_bonus == Some(bonus) {
                    temp_value += 50;
                }

                // C++ scans NUM_UNIT_PREREQ_OR_BONUSES slots; the data keeps the listed bonuses.
                for &or_bonus in &unit_info.prereq_or_bonuses {
                    if or_bonus == bonus {
                        temp_value += 40;
                    }
                }

                temp_value += unit_info.bonus_production_modifier[bonus] / 10;

                if temp_value > 0 {
                    let is_water = unit_info.domain_type == Some(DomainType::DOMAIN_SEA);

                    // if non-limited water unit, weight by coastal cities
                    if is_water && !unit_info.unit_class_type.is_some_and(|c| is_limited_unit_class(d, c)) {
                        temp_value *= min(coastal_city_count * 2, city_count); // double coastal cities, cap at total cities
                        temp_value /= max(1, city_count);
                    }

                    if self.player_can_train(p, loop_unit, false, false, false) {
                        // is it a water unit and no coastal cities or our coastal city cannot build because its obsolete
                        let water_obsolete =
                            is_water && coastal_city.map_or(true, |c| self.city_all_upgrades_available(c, loop_unit, 0).is_some());
                        // or our capital cannot build because its obsolete (we can already build all its upgrades)
                        if water_obsolete || capital.is_some_and(|c| self.city_all_upgrades_available(c, loop_unit, 0).is_some()) {
                            // its worthless
                            temp_value = 2;
                        } else {
                            // otherwise, value units we could build if we had this bonus double
                            temp_value *= 2;
                        }
                    }

                    era_scale(&mut temp_value, unit_info.prereq_and_tech);

                    value += temp_value;
                }
            }

            for building_class in d.building_classes.keys() {
                let Some(loop_building) = d.civ_building(civ, building_class) else {
                    continue;
                };
                let building_info = &d.buildings[loop_building];

                let mut temp_value = 0;

                if building_info.prereq_and_bonus == Some(bonus) {
                    temp_value += 30;
                }

                for &or_bonus in &building_info.prereq_or_bonuses {
                    if or_bonus == Some(bonus) {
                        temp_value += 20;
                    }
                }

                temp_value += building_info.bonus_production_modifier[bonus] / 10;

                if building_info.power_bonus == Some(bonus) {
                    temp_value += 60;
                }

                for yi in 0..YieldType::NUM {
                    temp_value += building_info.bonus_yield_modifier[bonus][yi] / 2;
                    if building_info.power_bonus == Some(bonus) {
                        temp_value += building_info.power_yield_modifier[yi];
                    }
                }

                // Performance only: a zero value stays zero through everything below (every
                // step multiplies or divides it), and the tech checks and canConstruct are
                // const, so they are skipped then. Same result as the C++.
                if temp_value == 0 {
                    continue;
                }

                {
                    // determine whether we have the tech for this building
                    let mut has_tech_for_building = true;
                    if !self.team_is_has_tech(team, building_info.prereq_and_tech) {
                        has_tech_for_building = false;
                    }
                    for &and_tech in &building_info.prereq_and_techs {
                        if !has_tech_for_building {
                            break;
                        }
                        if and_tech.is_some() {
                            if !self.team_is_has_tech(team, and_tech) {
                                has_tech_for_building = false;
                            }
                        }
                    }

                    let is_state_religion = building_info.state_religion.is_some();

                    //check if function call is cached
                    let can_construct = self.player_can_construct(p, loop_building, false, /*bTestVisible*/ true, /*bIgnoreCost*/ true);

                    // bCanNeverBuild when true is accurate, it may be false in some cases where we will never be able to build
                    let can_never_build = has_tech_for_building && !can_construct && !is_state_religion;

                    // if we can never build this, it is worthless
                    if can_never_build {
                        temp_value = 0;
                    }
                    // double value if we can build it right now
                    else if can_construct {
                        temp_value *= 2;
                    }

                    // if non-limited water building, weight by coastal cities
                    if building_info.water && !building_info.building_class_type.is_some_and(|c| is_limited_wonder_class(d, c)) {
                        temp_value *= coastal_city_count;
                        temp_value /= max(1, city_count / 2);
                    }

                    era_scale(&mut temp_value, building_info.prereq_and_tech);

                    value += temp_value;
                }
            }

            for project in d.projects.keys() {
                let project_info = &d.projects[project];
                let mut temp_value = 0;

                temp_value += project_info.bonus_production_modifier[bonus] / 10;

                if temp_value > 0 {
                    let maxed_out = self.is_project_maxed_out(project, 0) || self.team_is_project_maxed_out(team, project, 0);

                    if maxed_out {
                        // project worthless
                        temp_value = 0;
                    } else if self.player_can_create(p, project, false, false) {
                        temp_value *= 2;
                    }

                    era_scale(&mut temp_value, project_info.tech_prereq);

                    value += temp_value;
                }
            }

            // (C++ calls getBestRoute() again inside the comparison below; it is const and
            // returns the same route, so the first result is reused.)
            let best_route = self.player_get_best_route(p, None);
            for build in d.builds.keys() {
                let Some(route) = d.builds[build].route else {
                    continue;
                };
                let route_info = &d.routes[route];

                let mut temp_value = 0;
                if route_info.prereq_bonus == Some(bonus) {
                    temp_value += 80;
                }
                for &or_bonus in &route_info.prereq_or_bonus {
                    if or_bonus == Some(bonus) {
                        temp_value += 40;
                    }
                }
                if best_route.is_some_and(|best| d.routes[best].value <= route_info.value) {
                    value += temp_value;
                } else {
                    value += temp_value / 2;
                }
            }

            //	int iCorporationValue = AI_corporationBonusVal(eBonus);
            //	iValue += iCorporationValue;
            //
            //	if (iCorporationValue <= 0 && getNumAvailableBonuses(eBonus) > 0)
            //	{
            //		iValue /= 3;
            //	}

            value /= 10;
        }

        //clamp value non-negative
        max(0, value)
    }

    /// C++ `CvPlayerAI::AI_corporationBonusVal`. (FfH 0.41 defines no corporations, so this is
    /// always 0 with the shipped data.)
    pub fn player_ai_corporation_bonus_val(&self, p: PlayerId, bonus: BonusType) -> i32 {
        let d = self.d;
        let num_cities = self.players[p].num_cities();
        let mut value = 0;
        let mut city_count = num_cities;
        city_count += city_count / 6 + 1;

        for corporation in d.corporations.keys() {
            let mut corp_count = self.player_get_has_corporation_count(p, corporation);
            if corp_count > 0 {
                corp_count += num_cities / 6 + 1;
                let corp = &d.corporations[corporation];
                // C++ scans NUM_CORPORATION_PREREQ_BONUSES slots; the data keeps the listed bonuses.
                for &prereq_bonus in &corp.prereq_bonus {
                    if bonus == prereq_bonus {
                        let yields = &corp.yield_produced;
                        let commerces = &corp.commerce_produced;
                        value += (50 * yields[FOOD] * corp_count) / city_count;
                        value += (50 * yields[PRODUCTION] * corp_count) / city_count;
                        value += (30 * yields[COMMERCE] * corp_count) / city_count;

                        value += (30 * commerces[CommerceType::COMMERCE_GOLD.idx()] * corp_count) / city_count;
                        value += (30 * commerces[CommerceType::COMMERCE_RESEARCH.idx()] * corp_count) / city_count;
                        value += (12 * commerces[CommerceType::COMMERCE_CULTURE.idx()] * corp_count) / city_count;
                        value += (20 * commerces[CommerceType::COMMERCE_ESPIONAGE.idx()] * corp_count) / city_count;

                        //Disabled since you can't found/spread a corp unless there is already a bonus,
                        //and that bonus will provide the entirity of the bonusProduced benefit.
                    }
                }
            }
        }

        value /= 100; //percent
        value /= 10; //match AI_baseBonusVal

        value
    }

    /// C++ `CvPlayerAI::AI_getMinFoundValue` (used by the AI's site filtering), with the
    /// UNOFFICIAL_PATCH (06/11/09, jdog5000 & DanF5771) expense fix.
    pub fn player_ai_min_found_value(&self, p: PlayerId) -> i32 {
        let gold_per_turn = self.players[p].gold_per_turn;
        let mut value = 600;
        let net_commerce = 1
            + self.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD)
            + self.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH)
            + max(0, gold_per_turn);
        // UNOFFICIAL_PATCH bugfix; original BTS code:
        // `int iNetExpenses = calculateInflatedCosts() + std::min(0, getGoldPerTurn());`
        let net_expenses = self.player_calculate_inflated_costs(p) + max(0, -gold_per_turn);

        value *= net_commerce;
        value /= max(max(1, net_commerce / 4), net_commerce - net_expenses);

        if self.team_get_any_war_plan_count(self.players[p].team, true) > 0 {
            value *= 2;
        }

        value
    }
}

/// BETTER AI (Better City Placement) Sephi, "adjust for Kuriotates": the number of city plots
/// `AI_foundValue` looks at (C++ `iNumCityPlots`): the BtS fat cross (21), or for a sprawling
/// player 37 while below its city limit and 9 once at it. C++ quirk: with no limit
/// (`getMaxCities() == -1`) `getNumCities() < getMaxCities()` is false, so 9.
fn found_value_num_city_plots(pl: &Player) -> i32 {
    let mut num_city_plots = 21;
    if pl.sprawling {
        if pl.num_cities() < pl.max_cities {
            num_city_plots = 37;
        } else {
            num_city_plots = 9;
        }
    }
    num_city_plots
}

/// C++ `CvPlayerAI::AI_isPlotCitySite(pPlot)` over an explicit city-site list.
fn is_plot_city_site(city_sites: &[PlotIdx], plot: PlotIdx) -> bool {
    city_sites.contains(&plot)
}

/// C++ `pPlot->area()` where the C++ dereferences it (every plot has an area once
/// `CvMap::calculateAreas` has run).
fn area_of(g: &Game, plot: PlotIdx) -> AreaId {
    g.plot_area(plot).expect("AI_foundValue: CvPlot::area() is NULL (areas not calculated)")
}

/// C++ `CvPlayerAI::AI_bonusVal(eBonus, iChange)`.
fn ai_bonus_val(g: &Game, p: PlayerId, bonus: BonusType, change: i32) -> i32 {
    let d = g.d;
    let mut value = 0;
    let bonus_count = g.player_get_num_available_bonuses(p, bonus);
    let base_bonus_val = || g.player_ai_base_bonus_val(p, bonus);
    if change == 0 || (change == 1 && bonus_count == 0) || (change == -1 && bonus_count == 1) {
        //This is assuming the none-to-one or one-to-none case.
        value += base_bonus_val();
        value += g.player_ai_corporation_bonus_val(p, bonus);
    } else {
        //This is basically the marginal value of an additional instance of a bonus.
        value += base_bonus_val() / 5;
        value += g.player_ai_corporation_bonus_val(p, bonus);
    }

    // FfH: Added by Kael 12/08/2007
    // C++ indexes `GC.getLeaderHeadInfo(getPersonalityType())` unchecked. Every civ player has a
    // personality; a missing one (the barbarian slot until setup gives it BARBARIAN_LEADER, whose
    // iAttitudeBadBonus is 0 anyway) reads as 0.
    let attitude_bad_bonus = g.players[p].personality_type.map_or(0, |leader| d.leader_heads[leader].attitude_bad_bonus);
    value += d.bonuses[bonus].bad_attitude * attitude_bad_bonus * 100;
    if opt_to_i32(d.bonuses[bonus].bonus_class_type) == d.defines.bonusclass_mana {
        value += 100;
    }
    // FfH: End Add

    value
}

/// C++ `CvPlayerAI::AI_countDeadlockedBonuses(pPlot)` (CvPlayerAI.cpp:16767-16825): bonuses
/// just outside the fat cross of a city at `plot` that could only be claimed by a city site too
/// close to `plot` (there are sites, but none farther than `MIN_CITY_RANGE` from `plot`).
fn count_deadlocked_bonuses(g: &Game, p: PlayerId, plot: PlotIdx) -> i32 {
    let team = g.players[p].team;
    let (px, py) = g.map.xy(plot);
    let plot_area = g.plot_area(plot);

    let min_range = g.d.defines.min_city_range;
    let range = min_range * 2;
    let mut count = 0;

    for dx in -range..=range {
        for dy in -range..=range {
            if g.plot_distance(dx, dy, 0, 0) > CITY_PLOTS_RADIUS {
                let Some(loop_plot) = g.plot_xy(px, py, dx, dy) else {
                    continue;
                };
                if g.plot_get_bonus_type(loop_plot, Some(team)).is_some() {
                    if !g.plot_is_city_radius(loop_plot) && (g.plot_area(loop_plot) == plot_area || g.plot_is_water(loop_plot)) {
                        let mut can_found = false;
                        let mut never_found = true;
                        //potentially blockable resource
                        //look for a city site within a city radius
                        let (lx, ly) = g.map.xy(loop_plot);
                        for i in 0..NUM_CITY_PLOTS {
                            if let Some(loop_plot2) = g.plot_city(lx, ly, i) {
                                let (l2x, l2y) = g.map.xy(loop_plot2);
                                //canFound usually returns very quickly
                                if g.player_can_found(p, l2x, l2y, false) {
                                    never_found = false;
                                    if g.step_distance(px, py, l2x, l2y) > min_range {
                                        can_found = true;
                                        break;
                                    }
                                }
                            }
                        }
                        if !never_found && !can_found {
                            count += 1;
                        }
                    }
                }
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::GameSetup;

    #[test]
    fn num_city_plots_kuriotates() {
        let mut pl = Player::default();
        assert_eq!(found_value_num_city_plots(&pl), 21);

        pl.sprawling = true;
        pl.max_cities = 3;
        assert_eq!(found_value_num_city_plots(&pl), 37);
        pl.cities = vec![CityId(1), CityId(2), CityId(3)];
        assert_eq!(found_value_num_city_plots(&pl), 9);

        // C++ quirk: no city limit (-1) counts as being at the limit.
        pl.cities.clear();
        pl.max_cities = -1;
        assert_eq!(found_value_num_city_plots(&pl), 9);

        // Not sprawling: the limit does not matter.
        pl.sprawling = false;
        assert_eq!(found_value_num_city_plots(&pl), 21);
    }

    #[test]
    fn plot_city_site_list() {
        let sites = [PlotIdx(4), PlotIdx(9)];
        assert!(is_plot_city_site(&sites, PlotIdx(9)));
        assert!(!is_plot_city_site(&sites, PlotIdx(5)));
        assert!(!is_plot_city_site(&[], PlotIdx(4)));
    }


    #[test]
    fn corporation_bonus_val_is_zero_without_corporations() {
        let d = ffh_data::global();
        // FfH 0.41 ships no corporations, so AI_bonusVal never gets a corporation term.
        assert!(d.corporations.is_empty());
        let g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
        for bonus in d.bonuses.keys() {
            assert_eq!(g.player_ai_corporation_bonus_val(PlayerId(0), bonus), 0);
        }
    }
}
