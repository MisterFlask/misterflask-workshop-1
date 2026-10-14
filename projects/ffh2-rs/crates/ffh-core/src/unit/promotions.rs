//! CvUnit promotions, experience and levels, leaders (great generals), upgrades, and the unit
//! characteristics they modify: promotion-driven counters (`changeBlitzCount`, `changeExtraMoves`,
//! ...), hit points and base strength, combat modifiers, first strikes, XP values, interception,
//! fortification and the queries derived from the unit info (`getDomainType`, `isAnimal`, ...).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges. Every `todo!()` names the C++ function still to port.

#![allow(unused_variables, unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues, Unit, UnitResetInfo};
use ffh_data::*;

impl Game {
    /// C++ `CvUnit::canPromote` (CvUnit.cpp:7717-7776).
    /// C++ `iLeaderUnitId` is a unit id of the same owner, -1 = none.
    pub fn unit_can_promote(&self, u: UnitId, promotion: Option<PromotionType>, leader_unit: Option<UnitId>) -> bool {
        todo!("CvUnit::canPromote")
    }

    /// C++ `CvUnit::promote` (CvUnit.cpp:7778-7835).
    /// C++ quirk: `canPromote` accepts NO_PROMOTION when a warlord only gives experience, and
    /// `promote` then indexes the promotion table with it. Keep the call order; guard the lookup.
    pub fn unit_promote(&mut self, u: UnitId, promotion: Option<PromotionType>, leader_unit: Option<UnitId>) {
        todo!("CvUnit::promote")
    }

    /// C++ `CvUnit::lead` (CvUnit.cpp:7837-7877).
    /// `unit` `None` (C++ -1) opens the lead-unit popup for humans and returns false.
    pub fn unit_lead(&mut self, u: UnitId, unit: Option<UnitId>) -> bool {
        todo!("CvUnit::lead")
    }

    /// C++ `CvUnit::canLead` (CvUnit.cpp:7880-7920).
    /// Returns the number of units that could be led (C++ `int canLead`).
    pub fn unit_can_lead(&self, u: UnitId, plot: PlotIdx, unit: Option<UnitId>) -> i32 {
        todo!("CvUnit::canLead")
    }

    /// C++ `CvUnit::canGiveExperience` (CvUnit.cpp:7923-7943).
    pub fn unit_can_give_experience(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::canGiveExperience")
    }

    /// C++ `CvUnit::giveExperience` (CvUnit.cpp:7945-7980).
    pub fn unit_give_experience(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::giveExperience")
    }

    /// C++ `CvUnit::getStackExperienceToGive` (CvUnit.cpp:7982-7985).
    pub fn unit_get_stack_experience_to_give(&self, u: UnitId, num_units: i32) -> i32 {
        todo!("CvUnit::getStackExperienceToGive")
    }

    /// C++ `CvUnit::upgradePrice` (CvUnit.cpp:7987-8027).
    /// Starts with the `getUpgradePriceOverride` Python callback (port in `crate::ffh`).
    pub fn unit_upgrade_price(&self, u: UnitId, unit_type: UnitType) -> i32 {
        todo!("CvUnit::upgradePrice")
    }

    /// C++ `CvUnit::upgradeAvailable` (CvUnit.cpp:8030-8064).
    /// Recursive; C++ default `iCount = 0`.
    pub fn unit_upgrade_available(
        &self,
        u: UnitId,
        from_unit: UnitType,
        to_unit_class: UnitClassType,
        count: i32,
    ) -> bool {
        todo!("CvUnit::upgradeAvailable")
    }

    /// C++ `CvUnit::canUpgrade` (CvUnit.cpp:8067-8138).
    pub fn unit_can_upgrade(&self, u: UnitId, unit_type: Option<UnitType>, test_visible: bool) -> bool {
        todo!("CvUnit::canUpgrade")
    }

    /// C++ `CvUnit::isReadyForUpgrade` (CvUnit.cpp:8140-8153).
    pub fn unit_is_ready_for_upgrade(&self, u: UnitId) -> bool {
        todo!("CvUnit::isReadyForUpgrade")
    }

    /// C++ `CvUnit::hasUpgrade` (CvUnit.cpp:8159-8162).
    /// C++ overload `hasUpgrade(bool bSearch)`.
    pub fn unit_has_upgrade(&self, u: UnitId, search: bool) -> bool {
        todo!("CvUnit::hasUpgrade")
    }

    /// C++ `CvUnit::hasUpgrade` (CvUnit.cpp:8168-8171).
    /// C++ overload `hasUpgrade(UnitTypes eUnit, bool bSearch)`.
    pub fn unit_has_upgrade_to(&self, u: UnitId, unit_type: Option<UnitType>, search: bool) -> bool {
        todo!("CvUnit::hasUpgrade")
    }

