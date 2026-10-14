//! CvUnit commands and actions: `canDoCommand`/`doCommand`, automation, scrap/gift,
//! hold/sleep/fortify/patrol/heal (and heal rates), airlift, nuke, recon, paradrop, air bomb,
//! bombard, pillage, plunder and blockades.
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported. Interface-only code (messages, entity missions/animations,
//! selection cycling, hot keys) is omitted with `UI:` notes; a few messages go to the game log.

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// FfH 0.41 (Kael 04/22/2008) disables gifting: `CvUnit::canGift` returns false before its BtS
/// checks. Those checks are still ported below the early return (they never run).
const GIFTING_DISABLED: bool = true;

/// C++ `getGroup()` at call sites that dereference it: the unit's selection group (C++
/// `getSelectionGroup(m_iGroupID)` is NULL without a live group, and the C++ then crashes).
fn group_of(game: &Game, u: UnitId) -> GroupId {
    game.units[u].group.filter(|&g| game.groups.contains(g)).expect("CvUnit::getGroup: unit has no selection group")
}

/// C++ `::getUnit(IDInfo((PlayerTypes)iData1, iData2))` for COMMAND_LOAD_UNIT. The command data
/// is (owner, unit id); C++ ids are per player, here `iData2` is the global `UnitId` and the
/// owner must match. `None` (C++ NULL) for an invalid owner, a dead unit or another owner's unit.
fn command_unit(game: &Game, owner: i32, id: i32) -> Option<UnitId> {
    if owner < 0 || owner as usize >= game.num_players() || id < 0 {
        return None;
    }
    let unit = UnitId(id as u32);
    if game.units.contains(unit) && game.units[unit].owner.idx() == owner as usize {
        Some(unit)
    } else {
        None
    }
}

/// C++ `iLeaderUnitId` command data (COMMAND_PROMOTION `iData2`): a unit id, -1 = none.
fn command_leader(id: i32) -> Option<UnitId> {
    if id >= 0 {
        Some(UnitId(id as u32))
    } else {
        None
    }
}

/// C++ `(SpellTypes)iData1` for COMMAND_CAST / COMMAND_AUTOMATE_SPELL. C++ quirk: `canCast` and
/// `canSpellAutomate` index the spell table with the command data unchecked (out of bounds for
/// NO_SPELL); here a value outside the table is no spell and the command is not possible.
fn command_spell(game: &Game, data: i32) -> Option<SpellType> {
    if data >= 0 && (data as usize) < game.d.spells.len() {
        Some(SpellType::from_usize(data as usize))
    } else {
        None
    }
}

impl Game {
    /// C++ `CvUnit::canDoCommand` (CvUnit.cpp:2649-2785).
    /// C++ non-const only because `canGift`/`canCast` are; it changes nothing. C++ defaults:
    /// `bTestVisible = false`, `bTestBusy = true`. COMMAND_LOAD_UNIT data is (owner, unit id):
    /// `data1` is the owner's player index, `data2` the transport's `UnitId` (C++: the owner's
    /// per-player unit id). COMMAND_PROMOTION `data2` is the leader's `UnitId` (-1 = none).
    pub fn unit_can_do_command(
        &self,
        u: UnitId,
        command: CommandType,
        data1: i32,
        data2: i32,
        test_visible: bool,
        test_busy: bool,
    ) -> bool {
        if test_busy && self.group_is_busy(group_of(self, u)) {
            return false;
        }

        match command {
            CommandType::COMMAND_PROMOTION => self.unit_can_promote(u, opt_from_i32(data1), command_leader(data2)),

            CommandType::COMMAND_UPGRADE => self.unit_can_upgrade(u, opt_from_i32(data1), test_visible),

            CommandType::COMMAND_AUTOMATE => self.unit_can_automate(u, opt_from_i32(data1)),

            // ADDON (automatic Spellcasting) Sephi
            CommandType::COMMAND_AUTOMATE_SPELL => match command_spell(self, data1) {
                Some(spell) => self.unit_can_spell_automate(u, spell),
                None => false,
            },

            CommandType::COMMAND_WAKE => !self.unit_is_automated(u) && self.unit_is_waiting(u),

            CommandType::COMMAND_CANCEL | CommandType::COMMAND_CANCEL_ALL => {
                !self.unit_is_automated(u) && !self.groups[group_of(self, u)].mission_queue.is_empty()
            }

            CommandType::COMMAND_STOP_AUTOMATION => self.unit_is_automated(u),

            CommandType::COMMAND_DELETE => self.unit_can_scrap(u),

            // C++ `canGift(bTestVisible)` (default `bTestTransport = true`).
            CommandType::COMMAND_GIFT => self.unit_can_gift(u, test_visible, true),

            CommandType::COMMAND_LOAD => self.unit_can_load(u, self.unit_plot(u)),

            CommandType::COMMAND_LOAD_UNIT => match command_unit(self, data1, data2) {
                Some(transport) => self.unit_can_load_unit(u, transport, self.unit_plot(u)),
                None => false,
            },

            CommandType::COMMAND_UNLOAD => self.unit_can_unload(u),

            CommandType::COMMAND_UNLOAD_ALL => self.unit_can_unload_all(u),

            CommandType::COMMAND_HOTKEY => self.unit_is_group_head(u),

            // FfH Spell System: Added by Kael 07/23/2007
            CommandType::COMMAND_CAST => match command_spell(self, data1) {
                Some(spell) => self.unit_can_cast(u, spell, test_visible),
                None => false,
            },

            // C++ `FAssert(false)`.
            _ => false,
        }
    }

    /// C++ `CvUnit::doCommand` (CvUnit.cpp:2788-2905).
    /// COMMAND_HOTKEY only sets the interface hot key (`setHotKeyNumber`, not ported).
    pub fn unit_do_command(&mut self, u: UnitId, command: CommandType, data1: i32, data2: i32) {
        // (C++ FAssert: the unit has an owner.)

        if self.unit_can_do_command(u, command, data1, data2, false, true) {
            match command {
                CommandType::COMMAND_PROMOTION => {
                    self.unit_promote(u, opt_from_i32(data1), command_leader(data2));
                }

                CommandType::COMMAND_UPGRADE => {
                    self.unit_upgrade(u, opt_from_i32(data1));
                }

                CommandType::COMMAND_AUTOMATE => {
                    self.unit_automate(u, opt_from_i32(data1));
                }

                // ADDON (automatic spellcasting) Sephi: toggles the automatic spell.
                CommandType::COMMAND_AUTOMATE_SPELL => {
                    if opt_to_i32(self.units[u].auto_spell_cast) == data1 {
                        self.units[u].auto_spell_cast = None;
                    } else {
                        self.units[u].auto_spell_cast = command_spell(self, data1);
                    }
                }

                CommandType::COMMAND_WAKE => {
                    let g = group_of(self, u);
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
                }

                CommandType::COMMAND_CANCEL => {
                    let g = group_of(self, u);
                    self.group_pop_mission(g);
                }

                CommandType::COMMAND_CANCEL_ALL => {
                    let g = group_of(self, u);
                    self.group_clear_mission_queue(g);
                }

                CommandType::COMMAND_STOP_AUTOMATION => {
                    let g = group_of(self, u);
                    self.group_set_automate_type(g, None);
                }

                CommandType::COMMAND_DELETE => {
                    self.unit_scrap(u);
                }

                CommandType::COMMAND_GIFT => {
                    self.unit_gift(u, true);
                }

                CommandType::COMMAND_LOAD => {
                    self.unit_load(u);
                }

                CommandType::COMMAND_LOAD_UNIT => {
                    if let Some(transport) = command_unit(self, data1, data2) {
                        self.unit_load_unit(u, transport);
                    }
                }

                CommandType::COMMAND_UNLOAD => {
                    self.unit_unload(u);
                }

                CommandType::COMMAND_UNLOAD_ALL => {
                    self.unit_unload_all(u);
                }

                CommandType::COMMAND_HOTKEY => {
                    // UI: `setHotKeyNumber(iData1)` (interface hot keys).
                }

                // FfH Spell System: Added by Kael 07/23/2007
                CommandType::COMMAND_CAST => {
                    let spell = command_spell(self, data1).expect("CvUnit::doCommand: COMMAND_CAST with an invalid spell");
                    self.unit_cast(u, spell);
                }

                // C++ `FAssert(false)`.
                _ => {}
            }
        }

        // UI: the commands that set `bCycle` (upgrade, automate, delete, gift, load, load unit,
        // unload, unload all) reset the selection cycle counter when the unit is selected.

        // C++ calls `getGroup()` on `this` unchecked: the commands that remove the unit (scrap,
        // gift, upgrade, sacrifice spells) only start its delayed death, which this executes.
        if self.units.contains(u) {
            let g = group_of(self, u);
            self.group_do_delayed_death(g);
        }
    }

    /// C++ `CvUnit::canAutomate` (CvUnit.cpp:3730-3820).
    pub fn unit_can_automate(&self, u: UnitId, automate: Option<AutomateType>) -> bool {
        let Some(automate) = automate else {
            return false;
        };

        if !self.unit_is_group_head(u) {
            return false;
        }

        let unit_ai = self.units[u].ai_type;

        match automate {
            AutomateType::AUTOMATE_BUILD => {
                if unit_ai != Some(UnitAIType::UNITAI_WORKER) && unit_ai != Some(UnitAIType::UNITAI_WORKER_SEA) {
                    return false;
                }
            }

            AutomateType::AUTOMATE_NETWORK => {
                if unit_ai != Some(UnitAIType::UNITAI_WORKER) || !self.unit_can_build_route(u) {
                    return false;
                }
            }

            AutomateType::AUTOMATE_CITY => {
                if unit_ai != Some(UnitAIType::UNITAI_WORKER) {
                    return false;
                }
            }

            AutomateType::AUTOMATE_EXPLORE => {
                // BETTER_BTS_AI_MOD 06/02/09 jdog5000: enable exploration for air units.
                if (!self.unit_can_fight(u)
                    && self.unit_get_domain_type(u) != DomainType::DOMAIN_SEA
                    && self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR)
                    || self.unit_get_domain_type(u) == DomainType::DOMAIN_IMMOBILE
                {
                    return false;
                }

                if self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR && !self.unit_can_recon(u, None) {
                    return false;
                }
            }

            AutomateType::AUTOMATE_RELIGION => {
                if unit_ai != Some(UnitAIType::UNITAI_MISSIONARY) {
                    return false;
                }
            }

            // ADDON (automatic terraforming) Sephi, rewritten by Kael 09/19/2009
            AutomateType::AUTOMATE_TERRAFORMING => {
                if !self.units[u].terraformer {
                    return false;
                }
            }

            // C++ `FAssert(false)`.
            _ => {}
        }

        true
    }

    /// C++ `CvUnit::automate` (CvUnit.cpp:3920-3928).
    pub fn unit_automate(&mut self, u: UnitId, automate: Option<AutomateType>) {
        if !self.unit_can_automate(u, automate) {
            return;
        }

        let g = group_of(self, u);
        self.group_set_automate_type(g, automate);
    }

    /// C++ `CvUnit::canScrap` (CvUnit.cpp:3931-3950).
    pub fn unit_can_scrap(&self, u: UnitId) -> bool {
        if self.plot_is_fighting(self.unit_plot(u)) {
            return false;
        }

        // FfH: Added by Kael 11/06/2007
        if self.players[self.units[u].owner].temp_player_timer > 0 {
            return false;
        }
        if self.d.units[self.units[u].unit_type].equipment_promotion.is_some() {
            return false;
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::scrap` (CvUnit.cpp:3953-3974).
    pub fn unit_scrap(&mut self, u: UnitId) {
        if !self.unit_can_scrap(u) {
            return;
        }

        let d = self.d;

        // FfH: Added by Kael 11/04/2007 (the betrayal chance is not rolled: scrapping such a unit
        // always hands it to the barbarians).
        // C++ quirk: every held promotion with a betrayal chance betrays the unit again, so a
        // unit with two such promotions leaves two barbarian copies (`betray` only starts this
        // unit's delayed death, and the loop keeps reading its live promotion flags).
        for promotion in d.promotions.keys() {
            // (C++ uses `this` unchecked; `betray` only kills it with delay.)
            if !self.units.contains(u) {
                return;
            }
            if self.units[u].has_promotion[promotion] {
                if d.promotions[promotion].betrayal_chance != 0 {
                    let barbarian = self.barbarian_player();
                    self.unit_betray(u, barbarian);
                }
            }
        }
        // FfH: End Add

        if !self.units.contains(u) {
            return;
        }
        self.unit_kill(u, true, None);
    }

    /// C++ `CvUnit::canGift` (CvUnit.cpp:3977-4054).
    /// FfH 0.41 disables gifting: the C++ returns false before any other check.
    /// C++ non-const (asks `AI_acceptUnit`) but changes nothing.
    pub fn unit_can_gift(&self, u: UnitId, test_visible: bool, test_transport: bool) -> bool {
        // FfH: Added by Kael 04/22/2008 (to disable gifting)
        if GIFTING_DISABLED {
            return false;
        }
        // FfH: End Add

        let plot = self.unit_plot(u);
        // C++ `getTransportUnit()` (NULL once the transport is gone).
        let transport = self.units[u].transport_unit.filter(|&t| self.units.contains(t));
        let owner = self.units[u].owner;

        if !self.plot_is_owned(plot) {
            return false;
        }
        let plot_owner = self.map.plots[plot].owner.expect("owned plot without an owner");
        let plot_team = self.plot_get_team(plot).expect("owned plot without a team");

        if plot_owner == owner {
            return false;
        }

        if self.plot_is_visible_enemy_unit_for_unit(plot, u) {
            return false;
        }

        if self.plot_is_visible_enemy_unit(plot, plot_owner) {
            return false;
        }

        if !self.plot_is_valid_domain_for_location(plot, u) && transport.is_none() {
            return false;
        }

        let info = &self.d.units[self.units[u].unit_type];
        for corporation in self.d.corporations.keys() {
            if info.corporation_spreads[corporation] > 0 {
                return false;
            }
        }

        if test_transport {
            if let Some(transport) = transport {
                if self.unit_get_team(transport) != plot_team {
                    return false;
                }
            }
        }

        if !test_visible {
            let unit_class = self.unit_get_unit_class_type(u);

            if self.team_is_unit_class_maxed_out(plot_team, unit_class, self.team_get_unit_class_making(plot_team, unit_class)) {
                return false;
            }

            if self.player_is_unit_class_maxed_out(plot_owner, unit_class, self.players[plot_owner].unit_class_making[unit_class]) {
                return false;
            }

            // C++ `AI_acceptUnit` accepts every unit for human players before any AI logic.
            if !(self.players[plot_owner].human || self.controller(plot_owner).accept_unit(self, plot_owner, u)) {
                return false;
            }
        }

        // FfH: Added by Kael 11/06/2007
        if self.players[owner].temp_player_timer > 0 {
            return false;
        }
        // FfH: End Add

        !self.at_war(plot_team, self.unit_get_team(u))
    }

    /// C++ `CvUnit::gift` (CvUnit.cpp:4057-4091).
    /// C++ default `bTestTransport = true`. Never does anything in FfH 0.41 (`canGift` is false).
    pub fn unit_gift(&mut self, u: UnitId, test_transport: bool) {
        if !self.unit_can_gift(u, false, test_transport) {
            return;
        }

        for cargo in self.unit_get_cargo_units(u) {
            if !self.units.contains(cargo) {
                continue;
            }
            self.unit_gift(cargo, false);
        }

        let plot = self.unit_plot(u);
        // (C++ FAssert: the plot has an owner.)
        let plot_owner = self.map.plots[plot].owner.expect("CvUnit::gift: plot without an owner");
        let (unit_type, x, y, unit_ai) = (self.units[u].unit_type, self.units[u].x, self.units[u].y, self.units[u].ai_type);
        let gift_unit = self.player_init_unit(plot_owner, unit_type, x, y, unit_ai, None);

        let owner = self.units[u].owner;

        self.unit_convert(gift_unit, u);

        let gift_owner = self.units[gift_unit].owner;
        let grant_value = self.d.units[self.units[gift_unit].unit_type].production_cost / 5;
        self.player_ai_change_peacetime_grant_value(gift_owner, owner, grant_value);

        // UI: TXT_KEY_MISC_GIFTED_UNIT_TO_YOU message to the new owner.
        let turn = self.game_turn();
        let d = self.d;
        self.log.msg(turn, Some(gift_owner), || format!("Player {} gifted you a {}", owner, d.text.name(&d.units[unit_type].base.description)));

        // Python Event (`this` still exists: `convert` only starts its delayed death)
        let gifting_player = self.units[u].owner;
        let gift_plot = self.unit_plot(u);
        self.ev_unit_gifted(gift_unit, gifting_player, gift_plot);
    }

    /// C++ `CvUnit::canHold` (CvUnit.cpp:4387-4390).
    pub fn unit_can_hold(&self, _u: UnitId, _plot: PlotIdx) -> bool {
        true
    }

    /// C++ `CvUnit::canSleep` (CvUnit.cpp:4393-4406).
    pub fn unit_can_sleep(&self, u: UnitId, _plot: PlotIdx) -> bool {
        if self.unit_is_fortifyable(u) {
            return false;
        }

        if self.unit_is_waiting(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canFortify` (CvUnit.cpp:4409-4422).
    pub fn unit_can_fortify(&self, u: UnitId, _plot: PlotIdx) -> bool {
        if !self.unit_is_fortifyable(u) {
            return false;
        }

        if self.unit_is_waiting(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canAirPatrol` (CvUnit.cpp:4425-4443).
    pub fn unit_can_air_patrol(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
            return false;
        }

        if !self.unit_can_air_defend(u, Some(plot)) {
            return false;
        }

        if self.unit_is_waiting(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canSeaPatrol` (CvUnit.cpp:4446-4469).
    pub fn unit_can_sea_patrol(&self, u: UnitId, plot: PlotIdx) -> bool {
        if !self.plot_is_water(plot) {
            return false;
        }

        if self.unit_get_domain_type(u) != DomainType::DOMAIN_SEA {
            return false;
        }

        if !self.unit_can_fight(u) || self.unit_is_only_defensive(u) {
            return false;
        }

        if self.unit_is_waiting(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canHeal` (CvUnit.cpp:4501-4519).
    pub fn unit_can_heal(&self, u: UnitId, plot: PlotIdx) -> bool {
        if !self.unit_is_hurt(u) {
            return false;
        }

        if self.unit_is_waiting(u) {
            return false;
        }

        if self.unit_heal_rate(u, plot) <= 0 {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canSentry` (CvUnit.cpp:4522-4535).
    pub fn unit_can_sentry(&self, u: UnitId, plot: PlotIdx) -> bool {
        if !self.unit_can_defend(u, Some(plot)) {
            return false;
        }

        if self.unit_is_waiting(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::healRate` (CvUnit.cpp:4538-4673).
    pub fn unit_heal_rate(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let d = self.d;

        // FfH: Added by Kael 11/05/2008
        if self.is_option(GameOptionType::GAMEOPTION_NO_HEALING_FOR_HUMANS) {
            if self.unit_is_human(u) {
                if self.unit_is_alive(u) {
                    return 0;
                }
            }
        }
        // FfH: End Add

        let city = self.map.plots[plot].plot_city;
        let team = self.unit_get_team(u);
        let plot_team = self.plot_get_team(plot);
        let unit = &self.units[u];

        let mut total_heal = 0;

        if self.plot_is_city(plot, true, Some(team)) {
            total_heal += d.defines.city_heal_rate
                + if self.team_is_friendly_territory(team, plot_team) { unit.extra_friendly_heal } else { unit.extra_neutral_heal };
            if let Some(c) = city {
                if !self.city_is_occupation(c) {
                    total_heal += self.cities[c].heal_rate;
                }
            }
        } else if !self.team_is_friendly_territory(team, plot_team) {
            if self.unit_is_enemy(u, plot_team, Some(plot)) {
                total_heal += d.defines.enemy_heal_rate + unit.extra_enemy_heal;

                // FfH Mana Effects: Added by Kael 08/21/2007
                if let Some(plot_owner) = self.map.plots[plot].owner {
                    total_heal += self.players[plot_owner].heal_change_enemy;
                }
                // FfH: End Add
            } else {
                total_heal += d.defines.neutral_heal_rate + unit.extra_neutral_heal;
            }
        } else {
            total_heal += d.defines.friendly_heal_rate + unit.extra_friendly_heal;

            // FfH Mana Effects: Added by Kael 08/21/2007
            total_heal += self.players[unit.owner].heal_change;
            // FfH: End Add
        }

        // XXX optimize this (save it?)
        let mut best_heal = 0;

        for &loop_unit in &self.map.plots[plot].units {
            if self.unit_get_team(loop_unit) == team {
                // XXX what about alliances?
                let heal = self.units[loop_unit].same_tile_heal;

                if heal > best_heal {
                    best_heal = heal;
                }
            }
        }

        let (px, py) = self.map.xy(plot);
        for i in 0..DirectionType::NUM {
            if let Some(loop_plot) = self.plot_direction(px, py, DirectionType::from_usize(i)) {
                if self.map.plots[loop_plot].area == self.map.plots[plot].area {
                    for &loop_unit in &self.map.plots[loop_plot].units {
                        if self.unit_get_team(loop_unit) == team {
                            // XXX what about alliances?
                            let heal = self.units[loop_unit].adjacent_tile_heal;

                            if heal > best_heal {
                                best_heal = heal;
                            }
                        }
                    }
                }
            }
        }

        total_heal += best_heal;
        // XXX

        // FfH: Added by Kael 10/29/2007
        if let Some(improvement) = self.map.plots[plot].improvement_type {
            total_heal += d.improvements[improvement].heal_rate_change;
        }
        if total_heal < 0 {
            total_heal = 0;
        }
        // FfH: End Add

        total_heal
    }

    /// C++ `CvUnit::healTurns` (CvUnit.cpp:4676-4703).
    /// `MAX_INT` when the unit cannot heal on `plot`.
    pub fn unit_heal_turns(&self, u: UnitId, plot: PlotIdx) -> i32 {
        if !self.unit_is_hurt(u) {
            return 0;
        }

        let heal = self.unit_heal_rate(u, plot);

        if heal > 0 {
            let damage = self.units[u].damage;
            let mut turns = damage / heal;

            if damage % heal != 0 {
                turns += 1;
            }

            turns
        } else {
            MAX_INT
        }
    }

    /// C++ `CvUnit::doHeal` (CvUnit.cpp:4706-4709).
    pub fn unit_do_heal(&mut self, u: UnitId) {
        let heal = self.unit_heal_rate(u, self.unit_plot(u));
        self.unit_change_damage(u, -heal, None);
    }

    /// C++ `CvUnit::canAirlift` (CvUnit.cpp:4712-4744).
    pub fn unit_can_airlift(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_LAND {
            return false;
        }

        if self.unit_has_moved(u) {
            return false;
        }

        let Some(city) = self.map.plots[plot].plot_city else {
            return false;
        };

        if self.cities[city].curr_airlift >= self.cities[city].max_airlift {
            return false;
        }

        if self.city_team(city) != self.unit_get_team(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canAirliftAt` (CvUnit.cpp:4747-4782).
    pub fn unit_can_airlift_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        if !self.unit_can_airlift(u, plot) {
            return false;
        }

        // C++ passes the plot to `canMoveInto`, which dereferences it (crashes off the map).
        let target_plot = self.map.plot_xy(x, y).expect("CvUnit::canAirliftAt: target plot is off the map");

        if !self.unit_can_move_into(u, target_plot, false, false, false) {
            return false;
        }

        let Some(target_city) = self.map.plots[target_plot].plot_city else {
            return false;
        };

        if self.cities[target_city].airlift_targeted {
            return false;
        }

        let team = self.unit_get_team(u);
        let target_team = self.city_team(target_city);
        if target_team != team && !self.teams[target_team].vassal[team] {
            return false;
        }

        true
    }

    /// C++ `CvUnit::airlift` (CvUnit.cpp:4785-4815).
    pub fn unit_airlift(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        if !self.unit_can_airlift_at(u, self.unit_plot(u), x, y) {
            return false;
        }

        let city = self.map.plots[self.unit_plot(u)].plot_city.expect("CvUnit::airlift: no city to airlift from");
        let target_plot = self.map.plot_xy(x, y).expect("CvUnit::airlift: target plot is off the map");
        let target_city = self.map.plots[target_plot].plot_city.expect("CvUnit::airlift: no target city");
        // (C++ FAssert: the two cities differ.)

        // `changeCurrAirlift(1)`
        self.cities[city].curr_airlift += 1;
        if self.cities[target_city].max_airlift == 0 {
            // `setAirliftTargeted(true)`
            self.cities[target_city].airlift_targeted = true;
        }

        self.unit_finish_moves(u);

        let (tx, ty) = self.map.xy(target_plot);
        self.unit_set_xy(u, tx, ty, false);

        true
    }

    /// C++ `CvUnit::isNukeVictim` (CvUnit.cpp:4818-4855).
    pub fn unit_is_nuke_victim(&self, u: UnitId, plot: PlotIdx, team: TeamId) -> bool {
        if !self.team_is_alive(team) {
            return false;
        }

        let unit_team = self.unit_get_team(u);
        if team == unit_team {
            return false;
        }

        let nuke_range = self.unit_nuke_range(u);
        let (px, py) = self.map.xy(plot);

        for dx in -nuke_range..=nuke_range {
            for dy in -nuke_range..=nuke_range {
                if let Some(loop_plot) = self.plot_xy(px, py, dx, dy) {
                    if self.plot_get_team(loop_plot) == Some(team) {
                        return true;
                    }

                    if self
                        .plot_plot_check(loop_plot, Game::puf_is_combat_team, team.i(), unit_team.i(), None, None, None, -1, -1)
                        .is_some()
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvUnit::canNuke` (CvUnit.cpp:4858-4866).
    pub fn unit_can_nuke(&self, u: UnitId, _plot: PlotIdx) -> bool {
        if self.unit_nuke_range(u) == -1 {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canNukeAt` (CvUnit.cpp:4869-4904).
    pub fn unit_can_nuke_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        if !self.unit_can_nuke(u, plot) {
            return false;
        }

        let (px, py) = self.map.xy(plot);
        let distance = self.plot_distance(px, py, x, y);
        if distance <= self.unit_nuke_range(u) {
            return false;
        }

        if self.unit_air_range(u) > 0 && distance > self.unit_air_range(u) {
            return false;
        }

        // C++ passes the plot to `isNukeVictim`, which dereferences it (crashes off the map).
        let target_plot = self.map.plot_xy(x, y).expect("CvUnit::canNukeAt: target plot is off the map");

        for team in self.team_ids() {
            if self.unit_is_nuke_victim(u, target_plot, team) {
                if !self.unit_is_enemy(u, Some(team), Some(plot)) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvUnit::nuke` (CvUnit.cpp:4907-5084).
    /// The explosion itself happens when the unit dies (`kill` -> `CvPlot::nukeExplosion` on the
    /// attack plot when it made an attack).
    pub fn unit_nuke(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        let d = self.d;

        if !self.unit_can_nuke_at(u, self.unit_plot(u), x, y) {
            return false;
        }

        let plot = self.map.plot_xy(x, y).expect("CvUnit::nuke: target plot is off the map");

        let teams: Vec<TeamId> = self.team_ids().collect();
        let teams_affected: Vec<bool> = teams.iter().map(|&t| self.unit_is_nuke_victim(u, plot, t)).collect();

        for (&team, &affected) in teams.iter().zip(&teams_affected) {
            if affected {
                if !self.unit_is_enemy(u, Some(team), None) {
                    let unit_team = self.unit_get_team(u);
                    self.team_declare_war(unit_team, team, false, Some(WarPlanType::WARPLAN_LIMITED));
                    // (C++ uses `this` unchecked after declaring war, which can bump units.)
                    if !self.units.contains(u) {
                        return false;
                    }
                }
            }
        }

        let mut best_interception = 0;
        let mut best_team: Option<TeamId> = None;

        for (&team, &affected) in teams.iter().zip(&teams_affected) {
            if affected {
                if self.teams[team].nuke_interception > best_interception {
                    best_interception = self.teams[team].nuke_interception;
                    best_team = Some(team);
                }
            }
        }

        best_interception *= 100 - d.units[self.units[u].unit_type].evasion_probability;
        best_interception /= 100;

        self.unit_set_recon_plot(u, Some(plot));

        let turn = self.game_turn();

        if self.soren_rand_num(100, "Nuke") < best_interception {
            // UI: TXT_KEY_MISC_NUKE_INTERCEPTED to every alive player; the nuke entity mission.
            let owner = self.units[u].owner;
            self.log.msg(turn, None, || {
                format!("A nuke of player {} was intercepted by team {}", owner, best_team.map_or(-1, |t| t.i()))
            });

            self.unit_kill(u, true, None);
            return true; // Intercepted!!! (XXX need special event for this...)
        }

        // UI: the nuke entity mission (not intercepted).

        self.units[u].made_attack = true;
        self.unit_set_attack_plot(u, Some(plot), false);

        let unit_team = self.unit_get_team(u);
        for (&team, &affected) in teams.iter().zip(&teams_affected) {
            if affected {
                self.team_change_war_weariness(team, unit_team, 100 * d.defines.ww_hit_by_nuke);
                self.team_change_war_weariness(unit_team, team, 100 * d.defines.ww_attacked_with_nuke);
                // `AI_changeWarSuccess` (AI bookkeeping read by rules)
                self.teams[unit_team].ai_war_success[team] += d.defines.war_success_nuke;
            }
        }

        let owner = self.units[u].owner;
        for (&team_i, &affected_i) in teams.iter().zip(&teams_affected) {
            if self.team_is_alive(team_i) {
                if team_i != unit_team {
                    if affected_i {
                        for player in self.player_ids() {
                            if self.players[player].alive {
                                if self.players[player].team == team_i {
                                    // `AI_changeMemoryCount` (AI bookkeeping read by rules)
                                    self.players[player].ai_memory_count[owner][MemoryType::MEMORY_NUKED_US] += 1;
                                }
                            }
                        }
                    } else {
                        for (&team_j, &affected_j) in teams.iter().zip(&teams_affected) {
                            if self.team_is_alive(team_j) {
                                if affected_j {
                                    if self.teams[team_i].has_met[team_j] {
                                        if self.team_ai_get_attitude(team_i, team_j, true) >= AttitudeType::ATTITUDE_CAUTIOUS {
                                            for player in self.player_ids() {
                                                if self.players[player].alive {
                                                    if self.players[player].team == team_i {
                                                        self.players[player].ai_memory_count[owner][MemoryType::MEMORY_NUKED_FRIEND] += 1;
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // XXX some AI should declare war here...

        // UI: TXT_KEY_MISC_NUKE_LAUNCHED to every alive player.
        self.log.msg(turn, None, || format!("Player {} launched a nuke at ({}, {})", owner, x, y));

        if self.unit_is_suicide(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canRecon` (CvUnit.cpp:5087-5105).
    /// `plot` is unused by the C++ and `canAutomate` passes NULL.
    pub fn unit_can_recon(&self, u: UnitId, _plot: Option<PlotIdx>) -> bool {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
            return false;
        }

        if self.unit_air_range(u) == 0 {
            return false;
        }

        if self.d.units[self.units[u].unit_type].suicide {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canReconAt` (CvUnit.cpp:5109-5123).
    pub fn unit_can_recon_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        if !self.unit_can_recon(u, Some(plot)) {
            return false;
        }

        let (px, py) = self.map.xy(plot);
        let distance = self.plot_distance(px, py, x, y);
        if distance > self.unit_air_range(u) || 0 == distance {
            return false;
        }

        true
    }

    /// C++ `CvUnit::recon` (CvUnit.cpp:5126-5155).
    pub fn unit_recon(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        if !self.unit_can_recon_at(u, self.unit_plot(u), x, y) {
            return false;
        }

        // `canReconAt` only checks the distance, so this may be NULL off the map; `setReconPlot`
        // accepts NULL. (The C++ then crashes in the interface branch below, not ported.)
        let plot = self.map.plot_xy(x, y);

        self.unit_set_recon_plot(u, plot);

        self.unit_finish_moves(u);

        // UI: the MISSION_RECON air mission animation when the plot is visible to the active player.

        true
    }

    /// C++ `CvUnit::canParadrop` (CvUnit.cpp:5158-5176).
    pub fn unit_can_paradrop(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.unit_get_drop_range(u) <= 0 {
            return false;
        }

        if self.unit_has_moved(u) {
            return false;
        }

        if !self.plot_is_friendly_city(plot, u, true) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canParadropAt` (CvUnit.cpp:5180-5222).
    pub fn unit_can_paradrop_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        if !self.unit_can_paradrop(u, plot) {
            return false;
        }

        let target_plot = match self.map.plot_xy(x, y) {
            Some(p) if p != plot => p,
            _ => return false,
        };

        if !self.plot_is_visible(target_plot, self.unit_get_team(u)) {
            return false;
        }

        if !self.unit_can_move_into(u, target_plot, false, false, true) {
            return false;
        }

        let (px, py) = self.map.xy(plot);
        if self.plot_distance(px, py, x, y) > self.unit_get_drop_range(u) {
            return false;
        }

        if !self.unit_can_coexist_with_enemy_unit(u, None) {
            if self.plot_is_enemy_city(target_plot, u) {
                return false;
            }

            if self.plot_is_visible_enemy_unit_for_unit(target_plot, u) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::paradrop` (CvUnit.cpp:5225-5260).
    pub fn unit_paradrop(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        if !self.unit_can_paradrop_at(u, self.unit_plot(u), x, y) {
            return false;
        }

        let plot = self.map.plot_xy(x, y).expect("CvUnit::paradrop: target plot is off the map");

        self.unit_change_moves(u, self.d.defines.move_denominator / 2);
        self.units[u].made_attack = true;

        let (px, py) = self.map.xy(plot);
        self.unit_set_xy(u, px, py, false);

        // (C++ uses `this` unchecked; entering the plot can run Python that kills the unit.)
        if !self.units.contains(u) {
            return true;
        }

        //check if intercepted
        if self.unit_intercept_test(u, plot) {
            return true;
        }

        // UI: the MISSION_PARADROP air mission animation when the plot is visible to the active player.

        true
    }

    /// C++ `CvUnit::canAirBomb` (CvUnit.cpp:5263-5281).
    pub fn unit_can_air_bomb(&self, u: UnitId, _plot: PlotIdx) -> bool {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
            return false;
        }

        if self.unit_air_bomb_base_rate(u) == 0 {
            return false;
        }

        if self.units[u].made_attack {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canAirBombAt` (CvUnit.cpp:5284-5337).
    pub fn unit_can_air_bomb_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        if !self.unit_can_air_bomb(u, plot) {
            return false;
        }

        // C++ dereferences the target plot (crashes off the map).
        let target_plot = self.map.plot_xy(x, y).expect("CvUnit::canAirBombAt: target plot is off the map");

        let (px, py) = self.map.xy(plot);
        let (tx, ty) = self.map.xy(target_plot);
        if self.plot_distance(px, py, tx, ty) > self.unit_air_range(u) {
            return false;
        }

        if self.plot_is_owned(target_plot) {
            if !self.unit_potential_war_action(u, target_plot) {
                return false;
            }
        }

        match self.map.plots[target_plot].plot_city {
            Some(city) => {
                if !self.city_is_bombardable(city, Some(u)) {
                    return false;
                }
            }
            None => {
                let Some(improvement) = self.map.plots[target_plot].improvement_type else {
                    return false;
                };

                if self.d.improvements[improvement].permanent {
                    return false;
                }

                if self.d.improvements[improvement].air_bomb_defense == -1 {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvUnit::airBomb` (CvUnit.cpp:5340-5431).
    pub fn unit_air_bomb(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        let d = self.d;

        if !self.unit_can_air_bomb_at(u, self.unit_plot(u), x, y) {
            return false;
        }

        let plot = self.map.plot_xy(x, y).expect("CvUnit::airBomb: target plot is off the map");

        if !self.unit_is_enemy(u, self.plot_get_team(plot), None) {
            let g = group_of(self, u);
            self.group_declare_war(g, plot, true);
            // (C++ uses `this` unchecked after declaring war, which can bump units.)
            if !self.units.contains(u) {
                return false;
            }
        }

        if !self.unit_is_enemy(u, self.plot_get_team(plot), None) {
            return false;
        }

        if self.unit_intercept_test(u, plot) {
            return true;
        }

        let turn = self.game_turn();

        match self.map.plots[plot].plot_city {
            Some(city) => {
                let change = -self.unit_air_bomb_curr_rate(u);
                self.city_change_defense_modifier(city, change);

                // UI: TXT_KEY_MISC_YOU_DEFENSES_REDUCED_TO to the city owner and
                // TXT_KEY_MISC_ENEMY_DEFENSES_REDUCED_TO to the unit owner.
            }
            None => {
                if let Some(improvement) = self.map.plots[plot].improvement_type {
                    // C++ quirk: MSVC evaluation order (operands of `>=` left to right: offense first).
                    let offense = self.soren_rand_num(self.unit_air_bomb_curr_rate(u), "Air Bomb - Offense");
                    let defense = self.soren_rand_num(d.improvements[improvement].air_bomb_defense, "Air Bomb - Defense");
                    if offense >= defense {
                        // UI: TXT_KEY_MISC_YOU_UNIT_DESTROYED_IMP to the unit owner and
                        // TXT_KEY_MISC_YOU_IMP_WAS_DESTROYED to the plot owner.
                        let plot_owner = self.map.plots[plot].owner;
                        self.log.msg(turn, plot_owner, || format!("{} destroyed by an air bomb", d.text.name(&d.improvements[improvement].base.description)));

                        self.plot_set_improvement_type(plot, d.improvements[improvement].improvement_pillage);
                    } else {
                        // UI: TXT_KEY_MISC_YOU_UNIT_FAIL_DESTROY_IMP to the unit owner.
                    }
                }
            }
        }

        self.unit_set_recon_plot(u, Some(plot));

        self.units[u].made_attack = true;
        self.unit_change_moves(u, d.defines.move_denominator);

        // UI: the MISSION_AIRBOMB air mission animation when the plot is visible to the active player.

        if self.unit_is_suicide(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::bombardTarget` (CvUnit.cpp:5434-5470).
    pub fn unit_bombard_target(&self, u: UnitId, plot: PlotIdx) -> Option<CityId> {
        let mut best_value = MAX_INT;
        let mut best_city = None;

        let (px, py) = self.map.xy(plot);
        for i in 0..DirectionType::NUM {
            if let Some(loop_plot) = self.plot_direction(px, py, DirectionType::from_usize(i)) {
                if let Some(loop_city) = self.map.plots[loop_plot].plot_city {
                    if self.city_is_bombardable(loop_city, Some(u)) {
                        let mut value = self.cities[loop_city].defense_damage;

                        // always prefer cities we are at war with
                        if self.unit_is_enemy(u, Some(self.city_team(loop_city)), Some(plot)) {
                            value *= 128;
                        }

                        if value < best_value {
                            best_value = value;
                            best_city = Some(loop_city);
                        }
                    }
                }
            }
        }

        best_city
    }

    /// C++ `CvUnit::canBombard` (CvUnit.cpp:5473-5496).
    pub fn unit_can_bombard(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.unit_bombard_rate(u) <= 0 {
            return false;
        }

        if self.units[u].made_attack {
            return false;
        }

        if self.unit_is_cargo(u) {
            return false;
        }

        if self.unit_bombard_target(u, plot).is_none() {
            return false;
        }

        true
    }

    /// C++ `CvUnit::bombard` (CvUnit.cpp:5499-5560).
    pub fn unit_bombard(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);
        if !self.unit_can_bombard(u, plot) {
            return false;
        }

        let bombard_city = self.unit_bombard_target(u, plot).expect("CvUnit::bombard: no bombard target");

        let target_plot = self.city_plot(bombard_city);
        if !self.unit_is_enemy(u, self.plot_get_team(target_plot), None) {
            let g = group_of(self, u);
            self.group_declare_war(g, target_plot, true);
            // (C++ uses `this` unchecked after declaring war, which can bump units.)
            if !self.units.contains(u) {
                return false;
            }
        }

        if !self.unit_is_enemy(u, self.plot_get_team(target_plot), None) {
            return false;
        }

        let mut bombard_modifier = 0;
        if !self.unit_ignore_building_defense(u) {
            bombard_modifier -= self.cities[bombard_city].building_bombard_defense;
        }

        let change = -(self.unit_bombard_rate(u) * std::cmp::max(0, 100 + bombard_modifier)) / 100;
        self.city_change_defense_modifier(bombard_city, change);

        self.units[u].made_attack = true;
        self.unit_change_moves(u, self.d.defines.move_denominator);

        // UI: TXT_KEY_MISC_DEFENSES_IN_CITY_REDUCED_TO to the city owner and
        // TXT_KEY_MISC_YOU_REDUCE_CITY_DEFENSES to the unit owner; the MISSION_BOMBARD entity
        // mission (its `getBestDefender` call only picks the animated defender).

        // FfH: Added by Kael 08/30/2007
        if self.unit_is_suicide(u) {
            self.unit_kill(u, true, None);
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::canPillage` (CvUnit.cpp:5563-5620).
    pub fn unit_can_pillage(&self, u: UnitId, plot: PlotIdx) -> bool {
        let d = self.d;

        if !d.units[self.units[u].unit_type].pillage {
            return false;
        }

        if self.plot_is_city(plot, false, None) {
            return false;
        }

        let improvement = self.map.plots[plot].improvement_type;

        match improvement {
            None => {
                if !self.plot_is_route(plot) {
                    return false;
                }
            }
            Some(improvement) => {
                if d.improvements[improvement].permanent {
                    return false;
                }
            }
        }

        if self.plot_is_owned(plot) {
            if !self.unit_potential_war_action(u, plot) {
                if improvement.is_none() || self.map.plots[plot].owner != Some(self.units[u].owner) {
                    return false;
                }
            }
        }

        if !self.plot_is_valid_domain_for_action(plot, u) {
            return false;
        }

        // FfH: Added by Kael 03/24/2009
        if self.unit_is_barbarian(u) {
            if let Some(improvement) = improvement {
                if d.improvements[improvement].spawn_unit_type.is_some() {
                    return false;
                }
            }
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::pillage` (CvUnit.cpp:5623-5750).
    /// Pillage gold comes from the `doPillageGold` Python callback (port in `crate::ffh`).
    pub fn unit_pillage(&mut self, u: UnitId) -> bool {
        let d = self.d;
        let mut temp_improvement: Option<ImprovementType> = None;
        let mut temp_route: Option<RouteType> = None;

        let plot = self.unit_plot(u);

        if !self.unit_can_pillage(u, plot) {
            return false;
        }

        if self.plot_is_owned(plot) {
            // we should not be calling this without declaring war first, so do not declare war here
            if !self.unit_is_enemy(u, self.plot_get_team(plot), Some(plot)) {
                if self.map.plots[plot].improvement_type.is_none() || self.map.plots[plot].owner != Some(self.units[u].owner) {
                    return false;
                }
            }
        }

        if self.plot_is_water(plot) {
            let interceptor = self.unit_best_sea_pillage_interceptor(u, u, d.defines.combat_die_sides / 2);
            if let Some(interceptor) = interceptor {
                self.units[u].made_attack = false;

                let withdrawal = self.unit_withdrawal_probability(u);
                self.unit_change_extra_withdrawal(u, -withdrawal); // no withdrawal since we are really the defender
                let interceptor_plot = self.unit_plot(interceptor);
                self.unit_attack(u, interceptor_plot);
                // (C++ uses `this` unchecked: a lost combat only starts its delayed death.)
                if self.units.contains(u) {
                    self.unit_change_extra_withdrawal(u, withdrawal);
                }

                return false;
            }
        }

        if let Some(improvement) = self.map.plots[plot].improvement_type {
            temp_improvement = Some(improvement);

            if self.plot_get_team(plot) != Some(self.unit_get_team(u)) {
                // Use python to determine pillage amounts...
                let mut pillage_gold = self.py_do_pillage_gold(plot, u);

                if pillage_gold > 0 {
                    let owner = self.units[u].owner;

                    // FfH Traits: Added by Kale 08/02/2007
                    pillage_gold += (pillage_gold * self.players[owner].pillaging_gold) / 100;
                    // FfH: End Add

                    // `changeGold`
                    self.players[owner].gold += pillage_gold;

                    // UI: TXT_KEY_MISC_PLUNDERED_GOLD_FROM_IMP to the unit owner and
                    // TXT_KEY_MISC_IMP_DESTROYED to the plot owner.
                    let turn = self.game_turn();
                    self.log.msg(turn, Some(owner), || {
                        format!("Plundered {} gold from the {}", pillage_gold, d.text.name(&d.improvements[improvement].base.description))
                    });
                }
            }

            self.plot_set_improvement_type(plot, d.improvements[improvement].improvement_pillage);
        } else if self.plot_is_route(plot) {
            temp_route = self.map.plots[plot].route_type;
            self.plot_set_route_type(plot, None, true); // XXX downgrade rail???
        }

        self.unit_change_moves(u, d.defines.move_denominator);

        // UI: the MISSION_PILLAGE entity mission when the plot is visible to the active player.

        if temp_improvement.is_some() || temp_route.is_some() {
            let owner = self.units[u].owner;
            self.ev_unit_pillage(u, temp_improvement, temp_route, owner);
            // (C++ uses `this` unchecked after the Python event.)
            if !self.units.contains(u) {
                return true;
            }
        }

        // FfH: Added by Kael 03/02/2009
        for promotion in d.promotions.keys() {
            if self.units[u].has_promotion[promotion] {
                if d.promotions[promotion].removed_by_combat {
                    self.unit_set_has_promotion(u, promotion, false);
                }
            }
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::canPlunder` (CvUnit.cpp:5753-5789).
    /// C++ default `bTestVisible = false`.
    pub fn unit_can_plunder(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_SEA {
            return false;
        }

        if !self.d.units[self.units[u].unit_type].pillage {
            return false;
        }

        if !self.plot_is_water(plot) {
            return false;
        }

        if self.plot_is_fresh_water(plot) {
            return false;
        }

        if !self.plot_is_valid_domain_for_action(plot, u) {
            return false;
        }

        if !test_visible {
            if self.plot_get_team(plot) == Some(self.unit_get_team(u)) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::plunder` (CvUnit.cpp:5792-5806).
    pub fn unit_plunder(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_plunder(u, plot, false) {
            return false;
        }

        self.unit_set_blockading(u, true);

        self.unit_finish_moves(u);

        true
    }

    /// C++ `CvUnit::updatePlunder` (CvUnit.cpp:5809-5854).
    pub fn unit_update_plunder(&mut self, u: UnitId, change: i32, update_plot_groups: bool) {
        let blockade_range = self.d.defines.ship_blockade_range;

        // (C++ leaves `bOldTradeNet` uninitialized; it is always set before it is read.)
        let mut old_trade_net = false;
        let mut changed = false;

        let (x, y) = (self.units[u].x, self.units[u].y);

        for team in self.team_ids() {
            if self.unit_is_enemy(u, Some(team), None) {
                for i in -blockade_range..=blockade_range {
                    for j in -blockade_range..=blockade_range {
                        let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                            continue;
                        };

                        if self.plot_is_water(loop_plot) && self.map.plots[loop_plot].area == Some(self.unit_area(u)) {
                            if !changed {
                                old_trade_net = self.plot_is_trade_network(loop_plot, team);
                            }

                            self.plot_change_blockaded_count(loop_plot, team, change);

                            if !changed {
                                changed = old_trade_net != self.plot_is_trade_network(loop_plot, team);
                            }
                        }
                    }
                }
            }
        }

        if changed {
            // UI: `setDirty(BlockadedPlots_DIRTY_BIT)`.

            if update_plot_groups {
                self.update_plot_groups();
            }
        }
    }

    /// C++ `CvUnit::isAutomated` (CvUnit.cpp:10004-10007).
    /// C++ `getGroup()->isAutomated()`.
    pub fn unit_is_automated(&self, u: UnitId) -> bool {
        self.group_is_automated(group_of(self, u))
    }

    /// C++ `CvUnit::isWaiting` (CvUnit.cpp:10010-10013).
    /// C++ `getGroup()->isWaiting()`.
    pub fn unit_is_waiting(&self, u: UnitId) -> bool {
        self.group_is_waiting(group_of(self, u))
    }

    /// C++ `CvUnit::setBlockading` (CvUnit.cpp:12677-12685).
    pub fn unit_set_blockading(&mut self, u: UnitId, value: bool) {
        if value != self.units[u].blockading {
            self.units[u].blockading = value;

            self.unit_update_plunder(u, if value { 1 } else { -1 }, true);
        }
    }

    /// C++ `CvUnit::collectBlockadeGold` (CvUnit.cpp:12687-12728).
    pub fn unit_collect_blockade_gold(&mut self, u: UnitId) {
        let team = self.unit_get_team(u);

        if self.plot_get_team(self.unit_plot(u)) == Some(team) {
            return;
        }

        let blockade_range = self.d.defines.ship_blockade_range;
        let (x, y) = (self.units[u].x, self.units[u].y);

        for i in -blockade_range..=blockade_range {
            for j in -blockade_range..=blockade_range {
                let Some(loop_plot) = self.plot_xy(x, y, i, j) else {
                    continue;
                };

                if !self.map.plots[loop_plot].revealed[team] {
                    continue;
                }

                let Some(city) = self.map.plots[loop_plot].plot_city else {
                    continue;
                };

                let city_team = self.city_team(city);
                if !self.cities[city].plundered && self.unit_is_enemy(u, Some(city_team), None) && !self.at_war(city_team, team) {
                    if self.city_area(city) == self.unit_area(u) || self.plot_is_adjacent_to_area(self.city_plot(city), self.unit_area(u)) {
                        // (The city's trade profit with itself, times its trade routes.)
                        let gold = self.city_calculate_trade_profit(city, city) * self.city_trade_routes(city);
                        if gold > 0 {
                            self.city_set_plundered(city, true);
                            let owner = self.units[u].owner;
                            let city_owner = self.cities[city].owner;
                            // `changeGold`
                            self.players[owner].gold += gold;
                            self.players[city_owner].gold -= gold;

                            // UI: TXT_KEY_MISC_TRADE_ROUTE_PLUNDERED to the unit owner and
                            // TXT_KEY_MISC_TRADE_ROUTE_PLUNDER to the city owner.
                            let turn = self.game_turn();
                            self.log.msg(turn, Some(owner), || format!("Plundered {} gold from the trade routes of {}", gold, self.cities[city].name));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::SelectionGroup;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    const W: i32 = 6;
    const H: i32 = 4;

    /// A game on a 6x4 map (one area) with real `Player` objects (not `player_init`ed).
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

    /// C++ `CvPlayer::addUnit` + `reset`, with the coordinates set directly (no `init`/`setXY`,
    /// so the unit is not in the plot's unit list).
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
    fn command_data_decoding() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(1), "UNIT_WARRIOR", 1, 1);
        let id = u.0 as i32;

        // COMMAND_LOAD_UNIT (owner, unit id): C++ `::getUnit(IDInfo)` is NULL for another
        // owner's id, an invalid owner or an unknown id.
        assert_eq!(command_unit(&g, 1, id), Some(u));
        assert_eq!(command_unit(&g, 0, id), None);
        assert_eq!(command_unit(&g, -1, id), None);
        assert_eq!(command_unit(&g, g.num_players() as i32, id), None);
        assert_eq!(command_unit(&g, 1, -1), None);
        assert_eq!(command_unit(&g, 1, id + 1), None);

        // COMMAND_PROMOTION leader id (-1 = none).
        assert_eq!(command_leader(-1), None);
        assert_eq!(command_leader(id), Some(u));

        // Spell commands: values outside the spell table are no spell.
        let num_spells = g.d.spells.len();
        assert_eq!(command_spell(&g, -1), None);
        assert_eq!(command_spell(&g, num_spells as i32), None);
        assert_eq!(command_spell(&g, num_spells as i32 - 1), Some(SpellType::from_usize(num_spells - 1)));
    }

    #[test]
    fn scrap_kills_with_delay() {
        let mut g = test_game();
        let d = g.d;
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WARRIOR", 2, 1);

        // FfH: no scrapping while the owner is a temporary player.
        g.players[owner].temp_player_timer = 2;
        assert!(!g.unit_can_scrap(u));
        g.players[owner].temp_player_timer = 0;
        assert!(g.unit_can_scrap(u));

        g.unit_scrap(u);
        assert!(g.units.contains(u) && g.units[u].death_delay);

        // FfH: equipment units cannot be scrapped.
        let equipment = d.units.keys().find(|&t| d.units[t].equipment_promotion.is_some()).expect("an equipment unit");
        let name = d.units[equipment].base.type_.clone();
        let e = add_unit(&mut g, owner, &name, 2, 2);
        assert!(!g.unit_can_scrap(e));
        g.unit_scrap(e);
        assert!(!g.units[e].death_delay);
    }

    #[test]
    fn gifting_is_disabled() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 2, 1);
        assert!(!g.unit_can_gift(u, false, true));
        assert!(!g.unit_can_gift(u, true, false));

        let num_units = g.units.len();
        g.unit_gift(u, true);
        assert_eq!(g.units.len(), num_units);
        assert!(!g.units[u].death_delay);
        assert_eq!(g.units[u].owner, PlayerId(0));
    }

    #[test]
    fn heal_rate_in_friendly_territory() {
        let mut g = test_game();
        let d = g.d;
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WARRIOR", 2, 1);
        let plot = g.unit_plot(u);
        g.map.plots[plot].units.push(u);
        g.map.plots[plot].owner = Some(owner);
        assert!(!g.is_option(GameOptionType::GAMEOPTION_NO_HEALING_FOR_HUMANS));

        g.units[u].extra_friendly_heal = 5;
        g.units[u].extra_neutral_heal = 100; // not used in friendly territory
        g.players[owner].heal_change = 3;
        let base = d.defines.friendly_heal_rate + 5 + 3;
        assert_eq!(g.unit_heal_rate(u, plot), base);

        // The best same-tile or adjacent-tile heal of a unit of the same team is added once.
        g.units[u].same_tile_heal = 7;
        assert_eq!(g.unit_heal_rate(u, plot), base + 7);
        let medic = add_unit(&mut g, owner, "UNIT_WARRIOR", 3, 1);
        let medic_plot = g.unit_plot(medic);
        g.map.plots[medic_plot].units.push(medic);
        g.units[medic].adjacent_tile_heal = 10;
        assert_eq!(g.unit_heal_rate(u, plot), base + 10);

        // Other teams' healers do not count.
        let other = add_unit(&mut g, PlayerId(1), "UNIT_WARRIOR", 2, 2);
        let other_plot = g.unit_plot(other);
        g.map.plots[other_plot].units.push(other);
        g.units[other].adjacent_tile_heal = 50;
        assert_eq!(g.unit_heal_rate(u, plot), base + 10);

        // healTurns: 0 when unhurt, otherwise damage / heal rounded up.
        let heal = base + 10;
        assert_eq!(g.unit_heal_turns(u, plot), 0);
        g.units[u].damage = 2 * heal;
        assert_eq!(g.unit_heal_turns(u, plot), 2);
        g.units[u].damage = 2 * heal + 1;
        assert_eq!(g.unit_heal_turns(u, plot), 3);

        // FfH: improvements add their heal rate change.
        let improvement = d
            .improvements
            .keys()
            .find(|&i| d.improvements[i].heal_rate_change > 0 && !d.improvements[i].acts_as_city)
            .expect("a healing improvement");
        g.map.plots[plot].improvement_type = Some(improvement);
        assert_eq!(g.unit_heal_rate(u, plot), heal + d.improvements[improvement].heal_rate_change);
        g.map.plots[plot].improvement_type = None;

        // FfH: the heal rate never goes below 0; then the unit never heals.
        g.players[owner].heal_change = -1000;
        assert_eq!(g.unit_heal_rate(u, plot), 0);
        assert_eq!(g.unit_heal_turns(u, plot), MAX_INT);
    }

    #[test]
    fn waiting_follows_the_group_activity() {
        let mut g = test_game();
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WARRIOR", 1, 1);
        let id = g.groups.next_id();
        let group = g.player_add_selection_group(owner, SelectionGroup::new(id, owner));
        g.groups[group].units.push(u);
        g.units[u].group = Some(group);

        assert!(!g.unit_is_waiting(u));
        g.groups[group].activity_type = ActivityType::ACTIVITY_SLEEP;
        assert!(g.unit_is_waiting(u));

        // Blockading: setting the current value again changes nothing.
        g.unit_set_blockading(u, false);
        assert!(!g.units[u].blockading);
    }
}
