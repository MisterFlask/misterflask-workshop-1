//! CvCity: lifecycle (`init`, `kill`), turn processing (`doTurn`, `doTask`), the build queue
//! (`pushOrder`/`popOrder`, `doCheckProduction`, `doProduction`, `doDecay`), events,
//! liberation and FFH2 misc (meltdown, crime).

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{order_type_data, range, City};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
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
        todo!("CvCity::init")
    }

    /// C++ `CvCity::kill` (CvCity.cpp:732-912): destroys the city (the owner's
    /// `deleteCity` removes it from `self.cities`; `c` dangles afterwards). Python event
    /// `cityLost`.
    pub fn city_kill(&mut self, c: CityId, update_plot_groups: bool) {
        todo!("CvCity::kill")
    }

    /// C++ `CvCity::doTurn` (CvCity.cpp:915-1150). `AI_doTurn` is AI (omit); FFH: caster
    /// buildings, remove/free promotions on units in the city. Python event `cityDoTurn`.
    pub fn city_do_turn(&mut self, c: CityId) {
        todo!("CvCity::doTurn")
    }

    /// C++ `CvCity::doTask` (CvCity.cpp:1252-1327): applies a player command
    /// (`TaskType::TASK_*`). `TASK_SET_EMPHASIZE` -> `city_ai_set_emphasize`.
    #[allow(clippy::too_many_arguments)]
    pub fn city_do_task(&mut self, c: CityId, task: TaskType, data1: i32, data2: i32, option: bool, alt: bool, shift: bool, ctrl: bool) {
        todo!("CvCity::doTask")
    }

    /// C++ `CvCity::setGameTurnFounded` (CvCity.cpp:5444-5453): also refreshes the map's
    /// working-city assignment.
    pub fn city_set_game_turn_founded(&mut self, c: CityId, new_value: i32) {
        todo!("CvCity::setGameTurnFounded")
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
        todo!("CvCity::setName")
    }

    /// C++ `CvCity::doFoundMessage` (CvCity.cpp:9444-9453): founding message + replay
    /// message (game log in this port).
    pub fn city_do_found_message(&mut self, c: CityId) {
        todo!("CvCity::doFoundMessage")
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
        todo!("CvCity::pushOrder")
    }

    /// C++ `CvCity::popOrder` (CvCity.cpp:11258-11639): removes queue entry `num` (-1 = the
    /// last), completing it if `finish` (creates the unit / building / project). With `choose`
    /// and an empty queue the owner picks new production (`AI_chooseProduction` /
    /// `chooseProduction` -> `Controller::choose_production`). Python events `unitBuilt`,
    /// `buildingBuilt`, `projectBuilt`.
    pub fn city_pop_order(&mut self, c: CityId, num: i32, finish: bool, choose: bool) {
        todo!("CvCity::popOrder")
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
        todo!("CvCity::doCheckProduction")
    }

    /// C++ `CvCity::doProduction` (CvCity.cpp:12033-12084). Python `doProduction` callback.
    pub fn city_do_production(&mut self, c: CityId, allow_no_production: bool) {
        todo!("CvCity::doProduction")
    }

    /// C++ `CvCity::doDecay` (CvCity.cpp:12087-12142): decay of stale building / unit
    /// production (human owners only).
    pub fn city_do_decay(&mut self, c: CityId) {
        todo!("CvCity::doDecay")
    }

    /// C++ `CvCity::doMeltdown` (CvCity.cpp:12282-12321). Python `doMeltdown` callback.
    pub fn city_do_meltdown(&mut self, c: CityId) {
        todo!("CvCity::doMeltdown")
    }

    /// C++ `CvCity::getTriggerValue` (CvCity.cpp:13144-13356): weight of an event trigger in
    /// this city, `MIN_INT` = not possible. Python `PythonCanDoCity` callback.
    pub fn city_trigger_value(&self, c: CityId, trigger: EventTriggerType) -> i32 {
        todo!("CvCity::getTriggerValue")
    }

    /// C++ `CvCity::canApplyEvent` (CvCity.cpp:13358-13456).
    pub fn city_can_apply_event(&self, c: CityId, event: EventType, triggered: &EventTriggeredData) -> bool {
        todo!("CvCity::canApplyEvent")
    }

    /// C++ `CvCity::applyEvent` (CvCity.cpp:13458-13656).
    pub fn city_apply_event(&mut self, c: CityId, event: EventType, triggered: &EventTriggeredData, clear: bool) {
        todo!("CvCity::applyEvent")
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
        todo!("CvCity::liberate")
    }

    /// C++ `CvCity::getLiberationPlayer` (CvCity.cpp:14230-14342). C++ `NO_PLAYER` = `None`.
    pub fn city_liberation_player(&self, c: CityId, conquest: bool) -> Option<PlayerId> {
        todo!("CvCity::getLiberationPlayer")
    }

    /// C++ `CvCity::isAutoRaze` (CvCity.cpp:14382-14410).
    pub fn city_is_auto_raze(&self, c: CityId) -> bool {
        todo!("CvCity::isAutoRaze")
    }

    /// C++ `CvCity::applyBuildEffects` (CvCity.cpp:14499-14591, FFH): effects on a unit just
    /// built here (experience, rally plot mission, automation, race/civ-trait promotions,
    /// religion adoption and mutation rolls, free promotion picks). The unit-art-style lines
    /// are graphics (omit).
    pub fn city_apply_build_effects(&mut self, c: CityId, unit: UnitId) {
        todo!("CvCity::applyBuildEffects")
    }

    /// C++ `CvCity::getCrime` (CvCity.cpp:14675-14683, FFH): the game's global crime plus
    /// the city's own (`crime` field), at least 0.
    pub fn city_crime(&self, c: CityId) -> i32 {
        todo!("CvCity::getCrime")
    }
}
