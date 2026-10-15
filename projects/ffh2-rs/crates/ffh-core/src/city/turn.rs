//! CvCity: lifecycle (`init`, `kill`), turn processing (`doTurn`, `doTask`), the build queue
//! (`pushOrder`/`popOrder`, `doCheckProduction`, `doProduction`, `doDecay`), events,
//! liberation and FFH2 misc (meltdown, crime).

use super::{ai_choose_production, ai_reset, building_class_of, order_type_data, unit_class_of, City};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::is_world_unit_class;
use ffh_data::*;

impl Game {
    /// C++ `CvCity::init` (CvCity.cpp:146-348): founds city `c` for `owner` at (`x`, `y`).
    ///
    /// Calling convention: `c` is a slot of `self.cities` created by the caller
    /// (`player_init_city` = C++ `CvPlayer::initCity`); like C++ `init` -> `reset`, this
    /// overwrites it with `City::new(..)`.
    ///
    /// RNG: C++ `reset()` draws `getSorenRandNum(20, "Crime")`, and it runs twice per new
    /// city: first from the CvCity constructor (`FFreeListTrashArray::add`, value discarded),
    /// then from `init`. This function must perform both draws, in that order, first.
    ///
    /// C++ ends with `AI_init()`: `AI_reset()` (which clears the AI bookkeeping fields again,
    /// e.g. an `ai_choose_production_dirty` set by `setPopulation` during init), then
    /// `AI_assignWorkingPlots` (governor) and AI-only caches (omitted).
    pub fn city_init(&mut self, c: CityId, owner: PlayerId, x: i32, y: i32, bump_units: bool, update_plot_groups: bool) {
        let d = self.d;
        let defines = &d.defines;

        let plot = self.map.plot_xy(x, y).expect("CvCity::init: the city plot is off the map");
        let (plot_x, plot_y) = self.map.xy(plot);

        // Logging (cheat level only): not ported.

        // Init saved data: reset(iID, eOwner, pPlot->getX_INLINE(), pPlot->getY_INLINE()). The
        // first "Crime" draw is the constructor's reset (value discarded).
        let _ = self.soren_rand_num(20, "Crime");
        let crime = self.soren_rand_num(20, "Crime");
        self.cities[c] = City::new(d, self.num_players(), self.num_teams(), c, owner, plot_x, plot_y, crime);

        // Init non-saved data: setupGraphical() (graphics).

        // Init other game data
        let name = self.player_get_new_city_name(owner);
        self.city_set_name(c, &name, false);

        self.cities[c].ever_owned[owner] = true;

        self.city_update_culture_level(c, false);

        if self.map.plots[plot].culture[owner] < defines.free_city_culture {
            self.plot_set_culture(plot, owner, defines.free_city_culture, bump_units, false);
        }
        self.plot_set_owner(plot, Some(owner), bump_units, false);
        self.plot_set_plot_city(plot, Some(c));

        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(plot_x, plot_y, DirectionType::from_usize(i)) {
                if self.map.plots[adjacent].culture[owner] < defines.free_city_adjacent_culture {
                    self.plot_set_culture(adjacent, owner, defines.free_city_adjacent_culture, bump_units, false);
                }
                self.plot_update_culture(adjacent, bump_units, false);
            }
        }

        let team = self.city_team(c);
        for t in self.team_ids() {
            if self.teams[team].vassal[t] {
                self.plot_change_adjacent_sight(plot, t, defines.plot_visibility_range, true, None, false);
            }
        }

        if py_cities_destroy_features(self, x, y) {
            // UNOFFICIAL_PATCH 10/30/09 Mongoose & jdog5000: don't remove flood plains from
            // tiles when founding a city.
            // C++ quirk: FLOOD_PLAINS_FEATURE is not among the text defines converted to type
            // indices (SetPostGlobalsGlobalDefines), so getDefineINT reads the string's address,
            // which matches no feature: flood plains ARE removed. The define reads as -1 here.
            if let Some(feature) = self.map.plots[plot].feature_type {
                if Some(feature) != opt_from_i32(defines.flood_plains_feature) {
                    self.plot_set_feature_type(plot, None);
                }
            }
        }

        self.plot_set_improvement_type(plot, None);
        self.plot_update_city_route(plot, false);

        for t in self.team_ids() {
            if self.team_is_alive(t) && self.plot_is_visible(plot, t) {
                // setRevealed(eTeam, true): the other effects are interface updates.
                self.cities[c].revealed[t] = true;
            }
        }

        let military_happiness_units = self.plot_plot_count(plot, Some(Game::puf_is_military_happiness), -1, -1, None, None, None, -1, -1);
        self.city_change_military_happiness_units(c, military_happiness_units);

        for i in 0..CommerceType::NUM {
            let commerce = CommerceType::from_usize(i);
            self.city_change_commerce_happiness_per(c, commerce, d.commerces[commerce].initial_happiness);
        }

        for building in d.buildings.keys() {
            if self.player_is_building_free(owner, building) {
                self.city_set_num_free_building(c, building, 1);
            }
        }

        let area = self.city_area(c);
        self.area_change_cities_per_player(area, owner, 1);

        self.teams[team].num_cities += 1;

        self.num_cities += 1;

        let game_turn = self.game_turn();
        self.city_set_game_turn_founded(c, game_turn);
        self.cities[c].game_turn_acquired = game_turn;

        let start_era = self.start_era();
        self.city_change_population(c, defines.initial_city_population + d.eras[start_era].free_population);

        self.cities[c].air_unit_capacity += defines.city_air_unit_capacity;

        self.city_update_fresh_water_health(c);
        self.city_update_feature_health(c);
        self.city_update_feature_happiness(c);
        self.city_update_power_health(c);

        // FfH: Added by Kael 11/07/2007
        let max_cities = self.players[owner].max_cities;
        if max_cities != -1 {
            let num_cities = self.players[owner].num_cities();
            if num_cities - self.player_get_num_settlements(owner) - 1 >= max_cities {
                self.cities[c].settlement = true;
            } else if !self.players[owner].human || num_cities == 1 || self.is_game_multi_player() {
                self.city_set_plot_radius(c, 3);
            } else if self.players[owner].human && num_cities != 1 {
                // BUTTONPOPUP_CONFIRMSETTLEMENT (`OnOkClicked`): "city" (button 0,
                // setPlotRadius(3)) or "settlement" (button 1, setSettlement(true)), the
                // controller's `confirm_settlement`. C++ applies the answer when the human clicks,
                // after the city is founded; here it is applied where the AI gets radius 3. No
                // answer: the city stays as it is (radius 2, not a settlement).
                match self.controller(owner).confirm_settlement(self, owner, c) {
                    Some(false) => self.city_set_plot_radius(c, 3),
                    Some(true) => self.cities[c].settlement = true,
                    None => {}
                }
            }
        }
        self.cities[c].civ = self.player_get_civilization_type(owner);
        // FfH: End Add

        self.player_update_maintenance(owner);
        self.map_update_working_city();
        game_ai_make_assign_work_dirty(self);
        self.players[owner].founded_first_city = true;

        if self.final_initialized && self.players[owner].num_cities() == 1 {
            let civ = self.city_civilization_type(c);
            for building_class in d.building_classes.keys() {
                if d.civilizations[civ].civilization_free_building_class[building_class] {
                    if let Some(building) = d.civ_building(civ, building_class) {
                        self.city_set_num_real_building(c, building, 1);
                    }
                }
            }
            if !self.city_is_human(c) {
                self.city_change_overflow_production(c, defines.initial_ai_city_production, 0);
            }
        }

        self.city_update_espionage_visibility(c, false);

        if update_plot_groups {
            self.update_plot_groups();
        }

