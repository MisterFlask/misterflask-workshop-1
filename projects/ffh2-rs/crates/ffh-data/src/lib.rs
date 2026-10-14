//! FFH2 game data: loads the vendored Civ4/FFH2 XML into typed info tables.
//!
//! This is a port of the DLL's `CvInfos.cpp` + `CvXMLLoadUtility*.cpp`.
//! Field names mirror the C++ members (`m_iCombat` -> `combat`,
//! `m_piYieldChange` -> `yield_change`), so ported rules code maps
//! `kInfo.getCombat()` to `info.combat`.

pub mod defines;
pub mod defines_gen;
pub mod enums;
pub mod idx;
pub mod infos;
pub mod registry;
pub mod text;
pub mod xml;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub use defines::{DefineMap, DefineVal};
pub use defines_gen::Defines;
pub use enums::*;
pub use idx::{opt_from_i32, opt_to_i32, Idx, IdxVec};
pub use infos::*;
pub use registry::{Ctx, InfoId, Rd, Registry};
pub use text::Text;
use xml::XmlDoc;

/// Declares every info table: field, id type, info type, xml file, reader fn.
macro_rules! game_data {
    ($( $field:ident : $id:ty => $info:ty, $file:literal, $read:path; )*) => {
        /// All static game data. Load once, then share as `&'static GameData`.
        pub struct GameData {
            pub reg: Registry,
            pub define_map: DefineMap,
            pub defines: Defines,
            pub text: Text,
            /// Non-fatal data problems found while loading (unknown types etc.).
            pub warnings: Vec<String>,
            $( pub $field: IdxVec<$id, $info>, )*
        }

        /// (xml file, registry class) for every info table, in load order.
        pub const INFO_FILES: &[(&str, &str)] = &[ $( ($file, <$id as InfoId>::CLASS), )* ];

        impl GameData {
            fn load_tables(docs: &Docs, reg: Registry, define_map: DefineMap, text: Text) -> GameData {
                let mut warnings = Vec::new();
                $(
                    let $field: IdxVec<$id, $info> = {
                        let doc = docs.get($file);
                        let cx = Ctx::new(&reg, $file);
                        let mut v = IdxVec::new();
                        for e in entries(doc) {
                            let rd = Rd::new(e, &cx);
                            v.push($read(&rd));
                        }
                        warnings.extend(cx.warnings.into_inner());
                        v
                    };
                )*
                let defines = Defines::from_map(&define_map);
                GameData { reg, define_map, defines, text, warnings, $( $field, )* }
            }
        }
    };
}

