//! CvUnit lifecycle: `init`, `convert`, `kill`, `doTurn`, selection-group membership, position
//! (`setXY`, `plot`), damage, ownership, names and random events; plus the few `CvUnitAI`
//! bookkeeping functions that rules code depends on (`AI_init`, `AI_setUnitAIType`, ...).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported. AI-only parts are omitted with `// AI:` notes; call sites
//! whose callee signature does not fit the C++ are marked `// SIGNATURE:`.

use crate::defs::*;
use crate::game::Game;
use crate::group::SelectionGroup;
use crate::ids::*;
use crate::unit::{Unit, UnitResetInfo};
use crate::utils::{is_world_unit_class, range};
use ffh_data::*;

/// C++ `GROUPFLAG_DEFENSE_NEW` (CvDefines.h, Sephi's BETTER AI group flags; read by `joinGroup`).
const GROUPFLAG_DEFENSE_NEW: i32 = 7;
/// C++ `GROUPFLAG_CONQUEST` (CvDefines.h; read by `joinGroup`).
const GROUPFLAG_CONQUEST: i32 = 10;

/// C++ `(UnitClassTypes)m_pUnitInfo->getUnitClassType()`, used unchecked as an array index by
/// the C++ (every unit info names a class).
fn info_unit_class(info: &UnitInfo) -> UnitClassType {
    info.unit_class_type.expect("CvUnitInfo without a unit class")
}

/// C++ `isHasPromotion((PromotionTypes)GC.getDefineINT(..))` for a promotion define; C++ would
/// read out of bounds for a -1 define, here such a promotion is never held.
fn has_promotion_define(game: &Game, u: UnitId, define: i32) -> bool {
    match opt_from_i32::<PromotionType>(define) {
        Some(promotion) => game.units[u].has_promotion[promotion],
        None => false,
    }
}

impl Game {
    /// C++ `CvUnit::init` (CvUnit.cpp:106-308).
    /// C++ `init(iID, eUnit, eUnitAI, eOwner, iX, iY, eFacingDirection)`: the slot `u` is allocated by the
    /// `CvPlayer::initUnit` port; this first re-runs `reset` (`self.units[u] = Unit::new(..)`), then
    /// `setXY`, names, counters, free promotions, `AI_init` (-> `unit_ai_init`, which draws the
    /// "AI Unit Birthmark" sync rand) and the FfH additions. `eFacingDirection` (graphics) is dropped.
    pub fn unit_init(&mut self, u: UnitId, unit_type: UnitType, unit_ai: UnitAIType, owner: PlayerId, x: i32, y: i32) {
        let d = self.d;
        let info = &d.units[unit_type];

        //--------------------------------
        // Init saved data
        self.units[u] = Unit::new(u, unit_type, owner, d, &UnitResetInfo::from_info(info));
        // `m_eFacingDirection` is graphics.

        //--------------------------------
        // Init pre-setup() data
        self.unit_set_xy(u, x, y, false);

        // `setupGraphical()`, `plot()->updateCenterUnit()` and `plot()->setFlagDirty(true)` are
        // graphics.

        let unit_name = self.unit_created_count[unit_type];
        let num_names = info.unit_names.len() as i32;
        if unit_name < num_names {
            let offset = self.soren_rand_num(num_names, "Unit name selection");

            for i in 0..num_names {
                let index = (i + offset) % num_names;
                let name = d.text.name(&info.unit_names[index as usize]).to_string();
                if !self.is_great_person_born(&name) {
                    self.unit_set_name(u, name.clone());
                    self.great_people_born.push(name);
                    break;
                }
            }
        }

        self.units[u].game_turn_created = self.game_turn();

        self.unit_created_count[unit_type] += 1;

        let unit_class = info_unit_class(info);
        self.unit_class_created_count[unit_class] += 1;
        let team = self.unit_get_team(u);
        self.teams[team].unit_class_count[unit_class] += 1;
        self.players[owner].unit_class_count[unit_class] += 1;

        self.players[owner].extra_unit_cost += info.extra_cost;

        if info.nuke_range != -1 {
            self.players[owner].num_nuke_units += 1;
        }

        if info.military_support {
            self.players[owner].num_military_units += 1;
        }

        self.players[owner].assets += info.asset_value;

        self.players[owner].power += info.power_value;

        for promotion in d.promotions.keys() {
            if info.free_promotions[promotion] {
                self.unit_set_has_promotion(u, promotion, true);
            }
        }

        for t in d.traits.keys() {
            if self.players[owner].traits[t] {
                for promotion in d.promotions.keys() {
                    if d.traits[t].free_promotion[promotion] {
                        if let Some(unit_combat) = self.unit_get_unit_combat_type(u) {
                            if d.traits[t].free_promotion_unit_combat[unit_combat] {
                                self.unit_set_has_promotion(u, promotion, true);
                            }
                        }
                    }
                }
            }
        }

        if let Some(unit_combat) = self.unit_get_unit_combat_type(u) {
            for promotion in d.promotions.keys() {
                if self.player_is_free_promotion_unit_combat(owner, unit_combat, promotion) {
                    self.unit_set_has_promotion(u, promotion, true);
                }
            }
        }

        // C++ `if (NO_UNITCLASS != getUnitClassType())`: always true (every unit info has a class).
        let class_type = self.unit_get_unit_class_type(u);
        for promotion in d.promotions.keys() {
            if self.player_is_free_promotion_unit_class(owner, class_type, promotion) {
                self.unit_set_has_promotion(u, promotion, true);
            }
        }

        if self.unit_get_domain_type(u) == DomainType::DOMAIN_LAND {
            if self.unit_base_combat_str(u) > 0 {
                if self.best_land_unit.is_none() || self.unit_base_combat_str(u) > self.best_land_unit_combat() {
                    // `setBestLandUnit` (its only other effect is an interface dirty bit).
                    self.best_land_unit = Some(unit_type);
                }
            }
        }

        // UI: the active player's GameData dirty bit.

        if is_world_unit_class(d, unit_class)
            // FfH: Added by Kael 11/05/2007
            && self.unit_class_created_count[unit_class] == 1
        {
            // UI: TXT_KEY_MISC_SOMEONE_CREATED_UNIT / TXT_KEY_MISC_UNKNOWN_CREATED_UNIT to every
            // alive player (by has-met) and the replay message.
            let turn = self.game_turn();
            self.log.msg(turn, Some(owner), || format!("{} has been created", d.text.name(&info.base.description)));
        }

        self.unit_ai_init(u, unit_ai);

        // FfH Units: Added by Kael 04/18/2008
        if info.free_promotion_pick > 0 {
            self.unit_change_free_promotion_pick(u, info.free_promotion_pick);
            self.unit_set_promotion_ready(u, true);
        }
        self.change_global_counter(info.modify_global_counter);
        self.units[u].religion = info.religion_type;
        for bonus in d.bonuses.keys() {
            self.unit_change_bonus_affinity(u, bonus, info.bonus_affinity[bonus]);
        }

        if info.mech_unit {
            self.unit_change_alive(u, 1);
        }
        let civilization = self.unit_get_civilization_type(u);
        if let Some(default_race) = d.civilizations[civilization].default_race {
            if self.units[u].race.is_none() {
                if !is_world_unit_class(d, self.unit_get_unit_class_type(u))
                    && !self.unit_is_animal(u)
                    && self.unit_is_alive(u)
                    && self.unit_get_domain_type(u) == DomainType::DOMAIN_LAND
                {
                    self.unit_set_has_promotion(u, default_race, true);
                }
            }
        }
        // FfH: End Add

        self.ev_unit_created(u);
    }

