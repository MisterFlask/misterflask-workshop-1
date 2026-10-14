//! Port of CustomFunctions.py (FFH2 helper functions shared by spells, events and
//! per-turn processing). Methods are prefixed `cf_`.
//!
//! Not ported: `addPopup`, `addPlayerPopup`, `showUniqueImprovements`, `warn` (UI only);
//! `warScript`, `warScriptAllow`, `dogpile`, `dogPileAllow` (AI war policy, replaced by
//! the new AI -- see docs/PORTING.md).

#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `addBonus(iBonus, iNum, sIcon)`: place `num` of a bonus on random valid plots.
    pub fn cf_add_bonus(&mut self, bonus: BonusType, num: i32) {
        todo!("CustomFunctions.addBonus")
    }
    /// `addUnit(iUnit)`: spawn a barbarian unit on the best empty land plot.
    pub fn cf_add_unit(&mut self, unit: UnitType) {
        todo!("CustomFunctions.addUnit")
    }
    /// `addUnitFixed(caster, iUnit)`: spawn a barbarian unit next to the caster.
    pub fn cf_add_unit_fixed(&mut self, caster: UnitId, unit: UnitType) -> Option<UnitId> {
        todo!("CustomFunctions.addUnitFixed")
    }
    /// `doCrusade(iPlayer)`.
    pub fn cf_do_crusade(&mut self, p: PlayerId) {
        todo!("CustomFunctions.doCrusade")
    }
    /// `doFear(pVictim, pPlot, pCaster, bResistable)`: true if the victim fled.
    pub fn cf_do_fear(&mut self, victim: UnitId, plot: PlotIdx, caster: UnitId, resistable: bool) -> bool {
        todo!("CustomFunctions.doFear")
    }
    /// `exploreLairBigBad(caster)`: returns the Python result value.
    pub fn cf_explore_lair_big_bad(&mut self, caster: UnitId) -> i32 {
        todo!("CustomFunctions.exploreLairBigBad")
    }
    /// `exploreLairBad(caster)`.
    pub fn cf_explore_lair_bad(&mut self, caster: UnitId) -> i32 {
        todo!("CustomFunctions.exploreLairBad")
    }
    /// `exploreLairNeutral(caster)`.
    pub fn cf_explore_lair_neutral(&mut self, caster: UnitId) -> i32 {
        todo!("CustomFunctions.exploreLairNeutral")
    }
    /// `exploreLairGood(caster)`.
    pub fn cf_explore_lair_good(&mut self, caster: UnitId) -> i32 {
        todo!("CustomFunctions.exploreLairGood")
    }
    /// `exploreLairBigGood(caster)`.
    pub fn cf_explore_lair_big_good(&mut self, caster: UnitId) -> i32 {
        todo!("CustomFunctions.exploreLairBigGood")
    }
    /// `formEmpire(iCiv, iLeader, iTeam, pCity, iAlignment, pFromPlayer)`: a new player takes over `city`.
    pub fn cf_form_empire(&mut self, civ: CivilizationType, leader: LeaderHeadType, team: Option<TeamId>, city: CityId, alignment: Option<AlignmentType>, from_player: Option<PlayerId>) {
        todo!("CustomFunctions.formEmpire")
    }
    /// `grace()`: early-game grace period check (uses the RNG).
    pub fn cf_grace(&mut self) -> bool {
        todo!("CustomFunctions.grace")
    }
    /// `doCityFire(pCity)`.
    pub fn cf_do_city_fire(&mut self, city: CityId) {
        todo!("CustomFunctions.doCityFire")
    }
    /// `doHellTerrain()`: Armageddon terrain spread.
    pub fn cf_do_hell_terrain(&mut self) {
        todo!("CustomFunctions.doHellTerrain")
    }
    /// `doTurnKhazad(iPlayer)`.
    pub fn cf_do_turn_khazad(&mut self, p: PlayerId) {
        todo!("CustomFunctions.doTurnKhazad")
    }
    /// `doTurnLuchuirp(iPlayer)`.
    pub fn cf_do_turn_luchuirp(&mut self, p: PlayerId) {
        todo!("CustomFunctions.doTurnLuchuirp")
    }
    /// `findClearPlot(pUnit, plot)`: `unit = None` corresponds to Python `pUnit == -1`.
    pub fn cf_find_clear_plot(&mut self, unit: Option<UnitId>, plot: PlotIdx) -> Option<PlotIdx> {
        todo!("CustomFunctions.findClearPlot")
    }
    /// `genesis(iPlayer)`.
    pub fn cf_genesis(&mut self, p: PlayerId) {
        todo!("CustomFunctions.genesis")
    }
    /// `getAshenVeilCity(iNum)`: the num-th (1..=3) best Ashen Veil city.
    pub fn cf_get_ashen_veil_city(&self, num: i32) -> Option<CityId> {
        todo!("CustomFunctions.getAshenVeilCity")
    }
    /// `getCivilization(iCiv)`: the (last) player of this civilization.
    pub fn cf_get_civilization(&self, civ: CivilizationType) -> Option<PlayerId> {
        todo!("CustomFunctions.getCivilization")
    }
    /// `getHero(pPlayer)`: the civilization's hero unit class.
    pub fn cf_get_hero(&self, p: PlayerId) -> Option<UnitClassType> {
        todo!("CustomFunctions.getHero")
    }
    /// `getLeader(iLeader)`: the (last) player with this leader.
    pub fn cf_get_leader(&self, leader: LeaderHeadType) -> Option<PlayerId> {
        todo!("CustomFunctions.getLeader")
    }
    /// `getOpenPlayer()`: first never-alive player slot.
    pub fn cf_get_open_player(&self) -> Option<PlayerId> {
        todo!("CustomFunctions.getOpenPlayer")
    }
    /// `getUnholyVersion(pUnit)`: the unit type a unit becomes when corrupted.
    pub fn cf_get_unholy_version(&self, u: UnitId) -> Option<UnitType> {
        todo!("CustomFunctions.getUnholyVersion")
    }
    /// `giftUnit(iUnit, iCivilization, iXP, pFromPlot, iFromPlayer)`.
    pub fn cf_gift_unit(&mut self, unit: UnitType, civ: CivilizationType, xp: i32, from_plot: Option<PlotIdx>, from_player: Option<PlayerId>) {
        todo!("CustomFunctions.giftUnit")
    }
    /// `placeTreasure(iPlayer, iUnit)`.
    pub fn cf_place_treasure(&mut self, p: PlayerId, unit: UnitType) {
        todo!("CustomFunctions.placeTreasure")
    }
    /// `startWar(iPlayer, i2Player, iWarPlan)`.
    pub fn cf_start_war(&mut self, p: PlayerId, p2: PlayerId, war_plan: WarPlanType) {
        todo!("CustomFunctions.startWar")
    }
    /// `MarnokNameGenerator(unit)`: random unit name (uses the RNG).
    pub fn cf_marnok_name_generator(&mut self, u: UnitId) -> String {
        todo!("CustomFunctions.MarnokNameGenerator")
    }
    /// `BarbCityProduction(pCity)`: true if a production order was set.
    pub fn cf_barb_city_production(&mut self, city: CityId) -> bool {
        todo!("CustomFunctions.BarbCityProduction")
    }
}