game_data! {
    // ---- BasicInfos ----
    alignments: AlignmentType => AlignmentInfo, "Basicinfos/CIV4AlignmentInfos.xml", basic::read_alignment;
    attitudes: AttitudeType => AttitudeInfo, "Basicinfos/CIV4AttitudeInfos.xml", basic::read_attitude;
    calendars: CalendarType => CalendarInfo, "Basicinfos/CIV4CalendarInfos.xml", basic::read_calendar;
    damages: DamageType => DamageInfo, "Basicinfos/CIV4DamageInfos.xml", basic::read_damage;
    denials: DenialType => DenialInfo, "Basicinfos/CIV4DenialInfos.xml", basic::read_denial;
    domains: DomainType => DomainInfo, "Basicinfos/CIV4DomainInfos.xml", basic::read_domain;
    invisibles: InvisibleType => InvisibleInfo, "Basicinfos/CIV4InvisibleInfos.xml", basic::read_invisible;
    memories: MemoryType => MemoryInfo, "Basicinfos/CIV4MemoryInfos.xml", basic::read_memory;
    months: MonthType => MonthInfo, "Basicinfos/CIV4MonthInfos.xml", basic::read_month;
    seasons: SeasonType => SeasonInfo, "Basicinfos/CIV4SeasonInfos.xml", basic::read_season;
    unit_ais: UnitAIType => UnitAIInfo, "Basicinfos/CIV4UnitAIInfos.xml", basic::read_unit_ai;
    unit_combats: UnitCombatType => UnitCombatInfo, "Basicinfos/CIV4UnitCombatInfos.xml", basic::read_unit_combat;
    // ---- Terrain ----
    yields: YieldType => YieldInfo, "Terrain/CIV4YieldInfos.xml", terrain::read_yield;
    terrains: TerrainType => TerrainInfo, "Terrain/CIV4TerrainInfos.xml", terrain::read_terrain;
    features: FeatureType => FeatureInfo, "Terrain/CIV4FeatureInfos.xml", terrain::read_feature;
    bonus_classes: BonusClassType => BonusClassInfo, "Terrain/CIV4BonusClassInfos.xml", terrain::read_bonus_class;
    bonuses: BonusType => BonusInfo, "Terrain/CIV4BonusInfos.xml", terrain::read_bonus;
    improvements: ImprovementType => ImprovementInfo, "Terrain/CIV4ImprovementInfos.xml", terrain::read_improvement;
    routes: RouteType => RouteInfo, "Misc/CIV4RouteInfos.xml", terrain::read_route;
    // ---- GameInfo ----
    commerces: CommerceType => CommerceInfo, "Gameinfo/CIV4CommerceInfo.xml", game::read_commerce;
    game_speeds: GameSpeedType => GameSpeedInfo, "Gameinfo/CIV4GameSpeedInfo.xml", game::read_game_speed;
    worlds: WorldSizeType => WorldInfo, "Gameinfo/CIV4WorldInfo.xml", game::read_world;
    climates: ClimateType => ClimateInfo, "Gameinfo/CIV4ClimateInfo.xml", game::read_climate;
    sea_levels: SeaLevelType => SeaLevelInfo, "Gameinfo/CIV4SeaLevelInfo.xml", game::read_sea_level;
    eras: EraType => EraInfo, "Gameinfo/CIV4EraInfos.xml", game::read_era;
    handicaps: HandicapType => HandicapInfo, "Gameinfo/CIV4HandicapInfo.xml", game::read_handicap;
    game_options: GameOptionType => GameOptionInfo, "Gameinfo/CIV4GameOptionInfos.xml", game::read_game_option;
    goodies: GoodyType => GoodyInfo, "Gameinfo/CIV4GoodyInfo.xml", game::read_goody;
    hurries: HurryType => HurryInfo, "Gameinfo/CIV4HurryInfo.xml", game::read_hurry;
    upkeeps: UpkeepType => UpkeepInfo, "Gameinfo/CIV4UpkeepInfo.xml", game::read_upkeep;
    culture_levels: CultureLevelType => CultureLevelInfo, "Gameinfo/CIV4CultureLevelInfo.xml", game::read_culture_level;
    victories: VictoryType => VictoryInfo, "Gameinfo/CIV4VictoryInfo.xml", game::read_victory;
    processes: ProcessType => ProcessInfo, "Gameinfo/CIV4ProcessInfo.xml", game::read_process;
    emphasizes: EmphasizeType => EmphasizeInfo, "Gameinfo/CIV4EmphasizeInfo.xml", game::read_emphasize;
    specialists: SpecialistType => SpecialistInfo, "Gameinfo/CIV4SpecialistInfos.xml", civics::read_specialist;
    religions: ReligionType => ReligionInfo, "Gameinfo/CIV4ReligionInfo.xml", civics::read_religion;
    corporations: CorporationType => CorporationInfo, "Gameinfo/CIV4CorporationInfo.xml", civics::read_corporation;
    civic_options: CivicOptionType => CivicOptionInfo, "Gameinfo/CIV4CivicOptionInfos.xml", civics::read_civic_option;
    civics: CivicType => CivicInfo, "Gameinfo/CIV4CivicInfos.xml", civics::read_civic;
    votes: VoteType => VoteInfo, "Gameinfo/CIV4VoteInfo.xml", civics::read_vote;
    vote_sources: VoteSourceType => VoteSourceInfo, "Gameinfo/CIV4VoteSourceInfos.xml", civics::read_vote_source;
    espionage_missions: EspionageMissionType => EspionageMissionInfo, "Gameinfo/CIV4EspionageMissionInfo.xml", civics::read_espionage_mission;
    projects: ProjectType => ProjectInfo, "Gameinfo/CIV4ProjectInfo.xml", civics::read_project;
    // ---- Technologies ----
    techs: TechType => TechInfo, "Technologies/CIV4TechInfos.xml", civics::read_tech;
    // ---- Buildings ----
    special_buildings: SpecialBuildingType => SpecialBuildingInfo, "Buildings/CIV4SpecialBuildingInfos.xml", building::read_special_building;
    building_classes: BuildingClassType => BuildingClassInfo, "Buildings/CIV4BuildingClassInfos.xml", building::read_building_class;
    buildings: BuildingType => BuildingInfo, "Buildings/CIV4BuildingInfos.xml", building::read_building;
    // ---- Civilizations ----
    traits: TraitType => TraitInfo, "Civilizations/CIV4TraitInfos.xml", civ::read_trait;
    civilizations: CivilizationType => CivilizationInfo, "Civilizations/CIV4CivilizationInfos.xml", civ::read_civilization;
    leader_heads: LeaderHeadType => LeaderHeadInfo, "Civilizations/CIV4LeaderHeadInfos.xml", civ::read_leader_head;
    // ---- Units ----
    special_units: SpecialUnitType => SpecialUnitInfo, "Units/CIV4SpecialUnitInfos.xml", unit::read_special_unit;
    unit_classes: UnitClassType => UnitClassInfo, "Units/CIV4UnitClassInfos.xml", unit::read_unit_class;
    promotions: PromotionType => PromotionInfo, "Units/CIV4PromotionInfos.xml", unit::read_promotion;
    builds: BuildType => BuildInfo, "Units/CIV4BuildInfos.xml", unit::read_build;
    units: UnitType => UnitInfo, "Units/CIV4UnitInfos.xml", unit::read_unit;
    spells: SpellType => SpellInfo, "Units/CIV4SpellInfos.xml", unit::read_spell;
    automates: AutomateType => AutomateInfo, "Units/CIV4AutomateInfos.xml", unit::read_automate;
    commands: CommandType => CommandInfo, "Units/CIV4CommandInfos.xml", unit::read_command;
    missions: MissionType => MissionInfo, "Units/CIV4MissionInfos.xml", unit::read_mission;
    // ---- Events ----
    events: EventType => EventInfo, "Events/CIV4EventInfos.xml", event::read_event;
    event_triggers: EventTriggerType => EventTriggerInfo, "Events/CIV4EventTriggerInfos.xml", event::read_event_trigger;
}

