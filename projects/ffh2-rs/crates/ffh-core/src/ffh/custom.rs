//! Port of CustomFunctions.py (FFH2 helper functions shared by spells, events and
//! per-turn processing). Methods are prefixed `cf_`.
//!
//! Not ported: `addPopup`, `addPlayerPopup`, `showUniqueImprovements`, `warn` (UI only);
//! `warScript`, `warScriptAllow`, `dogpile`, `dogPileAllow` (AI war policy, replaced by
//! the new AI -- see docs/PORTING.md).
//!
//! Porting notes:
//! - Python `lList[CyGame().getSorenRandNum(len(lList), s) - 1]` (lair goodies, name parts)
//!   reads the *last* element on a roll of 0 (negative index); [`py_list_index`] keeps that.
//! - `CyMap().plot(x, y)` is `self.map.plot_xy(x, y)` (wraps; `None` is the Python NULL plot).
//! - `PyPlayer(i).getUnitList()` skips dead units (`cf_unit_list`); `getCityList()` is the
//!   owner's city list in order.
//! - `pPlot.getUnit(i)` loops run `for i in range(getNumUnits(), -1, -1)`: the first index is
//!   past the end (a NULL CyUnit, whose calls do nothing), and the list is re-read every step.
//! - UI-only Python (`CyInterface().addMessage`, sounds, `CyCamera`) is omitted; the message
//!   text keys go to the game log (`cf_msg`).