        // AI_init(): AI_reset(), AI_assignWorkingPlots(); AI: AI_updateWorkersNeededHere and
        // AI_updateBestBuild fill AI-only caches.
        ai_reset(&mut self.cities[c]);
        self.city_assign_working_plots(c);
    }

    /// C++ `CvCity::kill` (CvCity.cpp:732-912): destroys the city (the owner's
    /// `deleteCity` removes it from `self.cities`; `c` dangles afterwards). Python event
    /// `cityLost`.
    pub fn city_kill(&mut self, c: CityId, update_plot_groups: bool) {
        let d = self.d;
        let defines = &d.defines;

        // Interface: clear the city selection.

        let plot = self.city_plot(c);

        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.city_index_plot(c, i) {
                if self.map.plots[loop_plot].working_city_override == Some(c) {
                    self.plot_set_working_city_override(loop_plot, None);
                }
            }
        }

        self.city_set_culture_level(c, None, false);

        for building in d.buildings.keys() {
            // FfH: Added by Kael 08/08/2007
            if self.city_num_building(c, building) > 0 && d.buildings[building].modify_global_counter != 0 {
                self.change_global_counter(-d.buildings[building].modify_global_counter);
            }
            // FfH: End Add

            self.city_set_num_real_building(c, building, 0);
            self.city_set_num_free_building(c, building, 0);
        }

        for specialist in d.specialists.keys() {
            self.city_set_free_specialist_count(c, specialist, 0);
        }

        for i in 0..YieldType::NUM {
            let yield_type = YieldType::from_usize(i);
            self.city_set_trade_yield(c, yield_type, 0);
            self.city_set_corporation_yield(c, yield_type, 0);
        }

        for religion in d.religions.keys() {
            self.city_set_has_religion(c, religion, false, false, true);

            if self.city_is_holy_city(c, religion) {
                self.set_holy_city(religion, None, false);
            }
        }

        for corporation in d.corporations.keys() {
            self.city_set_has_corporation(c, corporation, false, false, true);

            if self.city_is_headquarters(c, corporation) {
                self.set_headquarters(corporation, None, false);
            }
        }

        self.city_set_population(c, 0);

        self.city_assign_working_plots(c);

        self.city_clear_order_queue(c);

        // remember the visibility before we take away the city from the plot below
        let espionage_visibility: Vec<bool> = self.cities[c].espionage_visibility.iter().copied().collect();

        // UNOFFICIAL_PATCH 08/04/09 jdog5000: clear the trade routes of the dead city, else they
        // are claimed for the owner forever.
        self.city_clear_trade_routes(c);

        self.plot_set_plot_city(plot, None);

        let area = self.city_area(c);
        let owner = self.cities[c].owner;
        self.area_change_cities_per_player(area, owner, -1);

        let team = self.city_team(c);
        self.teams[team].num_cities -= 1;

        self.num_cities -= 1;

        // FAsserts: no working population, specialists, great people, base yields, production.

        let capital = self.city_is_capital(c);

        self.plot_set_improvement_type(plot, opt_from_i32(defines.ruins_improvement));

        self.ev_city_lost(c);

        self.player_delete_city(owner, c);
        // `c` dangles from here on.

        self.plot_update_culture(plot, true, false);

        let (plot_x, plot_y) = self.map.xy(plot);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(plot_x, plot_y, DirectionType::from_usize(i)) {
                self.plot_update_culture(adjacent, true, false);
            }
        }

        let owner_team = self.players[owner].team;
        for t in self.team_ids() {
            if self.teams[owner_team].vassal[t] {
                self.plot_change_adjacent_sight(plot, t, defines.plot_visibility_range, false, None, false);
            }
        }

        for t in self.team_ids() {
            if espionage_visibility[t.idx()] {
                self.plot_change_adjacent_sight(plot, t, defines.plot_visibility_range, false, None, false);
            }
        }

        self.player_update_maintenance(owner);

        self.map_update_working_city();

        game_ai_make_assign_work_dirty(self);

        if capital {
            self.player_find_new_capital(owner);

            self.team_reset_victory_progress(owner_team);
        }

        if update_plot_groups {
            self.update_plot_groups();
        }

        // Interface: selection buttons dirty bit for the active player.
    }

    /// C++ `CvCity::doTurn` (CvCity.cpp:915-1150). `AI_doTurn` is AI (omit); FFH: caster
    /// buildings, remove/free promotions on units in the city. Python event `cityDoTurn`.
    pub fn city_do_turn(&mut self, c: CityId) {
        let d = self.d;
        let defines = &d.defines;

        if !self.cities[c].bombarded {
            self.city_change_defense_damage(c, -defines.city_defense_damage_heal_rate);
        }

        {
            let city = &mut self.cities[c];
            city.last_defense_damage = city.defense_damage;
            city.bombarded = false;
        }
        self.city_set_plundered(c, false);
        {
            let city = &mut self.cities[c];
            city.drafted = false;
            city.airlift_targeted = false;
            city.curr_airlift = 0;
        }
        // AI: AI_doTurn() (forced specialists of AI cities, plot stealing, worker/build/route
        // caches, panic, draft, hurry and emphasize decisions) is the controller's business.

        let allow_no_production = !self.city_do_check_production(c);

        // FfH: Modified by Kael 08/04/2007 (no growth for players that ignore food)
        let owner = self.cities[c].owner;
        if !self.players[owner].ignore_food {
            self.city_do_growth(c);
        }

        let plot = self.city_plot(c);
        for building in d.buildings.keys() {
            if self.city_num_building(c, building) > 0 {
                let info = &d.buildings[building];
                if info.requires_caster {
                    // The building stays only while a unit in the city can cast the spell that
                    // creates it.
                    let mut valid = false;
                    for unit in self.map.plots[plot].units.clone() {
                        if !self.units.contains(unit) {
                            continue;
                        }
                        for spell in d.spells.keys() {
                            // C++ tests `canCast` first; it is a pure query, so the cheap type
                            // test is done first here.
                            if d.spells[spell].create_building_type == Some(building) && self.unit_can_cast(unit, spell, true) {
                                valid = true;
                            }
                        }
                    }
                    if !valid {
                        self.city_set_num_real_building(c, building, 0);
                    }
                }
                if let Some(remove_promotion) = info.remove_promotion {
                    for unit in self.map.plots[plot].units.clone() {
                        if self.units.contains(unit) && self.units[unit].has_promotion[remove_promotion] {
                            self.unit_set_has_promotion(unit, remove_promotion, false);
                        }
                    }
                }
                if info.apply_free_promotion_on_move {
                    if let Some(free_promotion) = info.free_promotion {
                        for unit in self.map.plots[plot].units.clone() {
                            if !self.units.contains(unit) {
                                continue;
                            }
                            if let Some(unit_combat) = self.unit_get_unit_combat_type(unit) {
                                if d.promotions[free_promotion].unit_combat[unit_combat] {
                                    self.unit_set_has_promotion(unit, free_promotion, true);
                                }
                            }
                        }
                    }
                }
            }
        }
        // FfH: End Add

        self.city_do_culture(c);
        let culture_rate = self.city_commerce_rate(c, CommerceType::COMMERCE_CULTURE);
        let owner = self.cities[c].owner;
        self.city_do_plot_culture(c, false, owner, culture_rate);
        self.city_do_production(c, allow_no_production);
        self.city_do_decay(c);
        self.city_do_religion(c);
        self.city_do_great_people(c);
        self.city_do_meltdown(c);

        self.city_update_espionage_visibility(c, true);

        if !self.city_is_disorder(c) {
            for i in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.city_index_plot(c, i) {
                    if self.map.plots[loop_plot].working_city == Some(c) && self.plot_is_being_worked(loop_plot) {
                        self.plot_do_improvement(loop_plot);
                    }
                }
            }
        }

        if self.cities[c].culture_update_timer > 0 {
            self.cities[c].culture_update_timer -= 1;
        }

        if self.cities[c].occupation_timer > 0 {
            self.city_change_occupation_timer(c, -1);
        }

        if self.cities[c].hurry_anger_timer > 0 {
            self.city_change_hurry_anger_timer(c, -1);
        }

        if self.cities[c].conscript_anger_timer > 0 {
            self.city_change_conscript_anger_timer(c, -1);
        }

        if self.cities[c].defy_resolution_anger_timer > 0 {
            self.city_change_defy_resolution_anger_timer(c, -1);
        }

        if self.cities[c].happiness_timer > 0 {
            self.city_change_happiness_timer(c, -1);
        }

        if self.cities[c].espionage_health_counter > 0 {
            self.cities[c].espionage_health_counter -= 1;
        }

        if self.cities[c].espionage_happiness_counter > 0 {
            self.cities[c].espionage_happiness_counter -= 1;
        }

        if self.city_is_occupation(c) || self.city_angry_population(c, 0) > 0 || self.city_health_rate(c, false, 0) < 0 {
            self.city_set_we_love_the_king_day(c, false);
        } else if self.cities[c].population >= defines.we_love_the_king_population_min_population
            && self.soren_rand_num(defines.we_love_the_king_rand, "Do We Love The King?") < self.cities[c].population
        {
            self.city_set_we_love_the_king_day(c, true);
        } else {
            self.city_set_we_love_the_king_day(c, false);
        }

        // ONEVENT - Do turn
        let owner = self.cities[c].owner;
        self.ev_city_do_turn(c, owner);

        // The `_DEBUG` yield / commerce consistency asserts are not ported.
    }

    /// C++ `CvCity::doTask` (CvCity.cpp:1252-1327): applies a player command
    /// (`TaskType::TASK_*`). `TASK_SET_EMPHASIZE` -> `city_ai_set_emphasize`.
    #[allow(clippy::too_many_arguments)]
    pub fn city_do_task(&mut self, c: CityId, task: TaskType, data1: i32, data2: i32, option: bool, alt: bool, shift: bool, ctrl: bool) {
        match task {
            TaskType::TASK_RAZE => {
                let owner = self.cities[c].owner;
                self.player_raze(owner, c);
            }

            TaskType::TASK_DISBAND => {
                let owner = self.cities[c].owner;
                self.player_disband(owner, c);
            }

            TaskType::TASK_GIFT => {
                if opt_to_i32(self.city_liberation_player(c, false)) == data1 {
                    self.city_liberate(c, false);
                } else {
                    self.player_acquire_city(PlayerId::from_usize(data1 as usize), c, false, true, true);
                }
            }

            TaskType::TASK_LIBERATE => self.city_liberate(c, data1 != 0),

            TaskType::TASK_SET_AUTOMATED_CITIZENS => self.city_set_citizens_automated(c, option),

            TaskType::TASK_SET_AUTOMATED_PRODUCTION => self.city_set_production_automated(c, option, alt && shift && ctrl),

            TaskType::TASK_SET_EMPHASIZE => self.city_ai_set_emphasize(c, EmphasizeType::from_usize(data1 as usize), option),

            TaskType::TASK_CHANGE_SPECIALIST => self.city_alter_specialist_count(c, SpecialistType::from_usize(data1 as usize), data2),

            TaskType::TASK_CHANGE_WORKING_PLOT => self.city_alter_working_plot(c, data1 as usize),

            TaskType::TASK_CLEAR_WORKING_OVERRIDE => self.city_clear_working_override(c, data1 as usize),

            TaskType::TASK_HURRY => self.city_hurry(c, HurryType::from_usize(data1 as usize)),

            TaskType::TASK_CONSCRIPT => self.city_conscript(c),

            TaskType::TASK_CLEAR_ORDERS => self.city_clear_order_queue(c),

            TaskType::TASK_RALLY_PLOT => {
                let rally_plot = self.map.plot_xy(data1, data2);
                self.city_set_rally_plot(c, rally_plot);
            }

            TaskType::TASK_CLEAR_RALLY_PLOT => self.city_set_rally_plot(c, None),

            _ => debug_assert!(false, "eTask failed to match a valid option"),
        }
    }

    /// C++ `CvCity::setGameTurnFounded` (CvCity.cpp:5444-5453): also refreshes the map's
    /// working-city assignment.
    pub fn city_set_game_turn_founded(&mut self, c: CityId, new_value: i32) {
        if self.cities[c].game_turn_founded != new_value {
            self.cities[c].game_turn_founded = new_value;
            debug_assert!(new_value >= 0);

            self.map_update_working_city();
        }
    }

    /// C++ `CvCity::isOccupation` (CvCity.cpp:7496-7499).
    pub fn city_is_occupation(&self, c: CityId) -> bool {
        self.cities[c].occupation_timer > 0
    }

    /// C++ `CvCity::setOccupationTimer` (CvCity.cpp:7502-7526).
    pub fn city_set_occupation_timer(&mut self, c: CityId, new_value: i32) {
        if self.cities[c].occupation_timer != new_value {
            let old_occupation = self.city_is_occupation(c);
            self.cities[c].occupation_timer = new_value;
            if old_occupation != self.city_is_occupation(c) {
                self.city_update_corporation(c);
                self.city_update_maintenance(c);
                self.city_update_trade_routes(c);
                self.city_update_culture_level(c, true);
                self.cities[c].ai_assign_work_dirty = true;
            }
        }
    }

    /// C++ `CvCity::changeOccupationTimer` (CvCity.cpp:7529-7532).
    pub fn city_change_occupation_timer(&mut self, c: CityId, change: i32) {
        let new_value = self.cities[c].occupation_timer + change;
        self.city_set_occupation_timer(c, new_value);
    }

    /// C++ `CvCity::setName` (CvCity.cpp:9418-9441): sets the name if the owner accepts it
    /// (`CvPlayer::isCityNameValid`); `found` also sends the founding message.
    pub fn city_set_name(&mut self, c: CityId, new_value: &str, found: bool) {
        // C++ first runs `gDLL->stripSpecialCharacters` (exe-side text sanitizing; not ported).
        if !new_value.is_empty() {
            if self.player_is_city_name_valid(self.cities[c].owner, new_value, false) {
                self.cities[c].name = new_value.to_string();
                // Interface dirty bits.
            }
            if found {
                self.city_do_found_message(c);
            }
        }
    }

    /// C++ `CvCity::doFoundMessage` (CvCity.cpp:9444-9453): founding message + replay
    /// message (game log in this port).
    pub fn city_do_found_message(&mut self, c: CityId) {
        // TXT_KEY_MISC_CITY_HAS_BEEN_FOUNDED to the owner, TXT_KEY_MISC_CITY_IS_FOUNDED replay
        // message.
        let d = self.d;
        let turn = self.game_turn();
        let owner = self.cities[c].owner;
        self.log.msg(turn, Some(owner), || format!("{} has been founded", d.text.name(&self.cities[c].name)));
    }

    /// C++ `CvCity::clearOrderQueue` (CvCity.cpp:11126-11137).
    pub fn city_clear_order_queue(&mut self, c: CityId) {
        while !self.cities[c].order_queue.is_empty() {
            self.city_pop_order(c, 0, false, false);
        }
    }

    /// C++ `CvCity::pushOrder` (CvCity.cpp:11140-11255): queues an order (`data1` = unit /
    /// building / project / process type, `data2` = UnitAI for `ORDER_TRAIN`, -1 = the unit's
    /// default). Python events `cityBuildingUnit` / `cityBuildingBuilding`.
    #[allow(clippy::too_many_arguments)]
    pub fn city_push_order(&mut self, c: CityId, order: OrderType, data1: i32, data2: i32, save: bool, pop: bool, append: bool, force: bool) {
        let d = self.d;
        let mut data2 = data2;

        if pop {
            self.city_pop_order(c, 0, false, false);
        }

        let mut valid = false;

        match order {
            OrderType::ORDER_TRAIN => {
                let unit = UnitType::from_usize(data1 as usize);
                if self.city_can_train(c, unit, false, false, false, false) || force {
                    if data2 == -1 {
                        data2 = opt_to_i32(d.units[unit].default_unit_ai_type);
                    }

                    let owner = self.cities[c].owner;
                    self.players[owner].unit_class_making[unit_class_of(d, unit)] += 1;

                    change_num_train_ai_units(self, c, data2, 1);

                    valid = true;
                    self.ev_city_building_unit(c, unit);
                }
            }

            OrderType::ORDER_CONSTRUCT => {
                let building = BuildingType::from_usize(data1 as usize);
                if self.city_can_construct(c, building, false, false, false) || force {
                    let owner = self.cities[c].owner;
                    self.players[owner].building_class_making[building_class_of(d, building)] += 1;

                    valid = true;
                    self.ev_city_building_building(c, building);
                }
            }

            OrderType::ORDER_CREATE => {
                let project = ProjectType::from_usize(data1 as usize);
                if self.city_can_create(c, project, false, false) || force {
                    let team = self.city_team(c);
                    self.teams[team].project_making[project] += 1;

                    valid = true;
                }
            }

            OrderType::ORDER_MAINTAIN => {
                if self.city_can_maintain(c, ProcessType::from_usize(data1 as usize), false) || force {
                    valid = true;
                }
            }

            _ => debug_assert!(false, "iOrder did not match a valid option"),
        }

        if !valid {
            return;
        }

        let order_data = OrderData { order_type: order, data1, data2, save };

        if append {
            self.cities[c].order_queue.push(order_data);
        } else {
            self.city_stop_head_order(c);
            self.cities[c].order_queue.insert(0, order_data);
        }

        if !append || self.cities[c].order_queue.len() == 1 {
            self.city_start_head_order(c);
        }

        // Interface dirty bits.
    }

    /// C++ `CvCity::popOrder` (CvCity.cpp:11258-11639): removes queue entry `num` (-1 = the
    /// last), completing it if `finish` (creates the unit / building / project). With `choose`
    /// and an empty queue the owner picks new production (`AI_chooseProduction` /
    /// `chooseProduction` -> `Controller::choose_production`). Python events `unitBuilt`,
    /// `buildingBuilt`, `projectBuilt`.
    pub fn city_pop_order(&mut self, c: CityId, num: i32, finish: bool, choose: bool) {
        let d = self.d;
        let defines = &d.defines;

        let was_food_production = self.city_is_food_production(c);

        let len = self.cities[c].order_queue.len() as i32;
        let num = if num == -1 { len - 1 } else { num };

        // C++ walks the queue to node `iNum` and returns when there is none.
        if num < 0 || num >= len {
            return;
        }
        let index = num as usize;
        let order = self.cities[c].order_queue[index].clone();

        if finish && order.save {
            self.city_push_order(c, order.order_type, order.data1, order.data2, true, false, true, false);
        }

        let mut train_unit = None;
        let mut construct_building = None;
        let mut create_project = None;

        match order.order_type {
            OrderType::ORDER_TRAIN => {
                let unit: UnitType = order_type_data(&order);
                let train_unit_ai = order.data2;
                train_unit = Some(unit);

                let owner = self.cities[c].owner;
                self.players[owner].unit_class_making[unit_class_of(d, unit)] -= 1;

                change_num_train_ai_units(self, c, train_unit_ai, -1);

                if finish {
                    let production_needed = self.city_production_needed_unit(c, unit);

                    // max overflow is the value of the item produced (to eliminate prebuild exploits)
                    let mut overflow = self.cities[c].unit_production[unit] - production_needed;
                    let max_overflow = production_needed.max(self.city_current_production_difference(c, false, false));
                    let max_overflow_for_gold = production_needed.max(self.city_production_difference(c, self.city_production_needed(c), self.city_production(c), 0, self.city_is_food_production(c), false));
                    overflow = max_overflow.min(overflow);
                    if overflow > 0 {
                        let production_modifier = self.city_production_modifier_unit(c, unit);
                        self.city_change_overflow_production(c, overflow, production_modifier);
                    }
                    self.cities[c].unit_production[unit] = 0;

                    let production_gold = (overflow - max_overflow_for_gold).max(0) * defines.maxed_unit_gold_percent / 100;
                    if production_gold > 0 {
                        self.players[owner].gold += production_gold;
                    }

                    let (x, y) = (self.cities[c].x, self.cities[c].y);
                    let new_unit = self.player_init_unit(owner, unit, x, y, opt_from_i32(train_unit_ai), None);

                    // FfH: Modified by Kael 07/05/2008 (finishMoves, production experience, the
                    // rally plot mission and automation moved into applyBuildEffects)
                    self.city_apply_build_effects(c, new_unit);

                    self.ev_unit_built(c, new_unit);

                    if d.units[unit].domain_type == Some(DomainType::DOMAIN_AIR) && self.units.contains(new_unit) {
                        let team = self.city_team(c);
                        if self.plot_count_num_air_units(self.city_plot(c), team) > self.city_air_unit_capacity(c, team) {
                            self.unit_jump_to_nearest_valid_plot(new_unit); // can destroy unit
                        }
                    }
                }
            }

            OrderType::ORDER_CONSTRUCT => {
                let building: BuildingType = order_type_data(&order);
                construct_building = Some(building);
                let building_class = building_class_of(d, building);

                let owner = self.cities[c].owner;
                self.players[owner].building_class_making[building_class] -= 1;

                if finish {
                    // UNOFFICIAL_PATCH 10/08/09 davidlallen & jdog5000: the class's extra player
                    // instances (BTS passed 1).
                    if self.player_is_building_class_maxed_out(owner, building_class, d.building_classes[building_class].extra_player_instances) {
                        self.player_remove_building_class(owner, building_class);
                    }

                    let num_real_building = self.cities[c].num_real_building[building];
                    self.city_set_num_real_building(c, building, num_real_building + 1);

                    let production_needed = self.city_production_needed_building(c, building);
                    // max overflow is the value of the item produced (to eliminate prebuild exploits)
                    let mut overflow = self.cities[c].building_production[building] - production_needed;
                    let max_overflow = production_needed.max(self.city_current_production_difference(c, false, false));
                    let max_overflow_for_gold = production_needed.max(self.city_production_difference(c, self.city_production_needed(c), self.city_production(c), 0, self.city_is_food_production(c), false));
                    overflow = max_overflow.min(overflow);
                    if overflow > 0 {
                        let production_modifier = self.city_production_modifier_building(c, building);
                        self.city_change_overflow_production(c, overflow, production_modifier);
                    }
                    self.cities[c].building_production[building] = 0;

                    let production_gold = (overflow - max_overflow_for_gold).max(0) * defines.maxed_building_gold_percent / 100;
                    if production_gold > 0 {
                        self.players[owner].gold += production_gold;
                    }

                    self.ev_building_built(c, building);
                }
            }

            OrderType::ORDER_CREATE => {
                let project: ProjectType = order_type_data(&order);
                create_project = Some(project);

                let team = self.city_team(c);
                self.teams[team].project_making[project] -= 1;

                if finish {
                    // Event reported to Python before the project is built, so that we can show
                    // the movie before awarding free techs, for example
                    self.ev_project_built(c, project);

                    // FfH: Added by Kael 10/03/2008
                    self.change_global_counter(d.projects[project].modify_global_counter);
                    // FfH: End Add

                    self.team_change_project_count(team, project, 1);

                    if d.projects[project].spaceship {
                        // C++: when the team can launch the project's victory, the launch popup
                        // for a human owner, `AI_launch(eVictory)` for an AI owner (AI: launch
                        // decision, not ported; no FFH2 project is a spaceship); else the
                        // spaceship screen for humans. Project art types are graphics.
                    }

                    let production_needed = self.city_production_needed_project(c, project);
                    // max overflow is the value of the item produced (to eliminate prebuild exploits)
                    let mut overflow = self.cities[c].project_production[project] - production_needed;
                    let max_overflow = production_needed.max(self.city_current_production_difference(c, false, false));
                    let max_overflow_for_gold = production_needed.max(self.city_production_difference(c, self.city_production_needed(c), self.city_production(c), 0, self.city_is_food_production(c), false));
                    overflow = max_overflow.min(overflow);
                    if overflow > 0 {
                        let production_modifier = self.city_production_modifier_project(c, project);
                        self.city_change_overflow_production(c, overflow, production_modifier);
                    }
                    self.cities[c].project_production[project] = 0;

                    let production_gold = (overflow - max_overflow_for_gold).max(0) * defines.maxed_project_gold_percent / 100;
                    if production_gold > 0 {
                        let owner = self.cities[c].owner;
                        self.players[owner].gold += production_gold;
                    }
                }
            }

            OrderType::ORDER_MAINTAIN => {}

            _ => debug_assert!(false, "pOrderNode->m_data.eOrderType is not a valid option"),
        }

        // The Python events above must not delete the city (C++ would use a dangling `this`).
        if !self.cities.contains(c) {
            return;
        }

        // C++ deletes the node it popped; `pushOrder` above only appended, so it is still at
        // `index`.
        debug_assert_eq!(self.cities[c].order_queue.get(index), Some(&order));
        let start = index == 0;
        if start {
            self.city_stop_head_order(c);
        }

        self.cities[c].order_queue.remove(index);

        if start {
            self.city_start_head_order(c);
        }

        // Interface dirty bits.

        let mut message = false;

        if choose && self.cities[c].order_queue.is_empty() {
            if !self.city_is_human(c) || self.cities[c].production_automated {
                ai_choose_production(self, c);
            } else {
                if was_food_production {
                    self.city_assign_working_plots(c);
                }

                self.city_choose_production(c, train_unit, construct_building, create_project, finish, false);

                message = true;
            }
        }

        if finish && !message {
            // TXT_KEY_MISC_TRAINED_UNIT_IN / TXT_KEY_MISC_CONSTRUCTED_BUILD_IN /
            // TXT_KEY_MISC_CREATED_PROJECT_IN (+ TXT_KEY_MISC_WORK_HAS_BEGUN) to the owner.
            let item = match (train_unit, construct_building, create_project) {
                (Some(u), _, _) => Some(&d.units[u].base.description),
                (None, Some(b), _) => Some(&d.buildings[b].base.description),
                (None, None, Some(p)) => Some(&d.projects[p].base.description),
                _ => None,
            };
            if let Some(item) = item {
                let turn = self.game_turn();
                let owner = self.cities[c].owner;
                self.log.msg(turn, Some(owner), || format!("{} completed in {}", d.text.name(item), d.text.name(&self.cities[c].name)));
            }
        }

        // Interface dirty bits.
    }

    /// C++ `CvCity::startHeadOrder` (CvCity.cpp:11642-11655).
    pub fn city_start_head_order(&mut self, c: CityId) {
        let Some((order_type, data1)) = self.cities[c].head_order().map(|o| (o.order_type, o.data1)) else { return };
        if order_type == OrderType::ORDER_MAINTAIN {
            self.city_process_process(c, ProcessType::from_usize(data1 as usize), 1);
        }
        self.cities[c].ai_assign_work_dirty = true;
    }

    /// C++ `CvCity::stopHeadOrder` (CvCity.cpp:11658-11669).
    pub fn city_stop_head_order(&mut self, c: CityId) {
        let Some((order_type, data1)) = self.cities[c].head_order().map(|o| (o.order_type, o.data1)) else { return };
        if order_type == OrderType::ORDER_MAINTAIN {
            self.city_process_process(c, ProcessType::from_usize(data1 as usize), -1);
        }
    }

    /// C++ `CvCity::doCheckProduction` (CvCity.cpp:11888-12030): converts production on
    /// maxed classes to gold, upgrades queued units, pops orders that can no longer be built.
    /// Returns false if an order was dropped.
    pub fn city_do_check_production(&mut self, c: CityId) -> bool {
        let d = self.d;
        let defines = &d.defines;
        let mut ok = true;
        let owner = self.cities[c].owner;

        for unit in d.units.keys() {
            let production = self.cities[c].unit_production[unit];
            if production > 0 && self.player_is_production_maxed_unit_class(owner, unit_class_of(d, unit)) {
                let production_gold = (production * defines.maxed_unit_gold_percent) / 100;

                if production_gold > 0 {
                    self.players[owner].gold += production_gold;
                    log_lost_wonder_production(self, c, &d.units[unit].base.description, production_gold);
                }

                self.cities[c].unit_production[unit] = 0;
            }
        }

        for building in d.buildings.keys() {
            let production = self.cities[c].building_production[building];
            if production > 0 && self.player_is_production_maxed_building_class(owner, building_class_of(d, building), false) {
                let production_gold = (production * defines.maxed_building_gold_percent) / 100;

                if production_gold > 0 {
                    self.players[owner].gold += production_gold;
                    log_lost_wonder_production(self, c, &d.buildings[building].base.description, production_gold);
                }

                self.cities[c].building_production[building] = 0;
            }
        }

        for project in d.projects.keys() {
            let production = self.cities[c].project_production[project];
            if production > 0 && self.player_is_production_maxed_project(owner, project) {
                let production_gold = (production * defines.maxed_project_gold_percent) / 100;

                if production_gold > 0 {
                    self.players[owner].gold += production_gold;
                    log_lost_wonder_production(self, c, &d.projects[project].base.description, production_gold);
                }

                self.cities[c].project_production[project] = 0;
            }
        }

        if !self.city_is_production(c) && !self.city_is_disorder(c) && self.city_is_human(c) && !self.cities[c].production_automated {
            self.city_choose_production(c, None, None, None, false, false);
            return ok;
        }

        for unit in d.units.keys() {
            if self.city_first_unit_order(c, unit).is_some() {
                let upgrade_unit = self.city_all_upgrades_available(c, unit, 0);

                if let Some(upgrade_unit) = upgrade_unit {
                    // FfH: Added by Kael 10/11/2007
                    if d.units[unit].never_obsolete {
                        continue;
                    }
                    // FfH: End Add

                    debug_assert!(upgrade_unit != unit, "eUpgradeUnit is expected to be different from iI");
                    let upgrade_production = self.cities[c].unit_production[unit];
                    self.cities[c].unit_production[unit] = 0;
                    self.cities[c].unit_production[upgrade_unit] = upgrade_production;

                    for i in 0..self.cities[c].order_queue.len() {
                        let o = &self.cities[c].order_queue[i];
                        if o.order_type == OrderType::ORDER_TRAIN && o.data1 == unit.i() {
                            self.players[owner].unit_class_making[unit_class_of(d, unit)] -= 1;
                            self.cities[c].order_queue[i].data1 = upgrade_unit.i();
                            // AI: C++ resets the order's UnitAI to the upgrade's default UnitAI
                            // (moving the area/player train counts with it) when
                            // `AI_unitValue(eUpgradeUnit, eUnitAI, area()) == 0`. AI valuations
                            // are not ported: the order keeps its UnitAI.
                            self.players[owner].unit_class_making[unit_class_of(d, upgrade_unit)] += 1;
                        }
                    }
                }
            }
        }

        let len = self.cities[c].order_queue.len();
        for i in (0..len).rev() {
            // C++ re-reads the queue each time: popping (and the AI choosing anew) changes it.
            if let Some(order) = self.cities[c].order_queue.get(i).cloned() {
                if !self.city_can_continue_production(c, &order) {
                    self.city_pop_order(c, i as i32, false, true);
                    ok = false;
                }
            }
        }

        ok
    }

    /// C++ `CvCity::doProduction` (CvCity.cpp:12033-12084). Python `doProduction` callback.
    pub fn city_do_production(&mut self, c: CityId, allow_no_production: bool) {
        if py_do_production(self, c) {
            return;
        }

        if (!self.city_is_human(c) || self.cities[c].production_automated)
            && (!self.city_is_production(c) || self.city_is_production_process(c) || self.cities[c].ai_choose_production_dirty)
        {
            ai_choose_production(self, c);
        }

        if !allow_no_production && !self.city_is_production(c) {
            return;
        }

        if self.city_is_production_process(c) {
            return;
        }

        if self.city_is_disorder(c) {
            return;
        }

        if self.city_is_production(c) {
            let difference = self.city_current_production_difference(c, false, true);
            self.city_change_production(c, difference);
            self.cities[c].overflow_production = 0;
            self.cities[c].feature_production = 0;

            if self.city_production(c) >= self.city_production_needed(c) {
                self.city_pop_order(c, 0, true, true);
            }
        } else {
            let difference = self.city_current_production_difference(c, false, false);
            let production_modifier = self.city_production_modifier(c);
            self.city_change_overflow_production(c, difference, production_modifier);
        }
    }

    /// C++ `CvCity::doDecay` (CvCity.cpp:12087-12142): decay of stale building / unit
    /// production (human owners only).
    pub fn city_do_decay(&mut self, c: CityId) {
        let d = self.d;
        let defines = &d.defines;

        for building in d.buildings.keys() {
            if self.city_production_building(c) != Some(building) {
                if self.cities[c].building_production[building] > 0 {
                    self.cities[c].building_production_time[building] += 1;

                    if self.city_is_human(c) {
                        let game_speed_percent = d.game_speeds[self.game_speed_type()].construct_percent;
                        if 100 * self.cities[c].building_production_time[building] > defines.building_production_decay_time * game_speed_percent {
                            let production = self.cities[c].building_production[building];
                            self.cities[c].building_production[building] =
                                production - (production * (100 - defines.building_production_decay_percent) + game_speed_percent - 1) / game_speed_percent;
                        }
                    }
                } else {
                    self.cities[c].building_production_time[building] = 0;
                }
            }
        }

        for unit in d.units.keys() {
            if self.city_production_unit(c) != Some(unit) {
                if self.cities[c].unit_production[unit] > 0 {
                    self.cities[c].unit_production_time[unit] += 1;

                    if self.city_is_human(c) {
                        let game_speed_percent = d.game_speeds[self.game_speed_type()].train_percent;
                        if 100 * self.cities[c].unit_production_time[unit] > defines.unit_production_decay_time * game_speed_percent {
                            let production = self.cities[c].unit_production[unit];
                            self.cities[c].unit_production[unit] = production - (production * (100 - defines.unit_production_decay_percent) + game_speed_percent - 1) / game_speed_percent;
                        }
                    }
                } else {
                    self.cities[c].unit_production_time[unit] = 0;
                }
            }
        }
    }

    /// C++ `CvCity::doMeltdown` (CvCity.cpp:12282-12321). Python `doMeltdown` callback.
    pub fn city_do_meltdown(&mut self, c: CityId) {
        let d = self.d;

        if py_do_meltdown(self, c) {
            return;
        }

        for building in d.buildings.keys() {
            let nuke_explosion_rand = d.buildings[building].nuke_explosion_rand;
            if self.city_num_building(c, building) > 0 && nuke_explosion_rand != 0 && self.soren_rand_num(nuke_explosion_rand, "Meltdown!!!") == 0 {
                if self.cities[c].num_real_building[building] > 0 {
                    self.city_set_num_real_building(c, building, 0);
                }

                let plot = self.city_plot(c);
                self.plot_nuke_explosion(plot, 1, None);

                // TXT_KEY_MISC_MELTDOWN_CITY to the owner.
                let turn = self.game_turn();
                let owner = self.cities[c].owner;
                self.log.msg(turn, Some(owner), || format!("Meltdown in {}", d.text.name(&self.cities[c].name)));

                break;
            }
        }
    }

    /// C++ `CvCity::getTriggerValue` (CvCity.cpp:13144-13356): weight of an event trigger in
    /// this city, `MIN_INT` = not possible. Python `PythonCanDoCity` callback.
    pub fn city_trigger_value(&self, c: CityId, trigger: EventTriggerType) -> i32 {
        let d = self.d;
        let k_trigger = &d.event_triggers[trigger];
        let city = &self.cities[c];
        let owner = city.owner;

        // C++ calls the callback only when the trigger names one; the hook answers true
        // otherwise.
        if !self.py_trigger_can_do_city(trigger, owner, c) {
            return MIN_INT;
        }

        if k_trigger.num_buildings > 0 && !k_trigger.buildings_required.is_empty() {
            let mut found_valid = false;

            for &building_class in &k_trigger.buildings_required {
                if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                    if city.num_real_building[building] > 0 {
                        found_valid = true;
                    }
                }
            }

            if !found_valid {
                return MIN_INT;
            }
        }

        if self.city_religion_count(c) < k_trigger.num_religions {
            return MIN_INT;
        }

        if k_trigger.num_religions > 0 && !k_trigger.religions_required.is_empty() {
            // FfH: Modified by Kael 05/04/2008 (every required religion must match, not any)
            for &religion in &k_trigger.religions_required {
                if !city.has_religion[religion] {
                    return MIN_INT;
                }
                if k_trigger.holy_city && !self.city_is_holy_city(c, religion) {
                    return MIN_INT;
                }
                if k_trigger.state_religion && Some(religion) != self.player_get_state_religion(owner) {
                    return MIN_INT;
                }
            }
            // FfH: End Modify
        }

        if self.city_corporation_count(c) < k_trigger.num_corporations {
            return MIN_INT;
        }

        if k_trigger.num_corporations > 0 && !k_trigger.corporations_required.is_empty() {
            let mut found_valid = false;

            for &corporation in &k_trigger.corporations_required {
                if city.has_corporation[corporation] && (!k_trigger.headquarters || self.city_is_headquarters(c, corporation)) {
                    found_valid = true;
                }
            }

            if !found_valid {
                return MIN_INT;
            }
        }

        if k_trigger.min_population > 0 && city.population < k_trigger.min_population {
            return MIN_INT;
        }

        if k_trigger.max_population > 0 && city.population > k_trigger.max_population {
            return MIN_INT;
        }

        if k_trigger.angry > 0 {
            if self.city_unhappy_level(c, 0) - self.city_happy_level(c) < k_trigger.angry {
                return MIN_INT;
            }
        } else if k_trigger.angry < 0 && self.city_happy_level(c) - self.city_unhappy_level(c, 0) < -k_trigger.angry {
            return MIN_INT;
        }

        if k_trigger.unhealthy > 0 {
            if self.city_bad_health(c, false, 0) - self.city_good_health(c) < k_trigger.unhealthy {
                return MIN_INT;
            }
        } else if k_trigger.unhealthy < 0 && self.city_good_health(c) - self.city_bad_health(c, false, 0) < -k_trigger.unhealthy {
            return MIN_INT;
        }

        if k_trigger.prereq_event_city && !k_trigger.prereq_events.is_empty() {
            let found_valid = k_trigger.prereq_events.iter().all(|&event| self.city_is_event_occured(c, event));

            if !found_valid {
                return MIN_INT;
            }
        }

        let mut value = 0;

        if city.food == 0 && k_trigger.city_food_weight > 0 {
            return MIN_INT;
        }

        // FfH: Added by Kael 01/21/2008
        if k_trigger.prereq_crime != 0 && self.city_crime(c) < k_trigger.prereq_crime {
            return MIN_INT;
        }
        // FfH: End Add

        value += city.food * k_trigger.city_food_weight;

        value
    }

    /// C++ `CvCity::canApplyEvent` (CvCity.cpp:13358-13456).
    pub fn city_can_apply_event(&self, c: CityId, event: EventType, triggered: &EventTriggeredData) -> bool {
        let d = self.d;
        let k_event = &d.events[event];

        if !k_event.city_effect && !k_event.other_player_city_effect {
            return true;
        }

        if triggered.city.is_none() && k_event.city_effect {
            return false;
        }

        if triggered.other_player_city.is_none() && k_event.other_player_city_effect {
            return false;
        }

        let city = &self.cities[c];

        if k_event.food + ((100 + k_event.food_percent) * city.food) / 100 < 0 {
            return false;
        }

        if k_event.population_change + city.population <= 0 {
            return false;
        }

        if 100 * k_event.culture + city.culture[city.owner] < 0 {
            return false;
        }

        if let Some(building_class) = k_event.building_class {
            let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) else { return false };

            // FfH: Added by Kael 09/21/2008 (a building class without a change number makes
            // the event require that building in the city)
            if k_event.building_change == 0 && city.num_real_building[building] == 0 {
                return false;
            }
            // FfH: End Add

            if k_event.building_change > 0 {
                if self.city_num_building(c, building) >= d.defines.city_max_num_buildings {
                    return false;
                }
            } else if k_event.building_change < 0 && city.num_real_building[building] + k_event.building_change < 0 {
                return false;
            }
        }

        if k_event.max_num_religions != -1 && self.city_religion_count(c) > k_event.max_num_religions {
            return false;
        }

        if k_event.min_pillage > 0 {
            let mut num_improvements = 0;
            for i in 0..NUM_CITY_PLOTS {
                if i != CITY_HOME_PLOT {
                    if let Some(plot) = self.city_index_plot(c, i) {
                        let p = &self.map.plots[plot];
                        if p.owner == Some(city.owner) {
                            if let Some(improvement) = p.improvement_type {
                                if !d.improvements[improvement].permanent {
                                    num_improvements += 1;
                                }
                            }
                        }
                    }
                }
            }

            if num_improvements < k_event.min_pillage {
                return false;
            }
        }

        true
    }

    /// C++ `CvCity::applyEvent` (CvCity.cpp:13458-13656).
    pub fn city_apply_event(&mut self, c: CityId, event: EventType, triggered: &EventTriggeredData, clear: bool) {
        let d = self.d;

        if !self.city_can_apply_event(c, event, triggered) {
            return;
        }

        self.city_set_event_occured(c, event, true);

        let k_event = &d.events[event];

        if k_event.city_effect || k_event.other_player_city_effect {
            if k_event.happy != 0 {
                self.city_change_extra_happiness(c, k_event.happy);
            }

            if k_event.health != 0 {
                self.city_change_extra_health(c, k_event.health);
            }

            if k_event.hurry_anger != 0 {
                let change = k_event.hurry_anger * self.city_flat_hurry_anger_length(c);
                self.city_change_hurry_anger_timer(c, change);
            }

            if k_event.happy_turns != 0 {
                self.city_change_happiness_timer(c, k_event.happy_turns);
            }

            if k_event.food != 0 || k_event.food_percent != 0 {
                let food = self.cities[c].food;
                self.cities[c].food += k_event.food + (k_event.food_percent * food) / 100;
            }

            if k_event.population_change != 0 {
                self.city_change_population(c, k_event.population_change);
            }

            if k_event.revolt_turns > 0 {
                self.cities[c].culture_update_timer += k_event.revolt_turns;
                self.city_change_occupation_timer(c, k_event.revolt_turns);
            }

            if k_event.space_production_modifier != 0 {
                self.cities[c].space_production_modifier += k_event.space_production_modifier;
            }

            if k_event.max_pillage > 0 {
                debug_assert!(k_event.max_pillage >= k_event.min_pillage);
                let num_pillage = k_event.min_pillage + self.soren_rand_num(k_event.max_pillage - k_event.min_pillage, "Pick number of event pillaged plots");

                let mut num_pillaged = 0;
                for _ in 0..num_pillage {
                    let rand_offset = self.soren_rand_num(NUM_CITY_PLOTS as i32, "Pick event pillage plot") as usize;
                    for j in 0..NUM_CITY_PLOTS {
                        let i_plot = (j + rand_offset) % NUM_CITY_PLOTS;
                        if i_plot != CITY_HOME_PLOT {
                            if let Some(plot) = self.city_index_plot(c, i_plot) {
                                if self.map.plots[plot].owner == Some(self.cities[c].owner) {
                                    if let Some(improvement) = self.map.plots[plot].improvement_type {
                                        if !d.improvements[improvement].permanent {
                                            // TXT_KEY_EVENT_CITY_IMPROVEMENT_DESTROYED to the owner.
                                            let turn = self.game_turn();
                                            let owner = self.cities[c].owner;
                                            self.log.msg(turn, Some(owner), || format!("{} destroyed", d.text.name(&d.improvements[improvement].base.description)));
                                            self.plot_set_improvement_type(plot, None);
                                            num_pillaged += 1;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let mut other_player = triggered.other_player;
                if !k_event.city_effect && k_event.other_player_city_effect {
                    other_player = Some(triggered.player);
                }

                if let Some(other_player) = other_player {
                    // TXT_KEY_EVENT_NUM_CITY_IMPROVEMENTS_DESTROYED to the other player.
                    let turn = self.game_turn();
                    self.log.msg(turn, Some(other_player), || format!("{num_pillaged} city improvements destroyed"));
                }
            }

            for specialist in d.specialists.keys() {
                if k_event.free_specialist_count[specialist] > 0 {
                    self.city_change_free_specialist_count(c, specialist, k_event.free_specialist_count[specialist]);
                }
            }

            if k_event.culture != 0 {
                let owner = self.cities[c].owner;
                self.city_change_culture(c, owner, k_event.culture, true, true);
            }

            // FfH: Added by Kael 01/21/2008
            if k_event.crime != 0 {
                self.cities[c].crime += k_event.crime;
            }
            // FfH: End Add
        }

        if let Some(unit_class) = k_event.unit_class {
            if let Some(unit) = d.civ_unit(self.city_civilization_type(c), unit_class) {
                for _ in 0..k_event.num_units {
                    // FfH: Modified by Kael 10/29/2007 (the event's unit promotion)
                    let (owner, x, y) = {
                        let city = &self.cities[c];
                        (city.owner, city.x, city.y)
                    };
                    let new_unit = self.player_init_unit(owner, unit, x, y, None, None);
                    if let Some(promotion) = k_event.unit_promotion {
                        self.unit_set_has_promotion(new_unit, promotion, true);
                    }
                    // FfH: End Modify
                }
            }
        }

        if let Some(building_class) = k_event.building_class {
            if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                if k_event.building_change != 0 {
                    let num_real_building = self.cities[c].num_real_building[building];
                    self.city_set_num_real_building(c, building, num_real_building + k_event.building_change);
                }
            }
        }

        if !k_event.building_yield_changes.is_empty() {
            for building_class in d.building_classes.keys() {
                for i in 0..YieldType::NUM {
                    let yield_type = YieldType::from_usize(i);
                    let change = self.city_building_yield_change(c, building_class, yield_type) + k_event.building_yield_change(building_class, yield_type);
                    self.city_set_building_yield_change(c, building_class, yield_type, change);
                }
            }
        }

        if !k_event.building_commerce_changes.is_empty() {
            for building_class in d.building_classes.keys() {
                for i in 0..CommerceType::NUM {
                    let commerce = CommerceType::from_usize(i);
                    let change = self.city_building_commerce_change(c, building_class, commerce) + k_event.building_commerce_change(building_class, commerce);
                    self.city_set_building_commerce_change(c, building_class, commerce, change);
                }
            }
        }

        if !k_event.building_happy_changes.is_empty() {
            for building_class in d.building_classes.keys() {
                self.city_set_building_happy_change(c, building_class, k_event.building_happy_change(building_class));
            }
        }

        if !k_event.building_health_changes.is_empty() {
            for building_class in d.building_classes.keys() {
                self.city_set_building_health_change(c, building_class, k_event.building_health_change(building_class));
            }
        }

        if clear {
            for event in d.events.keys() {
                self.city_set_event_occured(c, event, false);
            }
        }
    }

    /// C++ `CvCity::isEventOccured` (CvCity.cpp:13658-13669).
    pub fn city_is_event_occured(&self, c: CityId, event: EventType) -> bool {
        self.cities[c].events_occured.contains(&event)
    }

    /// C++ `CvCity::setEventOccured` (CvCity.cpp:13671-13689).
    pub fn city_set_event_occured(&mut self, c: CityId, event: EventType, occured: bool) {
        let events = &mut self.cities[c].events_occured;
        if let Some(i) = events.iter().position(|&e| e == event) {
            if !occured {
                events.remove(i);
            }
            return;
        }
        if occured {
            events.push(event);
        }
    }

    /// C++ `CvCity::liberate` (CvCity.cpp:14169-14228): gives the city to
    /// `city_liberation_player` (`CvPlayer::acquireCity`; `c` dangles afterwards).
    pub fn city_liberate(&mut self, c: CityId, conquest: bool) {
        let d = self.d;
        let plot = self.city_plot(c);
        let player = self.city_liberation_player(c, conquest);
        let owner = self.cities[c].owner;

        let Some(player) = player else { return };

        let old_owner_culture = self.cities[c].culture[owner];
        let player_team = self.players[player].team;
        let owner_team = self.players[owner].team;
        let mut old_master_land = 0;
        let mut old_vassal_land = 0;
        if self.teams[player_team].vassal[owner_team] {
            old_master_land = self.team_get_total_land(owner_team, true);
            old_vassal_land = self.team_get_total_land(player_team, false);
        }

        // TXT_KEY_MISC_CITY_LIBERATED to every alive player that has revealed the city, and a
        // replay message.
        let turn = self.game_turn();
        self.log.msg(turn, None, || format!("{} liberated: player {} -> player {}", d.text.name(&self.cities[c].name), owner, player));

        self.player_acquire_city(player, c, false, true, true);
        // `c` dangles from here on.
        self.players[player].ai_memory_count[owner][MemoryType::MEMORY_LIBERATED_CITIES] += 1;

        if self.teams[player_team].vassal[owner_team] {
            let new_master_land = self.team_get_total_land(owner_team, true);
            let new_vassal_land = self.team_get_total_land(player_team, false);

            self.teams[player_team].master_power += new_master_land - old_master_land;
            self.teams[player_team].vassal_power += new_vassal_land - old_vassal_land;
        }

        // C++ `if (NULL != pPlot)`: the city's own plot, always there.
        let new_city = self.map.plots[plot].plot_city.filter(|&city| self.cities.contains(city));
        if let Some(new_city) = new_city {
            let culture = self.cities[new_city].culture[player] + old_owner_culture / 2;
            self.city_set_culture_times100(new_city, player, culture, true, true);
        }

        if self.team_is_a_vassal(player_team) {
            for _ in 0..d.defines.colony_num_free_defenders {
                // C++ quirk: `pCity` is not checked for NULL here (it would crash); there is
                // always a city after acquireCity.
                if let Some(new_city) = new_city {
                    self.city_init_conscripted_unit(new_city);
                }
            }
        }
    }

    /// C++ `CvCity::getLiberationPlayer` (CvCity.cpp:14230-14342). C++ `NO_PLAYER` = `None`.
    pub fn city_liberation_player(&self, c: CityId, conquest: bool) -> Option<PlayerId> {
        if self.city_is_capital(c) {
            return None;
        }

        let city = &self.cities[c];
        let owner = city.owner;
        let area = self.city_area(c);

        for p in self.civ_player_ids() {
            let loop_player = &self.players[p];
            if loop_player.alive && loop_player.parent == Some(owner) {
                if let Some(loop_capital) = self.player_get_capital_city(p) {
                    if self.city_area(loop_capital) == area {
                        return Some(p);
                    }
                }
            }
        }

        if self.player_can_split_empire(owner) && self.player_can_split_area(owner, area) {
            if let Some(player) = self.player_get_split_empire_player(owner, area) {
                if self.players[player].alive {
                    return Some(player);
                }
            }
        }

        let mut best_player = None;
        let mut best_value = 0;

        let total_culture_times100 = self.city_count_total_culture_times100(c);
        let city_team = self.city_team(c);

        for p in self.civ_player_ids() {
            if self.players[p].alive && self.player_can_receive_trade_city(p) {
                if let Some(capital) = self.player_get_capital_city(p) {
                    let mut capital_distance = self.plot_distance(city.x, city.y, self.cities[capital].x, self.cities[capital].y);
                    if area != self.city_area(capital) {
                        capital_distance *= 2;
                    }

                    let mut culture_times100 = city.culture[p];

                    if conquest && p == city.original_owner {
                        culture_times100 *= 3;
                        culture_times100 /= 2;
                    }

                    let player_team = self.players[p].team;
                    if player_team == city_team || self.teams[player_team].vassal[city_team] || self.teams[city_team].vassal[player_team] {
                        culture_times100 *= 2;
                        culture_times100 = (culture_times100 + total_culture_times100) / 2;
                    }

                    let value = culture_times100.max(100) / capital_distance.max(1);

                    if value > best_value {
                        best_value = value;
                        best_player = Some(p);
                    }
                }
            }
        }

        if let Some(best) = best_player {
            if owner == best {
                return None;
            }

            for i in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(city.x, city.y, i) {
                    if self.plot_is_visible_enemy_unit(loop_plot, best) {
                        return None;
                    }
                }
            }
        }

        best_player
    }

    /// C++ `CvCity::isAutoRaze` (CvCity.cpp:14382-14410).
    pub fn city_is_auto_raze(&self, c: CityId) -> bool {
        if !self.is_option(GameOptionType::GAMEOPTION_NO_CITY_RAZING) {
            if self.cities[c].highest_population == 1 {
                return true;
            }

            if self.max_city_elimination() > 0 {
                return true;
            }
        }

        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.city_is_human(c) {
            return true;
        }

        // FfH: Added by Kael 11/03/2008
        if self.is_option(GameOptionType::GAMEOPTION_ALWAYS_RAZE) {
            return true;
        }
        // FfH: End Add

        false
    }

    /// C++ `CvCity::applyBuildEffects` (CvCity.cpp:14499-14591, FFH): effects on a unit just
    /// built here (experience, rally plot mission, automation, race/civ-trait promotions,
    /// religion adoption and mutation rolls, free promotion picks). The unit-art-style lines
    /// are graphics (omit).
    pub fn city_apply_build_effects(&mut self, c: CityId, unit: UnitId) {
        let d = self.d;

        self.unit_finish_moves(unit);
        self.city_add_production_experience(c, unit, false);
        if let Some(rally_plot) = self.city_rally_plot(c) {
            let (rally_x, rally_y) = self.map.xy(rally_plot);
            if let Some(group) = self.units[unit].group {
                self.group_push_mission(group, MissionType::MISSION_MOVE_TO, rally_x, rally_y, 0, false, false, None, None, None);
            }
        }
        let owner = self.cities[c].owner;
        if self.city_is_human(c) {
            if self.players[owner].options[PlayerOptionType::PLAYEROPTION_START_AUTOMATED] {
                self.unit_automate(unit, Some(AutomateType::AUTOMATE_BUILD));
            }
            if self.players[owner].options[PlayerOptionType::PLAYEROPTION_MISSIONARIES_AUTOMATED] {
                self.unit_automate(unit, Some(AutomateType::AUTOMATE_RELIGION));
            }
        } else {
            let ai_free_xp = d.handicaps[self.handicap].ai_free_xp;
            self.unit_change_experience(unit, ai_free_xp, -1, false, false, false);
        }
        let city_civ = self.city_civilization_type(c);
        if Some(city_civ) != self.player_get_civilization_type(owner) {
            // FFHBUG denev: setUnitArtStyleType / reloadEntity (graphics).
            if let Some(default_race) = d.civilizations[city_civ].default_race {
                if self.units[unit].race.is_none()
                    && !is_world_unit_class(d, self.unit_get_unit_class_type(unit))
                    && !self.unit_is_animal(unit)
                    && self.unit_is_alive(unit)
                    && self.unit_get_domain_type(unit) == DomainType::DOMAIN_LAND
                {
                    self.unit_set_has_promotion(unit, default_race, true);
                }
            }
            if let Some(civ_trait) = d.civilizations[city_civ].civ_trait {
                let trait_info = &d.traits[civ_trait];
                for promotion in d.promotions.keys() {
                    if trait_info.free_promotion[promotion] {
                        if let Some(unit_combat) = self.unit_get_unit_combat_type(unit) {
                            if trait_info.free_promotion_unit_combat[unit_combat] {
                                self.unit_set_has_promotion(unit, promotion, true);
                            }
                        }
                    }
                }
            }
        }
        if self.unit_is_alive(unit) {
            for religion in d.religions.keys() {
                if self.cities[c].has_religion[religion]
                    && self.units[unit].religion.is_none()
                    && self.soren_rand_num(100, "Religion Adoption") <= d.defines.religion_adoption_chance
                {
                    self.units[unit].religion = Some(religion);
                }
            }
            if self.soren_rand_num(100, "Mutate Chance") < self.cities[c].mutate_chance {
                // C++ would index -1 without a MUTATED_PROMOTION define.
                if let Some(mutated) = opt_from_i32(d.defines.mutated_promotion) {
                    self.unit_set_has_promotion(unit, mutated, true);
                }
            }
        }
        for building in d.buildings.keys() {
            let free_promotion_pick = d.buildings[building].free_promotion_pick;
            if self.city_num_building(c, building) > 0 && free_promotion_pick > 0 {
                self.unit_change_free_promotion_pick(unit, free_promotion_pick);
                self.unit_set_promotion_ready(unit, true);
            }
        }
        self.unit_set_weapons(unit);
    }

    /// C++ `CvCity::getCrime` (CvCity.cpp:14675-14683, FFH): the game's global crime plus
    /// the city's own (`crime` field), at least 0.
    pub fn city_crime(&self, c: CityId) -> i32 {
        let crime = self.crime + self.cities[c].crime;
        if crime < 0 {
            return 0;
        }
        crime
    }

}

/// The TXT_KEY_MISC_LOST_WONDER_PROD_CONVERTED message of `doCheckProduction` (game log).
fn log_lost_wonder_production(g: &mut Game, c: CityId, item: &str, gold: i32) {
    let d = g.d;
    let turn = g.game_turn();
    let owner = g.cities[c].owner;
    g.log.msg(turn, Some(owner), || format!("{}: production of {} converted to {} gold", d.text.name(&g.cities[c].name), d.text.name(item), gold));
}

/// C++ `area()->changeNumTrainAIUnits(owner, eUnitAI, iChange)` and
/// `GET_PLAYER(owner).AI_changeNumTrainAIUnits(eUnitAI, iChange)` (AI bookkeeping fields).
/// Every FFH2 unit has a default UnitAI, so train orders always carry one; C++ would index the
/// arrays with -1 otherwise, here nothing is counted.
fn change_num_train_ai_units(g: &mut Game, c: CityId, unit_ai: i32, change: i32) {
    let Some(unit_ai) = opt_from_i32::<UnitAIType>(unit_ai) else { return };
    let owner = g.cities[c].owner;
    let area = g.city_area(c);
    g.map.areas[area].num_train_ai_units[owner][unit_ai] += change;
    g.players[owner].ai_num_train_ai_units[unit_ai] += change;
}

/// C++ `CvGameAI::AI_makeAssignWorkDirty`: every alive player's cities reassign their citizens.
fn game_ai_make_assign_work_dirty(g: &mut Game) {
    for p in g.player_ids() {
        if g.players[p].alive {
            g.player_ai_make_assign_work_dirty(p);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// CvGameUtils.py callbacks that CvCity calls but whose FFH2 bodies are constant ("use the
// default" answers), so they have no hook in `crate::ffh`.
// ---------------------------------------------------------------------------------------------

/// Python `CvGameUtils.citiesDestroyFeatures(iX, iY)` (always called by `CvCity::init`):
/// FFH2 returns True.
fn py_cities_destroy_features(_g: &Game, _x: i32, _y: i32) -> bool {
    true
}

/// Python `CvGameUtils.doProduction(pCity)` (always called by `CvCity::doProduction`): FFH2
/// returns False (use the default production).
fn py_do_production(_g: &Game, _c: CityId) -> bool {
    false
}

/// Python `CvGameUtils.doMeltdown(pCity)` (always called by `CvCity::doMeltdown`): FFH2
/// returns False.
fn py_do_meltdown(_g: &Game, _c: CityId) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;

    const P0: PlayerId = PlayerId(0);

    /// A two-civ game (players `Player::new`ed, no map) with one city of player 0.
    fn game_with_city() -> (Game, CityId) {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 9));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        let id = g.cities.next_id();
        let c = g.player_add_city(P0, City::new(d, num_players, num_teams, id, P0, 2, 2, 0));
        (g, c)
    }

    /// Buildings whose orders `city_push_order` accepts with `force` in a settlement (where
    /// `canConstruct` answers false before asking the player).
    fn buildings(g: &Game, n: usize) -> Vec<BuildingType> {
        g.d.buildings.keys().filter(|&b| g.d.buildings[b].building_class_type.is_some()).take(n).collect()
    }

    #[test]
    fn crime_adds_the_game_crime_and_is_never_negative() {
        let (mut g, c) = game_with_city();
        g.crime = 10;
        g.cities[c].crime = 7;
        assert_eq!(g.city_crime(c), 17);
        g.cities[c].crime = -4;
        assert_eq!(g.city_crime(c), 6);
        g.cities[c].crime = -30;
        assert_eq!(g.city_crime(c), 0);
    }

    #[test]
    fn auto_raze_rules() {
        let (mut g, c) = game_with_city();
        g.cities[c].highest_population = 3;
        assert!(!g.city_is_auto_raze(c));
        g.cities[c].highest_population = 1;
        assert!(g.city_is_auto_raze(c));
        g.setup.options[GameOptionType::GAMEOPTION_NO_CITY_RAZING] = true;
        assert!(!g.city_is_auto_raze(c));
        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_RAZE] = true;
        assert!(g.city_is_auto_raze(c));
        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_RAZE] = false;
        g.setup.options[GameOptionType::GAMEOPTION_NO_CITY_RAZING] = false;
        g.cities[c].highest_population = 4;
        g.setup.max_city_elimination = 2;
        assert!(g.city_is_auto_raze(c));
    }

    #[test]
    fn order_queue_push_pop_and_clear() {
        let (mut g, c) = game_with_city();
        let d = g.d;
        g.cities[c].settlement = true;
        let b = buildings(&g, 3);
        let class = |b: BuildingType| d.buildings[b].building_class_type.unwrap();

        // Not buildable and not forced: rejected.
        g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b[0].i(), -1, false, false, true, false);
        assert!(g.cities[c].order_queue.is_empty());

        // Appended to an empty queue: started.
        g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b[0].i(), -1, false, false, true, true);
        assert!(g.cities[c].ai_assign_work_dirty);
        g.cities[c].ai_assign_work_dirty = false;
        // Appended behind the head order: not started.
        g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b[1].i(), -1, true, false, true, true);
        assert!(!g.cities[c].ai_assign_work_dirty);
        // Not appended: becomes the head order and is started.
        g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b[2].i(), -1, false, false, false, true);
        assert!(g.cities[c].ai_assign_work_dirty);
        let queued: Vec<i32> = g.cities[c].order_queue.iter().map(|o| o.data1).collect();
        assert_eq!(queued, vec![b[2].i(), b[0].i(), b[1].i()]);
        assert!(g.cities[c].order_queue[2].save);
        assert_eq!(g.city_production_building(c), Some(b[2]));
        assert_eq!(g.city_first_building_order(c, b[1]), Some(2));
        for &x in &b {
            assert_eq!(g.players[P0].building_class_making[class(x)], 1);
        }

        // Out of range: nothing happens.
        g.city_pop_order(c, 3, false, false);
        g.city_pop_order(c, -2, false, false);
        assert_eq!(g.cities[c].order_queue.len(), 3);

        // -1 = the last order.
        g.city_pop_order(c, -1, false, false);
        assert_eq!(g.players[P0].building_class_making[class(b[1])], 0);
        g.city_pop_order(c, 0, false, false);
        assert_eq!(g.city_production_building(c), Some(b[0]));
        assert_eq!(g.players[P0].building_class_making[class(b[2])], 0);

        // pop + push replaces the head order.
        g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b[1].i(), -1, false, true, false, true);
        assert_eq!(g.cities[c].order_queue.len(), 1);
        assert_eq!(g.city_production_building(c), Some(b[1]));
        assert_eq!(g.players[P0].building_class_making[class(b[0])], 0);

        g.city_push_order(c, OrderType::ORDER_CONSTRUCT, b[2].i(), -1, false, false, true, true);
        g.city_do_task(c, TaskType::TASK_CLEAR_ORDERS, -1, -1, false, false, false, false);
        assert!(!g.city_is_production(c));
        for &x in &b {
            assert_eq!(g.players[P0].building_class_making[class(x)], 0);
        }
    }

    #[test]
    fn events_without_city_effects_always_apply() {
        let (g, c) = game_with_city();
        let d = g.d;
        let triggered = EventTriggeredData {
            id: TriggeredId(0),
            trigger: EventTriggerType(0),
            turn: 0,
            player: P0,
            city: None,
            plot_x: -1,
            plot_y: -1,
            unit: None,
            other_player: None,
            other_player_city: None,
            religion: None,
            corporation: None,
            building: None,
            text: String::new(),
            global_text: String::new(),
        };
        let e = d.events.keys().find(|&e| !d.events[e].city_effect && !d.events[e].other_player_city_effect).expect("an event without city effects");
        assert!(g.city_can_apply_event(c, e, &triggered));
        // A city effect needs the trigger's city.
        let e = d.events.keys().find(|&e| d.events[e].city_effect).expect("an event with a city effect");
        assert!(!g.city_can_apply_event(c, e, &triggered));
    }

    #[test]
    fn trigger_value_weighs_city_food() {
        let (mut g, c) = game_with_city();
        let d = g.d;
        let simple = |t: &EventTriggerInfo| {
            t.python_can_do_city.is_empty()
                && t.num_buildings == 0
                && t.num_religions == 0
                && t.num_corporations == 0
                && t.min_population == 0
                && t.max_population == 0
                && t.angry == 0
                && t.unhealthy == 0
                && !t.prereq_event_city
                && t.prereq_crime == 0
        };
        let trigger = d.event_triggers.keys().find(|&t| simple(&d.event_triggers[t]) && d.event_triggers[t].city_food_weight > 0).expect("a trigger weighted by city food");
        let weight = d.event_triggers[trigger].city_food_weight;
        g.cities[c].food = 0;
        assert_eq!(g.city_trigger_value(c, trigger), MIN_INT);
        g.cities[c].food = 7;
        assert_eq!(g.city_trigger_value(c, trigger), 7 * weight);
    }

    #[test]
    fn ai_reset_matches_new_city_defaults() {
        let (mut g, c) = game_with_city();
        let fresh = g.cities[c].clone();
        let city = &mut g.cities[c];
        city.ai_assign_work_dirty = true;
        city.ai_choose_production_dirty = true;
        city.ai_emphasize_avoid_growth_count = 2;
        city.ai_emphasize_great_people_count = 1;
        city.ai_emphasize_yield_count[1] = 3;
        city.ai_emphasize_commerce_count[0] = 1;
        city.ai_emphasize_avoid_angry_citizens_count = 1;
        city.ai_emphasize_avoid_unhealthy_citizens_count = 1;
        city.ai_emphasize.fill(true);
        ai_reset(city);
        assert_eq!(city.ai_assign_work_dirty, fresh.ai_assign_work_dirty);
        assert_eq!(city.ai_choose_production_dirty, fresh.ai_choose_production_dirty);
        assert_eq!(city.ai_emphasize_avoid_growth_count, fresh.ai_emphasize_avoid_growth_count);
        assert_eq!(city.ai_emphasize_great_people_count, fresh.ai_emphasize_great_people_count);
        assert_eq!(city.ai_emphasize_yield_count, fresh.ai_emphasize_yield_count);
        assert_eq!(city.ai_emphasize_commerce_count, fresh.ai_emphasize_commerce_count);
        assert_eq!(city.ai_emphasize_avoid_angry_citizens_count, 0);
        assert_eq!(city.ai_emphasize_avoid_unhealthy_citizens_count, 0);
        assert_eq!(city.ai_emphasize, fresh.ai_emphasize);
    }
}
