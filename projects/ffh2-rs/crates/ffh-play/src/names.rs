//! Type names <-> indices for every info table and every hard-coded C++ enum, and text lookup.
//!
//! The protocol accepts a type reference as its XML/C++ type name (`"UNIT_WARRIOR"`) or as its
//! integer index, and answers with indices (design §4.1). This module decodes both forms
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
//! Plus `PlayerVoteTypes` (negative sentinels, or a team id in team votes), which is not an index
//! type: [`decode_player_vote`] accepts only the choices the C++ DIPLOVOTE popup can send.

use std::borrow::Cow;

use ffh_core::ids::TeamId;
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
    /// The info entry's common fields (info tables only; `None` out of range).
    pub base: fn(&GameData, usize) -> Option<&InfoBase>,
    /// `{:#?}` of the info entry (info tables only; `inspect`).
    pub debug: fn(&GameData, usize) -> Option<String>,
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

/// The info table of an info-table kind.
trait Table: Named {
    fn info_base(d: &GameData, v: Self) -> &InfoBase;
    fn info_debug(d: &GameData, v: Self) -> String;
}

fn dyn_base<T: Table>(d: &GameData, i: usize) -> Option<&InfoBase> {
    (i < T::count(d)).then(|| T::info_base(d, T::from_usize(i)))
}

fn dyn_debug<T: Table>(d: &GameData, i: usize) -> Option<String> {
    (i < T::count(d)).then(|| T::info_debug(d, T::from_usize(i)))
}

fn no_base(_d: &GameData, _i: usize) -> Option<&InfoBase> {
    None
}

fn no_debug(_d: &GameData, _i: usize) -> Option<String> {
    None
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
            impl Table for $tt {
                fn info_base(d: &GameData, v: Self) -> &InfoBase {
                    &d.$field[v].base
                }
                fn info_debug(d: &GameData, v: Self) -> String {
                    format!("{:#?}", d.$field[v])
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
                base: dyn_base::<$tt>,
                debug: dyn_debug::<$tt>,
            },)*
            $(Kind {
                kind: stringify!($gt),
                source: Source::Global { class: <$gt as InfoId>::CLASS },
                count: <$gt as Named>::count,
                name: dyn_name::<$gt>,
                lookup: dyn_lookup::<$gt>,
                text_key: dyn_text_key::<$gt>,
                decode: dyn_decode::<$gt>,
                base: no_base,
                debug: no_debug,
            },)*
            $(Kind {
                kind: stringify!($et),
                source: Source::Enum,
                count: <$et as Named>::count,
                name: dyn_name::<$et>,
                lookup: dyn_lookup::<$et>,
                text_key: dyn_text_key::<$et>,
                decode: dyn_decode::<$et>,
                base: no_base,
                debug: no_debug,
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

/// The display names of the types whose description key has no usable text, by key (built
/// once): `TXT_KEY_UNIT_WARRIOR` -> "Warrior".
fn key_names(d: &GameData) -> &'static std::collections::HashMap<String, String> {
    static NAMES: std::sync::OnceLock<std::collections::HashMap<String, String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for k in KINDS {
            for i in 0..(k.count)(d) {
                if let Some(key) = (k.text_key)(d, i) {
                    m.entry(key.to_string()).or_insert_with(|| k.display(d, i).into_owned());
                }
            }
        }
        m
    })
}