    /// C++ `CvUnit::getUpgradeCity` (CvUnit.cpp:8178-8214).
    /// C++ overload `getUpgradeCity(bool bSearch)`. Ranks candidate unit types with
    /// `CvPlayerAI::AI_unitValue` (AI); only AI code calls it.
    pub fn unit_get_upgrade_city(&self, u: UnitId, search: bool) -> Option<CityId> {
        todo!("CvUnit::getUpgradeCity")
    }

    /// C++ `CvUnit::getUpgradeCity` (CvUnit.cpp:8222-8428).
    /// C++ overload `getUpgradeCity(UnitTypes eUnit, bool bSearch, int* iSearchValue)`.
    /// Sephi speed tweak: only cities with squared distance < 10 are path-tested.
    pub fn unit_get_upgrade_city_for(
        &self,
        u: UnitId,
        unit_type: Option<UnitType>,
        search: bool,
        search_value: Option<&mut i32>,
    ) -> Option<CityId> {
        todo!("CvUnit::getUpgradeCity")
    }

    /// C++ `CvUnit::upgrade` (CvUnit.cpp:8430-8470).
    pub fn unit_upgrade(&mut self, u: UnitId, unit_type: Option<UnitType>) {
        todo!("CvUnit::upgrade")
    }

    /// C++ `CvUnit::getHandicapType` (CvUnit.cpp:8473-8476).
    pub fn unit_get_handicap_type(&self, u: UnitId) -> HandicapType {
        todo!("CvUnit::getHandicapType")
    }

    /// C++ `CvUnit::getCivilizationType` (CvUnit.cpp:8479-8482).
    pub fn unit_get_civilization_type(&self, u: UnitId) -> CivilizationType {
        todo!("CvUnit::getCivilizationType")
    }

    /// C++ `CvUnit::getSpecialUnitType` (CvUnit.cpp:8494-8497).
    pub fn unit_get_special_unit_type(&self, u: UnitId) -> Option<SpecialUnitType> {
        todo!("CvUnit::getSpecialUnitType")
    }

    /// C++ `CvUnit::getCaptureUnitType` (CvUnit.cpp:8500-8504).
    pub fn unit_get_capture_unit_type(&self, u: UnitId, civilization: CivilizationType) -> Option<UnitType> {
        todo!("CvUnit::getCaptureUnitType")
    }

    /// C++ `CvUnit::getUnitCombatType` (CvUnit.cpp:8507-8510).
    pub fn unit_get_unit_combat_type(&self, u: UnitId) -> Option<UnitCombatType> {
        todo!("CvUnit::getUnitCombatType")
    }

    /// C++ `CvUnit::getDomainType` (CvUnit.cpp:8513-8516).
    pub fn unit_get_domain_type(&self, u: UnitId) -> DomainType {
        todo!("CvUnit::getDomainType")
    }

    /// C++ `CvUnit::flavorValue` (CvUnit.cpp:8586-8589).
    pub fn unit_flavor_value(&self, u: UnitId, flavor: FlavorType) -> i32 {
        todo!("CvUnit::flavorValue")
    }

    /// C++ `CvUnit::isBarbarian` (CvUnit.cpp:8592-8595).
    pub fn unit_is_barbarian(&self, u: UnitId) -> bool {
        // C++ `GET_PLAYER(getOwnerINLINE()).isBarbarian()` == `getID() == BARBARIAN_PLAYER`.
        self.units[u].owner == self.barbarian_player()
    }

    /// C++ `CvUnit::isHuman` (CvUnit.cpp:8598-8601).
    pub fn unit_is_human(&self, u: UnitId) -> bool {
        todo!("CvUnit::isHuman")
    }

    /// C++ `CvUnit::airRange` (CvUnit.cpp:8666-8669).
    pub fn unit_air_range(&self, u: UnitId) -> i32 {
        todo!("CvUnit::airRange")
    }

    /// C++ `CvUnit::nukeRange` (CvUnit.cpp:8672-8675).
    pub fn unit_nuke_range(&self, u: UnitId) -> i32 {
        todo!("CvUnit::nukeRange")
    }

    /// C++ `CvUnit::isAnimal` (CvUnit.cpp:8799-8802).
    pub fn unit_is_animal(&self, u: UnitId) -> bool {
        todo!("CvUnit::isAnimal")
    }

    /// C++ `CvUnit::isNoBadGoodies` (CvUnit.cpp:8805-8808).
    pub fn unit_is_no_bad_goodies(&self, u: UnitId) -> bool {
        todo!("CvUnit::isNoBadGoodies")
    }

