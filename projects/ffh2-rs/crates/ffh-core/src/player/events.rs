//! CvPlayer: random events -- trigger selection and weights, triggered-event data,
//! event options (`canDoEvent`/`applyEvent`), costs and techs, quests and expiry, and the
//! per-player event bookkeeping (events occurred, countdowns, fired triggers).
//!
//! Port of CvPlayer.cpp 13252-13265 and 17687-20955 (the event functions), plus the event
//! choice of `CvPlayer::trigger(const EventTriggeredData&)`: the human event popup
//! (`CvDLLButtonPopup::launchEventPopup`) and `CvPlayerAI::AI_chooseEvent` both offer the
//! trigger's events for which `canDoEvent` is true, so both go to the player's controller.
//!
//! - C++ keeps a pointer to the stored `EventTriggeredData` while it applies an event; only
//!   the trigger's `PythonCanDo` may edit the stored data, so the Rust code works on a copy
//!   (written back after `PythonCanDo`).
//! - UI only, not ported: quest messages (`addQuestMessage`, the turn-log entry that
//!   `expireEvent` removes), event and world-news messages (they go to the game log),
//!   replay messages and the other player's popup. The RNG draws that pick their texts are
//!   kept. Trigger texts are stored unformatted (the `%s` arguments are display-only).

use std::cmp::{max, min};

use crate::defs::{CITY_HOME_PLOT, INVALID_PLOT_COORD, MIN_INT, NUM_CITY_PLOTS};
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::{is_plot_event_trigger, set_trade_item};
use ffh_data::*;

/// C++ `CvPlayer::getCity(iID)`: the city if it still exists and `p` owns it (C++ city ids
/// index the owner's own list), else NULL.
fn player_city(g: &Game, p: PlayerId, id: Option<CityId>) -> Option<CityId> {
    let c = id?;
    match g.cities.get(c) {
        Some(city) if city.owner == p => Some(c),
        _ => None,
    }
}

/// C++ `CvPlayer::getUnit(iID)`: the unit if it still exists and `p` owns it, else NULL.
fn player_unit(g: &Game, p: PlayerId, id: Option<UnitId>) -> Option<UnitId> {
    let u = id?;
    match g.units.get(u) {
        Some(unit) if unit.owner == p => Some(u),
        _ => None,
    }
}

/// C++ `GC.getCivilizationInfo(getCivilizationType()).getCivilizationBuildings(eClass)`
/// (None also for a slot without a civilization, which C++ would index out of bounds).
fn civ_building(g: &Game, p: PlayerId, class: BuildingClassType) -> Option<BuildingType> {
    g.d.civ_building(g.player_get_civilization_type(p)?, class)
}

/// C++ `GC.getCivilizationInfo(getCivilizationType()).getCivilizationUnits(eClass)`.
fn civ_unit(g: &Game, p: PlayerId, class: UnitClassType) -> Option<UnitType> {
    g.d.civ_unit(g.player_get_civilization_type(p)?, class)
}

/// C++ `gDLL->getText(szKey, ...)` for event texts: the English text of the key, unformatted;
/// an empty key gives an empty text.
fn event_text(d: &GameData, key: &str) -> String {
    if key.is_empty() {
        String::new()
    } else {
        d.text.get(key).to_string()
    }
}

/// C++ `CvPlayerAI::AI_changeMemoryCount(eOther, eMemory, iChange)` on `p` (AI bookkeeping
/// read by the rules: a plain field).
fn change_memory_count(g: &mut Game, p: PlayerId, other: PlayerId, memory: MemoryType, change: i32) {
    let count = &mut g.players[p].ai_memory_count[other][memory];
    *count += change;
    debug_assert!(*count >= 0, "AI memory count below 0");
}

impl Game {
    // ------------------------------------------------------------------ triggered-event list

    /// C++ `CvPlayer::getEventTriggered(iID)`.
    pub fn player_get_event_triggered(&self, p: PlayerId, id: TriggeredId) -> Option<&EventTriggeredData> {
        self.players[p].events_triggered.iter().find(|d| d.id == id)
    }

    /// C++ `CvPlayer::addEventTriggered`: stores `data` under a fresh id (C++ returns a blank
    /// entry that the caller fills in) and returns that id.
    pub fn player_add_event_triggered(&mut self, p: PlayerId, mut data: EventTriggeredData) -> TriggeredId {
        let pl = &mut self.players[p];
        let id = TriggeredId(pl.next_triggered_id);
        pl.next_triggered_id += 1;
        data.id = id;
        pl.events_triggered.push(data);
        id
    }

    /// C++ `CvPlayer::deleteEventTriggered(iID)`.
    pub fn player_delete_event_triggered(&mut self, p: PlayerId, id: TriggeredId) {
        self.players[p].events_triggered.retain(|d| d.id != id);
    }

    // ------------------------------------------------------------------ events occurred / countdowns / fired triggers

    /// C++ `CvPlayer::getEventOccured(eEvent)`.
    pub fn player_get_event_occured(&self, p: PlayerId, event: EventType) -> Option<&EventTriggeredData> {
        self.players[p].events_occured.get(&event)
    }