/// `text` with every `TXT_KEY_*` token replaced by its text (ffh-core logs some names as text
/// keys: city names, civilization descriptions, unit names). A key without usable text becomes
/// the display name of the type it describes (`TXT_KEY_UNIT_WARRIOR` -> "Warrior"), else its
/// words (`TXT_KEY_NOT_A_KEY` -> "Not A Key"). Display only.
pub fn resolve_keys<'t>(d: &GameData, text: &'t str) -> Cow<'t, str> {
    const PREFIX: &str = "TXT_KEY_";
    if !text.contains(PREFIX) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(PREFIX) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let len = tail.find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')).unwrap_or(tail.len());
        let key = tail[..len].trim_end_matches('_');
        if key.len() <= PREFIX.len() {
            out.push_str(&tail[..len]);
            rest = &tail[len..];
            continue;
        }
        let t = d.text.get(key).trim();
        if t != key && !t.is_empty() && !has_placeholder(t) {
            out.push_str(t);
        } else if let Some(n) = key_names(d).get(key) {
            out.push_str(n);
        } else {
            let words: Vec<String> = key[PREFIX.len()..]
                .split('_')
                .filter(|w| !w.is_empty())
                .map(|w| {
                    let w = w.to_ascii_lowercase();
                    let mut c = w.chars();
                    c.next().map_or(String::new(), |f| f.to_ascii_uppercase().to_string() + c.as_str())
                })
                .collect();
            out.push_str(&words.join(" "));
        }
        rest = &tail[key.len()..];
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Display name of a value: its description text (`Text::name`, C++ `getDescription()`: the
/// whole text, colons included, as FfH2's English entries have no grammatical forms), with Civ4
/// markup such as `[ICON_TRADE]` kept (`VOTE_LIBERTY` -> "Global Civic: Liberty"), or its
/// [`humanize`]d type name when it has no description, the text is missing or empty, or the text
/// holds an unfilled `%D1`-style placeholder (the Somnium memories: `%D1: "Learn how to play
/// Somnium!"`).
///
/// The vendored text is FfH2's own file only, so descriptions FfH2 inherits from BtS, such as
/// `TXT_KEY_UNIT_WARRIOR`, have no text and use the type name.
pub fn display<T: Named>(d: &GameData, v: T) -> Cow<'_, str> {
    display_of(T::text_key(d, v), T::name(d, v), d)
}

fn display_of<'d>(key: Option<&'d str>, name: &'d str, d: &'d GameData) -> Cow<'d, str> {
    if let Some(k) = key {
        let t = d.text.name(k).trim();
        if t != k && !t.is_empty() && !has_placeholder(t) {
            return Cow::Borrowed(t);
        }
    }
    Cow::Owned(humanize(name))
}

/// Whether `t` holds a Civ4 text placeholder (`%s1`, `%d2`, `%D1`, `%F1`, ...): a '%', a letter
/// and a digit. A plain percent sign ("by 75%, ...") is not one.
fn has_placeholder(t: &str) -> bool {
    t.as_bytes().windows(3).any(|w| w[0] == b'%' && w[1].is_ascii_alphabetic() && w[2].is_ascii_digit())
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

/// C++ `PlayerVoteTypes` sentinels (CvEnums.h), to name vote *state* (`CvPlayer::getVote`).
/// A value >= 0 is a vote for a **team**, cast only in team votes (secretary general and
/// victory votes: `CvGame::castVote` compares it with `getTeam()`; ffh-core `team_vote`). The
/// last two are internal: `NO_PLAYER_VOTE` = not voted yet, `NO_PLAYER_VOTE_CHECKED` = not
/// voted after the extra turn `doVoteResults` gives.
pub const PLAYER_VOTE_NAMES: &[(&str, PlayerVote)] = &[
    ("PLAYER_VOTE_YES", PlayerVote::PLAYER_VOTE_YES),
    ("PLAYER_VOTE_NO", PlayerVote::PLAYER_VOTE_NO),
    ("PLAYER_VOTE_NEVER", PlayerVote::PLAYER_VOTE_NEVER),
    ("PLAYER_VOTE_ABSTAIN", PlayerVote::PLAYER_VOTE_ABSTAIN),
    ("NO_PLAYER_VOTE", PlayerVote::NO_PLAYER_VOTE),
    ("NO_PLAYER_VOTE_CHECKED", PlayerVote::NO_PLAYER_VOTE_CHECKED),
];

/// The sentinel choices a vote answer may name (`-5..=-2`): the YES / NO / NEVER / ABSTAIN
/// buttons of the C++ DIPLOVOTE popup, the first four of [`PLAYER_VOTE_NAMES`].
pub const PLAYER_VOTE_CHOICES: &[(&str, PlayerVote)] = &[
    ("PLAYER_VOTE_YES", PlayerVote::PLAYER_VOTE_YES),
    ("PLAYER_VOTE_NO", PlayerVote::PLAYER_VOTE_NO),
    ("PLAYER_VOTE_NEVER", PlayerVote::PLAYER_VOTE_NEVER),
    ("PLAYER_VOTE_ABSTAIN", PlayerVote::PLAYER_VOTE_ABSTAIN),
];

/// Exclusive upper bound of a team id in a vote choice: C++ `MAX_CIV_TEAMS` (=
/// `MAX_CIV_PLAYERS`, 35 in FfH2 0.41). The barbarian team is never a candidate.
pub const MAX_CIV_TEAMS: usize = ffh_core::defs::MAX_CIV_PLAYERS;

/// C++ name of a sentinel vote (`None` for a team vote).
pub fn player_vote_name(v: PlayerVote) -> Option<&'static str> {
    PLAYER_VOTE_NAMES.iter().find(|(_, x)| *x == v).map(|(n, _)| *n)
}