    /// C++ `CvUnit::isOnlyDefensive` (CvUnit.cpp:8811-8822).
    /// FfH: `only_defensive > 0` or the unit info flag.
    pub fn unit_is_only_defensive(&self, u: UnitId) -> bool {
        todo!("CvUnit::isOnlyDefensive")
    }

    /// C++ `CvUnit::isNoCapture` (CvUnit.cpp:8825-8836).
    /// FfH: hidden-nationality units never capture.
    pub fn unit_is_no_capture(&self, u: UnitId) -> bool {
        todo!("CvUnit::isNoCapture")
    }

    /// C++ `CvUnit::isRivalTerritory` (CvUnit.cpp:8839-8842).
    pub fn unit_is_rival_territory(&self, u: UnitId) -> bool {
        todo!("CvUnit::isRivalTerritory")
    }

    /// C++ `CvUnit::isMilitaryHappiness` (CvUnit.cpp:8845-8848).
    pub fn unit_is_military_happiness(&self, u: UnitId) -> bool {
        todo!("CvUnit::isMilitaryHappiness")
    }

    /// C++ `CvUnit::isInvestigate` (CvUnit.cpp:8851-8854).
    pub fn unit_is_investigate(&self, u: UnitId) -> bool {
        todo!("CvUnit::isInvestigate")
    }

    /// C++ `CvUnit::isCounterSpy` (CvUnit.cpp:8857-8860).
    pub fn unit_is_counter_spy(&self, u: UnitId) -> bool {
        todo!("CvUnit::isCounterSpy")
    }

    /// C++ `CvUnit::isSpy` (CvUnit.cpp:8863-8866).
    pub fn unit_is_spy(&self, u: UnitId) -> bool {
        todo!("CvUnit::isSpy")
    }

    /// C++ `CvUnit::isFound` (CvUnit.cpp:8869-8872).
    pub fn unit_is_found(&self, u: UnitId) -> bool {
        todo!("CvUnit::isFound")
    }

    /// C++ `CvUnit::isGoldenAge` (CvUnit.cpp:8875-8883).
    pub fn unit_is_golden_age(&self, u: UnitId) -> bool {
        todo!("CvUnit::isGoldenAge")
    }

    /// C++ `CvUnit::maxHitPoints` (CvUnit.cpp:8929-8932).
    pub fn unit_max_hit_points(&self, u: UnitId) -> i32 {
        self.d.defines.max_hit_points
    }

    /// C++ `CvUnit::currHitPoints` (CvUnit.cpp:8935-8938).
    pub fn unit_curr_hit_points(&self, u: UnitId) -> i32 {
        self.unit_max_hit_points(u) - self.units[u].damage
    }

    /// C++ `CvUnit::isHurt` (CvUnit.cpp:8941-8944).
    pub fn unit_is_hurt(&self, u: UnitId) -> bool {
        self.units[u].damage > 0
    }

    /// C++ `CvUnit::isDead` (CvUnit.cpp:8947-8950).
    pub fn unit_is_dead(&self, u: UnitId) -> bool {
        self.units[u].damage >= self.unit_max_hit_points(u)
    }

    /// C++ `CvUnit::baseCombatStr` (CvUnit.cpp:8958-8971).
    pub fn unit_base_combat_str(&self, u: UnitId) -> i32 {
        // FfH: base strength plus damage-type strength, floored at 0.
        let unit = &self.units[u];
        (unit.base_combat + unit.total_damage_type_combat).max(0)
    }

    /// C++ `CvUnit::baseCombatStrDefense` (CvUnit.cpp:8979-8987).
    pub fn unit_base_combat_str_defense(&self, u: UnitId) -> i32 {
        let unit = &self.units[u];
        (unit.base_combat_defense + unit.total_damage_type_combat).max(0)
    }

    /// C++ `CvUnit::isFortifyable` (CvUnit.cpp:10016-10024).
    pub fn unit_is_fortifyable(&self, u: UnitId) -> bool {
        todo!("CvUnit::isFortifyable")
    }

    /// C++ `CvUnit::fortifyModifier` (CvUnit.cpp:10027-10044).
    pub fn unit_fortify_modifier(&self, u: UnitId) -> i32 {
        todo!("CvUnit::fortifyModifier")
    }

    /// C++ `CvUnit::experienceNeeded` (CvUnit.cpp:10047-10065).
    /// C++ asks the `getExperienceNeeded` Python callback (port in `crate::ffh`).
    pub fn unit_experience_needed(&self, u: UnitId) -> i32 {
        todo!("CvUnit::experienceNeeded")
    }

    /// C++ `CvUnit::attackXPValue` (CvUnit.cpp:10068-10081).
    pub fn unit_attack_xp_value(&self, u: UnitId) -> i32 {
        todo!("CvUnit::attackXPValue")
    }