/// Lists declared in GlobalTypes.xml that are referenced by type name in XML.
const GLOBAL_TYPE_LISTS: &[(&str, &str)] = &[
    ("FlavorTypes", "FlavorTypes"),
    ("ArtStyleTypes", "ArtStyleTypes"),
    ("CitySizeTypes", "CitySizeTypes"),
    ("ContactTypes", "ContactTypes"),
    ("DiplomacyPowerTypes", "DiplomacyPowerTypes"),
    ("DirectionTypes", "DirectionTypes"),
    // CIV4AutomateInfos.xml entries have no <Type>; C++ names them from this list.
    ("AutomateTypes", "AutomateInfo"),
];

struct Docs {
    docs: Vec<(String, XmlDoc)>,
}

impl Docs {
    fn get(&self, file: &str) -> &XmlDoc {
        &self.docs.iter().find(|(f, _)| f == file).expect("document not loaded").1
    }
}

/// Entry elements of an info file: root -> container -> entries.
/// A container nested inside the container (FFH2's CIV4CommandInfos.xml has one)
/// is flattened in place, matching the C++ enum order.
fn entries(doc: &XmlDoc) -> Vec<xml::Elem<'_>> {
    fn collect<'a>(container: xml::Elem<'a>, out: &mut Vec<xml::Elem<'a>>) {
        for c in container.children() {
            if c.name() == container.name() {
                collect(c, out);
            } else {
                out.push(c);
            }
        }
    }
    let mut out = Vec::new();
    if let Some(container) = doc.root().first_child() {
        collect(container, &mut out);
    }
    out
}

#[derive(Debug)]
pub struct LoadError(pub String);

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

fn read_doc(dir: &Path, rel: &str) -> Result<XmlDoc, LoadError> {
    let p = dir.join(rel);
    let bytes = std::fs::read(&p).map_err(|e| LoadError(format!("{}: {}", p.display(), e)))?;
    XmlDoc::parse_bytes(&bytes).map_err(|e| LoadError(format!("{}: {}", p.display(), e)))
}

impl GameData {
    /// Loads everything from a data directory containing `xml/` and `text/`.
    pub fn load(data_dir: &Path) -> Result<GameData, LoadError> {
        let xml_dir = data_dir.join("xml");
        let mut docs = Vec::new();
        for (file, _) in INFO_FILES {
            docs.push((file.to_string(), read_doc(&xml_dir, file)?));
        }
        let docs = Docs { docs };

        // Pass 1: register every type name.
        let mut reg = Registry::new();
        for (file, class) in INFO_FILES {
            let doc = docs.get(file);
            let mut names = Vec::new();
            for e in entries(doc) {
                if e.name() != *class {
                    return Err(LoadError(format!("{file}: entry <{}> but expected <{class}>", e.name())));
                }
                names.push(e.child_text("Type").unwrap_or("").to_string());
            }
            reg.register_class(class, names);
        }
        let gt = read_doc(&xml_dir, "GlobalTypes.xml")?;
        for (list, class) in GLOBAL_TYPE_LISTS {
            if let Some(l) = gt.root().child(list) {
                let names = l.children().map(|c| c.text().to_string()).collect();
                reg.register_class(class, names);
            }
        }

        // Defines (text defines resolve through the registry).
        let mut define_map = DefineMap::default();
        for f in ["GlobalDefines.xml", "GlobalDefinesAlt.xml", "PythonCallbackDefines.xml"] {
            define_map.read_file(&read_doc(&xml_dir, f)?, &reg);
        }

        let text = match std::fs::read(data_dir.join("text").join("en.tsv")) {
            Ok(b) => Text::parse_tsv(&String::from_utf8_lossy(&b)),
            Err(_) => Text::default(),
        };

        // Pass 2: typed tables.
        let mut gd = GameData::load_tables(&docs, reg, define_map, text);
        for (name, a, b) in &gd.reg.duplicates {
            gd.warnings.push(format!("type {name} registered as both {a} and {b}"));
        }
        gd.check_enum_orders();
        Ok(gd)
    }