    /// C++ `CvPlayer::setEventOccured(eEvent, kEventTriggered, bOthers = true)`: records the
    /// event (a quest also gets its turn-log message: UI) and copies global/team events to the
    /// other players.
    pub fn player_set_event_occured(&mut self, p: PlayerId, event: EventType, data: &EventTriggeredData, others: bool) {
        let d = self.d;
        let k_event = &d.events[event];

        self.players[p].events_occured.insert(event, data.clone());

        // UI: a quest posts its description (GAMETEXT.setEventHelp) as a quest message.

        if others {
            if k_event.global {
                for i in self.civ_player_ids() {
                    if i != p {
                        self.player_set_event_occured(i, event, data, false);
                    }
                }
            } else if k_event.team {
                let team = self.players[p].team;
                for i in self.civ_player_ids() {
                    if i != p && team == self.players[i].team {
                        self.player_set_event_occured(i, event, data, false);
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::resetEventOccured(eEvent, bAnnounce = true)`: expires the event, then forgets it.
    pub fn player_reset_event_occured(&mut self, p: PlayerId, event: EventType, announce: bool) {
        if let Some(data) = self.players[p].events_occured.get(&event).cloned() {
            self.player_expire_event(p, event, &data, announce);
            self.players[p].events_occured.remove(&event);
        }
    }

    /// C++ `CvPlayer::getEventCountdown(eEvent)`.
    pub fn player_get_event_countdown(&self, p: PlayerId, event: EventType) -> Option<&EventTriggeredData> {
        self.players[p].event_countdown.get(&event)
    }

    /// C++ `CvPlayer::setEventCountdown(eEvent, kEventTriggered)`.
    pub fn player_set_event_countdown(&mut self, p: PlayerId, event: EventType, data: &EventTriggeredData) {
        self.players[p].event_countdown.insert(event, data.clone());
    }

    /// C++ `CvPlayer::resetEventCountdown(eEvent)`.
    pub fn player_reset_event_countdown(&mut self, p: PlayerId, event: EventType) {
        self.players[p].event_countdown.remove(&event);
    }

    /// C++ `CvPlayer::isTriggerFired(eEventTrigger)`.
    pub fn player_is_trigger_fired(&self, p: PlayerId, trigger: EventTriggerType) -> bool {
        self.players[p].triggers_fired.contains(&trigger)
    }

    /// C++ `CvPlayer::setTriggerFired(kTriggeredData, bOthers = true, bAnnounce = true)`:
    /// records the trigger (copied to the other players for global/team triggers), runs the
    /// trigger's Python callback (FfH: once per alive player it is recorded for, and even
    /// when it was already fired), marks the trigger as triggered in the game (FfH) and
    /// announces it (world news / trigger text: UI, sent to the game log).
    pub fn player_set_trigger_fired(&mut self, p: PlayerId, data: &EventTriggeredData, others: bool, announce: bool) {
        let d = self.d;
        let k_trigger = &d.event_triggers[data.trigger];

        if !self.player_is_trigger_fired(p, data.trigger) {
            self.players[p].triggers_fired.push(data.trigger);

            if others {
                if k_trigger.global {
                    for i in self.civ_player_ids() {
                        if i != p {
                            // FfH: only alive players (BTS: every civ slot).
                            if self.players[i].alive {
                                self.player_set_trigger_fired(i, data, false, false);
                            }
                        }
                    }
                } else if k_trigger.team {
                    let team = self.players[p].team;
                    for i in self.civ_player_ids() {
                        if i != p && team == self.players[i].team {
                            self.player_set_trigger_fired(i, data, false, false);
                        }
                    }
                }
            }
        }

        // FfH: Modified by Kael 09/25/2008 (BTS calls the callback with the triggered data only,
        // and for dead players too)
        if self.players[p].alive {
            if !k_trigger.python_callback.is_empty() {
                self.py_trigger_callback(data, p);
            }
        }
        self.event_triggered[data.trigger] = true;
        // FfH: End Modify

        if announce {
            // UI: the world news goes to every alive player who has met us (and the other
            // player), plus a replay message; otherwise the trigger text goes to us.
            let turn = self.game_turn();
            if !data.global_text.is_empty() {
                self.log.msg(turn, None, || data.global_text.clone());
            } else if !data.text.is_empty() {
                self.log.msg(turn, Some(p), || data.text.clone());
            }
        }
    }

    /// C++ `CvPlayer::resetTriggerFired(eTrigger)`.
    pub fn player_reset_trigger_fired(&mut self, p: PlayerId, trigger: EventTriggerType) {
        let fired = &mut self.players[p].triggers_fired;
        if let Some(i) = fired.iter().position(|&t| t == trigger) {
            fired.remove(i);
        }
    }

    // ------------------------------------------------------------------ triggering

    /// C++ `CvPlayer::doEvents`: expires finished quests/events, rolls for a new trigger
    /// (GAMEOPTION_NO_EVENTS, barbarians and minor civs excluded), then applies the delayed
    /// events whose countdown has run out.
    pub fn player_do_events(&mut self, p: PlayerId) {
        let d = self.d;

        if self.is_option(GameOptionType::GAMEOPTION_NO_EVENTS) {
            return;
        }

        if self.player_is_barbarian(p) || self.player_is_minor_civ(p) {
            return;
        }

        // C++ walks m_mapEventsOccured in key order, erasing the expired entries; neither the
        // check nor the expiry changes the map, so walking a copy of the keys is the same.
        let occured: Vec<EventType> = self.players[p].events_occured.keys().copied().collect();
        for event in occured {
            let Some(data) = self.players[p].events_occured.get(&event).cloned() else {
                continue;
            };
            if self.player_check_expire_event(p, event, &data) {
                self.player_expire_event(p, event, &data, true);
                self.players[p].events_occured.remove(&event);
            }
        }

        let mut new_event_eligible = true;
        if self.elapsed_game_turns < d.defines.first_event_delay_turns {
            new_event_eligible = false;
        }

        if new_event_eligible {
            // FfH: Modified by Kael 09/26/2007 (GAMEOPTION_DOUBLE_EVENTS doubles the era's chance)
            let mut chance = d.eras[self.players[p].current_era].event_chance_per_turn;
            if self.is_option(GameOptionType::GAMEOPTION_DOUBLE_EVENTS) {
                chance *= 2;
            }
            if self.soren_rand_num(d.defines.event_probability_roll_sides, "Global event check") >= chance {
                new_event_eligible = false;
            }
        }

        // (triggered data, cumulative weight)
        let mut possible_event_trigger_weights: Vec<(TriggeredId, i32)> = Vec::new();
        let mut total_weight = 0;
        for trigger in d.event_triggers.keys() {
            let weight = self.player_get_event_trigger_weight(p, trigger);
            if weight == -1 {
                self.player_trigger(p, trigger);
            } else if weight > 0 && new_event_eligible {
                let triggered = self.player_init_triggered_data(
                    p,
                    trigger,
                    false,
                    None,
                    INVALID_PLOT_COORD,
                    INVALID_PLOT_COORD,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                );
                if let Some(id) = triggered {
                    total_weight += weight;
                    possible_event_trigger_weights.push((id, total_weight));
                }
            }
        }

        if total_weight > 0 {
            let mut fired = false;
            let value = self.soren_rand_num(total_weight, "Event trigger");
            for (id, cumulative_weight) in possible_event_trigger_weights {
                if value < cumulative_weight && !fired {
                    if let Some(data) = self.player_get_event_triggered(p, id).cloned() {
                        self.player_trigger_data(p, &data);
                    }
                    fired = true;
                } else {
                    self.player_delete_event_triggered(p, id);
                }
            }
        }

        let mut cleanup: Vec<TriggeredId> = Vec::new();
        for event in d.events.keys() {
            let Some(data) = self.player_get_event_countdown(p, event) else {
                continue;
            };
            if self.game_turn() >= data.turn {
                let id = data.id;
                self.player_apply_event(p, event, id, true);
                self.player_reset_event_countdown(p, event);
                // C++ quirk: pushes m_iId read from the countdown entry it just erased (use
                // after free); an entry applyEvent stored for this event has the same id.
                cleanup.push(id);
            }
        }

        for id in cleanup {
            let mut delete = true;

            for event in d.events.keys() {
                if let Some(data) = self.player_get_event_countdown(p, event) {
                    if data.id == id {
                        delete = false;
                        break;
                    }
                }
            }

            if delete {
                self.player_delete_event_triggered(p, id);
            }
        }
    }

    /// C++ `CvPlayer::getEventTriggerWeight(eTrigger)` (0 = cannot trigger; -1 = always).
    pub fn player_get_event_trigger_weight(&self, p: PlayerId, trigger: EventTriggerType) -> i32 {
        let d = self.d;
        let k_trigger = &d.event_triggers[trigger];
        let team = self.players[p].team;

        if let Some(min_difficulty) = k_trigger.min_difficulty {
            if self.handicap < min_difficulty {
                return 0;
            }
        }

        if k_trigger.single_player && self.is_game_multi_player() {
            return 0;
        }

        if !self.is_event_active(trigger) {
            return 0;
        }

        for &tech in &k_trigger.obsolete_techs {
            if self.team_is_has_tech(team, Some(tech)) {
                return 0;
            }
        }

        if !k_trigger.recurring {
            if self.player_is_trigger_fired(p, trigger) {
                return 0;
            }

            // FfH: Added by Kael 09/19/2008 (a global trigger fires once per game)
            if k_trigger.global {
                if self.event_triggered[trigger] {
                    return 0;
                }
            }
            // FfH: End Add
        }

        if !k_trigger.prereq_or_techs.is_empty() {
            let mut found_valid = false;

            for &tech in &k_trigger.prereq_or_techs {
                if self.team_is_has_tech(team, Some(tech)) {
                    found_valid = true;
                    break;
                }
            }

            if !found_valid {
                return 0;
            }
        }

        if !k_trigger.prereq_and_techs.is_empty() {
            let mut found_valid = true;

            for &tech in &k_trigger.prereq_and_techs {
                if !self.team_is_has_tech(team, Some(tech)) {
                    found_valid = false;
                    break;
                }
            }

            if !found_valid {
                return 0;
            }
        }

        if !k_trigger.prereq_events.is_empty() {
            let mut found_valid = true;
            for &event in &k_trigger.prereq_events {
                if self.player_get_event_occured(p, event).is_none() {
                    found_valid = false;
                    break;
                }
            }

            if !found_valid {
                return 0;
            }
        }

        if let Some(civic) = k_trigger.civic {
            let mut found_valid = false;

            for option in d.civic_options.keys() {
                if self.players[p].civics[option] == Some(civic) {
                    found_valid = true;
                    break;
                }
            }

            if !found_valid {
                return 0;
            }
        }

        // FfH Events: Added by Kael 08/08/2007
        if k_trigger.prereq_global_counter != 0 {
            if self.get_global_counter() < k_trigger.prereq_global_counter {
                return 0;
            }
        }
        if let Some(alignment) = k_trigger.prereq_alignment {
            if self.players[p].alignment != Some(alignment) {
                return 0;
            }
        }
        if let Some(prereq_trait) = k_trigger.prereq_trait {
            if !self.players[p].traits[prereq_trait] {
                return 0;
            }
        }
        if let Some(civilization) = k_trigger.prereq_civilization_pleased {
            if Some(civilization) == self.player_get_civilization_type(p) {
                return 0;
            }
            let mut valid = false;
            for i in self.player_ids() {
                if self.players[i].alive {
                    if self.player_get_civilization_type(i) == Some(civilization) {
                        if self.teams[team].has_met[self.players[i].team] {
                            // C++ `AI_getAttitude(getID())` (bForced = true)
                            if self.player_ai_get_attitude(i, p, true) >= AttitudeType::ATTITUDE_PLEASED {
                                valid = true;
                            }
                        }
                    }
                }
            }
            if !valid {
                return 0;
            }
        }
        if let Some(leader) = k_trigger.prereq_leader {
            if self.player_get_leader_type(p) != Some(leader) {
                return 0;
            }
        }
        // FfH: End Add

        if k_trigger.min_treasury > 0 {
            if self.players[p].gold < k_trigger.min_treasury {
                return 0;
            }
        }

        if self.map.get_num_land_areas() < k_trigger.min_map_landmass {
            return 0;
        }

        if k_trigger.min_our_landmass > 0 || k_trigger.max_our_landmass != -1 {
            let mut num_landmass = 0;

            for (_, area) in self.map.areas.iter() {
                if !area.water {
                    if area.cities_per_player[p] > 0 {
                        num_landmass += 1;
                    }
                }
            }

            if num_landmass < k_trigger.min_our_landmass {
                return 0;
            }

            if k_trigger.max_our_landmass != -1 && num_landmass > k_trigger.max_our_landmass {
                return 0;
            }
        }

        if k_trigger.probability < 0 {
            return k_trigger.probability;
        }

        let mut probability = k_trigger.probability;

        if k_trigger.probability_unit_multiply && k_trigger.num_units > 0 {
            let mut num_units = 0;
            for &unit in &self.players[p].units {
                if MIN_INT != self.unit_get_trigger_value(unit, trigger, None, true) {
                    num_units += 1;
                }
            }

            probability *= num_units;
        }

        if k_trigger.probability_building_multiply && k_trigger.num_buildings > 0 {
            let mut num_buildings = 0;
            for &building_class in &k_trigger.buildings_required {
                num_buildings += self.players[p].building_class_count[building_class];
            }

            probability *= num_buildings;
        }

        probability
    }

    /// C++ `CvPlayer::initTriggeredData(eEventTrigger, bFire = false, iCityId = -1, iPlotX =
    /// INVALID_PLOT_COORD, iPlotY = INVALID_PLOT_COORD, eOtherPlayer = NO_PLAYER,
    /// iOtherPlayerCityId = -1, eReligion = NO_RELIGION, eCorporation = NO_CORPORATION,
    /// iUnitId = -1, eBuilding = NO_BUILDING)`: picks the trigger's city/plot/unit/other
    /// player/religion/corporation/building and texts, stores the triggered data and, with
    /// `fire`, triggers it. None if the trigger cannot happen (C++ NULL). With `fire`, the
    /// returned id may already be gone (applied and deleted).
    #[allow(clippy::too_many_arguments)]
    pub fn player_init_triggered_data(
        &mut self,
        p: PlayerId,
        trigger: EventTriggerType,
        fire: bool,
        city: Option<CityId>,
        plot_x: i32,
        plot_y: i32,
        other_player: Option<PlayerId>,
        other_player_city: Option<CityId>,
        religion: Option<ReligionType>,
        corporation: Option<CorporationType>,
        unit: Option<UnitId>,
        building: Option<BuildingType>,
    ) -> Option<TriggeredId> {
        let d = self.d;
        let k_trigger = &d.event_triggers[trigger];

        let mut city = player_city(self, p, city);
        let mut other_player = other_player;
        let mut other_city: Option<CityId> = None;
        if let Some(other) = other_player {
            other_city = player_city(self, other, other_player_city);
        }
        let mut plot = self.map.plot_xy(plot_x, plot_y);
        let mut unit = player_unit(self, p, unit);
        let mut religion = religion;
        let mut corporation = corporation;
        let mut building = building;

        let mut plots: Vec<PlotIdx> = Vec::new();
        let pick_plot = is_plot_event_trigger(d, trigger);

        if k_trigger.pick_city {
            if city.is_none() {
                city = self.player_pick_trigger_city(p, trigger);
            }

            if let Some(c) = city {
                if pick_plot {
                    for i in 0..NUM_CITY_PLOTS {
                        if CITY_HOME_PLOT != i {
                            if let Some(loop_plot) = self.city_index_plot(c, i) {
                                if self.plot_can_trigger(loop_plot, trigger, p) {
                                    plots.push(loop_plot);
                                }
                            }
                        }
                    }
                }
            } else {
                return None;
            }
        } else {
            if k_trigger.num_buildings > 0 && !k_trigger.buildings_required.is_empty() {
                let mut found_valid = 0;

                for &building_class in &k_trigger.buildings_required {
                    found_valid += self.players[p].building_class_count[building_class];
                }

                if found_valid < k_trigger.num_buildings {
                    return None;
                }
            }

            if k_trigger.num_religions > 0 {
                let mut found_valid = 0;

                if !k_trigger.religions_required.is_empty() {
                    for &required in &k_trigger.religions_required {
                        if self.players[p].has_religion_count[required] > 0 {
                            found_valid += 1;
                        }
                    }
                } else {
                    for loop_religion in d.religions.keys() {
                        if self.players[p].has_religion_count[loop_religion] > 0 {
                            found_valid += 1;
                        }
                    }
                }

                if found_valid < k_trigger.num_religions {
                    return None;
                }
            }

            if k_trigger.num_corporations > 0 {
                let mut found_valid = 0;

                if !k_trigger.corporations_required.is_empty() {
                    for &required in &k_trigger.corporations_required {
                        if self.player_get_has_corporation_count(p, required) > 0 {
                            found_valid += 1;
                        }
                    }
                } else {
                    for loop_corporation in d.corporations.keys() {
                        if self.player_get_has_corporation_count(p, loop_corporation) > 0 {
                            found_valid += 1;
                        }
                    }
                }

                if found_valid < k_trigger.num_corporations {
                    return None;
                }
            }

            if k_trigger.min_population > 0 {
                if self.players[p].total_population < k_trigger.min_population {
                    return None;
                }
            }

            if k_trigger.max_population > 0 {
                if self.players[p].total_population > k_trigger.max_population {
                    return None;
                }
            }

            if pick_plot {
                for loop_plot in self.map.plot_ids() {
                    if self.plot_can_trigger(loop_plot, trigger, p) {
                        plots.push(loop_plot);
                    }
                }
            }
        }

        if k_trigger.pick_religion {
            if religion.is_none() {
                if k_trigger.state_religion {
                    let state_religion = self.player_get_state_religion(p);
                    if let Some(state_religion) = state_religion {
                        if self.player_is_valid_trigger_religion(p, trigger, city, state_religion) {
                            religion = Some(state_religion);
                        }
                    }
                } else {
                    let num_religions = d.religions.len() as i32;
                    let offset = self.soren_rand_num(num_religions, "Event pick religion");

                    for i in 0..num_religions {
                        let loop_religion = ReligionType::from_usize(((i + offset) % num_religions) as usize);

                        if self.player_is_valid_trigger_religion(p, trigger, city, loop_religion) {
                            religion = Some(loop_religion);
                            break;
                        }
                    }
                }
            }

            if religion.is_none() {
                return None;
            }
        }

        if k_trigger.pick_corporation {
            if corporation.is_none() {
                let num_corporations = d.corporations.len() as i32;
                let offset = self.soren_rand_num(num_corporations, "Event pick corporation");

                for i in 0..num_corporations {
                    let loop_corporation = CorporationType::from_usize(((i + offset) % num_corporations) as usize);

                    if self.player_is_valid_trigger_corporation(p, trigger, city, loop_corporation) {
                        corporation = Some(loop_corporation);
                        break;
                    }
                }
            }

            if corporation.is_none() {
                return None;
            }
        }

        if plot.is_none() {
            if !plots.is_empty() {
                let chosen = self.soren_rand_num(plots.len() as i32, "Event pick plot");
                let chosen_plot = plots[chosen as usize];
                plot = Some(chosen_plot);

                if city.is_none() {
                    let (x, y) = self.map.xy(chosen_plot);
                    city = self.map_find_city(x, y, Some(p), None, false, false, None, None, None);
                }
            } else {
                if pick_plot {
                    return None;
                }

                if let Some(c) = city {
                    plot = Some(self.city_plot(c));
                }
            }
        }

        if k_trigger.num_buildings > 0 {
            if let (Some(c), None) = (city, building) {
                let mut buildings: Vec<BuildingType> = Vec::new();
                for &building_class in &k_trigger.buildings_required {
                    if let Some(test_building) = civ_building(self, p, building_class) {
                        if self.cities[c].num_real_building[test_building] > 0 {
                            buildings.push(test_building);
                        }
                    }
                }

                if !buildings.is_empty() {
                    let chosen = self.soren_rand_num(buildings.len() as i32, "Event pick building");
                    building = Some(buildings[chosen as usize]);
                } else {
                    return None;
                }
            }
        }

        if unit.is_none() {
            unit = self.player_pick_trigger_unit(p, trigger, plot, pick_plot);
        }

        if unit.is_none() && k_trigger.num_units > 0 {
            return None;
        }

        if plot.is_none() {
            if let Some(u) = unit {
                plot = Some(self.unit_plot(u));
            }
        }

        if plot.is_none() && pick_plot {
            return None;
        }

        if k_trigger.num_units_global > 0 {
            let mut num_units = 0;
            for i in self.civ_player_ids() {
                if self.players[i].alive {
                    for &loop_unit in &self.players[i].units {
                        if MIN_INT != self.unit_get_trigger_value(loop_unit, trigger, plot, true) {
                            num_units += 1;
                        }
                    }
                }
            }

            if num_units < k_trigger.num_units_global {
                return None;
            }
        }

        if k_trigger.num_buildings_global > 0 {
            let mut num_buildings = 0;
            for i in self.civ_player_ids() {
                if self.players[i].alive {
                    for &building_class in &k_trigger.buildings_required {
                        // C++ quirk: counts this player's buildings (getBuildingClassCount, not
                        // kLoopPlayer's) once per alive civ player.
                        num_buildings += self.players[p].building_class_count[building_class];
                    }
                }
            }

            if num_buildings < k_trigger.num_buildings_global {
                return None;
            }
        }

        if k_trigger.pick_player {
            let mut players: Vec<PlayerId> = Vec::new();
            let mut cities: Vec<Option<CityId>> = Vec::new();

            if other_player.is_none() {
                for i in self.civ_player_ids() {
                    if self.player_can_trigger(i, trigger, p, religion) {
                        if k_trigger.pick_other_player_city {
                            let best_city = if let Some(c) = city {
                                let (x, y) = (self.cities[c].x, self.cities[c].y);
                                self.map_find_city(x, y, Some(i), None, true, false, None, None, None)
                            } else {
                                self.player_pick_trigger_city(i, trigger)
                            };

                            if let Some(best_city) = best_city {
                                cities.push(Some(best_city));
                                players.push(i);
                            }
                        } else {
                            cities.push(None);
                            players.push(i);
                        }
                    }
                }

                if !players.is_empty() {
                    let chosen = self.soren_rand_num(players.len() as i32, "Event pick player") as usize;
                    other_player = Some(players[chosen]);
                    other_city = cities[chosen];
                } else {
                    return None;
                }
            }
        }

        let (stored_x, stored_y) = match plot {
            Some(pl) => self.map.xy(pl),
            None => (INVALID_PLOT_COORD, INVALID_PLOT_COORD),
        };
        let id = self.player_add_event_triggered(
            p,
            EventTriggeredData {
                id: TriggeredId(0),
                trigger,
                turn: self.game_turn(),
                player: p,
                city,
                plot_x: stored_x,
                plot_y: stored_y,
                unit,
                other_player,
                other_player_city: other_city,
                religion,
                corporation,
                building,
                text: String::new(),
                global_text: String::new(),
            },
        );

        if !k_trigger.python_can_do.is_empty() {
            // Python edits the stored data in place: run it on a copy and store the copy back.
            let mut py_data = self.player_get_event_triggered(p, id).cloned().expect("triggered data was just added");
            let result = self.py_trigger_can_do(&mut py_data);
            if let Some(stored) = self.players[p].events_triggered.iter_mut().find(|t| t.id == id) {
                *stored = EventTriggeredData { id, ..py_data };
            }

            if !result {
                self.player_delete_event_triggered(p, id);
                return None;
            }

            // (C++ then re-reads pCity, pPlot, pUnit, eOtherPlayer, pOtherPlayerCity, eReligion,
            // eCorporation and eBuilding from the data -- "python may change pTriggerData" --
            // only as arguments of the texts below, which are stored unformatted.)
        }

        let current_era = self.players[p].current_era;
        let texts: Vec<&str> = k_trigger
            .texts
            .iter()
            .zip(&k_trigger.text_eras)
            .filter(|&(_, &era)| era.is_none() || era == Some(current_era))
            .map(|(text, _)| text.as_str())
            .collect();

        let text = if !texts.is_empty() {
            let chosen = self.soren_rand_num(texts.len() as i32, "Event Text choice");
            event_text(d, texts[chosen as usize])
        } else {
            String::new()
        };

        let global_text = if !k_trigger.world_news.is_empty() {
            let chosen = self.soren_rand_num(k_trigger.world_news.len() as i32, "Trigger World News choice");
            event_text(d, &k_trigger.world_news[chosen as usize])
        } else {
            String::new()
        };

        if let Some(stored) = self.players[p].events_triggered.iter_mut().find(|t| t.id == id) {
            stored.text = text;
            stored.global_text = global_text;
        }

        if fire {
            if let Some(data) = self.player_get_event_triggered(p, id).cloned() {
                self.player_trigger_data(p, &data);
            }
        }

        Some(id)
    }

    /// C++ `CvPlayer::trigger(EventTriggerTypes)`: `initTriggeredData(eTrigger, true)`.
    pub fn player_trigger(&mut self, p: PlayerId, trigger: EventTriggerType) {
        self.player_init_triggered_data(p, trigger, true, None, INVALID_PLOT_COORD, INVALID_PLOT_COORD, None, None, None, None, None, None);
    }

    /// C++ `CvPlayer::trigger(const EventTriggeredData&)`: humans get the event popup, AI
    /// players choose with `AI_chooseEvent`; both offer the trigger's events for which
    /// `canDoEvent` is true (in trigger order), so both go to the controller here, and the
    /// choice is applied (`applyEvent`). No possible event: no choice (C++: `NO_EVENT`, or the
    /// popup is not launched) and the triggered data stays pending.
    pub fn player_trigger_data(&mut self, p: PlayerId, data: &EventTriggeredData) {
        let d = self.d;

        // AI_chooseEvent / launchEventPopup look the data up by id (NULL: no choice).
        let Some(triggered) = self.player_get_event_triggered(p, data.id).cloned() else {
            return;
        };

        let k_trigger = &d.event_triggers[triggered.trigger];
        let options: Vec<EventType> =
            k_trigger.events.iter().copied().filter(|&event| self.player_can_do_event(p, event, &triggered)).collect();

        if options.is_empty() {
            return;
        }

        // C++ human: BUTTONPOPUP_EVENT; AI: AI_chooseEvent(kData.getID()).
        let event = self.controller(p).choose_event(self, p, &triggered, &options);
        if let Some(event) = event {
            self.player_apply_event(p, event, triggered.id, true);
        }
    }

    /// C++ `CvPlayer::canTrigger(eTrigger, ePlayer, eReligion)`: can `p` be the "other
    /// player" of `other`'s trigger.
    pub fn player_can_trigger(&self, p: PlayerId, trigger: EventTriggerType, other: PlayerId, religion: Option<ReligionType>) -> bool {
        if !self.players[p].alive {
            return false;
        }

        if p == other {
            return false;
        }

        let k_trigger = &self.d.event_triggers[trigger];
        let team = self.players[p].team;
        let other_team = self.players[other].team;

        if team == other_team {
            return false;
        }

        if !k_trigger.pick_player {
            return false;
        }

        if !self.teams[team].has_met[other_team] {
            return false;
        }

        if self.players[p].human && k_trigger.other_player_ai {
            return false;
        }

        if self.teams[team].at_war[other_team] != k_trigger.other_player_war {
            return false;
        }

        if let Some(tech) = k_trigger.other_player_has_tech {
            if !self.team_is_has_tech(team, Some(tech)) {
                return false;
            }
        }

        if k_trigger.other_player_share_borders > 0 {
            let mut count = 0;

            for loop_plot in self.map.plot_ids() {
                if !self.plot_is_water(loop_plot) {
                    if self.map.plots[loop_plot].owner == Some(p) && self.plot_is_adjacent_player(loop_plot, other, true) {
                        count += 1;
                    }
                }
            }

            if count < k_trigger.other_player_share_borders {
                return false;
            }
        }

        if let Some(religion) = religion {
            let has_religion = if k_trigger.state_religion {
                self.player_get_state_religion(p) == Some(religion)
            } else {
                self.players[p].has_religion_count[religion] > 0
            };

            if k_trigger.other_player_has_religion {
                if !has_religion {
                    return false;
                }
            }

            if k_trigger.other_player_has_other_religion {
                if has_religion {
                    return false;
                }

                if k_trigger.state_religion && self.player_get_state_religion(p).is_none() {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvPlayer::pickTriggerCity(eTrigger)` (const in C++, but breaks ties with the game RNG).
    pub fn player_pick_trigger_city(&mut self, p: PlayerId, trigger: EventTriggerType) -> Option<CityId> {
        let mut cities: Vec<CityId> = Vec::new();
        let mut best_value = MIN_INT;
        for &loop_city in &self.players[p].cities {
            let value = self.city_trigger_value(loop_city, trigger);

            if value >= best_value && value != MIN_INT {
                if value > best_value {
                    cities.clear();
                    best_value = value;
                }
                cities.push(loop_city);
            }
        }

        if !cities.is_empty() {
            let chosen = self.soren_rand_num(cities.len() as i32, "Event pick city");
            return Some(cities[chosen as usize]);
        }

        None
    }

    /// C++ `CvPlayer::pickTriggerUnit(eTrigger, pPlot, bPickPlot)` (const in C++, but breaks
    /// ties with the game RNG).
    pub fn player_pick_trigger_unit(&mut self, p: PlayerId, trigger: EventTriggerType, plot: Option<PlotIdx>, pick_plot: bool) -> Option<UnitId> {
        let mut units: Vec<UnitId> = Vec::new();
        let mut best_value = MIN_INT;
        for &loop_unit in &self.players[p].units {
            let value = self.unit_get_trigger_value(loop_unit, trigger, plot, pick_plot);

            if value >= best_value && value != MIN_INT {
                if value > best_value {
                    units.clear();
                    best_value = value;
                }
                units.push(loop_unit);
            }
        }

        if !units.is_empty() {
            let chosen = self.soren_rand_num(units.len() as i32, "Event pick unit");
            return Some(units[chosen as usize]);
        }

        None
    }

    /// C++ `CvPlayer::isValidTriggerReligion(kTrigger, pCity, eReligion)`.
    pub fn player_is_valid_trigger_religion(&self, p: PlayerId, trigger: EventTriggerType, city: Option<CityId>, religion: ReligionType) -> bool {
        let k_trigger = &self.d.event_triggers[trigger];

        if !k_trigger.religions_required.is_empty() {
            let found = k_trigger.religions_required.contains(&religion);

            if !found {
                return false;
            }
        }

        if let Some(c) = city {
            if !self.cities[c].has_religion[religion] {
                return false;
            }

            if k_trigger.holy_city {
                if !self.city_is_holy_city(c, religion) {
                    return false;
                }
            }
        } else {
            if 0 == self.players[p].has_religion_count[religion] {
                return false;
            }

            if k_trigger.holy_city {
                let holy_city = self.get_holy_city(religion);
                if holy_city.is_none_or(|hc| self.cities[hc].owner != p) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvPlayer::isValidTriggerCorporation(kTrigger, pCity, eCorporation)`.
    pub fn player_is_valid_trigger_corporation(&self, p: PlayerId, trigger: EventTriggerType, city: Option<CityId>, corporation: CorporationType) -> bool {
        let k_trigger = &self.d.event_triggers[trigger];

        if !k_trigger.corporations_required.is_empty() {
            let found = k_trigger.corporations_required.contains(&corporation);

            if !found {
                return false;
            }
        }

        if let Some(c) = city {
            if !self.cities[c].has_corporation[corporation] {
                return false;
            }

            if k_trigger.headquarters {
                if !self.city_is_headquarters(c, corporation) {
                    return false;
                }
            }
        } else {
            if self.player_get_has_corporation_count(p, corporation) > 0 {
                return true;
            }

            if k_trigger.headquarters {
                let headquarters = self.get_headquarters(corporation);
                if headquarters.is_none_or(|hq| self.cities[hq].owner != p) {
                    return false;
                }
            }
        }

        // C++ quirk (BTS): falls through to false, so with a city every corporation is
        // rejected, and without one only `getHasCorporationCount > 0` accepts.
        false
    }

    // ------------------------------------------------------------------ event options

    /// C++ `CvPlayer::canDoEvent(eEvent, kTriggeredData)`.
    pub fn player_can_do_event(&self, p: PlayerId, event: EventType, data: &EventTriggeredData) -> bool {
        let d = self.d;
        let k_event = &d.events[event];
        let team = self.players[p].team;

        let gold = min(self.player_get_event_cost(p, event, data.other_player, false), self.player_get_event_cost(p, event, data.other_player, true));

        if gold != 0 {
            if gold > 0 && data.other_player.is_some() && k_event.gold_to_player {
                if let Some(other) = data.other_player {
                    if self.players[other].gold < gold {
                        return false;
                    }
                }
            } else if gold < 0 {
                if self.players[p].gold < -gold {
                    return false;
                }
            }
        }

        if 0 != k_event.space_production_modifier {
            let mut valid = false;
            for k_project in d.projects.iter() {
                if k_project.spaceship {
                    if let Some(victory) = k_project.victory_prereq {
                        if self.is_victory_valid(victory) {
                            valid = true;
                            break;
                        }
                    }
                }
            }

            if !valid {
                return false;
            }
        }

        if k_event.espionage_points > 0 && self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            return false;
        }

        if let Some(other) = data.other_player {
            if k_event.espionage_points + self.teams[team].espionage_points_against_team[self.players[other].team] < 0 {
                return false;
            }
        }

        if 0 != k_event.tech_percent || 0 != k_event.tech_cost_percent {
            if self.player_get_best_event_tech(p, event, data.other_player).is_none() {
                return false;
            }
        }

        if let Some(prereq_tech) = k_event.prereq_tech {
            if !self.team_is_has_tech(team, Some(prereq_tech)) {
                return false;
            }
        }

        if let Some(bonus) = k_event.bonus_gift {
            let Some(other) = data.other_player else {
                return false;
            };

            if !self.player_can_trade_network_with(p, other) {
                return false;
            }

            if self.player_get_num_available_bonuses(other, bonus) > 0 {
                return false;
            }

            if self.player_get_num_tradeable_bonuses(p, bonus) <= 1 {
                return false;
            }
        }

        if let Some(unit_class) = k_event.unit_class {
            if civ_unit(self, p, unit_class).is_none() {
                return false;
            }
        }

        if k_event.city_effect {
            let city = player_city(self, p, data.city);
            if city.is_none_or(|c| !self.city_can_apply_event(c, event, data)) {
                return false;
            }
        } else if k_event.other_player_city_effect {
            let Some(other) = data.other_player else {
                return false;
            };

            let city = player_city(self, other, data.other_player_city);
            if city.is_none_or(|c| !self.city_can_apply_event(c, event, data)) {
                return false;
            }
        }

        if is_plot_event_trigger(d, data.trigger) {
            if let Some(plot) = self.map.plot_xy(data.plot_x, data.plot_y) {
                if !self.plot_can_apply_event(plot, event) {
                    return false;
                }
            }
        }

        if let Some(unit) = player_unit(self, p, data.unit) {
            if !self.unit_can_apply_event(unit, event) {
                return false;
            }
        }

        if let Some(bonus) = k_event.bonus_revealed {
            if self.team_is_has_tech(team, d.bonuses[bonus].tech_reveal) {
                return false;
            }

            if self.team_is_force_revealed_bonus(team, bonus) {
                return false;
            }
        }

        if k_event.convert_own_cities > 0 {
            let mut found_valid = false;

            if let Some(religion) = data.religion {
                for &loop_city in &self.players[p].cities {
                    if !self.cities[loop_city].has_religion[religion] {
                        if -1 == k_event.max_num_religions || self.city_religion_count(loop_city) <= k_event.max_num_religions {
                            found_valid = true;
                            break;
                        }
                    }
                }
            }

            if !found_valid {
                return false;
            }
        }

        if k_event.convert_other_cities > 0 {
            let mut found_valid = false;

            if let Some(religion) = data.religion {
                if let Some(other) = data.other_player {
                    for &loop_city in &self.players[other].cities {
                        if !self.cities[loop_city].has_religion[religion] {
                            if -1 == k_event.max_num_religions || self.city_religion_count(loop_city) <= k_event.max_num_religions {
                                found_valid = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !found_valid {
                return false;
            }
        }

        if 0 != k_event.attitude_modifier {
            let Some(other) = data.other_player else {
                return false;
            };

            if self.players[other].team == team {
                return false;
            }

            if self.players[other].human {
                if 0 == k_event.our_attitude_modifier {
                    return false;
                }
            }
        }

        if 0 != k_event.their_enemy_attitude_modifier {
            let Some(other) = data.other_player else {
                return false;
            };

            let worst_enemy = self.teams[self.players[other].team].ai_worst_enemy;
            let Some(worst_enemy) = worst_enemy else {
                return false;
            };
            if worst_enemy == team {
                return false;
            }

            if !self.team_is_alive(worst_enemy) {
                return false;
            }

            if worst_enemy == team {
                return false;
            }
        }

        if k_event.declare_war {
            let Some(other) = data.other_player else {
                return false;
            };

            let other_team = self.players[other].team;
            if !self.team_can_declare_war(other_team, team) || !self.team_can_declare_war(team, other_team) {
                return false;
            }
        }

        if k_event.quest {
            for loop_trigger in d.event_triggers.keys() {
                let k_trigger = &d.event_triggers[loop_trigger];
                if !k_trigger.recurring {
                    for &prereq_event in &k_trigger.prereq_events {
                        if prereq_event == event {
                            if self.player_is_trigger_fired(p, loop_trigger) {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        // FfH: Added by Kael 01/21/2008
        if let Some(alignment) = k_event.prereq_alignment {
            if self.players[p].alignment != Some(alignment) {
                return false;
            }
        }
        if let Some(bonus) = k_event.prereq_bonus {
            if !self.player_has_bonus(p, bonus) {
                return false;
            }
        }
        if let Some(civilization) = k_event.prereq_civilization {
            if self.player_get_civilization_type(p) != Some(civilization) {
                return false;
            }
        }
        if let Some(corporation) = k_event.prereq_corporation {
            if let Some(c) = player_city(self, p, data.city) {
                if !self.cities[c].has_corporation[corporation] {
                    return false;
                }
            }
        }
        if let Some(religion) = k_event.prereq_religion {
            if let Some(c) = player_city(self, p, data.city) {
                if !self.cities[c].has_religion[religion] {
                    return false;
                }
            }
        }
        if let Some(religion) = k_event.prereq_state_religion {
            if self.player_get_state_religion(p) != Some(religion) {
                return false;
            }
        }
        // FfH: End Add

        if !k_event.python_can_do.is_empty() {
            if !self.py_event_can_do(event, data) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::applyEvent(eEvent, iTriggeredId, bUpdateTrigger = true)`: applies an
    /// event option of a pending triggered event (and the event's Python callback), then its
    /// additional events (now, or as countdowns) and deletes the triggered data.
    pub fn player_apply_event(&mut self, p: PlayerId, event: EventType, triggered: TriggeredId, update_trigger: bool) {
        let d = self.d;

        // C++ holds a pointer to the stored data; nothing below changes it.
        let Some(data) = self.player_get_event_triggered(p, triggered).cloned() else {
            self.player_delete_event_triggered(p, triggered);
            return;
        };

        if update_trigger {
            self.player_set_trigger_fired(p, &data, true, true);
        }

        if !self.player_can_do_event(p, event, &data) {
            if update_trigger {
                self.player_delete_event_triggered(p, triggered);
            }
            return;
        }

        self.player_set_event_occured(p, event, &data, true);

        let k_event = &d.events[event];
        let city = player_city(self, p, data.city);
        let mut other_player_city = None;

        if let Some(other) = data.other_player {
            other_player_city = player_city(self, other, data.other_player_city);
        }

        let mut gold = self.player_get_event_cost(p, event, data.other_player, false);
        let random_gold = self.player_get_event_cost(p, event, data.other_player, true);

        gold += self.soren_rand_num(random_gold - gold + 1, "Event random gold");

        if gold != 0 {
            self.players[p].gold += gold;

            if let Some(other) = data.other_player {
                if k_event.gold_to_player {
                    self.players[other].gold -= gold;
                }
            }
        }

        if let Some(other) = data.other_player {
            if k_event.espionage_points != 0 {
                let (team, other_team) = (self.players[p].team, self.players[other].team);
                self.team_change_espionage_points_against_team(team, other_team, k_event.espionage_points);
            }
        }

        if 0 != k_event.tech_percent {
            let best_tech = self.player_get_best_event_tech(p, event, data.other_player);

            if let Some(best_tech) = best_tech {
                let team = self.players[p].team;
                let beakers = self.team_change_research_progress_percent(team, best_tech, k_event.tech_percent, p);

                if beakers > 0 {
                    // UI: TXT_KEY_MISC_PROGRESS_TOWARDS_TECH (C++ sends it to the players whose
                    // team id equals our player id).
                    let turn = self.game_turn();
                    self.log.msg(turn, Some(p), || format!("{} beakers towards {}", beakers, d.text.name(&d.techs[best_tech].base.description)));
                }
            }
        }

        if k_event.golden_age {
            let length = self.player_get_golden_age_length(p);
            self.player_change_golden_age_turns(p, length);
        }

        if 0 != k_event.inflation_modifier {
            self.players[p].inflation_modifier += k_event.inflation_modifier;
        }

        if 0 != k_event.space_production_modifier {
            self.players[p].space_production_modifier += k_event.space_production_modifier;
        }

        if 0 != k_event.free_unit_support {
            self.players[p].base_free_units += k_event.free_unit_support;
        }

        if k_event.declare_war {
            if let Some(other) = data.other_player {
                let (team, other_team) = (self.players[p].team, self.players[other].team);
                self.team_declare_war(other_team, team, false, Some(WarPlanType::WARPLAN_LIMITED));
            }
        }

        if let Some(bonus) = k_event.bonus_gift {
            if let Some(other) = data.other_player {
                let our_list = [set_trade_item(TradeableItem::TRADE_RESOURCES, bonus.idx() as i32)];
                self.implement_deal(p, other, &our_list, &[], false);
            }
        }

        // C++ quirk: bClear keeps only the last roll (of the last event with a clear chance).
        let mut clear = false;
        for loop_event in d.events.keys() {
            if k_event.clear_event_chance[loop_event] > 0 {
                clear = self.soren_rand_num(100, "Event Clear") < k_event.clear_event_chance[loop_event];
                if clear {
                    if k_event.global {
                        for j in self.civ_player_ids() {
                            self.player_reset_event_occured(j, loop_event, j != p);
                        }
                    } else if k_event.team {
                        for j in self.civ_player_ids() {
                            if self.players[p].team == self.players[j].team {
                                self.player_reset_event_occured(j, loop_event, j != p);
                            }
                        }
                    } else {
                        self.player_reset_event_occured(p, loop_event, false);
                    }
                }
            }
        }

        // (Nothing above destroys cities; the checks only guard the copies of the ids.)
        let city = city.filter(|&c| self.cities.contains(c));
        let other_player_city = other_player_city.filter(|&c| self.cities.contains(c));

        if let (Some(c), true) = (city, k_event.city_effect) {
            self.city_apply_event(c, event, &data, clear);
        } else if let (Some(c), true) = (other_player_city, k_event.other_player_city_effect) {
            self.city_apply_event(c, event, &data, clear);
        }

        if !k_event.city_effect && !k_event.other_player_city_effect {
            if k_event.happy != 0 {
                self.player_change_extra_happiness(p, k_event.happy);
            }

            if k_event.health != 0 {
                self.player_change_extra_health(p, k_event.health);
            }

            if !k_event.building_yield_changes.is_empty() {
                for building_class in d.building_classes.keys() {
                    for i in 0..YieldType::NUM {
                        let yield_type = YieldType::from_usize(i);
                        for loop_city in self.players[p].cities.clone() {
                            if self.cities.contains(loop_city) {
                                let change = k_event.building_yield_change(building_class, yield_type);
                                self.city_change_building_yield_change(loop_city, building_class, yield_type, change);
                            }
                        }
                    }
                }
            }

            if !k_event.building_commerce_changes.is_empty() {
                for building_class in d.building_classes.keys() {
                    for i in 0..CommerceType::NUM {
                        let commerce = CommerceType::from_usize(i);
                        for loop_city in self.players[p].cities.clone() {
                            if self.cities.contains(loop_city) {
                                let change = k_event.building_commerce_change(building_class, commerce);
                                self.city_change_building_commerce_change(loop_city, building_class, commerce, change);
                            }
                        }
                    }
                }
            }

            if !k_event.building_happy_changes.is_empty() {
                for building_class in d.building_classes.keys() {
                    let change = k_event.building_happy_change(building_class);
                    if 0 != change {
                        if let Some(building) = civ_building(self, p, building_class) {
                            self.player_change_extra_building_happiness(p, building, change);
                        }
                    }
                }
            }

            if !k_event.building_health_changes.is_empty() {
                for building_class in d.building_classes.keys() {
                    let change = k_event.building_health_change(building_class);
                    if 0 != change {
                        if let Some(building) = civ_building(self, p, building_class) {
                            self.player_change_extra_building_health(p, building, change);
                        }
                    }
                }
            }

            if k_event.hurry_anger != 0 {
                for loop_city in self.players[p].cities.clone() {
                    if self.cities.contains(loop_city) {
                        let change = k_event.hurry_anger * self.city_flat_hurry_anger_length(loop_city);
                        self.city_change_hurry_anger_timer(loop_city, change);
                    }
                }
            }

            if k_event.happy_turns > 0 {
                for loop_city in self.players[p].cities.clone() {
                    if self.cities.contains(loop_city) {
                        self.city_change_happiness_timer(loop_city, k_event.happy_turns);
                    }
                }
            }

            if k_event.max_pillage > 0 {
                debug_assert!(k_event.max_pillage >= k_event.min_pillage);
                let num_pillage = k_event.min_pillage + self.soren_rand_num(k_event.max_pillage - k_event.min_pillage, "Pick number of event pillaged plots");

                let mut num_pillaged = 0;
                let num_plots = self.map.num_plots() as i32;
                for _ in 0..num_pillage {
                    let rand_offset = self.soren_rand_num(num_plots, "Pick event pillage plot (any city)");
                    for j in 0..num_plots {
                        let plot_index = (j + rand_offset) % num_plots;
                        if let Some(loop_plot) = self.map.plot_by_index(plot_index) {
                            if self.map.plots[loop_plot].owner == Some(p) && self.plot_is_city(loop_plot, false, None) {
                                if let Some(improvement) = self.map.plots[loop_plot].improvement_type {
                                    if !d.improvements[improvement].permanent {
                                        // UI: TXT_KEY_EVENT_CITY_IMPROVEMENT_DESTROYED to us.
                                        let turn = self.game_turn();
                                        self.log.msg(turn, Some(p), || format!("{} destroyed", d.text.name(&d.improvements[improvement].base.description)));
                                        self.plot_set_improvement_type(loop_plot, None);
                                        num_pillaged += 1;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(other) = data.other_player {
                    // UI: TXT_KEY_EVENT_NUM_CITY_IMPROVEMENTS_DESTROYED to the other player.
                    let turn = self.game_turn();
                    self.log.msg(turn, Some(other), || format!("{num_pillaged} city improvements destroyed"));
                }
            }

            if k_event.food != 0 {
                for loop_city in self.players[p].cities.clone() {
                    if self.cities.contains(loop_city) {
                        self.cities[loop_city].food += k_event.food;
                    }
                }
            }

            if k_event.food_percent != 0 {
                for loop_city in self.players[p].cities.clone() {
                    if self.cities.contains(loop_city) {
                        let food = self.cities[loop_city].food;
                        self.cities[loop_city].food += (food * k_event.food_percent) / 100;
                    }
                }
            }

            if k_event.population_change != 0 {
                for loop_city in self.players[p].cities.clone() {
                    if self.cities.contains(loop_city) {
                        if self.cities[loop_city].population + k_event.population_change > 0 {
                            self.city_change_population(loop_city, k_event.population_change);
                        }
                    }
                }
            }

            if k_event.culture != 0 {
                for loop_city in self.players[p].cities.clone() {
                    if self.cities.contains(loop_city) {
                        let owner = self.cities[loop_city].owner;
                        if self.cities[loop_city].culture[owner] + 100 * k_event.culture > 0 {
                            self.city_change_culture(loop_city, owner, k_event.culture, true, true);
                        }
                    }
                }
            }

            if let Some(unit_class) = k_event.unit_class {
                if let Some(unit_type) = civ_unit(self, p, unit_class) {
                    let mut unit_city = city.filter(|&c| self.cities.contains(c));

                    if unit_city.is_none() {
                        unit_city = self.player_get_capital_city(p);
                    }

                    if let Some(unit_city) = unit_city {
                        for _ in 0..k_event.num_units {
                            // FfH: Modified by Kael 10/29/2007 (the event's promotion and experience)
                            let (x, y) = (self.cities[unit_city].x, self.cities[unit_city].y);
                            let new_unit = self.player_init_unit(p, unit_type, x, y, None, None);
                            if let Some(promotion) = k_event.unit_promotion {
                                if self.units.contains(new_unit) {
                                    self.unit_set_has_promotion(new_unit, promotion, true);
                                }
                            }
                            if k_event.unit_experience != 0 {
                                if self.units.contains(new_unit) {
                                    self.unit_change_experience(new_unit, k_event.unit_experience, -1, false, false, false);
                                }
                            }
                            // FfH: End Modify
                        }
                    }
                }
            }
        }

        let plot = self.map.plot_xy(data.plot_x, data.plot_y);
        if let Some(plot) = plot {
            if is_plot_event_trigger(d, data.trigger) {
                // (C++ FAssert: pPlot->canApplyEvent(eEvent).)
                self.plot_apply_event(plot, event);
            }
        }

        if let Some(unit) = player_unit(self, p, data.unit) {
            // (C++ FAssert: pUnit->canApplyEvent(eEvent).)
            self.unit_apply_event(unit, event); // might kill the unit
        }

        for unit_combat in d.unit_combats.keys() {
            if let Some(promotion) = k_event.unit_combat_promotion[unit_combat] {
                for loop_unit in self.players[p].units.clone() {
                    if self.units.contains(loop_unit) {
                        if self.unit_get_unit_combat_type(loop_unit) == Some(unit_combat) {
                            self.unit_set_has_promotion(loop_unit, promotion, true);
                        }
                    }
                }

                self.player_set_free_promotion_unit_combat(p, unit_combat, promotion, true);
            }
        }

        for unit_class in d.unit_classes.keys() {
            if let Some(promotion) = k_event.unit_class_promotion[unit_class] {
                for loop_unit in self.players[p].units.clone() {
                    if self.units.contains(loop_unit) {
                        if self.unit_get_unit_class_type(loop_unit) == unit_class {
                            self.unit_set_has_promotion(loop_unit, promotion, true);
                        }
                    }
                }

                self.player_set_free_promotion_unit_class(p, unit_class, promotion, true);
            }
        }

        if let Some(bonus) = k_event.bonus_revealed {
            let team = self.players[p].team;
            self.team_set_force_revealed_bonus(team, bonus, true);
        }

        let mut spread_religion_cities: Vec<CityId> = Vec::new();

        if k_event.convert_own_cities > 0 {
            if let Some(religion) = data.religion {
                for &loop_city in &self.players[p].cities {
                    if !self.cities[loop_city].has_religion[religion] {
                        if -1 == k_event.max_num_religions || self.city_religion_count(loop_city) <= k_event.max_num_religions {
                            spread_religion_cities.push(loop_city);
                        }
                    }
                }
            }
        }

        while spread_religion_cities.len() as i32 > k_event.convert_own_cities {
            let chosen = self.soren_rand_num(spread_religion_cities.len() as i32, "Even Spread Religion (own)");
            spread_religion_cities.remove(chosen as usize);
        }

        if let Some(religion) = data.religion {
            for &loop_city in &spread_religion_cities {
                if self.cities.contains(loop_city) {
                    self.city_set_has_religion(loop_city, religion, true, true, false);
                }
            }
        }

        spread_religion_cities.clear();

        if k_event.convert_other_cities > 0 {
            if let Some(religion) = data.religion {
                if let Some(other) = data.other_player {
                    for &loop_city in &self.players[other].cities {
                        if !self.cities[loop_city].has_religion[religion] {
                            if -1 == k_event.max_num_religions || self.city_religion_count(loop_city) <= k_event.max_num_religions {
                                spread_religion_cities.push(loop_city);
                            }
                        }
                    }
                }
            }
        }

        while spread_religion_cities.len() as i32 > k_event.convert_other_cities {
            let chosen = self.soren_rand_num(spread_religion_cities.len() as i32, "Even Spread Religion (other)");
            spread_religion_cities.remove(chosen as usize);
        }

        if let Some(religion) = data.religion {
            for &loop_city in &spread_religion_cities {
                if self.cities.contains(loop_city) {
                    self.city_set_has_religion(loop_city, religion, true, true, false);
                }
            }
        }

        if 0 != k_event.our_attitude_modifier {
            if let Some(other) = data.other_player {
                if k_event.our_attitude_modifier > 0 {
                    change_memory_count(self, p, other, MemoryType::MEMORY_EVENT_GOOD_TO_US, k_event.our_attitude_modifier);
                } else {
                    change_memory_count(self, p, other, MemoryType::MEMORY_EVENT_BAD_TO_US, -k_event.our_attitude_modifier);
                }
            }
        }

        if 0 != k_event.attitude_modifier {
            if let Some(other) = data.other_player {
                if k_event.attitude_modifier > 0 {
                    change_memory_count(self, other, p, MemoryType::MEMORY_EVENT_GOOD_TO_US, k_event.attitude_modifier);
                } else {
                    change_memory_count(self, other, p, MemoryType::MEMORY_EVENT_BAD_TO_US, -k_event.attitude_modifier);
                }
            }
        }

        if 0 != k_event.their_enemy_attitude_modifier {
            if let Some(other) = data.other_player {
                let worst_enemy = self.teams[self.players[other].team].ai_worst_enemy;
                if let Some(worst_enemy) = worst_enemy {
                    for i in self.civ_player_ids() {
                        if self.players[i].alive && self.players[i].team == worst_enemy {
                            let modifier = k_event.their_enemy_attitude_modifier;
                            if modifier > 0 {
                                change_memory_count(self, i, p, MemoryType::MEMORY_EVENT_GOOD_TO_US, modifier);
                                change_memory_count(self, p, i, MemoryType::MEMORY_EVENT_GOOD_TO_US, modifier);
                            } else {
                                change_memory_count(self, i, p, MemoryType::MEMORY_EVENT_BAD_TO_US, -modifier);
                                change_memory_count(self, p, i, MemoryType::MEMORY_EVENT_BAD_TO_US, -modifier);
                            }
                        }
                    }
                }
            }
        }

        // FfH: Added by Kael 02/23/2008
        if k_event.global_counter != 0 {
            self.change_global_counter(k_event.global_counter);
        }
        // FfH: End Add

        if !k_event.python_callback.is_empty() {
            self.py_event_callback(event, &data);
        }

        if !k_event.world_news.is_empty() {
            let chosen = self.soren_rand_num(k_event.world_news.len() as i32, "Event World News choice");

            // UI: the world news goes to every alive player who has met us (and the other
            // player), plus a replay message.
            let turn = self.game_turn();
            self.log.msg(turn, None, || event_text(d, &k_event.world_news[chosen as usize]));
        }

        // UI: the event's local info text (to us) and the other player's popup.

        let mut delete_trigger = update_trigger;

        for loop_event in d.events.keys() {
            if 0 == k_event.additional_event_time[loop_event] {
                if k_event.additional_event_chance[loop_event] > 0 {
                    if self.player_can_do_event(p, loop_event, &data) {
                        if self.soren_rand_num(100, "Additional Event") < k_event.additional_event_chance[loop_event] {
                            self.player_apply_event(p, loop_event, triggered, false);
                        }
                    }
                }
            } else {
                let mut set_timer = true;
                if k_event.additional_event_chance[loop_event] > 0 {
                    if self.soren_rand_num(100, "Additional Event 2") >= k_event.additional_event_chance[loop_event] {
                        set_timer = false;
                    }
                }

                if set_timer {
                    let mut k_triggered = data.clone();
                    k_triggered.turn = (d.game_speeds[self.game_speed_type()].growth_percent * k_event.additional_event_time[loop_event]) / 100 + self.game_turn();

                    if let Some(existing_triggered) = self.player_get_event_countdown(p, loop_event) {
                        k_triggered.turn = min(k_triggered.turn, existing_triggered.turn);
                    }

                    self.player_set_event_countdown(p, loop_event, &k_triggered);
                    delete_trigger = false;
                }
            }
        }

        if delete_trigger {
            self.player_delete_event_triggered(p, triggered);
        }
    }

    /// C++ `CvPlayer::isValidEventTech(eTech, eEvent, eOtherPlayer)`.
    pub fn player_is_valid_event_tech(&self, p: PlayerId, tech: TechType, event: EventType, other: Option<PlayerId>) -> bool {
        let k_event = &self.d.events[event];
        let team = self.players[p].team;

        if 0 == k_event.tech_percent && 0 == k_event.tech_cost_percent {
            return false;
        }

        if k_event.tech_percent < 0 && self.team_get_research_progress(team, Some(tech)) <= 0 {
            return false;
        }

        if !self.player_can_research(p, tech, false) {
            return false;
        }

        if self.player_get_research_turns_left(p, tech, true) < k_event.tech_min_turns_left {
            return false;
        }

        if let Some(other) = other {
            if !self.team_is_has_tech(self.players[other].team, Some(tech)) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::getBestEventTech(eEvent, eOtherPlayer)`.
    pub fn player_get_best_event_tech(&self, p: PlayerId, event: EventType, other: Option<PlayerId>) -> Option<TechType> {
        let d = self.d;
        let mut best_tech = None;
        let k_event = &d.events[event];

        if 0 == k_event.tech_percent && 0 == k_event.tech_cost_percent {
            return None;
        }

        let num_flavor_types = d.reg.count::<FlavorType>();

        if k_event.tech.is_some() {
            best_tech = k_event.tech;
        } else {
            let mut found_flavor = false;
            for i in 0..num_flavor_types {
                if 0 != k_event.tech_flavor_value[FlavorType::from_usize(i)] {
                    found_flavor = true;
                    break;
                }
            }

            if !found_flavor {
                best_tech = self.player_get_current_research(p);
            }
        }

        if let Some(tech) = best_tech {
            if !self.player_is_valid_event_tech(p, tech, event, other) {
                best_tech = None;
            }
        } else {
            let mut best_value = 0;
            for tech in d.techs.keys() {
                if self.player_is_valid_event_tech(p, tech, event, other) {
                    let mut value = 0;
                    for i in 0..num_flavor_types {
                        let flavor = FlavorType::from_usize(i);
                        value += k_event.tech_flavor_value[flavor] * d.techs[tech].flavor_value[flavor];
                    }

                    if value > best_value {
                        best_tech = Some(tech);
                        best_value = value;
                    }
                }
            }
        }

        best_tech
    }

    /// C++ `CvPlayer::getEventCost(eEvent, eOtherPlayer, bRandom)` (gold; negative = income).
    pub fn player_get_event_cost(&self, p: PlayerId, event: EventType, other: Option<PlayerId>, random: bool) -> i32 {
        let k_event = &self.d.events[event];

        let mut gold = k_event.gold;
        if random {
            gold += k_event.random_gold;
        }

        gold *= max(0, self.player_calculate_inflation_rate(p) + 100);
        gold /= 100;

        let best_tech = self.player_get_best_event_tech(p, event, other);

        if let Some(best_tech) = best_tech {
            gold -= (k_event.tech_cost_percent * self.team_get_research_cost(self.players[p].team, best_tech)) / 100;
        }

        gold
    }

    // ------------------------------------------------------------------ expiry

    /// C++ `CvPlayer::checkExpireEvent(eEvent, kTriggeredData)` (Python expire checks; a
    /// non-quest expires after 2 turns; a quest when its state religion/civic, city, unit or
    /// other player is lost, or a trigger obsolete tech is known).
    pub fn player_check_expire_event(&self, p: PlayerId, event: EventType, data: &EventTriggeredData) -> bool {
        let d = self.d;
        let k_event = &d.events[event];

        if !k_event.python_expire_check.is_empty() {
            if self.py_event_expire_check(event, data) {
                return true;
            }
        }

        if !k_event.quest {
            if self.game_turn() - data.turn > 2 {
                return true;
            }

            return false;
        }

        let k_trigger = &d.event_triggers[data.trigger];

        let k_player = data.player;

        if k_trigger.state_religion & k_trigger.pick_religion {
            if self.player_get_state_religion(k_player) != data.religion {
                return true;
            }
        }

        if let Some(civic) = k_trigger.civic {
            if !self.player_is_civic(k_player, civic) {
                return true;
            }
        }

        if data.city.is_some() {
            if player_city(self, k_player, data.city).is_none() {
                return true;
            }
        }

        if data.unit.is_some() {
            if player_unit(self, k_player, data.unit).is_none() {
                return true;
            }
        }

        if let Some(other) = data.other_player {
            if !self.players[other].alive {
                return true;
            }

            if data.other_player_city.is_some() {
                if player_city(self, other, data.other_player_city).is_none() {
                    return true;
                }
            }
        }

        // (GET_TEAM(getTeam()): our team, not the triggered data's player's.)
        let team = self.players[p].team;
        for &tech in &k_trigger.obsolete_techs {
            if self.team_is_has_tech(team, Some(tech)) {
                return true;
            }
        }

        false
    }

    /// C++ `CvPlayer::expireEvent(eEvent, kTriggeredData, bFail)`: UI only (a quest's turn-log
    /// message is removed and, with `fail`, its quest-fail text shown).
    pub fn player_expire_event(&mut self, p: PlayerId, event: EventType, _data: &EventTriggeredData, fail: bool) {
        // (C++ FAsserts: the data is this player's entry for the event; a non-quest expires
        // within 4 turns.)
        let d = self.d;
        if d.events[event].quest {
            // UI: the quest's message (the one whose length field holds the triggered id) is
            // removed from the turn log; with bFail the quest-fail text is shown.
            if fail {
                let turn = self.game_turn();
                self.log.msg(turn, Some(p), || format!("Quest failed: {}", d.text.name(&d.events[event].base.description)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// A game whose first `civs.len()` slots hold freshly reset players, alive on their own
    /// teams, with the given civilizations/leaders; other slots stay placeholders.
    fn game(civs: &[(&str, &str)]) -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, civs.len(), "WORLDSIZE_DUEL", 7);
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
            g.players[p].ever_alive = true;
            let t = g.players[p].team;
            g.teams[t].num_members = 1;
            g.teams[t].alive_count = 1;
            g.teams[t].ever_alive_count = 1;
        }
        g
    }

    fn two_civs() -> Game {
        game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_LJOSALFAR", "LEADER_ARENDEL")])
    }

    /// Triggered data for `trigger` with nothing picked.
    fn triggered(g: &Game, p: PlayerId, trigger: EventTriggerType) -> EventTriggeredData {
        EventTriggeredData {
            id: TriggeredId(0),
            trigger,
            turn: g.game_turn(),
            player: p,
            city: None,
            plot_x: INVALID_PLOT_COORD,
            plot_y: INVALID_PLOT_COORD,
            unit: None,
            other_player: None,
            other_player_city: None,
            religion: None,
            corporation: None,
            building: None,
            text: String::new(),
            global_text: String::new(),
        }
    }

    #[test]
    fn trigger_weight_checks_fired_inactive_and_recurring_triggers() {
        let mut g = two_civs();
        let d = g.d;
        let p = PlayerId(0);
        // triggers whose only condition is their weight
        let texts = d.t::<EventTriggerType>("EVENTTRIGGER_ANCIENT_TEXTS");
        let duke = d.t::<EventTriggerType>("EVENTTRIGGER_AFFLUENT_DUKE");
        assert!(!d.event_triggers[texts].recurring && d.event_triggers[duke].recurring);
        assert_eq!(g.player_get_event_trigger_weight(p, texts), d.event_triggers[texts].probability);
        assert_eq!(g.player_get_event_trigger_weight(p, duke), d.event_triggers[duke].probability);

        // a fired trigger fires again only if it is recurring
        g.players[p].triggers_fired.extend([texts, duke]);
        assert_eq!(g.player_get_event_trigger_weight(p, texts), 0);
        assert_eq!(g.player_get_event_trigger_weight(p, duke), d.event_triggers[duke].probability);

        // an inactive trigger never fires
        g.inactive_triggers.push(duke);
        assert_eq!(g.player_get_event_trigger_weight(p, duke), 0);
    }

    #[test]
    fn global_trigger_is_shared_with_alive_players_and_fires_once_per_game() {
        let mut g = two_civs();
        let d = g.d;
        let champion = d.t::<EventTriggerType>("EVENTTRIGGER_CHAMPION");
        assert!(d.event_triggers[champion].global && d.event_triggers[champion].python_callback.is_empty());
        let (p0, p1, open) = (PlayerId(0), PlayerId(1), PlayerId(2));
        assert!(!g.players[open].alive);
        assert_eq!(g.player_get_event_trigger_weight(p1, champion), d.event_triggers[champion].probability);

        let data = triggered(&g, p0, champion);
        g.player_set_trigger_fired(p0, &data, true, false);
        assert!(g.player_is_trigger_fired(p0, champion) && g.player_is_trigger_fired(p1, champion));
        assert!(!g.player_is_trigger_fired(open, champion));
        assert!(g.event_triggered[champion]);
        assert_eq!(g.player_get_event_trigger_weight(p1, champion), 0);

        // FfH: once triggered in this game, a global trigger is out for everyone
        g.players[p1].triggers_fired.clear();
        assert_eq!(g.player_get_event_trigger_weight(p1, champion), 0);

        // recording it again does not duplicate it
        g.player_set_trigger_fired(p0, &data, true, false);
        assert_eq!(g.players[p0].triggers_fired.iter().filter(|&&t| t == champion).count(), 1);
        g.player_reset_trigger_fired(p0, champion);
        assert!(!g.player_is_trigger_fired(p0, champion));
    }

    #[test]
    fn can_trigger_needs_contact_and_the_trigger_war_state() {
        let mut g = two_civs();
        let d = g.d;
        let faux_pas = d.t::<EventTriggerType>("EVENTTRIGGER_FAUX_PAS");
        let k_trigger = &d.event_triggers[faux_pas];
        assert!(k_trigger.pick_player && !k_trigger.other_player_war && !k_trigger.other_player_ai);
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let (t0, t1) = (g.players[p0].team, g.players[p1].team);

        assert!(!g.player_can_trigger(p1, faux_pas, p0, None));
        g.teams[t1].has_met[t0] = true;
        assert!(g.player_can_trigger(p1, faux_pas, p0, None));
        assert!(!g.player_can_trigger(p0, faux_pas, p0, None));

        g.teams[t1].at_war[t0] = true;
        assert!(!g.player_can_trigger(p1, faux_pas, p0, None));
        g.teams[t1].at_war[t0] = false;

        g.players[p1].alive = false;
        assert!(!g.player_can_trigger(p1, faux_pas, p0, None));

        // a trigger that does not pick a player
        let texts = d.t::<EventTriggerType>("EVENTTRIGGER_ANCIENT_TEXTS");
        g.players[p1].alive = true;
        assert!(!g.player_can_trigger(p1, texts, p0, None));
    }

    #[test]
    fn event_cost_applies_inflation_to_gold_and_random_gold() {
        let mut g = two_civs();
        let d = g.d;
        let p = PlayerId(0);
        let e = d.t::<EventType>("EVENT_GIRLS_BEST_FRIEND_1");
        let k_event = &d.events[e];
        assert!(k_event.random_gold != 0 && k_event.tech_percent == 0 && k_event.tech_cost_percent == 0);

        let factor = max(0, g.player_calculate_inflation_rate(p) + 100);
        assert_eq!(g.player_get_event_cost(p, e, None, false), k_event.gold * factor / 100);
        assert_eq!(g.player_get_event_cost(p, e, None, true), (k_event.gold + k_event.random_gold) * factor / 100);

        g.setup.options[GameOptionType::GAMEOPTION_NO_INFLATION] = true;
        assert_eq!(g.player_get_event_cost(p, e, None, false), k_event.gold);
        assert_eq!(g.player_get_best_event_tech(p, e, None), None);
    }

    #[test]
    fn non_quest_events_expire_after_two_turns_and_quests_with_their_targets() {
        let mut g = two_civs();
        let d = g.d;
        let p = PlayerId(0);
        g.setup.game_turn = 10;

        let duke = d.t::<EventTriggerType>("EVENTTRIGGER_AFFLUENT_DUKE");
        let duke_3 = d.t::<EventType>("EVENT_AFFLUENT_DUKE_3");
        assert!(!d.events[duke_3].quest && d.events[duke_3].python_expire_check.is_empty());
        let mut data = triggered(&g, p, duke);
        data.turn = 8;
        assert!(!g.player_check_expire_event(p, duke_3, &data));
        data.turn = 7;
        assert!(g.player_check_expire_event(p, duke_3, &data));

        let olympics = d.t::<EventTriggerType>("EVENTTRIGGER_ANCIENTOLYMPICS");
        let quest = d.t::<EventType>("EVENT_ANCIENTOLYMPICS_2");
        assert!(d.events[quest].quest && d.events[quest].python_expire_check.is_empty());
        let mut quest_data = triggered(&g, p, olympics);
        quest_data.turn = 0;
        assert!(!g.player_check_expire_event(p, quest, &quest_data));
        // its city is gone
        quest_data.city = Some(CityId(999));
        assert!(g.player_check_expire_event(p, quest, &quest_data));
        quest_data.city = None;
        // its other player is dead
        quest_data.other_player = Some(PlayerId(1));
        assert!(!g.player_check_expire_event(p, quest, &quest_data));
        g.players[PlayerId(1)].alive = false;
        assert!(g.player_check_expire_event(p, quest, &quest_data));
    }

    #[test]
    fn events_occured_are_recorded_and_reset() {
        let mut g = two_civs();
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let duke = d.t::<EventTriggerType>("EVENTTRIGGER_AFFLUENT_DUKE");
        let duke_3 = d.t::<EventType>("EVENT_AFFLUENT_DUKE_3");
        assert!(!d.events[duke_3].global && !d.events[duke_3].team);

        let data = triggered(&g, p0, duke);
        g.player_set_event_occured(p0, duke_3, &data, true);
        assert_eq!(g.player_get_event_occured(p0, duke_3), Some(&data));
        assert!(g.player_get_event_occured(p1, duke_3).is_none());

        g.player_reset_event_occured(p0, duke_3, true);
        assert!(g.player_get_event_occured(p0, duke_3).is_none());

        let mut later = data.clone();
        later.turn = 5;
        g.player_set_event_countdown(p0, duke_3, &later);
        assert_eq!(g.player_get_event_countdown(p0, duke_3).map(|t| t.turn), Some(5));
        g.player_reset_event_countdown(p0, duke_3);
        assert!(g.player_get_event_countdown(p0, duke_3).is_none());
    }

    #[test]
    fn nothing_to_do_draws_nothing() {
        let mut g = two_civs();
        let d = g.d;
        let p = PlayerId(0);
        let seed = g.soren_rand.seed();

        // barbarians and GAMEOPTION_NO_EVENTS: no events at all
        let barbarian = g.barbarian_player();
        g.player_do_events(barbarian);
        assert_eq!(g.soren_rand.seed(), seed);

        // no cities or units to pick from
        let texts = d.t::<EventTriggerType>("EVENTTRIGGER_ANCIENT_TEXTS");
        assert_eq!(g.player_pick_trigger_city(p, texts), None);
        assert_eq!(g.player_pick_trigger_unit(p, texts, None, false), None);
        assert_eq!(g.soren_rand.seed(), seed);

        // applying an event of triggered data that is gone does nothing
        let duke_3 = d.t::<EventType>("EVENT_AFFLUENT_DUKE_3");
        g.player_apply_event(p, duke_3, TriggeredId(42), true);
        assert!(g.players[p].triggers_fired.is_empty() && g.players[p].events_occured.is_empty());
        assert_eq!(g.soren_rand.seed(), seed);

        g.setup.options[GameOptionType::GAMEOPTION_NO_EVENTS] = true;
        g.player_do_events(p);
        assert_eq!(g.soren_rand.seed(), seed);
    }

    #[test]
    fn triggered_data_ids_are_never_reused() {
        let mut g = two_civs();
        let d = g.d;
        let p = PlayerId(0);
        let duke = d.t::<EventTriggerType>("EVENTTRIGGER_AFFLUENT_DUKE");
        let a = g.player_add_event_triggered(p, triggered(&g, p, duke));
        let b = g.player_add_event_triggered(p, triggered(&g, p, duke));
        assert_ne!(a, b);
        assert_eq!(g.player_get_event_triggered(p, a).map(|t| t.id), Some(a));
        g.player_delete_event_triggered(p, a);
        assert!(g.player_get_event_triggered(p, a).is_none());
        let c = g.player_add_event_triggered(p, triggered(&g, p, duke));
        assert!(c != a && c != b);
    }
}
