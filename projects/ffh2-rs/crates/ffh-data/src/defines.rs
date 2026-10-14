//! Global defines (C++ `GC.getDefineINT` / `getDefineFLOAT` / `getDefineSTRING`).
//!
//! Files are read in C++ order -- GlobalDefines.xml, GlobalDefinesAlt.xml,
//! PythonCallbackDefines.xml -- later files overriding earlier ones. Text values
//! of the defines listed in [`POST_GLOBALS_TEXT_DEFINES`] resolve to the named type's
//! index (C++ `SetPostGlobalsGlobalDefines`); every other text define stays a string.
//! Prefer the typed [`crate::Defines`] struct (generated) in game code.

use std::collections::HashMap;

use crate::registry::Registry;
use crate::xml::XmlDoc;

/// The text defines C++ `CvXMLLoadUtility::SetPostGlobalsGlobalDefines` converts to type
/// indices (`FindInInfoClass`), in its order. Only these become ints in C++.
///
/// C++ quirk: any other text define keeps its string variable, and `getDefineINT` on it
/// reads the union member `m_iValue` of that variable, i.e. the address of the string --
/// never a valid type index. The only such define the rules read as an int is
/// `FLOOD_PLAINS_FEATURE` (`CvCity::init`, the Unofficial Patch "don't remove flood plains
/// when founding a city" check), so in the original DLL that check never matches and a city
/// founded on flood plains removes them. Unlisted text defines therefore read as -1 here
/// (matches no type), not as the index of the type they name.
pub const POST_GLOBALS_TEXT_DEFINES: &[&str] = &[
    "LAND_TERRAIN",
    "DEEP_WATER_TERRAIN",
    "SHALLOW_WATER_TERRAIN",
    "LAND_IMPROVEMENT",
    "WATER_IMPROVEMENT",
    "RUINS_IMPROVEMENT",
    "NUKE_FEATURE",
    "GLOBAL_WARMING_TERRAIN",
    "CAPITAL_BUILDINGCLASS",
    "DEFAULT_SPECIALIST",
    "INITIAL_CITY_ROUTE_TYPE",
    "STANDARD_HANDICAP",
    "STANDARD_HANDICAP_QUICK",
    "STANDARD_GAMESPEED",
    "STANDARD_TURNTIMER",
    "STANDARD_CLIMATE",
    "STANDARD_SEALEVEL",
    "STANDARD_ERA",
    "STANDARD_CALENDAR",
    "AI_HANDICAP",
    "BARBARIAN_HANDICAP",
    "BARBARIAN_CIVILIZATION",
    "BARBARIAN_LEADER",
    // FfH (Kael)
    "BONUS_MANA",
    "BONUSCLASS_MANA",
    "BONUSCLASS_MANA_RAW",
    "INVISIBLE_TYPE",
    "FLAMES_EXPIRE_EFFECT",
    "FLAMES_FEATURE",
    "FLAMES_SPREAD_EFFECT",
    "GREAT_COMMANDER_PROMOTION",
    "HIDDEN_NATIONALITY_PROMOTION",
    "MUTATED_PROMOTION",
    "POISONED_PROMOTION",
    "RANDOM_CIVILIZATION",
    "SPECIALUNIT_SPELL",
    "STARTING_SETTLER_PROMOTION",
    "SLAVE_UNIT",
    "WEAPON_PROMOTION_TIER1",
    "WEAPON_PROMOTION_TIER2",
    "WEAPON_PROMOTION_TIER3",
    "WEAPON_REQ_BONUS_TIER1",
    "WEAPON_REQ_BONUS_TIER2",
    "WEAPON_REQ_BONUS_TIER3",
    // Better AI (Skyre, Sephi; rewritten by Kael)
    "PIRATE_COVE_IMPROVEMENT",
    "BUILD_FORT",
    "CIVILIZATION_KHAZAD",
    "CIVILIZATION_LANUN",
    "PROMOTION_CHANNELING1",
    "PROMOTION_CHANNELING3",
    "SPELL_CONVERT_CITY_RANTINE",
    "SPELL_DISRUPT",
    "SPELL_FEAST",
    "SPELL_HASTE",
    "SPELL_TEACH_SPELLCASTING",
    "TECH_FESTIVALS",
    "TECH_TAXATION",
    "TECH_WAY_OF_THE_EARTHMOTHER",
    "UNIT_FIREBALL",
    "UNIT_GOVANNON",
    "UNIT_LOKI",
    "UNIT_PYRE_ZOMBIE",
    "UNIT_RANTINE",
    "UNIT_VAMPIRE",
    "UNIT_VAMPIRE_LORD",
    "UNITCLASS_SCOUT",
];

#[derive(Debug, Clone)]
pub enum DefineVal {
    Int(i32),
    Float(f32),
    /// Text value and what C++ `getDefineINT` yields for it: the resolved type index for a
    /// define in [`POST_GLOBALS_TEXT_DEFINES`] (-1 if the text names no type), -1 for any
    /// other text define (see the quirk note there).
    Text(String, i32),
}

#[derive(Debug, Clone, Default)]
pub struct DefineMap {
    pub map: HashMap<String, DefineVal>,
}

impl DefineMap {
    pub fn read_file(&mut self, doc: &XmlDoc, reg: &Registry) {
        for d in doc.root().children() {
            let name = match d.child_text("DefineName") {
                Some(n) => n.to_string(),
                None => continue,
            };
            let mut val = None;
            for c in d.children() {
                let t = c.text();
                match c.name() {
                    "iDefineIntVal" => val = Some(DefineVal::Int(t.parse().unwrap_or(0))),
                    "fDefineFloatVal" => val = Some(DefineVal::Float(t.parse().unwrap_or(0.0))),
                    "DefineTextVal" => {
                        let idx = if POST_GLOBALS_TEXT_DEFINES.contains(&name.as_str()) {
                            reg.info_type_for_string(t)
                        } else {
                            -1
                        };
                        val = Some(DefineVal::Text(t.to_string(), idx))
                    }
                    "bDefineBoolVal" => val = Some(DefineVal::Int((t == "1") as i32)),
                    _ => {}
                }
            }
            if let Some(v) = val {
                self.map.insert(name, v);
            }
        }
    }

    /// C++ `getDefineINT`: 0 when missing.
    pub fn int(&self, name: &str) -> i32 {
        match self.map.get(name) {
            Some(DefineVal::Int(i)) => *i,
            Some(DefineVal::Float(f)) => *f as i32,
            Some(DefineVal::Text(_, i)) => *i,
            None => 0,
        }
    }

    /// C++ `getDefineFLOAT`: 0.0 when missing.
    pub fn float(&self, name: &str) -> f32 {
        match self.map.get(name) {
            Some(DefineVal::Float(f)) => *f,
            Some(DefineVal::Int(i)) => *i as f32,
            _ => 0.0,
        }
    }

    /// C++ `getDefineSTRING`: "" when missing.
    pub fn string(&self, name: &str) -> &str {
        match self.map.get(name) {
            Some(DefineVal::Text(s, _)) => s,
            _ => "",
        }
    }
}