    /// Verifies that data-defined tables whose order is hard-coded in C++ enums match.
    fn check_enum_orders(&mut self) {
        fn check(w: &mut Vec<String>, reg: &Registry, class: &str, names: &[&str]) {
            let got = reg.names_of(class);
            for (i, n) in names.iter().enumerate() {
                if got.get(i).map(|s| s.as_str()) != Some(*n) {
                    w.push(format!("{class}[{i}] is {:?} but C++ enum expects {n}", got.get(i)));
                }
            }
        }
        let w = &mut self.warnings;
        check(w, &self.reg, "DomainInfo", DomainType::NAMES);
        check(w, &self.reg, "UnitAIInfo", UnitAIType::NAMES);
        check(w, &self.reg, "YieldInfo", YieldType::NAMES);
        check(w, &self.reg, "CommerceInfo", CommerceType::NAMES);
        check(w, &self.reg, "MissionInfo", MissionType::NAMES);
        check(w, &self.reg, "CommandInfo", CommandType::NAMES);
        check(w, &self.reg, "AutomateInfo", AutomateType::NAMES);
        check(w, &self.reg, "GameOptionInfo", GameOptionType::NAMES);
        check(w, &self.reg, "MemoryInfo", MemoryType::NAMES);
        check(w, &self.reg, "AttitudeInfo", AttitudeType::NAMES);
        check(w, &self.reg, "DenialInfo", DenialType::NAMES);
        check(w, &self.reg, "DirectionTypes", DirectionType::NAMES);
        check(w, &self.reg, "AlignmentInfo", AlignmentType::NAMES);
        check(w, &self.reg, "DamageInfo", DamageType::NAMES);
        check(w, &self.reg, "CalendarInfo", CalendarType::NAMES);
        check(w, &self.reg, "WorldInfo", WorldSizeType::NAMES);
        check(w, &self.reg, "ContactTypes", ContactType::NAMES);
        check(w, &self.reg, "DiplomacyPowerTypes", DiplomacyPowerType::NAMES);
        check(w, &self.reg, "CitySizeTypes", CitySizeType::NAMES);
    }

    /// Typed lookup of a type name (C++ `GC.getInfoTypeForString` + cast).
    pub fn ty<T: InfoId>(&self, name: &str) -> Option<T> {
        self.reg.lookup::<T>(name)
    }

    /// Like [`Self::ty`] but panics if the name is unknown: for names hard-coded in ported rules.
    pub fn t<T: InfoId>(&self, name: &str) -> T {
        self.reg.lookup::<T>(name).unwrap_or_else(|| panic!("unknown {} type {name}", T::CLASS))
    }

    /// C++ `GC.getCivilizationInfo(civ).getCivilizationUnits(class)`: the civ's unit for a
    /// unit class (`None` = this civ cannot have it).
    pub fn civ_unit(&self, civ: CivilizationType, class: UnitClassType) -> Option<UnitType> {
        self.civilizations[civ].civilization_units(class, self.unit_classes[class].default_unit_index)
    }

    /// C++ `GC.getCivilizationInfo(civ).getCivilizationBuildings(class)`.
    pub fn civ_building(&self, civ: CivilizationType, class: BuildingClassType) -> Option<BuildingType> {
        self.civilizations[civ].civilization_buildings(class, &self.building_classes)
    }

    /// Type string of an index (C++ `getType()`).
    pub fn type_name<T: InfoId>(&self, t: T) -> &str {
        self.reg.type_name(t)
    }
}

/// Default data directory: `<repo>/projects/ffh2-rs/data`.
pub fn default_data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FFH_DATA_DIR") {
        return PathBuf::from(d);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("data")
}

static GLOBAL: OnceLock<GameData> = OnceLock::new();

/// Loads the default data once per process and returns a `'static` reference.
pub fn global() -> &'static GameData {
    GLOBAL.get_or_init(|| GameData::load(&default_data_dir()).expect("failed to load game data"))
}
