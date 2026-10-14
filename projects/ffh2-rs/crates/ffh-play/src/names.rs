//! Type names <-> indices for every info table and every hard-coded C++ enum, and text lookup.
//!
//! The protocol accepts a type reference as its XML/C++ type name (`"UNIT_WARRIOR"`) or as its
//! integer index (`44`), and answers with indices (design §4.1). This module decodes both forms
//! for every type ([`decode`], [`field`]), names every index ([`name`]) and finds display text
//! ([`display`]). [`KINDS`] lists all of them with type-erased accessors, for the data tables,
//! `inspect {kind}` and the `ffh-play names` / `lookup` commands.
//!
//! Three sources of names:
//! * info tables (`GameData` fields such as `units`): names from the type registry, count from
//!   the table, display text from the info's `<Description>`;
//! * GlobalTypes.xml lists without a table (`FlavorType`, `DirectionType`, ...): registry only;
//! * hard-coded C++ enums with no XML (`ActivityType`, `OrderType`, `TaskType`, ...): the
//!   generated `NAMES` arrays of `ffh_data::enums`.
//!
//! Plus `PlayerVoteTypes` (negative sentinels or a player id), which is not an index type.

use std::borrow::Cow;

use ffh_core::structs::PlayerVote;
use ffh_data::*;

use crate::json::{FieldError, Json};

/// A type with names: an info table index, a GlobalTypes list index or a hard-coded C++ enum.
pub trait Named: Idx {
    /// Rust type name (`"UnitType"`), used in messages and as the kind name.
    const KIND: &'static str;
    /// Number of valid values (indices `0..count`).
    fn count(d: &GameData) -> usize;
    /// Type name of a value (`"?"` if out of range).
    fn name(d: &GameData, v: Self) -> &str;
    /// Value of a type name.
    fn lookup(d: &GameData, s: &str) -> Option<Self>;
    /// Text key of the value's description (info tables only).
    fn text_key(_d: &GameData, _v: Self) -> Option<&str> {
        None
    }
}

/// Where a kind's names come from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// An info table: `GameData` field name and registry class (`"units"`, `"UnitInfo"`).
    Table { field: &'static str, class: &'static str },
    /// A GlobalTypes.xml list without a table: registry class (`"FlavorTypes"`).
    Global { class: &'static str },
    /// A hard-coded C++ enum (`ffh_data::enums` `NAMES`).
    Enum,
}

/// One kind of named type, with type-erased accessors (see [`Named`]).
pub struct Kind {
    pub kind: &'static str,
    pub source: Source,
    pub count: fn(&GameData) -> usize,
    pub name: fn(&GameData, usize) -> &str,
    pub lookup: fn(&GameData, &str) -> Option<usize>,
    pub text_key: fn(&GameData, usize) -> Option<&str>,
    /// [`decode`] for this kind, returning the index.
    pub decode: fn(&GameData, &Json) -> Result<usize, String>,
}

impl Kind {
    /// The `GameData` table field, for info tables.
    pub fn table(&self) -> Option<&'static str> {
        match self.source {
            Source::Table { field, .. } => Some(field),
            _ => None,
        }
    }

    /// The type registry class, for info tables and GlobalTypes lists.
    pub fn class(&self) -> Option<&'static str> {
        match self.source {
            Source::Table { class, .. } | Source::Global { class } => Some(class),
            Source::Enum => None,
        }
    }

    /// All names, in index order.
    pub fn names<'d>(&self, d: &'d GameData) -> Vec<&'d str> {
        (0..(self.count)(d)).map(|i| (self.name)(d, i)).collect()
    }

    /// [`display`] for this kind.
    pub fn display<'d>(&self, d: &'d GameData, i: usize) -> Cow<'d, str> {
        display_of((self.text_key)(d, i), (self.name)(d, i), d)
    }
}

impl std::fmt::Debug for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Kind({})", self.kind)
    }
}