    /// C++ `CvUnit::defenseXPValue` (CvUnit.cpp:10084-10097).
    pub fn unit_defense_xp_value(&self, u: UnitId) -> i32 {
        todo!("CvUnit::defenseXPValue")
    }

    /// C++ `CvUnit::maxXPValue` (CvUnit.cpp:10100-10129).
    pub fn unit_max_xp_value(&self, u: UnitId) -> i32 {
        todo!("CvUnit::maxXPValue")
    }

    /// C++ `CvUnit::firstStrikes` (CvUnit.cpp:10132-10135).
    pub fn unit_first_strikes(&self, u: UnitId) -> i32 {
        todo!("CvUnit::firstStrikes")
    }

    /// C++ `CvUnit::chanceFirstStrikes` (CvUnit.cpp:10138-10141).
    pub fn unit_chance_first_strikes(&self, u: UnitId) -> i32 {
        todo!("CvUnit::chanceFirstStrikes")
    }

    /// C++ `CvUnit::maxFirstStrikes` (CvUnit.cpp:10144-10147).
    pub fn unit_max_first_strikes(&self, u: UnitId) -> i32 {
        todo!("CvUnit::maxFirstStrikes")
    }

    /// C++ `CvUnit::immuneToFirstStrikes` (CvUnit.cpp:10171-10174).
    pub fn unit_immune_to_first_strikes(&self, u: UnitId) -> bool {
        todo!("CvUnit::immuneToFirstStrikes")
    }

    /// C++ `CvUnit::noDefensiveBonus` (CvUnit.cpp:10177-10180).
    pub fn unit_no_defensive_bonus(&self, u: UnitId) -> bool {
        todo!("CvUnit::noDefensiveBonus")
    }

    /// C++ `CvUnit::ignoreBuildingDefense` (CvUnit.cpp:10183-10195).
    /// FfH: the unit info flag or `ignore_building_defense != 0`.
    pub fn unit_ignore_building_defense(&self, u: UnitId) -> bool {
        todo!("CvUnit::ignoreBuildingDefense")
    }

    /// C++ `CvUnit::isNukeImmune` (CvUnit.cpp:10384-10387).
    pub fn unit_is_nuke_immune(&self, u: UnitId) -> bool {
        todo!("CvUnit::isNukeImmune")
    }

    /// C++ `CvUnit::maxInterceptionProbability` (CvUnit.cpp:10390-10393).
    pub fn unit_max_interception_probability(&self, u: UnitId) -> i32 {
        todo!("CvUnit::maxInterceptionProbability")
    }

    /// C++ `CvUnit::currInterceptionProbability` (CvUnit.cpp:10396-10406).
    pub fn unit_curr_interception_probability(&self, u: UnitId) -> i32 {
        todo!("CvUnit::currInterceptionProbability")
    }

    /// C++ `CvUnit::evasionProbability` (CvUnit.cpp:10409-10412).
    pub fn unit_evasion_probability(&self, u: UnitId) -> i32 {
        todo!("CvUnit::evasionProbability")
    }

    /// C++ `CvUnit::withdrawalProbability` (CvUnit.cpp:10415-10430).
    pub fn unit_withdrawal_probability(&self, u: UnitId) -> i32 {
        todo!("CvUnit::withdrawalProbability")
    }

    /// C++ `CvUnit::collateralDamage` (CvUnit.cpp:10433-10436).
    pub fn unit_collateral_damage(&self, u: UnitId) -> i32 {
        todo!("CvUnit::collateralDamage")
    }

    /// C++ `CvUnit::collateralDamageLimit` (CvUnit.cpp:10439-10442).
    pub fn unit_collateral_damage_limit(&self, u: UnitId) -> i32 {
        todo!("CvUnit::collateralDamageLimit")
    }

    /// C++ `CvUnit::collateralDamageMaxUnits` (CvUnit.cpp:10445-10448).
    pub fn unit_collateral_damage_max_units(&self, u: UnitId) -> i32 {
        todo!("CvUnit::collateralDamageMaxUnits")
    }

    /// C++ `CvUnit::cityAttackModifier` (CvUnit.cpp:10451-10454).
    pub fn unit_city_attack_modifier(&self, u: UnitId) -> i32 {
        todo!("CvUnit::cityAttackModifier")
    }

    /// C++ `CvUnit::cityDefenseModifier` (CvUnit.cpp:10457-10460).
    pub fn unit_city_defense_modifier(&self, u: UnitId) -> i32 {
        todo!("CvUnit::cityDefenseModifier")
    }

