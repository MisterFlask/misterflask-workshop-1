//! Global type-string registry (C++ `GC.getInfoTypeForString`) and the XML
//! reader helpers (port of `CvXMLLoadUtility`).
//!
//! Loading is two-pass: pass 1 registers every `<Type>` of every info file
//! (so forward references resolve without Civ4's readPass2/readPass3 hacks),
//! pass 2 runs the per-class `read` functions through [`Rd`].

use std::cell::RefCell;
use std::collections::HashMap;

use crate::enums::*;
use crate::idx::{Idx, IdxVec};
use crate::xml::Elem;

/// Associates a typed index with the registry class it indexes.
pub trait InfoId: Idx {
    /// Registry class name (the XML entry element name, e.g. "UnitInfo",
    /// or a GlobalTypes list name, e.g. "FlavorTypes").
    const CLASS: &'static str;
}

macro_rules! info_ids {
    ($($t:ty => $c:literal),* $(,)?) => { $(impl InfoId for $t { const CLASS: &'static str = $c; })* };
}

info_ids! {
    AlignmentType => "AlignmentInfo",
    DamageType => "DamageInfo",
    InvisibleType => "InvisibleInfo",
    MemoryType => "MemoryInfo",
    UnitAIType => "UnitAIInfo",
    UnitCombatType => "UnitCombatInfo",
    AttitudeType => "AttitudeInfo",
    DenialType => "DenialInfo",
    DomainType => "DomainInfo",
    CalendarType => "CalendarInfo",
    SeasonType => "SeasonInfo",
    MonthType => "MonthInfo",
    BuildingClassType => "BuildingClassInfo",
    BuildingType => "BuildingInfo",
    SpecialBuildingType => "SpecialBuildingInfo",
    CivilizationType => "CivilizationInfo",
    LeaderHeadType => "LeaderHeadInfo",
    TraitType => "TraitInfo",
    EventType => "EventInfo",
    EventTriggerType => "EventTriggerInfo",
    CivicType => "CivicInfo",
    CivicOptionType => "CivicOptionInfo",
    CommerceType => "CommerceInfo",
    CorporationType => "CorporationInfo",
    CultureLevelType => "CultureLevelInfo",
    EmphasizeType => "EmphasizeInfo",
    EraType => "EraInfo",
    EspionageMissionType => "EspionageMissionInfo",
    GameOptionType => "GameOptionInfo",
    GameSpeedType => "GameSpeedInfo",
    GoodyType => "GoodyInfo",
    HandicapType => "HandicapInfo",
    ProcessType => "ProcessInfo",
    ProjectType => "ProjectInfo",
    ReligionType => "ReligionInfo",
    SpecialistType => "SpecialistInfo",
    VictoryType => "VictoryInfo",
    VoteType => "VoteInfo",
    VoteSourceType => "VoteSourceInfo",
    WorldSizeType => "WorldInfo",
    HurryType => "HurryInfo",
    UpkeepType => "UpkeepInfo",
    ClimateType => "ClimateInfo",
    SeaLevelType => "SeaLevelInfo",
    RouteType => "RouteInfo",
    TechType => "TechInfo",
    BonusClassType => "BonusClassInfo",
    BonusType => "BonusInfo",
    FeatureType => "FeatureInfo",
    ImprovementType => "ImprovementInfo",
    TerrainType => "TerrainInfo",
    YieldType => "YieldInfo",
    AutomateType => "AutomateInfo",
    BuildType => "BuildInfo",
    CommandType => "CommandInfo",
    MissionType => "MissionInfo",
    PromotionType => "PromotionInfo",
    SpecialUnitType => "SpecialUnitInfo",
    SpellType => "SpellInfo",
    UnitClassType => "UnitClassInfo",
    UnitType => "UnitInfo",
    FlavorType => "FlavorTypes",
    ArtStyleType => "ArtStyleTypes",
    CitySizeType => "CitySizeTypes",
    ContactType => "ContactTypes",
    DiplomacyPowerType => "DiplomacyPowerTypes",
    DirectionType => "DirectionTypes",
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegEntry {
    pub class: &'static str,
    pub index: u16,
}

#[derive(Default, Debug)]
pub struct Registry {
    map: HashMap<String, RegEntry>,
    /// per class: ordered type names
    names: HashMap<&'static str, Vec<String>>,
    /// duplicate registrations (name, first class, second class)
    pub duplicates: Vec<(String, &'static str, &'static str)>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the types of a class in order. Returns the count.
    pub fn register_class(&mut self, class: &'static str, types: Vec<String>) -> usize {
        for (i, t) in types.iter().enumerate() {
            if t.is_empty() {
                continue;
            }
            let e = RegEntry { class, index: i as u16 };
            if let Some(prev) = self.map.insert(t.clone(), e) {
                if prev.class != class || prev.index != e.index {
                    self.duplicates.push((t.clone(), prev.class, class));
                }
            }
        }
        let n = types.len();
        self.names.insert(class, types);
        n
    }

    /// C++ `GC.getInfoTypeForString`: index or -1. Class-agnostic.
    pub fn info_type_for_string(&self, s: &str) -> i32 {
        self.map.get(s).map(|e| e.index as i32).unwrap_or(-1)
    }

    pub fn entry(&self, s: &str) -> Option<RegEntry> {
        self.map.get(s).copied()
    }

    /// Typed lookup; `None` if missing or registered under another class.
    pub fn lookup<T: InfoId>(&self, s: &str) -> Option<T> {
        match self.map.get(s) {
            Some(e) if e.class == T::CLASS => Some(T::from_usize(e.index as usize)),
            _ => None,
        }
    }

    pub fn count<T: InfoId>(&self) -> usize {
        self.names.get(T::CLASS).map(|v| v.len()).unwrap_or(0)
    }

    pub fn count_class(&self, class: &str) -> usize {
        self.names.get(class).map(|v| v.len()).unwrap_or(0)
    }

    pub fn type_name<T: InfoId>(&self, t: T) -> &str {
        self.names.get(T::CLASS).and_then(|v| v.get(t.idx())).map(|s| s.as_str()).unwrap_or("?")
    }

    pub fn names_of(&self, class: &str) -> &[String] {
        self.names.get(class).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Load context shared by all readers of one file.
pub struct Ctx<'r> {
    pub reg: &'r Registry,
    pub file: String,
    pub warnings: RefCell<Vec<String>>,
}

impl<'r> Ctx<'r> {
    pub fn new(reg: &'r Registry, file: impl Into<String>) -> Self {
        Ctx { reg, file: file.into(), warnings: RefCell::new(Vec::new()) }
    }
    pub fn warn(&self, msg: String) {
        self.warnings.borrow_mut().push(format!("{}: {}", self.file, msg));
    }
}

/// Reader over one XML element; the methods mirror `CvXMLLoadUtility`.
#[derive(Clone, Copy)]
pub struct Rd<'a> {
    pub e: Elem<'a>,
    pub cx: &'a Ctx<'a>,
}

fn parse_int(s: &str) -> Option<i32> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<i32>().ok().or_else(|| t.parse::<f64>().ok().map(|f| f as i32))
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" | "True" | "TRUE" => Some(true),
        "0" | "false" | "False" | "FALSE" => Some(false),
        "" => None,
        other => parse_int(other).map(|i| i != 0),
    }
}

impl<'a> Rd<'a> {
    pub fn new(e: Elem<'a>, cx: &'a Ctx<'a>) -> Self {
        Rd { e, cx }
    }

    pub fn reg(&self) -> &'a Registry {
        self.cx.reg
    }

    pub fn child(&self, tag: &str) -> Option<Rd<'a>> {
        self.e.child(tag).map(|e| Rd { e, cx: self.cx })
    }

    pub fn children(&self) -> impl Iterator<Item = Rd<'a>> + 'a {
        let cx = self.cx;
        self.e.children().map(move |e| Rd { e, cx })
    }

    pub fn children_named(&self, tag: &'a str) -> impl Iterator<Item = Rd<'a>> + 'a {
        let cx = self.cx;
        self.e.children().filter(move |c| c.name() == tag).map(move |e| Rd { e, cx })
    }

    pub fn text(&self) -> &'a str {
        self.e.text()
    }

    pub fn has(&self, tag: &str) -> bool {
        self.e.child(tag).is_some()
    }

    // ---- GetChildXmlValByName ----

    pub fn int(&self, tag: &str) -> i32 {
        self.int_or(tag, 0)
    }
    pub fn int_or(&self, tag: &str, def: i32) -> i32 {
        match self.e.child(tag) {
            Some(c) => match parse_int(c.text()) {
                Some(v) => v,
                None => {
                    if !c.text().is_empty() {
                        self.cx.warn(format!("bad int in <{}>: {:?}", tag, c.text()));
                    }
                    def
                }
            },
            None => def,
        }
    }
    pub fn bool(&self, tag: &str) -> bool {
        self.bool_or(tag, false)
    }
    pub fn bool_or(&self, tag: &str, def: bool) -> bool {
        match self.e.child(tag) {
            Some(c) => parse_bool(c.text()).unwrap_or(def),
            None => def,
        }
    }
    pub fn float(&self, tag: &str) -> f32 {
        self.float_or(tag, 0.0)
    }
    pub fn float_or(&self, tag: &str, def: f32) -> f32 {
        match self.e.child(tag) {
            Some(c) => c.text().trim().parse::<f32>().unwrap_or(def),
            None => def,
        }
    }
    /// String value ("" if absent).
    pub fn string(&self, tag: &str) -> String {
        self.e.child(tag).map(|c| c.text().to_string()).unwrap_or_default()
    }
    pub fn string_or(&self, tag: &str, def: &str) -> String {
        self.e.child(tag).map(|c| c.text().to_string()).unwrap_or_else(|| def.to_string())
    }

    // ---- FindInInfoClass ----

    /// Class-agnostic index or -1 (C++ `FindInInfoClass`). Warns on unknown non-empty names.
    pub fn find_in_info_class(&self, s: &str) -> i32 {
        let idx = self.cx.reg.info_type_for_string(s);
        if idx < 0 && !s.is_empty() && s != "NONE" {
            self.cx.warn(format!("unknown type {:?}", s));
        }
        idx
    }

    /// Resolves a type string to a typed index; warns if unknown or of another class.
    pub fn resolve<T: InfoId>(&self, s: &str) -> Option<T> {
        let s = s.trim();
        if s.is_empty() || s == "NONE" {
            return None;
        }
        match self.cx.reg.entry(s) {
            Some(e) if e.class == T::CLASS => Some(T::from_usize(e.index as usize)),
            Some(e) => {
                self.cx.warn(format!("type {:?} is a {} but a {} was expected", s, e.class, T::CLASS));
                None
            }
            None => {
                self.cx.warn(format!("unknown type {:?} (expected {})", s, T::CLASS));
                None
            }
        }
    }

    /// `GetChildXmlValByName(szTextVal, tag); m_iX = FindInInfoClass(szTextVal);` with a typed result.
    pub fn rf<T: InfoId>(&self, tag: &str) -> Option<T> {
        match self.e.child(tag) {
            Some(c) => self.resolve::<T>(c.text()),
            None => None,
        }
    }

    // ---- SetYields / SetCommerce ----

    /// Positional int list under `tag` (C++ `SetYields`/`SetCommerce`/`SetAndLoadVar`),
    /// padded/truncated to `n` with zeros. Absent tag -> all zeros.
    pub fn positional_ints(&self, tag: &str, n: usize) -> Vec<i32> {
        let mut v = vec![0; n];
        if let Some(c) = self.e.child(tag) {
            for (i, ch) in c.children().enumerate() {
                if i >= n {
                    self.cx.warn(format!("<{}> has more than {} values", tag, n));
                    break;
                }
                v[i] = parse_int(ch.text()).unwrap_or(0);
            }
        }
        v
    }
    pub fn yields(&self, tag: &str) -> [i32; YieldType::NUM] {
        let v = self.positional_ints(tag, YieldType::NUM);
        [v[0], v[1], v[2]]
    }
    pub fn commerces(&self, tag: &str) -> [i32; CommerceType::NUM] {
        let v = self.positional_ints(tag, CommerceType::NUM);
        [v[0], v[1], v[2], v[3]]
    }
    /// Positional bool list (e.g. `CommerceFlexible`).
    pub fn positional_bools(&self, tag: &str, n: usize) -> Vec<bool> {
        let mut v = vec![false; n];
        if let Some(c) = self.e.child(tag) {
            for (i, ch) in c.children().enumerate() {
                if i < n {
                    v[i] = parse_bool(ch.text()).unwrap_or(false);
                }
            }
        }
        v
    }

    // ---- SetVariableListTagPair ----
    // Each child of <tag> is a pair element whose FIRST child holds a type name
    // (the key) and whose SECOND child holds the value; tag names of the pair are
    // ignored, exactly like the C++ implementation.

    fn pair_iter(&self, tag: &str) -> Vec<(Elem<'a>, Option<Elem<'a>>)> {
        let mut out = Vec::new();
        if let Some(c) = self.e.child(tag) {
            for pair in c.children() {
                let mut it = pair.children();
                if let Some(k) = it.next() {
                    out.push((k, it.next()));
                }
            }
        }
        out
    }

    pub fn pairs_int<K: InfoId>(&self, tag: &str, def: i32) -> IdxVec<K, i32> {
        let n = self.cx.reg.count::<K>();
        let mut v = IdxVec::from_elem(def, n);
        for (k, val) in self.pair_iter(tag) {
            if let Some(key) = self.resolve::<K>(k.text()) {
                // C++ GetNextXmlVal(&p[i]) defaults to 0 (not the list default) when the
                // value element is missing.
                v[key] = val.and_then(|x| parse_int(x.text())).unwrap_or(0);
            }
        }
        v
    }

    pub fn pairs_bool<K: InfoId>(&self, tag: &str) -> IdxVec<K, bool> {
        self.pairs_bool_or::<K>(tag, false)
    }

    pub fn pairs_bool_or<K: InfoId>(&self, tag: &str, def: bool) -> IdxVec<K, bool> {
        let n = self.cx.reg.count::<K>();
        let mut v = IdxVec::from_elem(def, n);
        for (k, val) in self.pair_iter(tag) {
            if let Some(key) = self.resolve::<K>(k.text()) {
                v[key] = val.and_then(|x| parse_bool(x.text())).unwrap_or(false);
            }
        }
        v
    }

    pub fn pairs_float<K: InfoId>(&self, tag: &str, def: f32) -> IdxVec<K, f32> {
        let n = self.cx.reg.count::<K>();
        let mut v = IdxVec::from_elem(def, n);
        for (k, val) in self.pair_iter(tag) {
            if let Some(key) = self.resolve::<K>(k.text()) {
                v[key] = val.and_then(|x| x.text().trim().parse().ok()).unwrap_or(0.0);
            }
        }
        v
    }

    pub fn pairs_string<K: InfoId>(&self, tag: &str) -> IdxVec<K, String> {
        let n = self.cx.reg.count::<K>();
        let mut v = IdxVec::from_elem(String::new(), n);
        for (k, val) in self.pair_iter(tag) {
            if let Some(key) = self.resolve::<K>(k.text()) {
                v[key] = val.map(|x| x.text().to_string()).unwrap_or_default();
            }
        }
        v
    }

    /// Pair list whose value is itself a type reference (C++ reads a string list then
    /// `FindInInfoClass`es each entry, e.g. `TerrainPassableTechs`).
    pub fn pairs_ref<K: InfoId, V: InfoId>(&self, tag: &str) -> IdxVec<K, Option<V>> {
        let n = self.cx.reg.count::<K>();
        let mut v = IdxVec::from_elem(None, n);
        for (k, val) in self.pair_iter(tag) {
            if let Some(key) = self.resolve::<K>(k.text()) {
                v[key] = val.and_then(|x| self.resolve::<V>(x.text()));
            }
        }
        v
    }

    /// Sparse pair list: keeps (key, int) in document order.
    pub fn pair_list_int<K: InfoId>(&self, tag: &str) -> Vec<(K, i32)> {
        let mut out = Vec::new();
        for (k, val) in self.pair_iter(tag) {
            if let Some(key) = self.resolve::<K>(k.text()) {
                out.push((key, val.and_then(|x| parse_int(x.text())).unwrap_or(0)));
            }
        }
        out
    }

    // ---- lists ----

    /// List of type references, one per child of `tag` (e.g. `<TechTypes><PrereqTech>..`).
    /// Unknown/NONE entries are skipped.
    pub fn ref_list<T: InfoId>(&self, tag: &str) -> Vec<T> {
        let mut out = Vec::new();
        if let Some(c) = self.e.child(tag) {
            for ch in c.children() {
                if let Some(t) = self.resolve::<T>(ch.text()) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// Fixed-size list of type references with -1 fill (C++ `InitList(&p, N, -1)` + positional
    /// `FindInInfoClass`), returned as a Vec<Option<T>> of length `n`.
    pub fn ref_list_fixed<T: InfoId>(&self, tag: &str, n: usize) -> Vec<Option<T>> {
        let mut out = vec![None; n];
        if let Some(c) = self.e.child(tag) {
            for (i, ch) in c.children().enumerate() {
                if i < n {
                    out[i] = self.resolve::<T>(ch.text());
                } else {
                    self.cx.warn(format!("<{}> has more than {} entries", tag, n));
                }
            }
        }
        out
    }

    /// C++ `SetStringList`: text of every child.
    pub fn string_list(&self, tag: &str) -> Vec<String> {
        self.e
            .child(tag)
            .map(|c| c.children().map(|ch| ch.text().to_string()).collect())
            .unwrap_or_default()
    }

    /// Comma-separated list of type references in a single tag (e.g. `SeeInvisible`).
    pub fn ref_tokens<T: InfoId>(&self, tag: &str) -> Vec<T> {
        let s = self.string(tag);
        s.split(',').filter_map(|tok| {
            let tok = tok.trim();
            if tok.is_empty() { None } else { self.resolve::<T>(tok) }
        }).collect()
    }
}