fn dyn_name<T: Named>(d: &GameData, i: usize) -> &str {
    if i < T::count(d) {
        T::name(d, T::from_usize(i))
    } else {
        "?"
    }
}

fn dyn_lookup<T: Named>(d: &GameData, s: &str) -> Option<usize> {
    T::lookup(d, s).map(Idx::idx)
}

fn dyn_text_key<T: Named>(d: &GameData, i: usize) -> Option<&str> {
    if i < T::count(d) {
        T::text_key(d, T::from_usize(i))
    } else {
        None
    }
}

fn dyn_decode<T: Named>(d: &GameData, j: &Json) -> Result<usize, String> {
    decode::<T>(d, j).map(Idx::idx)
}

macro_rules! kinds {
    (
        tables { $($tt:ident => $field:ident),* $(,)? }
        globals { $($gt:ident),* $(,)? }
        enums { $($et:ident),* $(,)? }
    ) => {
        $(
            impl Named for $tt {
                const KIND: &'static str = stringify!($tt);
                fn count(d: &GameData) -> usize {
                    d.$field.len()
                }
                fn name(d: &GameData, v: Self) -> &str {
                    d.reg.type_name(v)
                }
                fn lookup(d: &GameData, s: &str) -> Option<Self> {
                    d.ty::<$tt>(s).filter(|v| v.idx() < d.$field.len())
                }
                fn text_key(d: &GameData, v: Self) -> Option<&str> {
                    d.$field.get(v).map(|i| i.base.description.as_str()).filter(|k| !k.is_empty())
                }
            }
        )*
        $(
            impl Named for $gt {
                const KIND: &'static str = stringify!($gt);
                fn count(d: &GameData) -> usize {
                    d.reg.count::<$gt>()
                }
                fn name(d: &GameData, v: Self) -> &str {
                    d.reg.type_name(v)
                }
                fn lookup(d: &GameData, s: &str) -> Option<Self> {
                    d.ty::<$gt>(s)
                }
            }
        )*
        $(
            impl Named for $et {
                const KIND: &'static str = stringify!($et);
                fn count(_d: &GameData) -> usize {
                    $et::NAMES.len()
                }
                fn name(_d: &GameData, v: Self) -> &str {
                    $et::NAMES.get(v.idx()).copied().unwrap_or("?")
                }
                fn lookup(_d: &GameData, s: &str) -> Option<Self> {
                    $et::NAMES.iter().position(|n| *n == s).map($et::from_usize)
                }
            }
        )*

        /// Every named kind: info tables (in `GameData` load order), GlobalTypes lists, then
        /// hard-coded enums.
        pub static KINDS: &[Kind] = &[
            $(Kind {
                kind: stringify!($tt),
                source: Source::Table { field: stringify!($field), class: <$tt as InfoId>::CLASS },
                count: <$tt as Named>::count,
                name: dyn_name::<$tt>,
                lookup: dyn_lookup::<$tt>,
                text_key: dyn_text_key::<$tt>,
                decode: dyn_decode::<$tt>,
            },)*
            $(Kind {
                kind: stringify!($gt),
                source: Source::Global { class: <$gt as InfoId>::CLASS },
                count: <$gt as Named>::count,
                name: dyn_name::<$gt>,
                lookup: dyn_lookup::<$gt>,
                text_key: dyn_text_key::<$gt>,
                decode: dyn_decode::<$gt>,
            },)*
            $(Kind {
                kind: stringify!($et),
                source: Source::Enum,
                count: <$et as Named>::count,
                name: dyn_name::<$et>,
                lookup: dyn_lookup::<$et>,
                text_key: dyn_text_key::<$et>,
                decode: dyn_decode::<$et>,
            },)*
        ];
    };
}