use crate::ffh::game_utils::py_div;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `addBonus(iBonus, iNum, sIcon)`: place `num` of a bonus on random plots that can have it
    /// (latitude ignored) and have neither a bonus nor a city; the same plot may be drawn more
    /// than once. Python takes the bonus type name; `sIcon` only selects the (UI) message.
    pub fn cf_add_bonus(&mut self, bonus: BonusType, num: i32) {
        let mut list_plots = Vec::new();
        for plot in self.map.plot_ids() {
            if self.plot_can_have_bonus(plot, Some(bonus), true)
                && self.map.plots[plot].bonus_type.is_none()
                && self.map.plots[plot].plot_city.is_none()
            {
                list_plots.push(plot);
            }
        }
        if !list_plots.is_empty() {
            for _ in 0..num {
                let rnd = self.soren_rand_num(list_plots.len() as i32, "Add Bonus");
                let plot = list_plots[rnd as usize];
                self.plot_set_bonus_type(plot, Some(bonus));
                // UI: TXT_KEY_MESSAGE_RESOURCE_DISCOVERED to the active player.
            }
        }
    }

    /// `addUnit(iUnit)`: spawn a barbarian unit (UNITAI_ATTACK) on the best empty, passable land
    /// plot without a city: random 0..500 + 10 per tile of the plot's area, +200 in barbarian
    /// territory, then halved in owned territory.
    pub fn cf_add_unit(&mut self, unit: UnitType) {
        let mut best_plot = None;
        let mut best_value = -1;
        for plot in self.map.plot_ids() {
            if !self.plot_is_water(plot)
                && self.map.plots[plot].units.is_empty()
                && self.map.plots[plot].plot_city.is_none()
                && !self.plot_is_impassable(plot)
            {
                let mut value = self.soren_rand_num(500, "Add Unit");
                value += self.cf_plot_area_num_tiles(plot) * 10;
                if self.plot_is_barbarian(plot) {
                    value += 200;
                }
                if self.plot_is_owned(plot) {
                    value = py_div(value, 2);
                }
                if value > best_value {
                    best_value = value;
                    best_plot = Some(plot);
                }
            }
        }
        // Python: `if iBestPlot != -1` (a plot is recorded exactly when the best value rises).
        if let Some(plot) = best_plot {
            let barbarian = self.barbarian_player();
            let (x, y) = self.map.xy(plot);
            self.player_init_unit(barbarian, unit, x, y, Some(UnitAIType::UNITAI_ATTACK), Some(DirectionType::DIRECTION_SOUTH));
        }
    }

    /// `addUnitFixed(caster, iUnit)`: spawn a barbarian unit (UNITAI_ATTACK) on a clear plot next
    /// to the caster (`findClearPlot(-1, caster.plot())`); `None` (Python -1) if there is none.
    pub fn cf_add_unit_fixed(&mut self, caster: UnitId, unit: UnitType) -> Option<UnitId> {
        let plot = self.unit_plot(caster);
        let new_plot = self.cf_find_clear_plot(None, plot)?;
        let barbarian = self.barbarian_player();
        let (x, y) = self.map.xy(new_plot);
        Some(self.player_init_unit(barbarian, unit, x, y, Some(UnitAIType::UNITAI_ATTACK), Some(DirectionType::DIRECTION_SOUTH)))
    }

    /// `doCrusade(iPlayer)` (Crusade civic, every turn): each of the player's towns without a
    /// visible enemy unit spawns a Demagog with `CRUSADE_SPAWN_CHANCE`% and shrinks to a village.
    pub fn cf_do_crusade(&mut self, p: PlayerId) {
        let d = self.d;
        let crusade_chance = d.defines.crusade_spawn_chance;
        let demagog = d.t::<UnitType>("UNIT_DEMAGOG");
        let town = d.t::<ImprovementType>("IMPROVEMENT_TOWN");
        let village = d.t::<ImprovementType>("IMPROVEMENT_VILLAGE");
        for plot in self.map.plot_ids() {
            if !self.plot_is_visible_enemy_unit(plot, p)
                && self.map.plots[plot].improvement_type == Some(town)
                && self.map.plots[plot].owner == Some(p)
                && self.soren_rand_num(100, "Crusade") < crusade_chance
            {
                let (x, y) = self.map.xy(plot);
                self.player_init_unit(p, demagog, x, y, Some(UnitAIType::UNITAI_ATTACK), Some(DirectionType::DIRECTION_SOUTH));
                self.plot_set_improvement_type(plot, Some(village));
            }
        }
    }

    /// `doFear(pVictim, pPlot, pCaster, bResistable)`: the victim flees to a random adjacent plot
    /// it can enter that is not next to `plot` (the caster's plot); true if it fled.
    pub fn cf_do_fear(&mut self, victim: UnitId, plot: PlotIdx, caster: UnitId, resistable: bool) -> bool {
        if self.unit_is_immune_to_fear(victim) {
            return false;
        }
        if resistable {
            // Python quirk: the resist roll is logged as "Crusade".
            let roll = self.soren_rand_num(100, "Crusade");
            if roll < self.unit_get_resist_chance(victim, caster, self.d.t::<SpellType>("SPELL_ROAR")) {
                return false;
            }
        }
        let (x, y) = (self.units[victim].x, self.units[victim].y);
        let owner = self.units[victim].owner;
        let (plot_x, plot_y) = self.map.xy(plot);
        let mut best_plot = None;
        let mut best_value = 0;
        for xx in x - 1..x + 2 {
            for yy in y - 1..y + 2 {
                let Some(loop_plot) = self.map.plot_xy(xx, yy) else { continue };
                if !self.plot_is_visible_enemy_unit(loop_plot, owner) && self.unit_can_move_or_attack_into(victim, loop_plot, false) {
                    let (loop_x, loop_y) = self.map.xy(loop_plot);
                    if (loop_x - plot_x).abs() > 1 || (loop_y - plot_y).abs() > 1 {
                        let rnd = self.soren_rand_num(500, "Fear");
                        if rnd > best_value {
                            best_value = rnd;
                            best_plot = Some(loop_plot);
                        }
                    }
                }
            }
        }
        if let Some(best) = best_plot {
            let (best_x, best_y) = self.map.xy(best);
            self.unit_set_xy(victim, best_x, best_y, false);
            return true;
        }
        false
    }

    /// `exploreLairBigBad(caster)`: a named barbarian monster with a random promotion appears next
    /// to the caster, with 0-4 henchmen. Returns the Python result (chance in % that the lair is
    /// destroyed afterwards): always 0.
    pub fn cf_explore_lair_big_bad(&mut self, caster: UnitId) -> i32 {
        let d = self.d;
        let p = self.units[caster].owner;
        let plot = self.unit_plot(caster);

        let mut list: Vec<&'static str> = vec!["UNIT_AZER"];
        let mut promo_list: Vec<&'static str> = vec![
            "PROMOTION_MUTATED",
            "PROMOTION_CANNIBALIZE",
            "PROMOTION_MOBILITY1",
            "PROMOTION_STRONG",
            "PROMOTION_BLITZ",
            "PROMOTION_COMMAND1",
            "PROMOTION_HEROIC_STRENGTH",
            "PROMOTION_HEROIC_DEFENSE",
            "PROMOTION_MAGIC_IMMUNE",
            "PROMOTION_STONESKIN",
            "PROMOTION_VALOR",
            "PROMOTION_VILE_TOUCH",
        ];
        let mut henchman_list: Vec<&'static str> = vec!["UNIT_AZER", "UNIT_GRIFFON"];
        if !self.cf_grace() {
            list.push("UNIT_AIR_ELEMENTAL");
        }
        if !self.plot_is_water(plot) {
            list.extend_from_slice(&["UNIT_ASSASSIN", "UNIT_OGRE", "UNIT_GIANT_SPIDER", "UNIT_HILL_GIANT", "UNIT_SPECTRE", "UNIT_SCORPION"]);
            henchman_list.extend_from_slice(&[
                "UNIT_AXEMAN",
                "UNIT_WOLF",
                "UNIT_CHAOS_MARAUDER",
                "UNIT_WOLF_RIDER",
                "UNIT_MISTFORM",
                "UNIT_LION",
                "UNIT_TIGER",
                "UNIT_BABY_SPIDER",
                "UNIT_FAWN",
                "UNIT_SCORPION",
            ]);
            if !self.cf_grace() {
                list.extend_from_slice(&[
                    "UNIT_EARTH_ELEMENTAL",
                    "UNIT_FIRE_ELEMENTAL",
                    "UNIT_GARGOYLE",
                    "UNIT_VAMPIRE",
                    "UNIT_MYCONID",
                    "UNIT_EIDOLON",
                    "UNIT_LICH",
                    "UNIT_OGRE_WARCHIEF",
                    "UNIT_SATYR",
                    "UNIT_WEREWOLF",
                ]);
                promo_list.extend_from_slice(&["PROMOTION_FIRE2", "PROMOTION_AIR2", "PROMOTION_HERO", "PROMOTION_MARKSMAN", "PROMOTION_SHADOWWALK"]);
                henchman_list.push("UNIT_OGRE");
                if self.map.plots[plot].feature_type == Some(d.t::<FeatureType>("FEATURE_FOREST")) {
                    list.push("UNIT_TREANT");
                }
            }
            if self.map.plots[plot].terrain_type == Some(d.t::<TerrainType>("TERRAIN_SNOW")) {
                henchman_list.extend_from_slice(&["UNIT_FROSTLING_ARCHER", "UNIT_FROSTLING_WOLF_RIDER", "UNIT_POLAR_BEAR"]);
            }
            if self.map.plots[plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_BARROW")) {
                promo_list.push("PROMOTION_DEATH2");
                henchman_list.extend_from_slice(&["UNIT_SKELETON", "UNIT_PYRE_ZOMBIE"]);
                if !self.cf_grace() {
                    list.push("UNIT_WRAITH");
                }
            }
            if self.map.plots[plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_RUINS")) {
                promo_list.push("PROMOTION_POISONED_BLADE");
                henchman_list.extend_from_slice(&["UNIT_LIZARDMAN", "UNIT_GORILLA"]);
                if !self.cf_grace() {
                    list.push("UNIT_MANTICORE");
                }
            }
            if self.get_global_counter() > 40 {
                list.extend_from_slice(&["UNIT_PIT_BEAST", "UNIT_DEATH_KNIGHT", "UNIT_BALOR"]);
                promo_list.push("PROMOTION_FEAR");
                henchman_list.extend_from_slice(&["UNIT_IMP", "UNIT_HELLHOUND"]);
            }
        }
        if self.plot_is_water(plot) {
            list.extend_from_slice(&["UNIT_SEA_SERPENT", "UNIT_STYGIAN_GUARD", "UNIT_PIRATE"]);
            henchman_list.push("UNIT_DROWN");
            if !self.cf_grace() {
                list.extend_from_slice(&["UNIT_WATER_ELEMENTAL", "UNIT_KRAKEN"]);
            }
        }

        let monster = self.cf_pick(&list, "Pick Monster");
        let henchman = self.cf_pick(&henchman_list, "Pick Henchman");
        let promo = self.cf_pick(&promo_list, "Pick Promotion");
        let unit = d.t::<UnitType>(monster);
        let henchman = d.t::<UnitType>(henchman);
        if let Some(new_unit) = self.cf_add_unit_fixed(caster, unit) {
            self.unit_set_has_promotion(new_unit, d.t::<PromotionType>(promo), true);
            let name = self.cf_marnok_name_generator(new_unit);
            self.unit_set_name(new_unit, name);
            self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_BIGBAD");
            let barbarian = self.barbarian_player();
            let (x, y) = (self.units[new_unit].x, self.units[new_unit].y);
            for _ in 0..self.soren_rand_num(5, "Pick Henchmen") {
                self.player_init_unit(barbarian, henchman, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
            }
        }
        0
    }

    /// `exploreLairBad(caster)`: a bad lair result (damage, death, an affliction promotion or a
    /// hostile spawn). Returns the lair destruction chance in %.
    pub fn cf_explore_lair_bad(&mut self, caster: UnitId) -> i32 {
        let d = self.d;
        let p = self.units[caster].owner;
        let plot = self.unit_plot(caster);

        let mut list: Vec<&'static str> = vec!["COLLAPSE"];
        if self.units[caster].level == 1 {
            list.push("DEATH");
        }
        if self.unit_is_alive(caster) {
            list.extend_from_slice(&["CRAZED", "DEMONIC_POSSESSION", "DISEASED", "ENRAGED", "PLAGUED", "POISONED", "WITHERED"]);
        }
        if self.unit_get_unit_combat_type(caster) == Some(d.t::<UnitCombatType>("UNITCOMBAT_MELEE")) {
            list.push("RUSTED");
        }
        if self.plot_is_water(plot) {
            list.extend_from_slice(&["SPAWN_DROWN", "SPAWN_SEA_SERPENT"]);
        }
        if !self.plot_is_water(plot) {
            list.extend_from_slice(&["SPAWN_SPIDER", "SPAWN_SPECTRE"]);
        }
        if self.map.plots[plot].improvement_type == Some(d.t::<ImprovementType>("IMPROVEMENT_GOBLIN_FORT")) {
            list.extend_from_slice(&["SPAWN_SCORPION_BAD", "SPAWN_SCORPION_BAD", "SPAWN_SCORPION_BAD"]);
        }

        let promotion = |name: &str| d.t::<PromotionType>(name);
        match self.cf_pick(&list, "Pick Goody") {
            "DEATH" => {
                // Python quirk: `caster.kill(True, 0)` names player 0 as the killer.
                self.unit_kill(caster, true, Some(PlayerId(0)));
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_DEATH");
                0
            }
            "COLLAPSE" => {
                self.unit_do_damage(caster, 50, 90, None, Some(d.t::<DamageType>("DAMAGE_PHYSICAL")), false);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_COLLAPSE");
                100
            }
            "CRAZED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_CRAZED"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_CRAZED");
                80
            }
            "DEMONIC_POSSESSION" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_ENRAGED"), true);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_CRAZED"), true);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_DEMON"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_POSSESSED");
                80
            }
            "DISEASED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_DISEASED"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_DISEASED");
                80
            }
            "ENRAGED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_ENRAGED"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_ENRAGED");
                80
            }
            "PLAGUED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_PLAGUED"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_PLAGUED");
                80
            }
            "POISONED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_POISONED"), true);
                self.unit_do_damage(caster, 25, 90, None, Some(d.t::<DamageType>("DAMAGE_POISON")), false);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_POISONED");
                80
            }
            "WITHERED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_WITHERED"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_WITHERED");
                80
            }
            "RUSTED" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_RUSTED"), true);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_BRONZE_WEAPONS"), false);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_IRON_WEAPONS"), false);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_MITHRIL_WEAPONS"), false);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_RUSTED");
                80
            }
            "SPAWN_DROWN" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_DROWN", caster);
                50
            }
            "SPAWN_SCORPION_BAD" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_SCORPION_BAD", caster);
                50
            }
            "SPAWN_SEA_SERPENT" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_SEA_SERPENT", caster);
                50
            }
            "SPAWN_SPECTRE" => {
                self.cf_receive_goody(p, plot, "GOODY_GRAVE_SPECTRE", caster);
                50
            }
            "SPAWN_SPIDER" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_SPIDER", caster);
                50
            }
            _ => 100,
        }
    }

    /// `exploreLairNeutral(caster)`: nothing, a trap (cage), a mutation, a spawn or one of the
    /// lair events (portal, depths, dwarf vs lizardmen). Returns the lair destruction chance in %.
    pub fn cf_explore_lair_neutral(&mut self, caster: UnitId) -> i32 {
        let d = self.d;
        let p = self.units[caster].owner;
        let plot = self.unit_plot(caster);

        let mut list: Vec<&'static str> = vec!["NOTHING"];
        if !self.plot_is_water(plot) {
            list.extend_from_slice(&["SPAWN_SKELETON", "SPAWN_LIZARDMAN", "SPAWN_SPIDER", "PORTAL", "DEPTHS", "DWARF_VS_LIZARDMEN", "CAGE"]);
            if self.map.plots[plot].terrain_type == Some(d.t::<TerrainType>("TERRAIN_SNOW")) {
                list.push("SPAWN_FROSTLING");
            }
            let improvement = self.map.plots[plot].improvement_type;
            if improvement == Some(d.t::<ImprovementType>("IMPROVEMENT_BARROW")) {
                list.extend_from_slice(&["SPAWN_SKELETON", "SPAWN_SKELETON"]);
            }
            if improvement == Some(d.t::<ImprovementType>("IMPROVEMENT_RUINS")) {
                list.extend_from_slice(&["SPAWN_LIZARDMAN", "SPAWN_LIZARDMAN"]);
            }
            if improvement == Some(d.t::<ImprovementType>("IMPROVEMENT_GOBLIN_FORT")) {
                list.extend_from_slice(&["SPAWN_SCORPION", "SPAWN_SCORPION", "SPAWN_SCORPION"]);
            }
        }
        if self.plot_is_water(plot) {
            list.push("SPAWN_DROWN");
        }
        if self.unit_is_alive(caster) && !self.units[caster].has_promotion[d.t::<PromotionType>("PROMOTION_MUTATED")] {
            list.push("MUTATED");
        }

        match self.cf_pick(&list, "Pick Goody") {
            "CAGE" => {
                self.plot_set_improvement_type(plot, Some(d.t::<ImprovementType>("IMPROVEMENT_CAGE")));
                let held = d.t::<PromotionType>("PROMOTION_HELD");
                for i in (0..=self.map.plots[plot].units.len()).rev() {
                    if let Some(&unit) = self.map.plots[plot].units.get(i) {
                        self.unit_set_has_promotion(unit, held, true);
                    }
                }
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_CAGE");
                0
            }
            "DEPTHS" => {
                self.cf_lair_event(caster, "EVENTTRIGGER_EXPLORE_LAIR_DEPTHS");
                100
            }
            "DWARF_VS_LIZARDMEN" => {
                self.cf_lair_event(caster, "EVENTTRIGGER_EXPLORE_LAIR_DWARF_VS_LIZARDMEN");
                100
            }
            "MUTATED" => {
                self.unit_set_has_promotion(caster, d.t::<PromotionType>("PROMOTION_MUTATED"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_MUTATED");
                50
            }
            "NOTHING" => {
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_NOTHING");
                100
            }
            "PORTAL" => {
                self.cf_lair_event(caster, "EVENTTRIGGER_EXPLORE_LAIR_PORTAL");
                0
            }
            "SPAWN_DROWN" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_DROWN", caster);
                50
            }
            "SPAWN_FROSTLING" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_FROSTLING", caster);
                50
            }
            "SPAWN_LIZARDMAN" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_LIZARDMAN", caster);
                50
            }
            "SPAWN_SCORPION" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_SCORPION", caster);
                50
            }
            "SPAWN_SKELETON" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_SKELETON", caster);
                50
            }
            "SPAWN_SPIDER" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SPAWN_SPIDER", caster);
                50
            }
            _ => 100,
        }
    }

    /// `exploreLairGood(caster)`: gold, treasure, experience, supplies, an item, a captive
    /// disciple or an equipment/weapon promotion. Returns the lair destruction chance in %.
    pub fn cf_explore_lair_good(&mut self, caster: UnitId) -> i32 {
        let d = self.d;
        let p = self.units[caster].owner;
        let plot = self.unit_plot(caster);
        let has = |g: &Game, name: &str| g.units[caster].has_promotion[d.t::<PromotionType>(name)];
        let combat = self.unit_get_unit_combat_type(caster);
        let is_combat = |name: &str| combat == Some(d.t::<UnitCombatType>(name));

        let mut list: Vec<&'static str> = vec!["HIGH_GOLD", "TREASURE", "EXPERIENCE"];
        if self.unit_is_alive(caster) && !has(self, "PROMOTION_SPIRIT_GUIDE") {
            list.push("SPIRIT_GUIDE");
        }
        if !self.plot_is_water(plot) {
            list.extend_from_slice(&["ITEM_HEALING_SALVE", "SUPPLIES"]);
            if self.player_is_has_tech(p, d.t::<TechType>("TECH_MYSTICISM")) {
                list.extend_from_slice(&[
                    "PRISONER_DISCIPLE_ASHEN",
                    "PRISONER_DISCIPLE_EMPYREAN",
                    "PRISONER_DISCIPLE_LEAVES",
                    "PRISONER_DISCIPLE_OVERLORDS",
                    "PRISONER_DISCIPLE_RUNES",
                    "PRISONER_DISCIPLE_ORDER",
                ]);
            }
        }
        if is_combat("UNITCOMBAT_MELEE") && !has(self, "PROMOTION_ENCHANTED_BLADE") {
            list.push("ENCHANTED_BLADE");
        }
        if is_combat("UNITCOMBAT_ADEPT") && !has(self, "PROMOTION_SPELLSTAFF") {
            list.push("SPELLSTAFF");
        }
        if is_combat("UNITCOMBAT_RECON") && !has(self, "PROMOTION_POISONED_BLADE") {
            list.push("POISONED_BLADE");
        }
        if is_combat("UNITCOMBAT_ARCHER") && !has(self, "PROMOTION_FLAMING_ARROWS") {
            list.push("FLAMING_ARROWS");
        }
        if is_combat("UNITCOMBAT_DISCIPLE") && !has(self, "PROMOTION_SHIELD_OF_FAITH") {
            list.push("SHIELD_OF_FAITH");
        }
        let weapon_tier = d.units[self.units[caster].unit_type].weapon_tier;
        if weapon_tier >= 1 && !has(self, "PROMOTION_MITHRIL_WEAPONS") {
            if weapon_tier >= 3 && self.player_is_has_tech(p, d.t::<TechType>("TECH_IRON_WORKING")) {
                list.push("MITHRIL_WEAPONS");
            }
            if !has(self, "PROMOTION_IRON_WEAPONS") {
                if weapon_tier >= 2 && self.player_is_has_tech(p, d.t::<TechType>("TECH_BRONZE_WORKING")) {
                    list.push("IRON_WEAPONS");
                }
                if !has(self, "PROMOTION_BRONZE_WEAPONS") {
                    list.push("BRONZE_WEAPONS");
                }
            }
        }

        let promotion = |name: &str| d.t::<PromotionType>(name);
        match self.cf_pick(&list, "Pick Goody") {
            "HIGH_GOLD" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_HIGH_GOLD", caster);
                90
            }
            "SUPPLIES" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_SUPPLIES", caster);
                100
            }
            "TREASURE" => {
                self.cf_place_treasure(p, d.t::<UnitType>("EQUIPMENT_TREASURE"));
                80
            }
            "EXPERIENCE" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_EXPERIENCE", caster);
                100
            }
            "SPIRIT_GUIDE" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_SPIRIT_GUIDE"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_SPIRIT_GUIDE");
                80
            }
            "ITEM_HEALING_SALVE" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_ITEM_HEALING_SALVE", caster);
                100
            }
            // The two potion results are never put in the list (dead code in the Python).
            "ITEM_POTION_OF_INVISIBILITY" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_ITEM_POTION_OF_INVISIBILITY", caster);
                100
            }
            "ITEM_POTION_OF_RESTORATION" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_ITEM_POTION_OF_RESTORATION", caster);
                100
            }
            "ENCHANTED_BLADE" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_ENCHANTED_BLADE"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_ENCHANTED_BLADE");
                100
            }
            "SPELLSTAFF" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_SPELLSTAFF"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_SPELLSTAFF");
                100
            }
            "POISONED_BLADE" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_POISONED_BLADE"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_POISONED_BLADE");
                100
            }
            "FLAMING_ARROWS" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_FLAMING_ARROWS"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_FLAMING_ARROWS");
                100
            }
            "PRISONER_DISCIPLE_ASHEN" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_PRISONER_DISCIPLE_ASHEN", caster);
                100
            }
            "PRISONER_DISCIPLE_EMPYREAN" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_PRISONER_DISCIPLE_EMPYREAN", caster);
                100
            }
            "PRISONER_DISCIPLE_LEAVES" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_PRISONER_DISCIPLE_LEAVES", caster);
                100
            }
            "PRISONER_DISCIPLE_OVERLORDS" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_PRISONER_DISCIPLE_OVERLORDS", caster);
                100
            }
            "PRISONER_DISCIPLE_RUNES" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_PRISONER_DISCIPLE_RUNES", caster);
                100
            }
            "PRISONER_DISCIPLE_ORDER" => {
                self.cf_receive_goody(p, plot, "GOODY_EXPLORE_LAIR_PRISONER_DISCIPLE_ORDER", caster);
                100
            }
            "SHIELD_OF_FAITH" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_SHIELD_OF_FAITH"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_SHIELD_OF_FAITH");
                100
            }
            "BRONZE_WEAPONS" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_BRONZE_WEAPONS"), true);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_BRONZE_WEAPONS");
                if has(self, "PROMOTION_RUSTED") {
                    self.unit_set_has_promotion(caster, promotion("PROMOTION_RUSTED"), false);
                }
                100
            }
            "IRON_WEAPONS" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_IRON_WEAPONS"), true);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_BRONZE_WEAPONS"), false);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_IRON_WEAPONS");
                if has(self, "PROMOTION_RUSTED") {
                    self.unit_set_has_promotion(caster, promotion("PROMOTION_RUSTED"), false);
                }
                100
            }
            "MITHRIL_WEAPONS" => {
                self.unit_set_has_promotion(caster, promotion("PROMOTION_MITHRIL_WEAPONS"), true);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_IRON_WEAPONS"), false);
                self.unit_set_has_promotion(caster, promotion("PROMOTION_BRONZE_WEAPONS"), false);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_MITHRIL_WEAPONS");
                if has(self, "PROMOTION_RUSTED") {
                    self.unit_set_has_promotion(caster, promotion("PROMOTION_RUSTED"), false);
                }
                100
            }
            _ => 100,
        }
    }

    /// `exploreLairBigGood(caster)`: a treasure vault, a golden age, a tech, an artifact, a
    /// captive great person/unit or a new resource on the plot. Returns the lair destruction
    /// chance in % (always 100).
    pub fn cf_explore_lair_big_good(&mut self, caster: UnitId) -> i32 {
        let d = self.d;
        let p = self.units[caster].owner;
        let plot = self.unit_plot(caster);

        let mut list: Vec<&'static str> = vec!["TREASURE_VAULT", "GOLDEN_AGE"];
        if self.player_can_receive_goody(p, plot, d.t::<GoodyType>("GOODY_GRAVE_TECH"), Some(caster)) {
            list.push("TECH");
        }
        if !self.plot_is_water(plot) {
            list.extend_from_slice(&[
                "ITEM_JADE_TORC",
                "ITEM_ROD_OF_WINDS",
                "ITEM_TIMOR_MASK",
                "PRISONER_ADVENTURER",
                "PRISONER_ARTIST",
                "PRISONER_COMMANDER",
                "PRISONER_ENGINEER",
                "PRISONER_MERCHANT",
                "PRISONER_PROPHET",
                "PRISONER_SCIENTIST",
            ]);
            if self.map.plots[plot].bonus_type.is_none() {
                list.push("BONUS_MANA");
                if self.player_is_has_tech(p, d.t::<TechType>("TECH_MINING")) {
                    list.extend_from_slice(&["BONUS_COPPER", "BONUS_GEMS", "BONUS_GOLD"]);
                }
                if self.player_is_has_tech(p, d.t::<TechType>("TECH_SMELTING")) {
                    list.push("BONUS_IRON");
                }
            }
        }
        if self.plot_is_water(plot) {
            list.push("PRISONER_SEA_SERPENT");
            if self.map.plots[plot].bonus_type.is_none() {
                list.extend_from_slice(&["BONUS_CLAM", "BONUS_CRAB", "BONUS_FISH"]);
            }
        }
        if !self.cf_grace() {
            list.extend_from_slice(&["PRISONER_ANGEL", "PRISONER_MONK", "PRISONER_ASSASSIN", "PRISONER_CHAMPION", "PRISONER_MAGE"]);
        }

        let goody = self.cf_pick(&list, "Pick Goody");
        let goody_type = match goody {
            "TREASURE_VAULT" => "GOODY_EXPLORE_LAIR_TREASURE_VAULT",
            "BONUS_CLAM" | "BONUS_COPPER" | "BONUS_CRAB" | "BONUS_FISH" | "BONUS_GOLD" | "BONUS_GEMS" | "BONUS_IRON" | "BONUS_MANA" => {
                // These results are the bonus type names.
                self.plot_set_bonus_type(plot, Some(d.t::<BonusType>(goody)));
                return 100;
            }
            "GOLDEN_AGE" => {
                let length = self.golden_age_length();
                self.player_change_golden_age_turns(p, length);
                self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_GOLDEN_AGE");
                return 100;
            }
            "ITEM_JADE_TORC" => "GOODY_EXPLORE_LAIR_ITEM_JADE_TORC",
            "ITEM_ROD_OF_WINDS" => "GOODY_EXPLORE_LAIR_ITEM_ROD_OF_WINDS",
            "ITEM_TIMOR_MASK" => "GOODY_EXPLORE_LAIR_ITEM_TIMOR_MASK",
            "PRISONER_ADVENTURER" => "GOODY_EXPLORE_LAIR_PRISONER_ADVENTURER",
            "PRISONER_ANGEL" => "GOODY_EXPLORE_LAIR_PRISONER_ANGEL",
            "PRISONER_ARTIST" => "GOODY_EXPLORE_LAIR_PRISONER_ARTIST",
            "PRISONER_ASSASSIN" => "GOODY_EXPLORE_LAIR_PRISONER_ASSASSIN",
            "PRISONER_CHAMPION" => "GOODY_EXPLORE_LAIR_PRISONER_CHAMPION",
            "PRISONER_COMMANDER" => "GOODY_EXPLORE_LAIR_PRISONER_COMMANDER",
            "PRISONER_ENGINEER" => "GOODY_EXPLORE_LAIR_PRISONER_ENGINEER",
            "PRISONER_MAGE" => "GOODY_EXPLORE_LAIR_PRISONER_MAGE",
            "PRISONER_MERCHANT" => "GOODY_EXPLORE_LAIR_PRISONER_MERCHANT",
            "PRISONER_MONK" => "GOODY_EXPLORE_LAIR_PRISONER_MONK",
            "PRISONER_PROPHET" => "GOODY_EXPLORE_LAIR_PRISONER_PROPHET",
            "PRISONER_SEA_SERPENT" => "GOODY_EXPLORE_LAIR_PRISONER_SEA_SERPENT",
            "PRISONER_SCIENTIST" => "GOODY_EXPLORE_LAIR_PRISONER_SCIENTIST",
            "TECH" => "GOODY_GRAVE_TECH",
            _ => return 100,
        };
        self.cf_receive_goody(p, plot, goody_type, caster);
        100
    }

    /// `formEmpire(iCiv, iLeader, iTeam, pCity, iAlignment, pFromPlayer)`: a new player (first
    /// never-alive slot) on `team` (None: a fresh team) takes over `city` with four archers and
    /// a worker; the units in the city are moved to a clear adjacent plot first. With
    /// `from_player`, the new team gets every tech of that player's team.
    pub fn cf_form_empire(
        &mut self,
        civ: CivilizationType,
        leader: LeaderHeadType,
        team: Option<TeamId>,
        city: CityId,
        alignment: Option<AlignmentType>,
        from_player: Option<PlayerId>,
    ) {
        let d = self.d;
        let new_player = self.cf_get_open_player();
        let plot = self.city_plot(city);
        let plot2 = self.cf_find_clear_plot(None, plot);
        let (Some(new_player), Some(plot2)) = (new_player, plot2) else { return };
        let (x2, y2) = self.map.xy(plot2);
        for i in (0..=self.map.plots[plot].units.len()).rev() {
            if let Some(&unit) = self.map.plots[plot].units.get(i) {
                self.unit_set_xy(unit, x2, y2, true);
            }
        }
        self.add_player_advanced(new_player, team, leader, civ);
        self.player_acquire_city(new_player, city, false, false, true);
        // `pPlot.getPlotCity()`: the acquired city has a new id (a NULL city would ignore the call).
        if let Some(new_city) = self.map.plots[plot].plot_city {
            self.city_change_culture(new_city, new_player, 100, true, true);
        }
        let (x, y) = self.map.xy(plot);
        let archer = d.t::<UnitType>("UNIT_ARCHER");
        let worker = d.t::<UnitType>("UNIT_WORKER");
        for unit in [archer, archer, archer, archer, worker] {
            self.player_init_unit(new_player, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
        }
        if let Some(alignment) = alignment {
            self.players[new_player].alignment = Some(alignment);
        }
        if let Some(from_player) = from_player {
            let from_team = self.players[from_player].team;
            let new_team = self.players[new_player].team;
            for tech in d.techs.keys() {
                if self.team_is_has_tech(from_team, Some(tech)) {
                    self.team_set_has_tech(new_team, tech, true, Some(new_player), true, false);
                }
            }
        }
    }

    /// `grace()`: true during the early-game grace period, `20 * (speed + 1) * (number of
    /// handicaps + 1 - handicap)` turns plus a random 0..that (one RNG draw per call).
    pub fn cf_grace(&mut self) -> bool {
        let mut grace = 20 * (self.game_speed_type().i() + 1);
        let diff = self.d.handicaps.len() as i32 + 1 - self.handicap.i();
        grace *= diff;
        grace = self.soren_rand_num(grace, "grace") + grace;
        grace > self.game_turn()
    }

    /// `doCityFire(pCity)`: each real building (except Demonic Citizens and buildings that always
    /// survive conquest) burns down with an 11% chance.
    pub fn cf_do_city_fire(&mut self, city: CityId) {
        let d = self.d;
        let owner = self.cities[city].owner;
        let mut count = 0;
        let demon = d.t::<BuildingType>("BUILDING_DEMONIC_CITIZENS");
        for building in d.buildings.keys() {
            if building != demon
                && self.cities[city].num_real_building[building] > 0
                && d.buildings[building].conquest_probability != 100
                && self.soren_rand_num(100, "City Fire") <= 10
            {
                self.city_set_num_real_building(city, building, 0);
                let turn = self.game_turn();
                self.log.msg(turn, Some(owner), || {
                    d.text.get("TXT_KEY_MESSAGE_CITY_FIRE").replace("%s1_building", d.text.name(&d.buildings[building].base.description))
                });
                count += 1;
            }
        }
        if count == 0 {
            self.cf_msg(owner, "TXT_KEY_MESSAGE_CITY_FIRE_NO_DAMAGE");
        }
    }

    /// `doHellTerrain()` (every turn): the Armageddon plot counters spread from hell plots (>10)
    /// to owned plots of Ashen Veil / evil (counter >= 50) / neutral (counter >= 75) players and,
    /// past 25, to unowned plots; Infernal land is raised by 100; other plots cool down by 1.
    /// Bonuses and food resources turn hellish above 9 and back below 10; burning sands catch fire.
    pub fn cf_do_hell_terrain(&mut self) {
        let d = self.d;
        let ashen_veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
        let burning_sands = d.t::<TerrainType>("TERRAIN_BURNING_SANDS");
        let banana = d.t::<BonusType>("BONUS_BANANA");
        let cotton = d.t::<BonusType>("BONUS_COTTON");
        let corn = d.t::<BonusType>("BONUS_CORN");
        let cow = d.t::<BonusType>("BONUS_COW");
        let evil = d.t::<AlignmentType>("ALIGNMENT_EVIL");
        let farm = d.t::<ImprovementType>("IMPROVEMENT_FARM");
        let flames = d.t::<FeatureType>("FEATURE_FLAMES");
        let flames_spread_chance = d.defines.flames_spread_chance;
        let gulagarm = d.t::<BonusType>("BONUS_GULAGARM");
        let horse = d.t::<BonusType>("BONUS_HORSE");
        let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
        let marble = d.t::<BonusType>("BONUS_MARBLE");
        let neutral = d.t::<AlignmentType>("ALIGNMENT_NEUTRAL");
        let nightmare = d.t::<BonusType>("BONUS_NIGHTMARE");
        let pig = d.t::<BonusType>("BONUS_PIG");
        let razorweed = d.t::<BonusType>("BONUS_RAZORWEED");
        let rice = d.t::<BonusType>("BONUS_RICE");
        let sheep = d.t::<BonusType>("BONUS_SHEEP");
        let sheut_stone = d.t::<BonusType>("BONUS_SHEUT_STONE");
        let silk = d.t::<BonusType>("BONUS_SILK");
        let snake_pillar = d.t::<ImprovementType>("IMPROVEMENT_SNAKE_PILLAR");
        let sugar = d.t::<BonusType>("BONUS_SUGAR");
        let toad = d.t::<BonusType>("BONUS_TOAD");
        let wheat = d.t::<BonusType>("BONUS_WHEAT");
        let mut count = self.get_global_counter();
        for plot in self.map.plot_ids() {
            let terrain = self.map.plots[plot].terrain_type;
            let bonus = self.map.plots[plot].bonus_type;
            let improvement = self.map.plots[plot].improvement_type;
            let mut untouched = true;
            if let Some(owner) = self.map.plots[plot].owner {
                let alignment = self.players[owner].alignment;
                if self.player_get_civilization_type(owner) == Some(infernal) {
                    self.plot_change_plot_counter(plot, 100);
                    untouched = false;
                }
                // Python precedence: `(a and b) or c or d`.
                if (untouched && self.player_get_state_religion(owner) == Some(ashen_veil))
                    || (count >= 50 && alignment == Some(evil))
                    || (count >= 75 && alignment == Some(neutral))
                {
                    if self.cf_hell_spread_from_adjacent(plot) {
                        untouched = false;
                    }
                }
            }
            if untouched && !self.plot_is_owned(plot) && count > 25 && self.cf_hell_spread_from_adjacent(plot) {
                untouched = false;
            }
            let plot_count = self.map.plots[plot].plot_counter;
            if untouched && plot_count > 0 {
                self.plot_change_plot_counter(plot, -1);
            }
            if plot_count > 9 {
                if bonus == Some(sheep) || bonus == Some(pig) {
                    self.plot_set_bonus_type(plot, Some(toad));
                }
                if bonus == Some(horse) || bonus == Some(cow) {
                    self.plot_set_bonus_type(plot, Some(nightmare));
                }
                if bonus == Some(cotton) || bonus == Some(silk) {
                    self.plot_set_bonus_type(plot, Some(razorweed));
                }
                if bonus == Some(banana) || bonus == Some(sugar) {
                    self.plot_set_bonus_type(plot, Some(gulagarm));
                }
                if bonus == Some(marble) {
                    self.plot_set_bonus_type(plot, Some(sheut_stone));
                }
                if bonus == Some(corn) || bonus == Some(rice) || bonus == Some(wheat) {
                    self.plot_set_bonus_type(plot, None);
                    self.plot_set_improvement_type(plot, Some(snake_pillar));
                }
            }
            if plot_count < 10 {
                if bonus == Some(toad) {
                    let b = if self.soren_rand_num(100, "Hell Convert") < 50 { sheep } else { pig };
                    self.plot_set_bonus_type(plot, Some(b));
                }
                if bonus == Some(nightmare) {
                    let b = if self.soren_rand_num(100, "Hell Convert") < 50 { horse } else { cow };
                    self.plot_set_bonus_type(plot, Some(b));
                }
                if bonus == Some(razorweed) {
                    let b = if self.soren_rand_num(100, "Hell Convert") < 50 { cotton } else { silk };
                    self.plot_set_bonus_type(plot, Some(b));
                }
                if bonus == Some(gulagarm) {
                    let b = if self.soren_rand_num(100, "Hell Convert") < 50 { banana } else { sugar };
                    self.plot_set_bonus_type(plot, Some(b));
                }
                if bonus == Some(sheut_stone) {
                    self.plot_set_bonus_type(plot, Some(marble));
                }
                if improvement == Some(snake_pillar) {
                    self.plot_set_improvement_type(plot, Some(farm));
                    // Python quirk: the roll is stored in `iCount`, the Armageddon counter used by
                    // the spread tests of all later plots this turn.
                    count = self.soren_rand_num(100, "Hell Convert");
                    let b = if count < 33 {
                        corn
                    } else if count < 66 {
                        rice
                    } else {
                        wheat
                    };
                    self.plot_set_bonus_type(plot, Some(b));
                }
            }
            if terrain == Some(burning_sands)
                && self.map.plots[plot].plot_city.is_none()
                && !self.plot_is_peak(plot)
                && self.soren_rand_num(100, "Flames") <= flames_spread_chance
            {
                self.plot_set_feature_type(plot, Some(flames));
            }
        }
    }

    /// `doTurnKhazad(iPlayer)`: every city gets the Dwarven Vault level matching the player's
    /// gold per city (<50, <100, <150, <200, <300, <500, 500+).
    pub fn cf_do_turn_khazad(&mut self, p: PlayerId) {
        if self.players[p].num_cities() > 0 {
            let d = self.d;
            let vaults = [
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT_EMPTY"),
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT_LOW"),
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT"),
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT_STOCKED"),
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT_ABUNDANT"),
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT_FULL"),
                d.t::<BuildingType>("BUILDING_DWARVEN_VAULT_OVERFLOWING"),
            ];
            let gold = py_div(self.players[p].gold, self.players[p].num_cities());
            // The Python ranges cover every value.
            let new_vault = if gold <= 49 {
                vaults[0]
            } else if gold <= 99 {
                vaults[1]
            } else if gold <= 149 {
                vaults[2]
            } else if gold <= 199 {
                vaults[3]
            } else if gold <= 299 {
                vaults[4]
            } else if gold <= 499 {
                vaults[5]
            } else {
                vaults[6]
            };
            for city in self.players[p].cities.clone() {
                for vault in vaults {
                    self.city_set_num_real_building(city, vault, 0);
                }
                self.city_set_num_real_building(city, new_vault, 1);
            }
        }
    }

    /// `doTurnLuchuirp(iPlayer)`: golems get the Empower promotions matching Barnaxus' Combat
    /// promotions (the last Barnaxus found in the unit list counts; none: all removed).
    pub fn cf_do_turn_luchuirp(&mut self, p: PlayerId) {
        let d = self.d;
        let barnaxus_class = d.t::<UnitClassType>("UNITCLASS_BARNAXUS");
        if self.players[p].unit_class_count[barnaxus_class] > 0 {
            let combat = [
                d.t::<PromotionType>("PROMOTION_COMBAT1"),
                d.t::<PromotionType>("PROMOTION_COMBAT2"),
                d.t::<PromotionType>("PROMOTION_COMBAT3"),
                d.t::<PromotionType>("PROMOTION_COMBAT4"),
                d.t::<PromotionType>("PROMOTION_COMBAT5"),
            ];
            let empower = [
                d.t::<PromotionType>("PROMOTION_EMPOWER1"),
                d.t::<PromotionType>("PROMOTION_EMPOWER2"),
                d.t::<PromotionType>("PROMOTION_EMPOWER3"),
                d.t::<PromotionType>("PROMOTION_EMPOWER4"),
                d.t::<PromotionType>("PROMOTION_EMPOWER5"),
            ];
            let golem = d.t::<PromotionType>("PROMOTION_GOLEM");
            let mut barnaxus = None;
            let mut emp = [false; 5];
            let mut golems = Vec::new();
            for unit in self.cf_unit_list(p) {
                if self.unit_get_unit_class_type(unit) == barnaxus_class {
                    barnaxus = Some(unit);
                } else if self.units[unit].has_promotion[golem] {
                    golems.push(unit);
                }
            }
            if let Some(barnaxus) = barnaxus {
                for i in 0..5 {
                    emp[i] = self.units[barnaxus].has_promotion[combat[i]];
                }
            }
            for unit in golems {
                for promotion in empower {
                    self.unit_set_has_promotion(unit, promotion, false);
                }
                for i in 0..5 {
                    if emp[i] {
                        self.unit_set_has_promotion(unit, empower[i], true);
                    }
                }
            }
        }
    }

    /// `findClearPlot(pUnit, plot)`: a random plot of the 3x3 square around the unit (or `plot`).
    /// `unit = None` (Python `pUnit == -1`): an empty plot of the same land/water kind as `plot`,
    /// not a peak or city. With a unit (Python passes -1 as `plot`; `plot` is then unused): a plot
    /// other than the unit's own that it can enter, preferring plots with friendly units (+15 per
    /// unit) and the owner's cities (+50). Each candidate draws a 0..5 roll; ties go to the later
    /// plot. `None` (Python -1) if no plot qualifies.
    pub fn cf_find_clear_plot(&mut self, unit: Option<UnitId>, plot: PlotIdx) -> Option<PlotIdx> {
        let mut best_plot = None;
        let mut best_value = 0;
        let Some(unit) = unit else {
            let (x, y) = self.map.xy(plot);
            let water = self.plot_is_water(plot);
            for xx in x - 1..x + 2 {
                for yy in y - 1..y + 2 {
                    let mut current = 0;
                    let Some(loop_plot) = self.map.plot_xy(xx, yy) else { continue };
                    if self.map.plots[loop_plot].units.is_empty()
                        && self.plot_is_water(loop_plot) == water
                        && !self.plot_is_peak(loop_plot)
                        && self.map.plots[loop_plot].plot_city.is_none()
                    {
                        current += 5;
                    }
                    if current >= 1 {
                        current += self.soren_rand_num(5, "FindClearPlot");
                        if current >= best_value {
                            best_plot = Some(loop_plot);
                            best_value = current;
                        }
                    }
                }
            }
            return best_plot;
        };
        let (x, y) = (self.units[unit].x, self.units[unit].y);
        let owner = self.units[unit].owner;
        for xx in x - 1..x + 2 {
            for yy in y - 1..y + 2 {
                let mut current = 0;
                let Some(loop_plot) = self.map.plot_xy(xx, yy) else { continue };
                if self.map.plots[loop_plot].units.is_empty() && self.unit_can_move_or_attack_into(unit, loop_plot, false) {
                    current += 5;
                }
                for i in 0..self.map.plots[loop_plot].units.len() {
                    let other = self.map.plots[loop_plot].units[i];
                    if self.units[other].owner == owner && self.unit_can_move_or_attack_into(unit, loop_plot, false) {
                        current += 15;
                    }
                }
                if let Some(city) = self.map.plots[loop_plot].plot_city {
                    if self.cities[city].owner == owner {
                        current += 50;
                    }
                }
                if x == xx && y == yy {
                    current = 0;
                }
                if current >= 1 {
                    current += self.soren_rand_num(5, "FindClearPlot");
                    if current >= best_value {
                        best_plot = Some(loop_plot);
                        best_value = current;
                    }
                }
            }
        }
        best_plot
    }

    /// `genesis(iPlayer)` (Genesis ritual): the player's land warms by one step (snow -> tundra
    /// -> plains, desert/burning sands -> plains, plains/fields of perdition/broken lands ->
    /// grassland), unimproved grassland grows forest, and plot counters drop by 100.
    pub fn cf_genesis(&mut self, p: PlayerId) {
        let d = self.d;
        let broken_lands = d.t::<TerrainType>("TERRAIN_BROKEN_LANDS");
        let burning_sands = d.t::<TerrainType>("TERRAIN_BURNING_SANDS");
        let desert = d.t::<TerrainType>("TERRAIN_DESERT");
        let fields = d.t::<TerrainType>("TERRAIN_FIELDS_OF_PERDITION");
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        let snow = d.t::<TerrainType>("TERRAIN_SNOW");
        let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
        let plains = d.t::<TerrainType>("TERRAIN_PLAINS");
        let forest_ancient = d.t::<FeatureType>("FEATURE_FOREST_ANCIENT");
        let forest = d.t::<FeatureType>("FEATURE_FOREST");
        for plot in self.map.plot_ids() {
            if self.map.plots[plot].owner == Some(p) {
                // Every test reads the terrain from before this plot's changes.
                let terrain = self.map.plots[plot].terrain_type;
                if terrain == Some(snow) {
                    self.plot_set_terrain_type(plot, tundra, true);
                }
                if terrain == Some(tundra) {
                    self.plot_set_terrain_type(plot, plains, true);
                }
                if terrain == Some(desert) || terrain == Some(burning_sands) {
                    self.plot_set_terrain_type(plot, plains, true);
                }
                if terrain == Some(plains) || terrain == Some(fields) || terrain == Some(broken_lands) {
                    self.plot_set_terrain_type(plot, grass, true);
                }
                if terrain == Some(grass)
                    && self.map.plots[plot].improvement_type.is_none()
                    && self.map.plots[plot].feature_type != Some(forest_ancient)
                    && !self.plot_is_peak(plot)
                    && self.map.plots[plot].plot_city.is_none()
                {
                    self.plot_set_feature_type(plot, Some(forest));
                }
                self.plot_change_plot_counter(plot, -100);
            }
        }
    }

    /// `getAshenVeilCity(iNum)`: the `num`-th (1..=3) most valuable non-capital Ashen Veil city
    /// of the living non-Infernal players (population, culture, buildings, wonders, improved
    /// plots); `None` (Python -1) for other `num` or too few candidates.
    pub fn cf_get_ashen_veil_city(&self, num: i32) -> Option<CityId> {
        let d = self.d;
        let infernal = d.t::<CivilizationType>("CIVILIZATION_INFERNAL");
        let veil = d.t::<ReligionType>("RELIGION_THE_ASHEN_VEIL");
        let (mut best_value1, mut best_value2, mut best_value3) = (0, 0, 0);
        let (mut best_city1, mut best_city2, mut best_city3) = (None, None, None);
        for p in self.player_ids() {
            if self.players[p].alive && self.player_get_civilization_type(p) != Some(infernal) {
                for &city in &self.players[p].cities {
                    if self.cities[city].has_religion[veil] && !self.city_is_capital(city) {
                        let mut valid = true;
                        let mut value = self.cities[city].population * 100;
                        value += py_div(self.city_culture(city, p), 3);
                        value += self.cities[city].num_buildings * 10;
                        value += self.cities[city].num_world_wonders * 100;
                        value += self.city_count_num_improved_plots(city, None, false);
                        if value > best_value1 {
                            best_value3 = best_value2;
                            best_city3 = best_city2;
                            best_value2 = best_value1;
                            best_city2 = best_city1;
                            best_value1 = value;
                            best_city1 = Some(city);
                            valid = false;
                        }
                        if valid && value > best_value2 {
                            best_value3 = best_value2;
                            best_city3 = best_city2;
                            best_value2 = value;
                            best_city2 = Some(city);
                            valid = false;
                        }
                        if valid && value > best_value3 {
                            best_value3 = value;
                            best_city3 = Some(city);
                        }
                    }
                }
            }
        }
        match num {
            1 => best_city1,
            2 => best_city2,
            3 => best_city3,
            _ => None,
        }
    }

    /// `getCivilization(iCiv)`: the last player slot (dead or alive) of this civilization.
    pub fn cf_get_civilization(&self, civ: CivilizationType) -> Option<PlayerId> {
        self.player_ids().filter(|&p| self.player_get_civilization_type(p) == Some(civ)).last()
    }

    /// `getHero(pPlayer)`: the civilization's hero unit class (the one the Shrine of the Champion
    /// requires), `None` (Python -1) for civilizations without one.
    pub fn cf_get_hero(&self, p: PlayerId) -> Option<UnitClassType> {
        const HEROES: [(&str, &str); 18] = [
            ("CIVILIZATION_BANNOR", "UNITCLASS_DONAL"),
            ("CIVILIZATION_MALAKIM", "UNITCLASS_TEUTORIX"),
            ("CIVILIZATION_ELOHIM", "UNITCLASS_CORLINDALE"),
            ("CIVILIZATION_MERCURIANS", "UNITCLASS_BASIUM"),
            ("CIVILIZATION_LANUN", "UNITCLASS_GUYBRUSH"),
            ("CIVILIZATION_KURIOTATES", "UNITCLASS_EURABATRES"),
            ("CIVILIZATION_LJOSALFAR", "UNITCLASS_GILDEN"),
            ("CIVILIZATION_KHAZAD", "UNITCLASS_MAROS"),
            ("CIVILIZATION_HIPPUS", "UNITCLASS_MAGNADINE"),
            ("CIVILIZATION_AMURITES", "UNITCLASS_GOVANNON"),
            ("CIVILIZATION_BALSERAPHS", "UNITCLASS_LOKI"),
            ("CIVILIZATION_CLAN_OF_EMBERS", "UNITCLASS_RANTINE"),
            ("CIVILIZATION_SVARTALFAR", "UNITCLASS_ALAZKAN"),
            ("CIVILIZATION_CALABIM", "UNITCLASS_LOSHA"),
            ("CIVILIZATION_SHEAIM", "UNITCLASS_ABASHI"),
            ("CIVILIZATION_SIDAR", "UNITCLASS_RATHUS"),
            ("CIVILIZATION_ILLIANS", "UNITCLASS_WILBOMAN"),
            ("CIVILIZATION_INFERNAL", "UNITCLASS_HYBOREM"),
        ];
        let d = self.d;
        let civ = self.player_get_civilization_type(p);
        let mut hero = None;
        for (civ_name, hero_name) in HEROES {
            if civ == Some(d.t::<CivilizationType>(civ_name)) {
                hero = Some(d.t::<UnitClassType>(hero_name));
            }
        }
        hero
    }

    /// `getLeader(iLeader)`: the last player slot (dead or alive) with this leader.
    pub fn cf_get_leader(&self, leader: LeaderHeadType) -> Option<PlayerId> {
        self.player_ids().filter(|&p| self.player_get_leader_type(p) == Some(leader)).last()
    }

    /// `getOpenPlayer()`: the first never-alive player slot (`None`: Python -1).
    pub fn cf_get_open_player(&self) -> Option<PlayerId> {
        self.player_ids().find(|&p| !self.players[p].ever_alive)
    }

    /// `getUnholyVersion(pUnit)`: the unit type a unit becomes when it turns to evil (by unit
    /// combat and tier; `None` (Python -1) if there is no counterpart).
    pub fn cf_get_unholy_version(&self, u: UnitId) -> Option<UnitType> {
        type TierUnits = &'static [(i32, &'static str)];
        const UNHOLY: [(&[&str], TierUnits); 7] = [
            (&["UNITCOMBAT_ADEPT"], &[(2, "UNIT_IMP"), (3, "UNIT_MAGE"), (4, "UNIT_LICH")]),
            (
                &["UNITCOMBAT_ANIMAL", "UNITCOMBAT_BEAST"],
                &[(1, "UNIT_SCOUT"), (2, "UNIT_HELLHOUND"), (3, "UNIT_ASSASSIN"), (4, "UNIT_BEAST_OF_AGARES")],
            ),
            (&["UNITCOMBAT_ARCHER"], &[(2, "UNIT_ARCHER"), (3, "UNIT_LONGBOWMAN"), (4, "UNIT_CROSSBOWMAN")]),
            (
                &["UNITCOMBAT_DISCIPLE"],
                &[(2, "UNIT_DISCIPLE_THE_ASHEN_VEIL"), (3, "UNIT_PRIEST_OF_THE_VEIL"), (4, "UNIT_EIDOLON")],
            ),
            (
                &["UNITCOMBAT_MELEE"],
                &[(1, "UNIT_SKELETON"), (2, "UNIT_DISEASED_CORPSE"), (3, "UNIT_CHAMPION"), (4, "UNIT_BALOR")],
            ),
            (&["UNITCOMBAT_MOUNTED"], &[(2, "UNIT_HORSEMAN"), (3, "UNIT_CHARIOT"), (4, "UNIT_DEATH_KNIGHT")]),
            (
                &["UNITCOMBAT_RECON"],
                &[(1, "UNIT_SCOUT"), (2, "UNIT_HELLHOUND"), (3, "UNIT_ASSASSIN"), (4, "UNIT_BEASTMASTER")],
            ),
        ];
        let d = self.d;
        let combat = self.unit_get_unit_combat_type(u);
        let tier = d.units[self.units[u].unit_type].tier;
        let mut unit = None;
        for (combats, tiers) in UNHOLY {
            if combats.iter().any(|&c| combat == Some(d.t::<UnitCombatType>(c))) {
                for &(t, name) in tiers {
                    if tier == t {
                        unit = Some(d.t::<UnitType>(name));
                    }
                }
            }
        }
        unit
    }

    /// `giftUnit(iUnit, iCivilization, iXP, pFromPlot, iFromPlayer)`: every living player of
    /// `civ` gets the unit (with `xp` experience) in a random city. Angels and Manes only come
    /// with a chance (95% minus 3% per living civ, plus the xp, clamped to 5..=95); AI players
    /// may turn the Manes into population right away. `from_plot`/`from_player` (the unit that
    /// died) only matter for the messages.
    pub fn cf_gift_unit(&mut self, unit: UnitType, civ: CivilizationType, xp: i32, from_plot: Option<PlotIdx>, from_player: Option<PlayerId>) {
        let d = self.d;
        let angel = d.t::<UnitType>("UNIT_ANGEL");
        let manes = d.t::<UnitType>("UNIT_MANES");
        if unit == angel || unit == manes {
            let mut chance = 100 - self.count_civ_players_alive() * 3;
            chance += xp;
            chance = chance.clamp(5, 95);
            if self.soren_rand_num(100, "Gift Unit") > chance {
                return;
            }
        }
        let mut valid = false;
        for p in self.player_ids() {
            if self.players[p].alive && self.player_get_civilization_type(p) == Some(civ) && self.players[p].num_cities() > 0 {
                let rnd = self.soren_rand_num(self.players[p].num_cities(), "Gift Unit");
                let city = self.players[p].cities[rnd as usize];
                let plot = self.city_plot(city);
                let (x, y) = self.map.xy(plot);
                let new_unit = self.player_init_unit(p, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
                self.unit_change_experience(new_unit, xp, -1, false, false, false);
                self.unit_set_weapons(new_unit);
                if from_plot.is_some() && from_player.is_some_and(|fp| self.players[fp].human) {
                    valid = true;
                }
                if self.players[p].human {
                    if unit == manes {
                        self.cf_msg(p, "TXT_KEY_MESSAGE_ADD_MANES");
                    }
                    if unit == angel {
                        self.cf_msg(p, "TXT_KEY_MESSAGE_ADD_ANGEL");
                    }
                }
                // Python: `if not pPlayer.isHuman() and iUnit == iManes and pCity != -1` (a city
                // object, never -1).
                if !self.players[p].human
                    && unit == manes
                    && self.soren_rand_num(100, "Manes") < 100 - self.cities[city].population * 5
                {
                    self.city_change_population(city, 1);
                    self.unit_kill(new_unit, true, None);
                }
            }
        }
        if let (true, Some(from_player)) = (valid, from_player) {
            if unit == manes {
                self.cf_msg(from_player, "TXT_KEY_MESSAGE_UNIT_FALLS");
            }
            if unit == angel {
                self.cf_msg(from_player, "TXT_KEY_MESSAGE_UNIT_RISES");
            }
        }
    }

    /// `placeTreasure(iPlayer, iUnit)`: the player gets the treasure unit on the best empty,
    /// passable land plot without a city: random 0..1000, +1000 on areas under 8 tiles, +1000
    /// when unowned.
    pub fn cf_place_treasure(&mut self, p: PlayerId, unit: UnitType) {
        let mut best_plot = None;
        let mut best_value = -1;
        for plot in self.map.plot_ids() {
            if !self.plot_is_water(plot)
                && self.map.plots[plot].units.is_empty()
                && self.map.plots[plot].plot_city.is_none()
                && !self.plot_is_impassable(plot)
            {
                let mut value = self.soren_rand_num(1000, "Add Unit");
                if self.cf_plot_area_num_tiles(plot) < 8 {
                    value += 1000;
                }
                if !self.plot_is_owned(plot) {
                    value += 1000;
                }
                if value > best_value {
                    best_value = value;
                    best_plot = Some(plot);
                }
            }
        }
        // Python: `if iBestPlot != -1`.
        if let Some(plot) = best_plot {
            let (x, y) = self.map.xy(plot);
            self.player_init_unit(p, unit, x, y, None, Some(DirectionType::DIRECTION_SOUTH));
            self.cf_msg(p, "TXT_KEY_MESSAGE_EXPLORE_LAIR_TREASURE");
            // UI: CyCamera().JustLookAtPlot(pBestPlot).
        }
    }

    /// `startWar(iPlayer, i2Player, iWarPlan)`: `p`'s team declares war on `p2`'s team if both
    /// are alive, distinct, have met, are not at war and not locked in permanent war/peace.
    pub fn cf_start_war(&mut self, p: PlayerId, p2: PlayerId, war_plan: WarPlanType) {
        let team = self.players[p].team;
        let team2 = self.players[p2].team;
        if self.team_is_alive(team)
            && self.team_is_alive(team2)
            && !self.teams[team].at_war[team2]
            && team != team2
            && self.teams[team].has_met[team2]
            && !self.teams[team].permanent_war_peace[team2]
        {
            self.team_declare_war(team, team2, false, Some(war_plan));
        }
    }

    /// `MarnokNameGenerator(unit)`: a random name built from syllable/word lists that depend on
    /// the owner's alignment and state religion and on the unit's promotions, type and unit
    /// combat (RNG: one "Name Gen" roll for the schema and one per generated part).
    pub fn cf_marnok_name_generator(&mut self, u: UnitId) -> String {
        let d = self.d;
        let owner = self.units[u].owner;
        let religion = self.player_get_state_religion(owner);
        let alignment = self.players[owner].alignment;
        let unit_type = self.units[u].unit_type;
        let is_religion = |name: &str| religion == Some(d.t::<ReligionType>(name));
        let has = |name: &str| self.units[u].has_promotion[d.t::<PromotionType>(name)];
        let is_unit = |name: &str| unit_type == d.t::<UnitType>(name);
        let flags = MarnokFlags {
            evil: alignment == Some(d.t::<AlignmentType>("ALIGNMENT_EVIL")),
            ashen_veil: is_religion("RELIGION_THE_ASHEN_VEIL"),
            octopus_overlords: is_religion("RELIGION_OCTOPUS_OVERLORDS"),
            the_order: is_religion("RELIGION_THE_ORDER"),
            fellowship_of_leaves: is_religion("RELIGION_FELLOWSHIP_OF_LEAVES"),
            runes_of_kilmorph: is_religion("RELIGION_RUNES_OF_KILMORPH"),
            the_empyrean: is_religion("RELIGION_THE_EMPYREAN"),
            council_of_esus: is_religion("RELIGION_COUNCIL_OF_ESUS"),
            enraged: has("PROMOTION_ENRAGED"),
            crazed: has("PROMOTION_CRAZED"),
            vampire: has("PROMOTION_VAMPIRE"),
            demon: has("PROMOTION_DEMON"),
            hill_giant: is_unit("UNIT_HILL_GIANT"),
            lizardman: is_unit("UNIT_LIZARDMAN"),
            fire_elemental_or_azer: is_unit("UNIT_FIRE_ELEMENTAL") || is_unit("UNIT_AZER"),
            water_elemental: is_unit("UNIT_WATER_ELEMENTAL"),
            air_elemental: is_unit("UNIT_AIR_ELEMENTAL"),
            earth_elemental: is_unit("UNIT_EARTH_ELEMENTAL"),
            naval: self.unit_get_unit_combat_type(u) == Some(d.t::<UnitCombatType>("UNITCOMBAT_NAVAL")),
        };
        let lists = marnok_lists(&flags);
        marnok_name(&lists, &mut |n| self.soren_rand_num(n, "Name Gen"))
    }

    /// `BarbCityProduction(pCity)` (barbarian city production, from CvGameUtils
    /// `AI_chooseProduction`): trains the highest-valued unit the city can build (Acheron and
    /// Tumtum first while unique, then tier-2 then tier-1 units). False for human owners or when
    /// nothing can be trained; true if a production order was pushed.
    pub fn cf_barb_city_production(&mut self, city: CityId) -> bool {
        const PROD: [&str; 13] = [
            "UNITCLASS_WARRIOR",
            "UNITCLASS_ADEPT",
            "UNITCLASS_AXEMAN",
            "UNITCLASS_HUNTER",
            "UNITCLASS_ARCHER",
            "UNITCLASS_HORSEMAN",
            "UNITCLASS_CHAMPION",
            "UNITCLASS_CHARIOT",
            "UNITCLASS_RANGER",
            "UNITCLASS_LONGBOWMAN",
            "UNITCLASS_ASSASSIN",
            "UNITCLASS_ACHERON",
            "UNITCLASS_TUMTUM",
        ];
        // Python `sProd.index(..)`.
        const WARRIOR: usize = 0;
        const AXEMAN: usize = 2;
        const HUNTER: usize = 3;
        const ARCHER: usize = 4;
        const HORSEMAN: usize = 5;
        const CHAMPION: usize = 6;
        const CHARIOT: usize = 7;
        const RANGER: usize = 8;
        const LONGBOWMAN: usize = 9;
        const ASSASSIN: usize = 10;
        const ACHERON: usize = 11;
        const TUMTUM: usize = 12;

        let d = self.d;
        let owner = self.cities[city].owner;
        if self.players[owner].human {
            return false;
        }
        // A city owner always has a civilization (C++ would index the civ infos with -1).
        let Some(civ) = self.player_get_civilization_type(owner) else { return false };

        let mut value = [0i32; PROD.len()];
        value[AXEMAN] = 19500;
        value[ARCHER] = 19000 + self.soren_rand_num(650, "Archer Scores");
        value[HUNTER] = 19400 + self.soren_rand_num(200, "Lizard Scores");
        value[HORSEMAN] = 19400 + self.soren_rand_num(200, "Horseman Scores");
        value[WARRIOR] = 19002;

        let tumtum = d.t::<UnitClassType>("UNITCLASS_TUMTUM");
        if self.player_get_unit_class_count_plus_making(owner, tumtum) == 0 && self.unit_class_created_count[tumtum] == 0 {
            value[TUMTUM] = 100000;
        }
        let acheron = d.t::<UnitClassType>("UNITCLASS_ACHERON");
        if !self.is_option(GameOptionType::GAMEOPTION_NO_ACHERON)
            && self.unit_class_created_count[acheron] == 0
            && self.player_get_unit_class_count_plus_making(owner, acheron) == 0
        {
            value[ACHERON] = 150000;
        }
        // Value higher tier units more.
        if value[WARRIOR] > value[AXEMAN] {
            value[AXEMAN] = value[WARRIOR] + 1000;
        }
        if value[WARRIOR] > value[ARCHER] {
            value[ARCHER] = value[WARRIOR] + 1000;
        }
        if value[WARRIOR] > value[HUNTER] {
            value[HUNTER] = value[WARRIOR] + 1000;
        }
        if value[AXEMAN] > 0 {
            value[CHAMPION] += value[AXEMAN] + 1000;
        }
        if value[ARCHER] > 0 {
            value[LONGBOWMAN] += value[ARCHER] + 1000;
        }
        if value[HUNTER] > 0 {
            value[RANGER] += value[HUNTER] + 1000;
            value[ASSASSIN] += value[HUNTER] + 1000;
        }
        if value[HORSEMAN] > 0 {
            value[CHARIOT] += value[HORSEMAN] + 1000;
        }

        let mut best_building = None;
        let mut best_building_value = 0;
        for (i, class) in PROD.iter().enumerate() {
            if value[i] > best_building_value {
                if let Some(unit) = d.civ_unit(civ, d.t::<UnitClassType>(class)) {
                    if self.city_can_train(city, unit, true, false, false, false) {
                        best_building_value = value[i];
                        best_building = Some(unit);
                    }
                }
            }
        }
        // UI: TXT_KEY_MESSAGE_START_PRODUCTION_ACHERON when the last unit looked up is the Acheron.
        if let Some(unit) = best_building {
            let unit_ai = if d.units[unit].unit_combat_type == Some(d.t::<UnitCombatType>("UNITCOMBAT_ARCHER")) {
                d.t::<UnitAIType>("UNITAI_CITY_DEFENSE")
            } else {
                d.t::<UnitAIType>("UNITAI_ATTACK_CITY")
            };
            self.city_push_order(city, OrderType::ORDER_TRAIN, unit.i(), unit_ai.i(), false, false, false, false);
            return true;
        }
        false
    }

    // ---- helpers (Python helpers / Cy* wrapper behaviour not declared elsewhere) ----

    /// Python `lList[CyGame().getSorenRandNum(len(lList), log) - 1]`.
    fn cf_pick<T: Copy>(&mut self, list: &[T], log: &str) -> T {
        let roll = self.soren_rand_num(list.len() as i32, log);
        list[py_list_index(roll, list.len())]
    }

    /// `pPlot.area().getNumTiles()`: the Python NULL area (no area id) answers -1 (`CyArea`).
    fn cf_plot_area_num_tiles(&self, plot: PlotIdx) -> i32 {
        match self.map.plots[plot].area {
            Some(area) => self.map.areas[area].num_tiles,
            None => -1,
        }
    }

    /// `PyPlayer(iPlayer).getUnitList()` (BtS PyHelpers): the player's units, skipping dead ones
    /// (`isDead()`: damage at max hit points, pending delayed death).
    fn cf_unit_list(&self, p: PlayerId) -> Vec<UnitId> {
        self.players[p].units.iter().copied().filter(|&u| !self.unit_is_dead(u)).collect()
    }

    /// `getUnitPlayerID(pUnit)`: scans `pPlayer.getUnit(i)` for `i in range(getNumUnits())` and
    /// returns the `i` whose unit is `pUnit`, which `initTriggeredData` turns back into the unit.
    /// Python/C++ quirk not reproduced: `getUnit(i)` reads free-list slot `i`, so the C++ only
    /// finds units whose slot is below the unit count. Units have global ids here (no slots), so
    /// the unit is always found.
    fn cf_get_unit_player_id(&self, u: UnitId) -> Option<UnitId> {
        let owner = self.units[u].owner;
        self.players[owner].units.contains(&u).then_some(u)
    }

    /// The lair events of `exploreLairNeutral`: the caster's owner triggers `trigger` at the
    /// caster's plot with the caster as the event unit (and its owner as the other player).
    fn cf_lair_event(&mut self, caster: UnitId, trigger: &str) {
        if let Some(unit_id) = self.cf_get_unit_player_id(caster) {
            let event = self.d.t::<EventTriggerType>(trigger);
            let owner = self.units[caster].owner;
            let (x, y) = (self.units[caster].x, self.units[caster].y);
            self.player_init_triggered_data(owner, event, true, None, x, y, Some(owner), None, None, None, Some(unit_id), None);
        }
    }

    /// `pPlayer.receiveGoody(pPlot, gc.getInfoTypeForString(goody), caster)`.
    fn cf_receive_goody(&mut self, p: PlayerId, plot: PlotIdx, goody: &str, caster: UnitId) {
        let goody = self.d.t::<GoodyType>(goody);
        self.player_receive_goody(p, plot, goody, Some(caster));
    }

    /// `doHellTerrain` spread step: +1 plot counter for every plot of the 3x3 square around
    /// `plot` (itself included, read as it changes) whose counter is above 10. True if any.
    fn cf_hell_spread_from_adjacent(&mut self, plot: PlotIdx) -> bool {
        let (x, y) = self.map.xy(plot);
        let mut touched = false;
        for xx in x - 1..x + 2 {
            for yy in y - 1..y + 2 {
                if let Some(adjacent) = self.map.plot_xy(xx, yy) {
                    if self.map.plots[adjacent].plot_counter > 10 {
                        self.plot_change_plot_counter(plot, 1);
                        touched = true;
                    }
                }
            }
        }
        touched
    }

    /// Game-log stand-in for `CyInterface().addMessage(iPlayer, .., text(key), ..)`.
    fn cf_msg(&mut self, p: PlayerId, key: &'static str) {
        let d = self.d;
        let turn = self.game_turn();
        self.log.msg(turn, Some(p), || d.text.get(key).to_string());
    }
}