/// Decodes a vote choice as the C++ `BUTTONPOPUP_DIPLOVOTE` popup can send it
/// (`CvDLLButtonPopup::launchDiploVotePopup`): `YES`, `NO`, `NEVER` or `ABSTAIN` (C++ name
/// `"PLAYER_VOTE_YES"`, short name `"YES"`, or the value: -2, -3, -5, -4), or, in a team vote, a team id
/// in `0..MAX_CIV_TEAMS`. The internal sentinels `NO_PLAYER_VOTE` (-1) and
/// `NO_PLAYER_VOTE_CHECKED` (-6) are refused: casting them would reset the vote or skip the extra
/// turn a missing vote gets.
///
/// This checks the form only. The `vote` command checks what the popup offers for that vote:
/// a team vote takes only team ids (alive, `is_team_vote_eligible`, the vassal restriction) and
/// ABSTAIN (not offered to a vassal of an eligible team); other votes take YES, NO, NEVER (only
/// if `player_can_defy_resolution`) and ABSTAIN.
pub fn decode_player_vote(j: &Json) -> Result<PlayerVote, String> {
    const HINT: &str = "a vote choice is YES, NO, NEVER, ABSTAIN or (in a team vote) a team id";
    if let Json::Str(s) = j {
        let short = s.strip_prefix("PLAYER_VOTE_").unwrap_or(s);
        let full = format!("PLAYER_VOTE_{short}");
        if let Some((_, v)) = PLAYER_VOTE_CHOICES.iter().find(|(n, _)| *n == full) {
            return Ok(*v);
        }
        return Err(match PLAYER_VOTE_NAMES.iter().find(|(n, _)| n == s) {
            Some((n, _)) => format!("{n} is vote state, not a choice ({HINT})"),
            None => format!("unknown vote choice {s:?} ({HINT})"),
        });
    }
    match j.as_i64() {
        Some(i @ -5..=-2) => Ok(PlayerVote(i as i32)),
        Some(i) if i >= 0 && (i as u64) < MAX_CIV_TEAMS as u64 => Ok(PlayerVote::for_team(TeamId(i as u8))),
        Some(i @ (-1 | -6)) => {
            Err(format!("{i} ({}) is vote state, not a choice ({HINT})", player_vote_name(PlayerVote(i as i32)).unwrap_or("?")))
        }
        Some(i) => Err(format!("vote choice {i} out of range: -5..=-2 or a team id in 0..{MAX_CIV_TEAMS}")),
        None => Err(format!("expected a vote choice name or integer, got {}", j.kind())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_votes() {
        // The names are C++ CvEnums.h PlayerVoteTypes.
        for (n, v) in PLAYER_VOTE_NAMES {
            assert_eq!(player_vote_name(*v), Some(*n));
            assert_eq!(v.team(), None);
        }
        assert_eq!(PLAYER_VOTE_NAMES[..4], *PLAYER_VOTE_CHOICES);
        // Every choice the DIPLOVOTE popup has: by C++ name, short name, value, integral float.
        for (n, v) in PLAYER_VOTE_CHOICES {
            assert_eq!(decode_player_vote(&Json::from(*n)), Ok(*v));
            assert_eq!(decode_player_vote(&Json::from(&n["PLAYER_VOTE_".len()..])), Ok(*v));
            assert_eq!(decode_player_vote(&Json::Int(v.0 as i64)), Ok(*v));
            assert_eq!(decode_player_vote(&Json::Float(v.0 as f64)), Ok(*v));
        }
        assert_eq!(decode_player_vote(&Json::from("YES")), Ok(PlayerVote::PLAYER_VOTE_YES));
        assert_eq!(decode_player_vote(&Json::from("NO")), Ok(PlayerVote::PLAYER_VOTE_NO));
        assert_eq!(decode_player_vote(&Json::from("NEVER")), Ok(PlayerVote::PLAYER_VOTE_NEVER));
        assert_eq!(decode_player_vote(&Json::from("ABSTAIN")), Ok(PlayerVote::PLAYER_VOTE_ABSTAIN));
        assert_eq!(PlayerVote::PLAYER_VOTE_NEVER.0..=PlayerVote::PLAYER_VOTE_YES.0, -5..=-2);

        // Team votes: a team id (C++ (PlayerVoteTypes)eTeam), barbarians excluded.
        assert_eq!(MAX_CIV_TEAMS, 35);
        assert_eq!(decode_player_vote(&Json::Int(0)), Ok(PlayerVote::for_team(TeamId(0))));
        assert_eq!(decode_player_vote(&Json::Int(3)), Ok(PlayerVote::for_team(TeamId(3))));
        assert_eq!(decode_player_vote(&Json::Int(34)), Ok(PlayerVote::for_team(TeamId(34))));
        assert_eq!(decode_player_vote(&Json::Int(3)).unwrap().team(), Some(TeamId(3)));
        assert_eq!(player_vote_name(PlayerVote(3)), None);

        // Internal sentinels are vote state, never a choice: -1 would reset the vote, -6 would
        // skip the extra turn doVoteResults gives a missing vote.
        let state = "is vote state, not a choice (a vote choice is YES, NO, NEVER, ABSTAIN or (in a team vote) a team id)";
        assert_eq!(decode_player_vote(&Json::Int(-1)), Err(format!("-1 (NO_PLAYER_VOTE) {state}")));
        assert_eq!(decode_player_vote(&Json::Int(-6)), Err(format!("-6 (NO_PLAYER_VOTE_CHECKED) {state}")));
        assert_eq!(decode_player_vote(&Json::Float(-6.0)), Err(format!("-6 (NO_PLAYER_VOTE_CHECKED) {state}")));
        assert_eq!(decode_player_vote(&Json::from("NO_PLAYER_VOTE")), Err(format!("NO_PLAYER_VOTE {state}")));
        assert_eq!(
            decode_player_vote(&Json::from("NO_PLAYER_VOTE_CHECKED")),
            Err(format!("NO_PLAYER_VOTE_CHECKED {state}"))
        );

        let range = "out of range: -5..=-2 or a team id in 0..35";
        assert_eq!(decode_player_vote(&Json::Int(35)), Err(format!("vote choice 35 {range}")));
        assert_eq!(decode_player_vote(&Json::Int(-7)), Err(format!("vote choice -7 {range}")));
        assert_eq!(decode_player_vote(&Json::Int(i64::MAX)), Err(format!("vote choice {} {range}", i64::MAX)));
        assert!(decode_player_vote(&Json::from("MAYBE")).unwrap_err().starts_with("unknown vote choice \"MAYBE\""));
        assert!(decode_player_vote(&Json::from("")).is_err());
        assert!(decode_player_vote(&Json::from("PLAYER_VOTE_")).is_err());
        assert!(decode_player_vote(&Json::from("yes")).is_err());
        assert!(decode_player_vote(&Json::from("PLAYER_VOTE_PLAYER_VOTE_YES")).is_err());
        assert!(decode_player_vote(&Json::Float(0.5)).is_err());
        assert_eq!(
            decode_player_vote(&Json::Bool(true)),
            Err("expected a vote choice name or integer, got a boolean".to_string())
        );
    }

    #[test]
    fn placeholders() {
        assert!(has_placeholder("%D1: \"Learn how to play Somnium!\""));
        assert!(has_placeholder("x %s1"));
        assert!(!has_placeholder("Reduces the cost by 75%, increases crime by 5%"));
        assert!(!has_placeholder("%"));
        assert!(!has_placeholder("100%D"));
        assert!(!has_placeholder(""));
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
    fn text_keys_in_messages_resolve() {
        let d = ffh_data::global();
        assert_eq!(resolve_keys(d, "The borders of TXT_KEY_CITY_SHEAIM_1 have expanded"), "The borders of Galveholm have expanded");
        // Keys FfH2 inherits from BtS now resolve to their vendored stock text (tools/vendor_xml.py):
        // the unit names and the goody-hut message used to fall back to humanized words.
        assert_eq!(resolve_keys(d, "TXT_KEY_UNIT_WARRIOR died attacking TXT_KEY_UNIT_ARCHER."), "Warrior died attacking Archer.");
        assert_eq!(resolve_keys(d, "TXT_KEY_GOODY_MAP"), "The villagers have provided you with a map!");
        // A key with no text anywhere: its humanized words.
        assert_eq!(resolve_keys(d, "(TXT_KEY_NOT_A_REAL_KEY)"), "(Not A Real Key)");
        // Not keys.
        for s in ["", "plain", "TXT_KEY_", "TXT_KEY_ x", "a TXT_KEY", "TXT_KEY__"] {
            assert_eq!(resolve_keys(d, s), s, "{s:?}");
        }
        assert!(matches!(resolve_keys(d, "no keys here"), Cow::Borrowed(_)));
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
