//! CvPlayer: random events -- trigger selection and weights, triggered-event data,
//! event options (`canDoEvent`/`applyEvent`), costs and techs, quests and expiry, and the
//! per-player event bookkeeping (events occurred, countdowns, fired triggers).

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::defs::INVALID_PLOT_COORD;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

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

    /// C++ `CvPlayer::setEventOccured(eEvent, kEventTriggered, bOthers = true)`: quest
    /// message, and copies global/team events to the other players.
    pub fn player_set_event_occured(&mut self, p: PlayerId, event: EventType, data: &EventTriggeredData, others: bool) {
        todo!("CvPlayer::setEventOccured")
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
    /// records the trigger (copied to living players for global/team triggers) and
    /// announces it (world news).
    pub fn player_set_trigger_fired(&mut self, p: PlayerId, data: &EventTriggeredData, others: bool, announce: bool) {
        todo!("CvPlayer::setTriggerFired")
    }

    /// C++ `CvPlayer::resetTriggerFired(eTrigger)`.
    pub fn player_reset_trigger_fired(&mut self, p: PlayerId, trigger: EventTriggerType) {
        let fired = &mut self.players[p].triggers_fired;
        if let Some(i) = fired.iter().position(|&t| t == trigger) {
            fired.remove(i);
        }
    }

    // ------------------------------------------------------------------ triggering

    /// C++ `CvPlayer::doEvents`: expires finished quests/events, counts down delayed events,
    /// then rolls for a new trigger (GAMEOPTION_NO_EVENTS, barbarians and minor civs excluded).
    pub fn player_do_events(&mut self, p: PlayerId) {
        todo!("CvPlayer::doEvents")
    }

    /// C++ `CvPlayer::getEventTriggerWeight(eTrigger)` (0 = cannot trigger; -1 = always).
    pub fn player_get_event_trigger_weight(&self, p: PlayerId, trigger: EventTriggerType) -> i32 {
        todo!("CvPlayer::getEventTriggerWeight")
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
        todo!("CvPlayer::initTriggeredData")
    }

    /// C++ `CvPlayer::trigger(EventTriggerTypes)`: `initTriggeredData(eTrigger, true)`.
    pub fn player_trigger(&mut self, p: PlayerId, trigger: EventTriggerType) {
        self.player_init_triggered_data(p, trigger, true, None, INVALID_PLOT_COORD, INVALID_PLOT_COORD, None, None, None, None, None, None);
    }

    /// C++ `CvPlayer::trigger(const EventTriggeredData&)`: humans get the event popup (route to
    /// the controller); AI players choose through the controller (`AI_chooseEvent`) and
    /// `applyEvent` the choice.
    pub fn player_trigger_data(&mut self, p: PlayerId, data: &EventTriggeredData) {
        todo!("CvPlayer::trigger")
    }

    /// C++ `CvPlayer::canTrigger(eTrigger, ePlayer, eReligion)`: can `p` be the "other
    /// player" of `other`'s trigger.
    pub fn player_can_trigger(&self, p: PlayerId, trigger: EventTriggerType, other: PlayerId, religion: Option<ReligionType>) -> bool {
        todo!("CvPlayer::canTrigger")
    }

    /// C++ `CvPlayer::pickTriggerCity(eTrigger)` (const in C++, but breaks ties with the game RNG).
    pub fn player_pick_trigger_city(&mut self, p: PlayerId, trigger: EventTriggerType) -> Option<CityId> {
        todo!("CvPlayer::pickTriggerCity")
    }

    /// C++ `CvPlayer::pickTriggerUnit(eTrigger, pPlot, bPickPlot)` (const in C++, but breaks
    /// ties with the game RNG).
    pub fn player_pick_trigger_unit(&mut self, p: PlayerId, trigger: EventTriggerType, plot: Option<PlotIdx>, pick_plot: bool) -> Option<UnitId> {
        todo!("CvPlayer::pickTriggerUnit")
    }

    /// C++ `CvPlayer::isValidTriggerReligion(kTrigger, pCity, eReligion)`.
    pub fn player_is_valid_trigger_religion(&self, p: PlayerId, trigger: EventTriggerType, city: Option<CityId>, religion: ReligionType) -> bool {
        todo!("CvPlayer::isValidTriggerReligion")
    }

    /// C++ `CvPlayer::isValidTriggerCorporation(kTrigger, pCity, eCorporation)`.
    pub fn player_is_valid_trigger_corporation(&self, p: PlayerId, trigger: EventTriggerType, city: Option<CityId>, corporation: CorporationType) -> bool {
        todo!("CvPlayer::isValidTriggerCorporation")
    }

    // ------------------------------------------------------------------ event options

    /// C++ `CvPlayer::canDoEvent(eEvent, kTriggeredData)`.
    pub fn player_can_do_event(&self, p: PlayerId, event: EventType, data: &EventTriggeredData) -> bool {
        todo!("CvPlayer::canDoEvent")
    }

    /// C++ `CvPlayer::applyEvent(eEvent, iTriggeredId, bUpdateTrigger = true)`: applies an
    /// event option of a pending triggered event (and Python `applyEvent` callbacks).
    pub fn player_apply_event(&mut self, p: PlayerId, event: EventType, triggered: TriggeredId, update_trigger: bool) {
        todo!("CvPlayer::applyEvent")
    }

    /// C++ `CvPlayer::isValidEventTech(eTech, eEvent, eOtherPlayer)`.
    pub fn player_is_valid_event_tech(&self, p: PlayerId, tech: TechType, event: EventType, other: Option<PlayerId>) -> bool {
        todo!("CvPlayer::isValidEventTech")
    }

    /// C++ `CvPlayer::getBestEventTech(eEvent, eOtherPlayer)`.
    pub fn player_get_best_event_tech(&self, p: PlayerId, event: EventType, other: Option<PlayerId>) -> Option<TechType> {
        todo!("CvPlayer::getBestEventTech")
    }

    /// C++ `CvPlayer::getEventCost(eEvent, eOtherPlayer, bRandom)` (gold; negative = income).
    pub fn player_get_event_cost(&self, p: PlayerId, event: EventType, other: Option<PlayerId>, random: bool) -> i32 {
        todo!("CvPlayer::getEventCost")
    }

    // ------------------------------------------------------------------ expiry

    /// C++ `CvPlayer::checkExpireEvent(eEvent, kTriggeredData)` (Python expire checks, lost
    /// cities/units/religions, quest timeouts).
    pub fn player_check_expire_event(&self, p: PlayerId, event: EventType, data: &EventTriggeredData) -> bool {
        todo!("CvPlayer::checkExpireEvent")
    }

    /// C++ `CvPlayer::expireEvent(eEvent, kTriggeredData, bFail)` (quest failure message and
    /// Python `expireEvent` callbacks).
    pub fn player_expire_event(&mut self, p: PlayerId, event: EventType, data: &EventTriggeredData, fail: bool) {
        todo!("CvPlayer::expireEvent")
    }
}