kinds! {
    tables {
        // ---- BasicInfos ----
        AlignmentType => alignments,
        AttitudeType => attitudes,
        CalendarType => calendars,
        DamageType => damages,
        DenialType => denials,
        DomainType => domains,
        InvisibleType => invisibles,
        MemoryType => memories,
        MonthType => months,
        SeasonType => seasons,
        UnitAIType => unit_ais,
        UnitCombatType => unit_combats,
        // ---- Terrain ----
        YieldType => yields,
        TerrainType => terrains,
        FeatureType => features,
        BonusClassType => bonus_classes,
        BonusType => bonuses,
        ImprovementType => improvements,
        RouteType => routes,
        // ---- GameInfo ----
        CommerceType => commerces,
        GameSpeedType => game_speeds,
        WorldSizeType => worlds,
        ClimateType => climates,
        SeaLevelType => sea_levels,
        EraType => eras,
        HandicapType => handicaps,
        GameOptionType => game_options,
        GoodyType => goodies,
        HurryType => hurries,
        UpkeepType => upkeeps,
        CultureLevelType => culture_levels,
        VictoryType => victories,
        ProcessType => processes,
        EmphasizeType => emphasizes,
        SpecialistType => specialists,
        ReligionType => religions,
        CorporationType => corporations,
        CivicOptionType => civic_options,
        CivicType => civics,
        VoteType => votes,
        VoteSourceType => vote_sources,
        EspionageMissionType => espionage_missions,
        ProjectType => projects,
        // ---- Technologies ----
        TechType => techs,
        // ---- Buildings ----
        SpecialBuildingType => special_buildings,
        BuildingClassType => building_classes,
        BuildingType => buildings,
        // ---- Civilizations ----
        TraitType => traits,
        CivilizationType => civilizations,
        LeaderHeadType => leader_heads,
        // ---- Units ----
        SpecialUnitType => special_units,
        UnitClassType => unit_classes,
        PromotionType => promotions,
        BuildType => builds,
        UnitType => units,
        SpellType => spells,
        AutomateType => automates,
        CommandType => commands,
        MissionType => missions,
        // ---- Events ----
        EventType => events,
        EventTriggerType => event_triggers,
    }
    globals {
        FlavorType,
        ArtStyleType,
        CitySizeType,
        ContactType,
        DiplomacyPowerType,
        DirectionType,
    }
    enums {
        ActivityType,
        AreaAIType,
        CardinalDirectionType,
        ControlType,
        FeatType,
        GameStateType,
        InterfaceMessageType,
        MissionAIType,
        OrderType,
        PlayerOptionType,
        PlotType,
        ProbabilityType,
        TaskType,
        TradeableItem,
        WarPlanType,
    }
}

/// The kind with this kind name (`"UnitType"`) or table field name (`"units"`).
pub fn kind(name: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.kind == name || k.table() == Some(name))
}

/// Every `(kind, index)` that `s` names. Type names are unique across kinds in FfH2 data (a
/// test checks this), so this has at most one element unless the data changes.
pub fn find(d: &GameData, s: &str) -> Vec<(&'static Kind, usize)> {
    KINDS.iter().filter_map(|k| (k.lookup)(d, s).map(|i| (k, i))).collect()
}

/// Type name of a value.
pub fn name<T: Named>(d: &GameData, v: T) -> &str {
    T::name(d, v)
}

/// Value of a type name.
pub fn lookup<T: Named>(d: &GameData, s: &str) -> Option<T> {
    T::lookup(d, s)
}

/// Text of a `TXT_KEY_*` with Civ4 markup kept (the key itself if missing).
pub fn text<'d>(d: &'d GameData, key: &'d str) -> &'d str {
    d.text.get(key)
}

/// Display name of a value: its description text (cut at FfH2's ':' gender separator), or,
/// when it has no description or the text is missing, its [`humanize`]d type name. (The vendored
/// text is FfH2's own file only, so descriptions FfH2 inherits from BtS, such as
/// `TXT_KEY_UNIT_WARRIOR`, have no text.)
pub fn display<T: Named>(d: &GameData, v: T) -> Cow<'_, str> {
    display_of(T::text_key(d, v), T::name(d, v), d)
}