    /// C++ `CvUnit::animalCombatModifier` (CvUnit.cpp:10463-10466).
    pub fn unit_animal_combat_modifier(&self, u: UnitId) -> i32 {
        todo!("CvUnit::animalCombatModifier")
    }

    /// C++ `CvUnit::hillsAttackModifier` (CvUnit.cpp:10469-10472).
    pub fn unit_hills_attack_modifier(&self, u: UnitId) -> i32 {
        todo!("CvUnit::hillsAttackModifier")
    }

    /// C++ `CvUnit::hillsDefenseModifier` (CvUnit.cpp:10475-10478).
    pub fn unit_hills_defense_modifier(&self, u: UnitId) -> i32 {
        todo!("CvUnit::hillsDefenseModifier")
    }

    /// C++ `CvUnit::terrainAttackModifier` (CvUnit.cpp:10481-10486).
    pub fn unit_terrain_attack_modifier(&self, u: UnitId, terrain: TerrainType) -> i32 {
        todo!("CvUnit::terrainAttackModifier")
    }

    /// C++ `CvUnit::terrainDefenseModifier` (CvUnit.cpp:10489-10494).
    pub fn unit_terrain_defense_modifier(&self, u: UnitId, terrain: TerrainType) -> i32 {
        todo!("CvUnit::terrainDefenseModifier")
    }

    /// C++ `CvUnit::featureAttackModifier` (CvUnit.cpp:10497-10502).
    pub fn unit_feature_attack_modifier(&self, u: UnitId, feature: FeatureType) -> i32 {
        todo!("CvUnit::featureAttackModifier")
    }

    /// C++ `CvUnit::featureDefenseModifier` (CvUnit.cpp:10504-10509).
    pub fn unit_feature_defense_modifier(&self, u: UnitId, feature: FeatureType) -> i32 {
        todo!("CvUnit::featureDefenseModifier")
    }

    /// C++ `CvUnit::unitClassAttackModifier` (CvUnit.cpp:10511-10516).
    pub fn unit_unit_class_attack_modifier(&self, u: UnitId, unit_class: UnitClassType) -> i32 {
        todo!("CvUnit::unitClassAttackModifier")
    }

    /// C++ `CvUnit::unitClassDefenseModifier` (CvUnit.cpp:10519-10524).
    pub fn unit_unit_class_defense_modifier(&self, u: UnitId, unit_class: UnitClassType) -> i32 {
        todo!("CvUnit::unitClassDefenseModifier")
    }

    /// C++ `CvUnit::unitCombatModifier` (CvUnit.cpp:10527-10532).
    pub fn unit_unit_combat_modifier(&self, u: UnitId, unit_combat: UnitCombatType) -> i32 {
        todo!("CvUnit::unitCombatModifier")
    }

    /// C++ `CvUnit::domainModifier` (CvUnit.cpp:10535-10540).
    pub fn unit_domain_modifier(&self, u: UnitId, domain: DomainType) -> i32 {
        todo!("CvUnit::domainModifier")
    }

    /// C++ `CvUnit::bombardRate` (CvUnit.cpp:10543-10546).
    pub fn unit_bombard_rate(&self, u: UnitId) -> i32 {
        todo!("CvUnit::bombardRate")
    }

    /// C++ `CvUnit::airBombBaseRate` (CvUnit.cpp:10549-10552).
    pub fn unit_air_bomb_base_rate(&self, u: UnitId) -> i32 {
        todo!("CvUnit::airBombBaseRate")
    }

    /// C++ `CvUnit::airBombCurrRate` (CvUnit.cpp:10555-10558).
    pub fn unit_air_bomb_curr_rate(&self, u: UnitId) -> i32 {
        todo!("CvUnit::airBombCurrRate")
    }

    /// C++ `CvUnit::setExperience` (CvUnit.cpp:11788-11800).
    pub fn unit_set_experience(&mut self, u: UnitId, value: i32, max: i32) {
        // `max == -1` means no maximum (C++ default); the info pane update is interface-only.
        let max = if max == -1 { MAX_INT } else { max };
        let unit = &mut self.units[u];
        if unit.experience != value && unit.experience < max {
            unit.experience = max.min(value);
        }
    }

    /// C++ `CvUnit::changeExperience` (CvUnit.cpp:11802-11840).
    /// C++ defaults: `iMax = -1`, `bFromCombat = false`, `bInBorders = false`, `bUpdateGlobal = false`.
    pub fn unit_change_experience(
        &mut self,
        u: UnitId,
        change: i32,
        max: i32,
        from_combat: bool,
        in_borders: bool,
        update_global: bool,
    ) {
        todo!("CvUnit::changeExperience")
    }

    /// C++ `CvUnit::setLevel` (CvUnit.cpp:11847-11864).
    pub fn unit_set_level(&mut self, u: UnitId, value: i32) {
        todo!("CvUnit::setLevel")
    }