    /// C++ `CvUnit::convert` (CvUnit.cpp:570-719).
    /// `from` is C++ `pUnit` (the unit being replaced; it is killed with delay at the end).
    pub fn unit_convert(&mut self, u: UnitId, from: UnitId) {
        let d = self.d;
        let info = &d.units[self.units[u].unit_type];
        // C++ `CvPlot* pPlot = plot();` (only dereferenced for the hidden nationality check).
        let plot = self.unit_plot_opt(u);

        // FfH: Modified by Kael 08/21/2008 (replaces the plain copy of every promotion)
        if let Some(race) = self.units[u].race {
            if !info.free_promotions[race] {
                self.unit_set_has_promotion(u, race, false);
            } else {
                self.unit_set_has_promotion(from, race, true);
            }
        }
        if let Some(hidden_nationality) = opt_from_i32::<PromotionType>(d.defines.hidden_nationality_promotion) {
            if self.units[from].has_promotion[hidden_nationality] {
                let plot = plot.expect("CvUnit::convert: unit is not on the map");
                for loop_unit in self.map.plots[plot].units.clone() {
                    if !self.units.contains(loop_unit) {
                        continue;
                    }
                    if self.unit_get_team(loop_unit) != self.unit_get_team(u) {
                        self.unit_set_has_promotion(from, hidden_nationality, false);
                    }
                }
            }
        }
        for promotion in d.promotions.keys() {
            if self.units[from].has_promotion[promotion] {
                if promotion.i() == d.defines.mutated_promotion {
                    // Copies the flag without applying the promotion's effects (C++ writes
                    // `m_pabHasPromotion` directly).
                    self.units[u].has_promotion[promotion] = self.units[from].has_promotion[promotion];
                } else {
                    self.unit_set_has_promotion(u, promotion, true);
                    if d.promotions[promotion].equipment {
                        self.unit_set_has_promotion(from, promotion, false);
                    }
                }
                if d.promotions[promotion].validate {
                    let valid_combat = match self.unit_get_unit_combat_type(u) {
                        Some(unit_combat) => d.promotions[promotion].unit_combat[unit_combat],
                        // C++ quirk: `getUnitCombat(NO_UNITCOMBAT)` reads out of bounds; here a
                        // unit without a unit combat never validates the promotion.
                        None => false,
                    };
                    if !valid_combat {
                        self.unit_set_has_promotion(u, promotion, false);
                    }
                }
            }
        }
        let tier1 = d.defines.weapon_promotion_tier1;
        let tier2 = d.defines.weapon_promotion_tier2;
        let tier3 = d.defines.weapon_promotion_tier3;
        if let Some(promotion) = opt_from_i32::<PromotionType>(tier1) {
            if self.units[u].has_promotion[promotion] {
                if info.weapon_tier < 1 || has_promotion_define(self, u, tier2) || has_promotion_define(self, u, tier3) {
                    self.unit_set_has_promotion(u, promotion, false);
                }
            }
        }
        if let Some(promotion) = opt_from_i32::<PromotionType>(tier2) {
            if self.units[u].has_promotion[promotion] {
                if info.weapon_tier < 2 || has_promotion_define(self, u, tier3) {
                    self.unit_set_has_promotion(u, promotion, false);
                }
            }
        }
        if let Some(promotion) = opt_from_i32::<PromotionType>(tier3) {
            if self.units[u].has_promotion[promotion] {
                if info.weapon_tier < 3 {
                    self.unit_set_has_promotion(u, promotion, false);
                }
            }
        }
        if info.free_promotion_pick > 0 && self.units[u].game_turn_created == self.game_turn() {
            self.unit_set_promotion_ready(u, true);
        }
        self.units[u].duration = self.units[from].duration;
        if self.units[from].religion.is_some() && self.units[u].religion.is_none() {
            self.units[u].religion = self.units[from].religion;
        }
        if self.unit_is_immortal(from) {
            self.unit_change_immortal(from, -1);
        }
        if self.units[from].has_casted {
            self.units[u].has_casted = true;
        }
        if self.units[from].scenario_counter != -1 {
            self.units[u].scenario_counter = self.units[from].scenario_counter;
        }
        // FfH: End Modify

        self.units[u].game_turn_created = self.units[from].game_turn_created;
        let damage = self.units[from].damage;
        self.unit_set_damage(u, damage, None);
        // `setMoves`: the rest of it is interface updates.
        self.units[u].moves = self.units[from].moves;

        let level = self.units[from].level;
        self.unit_set_level(u, level);
        let old_modifier = std::cmp::max(1, 100 + self.players[self.units[from].owner].level_experience_modifier);
        let our_modifier = std::cmp::max(1, 100 + self.players[self.units[u].owner].level_experience_modifier);
        let experience = std::cmp::max(0, (self.units[from].experience * our_modifier) / old_modifier);
        self.unit_set_experience(u, experience, -1);

        let name = self.units[from].name.clone();
        self.unit_set_name(u, name);
        // `setLeaderUnitType` (C++ also reloads the entity: graphics).
        self.units[u].leader_unit_type = self.units[from].leader_unit_type;

        // BETTER AI (New UNITAI) Sephi
        self.units[u].ai_groupflag = self.units[from].ai_groupflag;
        // C++ `AI_setUnitAIType(pUnit->AI_getUnitAIType())` (never NO_UNITAI for an initialized unit).
        if let Some(unit_ai) = self.units[from].ai_type {
            self.unit_ai_set_unit_ai_type(u, unit_ai);
        }

        // FfH: Added by Kael 10/03/2008
        if !is_world_unit_class(d, info_unit_class(info)) && is_world_unit_class(d, self.unit_get_unit_class_type(from)) {
            let name = self.unit_get_name(from);
            self.unit_set_name(u, name);
        }

        if let Some(transport) = self.unit_transport(from) {
            self.unit_set_transport_unit(from, None);
            self.unit_set_transport_unit(u, Some(transport));
        }

        for cargo in self.unit_get_cargo_units(from) {
            self.unit_set_transport_unit(cargo, Some(u));
        }

        self.unit_kill(from, true, None);
    }