fn display_of<'d>(key: Option<&'d str>, name: &'d str, d: &'d GameData) -> Cow<'d, str> {
    if let Some(k) = key {
        let t = d.text.name(k);
        if t != k && !t.is_empty() {
            return Cow::Borrowed(t);
        }
    }
    Cow::Owned(humanize(name))
}

/// A readable name from a type name: the class prefix is dropped and words are capitalized
/// (`UNIT_WARRIOR` -> `Warrior`, `GAMEOPTION_NO_GOODY_HUTS` -> `No Goody Huts`).
pub fn humanize(type_name: &str) -> String {
    let rest = match type_name.split_once('_') {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => type_name,
    };
    let mut out = String::with_capacity(rest.len());
    for (i, w) in rest.split('_').filter(|w| !w.is_empty()).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut cs = w.chars();
        if let Some(c) = cs.next() {
            out.extend(c.to_uppercase());
            out.extend(cs.flat_map(char::to_lowercase));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------------------------

/// Decodes a type reference: a type name or an index in `0..count`.
pub fn decode<T: Named>(d: &GameData, j: &Json) -> Result<T, String> {
    if let Json::Str(s) = j {
        return T::lookup(d, s).ok_or_else(|| format!("unknown {} {:?}", T::KIND, s));
    }
    let n = T::count(d);
    match j.as_i64() {
        Some(i) if i >= 0 && (i as u64) < n as u64 => Ok(T::from_usize(i as usize)),
        Some(i) => Err(format!("{} index {i} out of range 0..{n}", T::KIND)),
        None => Err(format!("expected a {} name or index, got {}", T::KIND, j.kind())),
    }
}

/// Like [`decode`], but `null` and `-1` (C++ `NO_X`) mean none.
pub fn decode_opt<T: Named>(d: &GameData, j: &Json) -> Result<Option<T>, String> {
    if j.is_null() || j.as_i64() == Some(-1) {
        Ok(None)
    } else {
        decode(d, j).map(Some)
    }
}

/// Required type-reference member `key` of an object.
pub fn field<T: Named>(d: &GameData, obj: &Json, key: &str) -> Result<T, FieldError> {
    decode(d, obj.req_value(key)?).map_err(|e| FieldError::new(key, e))
}

/// Optional type-reference member: `None` when absent, `null` or `-1`.
pub fn opt_field<T: Named>(d: &GameData, obj: &Json, key: &str) -> Result<Option<T>, FieldError> {
    match obj.opt_value(key)? {
        None => Ok(None),
        Some(v) => decode_opt(d, v).map_err(|e| FieldError::new(key, e)),
    }
}

/// Required array of type references (`"civics": ["CIVIC_GOD_KING", 12]`).
pub fn list_field<T: Named>(d: &GameData, obj: &Json, key: &str) -> Result<Vec<T>, FieldError> {
    obj.req_arr(key)?
        .iter()
        .enumerate()
        .map(|(i, v)| decode(d, v).map_err(|e| FieldError::new(key, format!("element {i}: {e}"))))
        .collect()
}

/// `{kind: [names in index order]}` for every kind (the name part of `/api/data`).
pub fn names_json(d: &GameData) -> Json {
    Json::Obj(KINDS.iter().map(|k| (k.kind.to_string(), kind_names_json(d, k))).collect())
}

/// `[names in index order]` of one kind.
pub fn kind_names_json(d: &GameData, k: &Kind) -> Json {
    Json::Arr(k.names(d).into_iter().map(Json::from).collect())
}

// ---------------------------------------------------------------------------------------------
// PlayerVoteTypes
// ---------------------------------------------------------------------------------------------

/// C++ `PlayerVoteTypes` sentinels (CvEnums.h). Values >= 0 are votes for a player.
pub const PLAYER_VOTE_NAMES: &[(&str, PlayerVote)] = &[
    ("NO_PLAYER_VOTE_CHECKED", PlayerVote::NO_PLAYER_VOTE_CHECKED),
    ("PLAYER_VOTE_NEVER", PlayerVote::PLAYER_VOTE_NEVER),
    ("PLAYER_VOTE_ABSTAIN", PlayerVote::PLAYER_VOTE_ABSTAIN),
    ("PLAYER_VOTE_NO", PlayerVote::PLAYER_VOTE_NO),
    ("PLAYER_VOTE_YES", PlayerVote::PLAYER_VOTE_YES),
    ("NO_PLAYER_VOTE", PlayerVote::NO_PLAYER_VOTE),
];

/// C++ name of a sentinel vote (`None` for a vote for a player).
pub fn player_vote_name(v: PlayerVote) -> Option<&'static str> {
    PLAYER_VOTE_NAMES.iter().find(|(_, x)| *x == v).map(|(n, _)| *n)
}

/// Decodes a vote choice: a C++ name (`"PLAYER_VOTE_YES"`), a short name (`"YES"`, `"NO"`,
/// `"NEVER"`, `"ABSTAIN"`), or an integer (a sentinel or a player id; whether that player can be
/// voted for is the command's check).
pub fn decode_player_vote(j: &Json) -> Result<PlayerVote, String> {
    if let Json::Str(s) = j {
        let full = format!("PLAYER_VOTE_{s}");
        return PLAYER_VOTE_NAMES
            .iter()
            .find(|(n, _)| *n == s || *n == full)
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("unknown PlayerVote {s:?}"));
    }
    match j.as_i64() {
        Some(i) if (-6..=u8::MAX as i64).contains(&i) => Ok(PlayerVote(i as i32)),
        Some(i) => Err(format!("PlayerVote {i} out of range -6..=255")),
        None => Err(format!("expected a PlayerVote name or integer, got {}", j.kind())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_votes() {
        for (n, v) in PLAYER_VOTE_NAMES {
            assert_eq!(decode_player_vote(&Json::from(*n)), Ok(*v));
            assert_eq!(decode_player_vote(&Json::Int(v.0 as i64)), Ok(*v));
            assert_eq!(player_vote_name(*v), Some(*n));
        }
        assert_eq!(decode_player_vote(&Json::from("YES")), Ok(PlayerVote::PLAYER_VOTE_YES));
        assert_eq!(decode_player_vote(&Json::from("NO")), Ok(PlayerVote::PLAYER_VOTE_NO));
        assert_eq!(decode_player_vote(&Json::from("NEVER")), Ok(PlayerVote::PLAYER_VOTE_NEVER));
        assert_eq!(decode_player_vote(&Json::from("ABSTAIN")), Ok(PlayerVote::PLAYER_VOTE_ABSTAIN));
        assert_eq!(decode_player_vote(&Json::Int(3)), Ok(PlayerVote(3)));
        assert_eq!(player_vote_name(PlayerVote(3)), None);
        assert!(decode_player_vote(&Json::from("MAYBE")).is_err());
        assert!(decode_player_vote(&Json::from("")).is_err());
        assert!(decode_player_vote(&Json::Int(-7)).is_err());
        assert!(decode_player_vote(&Json::Int(256)).is_err());
        assert!(decode_player_vote(&Json::Bool(true)).is_err());
    }

    #[test]
    fn humanized_names() {
        assert_eq!(humanize("UNIT_WARRIOR"), "Warrior");
        assert_eq!(humanize("GAMEOPTION_NO_GOODY_HUTS"), "No Goody Huts");
        assert_eq!(humanize("BONUS_MANA_FIRE"), "Mana Fire");
        assert_eq!(humanize("PLAIN"), "Plain");
        assert_eq!(humanize("X_"), "X");
        assert_eq!(humanize(""), "");
    }

    #[test]
    fn kind_names_are_unique() {
        let mut seen: Vec<&str> = KINDS.iter().map(|k| k.kind).collect();
        seen.extend(KINDS.iter().filter_map(Kind::table));
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n, "kind and table names must not collide");
    }
}