    /// C++ `CvUnit::changeLevel` (CvUnit.cpp:11866-11869).
    pub fn unit_change_level(&mut self, u: UnitId, change: i32) {
        todo!("CvUnit::changeLevel")
    }

    /// C++ `CvUnit::setFortifyTurns` (CvUnit.cpp:11971-11980).
    pub fn unit_set_fortify_turns(&mut self, u: UnitId, value: i32) {
        // C++ `range(iNewValue, 0, MAX_FORTIFY_TURNS)`; the info bar update is interface-only.
        let max = self.d.defines.max_fortify_turns;
        let value = if value < 0 {
            0
        } else if value > max {
            max
        } else {
            value
        };
        self.units[u].fortify_turns = value;
    }

    /// C++ `CvUnit::changeFortifyTurns` (CvUnit.cpp:11982-11985).
    pub fn unit_change_fortify_turns(&mut self, u: UnitId, change: i32) {
        let value = self.units[u].fortify_turns + change;
        self.unit_set_fortify_turns(u, value);
    }

    /// C++ `CvUnit::isBlitz` (CvUnit.cpp:11992-11995).
    pub fn unit_is_blitz(&self, u: UnitId) -> bool {
        self.units[u].blitz_count > 0
    }

    /// C++ `CvUnit::changeBlitzCount` (CvUnit.cpp:11997-12001).
    pub fn unit_change_blitz_count(&mut self, u: UnitId, change: i32) {
        self.units[u].blitz_count += change;
    }

    /// C++ `CvUnit::isAmphib` (CvUnit.cpp:12008-12011).
    pub fn unit_is_amphib(&self, u: UnitId) -> bool {
        self.units[u].amphib_count > 0
    }

    /// C++ `CvUnit::changeAmphibCount` (CvUnit.cpp:12013-12017).
    pub fn unit_change_amphib_count(&mut self, u: UnitId, change: i32) {
        self.units[u].amphib_count += change;
    }

    /// C++ `CvUnit::isRiver` (CvUnit.cpp:12024-12027).
    pub fn unit_is_river(&self, u: UnitId) -> bool {
        self.units[u].river_count > 0
    }

    /// C++ `CvUnit::changeRiverCount` (CvUnit.cpp:12029-12033).
    pub fn unit_change_river_count(&mut self, u: UnitId, change: i32) {
        self.units[u].river_count += change;
    }

    /// C++ `CvUnit::isEnemyRoute` (CvUnit.cpp:12040-12043).
    pub fn unit_is_enemy_route(&self, u: UnitId) -> bool {
        self.units[u].enemy_route_count > 0
    }

    /// C++ `CvUnit::changeEnemyRouteCount` (CvUnit.cpp:12045-12049).
    pub fn unit_change_enemy_route_count(&mut self, u: UnitId, change: i32) {
        self.units[u].enemy_route_count += change;
    }

    /// C++ `CvUnit::isAlwaysHeal` (CvUnit.cpp:12056-12059).
    pub fn unit_is_always_heal(&self, u: UnitId) -> bool {
        self.units[u].always_heal_count > 0
    }

    /// C++ `CvUnit::changeAlwaysHealCount` (CvUnit.cpp:12061-12065).
    pub fn unit_change_always_heal_count(&mut self, u: UnitId, change: i32) {
        self.units[u].always_heal_count += change;
    }

    /// C++ `CvUnit::isHillsDoubleMove` (CvUnit.cpp:12072-12075).
    pub fn unit_is_hills_double_move(&self, u: UnitId) -> bool {
        self.units[u].hills_double_move_count > 0
    }

    /// C++ `CvUnit::changeHillsDoubleMoveCount` (CvUnit.cpp:12077-12081).
    pub fn unit_change_hills_double_move_count(&mut self, u: UnitId, change: i32) {
        self.units[u].hills_double_move_count += change;
    }

    /// C++ `CvUnit::changeImmuneToFirstStrikesCount` (CvUnit.cpp:12088-12092).
    pub fn unit_change_immune_to_first_strikes_count(&mut self, u: UnitId, change: i32) {
        self.units[u].immune_to_first_strikes_count += change;
    }

    /// C++ `CvUnit::changeExtraMoves` (CvUnit.cpp:12119-12123).
    /// Can become negative (Sephi 0.41k: mutations).
    pub fn unit_change_extra_moves(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_moves += change;
    }

    /// C++ `CvUnit::changeExtraMoveDiscount` (CvUnit.cpp:12132-12136).
    pub fn unit_change_extra_move_discount(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_move_discount += change;
    }