    /// C++ `CvUnit::kill` (CvUnit.cpp:722-1002).
    pub fn unit_kill(&mut self, u: UnitId, delay: bool, player: Option<PlayerId>) {
        let d = self.d;

        // C++ dereferences `plot()` (FAssert non-NULL).
        let plot = self.unit_plot(u);

        // Cargo dies with its transport (C++ copies the plot's unit list first).
        let old_units = self.map.plots[plot].units.clone();
        for loop_unit in old_units {
            // C++ `::getUnit(IDInfo)` is NULL for units killed meanwhile.
            if !self.units.contains(loop_unit) {
                continue;
            }
            if self.units[loop_unit].transport_unit == Some(u) {
                if self.plot_is_valid_domain_for_location(plot, loop_unit) {
                    self.unit_set_capturing_player(loop_unit, None);
                }

                self.unit_kill(loop_unit, false, player);
            }
        }

        if let Some(attacker) = player {
            // FfH: Modified by Kael 02/05/2009
            if !self.unit_is_immortal(u) {
                self.ev_unit_killed(u, Some(attacker));
            }

            // UI: TXT_KEY_MISC_GENERAL_KILLED to every alive player when a great general led the unit.
        }

        if delay {
            // `startDelayedDeath()`
            self.units[u].death_delay = true;
            return;
        }

        let unit_type = self.units[u].unit_type;
        let info = &d.units[unit_type];
        let owner = self.units[u].owner;

        // FfH: Added by Kael 07/23/2008
        if self.unit_is_immortal(u) {
            if self.player_get_capital_city(owner).is_some() {
                self.units[u].death_delay = false;
                self.unit_do_immortal_rebirth(u);
                return;
            }
        }
        self.change_global_counter(-info.modify_global_counter);
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                self.change_global_counter(-d.promotions[promotion].modify_global_counter);
                if d.promotions[promotion].equipment {
                    for equipment_unit in d.units.keys() {
                        if d.units[equipment_unit].equipment_promotion == Some(promotion) {
                            let (x, y) = (self.units[u].x, self.units[u].y);
                            let unit_ai = self.units[u].ai_type;
                            self.player_init_unit(owner, equipment_unit, x, y, unit_ai, None);
                            self.unit_set_has_promotion(u, promotion, false);
                        }
                    }
                }
            }
        }
        // UI: TXT_KEY_MISC_SOMEONE_KILLED_UNIT to other humans when the last world unit dies.
        for loop_unit in self.players[owner].units.clone() {
            if self.units[loop_unit].summoner == Some(u) {
                self.units[loop_unit].summoner = None;
            }
        }
        // FfH: End Add

        if self.units[u].made_attack && self.unit_nuke_range(u) != -1 {
            if let Some(target) = self.unit_get_attack_plot(u) {
                let nuke_range = self.unit_nuke_range(u);
                self.plot_nuke_explosion(target, nuke_range, Some(u));
                self.unit_set_attack_plot(u, None, false);
            }
        }

        self.unit_finish_moves(u);

        // UI: selection list / camera updates, `RemoveUnitFromBattle`.

        if self.unit_transport(u).is_some() {
            self.unit_set_transport_unit(u, None);
        }

        self.unit_set_recon_plot(u, None);
        self.unit_set_blockading(u, false);

        let unit_class = info_unit_class(info);
        let team = self.unit_get_team(u);
        self.teams[team].unit_class_count[unit_class] -= 1;
        self.players[owner].unit_class_count[unit_class] -= 1;

        self.players[owner].extra_unit_cost -= info.extra_cost;

        if info.nuke_range != -1 {
            self.players[owner].num_nuke_units -= 1;
        }

        if info.military_support {
            self.players[owner].num_military_units -= 1;
        }

        self.players[owner].assets -= info.asset_value;

        self.players[owner].power -= info.power_value;

        // C++ `AI_changeNumAIUnits(AI_getUnitAIType(), -1)` (never NO_UNITAI after `init`).
        if let Some(unit_ai) = self.units[u].ai_type {
            self.players[owner].ai_num_ai_units[unit_ai] -= 1;
        }

        let capturing_player = self.units[u].capturing_player;

        // FfH: Modified by Kael 09/01/2007 (the capture unit follows the dying unit's civ)
        let mut capture_unit_type = match capturing_player {
            Some(_) => {
                let civilization = self.player_get_civilization_type(owner).expect("unit owner without a civilization");
                self.unit_get_capture_unit_type(u, civilization)
            }
            None => None,
        };
        if info.unit_capture_class_type == Some(self.unit_get_unit_class_type(u)) {
            capture_unit_type = Some(unit_type);
        }
        let race = self.units[u].race;
        // FfH: End Modify

        self.unit_set_xy(u, INVALID_PLOT_COORD, INVALID_PLOT_COORD, true);

        self.unit_join_group(u, None, false);

        self.ev_unit_lost(u);

        self.player_delete_unit(owner, u);

        // FfH: Modified by Kael 01/19/2008 (barbarians may capture too)
        if let (Some(capturer), Some(capture_unit)) = (capturing_player, capture_unit_type) {
            let keep = self.players[capturer].human
                || self.controller(capturer).keep_captured_unit(self, capturer, capture_unit, plot)
                || d.defines.ai_can_disband_units == 0;
            if keep {
                let (px, py) = self.map.xy(plot);
                let captured = self.player_init_unit(capturer, capture_unit, px, py, None, None);

                // FfH: Added by Kael 08/18/2008
                if let Some(captured_race) = self.units[captured].race {
                    self.unit_set_has_promotion(captured, captured_race, false);
                }
                if let Some(race) = race {
                    self.unit_set_has_promotion(captured, race, true);
                }
                // FfH: End Add

                // UI: TXT_KEY_MISC_YOU_CAPTURED_UNIT message and the MISSION_CAPTURED animation.
                let turn = self.game_turn();
                self.log.msg(turn, Some(capturer), || format!("Captured {}", d.text.name(&d.units[capture_unit].base.description)));

                self.unit_finish_moves(captured);

                // AI: for non-human capturers C++ disbands the captured unit (`kill(false)`) when
                // it stands outside a city in danger (`AI_getPlotDanger(pPlot) &&
                // AI_CAN_DISBAND_UNITS && canScrap()`); the controller has no hook for that
                // decision, so the unit is kept.
            }
        }
    }

    /// C++ `CvUnit::doTurn` (CvUnit.cpp:1011-1269).
    pub fn unit_do_turn(&mut self, u: UnitId) {
        let d = self.d;
        let unit_type = self.units[u].unit_type;
        let info = &d.units[unit_type];

        // FfH Spell System: Added by Kael 07/23/2007
        let plot = self.unit_plot(u);
        if self.unit_has_moved(u) {
            if self.unit_is_always_heal(u) || self.unit_is_barbarian(u) {
                self.unit_do_heal(u);
            }
        } else {
            if self.unit_is_hurt(u) {
                self.unit_do_heal(u);
            }

            if !self.unit_is_cargo(u) {
                self.unit_change_fortify_turns(u, 1);
            }
        }
        if info.abandon {
            if !self.unit_is_barbarian(u) {
                let owner = self.units[u].owner;
                let mut valid = true;
                if let Some(prereq_civic) = info.prereq_civic {
                    valid = false;
                    for i in 0..d.defines.max_civic_options.max(0) as usize {
                        if self.players[owner].civics.get(CivicOptionType::from_usize(i)) == Some(&Some(prereq_civic)) {
                            valid = true;
                        }
                    }
                    if self.player_is_anarchy(owner) {
                        valid = true;
                    }
                }
                if valid {
                    if let Some(state_religion) = info.state_religion {
                        valid = false;
                        if self.player_get_state_religion(owner) == Some(state_religion) {
                            valid = true;
                        }
                    }
                }
                if !valid {
                    // UI: TXT_KEY_MESSAGE_UNIT_ABANDON.
                    let turn = self.game_turn();
                    self.log.msg(turn, Some(owner), || format!("{} abandoned", d.text.name(&info.base.description)));
                    self.unit_kill(u, true, None);
                    self.unit_created_count[unit_type] -= 1;
                    let unit_class = info_unit_class(info);
                    self.unit_class_created_count[unit_class] -= 1;
                    // C++ quirk: the team/player unit class counts and the extra unit cost are
                    // also removed by the delayed `kill(false)`, so abandoning takes them off twice.
                    let team = self.unit_get_team(u);
                    self.teams[team].unit_class_count[unit_class] -= 1;
                    self.players[owner].unit_class_count[unit_class] -= 1;
                    self.players[owner].extra_unit_cost += info.extra_cost * -1;
                }
            }
        }
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                let promotion_info = &d.promotions[promotion];
                if promotion_info.free_xp_per_turn != 0 {
                    if self.units[u].experience < d.defines.free_xp_max {
                        self.unit_change_experience(u, promotion_info.free_xp_per_turn, -1, false, false, false);
                    }
                }
                if let Some(random_apply) = promotion_info.promotion_random_apply {
                    if !self.units[u].has_promotion[random_apply] {
                        if self.soren_rand_num(100, "Promotion Random Apply") <= 3 {
                            self.unit_set_has_promotion(u, random_apply, true);
                        }
                    }
                }
                if promotion_info.betrayal_chance != 0 {
                    if !self.unit_is_immune_to_capture(u)
                        && !self.unit_is_barbarian(u)
                        && !self.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS)
                    {
                        if self.soren_rand_num(100, "Betrayal Chance") <= promotion_info.betrayal_chance {
                            let barbarian = self.barbarian_player();
                            self.unit_betray(u, barbarian);
                        }
                    }
                }
                if !promotion_info.py_per_turn.is_empty() {
                    self.py_promotion_effect(u, promotion);
                    // C++ keeps using `this`; a Python effect that deleted the unit would be a
                    // use-after-free there.
                    if !self.units.contains(u) {
                        return;
                    }
                }
                if promotion_info.expire_chance != 0 {
                    if self.soren_rand_num(100, "Promotion Expire") <= promotion_info.expire_chance {
                        self.unit_set_has_promotion(u, promotion, false);
                    }
                }
                if !self.unit_is_hurt(u) {
                    if promotion_info.removed_when_healed {
                        self.unit_set_has_promotion(u, promotion, false);
                    }
                }
            }
        }

        self.units[u].has_casted = false;

        if self.unit_get_spell_caster_xp(u) > 0 {
            if self.soren_rand_num(100, "SpellCasterXP") < self.unit_get_spell_caster_xp(u) - self.units[u].experience {
                self.unit_change_experience(u, 1, -1, false, false, false);
            }
        }
        if self.units[u].duration > 0 {
            self.unit_change_duration(u, -1);
            if self.units[u].duration == 0 {
                if self.unit_is_immortal(u) {
                    self.unit_change_immortal(u, -1);
                }
                self.unit_kill(u, true, None);
            }
        }
        if self.plot_is_city(plot, false, None) {
            if info.weapon_tier > 0 {
                self.unit_set_weapons(u);
            }
            if self.unit_is_barbarian(u) {
                if info.auto_raze {
                    if self.map.plots[plot].owner == Some(self.units[u].owner) {
                        if let Some(city) = self.map.plots[plot].plot_city {
                            self.city_kill(city, true);
                            // C++ keeps using `this` (a unit bumped to death by the razing would be
                            // a use-after-free there).
                            if !self.units.contains(u) {
                                return;
                            }
                        }
                    }
                }
            }
        }
        if info.immortal {
            if !self.unit_is_immortal(u) {
                self.unit_change_immortal(u, 1);
            }
        }
        // FfH: End Add

        self.unit_test_promotion_ready(u);

        if self.units[u].blockading {
            self.unit_collect_blockade_gold(u);
        }

        // FfH 02/03/2009: spy interception and feature turn damage are commented out in C++
        // (performance); healing moved to the start of the function.

        self.unit_change_immobile_timer(u, -1);

        self.units[u].made_attack = false;
        self.units[u].made_interception = false;

        self.unit_set_recon_plot(u, None);

        // `setMoves(0)`: the rest of it is interface updates.
        self.units[u].moves = 0;

        // AI: BETTER AI `AI_chooseGroupflag()` for non-human, non-barbarian units whose group flag
        // is GROUPFLAG_NONE (AI bookkeeping; omitted).
    }

    /// C++ `CvUnit::isInGroup` (CvUnit.cpp:10724-10727).
    pub fn unit_is_in_group(&self, u: UnitId) -> bool {
        self.units[u].group.is_some()
    }

    /// C++ `CvUnit::isGroupHead` (CvUnit.cpp:10730-10733).
    pub fn unit_is_group_head(&self, u: UnitId) -> bool {
        // C++ dereferences `getGroup()` (crashes for a unit without a group).
        let group = self.unit_group(u).expect("CvUnit::isGroupHead: unit has no group");
        self.group_get_head_unit(group) == Some(u)
    }

    /// C++ `CvUnit::canJoinGroup` (CvUnit.cpp:10742-10811).
    /// See [`Game::unit_can_join_group_opt`] for C++ calls with a NULL plot.
    pub fn unit_can_join_group(&self, u: UnitId, plot: PlotIdx, group: GroupId) -> bool {
        self.unit_can_join_group_opt(u, Some(plot), group)
    }

    /// C++ `CvUnit::canJoinGroup(pPlot, pSelectionGroup)` with `pPlot` as an `Option`: C++ passes
    /// `plot()`, which is NULL while `init` -> `setXY` puts a new unit (not yet on the map) into
    /// its first group (`joinGroup` and `CvSelectionGroup::addUnit` both check).
    pub fn unit_can_join_group_opt(&self, u: UnitId, plot: Option<PlotIdx>, group: GroupId) -> bool {
        let g = &self.groups[group];

        // do not allow someone to join a group that is about to be split apart
        // this prevents a case of a never-ending turn
        if g.ai_force_separate {
            return false;
        }

        // C++ also handles `pSelectionGroup->getOwnerINLINE() == NO_PLAYER` (compare with the head
        // unit's owner): ownerless groups are the UI selection list, which is not ported.
        if g.owner != self.units[u].owner {
            return false;
        }

        if !g.units.is_empty() {
            if !self.group_at_plot(group, plot) {
                return false;
            }

            if self.group_get_domain_type(group) != Some(self.unit_get_domain_type(u)) {
                return false;
            }

            // FfH: Added by Kael 06/20/2010
            if self.unit_is_ai_control(u) {
                return false;
            }
            if self.unit_is_hidden_nationality(u) {
                return false;
            }
            if let Some(head) = self.group_get_head_unit(group) {
                if self.unit_is_hidden_nationality(head) {
                    return false;
                }
                if self.unit_is_ai_control(head) {
                    return false;
                }
            }
            // FfH: End Add
        }

        true
    }

    /// C++ `CvUnit::joinGroup` (CvUnit.cpp:10814-10944).
    /// C++ `bRemoveSelected` (interface selection list) is dropped; C++ default `bRejoin` is true.
    pub fn unit_join_group(&mut self, u: UnitId, group: Option<GroupId>, rejoin: bool) {
        let owner = self.units[u].owner;
        let old_group = self.unit_group(u);

        if group != old_group || old_group.is_none() {
            let plot = self.unit_plot_opt(u);

            let new_group = match group {
                Some(g) => Some(g),
                None => {
                    if rejoin {
                        // `GET_PLAYER(owner).addSelectionGroup()` + `init(getID(), owner)`.
                        let id = self.groups.next_id();
                        let g = self.player_add_selection_group(owner, SelectionGroup::new(id, owner));
                        self.group_init(g, owner);
                        Some(g)
                    } else {
                        None
                    }
                }
            };

            let can_join = match new_group {
                None => true,
                Some(g) => self.unit_can_join_group_opt(u, plot, g),
            };
            if can_join {
                if let Some(old) = old_group {
                    let mut was_head = false;
                    if !self.unit_is_human(u) {
                        if self.groups[old].units.len() > 1 {
                            if self.group_get_head_unit(old) == Some(u) {
                                was_head = true;
                            }
                        }
                    }

                    self.group_remove_unit(old, u);

                    // if we were the head, if the head unitAI changed, then force the group to separate (non-humans)

                    // BETTER AI (Stop some groups from forceseperating) Sephi: tested on the
                    // requested group (`pSelectionGroup`), not the new one.
                    let mut valid = true;
                    if let Some(head) = group.and_then(|g| self.group_get_head_unit(g)) {
                        if self.units[u].ai_groupflag == GROUPFLAG_CONQUEST && self.units[head].ai_groupflag == GROUPFLAG_CONQUEST {
                            valid = false;
                        }
                    }

                    if let Some(head) = group.and_then(|g| self.group_get_head_unit(g)) {
                        if self.units[u].ai_groupflag == GROUPFLAG_DEFENSE_NEW && self.units[head].ai_groupflag == GROUPFLAG_DEFENSE_NEW {
                            valid = false;
                        }
                    }

                    if was_head && valid {
                        // FAssert(pOldSelectionGroup->getHeadUnit() != NULL): the group had another unit.
                        if let Some(head) = self.group_get_head_unit(old) {
                            if self.units[head].ai_type != self.units[u].ai_type {
                                // `AI_makeForceSeparate()`
                                self.groups[old].ai_force_separate = true;
                            }
                        }
                    }
                }

                let joined = match new_group {
                    Some(g) => self.group_add_unit(g, u, false),
                    None => false,
                };
                self.units[u].group = if joined { new_group } else { None };

                if let Some(g) = self.unit_group(u) {
                    if self.groups[g].units.len() > 1 {
                        self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
                    } else {
                        self.player_update_group_cycle(owner, u);
                    }
                }

                // UI: plot flag and plot list dirty bits.
            }

            // UI: `bRemoveSelected` (interface selection list).
        }
    }

    /// C++ `CvUnit::setXY` (CvUnit.cpp:10995-11579).
    /// C++ `bUpdate`, `bShow`, `bCheckPlotVisible` only drive graphics (plot center unit, flags,
    /// entity movement) and are dropped. `x`/`y` may be `INVALID_PLOT_COORD` (unit leaves the map).
    pub fn unit_set_xy(&mut self, u: UnitId, x: i32, y: i32, group: bool) {
        let d = self.d;
        let info = &d.units[self.units[u].unit_type];
        // Owner and team never change while the unit moves.
        let owner = self.units[u].owner;
        let team = self.unit_get_team(u);

        // C++ out-of-sync debug logging is not ported.

        // `NO_ACTIVITY` (None) when the unit has no group.
        let old_activity_type = self.unit_group(u).map(|g| self.groups[g].activity_type);

        self.unit_set_blockading(u, false);

        if !group || self.unit_is_cargo(u) {
            self.unit_join_group(u, None, true);
            // bShow = false (graphics)
        }

        let new_plot = self.map.plot_xy(x, y);

        if let Some(np) = new_plot {
            if let Some(transport) = self.unit_transport(u) {
                if !self.unit_at_plot(transport, np) {
                    self.unit_set_transport_unit(u, None);
                }
            }

            if self.unit_can_fight(u) {
                let old_units = self.map.plots[np].units.clone();

                for loop_unit in old_units {
                    // C++ `::getUnit(IDInfo)` is NULL for units killed meanwhile.
                    if !self.units.contains(loop_unit) {
                        continue;
                    }
                    let loop_team = self.unit_get_team(loop_unit);
                    if self.unit_is_enemy(u, Some(loop_team), Some(np)) || self.unit_is_enemy(loop_unit, Some(team), None) {
                        if !self.unit_can_coexist_with_enemy_unit(loop_unit, Some(team)) {
                            let loop_info = &d.units[self.units[loop_unit].unit_type];
                            if loop_info.unit_capture_class_type.is_none() && self.unit_can_defend(loop_unit, Some(np)) {
                                // FfH: Modified by Kael 04/25/2010 (was `jumpToNearestValidPlot()`)
                                if self.unit_plot_opt(loop_unit).is_some() {
                                    if !self.unit_is_invisible_in_plot(u, loop_team, Some(np), true) {
                                        self.unit_withdrawl_to_nearest_valid_plot(loop_unit, true); // can kill unit
                                    }
                                }
                            } else {
                                // FfH Hidden Nationality: Modified by Kael 08/27/2007
                                if !self.unit_is_hidden_nationality(u) && !self.unit_is_hidden_nationality(loop_unit) {
                                    self.team_change_war_weariness_at_plot(loop_team, team, np, d.defines.ww_unit_captured);
                                    self.team_change_war_weariness_at_plot(team, loop_team, np, d.defines.ww_captured_unit);
                                    // `AI_changeWarSuccess`
                                    self.teams[team].ai_war_success[loop_team] += d.defines.war_success_unit_capturing;
                                }

                                // FfH: Modified by Kael 12/30/2207 (equipment is always captured)
                                if !self.unit_is_no_capture(u) || loop_info.equipment_promotion.is_some() {
                                    if !self.unit_is_hidden_nationality(loop_unit) {
                                        self.unit_set_capturing_player(loop_unit, Some(owner));
                                    }
                                }

                                self.unit_kill(loop_unit, false, Some(owner));
                            }
                        }
                    }
                }
            }

            if self.plot_is_goody(np, Some(team)) {
                self.player_do_goody(owner, np, Some(u));
                // C++ keeps using `this`; a goody that deleted the unit would be a use-after-free.
                if !self.units.contains(u) {
                    return;
                }
            }
        }

        let old_plot = self.unit_plot_opt(u);

        if let Some(op) = old_plot {
            self.plot_remove_unit(op, u);

            let visibility_range = self.unit_visibility_range(u);
            self.plot_change_adjacent_sight(op, team, visibility_range, false, Some(u), true);

            let old_area = self.map.plots[op].area.expect("CvUnit::setXY: plot without an area");
            self.area_change_units_per_player(old_area, owner, -1);
            self.map.areas[old_area].power[owner] -= info.power_value;

            if let Some(unit_ai) = self.units[u].ai_type {
                self.map.areas[old_area].num_ai_units[owner][unit_ai] -= 1;
            }

            if self.unit_is_animal(u) {
                self.map.areas[old_area].animals_per_player[owner] -= 1;
            }

            let old_plot_team = self.plot_get_team(op);
            if old_plot_team != Some(team) && old_plot_team.map_or(true, |t| !self.teams[t].vassal[team]) {
                // FfH: Modified by Kael 04/19/2009 (summons do not count as outside units)
                if self.units[u].duration == 0 {
                    self.players[owner].num_outside_units -= 1;
                }
            }

            self.units[u].last_move_turn = self.turn_slice;

            if let Some(old_city) = self.map.plots[op].plot_city {
                if self.unit_is_military_happiness(u) {
                    self.city_change_military_happiness_units(old_city, -1);
                }
            }

            if let Some(working_city) = self.map.plots[op].working_city {
                if self.unit_can_siege(u, self.city_team(working_city)) {
                    // `AI_setAssignWorkDirty(true)`
                    self.cities[working_city].ai_assign_work_dirty = true;
                }
            }

            if self.plot_is_water(op) {
                let (ox, oy) = self.map.xy(op);
                for i in 0..DirectionType::NUM {
                    if let Some(loop_plot) = self.plot_direction(ox, oy, DirectionType::from_usize(i)) {
                        if self.plot_is_water(loop_plot) {
                            if let Some(working_city) = self.map.plots[loop_plot].working_city {
                                if self.unit_can_siege(u, self.city_team(working_city)) {
                                    self.cities[working_city].ai_assign_work_dirty = true;
                                }
                            }
                        }
                    }
                }
            }

            // UI: minimap color, plot list column.
        }

        match new_plot {
            Some(np) => {
                let (nx, ny) = self.map.xy(np);
                self.units[u].x = nx;
                self.units[u].y = ny;
            }
            None => {
                self.units[u].x = INVALID_PLOT_COORD;
                self.units[u].y = INVALID_PLOT_COORD;
            }
        }

        if let Some(np) = new_plot {
            if let Some(new_city) = self.map.plots[np].plot_city {
                let city_team = self.city_team(new_city);
                if self.unit_is_enemy(u, Some(city_team), None)
                    && !self.unit_can_coexist_with_enemy_unit(u, Some(city_team))
                    && self.unit_can_fight(u)
                {
                    self.team_change_war_weariness_at_plot(team, city_team, np, d.defines.ww_captured_city);
                    // `AI_changeWarSuccess`
                    self.teams[team].ai_war_success[city_team] += d.defines.war_success_city_capturing;

                    // C++ `if (NO_PLAYER != eNewOwner)`: `pickConqueredCityOwner` always names a player.
                    let new_owner = self.player_pick_conquered_city_owner(owner, new_city);
                    self.player_acquire_city(new_owner, new_city, true, false, true); // will delete the city
                }
            }

            // Facing direction: graphics.

            // update cargo mission animations
            if self.unit_is_cargo(u) {
                if old_activity_type != Some(ActivityType::ACTIVITY_MISSION) {
                    // C++ `getGroup()->setActivityType(eOldActivityType)`; cargo always had a group
                    // when it started moving (NO_ACTIVITY cannot occur here).
                    if let (Some(g), Some(activity)) = (self.unit_group(u), old_activity_type) {
                        self.group_set_activity_type(g, activity);
                    }
                }
            }

            self.unit_set_fortify_turns(u, 0);

            let visibility_range = self.unit_visibility_range(u);
            self.plot_change_adjacent_sight(np, team, visibility_range, true, Some(u), true); // needs to be here so that the square is considered visible when we move into it...
            self.plot_add_unit(np, u);

            let new_area = self.map.plots[np].area.expect("CvUnit::setXY: plot without an area");
            self.area_change_units_per_player(new_area, owner, 1);
            self.map.areas[new_area].power[owner] += info.power_value;

            if let Some(unit_ai) = self.units[u].ai_type {
                self.map.areas[new_area].num_ai_units[owner][unit_ai] += 1;
            }

            if self.unit_is_animal(u) {
                self.map.areas[new_area].animals_per_player[owner] += 1;
            }

            let new_plot_team = self.plot_get_team(np);
            if new_plot_team != Some(team) && new_plot_team.map_or(true, |t| !self.teams[t].vassal[team]) {
                // FfH: Modified by Kael 04/19/2009 (summons do not count as outside units)
                if self.units[u].duration == 0 {
                    self.players[owner].num_outside_units += 1;
                }
            }

            if self.unit_should_load_on_move(u, np) {
                self.unit_load(u);
            }

            for t in self.civ_team_ids() {
                if self.team_is_alive(t) {
                    if !self.unit_is_invisible(u, t, true) {
                        if self.plot_is_visible(np, t) {
                            self.team_meet(t, team, true);
                        }
                    }
                }
            }

            // The city may have been replaced by `acquireCity` above.
            if let Some(new_city) = self.map.plots[np].plot_city {
                if self.unit_is_military_happiness(u) {
                    self.city_change_military_happiness_units(new_city, 1);
                }
            }

            if let Some(working_city) = self.map.plots[np].working_city {
                if self.unit_can_siege(u, self.city_team(working_city)) {
                    if let Some(index) = self.city_plot_index(working_city, np) {
                        self.city_verify_working_plot(working_city, index);
                    }
                }
            }

            if self.plot_is_water(np) {
                let (nx, ny) = self.map.xy(np);
                for i in 0..DirectionType::NUM {
                    if let Some(loop_plot) = self.plot_direction(nx, ny, DirectionType::from_usize(i)) {
                        if self.plot_is_water(loop_plot) {
                            if let Some(working_city) = self.map.plots[loop_plot].working_city {
                                if self.unit_can_siege(u, self.city_team(working_city)) {
                                    if let Some(index) = self.city_plot_index(working_city, loop_plot) {
                                        self.city_verify_working_plot(working_city, index);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // UI: minimap color, entity movement (`QueueMove` / `SetPosition`), plot list column.
        }

        if let Some(op) = old_plot {
            if self.unit_has_cargo(u) {
                // C++ walks the old plot's live unit list (fetching the next node first): units
                // that left it meanwhile are not visited.
                for loop_unit in self.map.plots[op].units.clone() {
                    if !self.units.contains(loop_unit) || !self.map.plots[op].units.contains(&loop_unit) {
                        continue;
                    }

                    if self.units[loop_unit].transport_unit == Some(u) {
                        self.unit_set_xy(loop_unit, x, y, group);
                    }
                }
            }
        }

        // `bUpdate && hasCargo()`: center unit / flag updates (graphics).

        // FfH Units: Added by Kael 11/10/2008
        if let Some(np) = new_plot {
            if let Some(improvement) = self.map.plots[np].improvement_type {
                let improvement_info = &d.improvements[improvement];
                if improvement_info.spawn_unit_type.is_some() {
                    if !self.unit_is_human(u) || !improvement_info.permanent {
                        let barbarian_team = self.players[self.barbarian_player()].team;
                        if self.at_war(team, barbarian_team) {
                            if self.unit_is_human(u) {
                                // UI: TXT_KEY_MESSAGE_LAIR_DESTROYED.
                                let turn = self.game_turn();
                                self.log.msg(turn, Some(owner), || format!("{} destroyed", d.text.name(&improvement_info.base.description)));
                            }
                            self.plot_set_improvement_type(np, None);
                        }
                    }
                }
            }
            // rechecking because the previous function may have deleted the improvement
            if let Some(improvement) = self.map.plots[np].improvement_type {
                if !d.improvements[improvement].python_on_move.is_empty() {
                    if self.map.plots[np].python_active {
                        self.py_improvement_on_move(u, np, improvement);
                        if !self.units.contains(u) {
                            return;
                        }
                    }
                }
            }
            let (nx, ny) = self.map.xy(np);
            for i in 0..DirectionType::NUM {
                if let Some(loop_plot) = self.plot_direction(nx, ny, DirectionType::from_usize(i)) {
                    if let Some(improvement) = self.map.plots[loop_plot].improvement_type {
                        if !d.improvements[improvement].python_at_range.is_empty() {
                            if self.map.plots[loop_plot].python_active {
                                self.py_improvement_at_range(u, loop_plot, improvement);
                                if !self.units.contains(u) {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            if let Some(feature) = self.map.plots[np].feature_type {
                if !d.features[feature].python_on_move.is_empty() {
                    self.py_feature_on_move(u, np, feature);
                    if !self.units.contains(u) {
                        return;
                    }
                }
            }
            if self.plot_is_city(np, false, None) && info.weapon_tier > 0 {
                if Some(self.units[u].owner) == self.map.plots[np].owner {
                    self.unit_set_weapons(u);
                }
            }
            if info.auto_raze {
                if self.plot_is_owned(np) {
                    if let Some(improvement) = self.map.plots[np].improvement_type {
                        if !d.improvements[improvement].permanent {
                            let plot_owner = self.map.plots[np].owner.expect("owned plot without owner");
                            if self.at_war(team, self.players[plot_owner].team) {
                                self.plot_set_improvement_type(np, None);
                            }
                        }
                    }
                }
                self.plot_set_feature_type(np, None);
            }
        }
        // FfH: End Add

        self.player_update_group_cycle(owner, u);

        // UI: info bar, founding borders, colored plots, enemy glow.

        // report event to Python, along with some other key state
        // FfH: Modified by Kael 10/15/2008 (gated by USE_ON_UNIT_MOVE_CALLBACK)
        if d.defines.use_on_unit_move_callback != 0 {
            // SIGNATURE: `ev_unit_set_xy` takes a `PlotIdx`; C++ also reports `unitSetXY` with a
            // NULL plot when the unit leaves the map (the callback is disabled in FFH2 anyway).
            if let Some(np) = new_plot {
                self.ev_unit_set_xy(np, u);
            }
        }
    }

    /// C++ `CvUnit::at` (CvUnit.cpp:11582-11585).
    pub fn unit_at(&self, u: UnitId, x: i32, y: i32) -> bool {
        let unit = &self.units[u];
        unit.x == x && unit.y == y
    }

    /// C++ `CvUnit::atPlot` (CvUnit.cpp:11588-11591).
    pub fn unit_at_plot(&self, u: UnitId, plot: PlotIdx) -> bool {
        self.unit_plot_opt(u) == Some(plot)
    }

    /// C++ `CvUnit::plot` (CvUnit.cpp:11594-11597).
    /// C++ `plot()` (`plotSorenINLINE(m_iX, m_iY)`): `None` while the unit is off the map.
    /// See also [`Game::unit_plot`] for call sites that dereference the result.
    pub fn unit_plot_opt(&self, u: UnitId) -> Option<PlotIdx> {
        let unit = &self.units[u];
        self.map.plot_soren(unit.x, unit.y)
    }

    /// Convenience for C++ `plot()` call sites that dereference the pointer (C++ would crash on
    /// NULL; this panics). Not a separate C++ function.
    pub fn unit_plot(&self, u: UnitId) -> PlotIdx {
        self.unit_plot_opt(u).expect("CvUnit::plot: unit is not on the map")
    }

    /// C++ `CvUnit::getArea` (CvUnit.cpp:11600-11603) and `CvUnit::area` (CvUnit.cpp:11606-11609).
    /// C++ `getArea()` (area id) and `area()` (`CvArea*`) are the same thing here.
    pub fn unit_area(&self, u: UnitId) -> AreaId {
        // C++ `plot()->getArea()` (crashes off the map).
        self.map.plots[self.unit_plot(u)].area.expect("CvUnit::area: plot without an area")
    }

    /// C++ `CvUnit::onMap` (CvUnit.cpp:11612-11615).
    pub fn unit_on_map(&self, u: UnitId) -> bool {
        self.unit_plot_opt(u).is_some()
    }

    /// C++ `CvUnit::getReconPlot` (CvUnit.cpp:11631-11634).
    pub fn unit_get_recon_plot(&self, u: UnitId) -> Option<PlotIdx> {
        let unit = &self.units[u];
        self.map.plot_soren(unit.recon_x, unit.recon_y)
    }

    /// C++ `CvUnit::setReconPlot` (CvUnit.cpp:11637-11665).
    pub fn unit_set_recon_plot(&mut self, u: UnitId, plot: Option<PlotIdx>) {
        let old_plot = self.unit_get_recon_plot(u);

        if old_plot != plot {
            let team = self.unit_get_team(u);
            let range = self.d.defines.recon_visibility_range;

            if let Some(op) = old_plot {
                self.plot_change_adjacent_sight(op, team, range, false, Some(u), true);
                self.map.plots[op].recon_count -= 1; // changeAdjacentSight() tests for getReconCount()
            }

            match plot {
                None => {
                    self.units[u].recon_x = INVALID_PLOT_COORD;
                    self.units[u].recon_y = INVALID_PLOT_COORD;
                }
                Some(np) => {
                    let (nx, ny) = self.map.xy(np);
                    self.units[u].recon_x = nx;
                    self.units[u].recon_y = ny;

                    self.map.plots[np].recon_count += 1; // changeAdjacentSight() tests for getReconCount()
                    self.plot_change_adjacent_sight(np, team, range, true, Some(u), true);
                }
            }
        }
    }

    /// C++ `CvUnit::setDamage` (CvUnit.cpp:11687-11721).
    /// C++ `bNotifyEntity` (damage animation) is dropped. Kills the unit (with delay) when dead.
    pub fn unit_set_damage(&mut self, u: UnitId, value: i32, player: Option<PlayerId>) {
        let max_hit_points = self.unit_max_hit_points(u);
        self.units[u].damage = range(value, 0, max_hit_points);

        // UI: `NotifyEntity(MISSION_DAMAGE)`, info bar and plot list updates when the value changed.

        if self.unit_is_dead(u) {
            self.unit_kill(u, true, player);
        }
    }

    /// C++ `CvUnit::changeDamage` (CvUnit.cpp:11724-11727).
    pub fn unit_change_damage(&mut self, u: UnitId, change: i32, player: Option<PlayerId>) {
        let value = self.units[u].damage + change;
        self.unit_set_damage(u, value, player);
    }

    /// C++ `CvUnit::doDelayedDeath` (CvUnit.cpp:12643-12652).
    /// Returns true if the unit was killed.
    pub fn unit_do_delayed_death(&mut self, u: UnitId) -> bool {
        if self.units[u].death_delay && !self.unit_is_fighting(u) {
            self.unit_kill(u, false, None);
            return true;
        }

        false
    }

    /// C++ `CvUnit::getVisualOwner` (CvUnit.cpp:12736-12760).
    /// C++ `NO_TEAM` means "the active (interface) team"; rules call sites pass a team. There is no
    /// active team here, so `None` sees the unit as a team other than its own would.
    pub fn unit_get_visual_owner(&self, u: UnitId, for_team: Option<TeamId>) -> PlayerId {
        let team = self.unit_get_team(u);
        if for_team != Some(team) && for_team != Some(self.barbarian_team()) {
            // FfH Hidden Nationality: Modified by Kael 08/27/2007
            if self.unit_is_hidden_nationality(u) {
                if !self.plot_is_city(self.unit_plot(u), true, Some(team)) {
                    return self.barbarian_player();
                }
            }
        }

        self.units[u].owner
    }

    /// C++ `CvUnit::getCombatOwner` (CvUnit.cpp:12763-12774).
    pub fn unit_get_combat_owner(&self, u: UnitId, for_team: Option<TeamId>, plot: Option<PlotIdx>) -> PlayerId {
        if let Some(for_team) = for_team {
            if self.unit_get_team(u) != for_team && for_team != self.barbarian_team() {
                if self.unit_is_always_hostile(u, plot) {
                    return self.barbarian_player();
                }
            }
        }

        self.units[u].owner
    }

    /// C++ `CvUnit::getTeam` (CvUnit.cpp:12776-12779).
    /// C++ `GET_PLAYER(getOwnerINLINE()).getTeam()`.
    pub fn unit_get_team(&self, u: UnitId) -> TeamId {
        self.players[self.units[u].owner].team
    }

    /// C++ `CvUnit::setCapturingPlayer` (CvUnit.cpp:12788-12799).
    pub fn unit_set_capturing_player(&mut self, u: UnitId, player: Option<PlayerId>) {
        // FfH: units immune to capture never get a capturing player.
        if !self.unit_is_immune_to_capture(u) {
            self.units[u].capturing_player = player;
        }
    }

    /// C++ `CvUnit::getName` (CvUnit.cpp:12997-13009).
    /// C++ `getName(uiForm)`: `m_szName` + unit description, or just the description.
    /// The grammatical form argument (text formatting) is dropped.
    pub fn unit_get_name(&self, u: UnitId) -> String {
        let unit = &self.units[u];
        let description = self.d.text.name(&self.d.units[unit.unit_type].base.description);

        if unit.name.is_empty() {
            return description.to_string();
        }

        format!("{} ({})", unit.name, description)
    }

    /// C++ `CvUnit::getNameKey` (CvUnit.cpp:13012-13022).
    pub fn unit_get_name_key(&self, u: UnitId) -> String {
        let unit = &self.units[u];
        if unit.name.is_empty() {
            // `m_pUnitInfo->getTextKeyWide()`
            self.d.units[unit.unit_type].base.description.clone()
        } else {
            unit.name.clone()
        }
    }

    /// C++ `CvUnit::setName` (CvUnit.cpp:13031-13041).
    pub fn unit_set_name(&mut self, u: UnitId, name: String) {
        // C++ also runs `gDLL->stripSpecialCharacters` (exe function, cosmetic) and marks the
        // info pane dirty; names never affect rules.
        self.units[u].name = name;
    }

    /// C++ `CvUnit::getTriggerValue` (CvUnit.cpp:18082-18158).
    /// Calls the trigger's `PythonCanDoUnit` callback (port in `crate::ffh`).
    pub fn unit_get_trigger_value(
        &self,
        u: UnitId,
        trigger: EventTriggerType,
        plot: Option<PlotIdx>,
        check_plot: bool,
    ) -> i32 {
        let k_trigger = &self.d.event_triggers[trigger];
        if k_trigger.num_units <= 0 {
            return MIN_INT;
        }

        if self.unit_is_dead(u) {
            return MIN_INT;
        }

        let owner = self.units[u].owner;

        if !k_trigger.python_can_do_unit.is_empty() {
            if !self.unit_trigger_python_can_do_unit(trigger, owner, u) {
                return MIN_INT;
            }
        }

        if !k_trigger.units_required.is_empty() {
            let unit_class = self.unit_get_unit_class_type(u);
            let found_valid = k_trigger.units_required.iter().any(|&required| required == unit_class);

            if !found_valid {
                return MIN_INT;
            }
        }

        if check_plot {
            if k_trigger.units_on_plot {
                if !self.plot_can_trigger(self.unit_plot(u), trigger, owner) {
                    return MIN_INT;
                }
            }
        }

        let mut value = 0;

        let unit = &self.units[u];
        if 0 == unit.damage && k_trigger.unit_damaged_weight > 0 {
            return MIN_INT;
        }

        value += unit.damage * k_trigger.unit_damaged_weight;

        value += unit.experience * k_trigger.unit_experience_weight;

        if let Some(p) = plot {
            let (px, py) = self.map.xy(p);
            value += self.plot_distance(unit.x, unit.y, px, py) * k_trigger.unit_distance_weight;
        }

        value
    }

    /// The `PythonCanDoUnit(eTrigger, ePlayer, iUnitId)` call of `CvUnit::getTriggerValue`
    /// (`callFunction(PYRandomEventModule, ..)`; a 0 result rejects the unit).
    fn unit_trigger_python_can_do_unit(&self, trigger: EventTriggerType, owner: PlayerId, u: UnitId) -> bool {
        self.py_trigger_can_do_unit(trigger, owner, u)
    }

    /// C++ `CvUnit::canApplyEvent` (CvUnit.cpp:18160-18197).
    pub fn unit_can_apply_event(&self, u: UnitId, event: EventType) -> bool {
        let k_event = &self.d.events[event];

        if 0 != k_event.unit_experience {
            if !self.unit_can_acquire_promotion_any(u) {
                return false;
            }
        }

        if let Some(promotion) = k_event.unit_promotion {
            // FfH: Modified by Kael 10/29/2007 (was `!canAcquirePromotion(..)`)
            if self.units[u].has_promotion[promotion] {
                return false;
            }
        }

        if k_event.unit_immobile_turns > 0 {
            if !self.unit_can_attack(u) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::applyEvent` (CvUnit.cpp:18199-18245).
    pub fn unit_apply_event(&mut self, u: UnitId, event: EventType) {
        if !self.unit_can_apply_event(u, event) {
            return;
        }

        let d = self.d;
        let k_event = &d.events[event];

        if 0 != k_event.unit_experience {
            self.unit_set_damage(u, 0, None);
            self.unit_change_experience(u, k_event.unit_experience, -1, false, false, false);
        }

        if let Some(promotion) = k_event.unit_promotion {
            // FfH: Modified by Kael 02/02/2009 (if we spawn a new unit the promotion goes to the
            // spawned unit, not to the eventtrigger target)
            if k_event.unit_class.is_none() || k_event.num_units == 0 {
                self.unit_set_has_promotion(u, promotion, true);
            }
        }

        if k_event.unit_immobile_turns > 0 {
            self.unit_change_immobile_timer(u, k_event.unit_immobile_turns);
            // UI: TXT_KEY_EVENT_UNIT_IMMOBILE message.
        }

        if !k_event.unit_name_key.is_empty() {
            let name = d.text.name(&k_event.unit_name_key).to_string();
            self.unit_set_name(u, name);
        }

        if k_event.disband_unit {
            self.unit_kill(u, false, None);
        }
    }

    /// C++ `CvUnitAI::AI_init` (CvUnitAI.cpp:41-52).
    /// Called by `CvUnit::init`: `AI_reset(eUnitAI)` (sets `ai_type` and the other AI fields),
    /// `AI_setBirthmark(getSorenRandNum(10000, "AI Unit Birthmark"))` (the sync rand draw must be
    /// kept for RNG parity; `AI_setBirthmark` adjusts it for UNITAI_EXPLORE_SEA), then
    /// `area()->changeNumAIUnits` / `GET_PLAYER(owner).AI_changeNumAIUnits` (+1).
    pub fn unit_ai_init(&mut self, u: UnitId, unit_ai: UnitAIType) {
        self.unit_ai_reset(u, Some(unit_ai));

        //--------------------------------
        // Init other game data
        let birthmark = self.soren_rand_num(10000, "AI Unit Birthmark");
        self.unit_ai_set_birthmark(u, birthmark);

        let owner = self.units[u].owner;
        let area = self.unit_area(u);
        self.map.areas[area].num_ai_units[owner][unit_ai] += 1;
        self.players[owner].ai_num_ai_units[unit_ai] += 1;
    }

    /// C++ `CvUnitAI::AI_reset(eUnitAI)` (CvUnitAI.cpp:60-90): the AI members rules code keeps
    /// (`m_bPermanentSummon` and `m_iAutomatedAbortTurn` are AI-internal and not ported).
    fn unit_ai_reset(&mut self, u: UnitId, unit_ai: Option<UnitAIType>) {
        let unit = &mut self.units[u];
        unit.ai_birthmark = 0;

        // BETTER AI (New Functions Definition) Sephi
        unit.ai_groupflag = GROUPFLAG_NONE;
        unit.ai_origin_x = -1;
        unit.ai_origin_y = -1;
        unit.ai_suicide_summon = false;

        unit.ai_allowed_perm_defense = true;
        unit.ai_allowed_patrol = true;
        unit.ai_allowed_explore = true;

        unit.ai_type = unit_ai;
    }

    /// C++ `CvUnitAI::AI_setBirthmark` (CvUnitAI.cpp:1391-1421): sea explorers get birthmarks that
    /// spread them over the wrap directions when circumnavigation is still available.
    fn unit_ai_set_birthmark(&mut self, u: UnitId, value: i32) {
        self.units[u].ai_birthmark = value;
        if self.units[u].ai_type == Some(UnitAIType::UNITAI_EXPLORE_SEA) {
            if self.circumnavigation_available() {
                let owner = self.units[u].owner;
                let mut birthmark = self.units[u].ai_birthmark;
                birthmark -= birthmark % 4;
                let mut explorer_count = self.players[owner].ai_num_ai_units[UnitAIType::UNITAI_EXPLORE_SEA];
                explorer_count += owner.i() % 4;
                if self.map.wrap_x {
                    if (explorer_count % 2) == 1 {
                        birthmark += 1;
                    }
                }
                if self.map.wrap_y {
                    if !self.map.wrap_x {
                        explorer_count *= 2;
                    }

                    if ((explorer_count >> 1) % 2) == 1 {
                        birthmark += 2;
                    }
                }
                self.units[u].ai_birthmark = birthmark;
            }
        }
    }

    /// C++ `CvUnitAI::AI_isCityAIType` (CvUnitAI.cpp:1376-1382).
    /// Used by rules code through `PUF_isCityAIType` (CvGameCoreUtils).
    pub fn unit_ai_is_city_ai_type(&self, u: UnitId) -> bool {
        let t = self.units[u].ai_type;
        t == Some(UnitAIType::UNITAI_CITY_DEFENSE)
            || t == Some(UnitAIType::UNITAI_CITY_COUNTER)
            || t == Some(UnitAIType::UNITAI_CITY_SPECIAL)
            || t == Some(UnitAIType::UNITAI_RESERVE)
    }

    /// C++ `CvUnitAI::AI_setUnitAIType` (CvUnitAI.cpp:1432-1448).
    /// Called by rules code (`CvUnit::convert`, `CvUnit::found`, CvSelectionGroup). On change: area and
    /// player AI-unit counts -1/+1 and `joinGroup(NULL)`.
    pub fn unit_ai_set_unit_ai_type(&mut self, u: UnitId, unit_ai: UnitAIType) {
        if self.units[u].ai_type != Some(unit_ai) {
            let owner = self.units[u].owner;
            let area = self.unit_area(u);

            // C++ indexes with the old type unchecked (never NO_UNITAI after `AI_init`).
            if let Some(old) = self.units[u].ai_type {
                self.map.areas[area].num_ai_units[owner][old] -= 1;
                self.players[owner].ai_num_ai_units[old] -= 1;
            }

            self.units[u].ai_type = Some(unit_ai);

            self.map.areas[area].num_ai_units[owner][unit_ai] += 1;
            self.players[owner].ai_num_ai_units[unit_ai] += 1;

            self.unit_join_group(u, None, true);
        }
    }

    /// C++ `CvUnitAI::getOriginPlot` (CvUnitAI.cpp:19100-19107).
    /// `None` when `ai_origin_x == -1 && ai_origin_y == -1`.
    pub fn unit_ai_get_origin_plot(&self, u: UnitId) -> Option<PlotIdx> {
        let unit = &self.units[u];
        if unit.ai_origin_x == -1 && unit.ai_origin_y == -1 {
            return None;
        }
        self.plot_xy(unit.ai_origin_x, unit.ai_origin_y, 0, 0)
    }

    /// C++ `CvUnitAI::setOriginPlot` (CvUnitAI.cpp:19109-19121).
    /// Called by `CvUnit::found`. `None` stores -1/-1.
    pub fn unit_ai_set_origin_plot(&mut self, u: UnitId, plot: Option<PlotIdx>) {
        let (x, y) = match plot {
            None => (-1, -1),
            Some(p) => self.map.xy(p),
        };
        self.units[u].ai_origin_x = x;
        self.units[u].ai_origin_y = y;
    }

    // ------------------------------------------------------------------
    // Pointer helpers (C++ `getGroup()` / `getTransportUnit()` return NULL for dead objects)
    // ------------------------------------------------------------------

    /// C++ `getGroup()`: `GET_PLAYER(owner).getSelectionGroup(m_iGroupID)`, NULL without a group
    /// or when the group was deleted.
    fn unit_group(&self, u: UnitId) -> Option<GroupId> {
        self.units[u].group.filter(|&g| self.groups.contains(g))
    }

    /// C++ `getTransportUnit()`: `::getUnit(m_transportUnit)`, NULL once the transport is gone.
    fn unit_transport(&self, u: UnitId) -> Option<UnitId> {
        self.units[u].transport_unit.filter(|&t| self.units.contains(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;

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

    /// C++ `CvPlayer::addUnit` + `reset`, with the coordinates set directly (no `init`/`setXY`).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        u
    }

    #[test]
    fn position_queries() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 2, 1);
        let plot = g.map.plot_idx(2, 1).unwrap();
        assert_eq!(g.unit_plot_opt(u), Some(plot));
        assert_eq!(g.unit_plot(u), plot);
        assert!(g.unit_on_map(u) && g.unit_at_plot(u, plot) && g.unit_at(u, 2, 1));
        assert!(!g.unit_at_plot(u, g.map.plot_idx(3, 1).unwrap()));
        assert_eq!(Some(g.unit_area(u)), g.map.plots[plot].area);

        // Recon plot: none after reset; clearing it again changes nothing.
        assert_eq!(g.unit_get_recon_plot(u), None);
        g.unit_set_recon_plot(u, None);
        assert_eq!((g.units[u].recon_x, g.units[u].recon_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));

        g.units[u].x = INVALID_PLOT_COORD;
        g.units[u].y = INVALID_PLOT_COORD;
        assert_eq!(g.unit_plot_opt(u), None);
        assert!(!g.unit_on_map(u) && !g.unit_at_plot(u, plot));
    }

    #[test]
    fn origin_plot_round_trip() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 2, 1);
        assert_eq!(g.unit_ai_get_origin_plot(u), None);
        let plot = g.map.plot_idx(4, 3).unwrap();
        g.unit_ai_set_origin_plot(u, Some(plot));
        assert_eq!((g.units[u].ai_origin_x, g.units[u].ai_origin_y), (4, 3));
        assert_eq!(g.unit_ai_get_origin_plot(u), Some(plot));
        g.unit_ai_set_origin_plot(u, None);
        assert_eq!((g.units[u].ai_origin_x, g.units[u].ai_origin_y), (-1, -1));
        assert_eq!(g.unit_ai_get_origin_plot(u), None);
    }

    #[test]
    fn names_team_and_owners() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(1);
        let u = add_unit(&mut g, p, "UNIT_WARRIOR", 0, 0);
        let info = &d.units[g.units[u].unit_type];
        let description = d.text.name(&info.base.description).to_string();

        assert_eq!(g.unit_get_name(u), description);
        assert_eq!(g.unit_get_name_key(u), info.base.description);
        g.unit_set_name(u, "Branding".to_string());
        assert_eq!(g.unit_get_name(u), format!("Branding ({description})"));
        assert_eq!(g.unit_get_name_key(u), "Branding");

        assert_eq!(g.unit_get_team(u), g.players[p].team);
        let team = g.unit_get_team(u);
        let barbarian_team = g.barbarian_team();
        let other_team = g.players[PlayerId(0)].team;
        assert_eq!(g.unit_get_visual_owner(u, Some(other_team)), p);
        assert_eq!(g.unit_get_combat_owner(u, None, None), p);
        assert_eq!(g.unit_get_combat_owner(u, Some(team), None), p);
        assert_eq!(g.unit_get_combat_owner(u, Some(barbarian_team), None), p);

        // Hidden nationality only hides the owner from other (non-barbarian) teams.
        g.units[u].hidden_nationality = 1;
        assert_eq!(g.unit_get_visual_owner(u, Some(team)), p);
        assert_eq!(g.unit_get_visual_owner(u, Some(barbarian_team)), p);
    }

    #[test]
    fn damage_and_delayed_death() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 3, 2);
        let max = g.unit_max_hit_points(u);

        g.unit_set_damage(u, -5, None);
        assert_eq!(g.units[u].damage, 0);
        g.unit_set_damage(u, 40, None);
        g.unit_change_damage(u, 15, None);
        assert_eq!(g.units[u].damage, 55);
        assert!(!g.units[u].death_delay);

        // Not delayed-dead: nothing happens.
        assert!(!g.unit_do_delayed_death(u));

        // Lethal damage is clamped and kills with delay (the unit stays until doDelayedDeath).
        g.unit_change_damage(u, 1000, Some(PlayerId(1)));
        assert_eq!(g.units[u].damage, max);
        assert!(g.units[u].death_delay && g.units.contains(u));

        // A fighting unit does not die yet.
        let enemy = add_unit(&mut g, PlayerId(1), "UNIT_WARRIOR", 3, 1);
        g.units[u].combat_unit = Some(enemy);
        assert!(!g.unit_do_delayed_death(u));
        assert!(g.units.contains(u));
    }

    #[test]
    fn can_join_empty_group() {
        let mut g = test_game();
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WARRIOR", 1, 1);
        let id = g.groups.next_id();
        let group = g.player_add_selection_group(owner, SelectionGroup::new(id, owner));
        let plot = g.unit_plot(u);

        // An empty group of the same owner takes any unit, on or off the map.
        assert!(g.unit_can_join_group(u, plot, group));
        assert!(g.unit_can_join_group_opt(u, None, group));

        g.groups[group].ai_force_separate = true;
        assert!(!g.unit_can_join_group(u, plot, group));
        g.groups[group].ai_force_separate = false;

        let other = PlayerId(1);
        let id = g.groups.next_id();
        let foreign = g.player_add_selection_group(other, SelectionGroup::new(id, other));
        assert!(!g.unit_can_join_group(u, plot, foreign));
    }

    #[test]
    fn ai_init_counts_and_birthmark() {
        let mut g = test_game();
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WARRIOR", 2, 2);
        let area = g.unit_area(u);
        let attack = UnitAIType::UNITAI_ATTACK;
        g.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
        g.units[u].ai_origin_x = 1;
        g.units[u].ai_allowed_patrol = false;

        let mut rng = g.soren_rand.clone();
        let expected_birthmark = rng.get_i(10000, "AI Unit Birthmark");

        g.unit_ai_init(u, attack);
        assert_eq!(g.soren_rand, rng, "AI_init draws exactly one sync rand");
        let unit = &g.units[u];
        assert_eq!(unit.ai_type, Some(attack));
        assert_eq!(unit.ai_birthmark, expected_birthmark);
        assert_eq!((unit.ai_groupflag, unit.ai_origin_x, unit.ai_origin_y), (GROUPFLAG_NONE, -1, -1));
        assert!(unit.ai_allowed_patrol && unit.ai_allowed_explore && unit.ai_allowed_perm_defense);
        assert_eq!(g.map.areas[area].num_ai_units[owner][attack], 1);
        assert_eq!(g.players[owner].ai_num_ai_units[attack], 1);

        // Setting the same unit AI changes nothing (and does not regroup).
        g.unit_ai_set_unit_ai_type(u, attack);
        assert_eq!(g.players[owner].ai_num_ai_units[attack], 1);
        assert!(g.units[u].group.is_none());
    }

    #[test]
    fn trigger_values() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 0);
        let trigger = |name: &str| {
            let t = d.t::<EventTriggerType>(name);
            assert!(d.event_triggers[t].units_required.is_empty());
            t
        };

        // A trigger without units never picks one.
        let no_units = d.event_triggers.keys().find(|&t| d.event_triggers[t].num_units <= 0).unwrap();
        assert_eq!(g.unit_get_trigger_value(u, no_units, None, false), MIN_INT);

        // iUnitDamagedWeight 1: only damaged units qualify; the value is the damage.
        let alchemist = trigger("EVENTTRIGGER_ALCHEMIST");
        assert_eq!(g.unit_get_trigger_value(u, alchemist, None, false), MIN_INT);
        g.units[u].damage = 30;
        assert_eq!(g.unit_get_trigger_value(u, alchemist, None, false), 30);

        // iUnitDistanceWeight -1: closer units are preferred.
        let farm_bandits = trigger("EVENTTRIGGER_FARM_BANDITS");
        assert_eq!(d.event_triggers[farm_bandits].unit_experience_weight, 0);
        let far = g.map.plot_idx(3, 0).unwrap();
        assert_eq!(g.unit_get_trigger_value(u, farm_bandits, Some(far), false), -3);
        assert_eq!(g.unit_get_trigger_value(u, farm_bandits, None, false), 0);

        // Dead units never qualify.
        g.units[u].damage = g.unit_max_hit_points(u);
        assert_eq!(g.unit_get_trigger_value(u, farm_bandits, None, false), MIN_INT);
    }

    #[test]
    fn can_apply_promotion_event() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 0);
        let event = d.t::<EventType>("EVENT_ALCHEMIST_2");
        let k_event = &d.events[event];
        assert_eq!((k_event.unit_experience, k_event.unit_immobile_turns), (0, 0));
        let promotion = k_event.unit_promotion.unwrap();

        assert!(g.unit_can_apply_event(u, event));
        g.units[u].has_promotion[promotion] = true;
        assert!(!g.unit_can_apply_event(u, event));
    }
}