/// Index read by Python `lList[roll - 1]` for a roll in `0..len`: a roll of 0 is index -1, the
/// last element.
fn py_list_index(roll: i32, len: usize) -> usize {
    (roll - 1).rem_euclid(len as i32) as usize
}

/// Python 2 `str.capitalize()` (ASCII): first character upper case, the rest lower case.
fn py_capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

/// What `MarnokNameGenerator` tests, in the Python's order: owner alignment and state
/// religion, unit promotions, unit type, unit combat.
#[derive(Clone, Copy, Debug, Default)]
struct MarnokFlags {
    evil: bool,
    ashen_veil: bool,
    octopus_overlords: bool,
    the_order: bool,
    fellowship_of_leaves: bool,
    runes_of_kilmorph: bool,
    the_empyrean: bool,
    council_of_esus: bool,
    enraged: bool,
    crazed: bool,
    vampire: bool,
    demon: bool,
    hill_giant: bool,
    lizardman: bool,
    /// UNIT_FIRE_ELEMENTAL or UNIT_AZER.
    fire_elemental_or_azer: bool,
    water_elemental: bool,
    air_elemental: bool,
    earth_elemental: bool,
    naval: bool,
}

/// The name-part lists of `MarnokNameGenerator` (`lPre`, `lMid`, `lEnd`, `lEpithet`, `lNoun`,
/// `lSchema`).
struct MarnokLists {
    pre: Vec<&'static str>,
    mid: Vec<&'static str>,
    end: Vec<&'static str>,
    epithet: Vec<&'static str>,
    noun: Vec<&'static str>,
    schema: Vec<&'static str>,
}

