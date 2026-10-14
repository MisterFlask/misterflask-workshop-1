//! FfH2 additions to CvUnit: spells (`canCast`, `cast`, ...), summons, damage types and
//! resistances, immortality, weapons, bonus affinity, betrayal, terraformers and the FfH status
//! counters (`isFlying`, `changeFear`, ...).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported (`CvUnit::chooseSpell` is AI and lives in ffh-ai).
//!
//! Spell effects (`cast*`) walk the units of each plot around the caster the way the C++ walks
//! the live `CLinkList` (see [`PlotUnitWalk`]) and re-read the caster's position for every plot
//! (C++ `plotXY(plot()->getX_INLINE(), plot()->getY_INLINE(), i, j)`): an effect can move the
//! caster (losing `PROMOTION_HIDDEN` next to an enemy withdraws it). When an effect deletes the
//! caster or the unit being processed, the C++ keeps using the deleted object (undefined
//! behaviour); the port stops using it there.

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// C++ `CvPlot::getPlotCity()` (`getCity(IDInfo)`: NULL for a deleted city).
fn live_plot_city(g: &Game, p: PlotIdx) -> Option<CityId> {
    g.map.plots[p].plot_city.filter(|&c| g.cities.contains(c))
}

/// C++ `CvUnit::getGroup()`: NULL without a group or when the group was deleted.
fn live_group(g: &Game, u: UnitId) -> Option<GroupId> {
    g.units[u].group.filter(|&grp| g.groups.contains(grp))
}

/// C++ `plot()->getX_INLINE(), plot()->getY_INLINE()` of a spell caster, read anew for every plot
/// of a spell's range. `None` once the caster was deleted or left the map (C++ would dereference
/// a deleted unit or a NULL plot); the spell effect stops there.
fn caster_xy(g: &Game, u: UnitId) -> Option<(i32, i32)> {
    if !g.units.contains(u) {
        return None;
    }
    let unit = &g.units[u];
    if unit.x == INVALID_PLOT_COORD || unit.y == INVALID_PLOT_COORD {
        return None;
    }
    Some((unit.x, unit.y))
}

/// C++ `isHasPromotion((PromotionTypes)GC.getDefineINT(..))`: C++ would read out of bounds for a
/// -1 define; here such a promotion is never held.
fn has_promotion_define(g: &Game, u: UnitId, define: i32) -> bool {
    opt_from_i32::<PromotionType>(define).map_or(false, |p| g.units[u].has_promotion[p])
}

/// A C++ walk over a plot's live unit list that fetches the next node before using the current
/// unit, while the loop body may move, kill or create units:
///
/// ```text
/// pUnitNode = pPlot->headUnitNode();
/// while (pUnitNode != NULL)
/// {
///     pLoopUnit = ::getUnit(pUnitNode->m_data);
///     pUnitNode = pPlot->nextUnitNode(pUnitNode);
///     ...
/// }
/// ```
///
/// Units that join the plot after the fetched node (`CvPlot::addUnit` keeps the unit-cycle
/// order) are visited; units that leave the plot before their turn are not. If the fetched node
/// itself leaves the plot meanwhile, C++ follows a deleted node (undefined behaviour); the walk
/// then resumes at the first unit after it (in the list it was fetched from) still on the plot.
struct PlotUnitWalk {
    plot: PlotIdx,
    /// C++ `pUnitNode`: the unit to visit next.
    next: Option<UnitId>,
    /// The plot's unit list when `next` was fetched.
    list: Vec<UnitId>,
}

impl PlotUnitWalk {
    fn new(g: &Game, plot: PlotIdx) -> PlotUnitWalk {
        let list = g.map.plots[plot].units.clone();
        PlotUnitWalk { plot, next: list.first().copied(), list }
    }

    /// The next unit to visit (C++ `pLoopUnit`); fetches the one after it (`nextUnitNode`).
    fn next_unit(&mut self, g: &Game) -> Option<UnitId> {
        let units = &g.map.plots[self.plot].units;
        let mut current = self.next?;
        if !units.contains(&current) {
            let i = self.list.iter().position(|&x| x == current)?;
            current = self.list[i + 1..].iter().copied().find(|x| units.contains(x))?;
        }
        let i = units.iter().position(|&x| x == current)?;
        self.next = units.get(i + 1).copied();
        self.list = units.clone();
        Some(current)
    }
}

impl Game {
    /// C++ `CvUnit::canSpellAutomate` (CvUnit.cpp:3827-3915).
    /// Sephi: human-only automatic spell casting (COMMAND_AUTOMATE_SPELL). C++ non-const;
    /// changes nothing.
    pub fn unit_can_spell_automate(&self, u: UnitId, spell: SpellType) -> bool {
        let d = self.d;
        let info = &d.spells[spell];
        let unit = &self.units[u];
        let owner = unit.owner;

        if !self.players[owner].human {
            return false;
        }

        if !info.allow_auto {
            return false;
        }

        if self.players[owner].disable_spellcasting > 0 {
            return false;
        }

        if let Some(current_spell) = unit.auto_spell_cast {
            return spell == current_spell;
        }
        let mut valid = true;
        // C++ quirk: the promotion prereqs are tested without a NO_PROMOTION check (an
        // out-of-bounds read for -1); a missing prereq counts as not held here. (No FfH2 spell
        // has bAllowAuto, so this is never reached with the shipped XML.)
        if !info.promotion_prereq1.map_or(false, |p| unit.has_promotion[p]) {
            valid = false;
        }
        if !info.promotion_prereq2.map_or(false, |p| unit.has_promotion[p]) {
            valid = false;
        }

        if let Some(unit_prereq) = info.unit_prereq {
            if unit.unit_type != unit_prereq {
                valid = false;
            }
        }

        if let Some(unit_class_prereq) = info.unit_class_prereq {
            if self.unit_get_unit_class_type(u) != unit_class_prereq {
                valid = false;
            }
        }

        if let Some(civilization_prereq) = info.civilization_prereq {
            if self.unit_get_civilization_type(u) != civilization_prereq {
                valid = false;
            }
        }

        if let Some(building_class) = info.building_class_owned_prereq {
            if self.players[owner].building_class_count[building_class] == 0 {
                valid = false;
            }
        }

        if let Some(religion_prereq) = info.religion_prereq {
            if unit.religion != Some(religion_prereq) {
                valid = false;
            }
        }

        if let Some(state_religion_prereq) = info.state_religion_prereq {
            // C++ quirk: compares the unit's religion, not the owner's state religion (canCast
            // checks the state religion).
            if unit.religion != Some(state_religion_prereq) {
                valid = false;
            }
        }

        valid
    }

