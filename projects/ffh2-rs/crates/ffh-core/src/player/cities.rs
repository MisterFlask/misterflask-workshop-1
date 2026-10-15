//! CvPlayer: cities -- founding, acquiring (conquest/trade), razing and disbanding,
//! city names, the capital, per-player city/plot-group lists, city counts, and goody huts
//! (exploration rewards).
//!
//! City names: C++ compares *localized* names (`CvCity::getName()` =
//! `getObjectText(m_szName, 0)`, `gDLL->getText(key)`) while cities store the text key
//! (`City::name` = `getNameKey`). Both sides are localized here with `Text::name` (as
//! `unit_init` does for great-person names), so the comparisons behave like the C++ ones.
//!
//! Python callbacks of CvGameUtils.py reached from this file that are no-ops in FFH2 0.41:
//! `canRazeCity` (always True), `doGoody` (always False = use the DLL default),
//! `cannotFoundCity` / `canFoundCitiesOnWater` (disabled by USE_CANNOT_FOUND_CITY_CALLBACK /
//! USE_CAN_FOUND_CITIES_ON_WATER_CALLBACK = 0, and they return False anyway).

#![allow(unused_imports)]

use crate::city::City;
use crate::controller::ConqueredCityChoice;
use crate::defs::NUM_CITY_PLOTS;
use crate::game::Game;
use crate::ids::*;
use crate::plot_group::PlotGroup;
use crate::structs::*;
use crate::utils::is_world_wonder_class;
use ffh_data::*;

/// C++ `GC.getCivilizationInfo(getCivilizationType())` for a player that must have a civ.
fn civ_of(g: &Game, p: PlayerId) -> CivilizationType {
    g.player_get_civilization_type(p).expect("player has no civilization")
}

/// C++ `CvCity::getName()` (= `getObjectText(m_szName, 0)`): the localized city name.
fn city_name(g: &Game, c: CityId) -> &str {
    g.d.text.name(&g.cities[c].name)
}

/// The building class of a building (C++ `getBuildingClassType()`, always set in the data).
fn building_class_of(d: &GameData, building: BuildingType) -> BuildingClassType {
    d.buildings[building].building_class_type.expect("building without a building class")
}

/// C++ `CvPlayerAI::AI_conquerCity(pCity)` (from `acquireCity`): an AI conqueror razes the
/// city (if allowed) or keeps it. The raze valuation is the controller's decision.
fn ai_conquer_city(g: &mut Game, p: PlayerId, c: CityId) {
    let mut raze = false;

    if g.player_can_raze(p, c) {
        // AI: the C++ raze value (elapsed turns, distance to our cities, financial trouble,
        // state religion and shrine, closeness, leader raze probability, traits) and its
        // `getSorenRandNum(100, "AI Raze City")` roll -> `Controller::raze_conquered_city`.
        let controller = g.controller(p);
        if controller.raze_conquered_city(g, p, c) {
            raze = true;
            g.city_do_task(c, TaskType::TASK_RAZE, -1, -1, false, false, false, false);
        }
    }

    if !raze {
        // UNOFFICIAL_PATCH 06/14/09: reports the AI player (BTS passed the active player).
        g.ev_city_acquired_and_kept(p, c);
    }
}

/// The human player's BUTTONPOPUP_RAZECITY popup (CvDLLButtonPopup `launchRazeCityPopup` +
/// `OnOkClicked`) as the controller's `conquered_city_choice`: keep (button 0: choose
/// production, `cityAcquiredAndKept`), raze (button 1, offered when `raze`: TASK_RAZE) or give
/// the city to its liberation player (button 2, offered when `gift_to` is set:
/// `cityAcquiredAndKept`, then TASK_GIFT). Button 3 ("let me see the city first") shows the
/// city and asks again: interface only. An answer whose button is not offered keeps the city.
fn raze_city_popup(g: &mut Game, p: PlayerId, c: CityId, raze: bool, gift_to: Option<PlayerId>) {
    // launchRazeCityPopup: PLAYER_ALWAYS_RAZES_CITIES (0 in FFH2) razes without asking.
    if g.d.defines.player_always_razes_cities != 0 {
        g.player_raze(p, c);
        return;
    }

    let controller = g.controller(p);
    match controller.conquered_city_choice(g, p, c, raze, gift_to) {
        ConqueredCityChoice::Raze if raze => {
            g.city_do_task(c, TaskType::TASK_RAZE, -1, -1, false, false, false, false);
        }
        ConqueredCityChoice::Gift if gift_to.is_some() => {
            // CvMessageControl::sendDoTask(TASK_GIFT, eHighestCulturePlayer): CvNetDoTask finds
            // the city among the (human) owner's cities.
            let to = gift_to.expect("gift offered");
            g.ev_city_acquired_and_kept(p, c);
            if g.cities.contains(c) && g.cities[c].owner == p {
                g.city_do_task(c, TaskType::TASK_GIFT, to.i(), -1, false, false, false, false);
            }
        }
        _ => {
            g.city_choose_production(c, None, None, None, false, false);
            g.ev_city_acquired_and_kept(p, c);
        }
    }
}

/// The human player's BUTTONPOPUP_DISBANDCITY popup for a city acquired without conquest or
/// trade (culture flip, FFH spells): disband (button 1, TASK_DISBAND) or keep (button 0: choose
/// production, `cityAcquiredAndKept`), the controller's `disband_acquired_city`.
fn disband_city_popup(g: &mut Game, p: PlayerId, c: CityId) {
    let controller = g.controller(p);
    if controller.disband_acquired_city(g, p, c) {
        g.city_do_task(c, TaskType::TASK_DISBAND, -1, -1, false, false, false, false);
    } else {
        g.city_choose_production(c, None, None, None, false, false);
        g.ev_city_acquired_and_kept(p, c);
    }
}

impl Game {
    // ------------------------------------------------------------------ city / plot group lists

    /// C++ `CvPlayer::addCity`: stores a new city object and appends it to the owner list.
    /// (C++ returns an uninitialized object that the caller then `init`s.)
    pub fn player_add_city(&mut self, p: PlayerId, city: City) -> CityId {
        let id = self.cities.insert(city);
        self.players[p].cities.push(id);
        id
    }

    /// C++ `CvPlayer::deleteCity(iID)`: destroys the city object and removes it from the owner list.
    pub fn player_delete_city(&mut self, p: PlayerId, c: CityId) {
        // A plot's `m_workingCity` may still name the dead city; C++ `getWorkingCity()` then
        // answers NULL. Clearing those references is equivalent and lets reads index directly.
        let (x, y) = (self.cities[c].x, self.cities[c].y);
        for i in 0..crate::defs::NUM_CITY_PLOTS {
            if let Some(q) = self.plot_city(x, y, i) {
                if self.map.plots[q].working_city == Some(c) {
                    self.map.plots[q].working_city = None;
                }
            }
        }
        self.cities.remove(c);
        self.players[p].cities.retain(|&x| x != c);
    }

    /// C++ `CvPlayer::addPlotGroup`.
    pub fn player_add_plot_group(&mut self, p: PlayerId, plot_group: PlotGroup) -> PlotGroupId {
        let id = self.plot_groups.insert(plot_group);
        self.players[p].plot_groups.push(id);
        id
    }

    /// C++ `CvPlayer::deletePlotGroup(iID)`.
    pub fn player_delete_plot_group(&mut self, p: PlayerId, pg: PlotGroupId) {
        self.plot_groups.remove(pg);
        self.players[p].plot_groups.retain(|&x| x != pg);
    }