/// `MarnokNameGenerator` list building (order matters: parts are picked by index).
fn marnok_lists(f: &MarnokFlags) -> MarnokLists {
    let mut pre = vec![
        "ta", "go", "da", "bar", "arc", "ken", "an", "ad", "mi", "kon", "kar", "mar", "wal", "he", "ha", "re", "ar", "bal", "bel", "bo", "bri",
        "car", "dag", "dan", "ma", "ja", "co", "be", "ga", "qui", "sa",
    ];
    let mut mid = vec![
        "ad", "z", "the", "and", "tha", "ent", "ion", "tio", "for", "tis", "oft", "che", "gan", "an", "en", "wen", "on", "d", "n", "g", "t", "ow",
        "dal",
    ];
    let mut end = vec![
        "ar", "sta", "na", "is", "el", "es", "ie", "us", "un", "th", "er", "on", "an", "re", "in", "ed", "nd", "at", "en", "le", "man", "ck", "ton",
        "nok", "git", "us", "or", "a", "da", "u", "cha", "ir",
    ];
    let mut epithet = vec!["red", "blue", "black", "grey", "white", "strong", "brave", "old", "young", "great", "slayer", "hunter", "seeker"];
    let mut noun = vec![
        "spirit", "soul", "boon", "born", "staff", "rod", "shield", "autumn", "winter", "spring", "summer", "wit", "horn", "tusk", "glory", "claw",
        "tooth", "head", "heart", "blood", "breath", "blade", "hand", "lover", "bringer", "maker", "taker", "river", "stream", "moon", "star",
        "face", "foot", "half", "one", "hundred", "thousand",
    ];
    let mut schema = vec![
        "CPME",
        "CPMESCPME",
        "CPESCPE",
        "CPE",
        "CPMME",
        "CPMDCME",
        "CPMAME",
        "KCPMESCUM",
        "CPMME[ the ]CX",
        "CPMESCXN",
        "CPMME[ of ]CPMME",
        "CNNSCXN",
    ];
    const ELEMENTAL_SCHEMA: [&str; 6] = ["CNN", "CNX", "CPME", "CPME[ the ]CNX", "CPMME", "CNNSCPME"];

    if f.evil {
        noun.extend_from_slice(&[
            "fear", "terror", "reign", "brood", "snare", "war", "strife", "pain", "hate", "evil", "hell", "misery", "murder", "anger", "fury", "rage",
            "spawn", "sly", "blood", "bone", "scythe", "slave", "bound", "ooze", "scum",
        ]);
        epithet = vec!["dark", "black", "white", "cruel", "foul"];
    }
    if f.ashen_veil {
        epithet.extend_from_slice(&["fallen", "diseased", "infernal", "profane", "corrupt"]);
        schema.push("CPME[ the ]CX");
    }
    if f.octopus_overlords {
        pre.extend_from_slice(&["cth", "cht", "shu", "az", "ts", "dag", "hy", "gla", "gh", "rh", "x", "ll"]);
        mid.extend_from_slice(&["ul", "tha", "on", "ug", "st", "oi"]);
        end.extend_from_slice(&["hu", "on", "ha", "ua", "oa", "uth", "oth", "ath", "thua", "thoa", "ur", "ll", "og", "hua"]);
        epithet.extend_from_slice(&["nameless", "webbed", "deep", "watery"]);
        noun.extend_from_slice(&["tentacle", "wind", "wave", "sea", "ocean", "dark", "crab", "abyss", "island"]);
        schema.extend_from_slice(&["CPMME", "CPDMME", "CPAMAME", "CPMAME", "CPAMAMEDCPAMAE"]);
    }
    if f.the_order {
        pre.extend_from_slice(&["ph", "v", "j"]);
        mid.extend_from_slice(&["an", "al", "un"]);
        end.extend_from_slice(&["uel", "in", "il"]);
        epithet.extend_from_slice(&["confessor", "crusader", "faithful", "obedient", "good"]);
        noun.extend_from_slice(&["order", "faith", "heaven", "law"]);
        schema.extend_from_slice(&["CPESCPME", "CPMESCPE", "CPMESCPME", "CPESCPE"]);
    }
    if f.fellowship_of_leaves {
        pre.extend_from_slice(&["ki", "ky", "yv"]);
        mid.extend_from_slice(&["th", "ri"]);
        end.extend_from_slice(&["ra", "el", "ain"]);
        epithet.push("green");
        noun.extend_from_slice(&["tree", "bush", "wood", "berry", "elm", "willow", "oak", "leaf", "flower", "blossom"]);
        schema.extend_from_slice(&["CPESCN", "CPMESCNN", "CPMESCXN"]);
    }
    if f.runes_of_kilmorph {
        pre.extend_from_slice(&["bam", "ar", "khel", "ki"]);
        mid.extend_from_slice(&["th", "b", "en"]);
        end.extend_from_slice(&["ur", "dain", "ain", "don"]);
        epithet.extend_from_slice(&["deep", "guard", "miner"]);
        noun.extend_from_slice(&["rune", "flint", "slate", "stone", "rock", "iron", "copper", "mithril", "thane", "umber"]);
        schema.extend_from_slice(&["CPME", "CPMME"]);
    }
    if f.the_empyrean {
        epithet.extend_from_slice(&["radiant", "holy"]);
        noun.push("honor");
    }
    if f.council_of_esus {
        epithet.extend_from_slice(&["hidden", "dark"]);
        noun.extend_from_slice(&["cloak", "shadow", "mask"]);
        schema.extend_from_slice(&["CPME", "CPMME"]);
    }
    if f.enraged {
        pre.extend_from_slice(&["gru", "bra", "no", "os", "dir", "ka", "z"]);
        mid.extend_from_slice(&["g", "ck", "gg", "sh", "b", "bh", "aa"]);
        end.extend_from_slice(&["al", "e", "ek", "esh", "ol", "olg", "alg"]);
        noun.extend_from_slice(&["death", "hate", "rage", "mad", "insane", "berserk"]);
        epithet.extend_from_slice(&["smasher", "breaker", "mangle", "monger"]);
    }
    if f.crazed {
        pre.extend_from_slice(&["mad", "pim", "zi", "zo", "fli", "mum", "dum", "odd", "slur"]);
        mid.extend_from_slice(&["bl", "pl", "gg", "ug", "bl", "b", "zz", "abb", "odd"]);
        end.extend_from_slice(&["ad", "ap", "izzle", "onk", "ing", "er", "po", "eep", "oggle", "y"]);
    }
    if f.vampire {
        pre.extend_from_slice(&["dra", "al", "nos", "vam", "vla", "tep", "bat", "bar", "cor", "lil", "ray", "zar", "stra", "le"]);
        mid.extend_from_slice(&["cul", "u", "car", "fer", "pir", "or", "na", "ov", "sta"]);
        end.extend_from_slice(&["a", "d", "u", "e", "es", "y", "bas", "vin", "ith", "ne", "ak", "ich", "hd", "t"]);
    }
    if f.demon {
        pre.extend_from_slice(&["aa", "ab", "adr", "ah", "al", "de", "ba", "cro", "da", "be", "eu", "el", "ha", "ib", "me", "she", "sth", "z"]);
        mid.extend_from_slice(&["rax", "lia", "ri", "al", "as", "b", "bh", "aa", "al", "ze", "phi", "sto", "phe", "cc", "ee"]);
        end.extend_from_slice(&["tor", "tan", "ept", "lu", "res", "ah", "mon", "gon", "bul", "gul", "lis", "les", "uz"]);
        schema = vec!["CPMMME", "CPMACME", "CPKMAUAPUE", "CPMMME[ the ]CNX"];
    }
    if f.hill_giant {
        pre.extend_from_slice(&["gor", "gra", "gar", "gi", "gol"]);
        mid.extend_from_slice(&["gan", "li", "ri", "go"]);
        end.extend_from_slice(&["tus", "tan", "ath", "tha"]);
        schema.extend_from_slice(&["CXNSCNN", "CPESCNE", "CPMME[ the ]CX"]);
        epithet.extend_from_slice(&["large", "huge", "collossal", "brutal", "basher", "smasher", "crasher", "crusher"]);
        noun.extend_from_slice(&["fist", "tor", "hill", "brute", "stomp"]);
    }
    if f.lizardman {
        pre.extend_from_slice(&["ss", "s", "th", "sth", "hss"]);
        end.extend_from_slice(&["ess", "iss", "ath", "tha"]);
        epithet.push("cold");
        noun.extend_from_slice(&["hiss", "tongue", "slither", "scale", "tail", "ruin"]);
        schema.extend_from_slice(&["CPAECPAE", "CPAKECPAU", "CPAMMAE"]);
    }
    if f.fire_elemental_or_azer {
        pre.extend_from_slice(&["ss", "cra", "th", "sth", "hss", "roa"]);
        mid.extend_from_slice(&["ss", "ck", "rr", "oa", "iss", "tt"]);
        end.extend_from_slice(&["le", "iss", "st", "r", "er"]);
        noun.extend_from_slice(&["hot", "burn", "scald", "roast", "flame", "scorch", "char", "sear", "singe", "fire", "spit"]);
        schema = ELEMENTAL_SCHEMA.to_vec();
    }
    if f.water_elemental {
        pre.extend_from_slice(&["who", "spl", "dr", "sl", "spr", "sw", "b"]);
        mid.extend_from_slice(&["o", "a", "i", "ub", "ib"]);
        end.extend_from_slice(&["sh", "p", "ter", "ble"]);
        noun.extend_from_slice(&["wave", "lap", "sea", "lake", "water", "tide", "surf", "spray", "wet", "damp", "soak", "gurgle", "bubble"]);
        schema = ELEMENTAL_SCHEMA.to_vec();
    }
    if f.air_elemental {
        pre.extend_from_slice(&["ff", "ph", "th", "ff", "ph", "th"]);
        mid.extend_from_slice(&["oo", "aa", "ee", "ah", "oh"]);
        end.extend_from_slice(&["ff", "ph", "th", "ff", "ph", "th"]);
        noun.extend_from_slice(&["wind", "air", "zephyr", "breeze", "gust", "blast", "blow"]);
        schema = ELEMENTAL_SCHEMA.to_vec();
    }
    if f.earth_elemental {
        pre.extend_from_slice(&["gra", "gro", "kro", "ff", "ph", "th"]);
        mid.extend_from_slice(&["o", "a", "u"]);
        end.extend_from_slice(&["ck", "g", "k"]);
        noun.extend_from_slice(&["rock", "stone", "boulder", "slate", "granite", "rumble", "quake"]);
        schema = ELEMENTAL_SCHEMA.to_vec();
    }
    // Ships: special schemas.
    if f.naval {
        end.extend_from_slice(&["ton", "town", "port"]);
        noun.extend_from_slice(&["lady", "jolly", "keel", "bow", "stern", "mast", "sail", "deck", "hull", "reef", "wave"]);
        epithet.extend_from_slice(&["sea", "red", "blue", "grand", "barnacle", "gull"]);
        schema = vec![
            "[The ]CNN",
            "[The ]CXN",
            "[The ]CNX",
            "[The ]CNSCN",
            "[The ]CNSCX",
            "CPME['s ]CN",
            "[The ]CPME",
            "[The ]CNX",
            "CNX",
            "CN['s ]CN",
        ];
    }
    MarnokLists { pre, mid, end, epithet, noun, schema }
}