    /// C++ `CvUnit::canCast` (CvUnit.cpp:13675-14101).
    /// C++ non-const; changes nothing (the FfH `req*` Python requirement functions have no side
    /// effects). The spell's `PyRequirement` goes through `crate::ffh`.
    pub fn unit_can_cast(&self, u: UnitId, spell: SpellType, test_visible: bool) -> bool {
        let d = self.d;
        let info = &d.spells[spell];
        let unit = &self.units[u];
        let owner = unit.owner;
        // C++ `CvPlot* pPlot = plot();`, dereferenced (unchecked) only by the tests that need it.
        let unit_plot = self.unit_plot_opt(u);
        let plot = || unit_plot.expect("CvUnit::canCast: unit is not on the map");

        if self.unit_get_immobile_timer(u) > 0 {
            return false;
        }
        if !self.unit_is_human(u) {
            if !info.allow_ai {
                return false;
            }
        }
        if let Some(promotion) = info.promotion_prereq1 {
            if !unit.has_promotion[promotion] {
                return false;
            }
        }
        if let Some(promotion) = info.promotion_prereq2 {
            if !unit.has_promotion[promotion] {
                return false;
            }
        }
        if let Some(unit_class_prereq) = info.unit_class_prereq {
            if self.unit_get_unit_class_type(u) != unit_class_prereq {
                return false;
            }
        }
        if let Some(unit_prereq) = info.unit_prereq {
            if unit.unit_type != unit_prereq {
                return false;
            }
        }
        if let Some(unit_combat_prereq) = info.unit_combat_prereq {
            if self.unit_get_unit_combat_type(u) != Some(unit_combat_prereq) {
                return false;
            }
        }
        if let Some(building_prereq) = info.building_prereq {
            if !self.plot_is_city(plot(), false, None) {
                return false;
            }
            if let Some(city) = live_plot_city(self, plot()) {
                if self.city_num_building(city, building_prereq) == 0 {
                    return false;
                }
            }
        }
        if let Some(building_class) = info.building_class_owned_prereq {
            if self.players[owner].building_class_count[building_class] == 0 {
                return false;
            }
        }
        if let Some(civilization_prereq) = info.civilization_prereq {
            if self.unit_get_civilization_type(u) != civilization_prereq {
                return false;
            }
        }
        if let Some(corporation_prereq) = info.corporation_prereq {
            if !self.plot_is_city(plot(), false, None) {
                return false;
            }
            if let Some(city) = live_plot_city(self, plot()) {
                if !self.cities[city].has_corporation[corporation_prereq] {
                    return false;
                }
            }
        }
        if let Some(improvement_prereq) = info.improvement_prereq {
            if self.map.plots[plot()].improvement_type != Some(improvement_prereq) {
                return false;
            }
        }
        if let Some(religion_prereq) = info.religion_prereq {
            if unit.religion != Some(religion_prereq) {
                return false;
            }
        }
        if let Some(state_religion_prereq) = info.state_religion_prereq {
            if self.player_get_state_religion(owner) != Some(state_religion_prereq) {
                return false;
            }
        }
        if let Some(tech_prereq) = info.tech_prereq {
            if !self.team_is_has_tech(self.unit_get_team(u), Some(tech_prereq)) {
                return false;
            }
        }
        if let Some(convert_unit_type) = info.convert_unit_type {
            if unit.unit_type == convert_unit_type {
                return false;
            }
        }
        if info.global {
            if self.is_option(GameOptionType::GAMEOPTION_NO_WORLD_SPELLS) {
                return false;
            }
            if self.players[owner].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] {
                return false;
            }
        }
        if info.prereq_slave_trade {
            if !self.player_is_slave_trade(owner) {
                return false;
            }
        }
        if d.units[unit.unit_type].equipment_promotion.is_some() {
            // C++ compares `getUnitClassPrereq()` (-1 when unset) with the unit's class.
            if info.unit_class_prereq != Some(self.unit_get_unit_class_type(u)) {
                return false;
            }
        }
        if info.caster_min_level != 0 {
            if unit.level < info.caster_min_level {
                return false;
            }
        }
        if info.caster_must_be_alive {
            if !self.unit_is_alive(u) {
                return false;
            }
        }
        if info.caster_no_duration {
            if unit.duration != 0 {
                return false;
            }
        }
        if self.players[owner].disable_spellcasting > 0 {
            if !info.ability {
                return false;
            }
        }
        if let Some(promotion) = info.promotion_in_stack_prereq {
            if unit.has_promotion[promotion] {
                return false;
            }
            let mut valid = false;
            for &loop_unit in &self.map.plots[plot()].units {
                if self.units[loop_unit].has_promotion[promotion] {
                    if owner == self.units[loop_unit].owner {
                        valid = true;
                    }
                }
            }
            if !valid {
                return false;
            }
        }
        if let Some(unit_in_stack) = info.unit_in_stack_prereq {
            if unit.unit_type == unit_in_stack {
                return false;
            }
            let mut valid = false;
            for &loop_unit in &self.map.plots[plot()].units {
                let other = &self.units[loop_unit];
                if other.unit_type == unit_in_stack {
                    if owner == other.owner {
                        if !other.death_delay {
                            valid = true;
                        }
                    }
                }
            }
            if !valid {
                return false;
            }
        }
        if test_visible {
            if info.display_when_disabled {
                return true;
            }
        }
        if let Some(feature_prereq1) = info.feature_or_prereq1 {
            let feature = self.map.plots[plot()].feature_type;
            if feature != Some(feature_prereq1) {
                if info.feature_or_prereq2.is_none() || feature != info.feature_or_prereq2 {
                    return false;
                }
            }
        }
        if !info.ignore_has_casted {
            if unit.has_casted {
                return false;
            }
        }
        if info.adjacent_to_water_only {
            if !self.plot_is_adjacent_to_water(plot()) {
                return false;
            }
        }
        if info.in_borders_only {
            if self.map.plots[plot()].owner != Some(owner) {
                return false;
            }
        }
        if info.in_city_only {
            if !self.plot_is_city(plot(), false, None) {
                return false;
            }
        }
        if info.change_population != 0 {
            if !self.plot_is_city(plot(), false, None) {
                return false;
            }
            if let Some(city) = live_plot_city(self, plot()) {
                if self.cities[city].population <= -info.change_population {
                    return false;
                }
            }
        }
        let mut cost = info.cost;
        if cost != 0 {
            if info.convert_unit_type.is_some() {
                cost += (cost * self.players[owner].upgrade_cost_modifier) / 100;
            }
            if self.players[owner].gold < cost {
                return false;
            }
        }
        if info.remove_has_casted {
            if !unit.has_casted {
                return false;
            }
            if unit.duration > 0 {
                return false;
            }
        }
        if info.create_unit_type.is_some() {
            if !self.unit_can_create_unit(u, spell) {
                return false;
            }
        }
        if info.create_building_type.is_some() {
            if !self.unit_can_create_building(u, spell) {
                return false;
            }
        }
        if !info.py_requirement.is_empty() {
            // Python `canCast(pCaster, eSpell)` decides alone once the XML prereqs pass.
            return self.py_can_cast(u, spell);
        }
        if info.remove_has_casted {
            if unit.has_casted {
                return true;
            }
        }
        if info.add_promotion_type1.is_some() {
            if self.unit_can_add_promotion(u, spell) {
                return true;
            }
        }
        if info.remove_promotion_type1.is_some() {
            if self.unit_can_remove_promotion(u, spell) {
                return true;
            }
        }
        if info.convert_unit_type.is_some() {
            return true;
        }
        if info.create_feature_type.is_some() {
            if self.unit_can_create_feature(u, spell) {
                return true;
            }
        }
        if info.create_improvement_type.is_some() {
            if self.unit_can_create_improvement(u, spell) {
                return true;
            }
        }
        if info.spread_religion.is_some() {
            if self.unit_can_spread_religion(u, spell) {
                return true;
            }
        }
        if info.create_building_type.is_some() {
            return true;
        }
        if info.create_unit_type.is_some() {
            return true;
        }
        if info.damage != 0 {
            return true;
        }
        if info.dispel {
            if self.unit_can_dispel(u, spell) {
                return true;
            }
        }
        if info.immobile_turns > 0 {
            if self.unit_can_immobile(u, spell) {
                return true;
            }
        }
        if info.push {
            if self.unit_can_push(u, spell) {
                return true;
            }
        }
        if info.change_population > 0 {
            return true;
        }
        if !info.py_result.is_empty() {
            return true;
        }
        false
    }

    /// C++ `CvUnit::canCreateUnit` (CvUnit.cpp:14103-14140).
    pub fn unit_can_create_unit(&self, u: UnitId, spell: SpellType) -> bool {
        let d = self.d;
        let info = &d.spells[spell];
        let unit = &self.units[u];
        let owner = unit.owner;

        // to prevent summons summoning spinlocks
        if unit.duration > 0 {
            if info.create_unit_type == Some(unit.unit_type) {
                return false;
            }
        }
        // keeps invisible units from CtDing summoning on top of enemies
        if self.plot_is_visible_enemy_unit(self.unit_plot(u), owner) {
            return false;
        }
        if info.permanent_unit_create {
            let mut count = 0;
            for &loop_unit in &self.players[owner].units {
                let other = &self.units[loop_unit];
                if info.promotion_prereq1.map_or(true, |p| other.has_promotion[p])
                    && info.promotion_prereq2.map_or(true, |p| other.has_promotion[p])
                    && info.religion_prereq.map_or(true, |r| other.religion == Some(r))
                {
                    count += 1;
                }
            }
            // C++ reads `GC.getUnitInfo(NO_UNIT)` (out of bounds) for a spell that creates no
            // unit; canCast only asks for spells that do.
            if let Some(create_unit_type) = info.create_unit_type {
                let unit_class = d.units[create_unit_type].unit_class_type.expect("CvUnitInfo without a unit class");
                if count <= self.players[owner].unit_class_count[unit_class] {
                    return false;
                }
            }
        }
        true
    }

    /// C++ `CvUnit::canAddPromotion` (CvUnit.cpp:14142-14234).
    /// C++ non-const; changes nothing.
    pub fn unit_can_add_promotion(&self, u: UnitId, spell: SpellType) -> bool {
        let d = self.d;
        let info = &d.spells[spell];
        let promotions = [info.add_promotion_type1, info.add_promotion_type2, info.add_promotion_type3];
        if info.buff_caster_only {
            for promotion in promotions.into_iter().flatten() {
                if !self.units[u].has_promotion[promotion] {
                    return true;
                }
            }
            return false;
        }
        let (x, y) = self.map.xy(self.unit_plot(u));
        let range = info.range;
        for i in -range..=range {
            for j in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                for &loop_unit in &self.map.plots[loop_plot].units {
                    if !self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        for promotion in promotions.into_iter().flatten() {
                            if let Some(unit_combat) = self.unit_get_unit_combat_type(loop_unit) {
                                if d.promotions[promotion].unit_combat[unit_combat] {
                                    if !self.units[loop_unit].has_promotion[promotion] {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// C++ `CvUnit::canCreateBuilding` (CvUnit.cpp:14236-14247).
    pub fn unit_can_create_building(&self, u: UnitId, spell: SpellType) -> bool {
        let plot = self.unit_plot(u);
        if !self.plot_is_city(plot, false, None) {
            return false;
        }
        // C++ reads `getNumBuilding(NO_BUILDING)` (out of bounds) for a spell that creates no
        // building; canCast and cast only ask for spells that do.
        if let (Some(city), Some(building)) = (live_plot_city(self, plot), self.d.spells[spell].create_building_type) {
            if self.city_num_building(city, building) > 0 {
                return false;
            }
        }
        true
    }

    /// C++ `CvUnit::canCreateFeature` (CvUnit.cpp:14249-14271).
    pub fn unit_can_create_feature(&self, u: UnitId, spell: SpellType) -> bool {
        let d = self.d;
        let feature = d.spells[spell].create_feature_type;
        let plot = self.unit_plot(u);
        if self.plot_is_city(plot, false, None) {
            return false;
        }
        if !self.plot_can_have_feature(plot, feature) {
            return false;
        }
        if self.map.plots[plot].feature_type.is_some() {
            return false;
        }
        if self.map.plots[plot].improvement_type.is_some() {
            let civilization = self.unit_get_civilization_type(u);
            // C++ reads `isMaintainFeatures(NO_FEATURE)` (out of bounds) for a spell that creates
            // no feature; canCast and cast only ask for spells that do.
            if !feature.map_or(false, |f| d.civilizations[civilization].maintain_features[f]) {
                return false;
            }
        }
        true
    }

    /// C++ `CvUnit::canCreateImprovement` (CvUnit.cpp:14273-14288).
    pub fn unit_can_create_improvement(&self, u: UnitId, spell: SpellType) -> bool {
        let plot = self.unit_plot(u);
        if self.plot_is_city(plot, false, None) {
            return false;
        }
        // C++ passes `NO_IMPROVEMENT` to `canHaveImprovement` (out-of-bounds reads) for a spell
        // that creates no improvement; canCast and cast only ask for spells that do.
        let Some(improvement) = self.d.spells[spell].create_improvement_type else {
            return false;
        };
        if !self.plot_can_have_improvement(plot, improvement, None, false) {
            return false;
        }
        if self.map.plots[plot].improvement_type.is_some() {
            return false;
        }
        true
    }

    /// C++ `CvUnit::canDispel` (CvUnit.cpp:14290-14330).
    /// C++ non-const; changes nothing.
    pub fn unit_can_dispel(&self, u: UnitId, spell: SpellType) -> bool {
        let d = self.d;
        let range = d.spells[spell].range;
        let team = self.unit_get_team(u);
        let (x, y) = self.map.xy(self.unit_plot(u));
        for i in -range..=range {
            for j in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                for &loop_unit in &self.map.plots[loop_plot].units {
                    if !self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        for promotion in d.promotions.keys() {
                            if self.units[loop_unit].has_promotion[promotion] {
                                let promotion_info = &d.promotions[promotion];
                                if promotion_info.dispellable {
                                    if (promotion_info.ai_weight < 0 && self.unit_get_team(loop_unit) == team)
                                        || (promotion_info.ai_weight > 0 && self.unit_is_enemy(loop_unit, Some(team), None))
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// C++ `CvUnit::canImmobile` (CvUnit.cpp:14332-14359).
    /// C++ non-const; changes nothing.
    pub fn unit_can_immobile(&self, u: UnitId, spell: SpellType) -> bool {
        // C++ quirk: the caster's own plot counts here although `castImmobile` skips it.
        let range = self.d.spells[spell].range;
        let (x, y) = self.map.xy(self.unit_plot(u));
        for i in -range..=range {
            for j in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                for &loop_unit in &self.map.plots[loop_plot].units {
                    if !self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// C++ `CvUnit::canPush` (CvUnit.cpp:14361-14391).
    /// C++ non-const; changes nothing.
    pub fn unit_can_push(&self, u: UnitId, spell: SpellType) -> bool {
        // C++ quirk: the caster's own plot counts here although `castPush` skips it.
        let range = self.d.spells[spell].range;
        let (x, y) = self.map.xy(self.unit_plot(u));
        for i in -range..=range {
            for j in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                if !self.plot_is_city(loop_plot, false, None) {
                    for &loop_unit in &self.map.plots[loop_plot].units {
                        if !self.unit_is_immune_to_spell(loop_unit, u, spell) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// C++ `CvUnit::canRemovePromotion` (CvUnit.cpp:14393-14482).
    /// C++ non-const; changes nothing.
    pub fn unit_can_remove_promotion(&self, u: UnitId, spell: SpellType) -> bool {
        let d = self.d;
        let info = &d.spells[spell];
        let promotions = [info.remove_promotion_type1, info.remove_promotion_type2, info.remove_promotion_type3];
        let plot = self.unit_plot(u);
        if self.plot_is_visible_enemy_unit(plot, self.units[u].owner) {
            // C++ compares the promotion ints with the define (a NO_PROMOTION slot would match a
            // -1 define).
            let hidden_nationality = d.defines.hidden_nationality_promotion;
            for promotion in promotions {
                if opt_to_i32(promotion) == hidden_nationality {
                    return false;
                }
            }
        }
        if info.buff_caster_only {
            for promotion in promotions.into_iter().flatten() {
                if self.units[u].has_promotion[promotion] {
                    return true;
                }
            }
            return false;
        }
        let (x, y) = self.map.xy(plot);
        let range = info.range;
        for i in -range..=range {
            for j in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                for &loop_unit in &self.map.plots[loop_plot].units {
                    if !self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        for promotion in promotions.into_iter().flatten() {
                            if self.units[loop_unit].has_promotion[promotion] {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// C++ `CvUnit::canSpreadReligion` (CvUnit.cpp:14484-14501).
    pub fn unit_can_spread_religion(&self, u: UnitId, spell: SpellType) -> bool {
        let Some(religion) = self.d.spells[spell].spread_religion else {
            return false;
        };
        let Some(city) = live_plot_city(self, self.unit_plot(u)) else {
            return false;
        };
        if self.cities[city].has_religion[religion] {
            return false;
        }
        true
    }

    /// C++ `CvUnit::cast` (CvUnit.cpp:14503-14675).
    /// Runs the spell's `PyResult` through `crate::ffh` and the built-in effects below.
    pub fn unit_cast(&mut self, u: UnitId, spell: SpellType) {
        let d = self.d;
        let info = &d.spells[spell];

        if info.has_casted {
            self.units[u].has_casted = true;
        }
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                if d.promotions[promotion].removed_by_casting {
                    self.unit_set_has_promotion(u, promotion, false);
                    // Losing PROMOTION_HIDDEN next to an enemy withdraws the caster, or deletes it
                    // when no plot is free (C++ then reads the deleted unit).
                    if !self.units.contains(u) {
                        return;
                    }
                }
            }
        }
        if info.global {
            let owner = self.units[u].owner;
            self.players[owner].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] = true;
            // UI: TXT_KEY_MESSAGE_GLOBAL_SPELL to every alive civ player.
            let turn = self.game_turn();
            self.log.msg(turn, Some(owner), || format!("World spell: {}", d.text.name(&info.base.description)));
        }
        let miscast_chance = info.miscast_chance + d.units[self.units[u].unit_type].miscast_chance;
        if miscast_chance > 0 {
            if self.soren_rand_num(100, "Miscast") < miscast_chance {
                if !info.py_miscast.is_empty() {
                    self.py_miscast(u, spell);
                }
                // UI: TXT_KEY_MESSAGE_SPELL_MISCAST, selection buttons.
                let turn = self.game_turn();
                let owner = self.units.get(u).map(|unit| unit.owner);
                self.log.msg(turn, owner, || format!("Miscast: {}", d.text.name(&info.base.description)));
                return;
            }
        }
        // A delayed spell is cast again by `setImmobileTimer` when the timer runs out; that cast
        // repeats everything above (removed-by-casting promotions, a new "Miscast" roll).
        if info.delay > 0 {
            if self.units[u].delayed_spell.is_none() {
                self.unit_change_immobile_timer(u, info.delay);
                self.units[u].delayed_spell = Some(spell);
                // UI: selection buttons, cycle selection counter.
                return;
            }
            self.units[u].delayed_spell = None;
        }
        if info.create_unit_type.is_some() {
            let mut unit_num = info.create_unit_num;
            if self.unit_is_twincast(u) {
                unit_num *= 2;
            }
            for _ in 0..unit_num {
                if !self.units.contains(u) {
                    return;
                }
                self.unit_cast_create_unit(u, spell);
            }
        }
        if info.add_promotion_type1.is_some() {
            self.unit_cast_add_promotion(u, spell);
        }
        if !self.units.contains(u) {
            return;
        }
        if info.remove_promotion_type1.is_some() {
            self.unit_cast_remove_promotion(u, spell);
        }
        if !self.units.contains(u) {
            return;
        }
        if info.convert_unit_type.is_some() {
            // Kills the caster with delay: it stays on its plot until `doDelayedDeath`.
            self.unit_cast_convert_unit(u, spell);
        }
        if let Some(building) = info.create_building_type {
            if self.unit_can_create_building(u, spell) {
                if let Some(city) = live_plot_city(self, self.unit_plot(u)) {
                    // C++ `setNumRealBuilding(eBuilding, true)`.
                    self.city_set_num_real_building(city, building, 1);
                }
            }
        }
        if let Some(feature) = info.create_feature_type {
            if self.unit_can_create_feature(u, spell) {
                let plot = self.unit_plot(u);
                // C++ `setFeatureType(eFeature, -1)` (-1: a random art variety).
                self.plot_set_feature_type(plot, Some(feature));
            }
        }
        if let Some(improvement) = info.create_improvement_type {
            if self.unit_can_create_improvement(u, spell) {
                let plot = self.unit_plot(u);
                self.plot_set_improvement_type(plot, Some(improvement));
            }
        }
        if let Some(religion) = info.spread_religion {
            if self.unit_can_spread_religion(u, spell) {
                if let Some(city) = live_plot_city(self, self.unit_plot(u)) {
                    self.city_set_has_religion(city, religion, true, true, true);
                }
            }
        }
        if info.damage != 0 {
            self.unit_cast_damage(u, spell);
        }
        if info.immobile_turns != 0 {
            self.unit_cast_immobile(u, spell);
        }
        if info.push {
            self.unit_cast_push(u, spell);
        }
        if !self.units.contains(u) {
            return;
        }
        if info.remove_has_casted {
            if self.units[u].duration == 0 {
                self.units[u].has_casted = false;
            }
        }
        let mut cost = info.cost;
        if cost != 0 {
            let owner = self.units[u].owner;
            if info.convert_unit_type.is_some() {
                cost += (cost * self.players[owner].upgrade_cost_modifier) / 100;
            }
            // `changeGold(-1 * iCost)`
            self.players[owner].gold += -cost;
        }
        if info.change_population != 0 {
            // C++ dereferences `plot()->getPlotCity()` unchecked (canCast requires a city for
            // these spells); without a city nothing happens here.
            if let Some(city) = live_plot_city(self, self.unit_plot(u)) {
                self.city_change_population(city, info.change_population);
            }
        }
        if info.dispel {
            self.unit_cast_dispel(u, spell);
        }
        if !self.units.contains(u) {
            return;
        }
        // UI: when a human watches the plot, the spell effect (its angle comes from the async
        // rand), sound and message.
        if !info.py_result.is_empty() {
            self.py_cast(u, spell);
        }
        // UI: selection buttons.
        if info.sacrifice_caster {
            if !self.units.contains(u) {
                return;
            }
            self.unit_kill(u, true, None);
        }
    }

    /// C++ `CvUnit::castAddPromotion` (CvUnit.cpp:14677-14793).
    pub fn unit_cast_add_promotion(&mut self, u: UnitId, spell: SpellType) {
        let d = self.d;
        let info = &d.spells[spell];
        let promotions = [info.add_promotion_type1, info.add_promotion_type2, info.add_promotion_type3];
        if info.buff_caster_only {
            for promotion in promotions.into_iter().flatten() {
                if !self.units.contains(u) {
                    return;
                }
                self.unit_set_has_promotion(u, promotion, true);
            }
            return;
        }
        let range = info.range;
        let resistable = info.resistable;
        for i in -range..=range {
            for j in -range..=range {
                let Some((x, y)) = caster_xy(self, u) else {
                    return;
                };
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                let mut walk = PlotUnitWalk::new(self, loop_plot);
                while let Some(loop_unit) = walk.next_unit(self) {
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        continue;
                    }
                    if resistable && self.unit_is_resisted(loop_unit, u, spell) {
                        continue;
                    }
                    for promotion in promotions.into_iter().flatten() {
                        if !self.units.contains(loop_unit) {
                            break;
                        }
                        if let Some(unit_combat) = self.unit_get_unit_combat_type(loop_unit) {
                            if d.promotions[promotion].unit_combat[unit_combat] {
                                self.unit_set_has_promotion(loop_unit, promotion, true);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvUnit::castDamage` (CvUnit.cpp:14795-14838).
    pub fn unit_cast_damage(&mut self, u: UnitId, spell: SpellType) {
        let info = &self.d.spells[spell];
        let resistable = info.resistable;
        let damage = info.damage;
        let damage_limit = info.damage_limit;
        let damage_type = info.damage_type;
        let range = info.range;
        for i in -range..=range {
            for j in -range..=range {
                let Some((x, y)) = caster_xy(self, u) else {
                    return;
                };
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                // Not the caster's own plot.
                if self.map.xy(loop_plot) == (x, y) {
                    continue;
                }
                let mut walk = PlotUnitWalk::new(self, loop_plot);
                while let Some(loop_unit) = walk.next_unit(self) {
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        continue;
                    }
                    if resistable && self.unit_is_resisted(loop_unit, u, spell) {
                        continue;
                    }
                    let amount = (damage / 2) + self.soren_rand_num(damage, "doDamage");
                    self.unit_do_damage(loop_unit, amount, damage_limit, Some(u), damage_type, true);
                }
            }
        }
    }

    /// C++ `CvUnit::castDispel` (CvUnit.cpp:14840-14893).
    pub fn unit_cast_dispel(&mut self, u: UnitId, spell: SpellType) {
        let d = self.d;
        let info = &d.spells[spell];
        let resistable = info.resistable;
        let range = info.range;
        for i in -range..=range {
            for j in -range..=range {
                let Some((x, y)) = caster_xy(self, u) else {
                    return;
                };
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                let mut walk = PlotUnitWalk::new(self, loop_plot);
                while let Some(loop_unit) = walk.next_unit(self) {
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        continue;
                    }
                    let team = self.unit_get_team(u);
                    // Enemies lose their good dispellable promotions, everyone else the bad ones.
                    let enemy = self.unit_is_enemy(loop_unit, Some(team), None);
                    if enemy {
                        if resistable {
                            if self.unit_is_resisted(loop_unit, u, spell) {
                                continue;
                            }
                        }
                    }
                    for promotion in d.promotions.keys() {
                        let promotion_info = &d.promotions[promotion];
                        let dispel = if enemy { promotion_info.ai_weight > 0 } else { promotion_info.ai_weight < 0 };
                        if promotion_info.dispellable && dispel {
                            if !self.units.contains(loop_unit) {
                                break;
                            }
                            self.unit_set_has_promotion(loop_unit, promotion, false);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvUnit::castImmobile` (CvUnit.cpp:14895-14940).
    pub fn unit_cast_immobile(&mut self, u: UnitId, spell: SpellType) {
        let info = &self.d.spells[spell];
        let resistable = info.resistable;
        let immobile_turns = info.immobile_turns;
        let range = info.range;
        for i in -range..=range {
            for j in -range..=range {
                let Some((x, y)) = caster_xy(self, u) else {
                    return;
                };
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                // Not the caster's own plot.
                if self.map.xy(loop_plot) == (x, y) {
                    continue;
                }
                let mut walk = PlotUnitWalk::new(self, loop_plot);
                while let Some(loop_unit) = walk.next_unit(self) {
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.unit_is_immune_to_spell(loop_unit, u, spell) || self.unit_get_immobile_timer(loop_unit) != 0 {
                        continue;
                    }
                    if resistable && self.unit_is_resisted(loop_unit, u, spell) {
                        continue;
                    }
                    self.unit_change_immobile_timer(loop_unit, immobile_turns);
                    // UI: TXT_KEY_MESSAGE_SPELL_IMMOBILE to both owners.
                }
            }
        }
    }

    /// C++ `CvUnit::castPush` (CvUnit.cpp:14942-14998).
    pub fn unit_cast_push(&mut self, u: UnitId, spell: SpellType) {
        let info = &self.d.spells[spell];
        let resistable = info.resistable;
        let range = info.range;
        for i in -range..=range {
            for j in -range..=range {
                let Some((x, y)) = caster_xy(self, u) else {
                    return;
                };
                // (C++ also computes an unused `iPushY`.)
                let loop_plot = self.plot_xy(x, y, i, j);
                let push_plot = self.plot_xy(x, y, i * 2, j * 2);
                // C++ quirk: `pLoopPlot->isCity()` runs before the NULL check, so the C++ crashes
                // when the range leaves the map; here such a plot is skipped. (No FfH2 spell pushes.)
                let Some(loop_plot) = loop_plot else {
                    continue;
                };
                if self.plot_is_city(loop_plot, false, None) {
                    continue;
                }
                let Some(push_plot) = push_plot else {
                    continue;
                };
                // Not the caster's own plot.
                if self.map.xy(loop_plot) == (x, y) {
                    continue;
                }
                let (push_x, push_y) = self.map.xy(push_plot);
                let mut walk = PlotUnitWalk::new(self, loop_plot);
                while let Some(loop_unit) = walk.next_unit(self) {
                    if !self.units.contains(u) {
                        return;
                    }
                    if !self.unit_can_move_into(loop_unit, push_plot, false, false, false) {
                        continue;
                    }
                    if self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        continue;
                    }
                    if resistable && self.unit_is_resisted(loop_unit, u, spell) {
                        continue;
                    }
                    // C++ `setXY(.., false, true, true)`: alone (not with its group).
                    self.unit_set_xy(loop_unit, push_x, push_y, false);
                    // UI: TXT_KEY_MESSAGE_SPELL_PUSH to both owners.
                }
            }
        }
    }

    /// C++ `CvUnit::castRemovePromotion` (CvUnit.cpp:15000-15080).
    pub fn unit_cast_remove_promotion(&mut self, u: UnitId, spell: SpellType) {
        let info = &self.d.spells[spell];
        let promotions = [info.remove_promotion_type1, info.remove_promotion_type2, info.remove_promotion_type3];
        if info.buff_caster_only {
            for promotion in promotions.into_iter().flatten() {
                if !self.units.contains(u) {
                    return;
                }
                self.unit_set_has_promotion(u, promotion, false);
            }
            return;
        }
        let range = info.range;
        let resistable = info.resistable;
        for i in -range..=range {
            for j in -range..=range {
                let Some((x, y)) = caster_xy(self, u) else {
                    return;
                };
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                let mut walk = PlotUnitWalk::new(self, loop_plot);
                while let Some(loop_unit) = walk.next_unit(self) {
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.unit_is_immune_to_spell(loop_unit, u, spell) {
                        continue;
                    }
                    if resistable && self.unit_is_resisted(loop_unit, u, spell) {
                        continue;
                    }
                    for promotion in promotions.into_iter().flatten() {
                        // Losing invisibility can withdraw or delete the unit.
                        if !self.units.contains(loop_unit) {
                            break;
                        }
                        self.unit_set_has_promotion(loop_unit, promotion, false);
                    }
                }
            }
        }
    }

    /// C++ `CvUnit::castConvertUnit` (CvUnit.cpp:15082-15088).
    pub fn unit_cast_convert_unit(&mut self, u: UnitId, spell: SpellType) {
        // C++ `initUnit((UnitTypes)getConvertUnitType(), ..)`: only called for spells that convert.
        let Some(convert_unit_type) = self.d.spells[spell].convert_unit_type else {
            return;
        };
        let unit = &self.units[u];
        let (owner, x, y, unit_ai) = (unit.owner, unit.x, unit.y, unit.ai_type);
        let new_unit = self.player_init_unit(owner, convert_unit_type, x, y, unit_ai, None);
        self.unit_convert(new_unit, u);
        self.unit_change_immobile_timer(new_unit, 1);
    }

    /// C++ `CvUnit::castCreateUnit` (CvUnit.cpp:15090-15175).
    /// In AI "summon suicide mode" the summon is flagged (`ai_suicide_summon`, groupflag); the C++
    /// then runs its `AI_update` at once, which is AI and omitted here (`Controller` has no hook
    /// for it).
    pub fn unit_cast_create_unit(&mut self, u: UnitId, spell: SpellType) {
        let d = self.d;
        let info = &d.spells[spell];
        // C++ `initUnit((UnitTypes)getCreateUnitType(), ..)`: only called for spells that summon.
        let Some(create_unit_type) = info.create_unit_type else {
            return;
        };
        let unit = &self.units[u];
        let (owner, x, y) = (unit.owner, unit.x, unit.y);
        let summon = self.player_init_unit(owner, create_unit_type, x, y, Some(UnitAIType::UNITAI_ATTACK), None);
        self.units[summon].summoner = Some(u);
        if info.permanent_unit_create {
            self.unit_change_immobile_timer(summon, 2);
        } else {
            self.unit_change_duration(summon, 2);
            if opt_to_i32(self.unit_get_special_unit_type(summon)) != d.defines.specialunit_spell {
                let summon_duration = self.players[owner].summon_duration;
                self.unit_change_duration(summon, summon_duration);
            }
            if self.plot_get_team(self.unit_plot(u)) != Some(self.unit_get_team(u)) {
                // `changeNumOutsideUnits(-1)`: summons do not count as units outside the borders.
                self.players[owner].num_outside_units += -1;
            }
        }
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                if info.copy_casters_promotions {
                    let promotion_info = &d.promotions[promotion];
                    if !promotion_info.equipment && !promotion_info.race && promotion.i() != d.defines.great_commander_promotion {
                        self.unit_set_has_promotion(summon, promotion, true);
                    }
                }
                // BUGFfH (Denev 2009/10/08): summoning abilities that are not spells (e.g. Hire
                // Goblin) do not pass the summoner's perks on.
                else if !info.ability {
                    if let Some(perk) = d.promotions[promotion].promotion_summon_perk {
                        self.unit_set_has_promotion(summon, perk, true);
                    }
                }
            }
        }
        if let Some(promotion) = info.create_unit_promotion {
            self.unit_set_has_promotion(summon, promotion, true);
        }
        self.unit_do_turn(summon);
        if !self.units.contains(summon) || !self.units.contains(u) {
            return;
        }
        if !self.unit_is_human(u) {
            // BETTER AI (Better use of Summons) Sephi
            if self.players[owner].sum_sui_mode {
                if self.unit_can_move(summon) {
                    self.units[summon].ai_groupflag = GROUPFLAG_NONE;
                    self.units[summon].ai_suicide_summon = true;
                    // AI: `pUnit->AI_update()` (the summon acts at once).
                }
            } else if self.units[summon].unit_type.i() == d.defines.unit_fireball {
                if self.unit_bombard(summon) {
                    return;
                }
            }
            if self.units[summon].duration > 0 {
                if self.units[summon].moves > 0 {
                    let group = live_group(self, u);
                    self.unit_join_group(summon, group, true);
                }
            }
        }
    }

    /// C++ `CvUnit::isImmuneToSpell` (CvUnit.cpp:15197-15245).
    pub fn unit_is_immune_to_spell(&self, u: UnitId, caster: UnitId, spell: SpellType) -> bool {
        let info = &self.d.spells[spell];
        if self.unit_is_immune_to_magic(u) {
            if !info.ability {
                return true;
            }
        }
        let caster_team = self.unit_get_team(caster);
        if info.immune_team {
            if self.unit_get_team(u) == caster_team {
                return true;
            }
        }
        if info.immune_neutral {
            if self.unit_get_team(u) != caster_team {
                if !self.unit_is_enemy(u, Some(caster_team), None) {
                    return true;
                }
            }
        }
        if info.immune_enemy {
            if self.unit_is_enemy(u, Some(caster_team), None) {
                return true;
            }
        }
        if info.immune_flying {
            if self.unit_is_flying(u) {
                return true;
            }
        }
        if info.immune_not_alive {
            if !self.unit_is_alive(u) {
                return true;
            }
        }
        false
    }

    /// C++ `CvUnit::changeDuration` (CvUnit.cpp:15267-15270).
    pub fn unit_change_duration(&mut self, u: UnitId, change: i32) {
        // C++ `setDuration(getDuration() + iChange)`.
        self.units[u].duration += change;
    }

    /// C++ `CvUnit::isAlive` (CvUnit.cpp:15282-15285).
    pub fn unit_is_alive(&self, u: UnitId) -> bool {
        // `alive` counts "not alive" sources (mechanical units, undead promotions, ...).
        self.units[u].alive == 0
    }

    /// C++ `CvUnit::changeAlive` (CvUnit.cpp:15287-15293).
    pub fn unit_change_alive(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].alive += change;
        }
    }

    /// C++ `CvUnit::isAIControl` (CvUnit.cpp:15295-15298).
    pub fn unit_is_ai_control(&self, u: UnitId) -> bool {
        self.units[u].ai_control != 0
    }

    /// C++ `CvUnit::changeAIControl` (CvUnit.cpp:15300-15306).
    pub fn unit_change_ai_control(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].ai_control += change;
        }
    }

    /// C++ `CvUnit::isBoarding` (CvUnit.cpp:15308-15311).
    pub fn unit_is_boarding(&self, u: UnitId) -> bool {
        self.units[u].boarding != 0
    }

    /// C++ `CvUnit::changeBoarding` (CvUnit.cpp:15313-15319).
    pub fn unit_change_boarding(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].boarding += change;
        }
    }

    /// C++ `CvUnit::changeDefensiveStrikeChance` (CvUnit.cpp:15326-15332).
    pub fn unit_change_defensive_strike_chance(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].defensive_strike_chance += change;
        }
    }

    /// C++ `CvUnit::changeDefensiveStrikeDamage` (CvUnit.cpp:15339-15345).
    pub fn unit_change_defensive_strike_damage(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].defensive_strike_damage += change;
        }
    }

    /// C++ `CvUnit::isDoubleFortifyBonus` (CvUnit.cpp:15347-15350).
    pub fn unit_is_double_fortify_bonus(&self, u: UnitId) -> bool {
        self.units[u].double_fortify_bonus != 0
    }

    /// C++ `CvUnit::changeDoubleFortifyBonus` (CvUnit.cpp:15352-15358).
    pub fn unit_change_double_fortify_bonus(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].double_fortify_bonus += change;
        }
    }

    /// C++ `CvUnit::isFear` (CvUnit.cpp:15360-15363).
    pub fn unit_is_fear(&self, u: UnitId) -> bool {
        self.units[u].fear != 0
    }

    /// C++ `CvUnit::changeFear` (CvUnit.cpp:15365-15371).
    pub fn unit_change_fear(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].fear += change;
        }
    }

    /// C++ `CvUnit::isFlying` (CvUnit.cpp:15373-15376).
    pub fn unit_is_flying(&self, u: UnitId) -> bool {
        self.units[u].flying != 0
    }

    /// C++ `CvUnit::changeFlying` (CvUnit.cpp:15378-15384).
    pub fn unit_change_flying(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].flying += change;
        }
    }

    /// C++ `CvUnit::isHeld` (CvUnit.cpp:15386-15389).
    pub fn unit_is_held(&self, u: UnitId) -> bool {
        self.units[u].held != 0
    }

    /// C++ `CvUnit::changeHeld` (CvUnit.cpp:15391-15397).
    pub fn unit_change_held(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].held += change;
        }
    }

    /// C++ `CvUnit::isHiddenNationality` (CvUnit.cpp:15399-15409).
    pub fn unit_is_hidden_nationality(&self, u: UnitId) -> bool {
        // Cargo is only hidden if its transport is.
        if self.unit_is_cargo(u) {
            if let Some(transport) = self.units[u].transport_unit {
                if !self.unit_is_hidden_nationality(transport) {
                    return false;
                }
            }
        }
        self.units[u].hidden_nationality != 0
    }

    /// C++ `CvUnit::changeHiddenNationality` (CvUnit.cpp:15411-15427).
    /// Losing the last source re-applies blockades and leaves the group (`joinGroup(NULL, true)`).
    pub fn unit_change_hidden_nationality(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            if self.units[u].hidden_nationality + change == 0 {
                self.unit_update_plunder(u, -1, false);
                self.units[u].hidden_nationality += change;
                // C++ `joinGroup(NULL, true)`: bRemoveSelected (interface) with the default
                // bRejoin = true.
                self.unit_join_group(u, None, true);
                self.unit_update_plunder(u, 1, false);
            } else {
                self.units[u].hidden_nationality += change;
            }
        }
    }

    /// C++ `CvUnit::changeIgnoreBuildingDefense` (CvUnit.cpp:15429-15435).
    pub fn unit_change_ignore_building_defense(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].ignore_building_defense += change;
        }
    }

    /// C++ `CvUnit::isImmortal` (CvUnit.cpp:15437-15440).
    pub fn unit_is_immortal(&self, u: UnitId) -> bool {
        self.units[u].immortal != 0
    }

    /// C++ `CvUnit::changeImmortal` (CvUnit.cpp:15442-15452).
    pub fn unit_change_immortal(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            let unit = &mut self.units[u];
            unit.immortal += change;
            if unit.immortal < 0 {
                unit.immortal = 0;
            }
        }
    }

    /// C++ `CvUnit::isImmuneToCapture` (CvUnit.cpp:15454-15457).
    pub fn unit_is_immune_to_capture(&self, u: UnitId) -> bool {
        self.units[u].immune_to_capture != 0
    }

    /// C++ `CvUnit::changeImmuneToCapture` (CvUnit.cpp:15459-15465).
    pub fn unit_change_immune_to_capture(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_capture += change;
        }
    }

    /// C++ `CvUnit::isImmuneToDefensiveStrike` (CvUnit.cpp:15467-15470).
    pub fn unit_is_immune_to_defensive_strike(&self, u: UnitId) -> bool {
        self.units[u].immune_to_defensive_strike != 0
    }

    /// C++ `CvUnit::changeImmuneToDefensiveStrike` (CvUnit.cpp:15472-15478).
    pub fn unit_change_immune_to_defensive_strike(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_defensive_strike += change;
        }
    }

    /// C++ `CvUnit::isImmuneToFear` (CvUnit.cpp:15480-15487).
    pub fn unit_is_immune_to_fear(&self, u: UnitId) -> bool {
        // FfH: units that are not alive are immune to fear.
        if !self.unit_is_alive(u) {
            return true;
        }
        self.units[u].immune_to_fear != 0
    }

    /// C++ `CvUnit::changeImmuneToFear` (CvUnit.cpp:15489-15495).
    pub fn unit_change_immune_to_fear(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_fear += change;
        }
    }

    /// C++ `CvUnit::isImmuneToMagic` (CvUnit.cpp:15497-15500).
    pub fn unit_is_immune_to_magic(&self, u: UnitId) -> bool {
        self.units[u].immune_to_magic != 0
    }

    /// C++ `CvUnit::changeImmuneToMagic` (CvUnit.cpp:15502-15508).
    pub fn unit_change_immune_to_magic(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].immune_to_magic += change;
        }
    }

    /// C++ `CvUnit::isInvisibleFromPromotion` (CvUnit.cpp:15510-15513).
    pub fn unit_is_invisible_from_promotion(&self, u: UnitId) -> bool {
        self.units[u].invisible != 0
    }

    /// C++ `CvUnit::changeInvisibleFromPromotion` (CvUnit.cpp:15515-15528).
    /// Becoming visible next to enemies withdraws the unit (`withdrawlToNearestValidPlot(true)`).
    pub fn unit_change_invisible_from_promotion(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].invisible += change;
            if !self.unit_is_invisible_from_promotion(u) {
                // C++ dereferences `plot()` here.
                let plot = self.unit_plot(u);
                if self.plot_is_visible_enemy_unit_for_unit(plot, u) {
                    if self.unit_get_domain_type(u) != DomainType::DOMAIN_IMMOBILE {
                        // Kills the unit (without delay) when no plot around is free.
                        self.unit_withdrawl_to_nearest_valid_plot(u, true);
                    }
                }
            }
        }
    }

    /// C++ `CvUnit::isSeeInvisible` (CvUnit.cpp:15530-15533).
    pub fn unit_is_see_invisible(&self, u: UnitId) -> bool {
        self.units[u].see_invisible != 0
    }

    /// C++ `CvUnit::changeSeeInvisible` (CvUnit.cpp:15535-15541).
    pub fn unit_change_see_invisible(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].see_invisible += change;
        }
    }

    /// C++ `CvUnit::changeOnlyDefensive` (CvUnit.cpp:15543-15549).
    pub fn unit_change_only_defensive(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].only_defensive += change;
        }
    }

    /// C++ `CvUnit::isTargetWeakestUnit` (CvUnit.cpp:15551-15554).
    pub fn unit_is_target_weakest_unit(&self, u: UnitId) -> bool {
        self.units[u].target_weakest_unit != 0
    }

    /// C++ `CvUnit::changeTargetWeakestUnit` (CvUnit.cpp:15556-15562).
    pub fn unit_change_target_weakest_unit(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].target_weakest_unit += change;
        }
    }

    /// C++ `CvUnit::isTargetWeakestUnitCounter` (CvUnit.cpp:15564-15567).
    pub fn unit_is_target_weakest_unit_counter(&self, u: UnitId) -> bool {
        self.units[u].target_weakest_unit_counter != 0
    }

    /// C++ `CvUnit::changeTargetWeakestUnitCounter` (CvUnit.cpp:15569-15575).
    pub fn unit_change_target_weakest_unit_counter(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].target_weakest_unit_counter += change;
        }
    }

    /// C++ `CvUnit::isTwincast` (CvUnit.cpp:15577-15580).
    pub fn unit_is_twincast(&self, u: UnitId) -> bool {
        self.units[u].twincast != 0
    }

    /// C++ `CvUnit::changeTwincast` (CvUnit.cpp:15582-15588).
    pub fn unit_change_twincast(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].twincast += change;
        }
    }

    /// C++ `CvUnit::isWaterWalking` (CvUnit.cpp:15590-15593).
    pub fn unit_is_water_walking(&self, u: UnitId) -> bool {
        self.units[u].water_walking != 0
    }

    /// C++ `CvUnit::changeWaterWalking` (CvUnit.cpp:15595-15601).
    pub fn unit_change_water_walking(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].water_walking += change;
        }
    }

    /// C++ `CvUnit::changeBetterDefenderThanPercent` (CvUnit.cpp:15608-15614).
    pub fn unit_change_better_defender_than_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].better_defender_than_percent += change;
        }
    }

    /// C++ `CvUnit::changeCombatHealPercent` (CvUnit.cpp:15621-15627).
    pub fn unit_change_combat_heal_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].combat_heal_percent += change;
        }
    }

    /// C++ `CvUnit::calcCombatLimit` (CvUnit.cpp:15629-15648).
    /// `combat_limit` = min(unit info `iCombatLimit`, every non-zero promotion `iCombatLimit` the
    /// unit has).
    pub fn unit_calc_combat_limit(&mut self, u: UnitId) {
        let d = self.d;
        let mut best_value = d.units[self.units[u].unit_type].combat_limit;
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                let value = d.promotions[promotion].combat_limit;
                if value != 0 {
                    if value < best_value {
                        best_value = value;
                    }
                }
            }
        }
        self.units[u].combat_limit = best_value;
    }

    /// C++ `CvUnit::changeCombatPercentInBorders` (CvUnit.cpp:15655-15662).
    pub fn unit_change_combat_percent_in_borders(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].combat_percent_in_borders += change;
        }
    }

    /// C++ `CvUnit::changeCombatPercentGlobalCounter` (CvUnit.cpp:15669-15676).
    pub fn unit_change_combat_percent_global_counter(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].combat_percent_global_counter += change;
        }
    }

    /// C++ `CvUnit::changeFreePromotionPick` (CvUnit.cpp:15683-15689).
    pub fn unit_change_free_promotion_pick(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].free_promotion_pick += change;
        }
    }

    /// C++ `CvUnit::changeGoldFromCombat` (CvUnit.cpp:15696-15702).
    pub fn unit_change_gold_from_combat(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].gold_from_combat += change;
        }
    }

    /// C++ `CvUnit::mutate` (CvUnit.cpp:15709-15722).
    pub fn unit_mutate(&mut self, u: UnitId) {
        let d = self.d;
        let mutation_chance = d.defines.mutation_chance;
        for promotion in d.promotions.keys() {
            if d.promotions[promotion].mutation {
                if self.soren_rand_num(100, "Mutation") <= mutation_chance {
                    self.unit_set_has_promotion(u, promotion, true);
                }
            }
        }
    }

    /// C++ `CvUnit::getWithdrawlProbDefensive` (CvUnit.cpp:15724-15732).
    pub fn unit_get_withdrawl_prob_defensive(&self, u: UnitId) -> i32 {
        if self.unit_get_immobile_timer(u) > 0 {
            return 0;
        }
        let unit = &self.units[u];
        std::cmp::max(0, self.d.units[unit.unit_type].withdrawl_prob_defensive + unit.extra_withdrawal)
    }

    /// C++ `CvUnit::setRace` (CvUnit.cpp:15744-15751).
    /// Removes the old race promotion first (`setHasPromotion(old, false)`).
    pub fn unit_set_race(&mut self, u: UnitId, race: Option<PromotionType>) {
        if let Some(old_race) = self.units[u].race {
            self.unit_set_has_promotion(u, old_race, false);
        }
        self.units[u].race = race;
    }

    /// C++ `CvUnit::changeResist` (CvUnit.cpp:15773-15776).
    pub fn unit_change_resist(&mut self, u: UnitId, change: i32) {
        // C++ `setResist(getResist() + iChange)`.
        self.units[u].resist += change;
    }

    /// C++ `CvUnit::changeResistModify` (CvUnit.cpp:15788-15791).
    pub fn unit_change_resist_modify(&mut self, u: UnitId, change: i32) {
        // C++ `setResistModify(getResistModify() + iChange)`.
        self.units[u].resist_modify += change;
    }

    /// C++ `CvUnit::getSpellCasterXP` (CvUnit.cpp:15803-15810).
    /// 0 unless the unit info has `bFreeXP`; else the `spell_caster_xp` field.
    pub fn unit_get_spell_caster_xp(&self, u: UnitId) -> i32 {
        let unit = &self.units[u];
        if !self.d.units[unit.unit_type].free_xp {
            return 0;
        }
        unit.spell_caster_xp
    }

    /// C++ `CvUnit::changeSpellCasterXP` (CvUnit.cpp:15812-15818).
    pub fn unit_change_spell_caster_xp(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].spell_caster_xp += change;
        }
    }

    /// C++ `CvUnit::changeSpellDamageModify` (CvUnit.cpp:15825-15831).
    pub fn unit_change_spell_damage_modify(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].spell_damage_modify += change;
        }
    }

    /// C++ `CvUnit::changeWorkRateModify` (CvUnit.cpp:15848-15854).
    pub fn unit_change_work_rate_modify(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].work_rate_modify += change;
        }
    }

    /// C++ `CvUnit::isResisted` (CvUnit.cpp:15856-15866).
    /// C++ is `const` but draws the "is Resisted" sync rand, hence `&mut self`.
    pub fn unit_is_resisted(&mut self, u: UnitId, caster: UnitId, spell: SpellType) -> bool {
        if self.soren_rand_num(100, "is Resisted") <= self.unit_get_resist_chance(u, caster, spell) {
            // UI: TXT_KEY_MESSAGE_SPELL_RESISTED to both owners, AS3D_RESIST sound.
            return true;
        }
        false
    }

    /// C++ `CvUnit::getResistChance` (CvUnit.cpp:15868-15895).
    pub fn unit_get_resist_chance(&self, u: UnitId, caster: UnitId, spell: SpellType) -> i32 {
        let d = self.d;
        if self.unit_is_immune_to_spell(u, caster, spell) {
            return 100;
        }
        let unit = &self.units[u];
        let caster_unit = &self.units[caster];
        let mut resist = d.defines.spell_resist_chance_base;
        resist += unit.level * 5;
        resist += unit.resist;
        resist += caster_unit.resist_modify;
        resist += d.spells[spell].resist_modify;
        resist += self.players[unit.owner].resist_modify;
        resist += self.players[caster_unit.owner].resist_enemy_modify;
        let plot = self.unit_plot(u);
        if self.plot_is_city(plot, false, None) {
            resist += 10;
            if let Some(city) = live_plot_city(self, plot) {
                resist += self.cities[city].resist_magic;
            }
        }
        if resist >= d.defines.spell_resist_chance_max {
            resist = d.defines.spell_resist_chance_max;
        }
        if resist <= d.defines.spell_resist_chance_min {
            resist = d.defines.spell_resist_chance_min;
        }
        resist
    }

    /// C++ `CvUnit::changeBaseCombatStr` (CvUnit.cpp:15897-15900).
    pub fn unit_change_base_combat_str(&mut self, u: UnitId, change: i32) {
        // C++ `setBaseCombatStr(m_iBaseCombat + iChange)`.
        self.units[u].base_combat += change;
    }

    /// C++ `CvUnit::changeBaseCombatStrDefense` (CvUnit.cpp:15902-15905).
    pub fn unit_change_base_combat_str_defense(&mut self, u: UnitId, change: i32) {
        // C++ `setBaseCombatStrDefense(m_iBaseCombatDefense + iChange)`.
        self.units[u].base_combat_defense += change;
    }

    /// C++ `CvUnit::getExtraSpellMove` (CvUnit.cpp:16090-16104).
    /// Only used by AI code (`chooseSpell`, CvUnitAI); a pure query.
    pub fn unit_get_extra_spell_move(&self, u: UnitId) -> i32 {
        let d = self.d;
        let mut count = 0;
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                if let Some(perk) = d.promotions[promotion].promotion_summon_perk {
                    count += d.promotions[perk].moves_change;
                }
            }
        }
        count
    }

    /// C++ `CvUnit::doDamage` (CvUnit.cpp:16106-16204).
    /// C++ `iDmgType` -1 = no damage type.
    pub fn unit_do_damage(
        &mut self,
        u: UnitId,
        damage: i32,
        damage_limit: i32,
        attacker: Option<UnitId>,
        damage_type: Option<DamageType>,
        start_war: bool,
    ) {
        let d = self.d;
        // prevents some pyre zombie caused CtD's 05/23/2010
        if self.units[u].death_delay {
            return;
        }
        let mut damage = damage;

        let mut resist = self.unit_base_combat_str_defense(u) * 2;
        resist += self.units[u].level * 2;
        if let Some(city) = live_plot_city(self, self.unit_plot(u)) {
            resist += self.city_defense_modifier(city, false) / 4;
        }
        if let Some(damage_type) = damage_type {
            resist += self.unit_get_damage_type_resist(u, damage_type);
        }
        if let Some(attacker) = attacker {
            // (-1 = no damage type counts as not physical.)
            if damage_type != Some(DamageType::DAMAGE_PHYSICAL) {
                damage += self.units[attacker].spell_damage_modify;
            }
        }
        if resist < 100 {
            // Binary `+`: the left draw first.
            damage = self.soren_rand_num(damage, "Damage") + self.soren_rand_num(damage, "Damage");
            damage = damage * (100 - resist) / 100;

            if damage + self.units[u].damage > damage_limit {
                damage = damage_limit - self.units[u].damage;
            }
            if damage > 0 {
                // UI: TXT_KEY_MESSAGE_KILLED_BY / TXT_KEY_MESSAGE_DAMAGED_BY to the owner (and the
                // attacker's owner).
                let max_hit_points = d.defines.max_hit_points;
                if let Some(attacker) = attacker {
                    let attacker_owner = self.units[attacker].owner;
                    // C++ quirk: `changeDamage` already kills a dead unit (`kill(true, ..)`), so
                    // the dead unit is killed twice and `unitKilled` is reported twice.
                    self.unit_change_damage(u, damage, Some(attacker_owner));
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.units[u].damage >= max_hit_points {
                        self.unit_kill(u, true, Some(attacker_owner));
                    }
                    if !self.units.contains(u) || !self.units.contains(attacker) {
                        return;
                    }
                    if start_war {
                        if !self.unit_is_hidden_nationality(attacker) && !self.unit_is_hidden_nationality(u) {
                            // GAMECHANGE (Sephi): area spells only start a war on the victim's own
                            // territory (was: on any team difference).
                            let team = self.unit_get_team(u);
                            let attacker_team = self.unit_get_team(attacker);
                            if self.plot_get_team(self.unit_plot(u)) == Some(team) {
                                if !self.teams[team].vassal[attacker_team] && !self.teams[attacker_team].vassal[team] {
                                    if team != attacker_team {
                                        if !self.teams[attacker_team].permanent_war_peace[team] {
                                            self.team_declare_war(attacker_team, team, false, Some(WarPlanType::WARPLAN_TOTAL));
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // C++ quirk: killed twice, as above.
                    self.unit_change_damage(u, damage, None);
                    if !self.units.contains(u) {
                        return;
                    }
                    if self.units[u].damage >= max_hit_points {
                        self.unit_kill(u, true, None);
                    }
                }
            }
        }
    }

    /// C++ `CvUnit::doEscape` (CvUnit.cpp:16261-16267).
    pub fn unit_do_escape(&mut self, u: UnitId) {
        let owner = self.units[u].owner;
        if let Some(capital) = self.player_get_capital_city(owner) {
            let (x, y) = (self.cities[capital].x, self.cities[capital].y);
            // C++ `setXY(.., false, true, true)`: alone (not with its group).
            self.unit_set_xy(u, x, y, false);
        }
    }

    /// C++ `CvUnit::doImmortalRebirth` (CvUnit.cpp:16269-16304).
    /// Ends with `pushMission(MISSION_HEAL)` for AI units (Better AI bookkeeping in rules code).
    pub fn unit_do_immortal_rebirth(&mut self, u: UnitId) {
        let d = self.d;
        // C++ `joinGroup(NULL)` (default bRejoin = true).
        self.unit_join_group(u, None, true);
        self.unit_set_damage(u, 75, None);
        let mut from_promotion = false;
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                if d.promotions[promotion].immortal {
                    self.unit_set_has_promotion(u, promotion, false);
                    from_promotion = true;
                    break;
                }
            }
        }
        if !from_promotion {
            self.unit_change_immortal(u, -1);
        }
        self.unit_do_escape(u);
        // BETTER AI (Immortal Units heal) Sephi
        if !self.unit_is_human(u) {
            // C++ `getGroup()->pushMission(MISSION_HEAL)` with the default arguments.
            if let Some(group) = live_group(self, u) {
                self.group_push_mission(group, MissionType::MISSION_HEAL, -1, -1, 0, false, false, None, None, None);
            }
        }
    }

    /// C++ `CvUnit::setWeapons` (CvUnit.cpp:16562-16617).
    pub fn unit_set_weapons(&mut self, u: UnitId) {
        let d = self.d;
        let tier1 = d.defines.weapon_promotion_tier1;
        if tier1 == -1 {
            return;
        }
        let plot = self.unit_plot(u);
        if !self.plot_is_city(plot, false, None) {
            return;
        }
        let Some(city) = live_plot_city(self, plot) else {
            return;
        };
        if self.cities[city].owner != self.units[u].owner {
            return;
        }
        // C++ would index out of bounds with a -1 tier define; such a promotion is never held or
        // given and such a bonus never present here.
        let city_has_bonus = |g: &Game, define: i32| opt_from_i32::<BonusType>(define).map_or(false, |b| g.city_has_bonus(city, b));
        let set_promotion = |g: &mut Game, define: i32, value: bool| {
            if let Some(promotion) = opt_from_i32::<PromotionType>(define) {
                g.unit_set_has_promotion(u, promotion, value);
            }
        };
        let tier2 = d.defines.weapon_promotion_tier2;
        let tier3 = d.defines.weapon_promotion_tier3;
        let weapon_tier = d.units[self.units[u].unit_type].weapon_tier;
        if !has_promotion_define(self, u, tier3) {
            if city_has_bonus(self, d.defines.weapon_req_bonus_tier3) && weapon_tier >= 3 {
                set_promotion(self, tier3, true);
                // UI: TXT_KEY_MESSAGE_WEAPONS_MITHRIL.
                set_promotion(self, tier2, false);
                set_promotion(self, tier1, false);
            } else if !has_promotion_define(self, u, tier2) {
                if city_has_bonus(self, d.defines.weapon_req_bonus_tier2) && weapon_tier >= 2 {
                    set_promotion(self, tier2, true);
                    // UI: TXT_KEY_MESSAGE_WEAPONS_IRON.
                    set_promotion(self, tier1, false);
                } else if !has_promotion_define(self, u, tier1) {
                    if city_has_bonus(self, d.defines.weapon_req_bonus_tier1) && weapon_tier >= 1 {
                        // UI: TXT_KEY_MESSAGE_WEAPONS_BRONZE.
                        set_promotion(self, tier1, true);
                    }
                }
            }
        }
    }

    /// C++ `CvUnit::changeBonusAffinity` (CvUnit.cpp:16619-16626).
    pub fn unit_change_bonus_affinity(&mut self, u: UnitId, bonus: BonusType, change: i32) {
        if change != 0 {
            self.units[u].bonus_affinity[bonus] += change;
        }
        // C++ updates even when nothing changed.
        self.unit_update_bonus_affinity(u, bonus);
    }

    /// C++ `CvUnit::updateBonusAffinity` (CvUnit.cpp:16633-16648).
    pub fn unit_update_bonus_affinity(&mut self, u: UnitId, bonus: BonusType) {
        let owner = self.units[u].owner;
        let new_amount = self.player_get_num_available_bonuses(owner, bonus) * self.units[u].bonus_affinity[bonus];
        let unit = &mut self.units[u];
        let old_amount = unit.bonus_affinity_amount[bonus];
        match self.d.bonuses[bonus].damage_type {
            None => {
                unit.base_combat += new_amount - old_amount;
                unit.base_combat_defense += new_amount - old_amount;
            }
            Some(damage_type) => {
                unit.damage_type_combat[damage_type] += new_amount - old_amount;
                unit.total_damage_type_combat += new_amount - old_amount;
            }
        }
        unit.bonus_affinity_amount[bonus] = new_amount;
    }

    /// C++ `CvUnit::changeDamageTypeCombat` (CvUnit.cpp:16650-16657).
    pub fn unit_change_damage_type_combat(&mut self, u: UnitId, damage_type: DamageType, change: i32) {
        if change != 0 {
            let unit = &mut self.units[u];
            unit.damage_type_combat[damage_type] += change;
            unit.total_damage_type_combat += change;
        }
    }

    /// C++ `CvUnit::getDamageTypeResist` (CvUnit.cpp:16669-16681).
    pub fn unit_get_damage_type_resist(&self, u: UnitId, damage_type: DamageType) -> i32 {
        // Clamped to [-100, 100].
        let i = self.units[u].damage_type_resist[damage_type];
        if i <= -100 {
            return -100;
        }
        if i >= 100 {
            return 100;
        }
        i
    }

    /// C++ `CvUnit::changeDamageTypeResist` (CvUnit.cpp:16683-16689).
    pub fn unit_change_damage_type_resist(&mut self, u: UnitId, damage_type: DamageType, change: i32) {
        if change != 0 {
            self.units[u].damage_type_resist[damage_type] += change;
        }
    }

    /// C++ `CvUnit::countUnitsWithinRange` (CvUnit.cpp:16691-16726).
    /// C++ non-const; changes nothing.
    pub fn unit_count_units_within_range(&self, u: UnitId, range: i32, enemy: bool, neutral: bool, team: bool) -> i32 {
        let our_team = self.unit_get_team(u);
        let (x, y) = self.map.xy(self.unit_plot(u));
        let mut count = 0;
        for i in -range..=range {
            for j in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };
                for &loop_unit in &self.map.plots[loop_plot].units {
                    let loop_team = self.unit_get_team(loop_unit);
                    if team && loop_team == our_team {
                        count += 1;
                    }
                    if enemy && self.at_war(loop_team, our_team) {
                        count += 1;
                    }
                    if neutral && loop_team != our_team && !self.at_war(loop_team, our_team) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// C++ `CvUnit::betray` (CvUnit.cpp:16765-16781).
    pub fn unit_betray(&mut self, u: UnitId, player: PlayerId) {
        if self.units[u].owner == player {
            return;
        }
        if let Some(new_plot) = self.unit_get_open_plot(u) {
            let (x, y) = self.map.xy(new_plot);
            let (unit_type, unit_ai) = (self.units[u].unit_type, self.units[u].ai_type);
            let new_unit = self.player_init_unit(player, unit_type, x, y, unit_ai, None);
            self.unit_convert(new_unit, u);
            if self.units[new_unit].duration > 0 {
                self.units[new_unit].duration = 0;
            }
        }
    }

    /// C++ `CvUnit::updateTerraformer` (CvUnit.cpp:16783-16798).
    /// `terraformer` = can cast any spell with `bAllowAutomateTerrain`.
    pub fn unit_update_terraformer(&mut self, u: UnitId) {
        let d = self.d;
        self.units[u].terraformer = false;
        for spell in d.spells.keys() {
            if d.spells[spell].allow_automate_terrain {
                if self.unit_can_cast(u, spell, false) {
                    self.units[u].terraformer = true;
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    const W: i32 = 6;
    const H: i32 = 4;

    /// A game on a 6x4 map (one land area) with real `Player` objects (not `player_init`ed).
    fn test_game() -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        g.map = Map::new(d, &MapInitData { grid_w: W, grid_h: H, ..Default::default() }, num_players, num_teams);
        let area = g.map_add_area(false);
        for p in g.map.plot_ids() {
            g.map.plots[p].area = Some(area);
        }
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        g
    }

    /// C++ `CvPlayer::addUnit` + `reset`, placed on the plot's unit list directly (no `init` /
    /// `setXY`).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        let plot = g.map.plot_idx(x, y).unwrap();
        g.map.plots[plot].units.push(u);
        u
    }

    #[test]
    fn plot_unit_walk_follows_the_live_list() {
        let mut g = test_game();
        let plot = g.map.plot_idx(1, 1).unwrap();
        let [a, b, c, d, e, f] = [1, 2, 3, 4, 5, 6].map(UnitId);
        g.map.plots[plot].units = vec![a, b, c, d];

        let mut walk = PlotUnitWalk::new(&g, plot);
        assert_eq!(walk.next_unit(&g), Some(a));
        // The current unit leaves: the walk goes on with the node fetched before.
        g.map.plots[plot].units.retain(|&x| x != a);
        assert_eq!(walk.next_unit(&g), Some(b));
        // A unit inserted before the fetched node is not visited, one appended later is; a unit
        // that leaves before its turn is skipped.
        g.map.plots[plot].units = vec![b, e, c, d, f];
        g.map.plots[plot].units.retain(|&x| x != d);
        assert_eq!(walk.next_unit(&g), Some(c));
        assert_eq!(walk.next_unit(&g), Some(f));
        assert_eq!(walk.next_unit(&g), None);

        // The fetched node itself leaves (C++: a deleted node): resume after it.
        g.map.plots[plot].units = vec![a, b, c, d];
        let mut walk = PlotUnitWalk::new(&g, plot);
        assert_eq!(walk.next_unit(&g), Some(a));
        g.map.plots[plot].units.retain(|&x| x != b);
        assert_eq!(walk.next_unit(&g), Some(c));
        g.map.plots[plot].units.retain(|&x| x != d);
        assert_eq!(walk.next_unit(&g), None);
    }

    #[test]
    fn resist_chance_sums_and_clamps() {
        let mut g = test_game();
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let caster = add_unit(&mut g, p0, "UNIT_ADEPT", 1, 1);
        let target = add_unit(&mut g, p1, "UNIT_WARRIOR", 2, 1);
        // SPELL_RAGE: only units that are not alive are immune.
        let rage = d.t::<SpellType>("SPELL_RAGE");
        assert_eq!(d.spells[rage].resist_modify, 0);

        // Base 20 + level 1 * 5.
        assert_eq!(g.unit_get_resist_chance(target, caster, rage), 25);
        g.units[target].level = 3;
        g.units[target].resist = 10;
        g.units[caster].resist_modify = -5;
        g.players[p1].resist_modify = 7;
        g.players[p0].resist_enemy_modify = -2;
        assert_eq!(g.unit_get_resist_chance(target, caster, rage), 20 + 15 + 10 - 5 + 7 - 2);

        g.units[target].resist = 500;
        assert_eq!(g.unit_get_resist_chance(target, caster, rage), d.defines.spell_resist_chance_max);
        g.units[target].resist = -500;
        assert_eq!(g.unit_get_resist_chance(target, caster, rage), d.defines.spell_resist_chance_min);

        // Immune units always resist.
        g.unit_change_alive(target, 1);
        assert!(g.unit_is_immune_to_spell(target, caster, rage));
        assert_eq!(g.unit_get_resist_chance(target, caster, rage), 100);

        // `isResisted` draws one "is Resisted" number and compares with `<=`.
        let mut rand = g.soren_rand.clone();
        let roll = rand.get_i(100, "is Resisted");
        g.unit_change_alive(target, -1);
        g.units[target].resist = 0;
        let chance = g.unit_get_resist_chance(target, caster, rage);
        assert_eq!(g.unit_is_resisted(target, caster, rage), roll <= chance);
        assert_eq!(g.soren_rand.seed(), rand.seed());
    }

    #[test]
    fn count_units_within_range_by_relation() {
        let mut g = test_game();
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let u = add_unit(&mut g, p0, "UNIT_WARRIOR", 2, 1);
        add_unit(&mut g, p0, "UNIT_WARRIOR", 2, 1);
        add_unit(&mut g, p1, "UNIT_WARRIOR", 3, 2);
        add_unit(&mut g, p1, "UNIT_WARRIOR", 5, 3);

        // The unit counts itself among its team.
        assert_eq!(g.unit_count_units_within_range(u, 1, false, false, true), 2);
        assert_eq!(g.unit_count_units_within_range(u, 1, false, true, false), 1);
        assert_eq!(g.unit_count_units_within_range(u, 1, true, false, false), 0);
        assert_eq!(g.unit_count_units_within_range(u, 0, true, true, true), 2);

        let (t0, t1) = (g.players[p0].team, g.players[p1].team);
        g.teams[t0].at_war[t1] = true;
        g.teams[t1].at_war[t0] = true;
        assert_eq!(g.unit_count_units_within_range(u, 1, true, true, false), 1);
        assert_eq!(g.unit_count_units_within_range(u, 3, true, false, false), 2);
        assert_eq!(g.unit_count_units_within_range(u, 3, true, true, true), 4);
    }

    #[test]
    fn promotion_derived_values() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_ADEPT", 1, 1);
        let promotion = |name: &str| d.t::<PromotionType>(name);

        // calcCombatLimit: the smallest non-zero limit among the unit and its promotions.
        assert_eq!(g.units[u].combat_limit, 100);
        g.units[u].has_promotion[promotion("PROMOTION_ILLUSION")] = true;
        g.unit_calc_combat_limit(u);
        assert_eq!(g.units[u].combat_limit, 90);
        g.units[u].has_promotion[promotion("PROMOTION_ILLUSION")] = false;
        g.unit_calc_combat_limit(u);
        assert_eq!(g.units[u].combat_limit, 100);

        // getExtraSpellMove: the moves of the summon perks (Extension -> Mobility).
        assert_eq!(g.unit_get_extra_spell_move(u), 0);
        g.units[u].has_promotion[promotion("PROMOTION_EXTENSION1")] = true;
        g.units[u].has_promotion[promotion("PROMOTION_EXTENSION2")] = true;
        assert_eq!(g.unit_get_extra_spell_move(u), 2);

        // getSpellCasterXP: only for bFreeXP units.
        g.unit_change_spell_caster_xp(u, 12);
        assert_eq!(g.unit_get_spell_caster_xp(u), 12);
        let warrior = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        g.unit_change_spell_caster_xp(warrior, 12);
        assert_eq!(g.unit_get_spell_caster_xp(warrior), 0);

        // getWithdrawlProbDefensive: never negative, 0 while immobile.
        g.units[warrior].extra_withdrawal = 15;
        let base = d.units[g.units[warrior].unit_type].withdrawl_prob_defensive;
        assert_eq!(g.unit_get_withdrawl_prob_defensive(warrior), (base + 15).max(0));
        g.units[warrior].extra_withdrawal = -1000;
        assert_eq!(g.unit_get_withdrawl_prob_defensive(warrior), 0);
        g.units[warrior].extra_withdrawal = 15;
        g.units[warrior].immobile_timer = 1;
        assert_eq!(g.unit_get_withdrawl_prob_defensive(warrior), 0);
    }
}