    /// C++ `CvPlayer::initPlotGroup(pPlot)`: new plot group owned by `p` seeded at `plot`.
    pub fn player_init_plot_group(&mut self, p: PlayerId, plot: PlotIdx) -> PlotGroupId {
        // addPlotGroup() + CvPlotGroup::init(iID, eOwner, pPlot) = reset + addPlot.
        let id = self.plot_groups.next_id();
        let plot_group = self.player_add_plot_group(p, PlotGroup::new(self.d, id, p));
        debug_assert_eq!(plot_group, id);

        self.plot_group_add_plot(plot_group, plot);

        plot_group
    }

    /// C++ `CvPlayer::updatePlotGroups`: recalculates every plot group of the player (only
    /// once the game is final-initialized), then trade routes.
    pub fn player_update_plot_groups(&mut self, p: PlayerId) {
        if !self.final_initialized {
            return;
        }

        // C++ walks the live free list: groups deleted by an earlier recalculation (merged by
        // `combinePlotGroups`) are skipped. Groups created during the walk may also be visited
        // by C++; a group just rebuilt from connected plots is unchanged by `recalculatePlots`.
        for plot_group in self.players[p].plot_groups.clone() {
            if self.plot_groups.contains(plot_group) {
                self.plot_group_recalculate_plots(plot_group);
            }
        }

        for plot in self.map.plot_ids() {
            self.plot_update_plot_group_player(plot, p, false);
        }

        self.player_update_trade_routes(p);
    }

    // ------------------------------------------------------------------ founding / acquiring / razing

    /// C++ `CvPlayer::initCity(iX, iY, bBumpUnits, bUpdatePlotGroups)`: creates and inits a city.
    pub fn player_init_city(&mut self, p: PlayerId, x: i32, y: i32, bump_units: bool, update_plot_groups: bool) -> CityId {
        // addCity(): the new slot holds a default (reset) city until `init` re-runs `reset`.
        // Both "Crime" draws of the C++ (constructor + init) are made by `city_init`.
        let id = self.cities.next_id();
        let placeholder = City::new(self.d, self.num_players(), self.num_teams(), id, p, x, y, 0);
        let city = self.player_add_city(p, placeholder);
        debug_assert_eq!(city, id);

        debug_assert!(
            self.map.plot_xy(x, y).map_or(true, |plot| self.map.plots[plot].plot_city.is_none()),
            "No city is expected at this plot when initializing new city"
        );

        self.city_init(city, p, x, y, bump_units, update_plot_groups);

        city
    }