/// `MarnokNameGenerator` name building: pick a schema, then run its actions -- P/M/E/X/N add a
/// random prefix/middle/ending/epithet/noun, S/D/A add a space/dash/apostrophe, C capitalizes
/// the next generated phrase, K stores it for U, `[..]` is copied literally -- and cut the name
/// to 25 characters. `rand(n)` is the "Name Gen" roll in `0..n`.
fn marnok_name(l: &MarnokLists, rand: &mut dyn FnMut(i32) -> i32) -> String {
    fn pick(list: &[&'static str], rand: &mut dyn FnMut(i32) -> i32) -> &'static str {
        list[py_list_index(rand(list.len() as i32), list.len())]
    }
    let schema = pick(&l.schema, rand);
    let mut full = String::new();
    let mut keep = String::new();
    let mut upper = false;
    let mut keep_next = false;
    let mut skip = false;
    for action in schema.chars() {
        let mut add = String::new();
        let mut done = false;
        if skip {
            if action == ']' {
                skip = false;
            } else {
                add.push(action);
                done = true;
            }
        } else {
            match action {
                'P' => {
                    add = pick(&l.pre, rand).to_string();
                    done = true;
                }
                'M' => {
                    add = pick(&l.mid, rand).to_string();
                    done = true;
                }
                'E' => {
                    add = pick(&l.end, rand).to_string();
                    done = true;
                }
                'X' => {
                    add = pick(&l.epithet, rand).to_string();
                    done = true;
                }
                'N' => {
                    add = pick(&l.noun, rand).to_string();
                    done = true;
                }
                'S' => {
                    add = " ".to_string();
                    done = true;
                }
                'D' => {
                    add = "-".to_string();
                    done = true;
                }
                'A' => {
                    add = "'".to_string();
                    done = true;
                }
                'C' => upper = true,
                'K' => keep_next = true,
                'U' => {
                    add = keep.clone();
                    done = true;
                }
                '[' => skip = true,
                _ => {}
            }
        }
        // Capitalizes the phrase once.
        if upper && done {
            add = py_capitalize(&add);
            upper = false;
        }
        // Stores the next phrase generated.
        if keep_next && done {
            keep = add.clone();
            keep_next = false;
        }
        if done {
            full.push_str(&add);
        }
    }
    // Trim the name length.
    if full.chars().count() > 25 {
        full = full.chars().take(25).collect();
    }
    full
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::setup::GameSetup;

    #[test]
    fn python_list_index_wraps_roll_zero_to_last() {
        assert_eq!(py_list_index(0, 5), 4);
        assert_eq!(py_list_index(1, 5), 0);
        assert_eq!(py_list_index(4, 5), 3);
        assert_eq!(py_list_index(0, 1), 0);
    }

    #[test]
    fn python_capitalize() {
        assert_eq!(py_capitalize("the"), "The");
        assert_eq!(py_capitalize("mIxED"), "Mixed");
        assert_eq!(py_capitalize("Ta"), "Ta");
        assert_eq!(py_capitalize(" "), " ");
        assert_eq!(py_capitalize("'"), "'");
        assert_eq!(py_capitalize(""), "");
    }

    fn marnok_flags(names: &[&str]) -> MarnokFlags {
        let mut f = MarnokFlags::default();
        for &n in names {
            let flag = match n {
                "evil" => &mut f.evil,
                "ashen_veil" => &mut f.ashen_veil,
                "octopus_overlords" => &mut f.octopus_overlords,
                "the_order" => &mut f.the_order,
                "fellowship_of_leaves" => &mut f.fellowship_of_leaves,
                "runes_of_kilmorph" => &mut f.runes_of_kilmorph,
                "the_empyrean" => &mut f.the_empyrean,
                "council_of_esus" => &mut f.council_of_esus,
                "enraged" => &mut f.enraged,
                "crazed" => &mut f.crazed,
                "vampire" => &mut f.vampire,
                "demon" => &mut f.demon,
                "hill_giant" => &mut f.hill_giant,
                "lizardman" => &mut f.lizardman,
                "fire_elemental_or_azer" => &mut f.fire_elemental_or_azer,
                "water_elemental" => &mut f.water_elemental,
                "air_elemental" => &mut f.air_elemental,
                "earth_elemental" => &mut f.earth_elemental,
                "naval" => &mut f.naval,
                _ => panic!("unknown flag {n}"),
            };
            *flag = true;
        }
        f
    }

    /// Expected values produced by running the original CustomFunctions.MarnokNameGenerator
    /// (Python, mocked Cy* objects, CvRandom LCG): two names drawn in a row from `seed`, and the
    /// RNG seed afterwards.
    #[test]
    fn marnok_names_match_python() {
        #[rustfmt::skip]
        let rows: &[(&[&str], u32, &str, &str, u32)] = &[
            (&[], 1, "Balis Dagcha", "Jaan'onar", 267834847),
            (&[], 7, "Bartheionin the Brave", "Hazenel", 734865450),
            (&[], 42, "Dagwengck the Strong", "Beioner Began", 2538925513),
            (&[], 1234, "Spiritthousand Bluestar", "Mardwenck of Baltisadin", 2141587300),
            (&[], 99999, "Waldalganna of Sagtnd", "Arcwenth Bravetooth", 2054532334),
            (&[], 31337, "Barentchend", "Mid'forton", 3096086407),
            (&[], 2026, "Arcche-Ganor", "Argzre", 2002689936),
            (&[], 65535, "Loverface Whiteglory", "Gaor Briat", 2420578509),
            (&["evil", "ashen_veil"], 1, "Balis Dagcha", "Jaan'onar", 267834847),
            (&["evil", "ashen_veil"], 7, "Bartheer Cruelhalf", "Haz-Enel", 734865450),
            (&["evil", "ashen_veil"], 42, "Dagwena Diseasedface", "Beiontioat the Black", 1108520142),
            (&["evil", "ashen_veil"], 1234, "Sadales the Infernal", "Loverhell Fallenstrife", 3080333240),
            (&["evil", "ashen_veil"], 99999, "Makerscum Foulrod", "Cotnd the Infernal", 3682460781),
            (&["evil", "ashen_veil"], 31337, "Barent-Chend", "Midan Miwen", 3096086407),
            (&["evil", "ashen_veil"], 2026, "Arcche'ganor", "Argzre", 2002689936),
            (&["evil", "ashen_veil"], 65535, "Bogon the Cruel", "Gaor Briat", 2420578509),
            (&["octopus_overlords", "enraged"], 1, "Gaus", "Dirtgir", 2633739833),
            (&["octopus_overlords", "enraged"], 7, "Anthacheus", "Danonandha the Black", 734865450),
            (&["octopus_overlords", "enraged"], 42, "Dagonckoa", "Azgged Azche", 1535244752),
            (&["octopus_overlords", "enraged"], 1234, "Ta'aa'entog-Ka'an'thua", "Azgenna of Danaddton", 1890111891),
            (&["octopus_overlords", "enraged"], 99999, "Briaa'gel", "Ll'gg'gog-Ad'on'nd", 1409266928),
            (&["octopus_overlords", "enraged"], 31337, "Anforda Ang", "Hastanuth of Grushnus", 2147357987),
            (&["octopus_overlords", "enraged"], 2026, "Mintll the Young", "Llandwenhu the Deep", 3469224078),
            (&["octopus_overlords", "enraged"], 65535, "Cht'ck'anck-Wal'b'og", "Beoftoigit of Xionnen", 1586255356),
            (&["the_order", "crazed", "vampire"], 1, "Vun", "Faithlover Crusaderbringe", 2633739833),
            (&["the_order", "crazed", "vampire"], 7, "Rodrod Strongblood", "Cotie Cogg", 2862681365),
            (&["the_order", "crazed", "vampire"], 42, "Starmoon Crusaderstream", "Mumferat Mumen", 1535244752),
            (&["the_order", "crazed", "vampire"], 1234, "Taar Kares", "Carabber Mumalgit", 1886064886),
            (&["the_order", "crazed", "vampire"], 99999, "Danstauel Arcady", "Sauun Odden", 1409266928),
            (&["the_order", "crazed", "vampire"], 31337, "Adche'anuel", "Balabbton Hunterhundred", 3786475188),
            (&["the_order", "crazed", "vampire"], 2026, "Kondalin Konu", "Phcar-Thaor", 2002689936),
            (&["the_order", "crazed", "vampire"], 65535, "Zobas Dagnok", "Rayes", 3697340407),
            (&["fellowship_of_leaves", "hill_giant"], 1, "Cares", "Garck Oak", 3655513600),
            (&["fellowship_of_leaves", "hill_giant"], 7, "Arcandan Bringerriver", "Balthegit Baland", 734865450),
            (&["fellowship_of_leaves", "hill_giant"], 42, "Gatel Hundredmaker", "Blossomglory Bravemoon", 2538925513),
            (&["fellowship_of_leaves", "hill_giant"], 1234, "Tagothara the Crasher", "Quig'gck", 2500667793),
            (&["fellowship_of_leaves", "hill_giant"], 99999, "Retha Streamis", "Yvganwenra the Black", 696363170),
            (&["fellowship_of_leaves", "hill_giant"], 31337, "Arcionman Arcwen", "Mardalofta of Yvganenen", 2147357987),
            (&["fellowship_of_leaves", "hill_giant"], 2026, "Anenwenir the Young", "Kytheat Seekerelm", 3469224078),
            (&["fellowship_of_leaves", "hill_giant"], 65535, "Jariofted the Strong", "Kyus Streamon", 1398945154),
            (&["runes_of_kilmorph", "lizardman"], 1, "Dages", "Thendain", 3655513600),
            (&["runes_of_kilmorph", "lizardman"], 7, "Arcandan", "Carcheen Carthe", 523310796),
            (&["runes_of_kilmorph", "lizardman"], 42, "Quinain", "Takerthousand Guardclaw", 1314989459),
            (&["runes_of_kilmorph", "lizardman"], 1234, "Ta'entha'dain", "Ha'chaCo'cha", 2998352411),
            (&["runes_of_kilmorph", "lizardman"], 99999, "Ar'thaCar'tha", "Hssowdon", 3923225239),
            (&["runes_of_kilmorph", "lizardman"], 31337, "Arcionck Arcan", "Margtisa of Kidalganen", 2147357987),
            (&["runes_of_kilmorph", "lizardman"], 2026, "Ananenur the Brave", "Khelthechegit the Seeker", 3469224078),
            (&["runes_of_kilmorph", "lizardman"], 65535, "Ja'daltis'ed", "Thdain", 3697340407),
            (&["the_empyrean", "fire_elemental_or_azer"], 1, "Rivershield", "Hsswencha the Moonslayer", 3210001534),
            (&["the_empyrean", "fire_elemental_or_azer"], 7, "Arcandon the Loverold", "Handred", 523310796),
            (&["the_empyrean", "fire_elemental_or_azer"], 42, "Betir the Halfbrave", "Cratioan the Riverblack", 1108520142),
            (&["the_empyrean", "fire_elemental_or_azer"], 1234, "Spiritspit Anckr", "Honorgreat", 2998352411),
            (&["the_empyrean", "fire_elemental_or_azer"], 99999, "Hattwenna", "Flamescorch Bossel", 696363170),
            (&["the_empyrean", "fire_elemental_or_azer"], 31337, "Shieldgrey", "Boner", 1360843272),
            (&["the_empyrean", "fire_elemental_or_azer"], 2026, "Kenenman", "Rewenckis", 542493267),
            (&["the_empyrean", "fire_elemental_or_azer"], 65535, "Faceflame Hecheth", "Sanwenon", 1398945154),
            (&["council_of_esus", "water_elemental", "evil"], 1, "Painwit", "Swwenu the Evilfoul", 3210001534),
            (&["council_of_esus", "water_elemental", "evil"], 7, "Arcandon the Fearwhite", "Thousanddark", 523310796),
            (&["council_of_esus", "water_elemental", "evil"], 42, "Begcha the Slywhite", "Drtioon the Hatedark", 1108520142),
            (&["council_of_esus", "water_elemental", "evil"], 1234, "Soulspirit Anoter", "Oozecruel", 2998352411),
            (&["council_of_esus", "water_elemental", "evil"], 99999, "Reibenna", "Seatide Brioel", 696363170),
            (&["council_of_esus", "water_elemental", "evil"], 31337, "Summerdark", "Brider", 1360843272),
            (&["council_of_esus", "water_elemental", "evil"], 2026, "Kenenman", "Rewenois", 542493267),
            (&["council_of_esus", "water_elemental", "evil"], 65535, "Furylake Haoftun", "Whonener", 1398945154),
            (&["demon", "air_elemental"], 1, "Bringerrod", "Fflialis the Makeryoung", 3210001534),
            (&["demon", "air_elemental"], 7, "Adionnd the Breathbrave", "Breathred", 523310796),
            (&["demon", "air_elemental"], 42, "Beaauz the Starstrong", "Zganat the Makerblack", 1108520142),
            (&["demon", "air_elemental"], 1234, "Spiritblow Konccph", "Halfyoung", 2998352411),
            (&["demon", "air_elemental"], 99999, "Dagohraxel", "Windzephyr Quipheus", 696363170),
            (&["demon", "air_elemental"], 31337, "Rodblack", "Quiased", 1360843272),
            (&["demon", "air_elemental"], 2026, "Kondalda", "Danraxcces", 542493267),
            (&["demon", "air_elemental"], 65535, "Streamair Caronan", "Shebraxed", 1398945154),
            (&["earth_elemental", "evil"], 1, "Reignspring", "Phenda the Snarewhite", 3210001534),
            (&["earth_elemental", "evil"], 7, "Arcandon the Halfblack", "Footfoul", 523310796),
            (&["earth_elemental", "evil"], 42, "Bencha the Miseryblack", "Groionon the Reigndark", 1108520142),
            (&["earth_elemental", "evil"], 1234, "Soulspirit Anowg", "Spawnwhite", 2998352411),
            (&["earth_elemental", "evil"], 99999, "Hauanna", "Boundrock Boowel", 696363170),
            (&["earth_elemental", "evil"], 31337, "Springdark", "Booner", 1360843272),
            (&["earth_elemental", "evil"], 2026, "Kenanle", "Reanowis", 542493267),
            (&["earth_elemental", "evil"], 65535, "Hateooze Hetisun", "Saonaner", 1398945154),
            (&["naval", "the_order"], 1, "The Confessorautumn", "The Venir", 3655513600),
            (&["naval", "the_order"], 7, "The Arcandan", "The River Bringer", 2099423262),
            (&["naval", "the_order"], 42, "The Manuel", "The Ardin", 3884214914),
            (&["naval", "the_order"], 1234, "Spirit's Spirit", "The Keelreef", 4265552588),
            (&["naval", "the_order"], 99999, "Bladegull", "The Staff Gull", 100603441),
            (&["naval", "the_order"], 31337, "The Shield Tusk", "The Stream Crusader", 2842418091),
            (&["naval", "the_order"], 2026, "The Winter Hunter", "The Jolly Great", 1877955876),
            (&["naval", "the_order"], 65535, "Half's Stern", "The Breath Tusk", 1178322065),
            (&["demon"], 1, "Bethabccus", "Quias'as'daasar", 180171308),
            (&["demon"], 7, "Anent'ent'reentck", "Dagd'Andda", 2862681365),
            (&["demon"], 42, "Deal'al'elalir", "Adrphi'Ofted", 1535244752),
            (&["demon"], 1234, "Taeeionalles the Tuskgrea", "Ahow'Wensta", 285564151),
            (&["demon"], 99999, "Briee'ee'coeeis", "Elphitalie the Takergrey", 1409266928),
            (&["demon"], 31337, "Antisttda", "Baenalzebul", 3953320669),
            (&["demon"], 2026, "Adg'Owres", "Bezeandona", 2705850505),
            (&["demon"], 65535, "Aazeenenon the Oneslayer", "Coche'Aaman", 2073574608),
            (&["demon", "enraged", "evil"], 1, "Gruentalpheu", "Nori'ri'darista", 180171308),
            (&["demon", "enraged", "evil"], 7, "Adion'ion'beliongit", "Cot'Thae", 2862681365),
            (&["demon", "enraged", "evil"], 42, "Ahlia'lia'euliaesh", "Aaze'Ganat", 1535244752),
            (&["demon", "enraged", "evil"], 1234, "Taeetioaales the Takerbre", "Abgg'Nna", 285564151),
            (&["demon", "enraged", "evil"], 99999, "Danee'ee'saeeel", "Euphickaaus the Murderbla", 1409266928),
            (&["demon", "enraged", "evil"], 31337, "Adcheckcke", "Dedliaalgon", 3953320669),
            (&["demon", "enraged", "evil"], 2026, "Kong'Gglu", "Grualthagir", 2705850505),
            (&["demon", "enraged", "evil"], 65535, "Kaalddre the Ragebreaker", "Saan'Bgit", 2073574608),
            (&["lizardman", "naval"], 1, "The Slayerautumn", "The Sthganess", 3655513600),
            (&["lizardman", "naval"], 7, "The Arcthean", "The Stream Maker", 2099423262),
            (&["lizardman", "naval"], 42, "The Coweniss", "The Balwenath", 3884214914),
            (&["lizardman", "naval"], 1234, "Spirit's Spirit", "The Keelreef", 4265552588),
            (&["lizardman", "naval"], 99999, "Handgull", "The Staff Gull", 100603441),
            (&["lizardman", "naval"], 31337, "The Autumn Tusk", "The Moon Seeker", 2842418091),
            (&["lizardman", "naval"], 2026, "The Winter Great", "The Jolly Old", 1877955876),
            (&["lizardman", "naval"], 65535, "Hundred's Bow", "The Blade Tusk", 1178322065),
        ];
        for &(flags, seed, first, second, end_seed) in rows {
            let lists = marnok_lists(&marnok_flags(flags));
            let mut rng = Rng::new(seed);
            let a = marnok_name(&lists, &mut |n| rng.get_i(n, "Name Gen"));
            let b = marnok_name(&lists, &mut |n| rng.get_i(n, "Name Gen"));
            assert_eq!((a.as_str(), b.as_str(), rng.seed()), (first, second, end_seed), "flags {flags:?}, seed {seed}");
        }
    }

    /// Every type name hard-coded in this file (string literals with a type prefix) exists in
    /// the game data, in the class its prefix names (Python `getInfoTypeForString` never -1).
    #[test]
    fn hard_coded_type_names_resolve() {
        let d = ffh_data::global();
        let mut checked = 0;
        let mut missing = Vec::new();
        for line in include_str!("custom.rs").lines() {
            let code = line.split("//").next().unwrap_or("");
            for (i, s) in code.split('"').enumerate() {
                let is_name = i % 2 == 1
                    && s.len() > 2
                    && s.contains('_')
                    && !s.ends_with('_')
                    && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if !is_name {
                    continue;
                }
                let found = if s.starts_with("UNITCLASS_") {
                    d.ty::<UnitClassType>(s).is_some()
                } else if s.starts_with("UNITCOMBAT_") {
                    d.ty::<UnitCombatType>(s).is_some()
                } else if s.starts_with("UNITAI_") {
                    d.ty::<UnitAIType>(s).is_some()
                } else if s.starts_with("UNIT_") || s.starts_with("EQUIPMENT_") {
                    d.ty::<UnitType>(s).is_some()
                } else if s.starts_with("PROMOTION_") {
                    d.ty::<PromotionType>(s).is_some()
                } else if s.starts_with("GOODY_") {
                    d.ty::<GoodyType>(s).is_some()
                } else if s.starts_with("BONUS_") {
                    d.ty::<BonusType>(s).is_some()
                } else if s.starts_with("TERRAIN_") {
                    d.ty::<TerrainType>(s).is_some()
                } else if s.starts_with("FEATURE_") {
                    d.ty::<FeatureType>(s).is_some()
                } else if s.starts_with("IMPROVEMENT_") {
                    d.ty::<ImprovementType>(s).is_some()
                } else if s.starts_with("TECH_") {
                    d.ty::<TechType>(s).is_some()
                } else if s.starts_with("CIVILIZATION_") {
                    d.ty::<CivilizationType>(s).is_some()
                } else if s.starts_with("RELIGION_") {
                    d.ty::<ReligionType>(s).is_some()
                } else if s.starts_with("ALIGNMENT_") {
                    d.ty::<AlignmentType>(s).is_some()
                } else if s.starts_with("BUILDING_") {
                    d.ty::<BuildingType>(s).is_some()
                } else if s.starts_with("SPELL_") {
                    d.ty::<SpellType>(s).is_some()
                } else if s.starts_with("DAMAGE_") {
                    d.ty::<DamageType>(s).is_some()
                } else if s.starts_with("EVENTTRIGGER_") {
                    d.ty::<EventTriggerType>(s).is_some()
                } else {
                    continue; // goody keys (SPAWN_X, PRISONER_X, ...) and text keys
                };
                checked += 1;
                if !found {
                    missing.push(s.to_string());
                }
            }
        }
        assert!(missing.is_empty(), "unknown type names: {missing:?}");
        assert!(checked > 250, "only {checked} names checked");
    }

    #[test]
    fn player_slot_lookups() {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, 3, "WORLDSIZE_DUEL", 7);
        let khazad = d.t::<CivilizationType>("CIVILIZATION_KHAZAD");
        let sheaim = d.t::<CivilizationType>("CIVILIZATION_SHEAIM");
        let arendel = d.t::<LeaderHeadType>("LEADER_ARENDEL");
        setup.players[0].civ = Some(khazad);
        setup.players[1].civ = Some(sheaim);
        setup.players[2].civ = Some(khazad);
        setup.players[1].leader = Some(arendel);
        let mut g = Game::new(d, setup);
        // getCivilization / getLeader return the last matching slot.
        assert_eq!(g.cf_get_civilization(khazad), Some(PlayerId(2)));
        assert_eq!(g.cf_get_civilization(d.t::<CivilizationType>("CIVILIZATION_LANUN")), None);
        assert_eq!(g.cf_get_leader(arendel), Some(PlayerId(1)));
        assert_eq!(g.cf_get_hero(PlayerId(0)), Some(d.t::<UnitClassType>("UNITCLASS_MAROS")));
        assert_eq!(g.cf_get_hero(PlayerId(1)), Some(d.t::<UnitClassType>("UNITCLASS_ABASHI")));
        assert_eq!(g.cf_get_hero(PlayerId(3)), None);
        // getOpenPlayer: the first never-alive slot.
        for p in [0, 1, 2] {
            g.players[PlayerId(p)].ever_alive = true;
        }
        assert_eq!(g.cf_get_open_player(), Some(PlayerId(3)));
        for p in g.player_ids() {
            g.players[p].ever_alive = true;
        }
        assert_eq!(g.cf_get_open_player(), None);
    }

    #[test]
    fn grace_period() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let base = 20 * (g.game_speed_type().i() + 1) * (d.handicaps.len() as i32 + 1 - g.handicap.i());
        let mut rng = g.soren_rand.clone();
        let roll = rng.get_i(base, "grace");
        g.setup.game_turn = base + roll - 1;
        assert!(g.cf_grace());
        assert_eq!(g.soren_rand, rng);
        let roll = rng.get_i(base, "grace");
        g.setup.game_turn = base + roll;
        assert!(!g.cf_grace());
        assert_eq!(g.soren_rand, rng);
    }
}