    /// C++ `CvUnit::changeExtraAirRange` (CvUnit.cpp:12143-12146).
    pub fn unit_change_extra_air_range(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_air_range += change;
    }

    /// C++ `CvUnit::changeExtraIntercept` (CvUnit.cpp:12153-12156).
    pub fn unit_change_extra_intercept(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_intercept += change;
    }

    /// C++ `CvUnit::changeExtraEvasion` (CvUnit.cpp:12163-12166).
    pub fn unit_change_extra_evasion(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_evasion += change;
    }

    /// C++ `CvUnit::changeExtraFirstStrikes` (CvUnit.cpp:12173-12177).
    pub fn unit_change_extra_first_strikes(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_first_strikes += change;
    }

    /// C++ `CvUnit::changeExtraChanceFirstStrikes` (CvUnit.cpp:12184-12188).
    pub fn unit_change_extra_chance_first_strikes(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_chance_first_strikes += change;
    }

    /// C++ `CvUnit::changeExtraWithdrawal` (CvUnit.cpp:12197-12201).
    pub fn unit_change_extra_withdrawal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_withdrawal += change;
    }

    /// C++ `CvUnit::changeExtraCollateralDamage` (CvUnit.cpp:12208-12212).
    pub fn unit_change_extra_collateral_damage(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_collateral_damage += change;
    }

    /// C++ `CvUnit::changeExtraBombardRate` (CvUnit.cpp:12219-12223).
    pub fn unit_change_extra_bombard_rate(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_bombard_rate += change;
    }

    /// C++ `CvUnit::changeExtraEnemyHeal` (CvUnit.cpp:12230-12234).
    pub fn unit_change_extra_enemy_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_enemy_heal += change;
    }

    /// C++ `CvUnit::changeExtraNeutralHeal` (CvUnit.cpp:12241-12245).
    pub fn unit_change_extra_neutral_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_neutral_heal += change;
    }

    /// C++ `CvUnit::changeExtraFriendlyHeal` (CvUnit.cpp:12253-12257).
    pub fn unit_change_extra_friendly_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_friendly_heal += change;
    }

    /// C++ `CvUnit::changeSameTileHeal` (CvUnit.cpp:12264-12268).
    pub fn unit_change_same_tile_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].same_tile_heal += change;
    }

    /// C++ `CvUnit::changeAdjacentTileHeal` (CvUnit.cpp:12275-12279).
    pub fn unit_change_adjacent_tile_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].adjacent_tile_heal += change;
    }

    /// C++ `CvUnit::getExtraCombatPercent` (CvUnit.cpp:12281-12294).
    /// FfH: `extra_combat_percent` plus `combat_percent_in_borders` inside own borders.
    pub fn unit_get_extra_combat_percent(&self, u: UnitId) -> i32 {
        todo!("CvUnit::getExtraCombatPercent")
    }

    /// C++ `CvUnit::changeExtraCombatPercent` (CvUnit.cpp:12296-12304).
    pub fn unit_change_extra_combat_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_combat_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraCityAttackPercent` (CvUnit.cpp:12311-12319).
    pub fn unit_change_extra_city_attack_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_city_attack_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraCityDefensePercent` (CvUnit.cpp:12326-12334).
    pub fn unit_change_extra_city_defense_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_city_defense_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraHillsAttackPercent` (CvUnit.cpp:12341-12349).
    pub fn unit_change_extra_hills_attack_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_hills_attack_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraHillsDefensePercent` (CvUnit.cpp:12356-12364).
    pub fn unit_change_extra_hills_defense_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_hills_defense_percent += change;
        }
    }

    /// C++ `CvUnit::changeRevoltProtection` (CvUnit.cpp:12371-12379).
    pub fn unit_change_revolt_protection(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].revolt_protection += change;
        }
    }

    /// C++ `CvUnit::changeCollateralDamageProtection` (CvUnit.cpp:12386-12394).
    pub fn unit_change_collateral_damage_protection(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].collateral_damage_protection += change;
        }
    }

    /// C++ `CvUnit::changePillageChange` (CvUnit.cpp:12401-12409).
    pub fn unit_change_pillage_change(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].pillage_change += change;
        }
    }

    /// C++ `CvUnit::changeUpgradeDiscount` (CvUnit.cpp:12416-12424).
    pub fn unit_change_upgrade_discount(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].upgrade_discount += change;
        }
    }

    /// C++ `CvUnit::changeExperiencePercent` (CvUnit.cpp:12431-12439).
    pub fn unit_change_experience_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].experience_percent += change;
        }
    }

    /// C++ `CvUnit::changeKamikazePercent` (CvUnit.cpp:12446-12454).
    pub fn unit_change_kamikaze_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].kamikaze_percent += change;
        }
    }

    /// C++ `CvUnit::setPromotionReady` (CvUnit.cpp:12591-12621).
    /// On becoming ready (unless UNITAI_TERRAFORMER) the group stops automation, clears its
    /// missions and wakes up. The promotion glow is graphics.
    pub fn unit_set_promotion_ready(&mut self, u: UnitId, value: bool) {
        todo!("CvUnit::setPromotionReady")
    }

    /// C++ `CvUnit::testPromotionReady` (CvUnit.cpp:12624-12627).
    pub fn unit_test_promotion_ready(&mut self, u: UnitId) {
        todo!("CvUnit::testPromotionReady")
    }

    /// C++ `CvUnit::getUnitClassType` (CvUnit.cpp:12813-12816).
    pub fn unit_get_unit_class_type(&self, u: UnitId) -> UnitClassType {
        todo!("CvUnit::getUnitClassType")
    }

    /// C++ `CvUnit::changeExtraDomainModifier` (CvUnit.cpp:12989-12994).
    pub fn unit_change_extra_domain_modifier(&mut self, u: UnitId, domain: DomainType, change: i32) {
        self.units[u].extra_domain_modifier[domain.idx()] += change;
    }

    /// C++ `CvUnit::changeExtraTerrainAttackPercent` (CvUnit.cpp:13114-13125).
    pub fn unit_change_extra_terrain_attack_percent(&mut self, u: UnitId, terrain: TerrainType, change: i32) {
        if change != 0 {
            self.units[u].extra_terrain_attack_percent[terrain] += change;
        }
    }

    /// C++ `CvUnit::changeExtraTerrainDefensePercent` (CvUnit.cpp:13135-13146).
    pub fn unit_change_extra_terrain_defense_percent(&mut self, u: UnitId, terrain: TerrainType, change: i32) {
        if change != 0 {
            self.units[u].extra_terrain_defense_percent[terrain] += change;
        }
    }

    /// C++ `CvUnit::changeExtraFeatureAttackPercent` (CvUnit.cpp:13156-13167).
    pub fn unit_change_extra_feature_attack_percent(&mut self, u: UnitId, feature: FeatureType, change: i32) {
        if change != 0 {
            self.units[u].extra_feature_attack_percent[feature] += change;
        }
    }

    /// C++ `CvUnit::changeExtraFeatureDefensePercent` (CvUnit.cpp:13177-13188).
    pub fn unit_change_extra_feature_defense_percent(&mut self, u: UnitId, feature: FeatureType, change: i32) {
        if change != 0 {
            self.units[u].extra_feature_defense_percent[feature] += change;
        }
    }

    /// C++ `CvUnit::changeExtraUnitCombatModifier` (CvUnit.cpp:13198-13203).
    pub fn unit_change_extra_unit_combat_modifier(&mut self, u: UnitId, unit_combat: UnitCombatType, change: i32) {
        self.units[u].extra_unit_combat_modifier[unit_combat] += change;
    }

    /// C++ `CvUnit::canAcquirePromotion` (CvUnit.cpp:13206-13347).
    pub fn unit_can_acquire_promotion(&self, u: UnitId, promotion: PromotionType) -> bool {
        todo!("CvUnit::canAcquirePromotion")
    }

    /// C++ `CvUnit::isPromotionValid` (CvUnit.cpp:13349-13383).
    /// Starts with `::isPromotionValid(ePromotion, getUnitType(), true)` (CvGameCoreUtils).
    pub fn unit_is_promotion_valid(&self, u: UnitId, promotion: PromotionType) -> bool {
        todo!("CvUnit::isPromotionValid")
    }

    /// C++ `CvUnit::canAcquirePromotionAny` (CvUnit.cpp:13386-13399).
    pub fn unit_can_acquire_promotion_any(&self, u: UnitId) -> bool {
        todo!("CvUnit::canAcquirePromotionAny")
    }

    /// C++ `CvUnit::setHasPromotion` (CvUnit.cpp:13410-13620).
    /// Applies/removes every promotion effect through the `unit_change_*` methods (the FfH
    /// part includes race, weapons art style/group size (graphics), terraformer update).
    pub fn unit_set_has_promotion(&mut self, u: UnitId, promotion: PromotionType, value: bool) {
        todo!("CvUnit::setHasPromotion")
    }

    /// C++ `CvUnit::isSuicide` (CvUnit.cpp:18041-18044).
    pub fn unit_is_suicide(&self, u: UnitId) -> bool {
        todo!("CvUnit::isSuicide")
    }

    /// C++ `CvUnit::getDropRange` (CvUnit.cpp:18046-18049).
    pub fn unit_get_drop_range(&self, u: UnitId) -> i32 {
        todo!("CvUnit::getDropRange")
    }
}