    /// C++ `CvPlayer::acquireCity(pCity, bConquest, bTrade, bUpdatePlotGroups)`: transfers
    /// `old_city` to `p` (kills it and re-creates it with the carried-over state); on
    /// conquest the raze decision goes to the controller (C++ `AI_conquerCity`) or a popup.
    pub fn player_acquire_city(&mut self, p: PlayerId, old_city: CityId, conquest: bool, trade: bool, update_plot_groups: bool) {
        let d = self.d;
        let team = self.players[p].team;

        let city_plot = self.city_plot(old_city);
        let (city_x, city_y) = self.map.xy(city_plot);
        // C++ `pOldCity->getOwnerINLINE()` / `getOriginalOwner()` (`eOldOwner` / `eOriginalOwner`;
        // nothing below changes them before the old city is killed).
        let old_owner = self.cities[old_city].owner;
        let original_owner = self.cities[old_city].original_owner;

        // C++ copies the plot's unit list first (`oldUnits`).
        let old_units = self.map.plots[city_plot].units.clone();
        for u in old_units {
            // C++ `::getUnit(IDInfo)` is NULL for a unit that is gone.
            if !self.units.contains(u) {
                continue;
            }
            if self.unit_get_team(u) != team && self.unit_get_domain_type(u) == DomainType::DOMAIN_IMMOBILE {
                self.unit_kill(u, false, Some(p));
            }
        }

        if conquest {
            let old_team = self.city_team(old_city);
            // `getCultureLevel()` as an int (NO_CULTURELEVEL = -1: no plots).
            let range = opt_to_i32(self.cities[old_city].culture_level);

            for dx in -range..=range {
                for dy in -range..=range {
                    if self.city_culture_distance(old_city, dx, dy) <= range {
                        let Some(loop_plot) = self.plot_xy(city_x, city_y, dx, dy) else {
                            continue;
                        };
                        if self.map.plots[loop_plot].owner == Some(old_owner)
                            && self.plot_get_num_culture_range_cities(loop_plot, old_owner) == 1
                        {
                            let mut force_unowned = false;

                            for i in self.player_ids() {
                                if self.players[i].alive {
                                    let loop_team = self.players[i].team;
                                    if loop_team != team
                                        && loop_team != old_team
                                        && self.plot_get_num_culture_range_cities(loop_plot, i) > 0
                                    {
                                        force_unowned = true;
                                        break;
                                    }
                                }
                            }

                            if force_unowned {
                                self.map.plots[loop_plot].force_unowned_timer = d.defines.force_unowned_city_timer;
                            }
                        }
                    }
                }
            }
        }

        // changeCitiesLost
        if original_owner == old_owner {
            self.players[original_owner].cities_lost += 1;
        } else if original_owner == p {
            self.players[original_owner].cities_lost -= 1;
        }

        if conquest {
            // C++: TXT_KEY_MISC_CAPTURED_CITY to us, TXT_KEY_MISC_CITY_CAPTURED_BY to every other
            // alive player that has revealed the city, and a replay message.
            if self.log.enabled {
                let turn = self.game_turn();
                let name = city_name(self, old_city).to_string();
                let old_owner_name = self.player_get_name(old_owner);
                let civ = self.player_get_civilization_description(p);
                self.log.msg(turn, Some(p), || format!("You have captured {name}!"));
                self.log.msg(turn, None, || format!("{name} ({old_owner_name}) was captured by the {civ}"));
            }
        }

        let mut capture_gold = 0;

        if conquest {
            // Python `doCityCaptureGold(pOldCity)`.
            capture_gold = self.py_do_city_capture_gold(old_city);
        }

        // changeGold (only UI side effects in C++).
        self.players[p].gold += capture_gold;

        for vote_source in d.vote_sources.keys() {
            self.city_process_vote_source_bonus(old_city, vote_source, false);
        }

        let highest_culture_player = self.city_find_highest_culture(old_city);
        let game_turn_founded = self.cities[old_city].game_turn_founded;
        let population = self.cities[old_city].population;
        let highest_population = self.cities[old_city].highest_population;
        let hurry_anger_timer = self.cities[old_city].hurry_anger_timer;
        let conscript_anger_timer = self.cities[old_city].conscript_anger_timer;
        let defy_resolution_anger_timer = self.cities[old_city].defy_resolution_anger_timer;
        let occupation_timer = self.cities[old_city].occupation_timer;
        let name = self.cities[old_city].name.clone();
        let damage = self.cities[old_city].defense_damage;
        // `iOldCityId`: `old_city` itself (ids are never reused).

        //FfH: Added by Kael 07/02/2008
        let mut civ = self.cities[old_city].civ;
        if opt_to_i32(civ) == d.defines.barbarian_civilization {
            civ = None;
        }
        //FfH: End Add

        let free_specialists: Vec<i32> =
            d.specialists.keys().map(|specialist| self.city_added_free_specialist_count(old_city, specialist)).collect();

        let mut ever_owned = self.cities[old_city].ever_owned.clone();
        let culture = self.cities[old_city].culture.clone();

        ever_owned[p] = true;

        let has_religion: Vec<bool> = d.religions.keys().map(|r| self.cities[old_city].has_religion[r]).collect();
        let holy_city: Vec<bool> = d.religions.keys().map(|r| self.city_is_holy_city(old_city, r)).collect();

        let has_corporation: Vec<bool> = d.corporations.keys().map(|corp| self.cities[old_city].has_corporation[corp]).collect();
        let headquarters: Vec<bool> = d.corporations.keys().map(|corp| self.city_is_headquarters(old_city, corp)).collect();

        let num_real_building = self.cities[old_city].num_real_building.clone();
        let building_original_owner = self.cities[old_city].building_original_owner.clone();
        let building_original_time = self.cities[old_city].building_original_time.clone();

        let mut building_yield_changes: Vec<BuildingYieldChange> = Vec::new();
        let mut building_commerce_changes: Vec<BuildingCommerceChange> = Vec::new();
        let mut building_happy_changes: BuildingChangeArray = Vec::new();
        let mut building_health_changes: BuildingChangeArray = Vec::new();
        for building_class in d.building_classes.keys() {
            for i in 0..YieldType::NUM {
                let yield_type = YieldType::from_usize(i);
                let change = self.city_building_yield_change(old_city, building_class, yield_type);
                if change != 0 {
                    building_yield_changes.push(BuildingYieldChange { building_class, yield_type, change });
                }
            }

            for i in 0..CommerceType::NUM {
                let commerce_type = CommerceType::from_usize(i);
                let change = self.city_building_commerce_change(old_city, building_class, commerce_type);
                if change != 0 {
                    building_commerce_changes.push(BuildingCommerceChange { building_class, commerce_type, change });
                }
            }

            let change = self.city_building_happy_change(old_city, building_class);
            if change != 0 {
                building_happy_changes.push((building_class, change));
            }

            let change = self.city_building_health_change(old_city, building_class);
            if change != 0 {
                building_health_changes.push((building_class, change));
            }
        }

        let recapture = highest_culture_player.is_some_and(|h| self.players[h].team == team);

        //FfH: Added by Kael 09/21/2008
        if self.players[old_owner].num_cities() == 1 {
            // changePlayersKilled(1)
            self.players[p].players_killed += 1;
        }
        //FfH: End Add

        self.city_kill(old_city, false);

        if trade {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(loop_plot) = self.plot_xy(city_x, city_y, dx, dy) {
                        self.plot_set_culture(loop_plot, old_owner, 0, false, false);
                    }
                }
            }
        }

        let new_city = self.player_init_city(p, city_x, city_y, !conquest, false);

        self.cities[new_city].previous_owner = Some(old_owner);
        self.cities[new_city].original_owner = original_owner;
        self.city_set_game_turn_founded(new_city, game_turn_founded);
        let new_population = if conquest && !recapture { (population - 1).max(1) } else { population };
        self.city_set_population(new_city, new_population);
        self.cities[new_city].highest_population = highest_population;
        self.city_set_name(new_city, &name, false);
        self.cities[new_city].never_lost = false;
        self.city_change_defense_damage(new_city, damage);

        //FfH: Added by Kael 07/05/2008
        if let Some(civ) = civ {
            // setCivilizationType
            self.cities[new_city].civ = Some(civ);
        }
        //FfH: End Add

        for i in self.player_ids() {
            self.cities[new_city].ever_owned[i] = ever_owned[i];
            self.city_set_culture_times100(new_city, i, culture[i], false, false);
        }

        let our_civ = civ_of(self, p);
        for building in d.buildings.keys() {
            let mut num = 0;

            if num_real_building[building] > 0 {
                let building_class = building_class_of(d, building);
                let new_building =
                    if is_world_wonder_class(d, building_class) { Some(building) } else { d.civ_building(our_civ, building_class) };

                if let Some(new_building) = new_building {
                    if trade || !d.buildings[building].never_capture {
                        if !self.player_is_production_maxed_building_class(p, building_class_of(d, new_building), true) {
                            if self.city_is_valid_building_location(new_city, new_building) {
                                // Short-circuit: the RNG is drawn only for a conquest that is no
                                // recapture.
                                if !conquest
                                    || recapture
                                    || self.soren_rand_num(100, "Capture Probability") < d.buildings[building].conquest_probability
                                {
                                    num += num_real_building[building];
                                }
                            }
                        }
                    }

                    // C++ calls this even when nothing is captured (`iNum == 0`).
                    let new_value = (self.cities[new_city].num_real_building[new_building] + num).min(d.defines.city_max_num_buildings);
                    self.city_set_num_real_building_timed(
                        new_city,
                        new_building,
                        new_value,
                        false,
                        building_original_owner[building],
                        building_original_time[building],
                    );
                }
            }
        }

        for change in &building_yield_changes {
            self.city_set_building_yield_change(new_city, change.building_class, change.yield_type, change.change);
        }

        for change in &building_commerce_changes {
            self.city_set_building_commerce_change(new_city, change.building_class, change.commerce_type, change.change);
        }

        for &(building_class, change) in &building_happy_changes {
            self.city_set_building_happy_change(new_city, building_class, change);
        }

        for &(building_class, change) in &building_health_changes {
            self.city_set_building_health_change(new_city, building_class, change);
        }

        for specialist in d.specialists.keys() {
            self.city_change_free_specialist_count(new_city, specialist, free_specialists[specialist.idx()]);
        }

        for religion in d.religions.keys() {
            if has_religion[religion.idx()] {
                self.city_set_has_religion(new_city, religion, true, false, true);
            }

            if holy_city[religion.idx()] {
                self.set_holy_city(religion, Some(new_city), false);
            }
        }

        for corporation in d.corporations.keys() {
            if has_corporation[corporation.idx()] {
                // C++ default `bArrows = true`.
                self.city_set_has_corporation(new_city, corporation, true, false, true);
            }

            if headquarters[corporation.idx()] {
                self.set_headquarters(corporation, Some(new_city), false);
            }
        }

        if trade {
            if self.players[p].human || team == self.players[old_owner].team {
                self.city_change_hurry_anger_timer(new_city, hurry_anger_timer);
                self.city_change_conscript_anger_timer(new_city, conscript_anger_timer);
                self.city_change_defy_resolution_anger_timer(new_city, defy_resolution_anger_timer);
            }

            if !recapture {
                self.city_change_occupation_timer(new_city, occupation_timer);
            }
        }

        if conquest {
            let team_culture_percent = self.city_calculate_team_culture_percent(new_city, team);

            if team_culture_percent < d.defines.occupation_culture_percent_threshold {
                let new_city_population = self.cities[new_city].population;
                let change = ((d.defines.base_occupation_turns
                    + ((new_city_population * d.defines.occupation_turns_population_percent) / 100))
                    * (100 - team_culture_percent))
                    / 100;
                self.city_change_occupation_timer(new_city, change);
            }

            self.map_verify_unit_valid_plot();
        }

        let old_team = self.players[old_owner].team;
        self.plot_set_revealed(city_plot, old_team, true, false, None, false);

        self.city_update_espionage_visibility(new_city, false);

        if update_plot_groups {
            self.update_plot_groups();
        }

        self.ev_city_acquired(old_owner, p, new_city, conquest, trade);

        // The C++ keeps using pNewCity; nothing above destroys it, but Python handlers are not
        // supposed to either. Stop safely if one did.
        if self.cities.contains(new_city) {
            if conquest {
                // Python `canRazeCity(iPlayer, pCity)` always returns True in FFH2 (lResult == 1).

                // auto raze based on game rules
                if self.city_is_auto_raze(new_city) {
                    if capture_gold > 0 {
                        // C++: TXT_KEY_MISC_PILLAGED_CITY message.
                        let turn = self.game_turn();
                        self.log.msg(turn, Some(p), || format!("Pillaged {} gold from {}", capture_gold, d.text.name(&name)));
                    }

                    self.city_do_task(new_city, TaskType::TASK_RAZE, -1, -1, false, false, false, false);
                } else if !self.players[p].human {
                    ai_conquer_city(self, p, new_city); // could delete the city
                } else {
                    // popup raze option (BUTTONPOPUP_RAZECITY)
                    let liberation_player = self.city_liberation_player(new_city, true);
                    let raze = self.player_can_raze(p, new_city);
                    let gift = liberation_player.is_some_and(|h| {
                        let h_team = self.players[h].team;
                        h != p && (team == h_team || self.team_is_open_borders(team, h_team) || self.teams[h_team].vassal[team])
                    });

                    if raze || gift {
                        raze_city_popup(self, p, new_city, raze, liberation_player.filter(|_| gift));
                    } else {
                        self.city_choose_production(new_city, None, None, None, false, false);
                        self.ev_city_acquired_and_kept(p, new_city);
                    }
                }
            } else if !trade {
                if self.players[p].human {
                    disband_city_popup(self, p, new_city);
                } else {
                    self.ev_city_acquired_and_kept(p, new_city);
                }
            }
        }

        // Forcing events that deal with the old city not to expire just because we conquered that city
        for trigger_data in self.players[p].events_occured.values_mut() {
            if trigger_data.other_player == Some(old_owner) && trigger_data.other_player_city == Some(old_city) {
                trigger_data.other_player_city = None;
            }
        }
    }

    /// C++ `CvPlayer::killCities`.
    pub fn player_kill_cities(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            if self.cities.contains(c) {
                self.city_kill(c, false);
            }
        }

        self.update_plot_groups();
    }

    /// C++ `CvPlayer::canFound(iX, iY, bTestVisible = false)`.
    pub fn player_can_found(&self, p: PlayerId, x: i32, y: i32, test_visible: bool) -> bool {
        let d = self.d;

        // C++ dereferences the plot without a NULL check; off the map there is nothing to found on.
        let Some(plot) = self.map.plot_xy(x, y) else {
            return false;
        };

        // `if (GC.getUSE_CANNOT_FOUND_CITY_CALLBACK())`: Python `cannotFoundCity(iPlayer, iX, iY)`
        // (the define is 0 in FFH2, and the function returns False anyway).

        if self.final_initialized
            && self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE)
            && self.players[p].human
            && self.players[p].num_cities() > 0
        {
            return false;
        }

        if self.plot_is_impassable(plot) {
            return false;
        }

        if let Some(feature) = self.map.plots[plot].feature_type {
            if d.features[feature].no_city {
                return false;
            }
        }

        if self.plot_is_owned(plot) && self.map.plots[plot].owner != Some(p) {
            return false;
        }

        let terrain = self.map.plots[plot].terrain_type.expect("plot without terrain");
        let terrain_info = &d.terrains[terrain];

        let mut valid = false;

        if !valid && terrain_info.found {
            valid = true;
        }

        if !valid && terrain_info.found_coast && self.plot_is_coastal_land(plot, -1) {
            valid = true;
        }

        if !valid && terrain_info.found_fresh_water && self.plot_is_fresh_water(plot) {
            valid = true;
        }

        // UNOFFICIAL_PATCH 10/04/09 (EmperorFool & jdog5000): `if (!bValid && pPlot->isWater())`
        // and `GC.getUSE_CAN_FOUND_CITIES_ON_WATER_CALLBACK()`, Python `canFoundCitiesOnWater(iX,
        // iY)` could validate a water plot (the define is 0 in FFH2; the function returns False).

        if !valid {
            return false;
        }

        if !test_visible {
            let range = d.defines.min_city_range;
            let area = self.map.plots[plot].area;

            for dx in -range..=range {
                for dy in -range..=range {
                    if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                        if self.map.plots[loop_plot].plot_city.is_some() && self.map.plots[loop_plot].area == area {
                            return false;
                        }
                    }
                }
            }
        }

        //FfH: Added by Kael 04/06/2010
        if let Some(improvement) = self.map.plots[plot].improvement_type {
            if d.improvements[improvement].permanent {
                return false;
            }
        }
        if let Some(bonus) = self.map.plots[plot].bonus_type {
            let bonus_class = opt_to_i32(d.bonuses[bonus].bonus_class_type);
            if d.defines.bonusclass_mana != -1 && bonus_class == d.defines.bonusclass_mana {
                return false;
            }
            if d.defines.bonusclass_mana_raw != -1 && bonus_class == d.defines.bonusclass_mana_raw {
                return false;
            }
        }
        if self.plot_is_visible_enemy_unit(plot, p) {
            return false;
        }
        //FfH: End Add

        true
    }

    /// C++ `CvPlayer::found(iX, iY)`: founds a city (barbarian defenders, free start-era
    /// buildings, advanced-start Creative culture, Python `cityBuilt`).
    pub fn player_found(&mut self, p: PlayerId, x: i32, y: i32) {
        let d = self.d;

        if !self.player_can_found(p, x, y, false) {
            return;
        }

        let city = self.player_init_city(p, x, y, true, true);

        if self.player_is_barbarian(p) {
            // AI: C++ `pCity->AI_bestUnitAI(UNITAI_CITY_DEFENSE)`, else
            // `AI_bestUnitAI(UNITAI_ATTACK)`: the best trainable unit by `AI_unitValue`, with an
            // "AI Best Unit" soren draw per candidate. There is no controller decision for it
            // (SIGNATURE: needs e.g. `Controller::best_unit_ai(g, c, unit_ai) -> Option<UnitType>`).
            // Stand-in: `CvCity::getConscriptUnit`, the rules-side pick FfH itself substitutes
            // for this same pair of calls in `CvPlot::setOwner` (Kael 01/23/2009).
            let defender_unit = self.city_conscript_unit(city);

            if let Some(defender_unit) = defender_unit {
                for _ in 0..d.handicaps[self.handicap].barbarian_initial_defenders {
                    self.player_init_unit(p, defender_unit, x, y, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
                }
            }
        }

        let civ = civ_of(self, p);
        for building_class in d.building_classes.keys() {
            let Some(building) = d.civ_building(civ, building_class) else {
                continue;
            };

            if let Some(free_start_era) = d.buildings[building].free_start_era {
                if self.start_era() >= free_start_era && self.city_can_construct(city, building, false, false, false) {
                    self.city_set_num_real_building(city, building, 1);
                }
            }
        }

        if self.players[p].advanced_start_points >= 0 {
            // Free border expansion for Creative
            let mut creative = false;
            for t in d.traits.keys() {
                if self.players[p].traits[t] && d.traits[t].commerce_change[CommerceType::COMMERCE_CULTURE.idx()] > 0 {
                    creative = true;
                    break;
                }
            }

            if creative {
                for level in d.culture_levels.keys() {
                    let culture = self.culture_threshold(level);
                    if culture > 0 {
                        self.city_set_culture(city, p, culture, true, true);
                        break;
                    }
                }
            }
        }

        if self.players[p].human && self.players[p].advanced_start_points < 0 {
            self.city_choose_production(city, None, None, None, false, false);
        } else {
            self.city_do_found_message(city);
        }

        self.ev_city_built(city);
    }

    /// C++ `CvPlayer::canRaze(pCity)` (also Python `canRazeCity`).
    pub fn player_can_raze(&self, p: PlayerId, city: CityId) -> bool {
        if !self.city_is_auto_raze(city) {
            if self.is_option(GameOptionType::GAMEOPTION_NO_CITY_RAZING) {
                return false;
            }

            if self.cities[city].owner != p {
                return false;
            }

            if self.city_calculate_team_culture_percent(city, self.players[p].team) >= self.d.defines.razing_cultural_percent_threshold {
                return false;
            }
        }

        // Python `canRazeCity(iPlayer, pCity)` always returns True in FFH2 (never lResult == 0).

        true
    }

    /// C++ `CvPlayer::raze(pCity)`: memories, FFH global counter and equipment units, then `disband`.
    pub fn player_raze(&mut self, p: PlayerId, city: CityId) {
        let d = self.d;

        if !self.player_can_raze(p, city) {
            return;
        }

        debug_assert!(self.cities[city].owner == p);

        let team = self.players[p].team;

        if let Some(highest_culture_player) = self.city_find_highest_culture(city) {
            if self.players[highest_culture_player].team != team {
                // AI_changeMemoryCount(getID(), MEMORY_RAZED_CITY, 1)
                self.players[highest_culture_player].ai_memory_count[p][MemoryType::MEMORY_RAZED_CITY] += 1;
            }
        }

        for religion in d.religions.keys() {
            if self.city_is_holy_city(city, religion) {
                for j in self.player_ids() {
                    if self.players[j].alive && j != p && self.player_get_state_religion(j) == Some(religion) {
                        // AI_changeMemoryCount(getID(), MEMORY_RAZED_HOLY_CITY, 1)
                        self.players[j].ai_memory_count[p][MemoryType::MEMORY_RAZED_HOLY_CITY] += 1;
                    }
                }
            }
        }

        // C++: TXT_KEY_MISC_DESTROYED_CITY to us, TXT_KEY_MISC_CITY_HAS_BEEN_RAZED_BY to the
        // other alive players that revealed the city, and a replay message.
        if self.log.enabled {
            let turn = self.game_turn();
            let name = city_name(self, city).to_string();
            let civ = self.player_get_civilization_description(p);
            self.log.msg(turn, Some(p), || format!("You have destroyed {name}!"));
            self.log.msg(turn, None, || format!("{name} was razed by the {civ}"));
        }

        //FfH Global Counter: Added by Kael 08/11/2007
        let mut valid = true;
        for religion in d.religions.keys() {
            if self.cities[city].has_religion[religion] && d.religions[religion].global_counter_spread > 0 {
                valid = false;
            }
        }
        if valid {
            let population = self.cities[city].population;
            let mut cost = d.defines.raze_cost_base;
            if (population - d.defines.raze_cost_per_population_over) > 0 {
                cost += (population - d.defines.raze_cost_per_population_over) * d.defines.raze_cost_per_population;
            }
            self.change_global_counter(cost);
        }
        let (x, y) = (self.cities[city].x, self.cities[city].y);
        for building in d.buildings.keys() {
            if self.cities[city].num_real_building[building] > 0 && d.buildings[building].equipment {
                for unit in d.units.keys() {
                    if d.units[unit].buildings[building] {
                        self.player_init_unit(p, unit, x, y, None, None);
                    }
                }
            }
        }
        //FfH: End Add

        // Report this event
        self.ev_city_razed(city, p);

        self.player_disband(p, city);
    }

    /// C++ `CvPlayer::disband(pCity)`.
    pub fn player_disband(&mut self, p: PlayerId, city: CityId) {
        if self.players[p].num_cities() == 1 {
            // setFoundedFirstCity(false): plain store (+ UI dirty bits)
            self.players[p].founded_first_city = false;
        }

        // addDestroyedCityName(pCity->getName())
        let name = city_name(self, city).to_string();
        self.destroyed_cities.push(name);

        self.city_kill(city, true);
    }

    /// C++ `CvPlayer::pickConqueredCityOwner(kCity)`: the liberation player if we are its
    /// vassal (C++ `GET_TEAM(getTeam()).isVassal(<liberation player's team>)`), else `p`.
    pub fn player_pick_conquered_city_owner(&self, p: PlayerId, city: CityId) -> PlayerId {
        if let Some(best_player) = self.city_liberation_player(city, true) {
            let best_team = self.players[best_player].team;
            if self.teams[self.players[p].team].vassal[best_team] {
                return best_player;
            }
        }

        p
    }

    // ------------------------------------------------------------------ city names

    /// C++ `CvPlayer::getNewCityName` (const in C++, but draws from the game RNG).
    pub fn player_get_new_city_name(&mut self, p: PlayerId) -> String {
        let d = self.d;
        let mut name = String::new();

        for key in &self.players[p].city_names {
            // C++ quirk: `szName` keeps the localized text of the last entry when none of the
            // player's custom names is valid, so the fallbacks below are skipped and that
            // (already used) name is returned.
            name = d.text.name(key).to_string();
            if self.player_is_city_name_valid(p, &name, true) {
                name = key.clone();
                break;
            }
        }

        if name.is_empty() {
            if let Some(civ_name) = self.player_get_civilization_city_name(p, civ_of(self, p)) {
                name = civ_name;
            }
        }

        if name.is_empty() {
            // Pick a name from another random civ
            let num_civs = d.civilizations.len() as i32;
            let rand_offset = self.soren_rand_num(num_civs, "Place Units (Player)");
            for i in 0..num_civs {
                let loop_civ = CivilizationType::from_usize(((i + rand_offset) % num_civs) as usize);

                if let Some(civ_name) = self.player_get_civilization_city_name(p, loop_civ) {
                    name = civ_name;
                }

                if !name.is_empty() {
                    break;
                }
            }
        }

        if name.is_empty() {
            name = "TXT_KEY_CITY".to_string();
        }

        name
    }

    /// C++ `CvPlayer::getCivilizationCityName(szBuffer, eCivilization)`: first valid city
    /// name of `civ` (None = the C++ buffer is left unchanged). Const in C++ but barbarian
    /// and minor civs draw a random offset from the game RNG.
    pub fn player_get_civilization_city_name(&mut self, p: PlayerId, civ: CivilizationType) -> Option<String> {
        let d = self.d;
        let city_names = &d.civilizations[civ].city_names;
        let num_city_names = city_names.len() as i32;

        // C++ draws even when the civ has no city names (`getSorenRandNum(0, ..)` = 0).
        let rand_offset = if self.player_is_barbarian(p) || self.player_is_minor_civ(p) {
            self.soren_rand_num(num_city_names, "Place Units (Player)")
        } else {
            0
        };

        for i in 0..num_city_names {
            let loop_name = ((i + rand_offset) % num_city_names) as usize;

            let name = d.text.name(&city_names[loop_name]);

            if self.player_is_city_name_valid(p, name, true) {
                return Some(city_names[loop_name].clone());
            }
        }

        None
    }

    /// C++ `CvPlayer::isCityNameValid(szName, bTestDestroyed = true)`.
    pub fn player_is_city_name_valid(&self, p: PlayerId, name: &str, test_destroyed: bool) -> bool {
        if test_destroyed {
            if self.is_destroyed_city_name(name) {
                return false;
            }

            for i in self.player_ids() {
                for &c in &self.players[i].cities {
                    if city_name(self, c) == name {
                        return false;
                    }
                }
            }
        } else {
            for &c in &self.players[p].cities {
                if city_name(self, c) == name {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvPlayer::getCityName(iIndex)`: entry `index` of `city_names`, "" if out of range.
    pub fn player_get_city_name(&self, p: PlayerId, index: i32) -> String {
        if index < 0 {
            return String::new();
        }
        self.players[p].city_names.get(index as usize).cloned().unwrap_or_default()
    }

    // ------------------------------------------------------------------ capital

    /// C++ `CvPlayer::getCapitalCity`: `capital_city` if that city still exists (C++ looks
    /// the id up in the owner's city list, so a deleted capital reads as NULL). City ids
    /// are never reused and a city never changes owner without being re-created.
    pub fn player_get_capital_city(&self, p: PlayerId) -> Option<CityId> {
        self.players[p].capital_city.filter(|&c| self.cities.contains(c))
    }

    /// C++ `CvPlayer::setCapitalCity(pNewCapitalCity)`: plot group bonuses, maintenance,
    /// trade routes and both cities' commerce.
    pub fn player_set_capital_city(&mut self, p: PlayerId, city: Option<CityId>) {
        let old_capital_city = self.player_get_capital_city(p);

        if old_capital_city == city {
            return;
        }

        let update_plot_groups = match (old_capital_city, city) {
            (Some(old), Some(new)) => {
                self.plot_get_owner_plot_group(self.city_plot(old)) != self.plot_get_owner_plot_group(self.city_plot(new))
            }
            _ => true,
        };

        if update_plot_groups {
            if let Some(old) = old_capital_city {
                self.plot_update_plot_group_bonus(self.city_plot(old), false);
            }
            if let Some(new) = city {
                self.plot_update_plot_group_bonus(self.city_plot(new), false);
            }
        }

        // C++ `m_iCapitalCityID = pNewCapitalCity->getID()` or `FFreeList::INVALID_INDEX`.
        self.players[p].capital_city = city;

        if update_plot_groups {
            if let Some(old) = old_capital_city {
                self.plot_update_plot_group_bonus(self.city_plot(old), true);
            }
            if let Some(new) = city {
                self.plot_update_plot_group_bonus(self.city_plot(new), true);
            }
        }

        self.player_update_maintenance(p);
        self.player_update_trade_routes(p);

        // `setInfoDirty(true)` is UI.
        if let Some(old) = old_capital_city {
            self.city_update_commerces(old);
        }
        if let Some(new) = city {
            self.city_update_commerces(new);
        }
    }

    /// C++ `CvPlayer::findNewCapital`: moves the capital building to the best other city.
    pub fn player_find_new_capital(&mut self, p: PlayerId) {
        let d = self.d;

        let capital_building =
            opt_from_i32::<BuildingClassType>(d.defines.capital_buildingclass).and_then(|class| d.civ_building(civ_of(self, p), class));
        let Some(capital_building) = capital_building else {
            return;
        };

        let old_capital = self.player_get_capital_city(p);

        let mut best_value = 0;
        let mut best_city = None;

        for &c in &self.players[p].cities {
            if Some(c) != old_capital && self.cities[c].num_real_building[capital_building] == 0 {
                let city = &self.cities[c];

                let mut value = city.population * 4;

                value += self.city_yield_rate(c, YieldType::YIELD_FOOD);
                value += self.city_yield_rate(c, YieldType::YIELD_PRODUCTION) * 3;
                value += self.city_yield_rate(c, YieldType::YIELD_COMMERCE) * 2;
                value += opt_to_i32(city.culture_level);
                value += self.city_religion_count(c);
                value += self.city_corporation_count(c);
                value += city.num_great_people * 2;

                value *= self.city_calculate_culture_percent(c, p) + 100;
                value /= 100;

                if value > best_value {
                    best_value = value;
                    best_city = Some(c);
                }
            }
        }

        if let Some(best_city) = best_city {
            if let Some(old_capital) = old_capital {
                self.city_set_num_real_building(old_capital, capital_building, 0);
            }
            debug_assert!(
                self.cities[best_city].num_real_building[capital_building] == 0,
                "(pBestCity->getNumRealBuilding(eCapitalBuilding)) did not return false as expected"
            );
            self.city_set_num_real_building(best_city, capital_building, 1);
        }
    }

    /// C++ `CvPlayer::getNumGovernmentCenters`.
    pub fn player_get_num_government_centers(&self, p: PlayerId) -> i32 {
        self.players[p].cities.iter().filter(|&&c| self.city_is_government_center(c)).count() as i32
    }

    // ------------------------------------------------------------------ counts over cities

    /// C++ `CvPlayer::countNumCoastalCities`.
    pub fn player_count_num_coastal_cities(&self, p: PlayerId) -> i32 {
        let min_water_size = self.d.defines.min_water_size_for_ocean;
        self.players[p].cities.iter().filter(|&&c| self.city_is_coastal(c, min_water_size)).count() as i32
    }

    /// C++ `CvPlayer::countNumCoastalCitiesByArea(pArea)`.
    pub fn player_count_num_coastal_cities_by_area(&self, p: PlayerId, area: AreaId) -> i32 {
        let min_water_size = self.d.defines.min_water_size_for_ocean;
        let mut count = 0;

        for &c in &self.players[p].cities {
            if self.city_is_coastal(c, min_water_size)
                && (self.city_area(c) == area || self.plot_is_adjacent_to_area(self.city_plot(c), area))
            {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvPlayer::countTotalCulture` (sum of own culture in own cities, /100).
    pub fn player_count_total_culture(&self, p: PlayerId) -> i32 {
        let count: i32 = self.players[p].cities.iter().map(|&c| self.cities[c].culture[p]).sum();
        count / 100
    }

    /// C++ `CvPlayer::countCityFeatures(eFeature)`.
    pub fn player_count_city_features(&self, p: PlayerId, feature: FeatureType) -> i32 {
        let mut count = 0;

        for &c in &self.players[p].cities {
            let (x, y) = (self.cities[c].x, self.cities[c].y);
            for i in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(x, y, i) {
                    if self.map.plots[loop_plot].feature_type == Some(feature) {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvPlayer::countNumBuildings(eBuilding)`.
    pub fn player_count_num_buildings(&self, p: PlayerId, building: BuildingType) -> i32 {
        let mut count = 0;

        for &c in &self.players[p].cities {
            let num = self.city_num_building(c, building);
            if num > 0 {
                count += num;
            }
        }

        count
    }

    /// C++ `CvPlayer::countNumCitiesConnectedToCapital`.
    pub fn player_count_num_cities_connected_to_capital(&self, p: PlayerId) -> i32 {
        self.players[p].cities.iter().filter(|&&c| self.city_is_connected_to_capital(c, None)).count() as i32
    }

    /// C++ `CvPlayer::getNumBuilding(iBuilding)` (FFH: real buildings of a type over all cities).
    pub fn player_get_num_building(&self, p: PlayerId, building: BuildingType) -> i32 {
        self.players[p].cities.iter().map(|&c| self.cities[c].num_real_building[building]).sum()
    }

    /// C++ `CvPlayer::getNumSettlements` (FFH: cities that are settlements).
    pub fn player_get_num_settlements(&self, p: PlayerId) -> i32 {
        self.players[p].cities.iter().filter(|&&c| self.cities[c].settlement).count() as i32
    }

    // ------------------------------------------------------------------ goody huts

    /// C++ `CvPlayer::canReceiveGoody(pPlot, eGoody, pUnit)`.
    pub fn player_can_receive_goody(&self, p: PlayerId, plot: PlotIdx, goody: GoodyType, unit: Option<UnitId>) -> bool {
        let d = self.d;
        let info = &d.goodies[goody];

        if info.experience > 0 {
            match unit {
                Some(u) if self.unit_can_acquire_promotion_any(u) && self.elapsed_game_turns >= 10 => {}
                _ => return false,
            }
        }

        if info.damage_prereq > 0 {
            match unit {
                Some(u) if self.units[u].damage >= (self.unit_max_hit_points(u) * info.damage_prereq) / 100 => {}
                _ => return false,
            }
        }

        if info.tech {
            let mut tech_found = false;

            for tech in d.techs.keys() {
                if d.techs[tech].goody_tech && self.player_can_research(p, tech, false) {
                    tech_found = true;
                    break;
                }
            }

            if !tech_found {
                return false;
            }
        }

        if info.bad {
            match unit {
                Some(u) if !self.unit_is_no_bad_goodies(u) => {}
                _ => return false,
            }
        }

        if let Some(unit_class) = info.unit_class_type {
            let Some(unit_type) = d.civ_unit(civ_of(self, p), unit_class) else {
                return false;
            };

            if d.units[unit_type].combat > 0
                && !d.units[unit_type].only_defensive
                && (self.is_game_multi_player() || self.elapsed_game_turns < 20)
            {
                return false;
            }

            if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.players[p].human && d.units[unit_type].found {
                return false;
            }
        }

        if info.barbarian_unit_class.is_some() {
            if self.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS) {
                return false;
            }

            let num_cities = self.players[p].num_cities();

            if num_cities == 0 {
                return false;
            }

            if num_cities == 1 {
                let (x, y) = self.map.xy(plot);
                let team = self.players[p].team;
                if let Some(city) = self.map_find_city(x, y, None, Some(team), true, false, None, None, None) {
                    if self.plot_distance(x, y, self.cities[city].x, self.cities[city].y) <= (8 - num_cities) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// C++ `CvPlayer::receiveGoody(pPlot, eGoody, pUnit)`.
    pub fn player_receive_goody(&mut self, p: PlayerId, plot: PlotIdx, goody: GoodyType, unit: Option<UnitId>) {
        let d = self.d;
        let info = &d.goodies[goody];
        let team = self.players[p].team;
        let (plot_x, plot_y) = self.map.xy(plot);

        // FAssert(canReceiveGoody(..)) is commented out in FfH: "graveyards can also give goodys Sephi".

        // C++ quirk: MSVC evaluation order (left operand of `+` first: "Goody Gold 1" draws first).
        let mut gold = info.gold;
        gold += self.soren_rand_num(info.gold_rand1, "Goody Gold 1");
        gold += self.soren_rand_num(info.gold_rand2, "Goody Gold 2");
        gold = (gold * d.game_speeds[self.game_speed_type()].growth_percent) / 100;

        if gold != 0 {
            // changeGold
            self.players[p].gold += gold;
        }

        // C++: the goody description (+ TXT_KEY_MISC_RECEIVED_GOLD) as a message to us.
        let turn = self.game_turn();
        self.log.msg(turn, Some(p), || {
            let description = d.text.name(&info.base.description);
            if gold != 0 {
                format!("{description}: received {gold} gold")
            } else {
                description.to_string()
            }
        });

        let range = info.map_range;

        if range > 0 {
            let offset = info.map_offset;
            let mut best_plot = None;

            if offset > 0 {
                let mut best_value = 0;

                for dx in -offset..=offset {
                    for dy in -offset..=offset {
                        let Some(loop_plot) = self.plot_xy(plot_x, plot_y, dx, dy) else {
                            continue;
                        };
                        if !self.map.plots[loop_plot].revealed[team] {
                            let mut value = 1 + self.soren_rand_num(10000, "Goody Map");

                            let (loop_x, loop_y) = self.map.xy(loop_plot);
                            value *= self.plot_distance(plot_x, plot_y, loop_x, loop_y);

                            if value > best_value {
                                best_value = value;
                                best_plot = Some(loop_plot);
                            }
                        }
                    }
                }
            }

            let best_plot = best_plot.unwrap_or(plot);
            let (best_x, best_y) = self.map.xy(best_plot);

            for dx in -range..=range {
                for dy in -range..=range {
                    let Some(loop_plot) = self.plot_xy(best_x, best_y, dx, dy) else {
                        continue;
                    };
                    let (loop_x, loop_y) = self.map.xy(loop_plot);
                    if self.plot_distance(best_x, best_y, loop_x, loop_y) <= range
                        && self.soren_rand_num(100, "Goody Map") < info.map_prob
                    {
                        self.plot_set_revealed(loop_plot, team, true, false, None, true);
                    }
                }
            }
        }

        if let Some(u) = unit {
            //FfH: Modified by Kael 08/31/2008
            if info.unit_class_type.is_none() && info.barbarian_unit_class.is_none() {
                self.unit_change_experience(u, info.experience, -1, false, false, false);
            }
            //FfH: End Modify
        }

        if let Some(u) = unit {
            self.unit_change_damage(u, -info.healing, None);
        }

        if info.tech {
            let mut best_value = 0;
            let mut best_tech = None;

            for tech in d.techs.keys() {
                if d.techs[tech].goody_tech && self.player_can_research(p, tech, false) {
                    let value = 1 + self.soren_rand_num(10000, "Goody Tech");

                    if value > best_value {
                        best_value = value;
                        best_tech = Some(tech);
                    }
                }
            }

            // C++ FAssert(eBestTech != NO_TECH) and then indexes the tech arrays with NO_TECH;
            // skipped here (reachable through spells that skip `canReceiveGoody`).
            if let Some(best_tech) = best_tech {
                self.team_set_has_tech(team, best_tech, true, Some(p), true, true);
                // setNoTradeTech
                self.teams[team].no_trade_tech[best_tech] = true;
            }
        }

        if let Some(unit_class) = info.unit_class_type {
            //FfH: Modified by Kael 08/31/2008 (falls back to the class default unit)
            let unit_type = d.civ_unit(civ_of(self, p), unit_class).or(d.unit_classes[unit_class].default_unit_index);
            // C++ initUnit(NO_UNIT) has no defined result; every goody class has a default unit.
            if let Some(unit_type) = unit_type {
                let new_unit = self.player_init_unit(p, unit_type, plot_x, plot_y, None, None);
                if self.units.contains(new_unit) {
                    self.unit_change_experience(new_unit, info.experience, -1, false, false, false);
                }
            }
            //FfH: End Modify
        }

        if let Some(barbarian_unit_class) = info.barbarian_unit_class {
            let mut barb_count = 0;

            let barbarian = self.barbarian_player();
            let unit_type = d.civ_unit(civ_of(self, barbarian), barbarian_unit_class);

            if let Some(unit_type) = unit_type {
                let area = self.map.plots[plot].area;

                for pass in 0..2 {
                    if barb_count < info.min_barbarians {
                        for i in 0..DirectionType::NUM {
                            let Some(loop_plot) = self.plot_direction(plot_x, plot_y, DirectionType::from_usize(i)) else {
                                continue;
                            };

                            if self.map.plots[loop_plot].area == area
                                && !self.plot_is_impassable(loop_plot)
                                && self.map.plots[loop_plot].units.is_empty()
                                && (pass > 0 || self.soren_rand_num(100, "Goody Barbs") < info.barbarian_unit_prob)
                            {
                                //FfH: Modified by Kael 08/31/2008
                                let unit_ai = if self.plot_is_water(loop_plot) {
                                    UnitAIType::UNITAI_ATTACK_SEA
                                } else {
                                    UnitAIType::UNITAI_ATTACK
                                };
                                let (loop_x, loop_y) = self.map.xy(loop_plot);
                                let new_unit = self.player_init_unit(barbarian, unit_type, loop_x, loop_y, Some(unit_ai), None);
                                if self.units.contains(new_unit) {
                                    self.unit_change_experience(new_unit, info.experience, -1, false, false, false);
                                }
                                //FfH: End Modify

                                barb_count += 1;

                                if pass > 0 && barb_count == info.min_barbarians {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::doGoody(pPlot, pUnit)` (Python `doGoody` override, then handicap goodies).
    pub fn player_do_goody(&mut self, p: PlayerId, plot: PlotIdx, unit: Option<UnitId>) {
        let d = self.d;

        // Python `doGoody(iPlayer, pPlot, pUnit)` returns False in FFH2: the DLL handles it.

        // FAssertMsg(pPlot->isGoody(), ..)

        self.plot_remove_goody(plot);

        if !self.player_is_barbarian(p) {
            for _ in 0..d.defines.num_do_goody_attempts {
                let handicap = self.player_get_handicap_type(p).expect("player has no handicap");
                let goodies = &d.handicaps[handicap].goodies;

                if !goodies.is_empty() {
                    let goody = goodies[self.soren_rand_num(goodies.len() as i32, "Goodies") as usize];

                    if self.player_can_receive_goody(p, plot, goody, unit) {
                        self.player_receive_goody(p, plot, goody, unit);

                        // Python Event
                        self.ev_goody_received(p, plot, unit, goody);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// A game whose first `civs.len()` slots hold freshly reset, alive players with the given
    /// civilizations/leaders (each on its own team). No map: only list/field-based functions
    /// can be exercised.
    fn game(civs: &[(&str, &str)]) -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, civs.len(), "WORLDSIZE_DUEL", 11);
        for (i, (civ, leader)) in civs.iter().enumerate() {
            setup.players[i].civ = Some(d.t::<CivilizationType>(civ));
            setup.players[i].leader = Some(d.t::<LeaderHeadType>(leader));
        }
        let mut g = Game::new(d, setup);
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for i in 0..civs.len() {
            let p = PlayerId(i as u8);
            g.players[p] = Player::new(d, p, &g.setup.players[i], n_players, n_teams);
            g.players[p].alive = true;
        }
        g
    }

    /// A bare (reset) city of `p` named `name`, registered in the owner list.
    fn add_city(g: &mut Game, p: PlayerId, name: &str) -> CityId {
        let id = g.cities.next_id();
        let mut city = City::new(g.d, g.num_players(), g.num_teams(), id, p, 0, 0, 0);
        city.name = name.to_string();
        g.player_add_city(p, city)
    }

    #[test]
    fn city_name_validity_compares_localized_names() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let bannor = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        let key = d.civilizations[bannor].city_names[0].clone();
        let text = d.text.name(&key).to_string();
        assert_ne!(key, text, "city name keys have a translation");

        add_city(&mut g, p1, &key);
        // Cities store keys and are compared by their localized name.
        assert!(!g.player_is_city_name_valid(p0, &text, true));
        assert!(g.player_is_city_name_valid(p0, &key, true));
        // Without bTestDestroyed only our own cities count.
        assert!(g.player_is_city_name_valid(p0, &text, false));
        assert!(!g.player_is_city_name_valid(p1, &text, false));

        g.destroyed_cities.push("Ruinsville".to_string());
        assert!(!g.player_is_city_name_valid(p0, "Ruinsville", true));
        assert!(g.player_is_city_name_valid(p0, "Ruinsville", false));
    }

    #[test]
    fn new_city_names_follow_the_civ_list_without_rng() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let p = PlayerId(0);
        let bannor = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        let names = &d.civilizations[bannor].city_names;
        let seed = g.soren_rand.seed();

        let first = g.player_get_new_city_name(p);
        assert_eq!(first, names[0]);
        add_city(&mut g, p, &first);
        assert_eq!(g.player_get_new_city_name(p), names[1]);
        // A destroyed name is skipped as well.
        g.destroyed_cities.push(d.text.name(&names[1]).to_string());
        assert_eq!(g.player_get_new_city_name(p), names[2]);
        assert_eq!(g.soren_rand.seed(), seed, "civ players take names in order");
    }

    #[test]
    fn custom_city_names_come_first_and_keep_the_cpp_fallthrough() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let p = PlayerId(0);
        g.players[p].city_names = vec!["Alpha".to_string(), "Beta".to_string()];
        assert_eq!(g.player_get_new_city_name(p), "Alpha");
        add_city(&mut g, p, "Alpha");
        assert_eq!(g.player_get_new_city_name(p), "Beta");
        add_city(&mut g, p, "Beta");
        // C++ quirk: all custom names used -> the last one is returned again.
        assert_eq!(g.player_get_new_city_name(p), "Beta");
    }

    #[test]
    fn barbarian_city_names_start_at_a_random_offset() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")]);
        let d = g.d;
        let barbarian = g.barbarian_player();
        let bannor = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        let names = &d.civilizations[bannor].city_names;

        let mut expected_rng = g.soren_rand.clone();
        let offset = expected_rng.get_i(names.len() as i32, "Place Units (Player)") as usize;

        let name = g.player_get_civilization_city_name(barbarian, bannor);
        assert_eq!(name.as_deref(), Some(names[offset].as_str()));
        assert_eq!(g.soren_rand, expected_rng, "exactly one draw");

        // Non-barbarian players never draw.
        let seed = g.soren_rand.seed();
        assert_eq!(g.player_get_civilization_city_name(PlayerId(0), bannor).as_deref(), Some(names[0].as_str()));
        assert_eq!(g.soren_rand.seed(), seed);
    }

    #[test]
    fn counts_over_cities() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")]);
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let a = add_city(&mut g, p0, "A");
        let b = add_city(&mut g, p0, "B");
        let c = add_city(&mut g, p1, "C");

        g.cities[a].culture[p0] = 250;
        g.cities[b].culture[p0] = 199;
        g.cities[b].culture[p1] = 1000;
        g.cities[c].culture[p0] = 5000;
        // Only our culture in our cities, divided by 100 at the end.
        assert_eq!(g.player_count_total_culture(p0), 4);

        g.cities[b].settlement = true;
        assert_eq!(g.player_get_num_settlements(p0), 1);
        assert_eq!(g.player_get_num_settlements(p1), 0);

        let building = BuildingType(0);
        g.cities[a].num_real_building[building] = 1;
        g.cities[b].num_real_building[building] = 2;
        g.cities[c].num_real_building[building] = 1;
        assert_eq!(g.player_get_num_building(p0, building), 3);

        g.cities[a].government_center_count = 1;
        assert_eq!(g.player_get_num_government_centers(p0), 1);
        assert_eq!(g.player_get_num_government_centers(p1), 0);
        let _ = d;
    }
}
