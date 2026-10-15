//! Name resolution for every info table, GlobalTypes list and hard-coded C++ enum (P0).
//!
//! Coverage is checked against the sources that define the types, so a table or enum added
//! later without a `names::KINDS` entry fails here: `ffh_data::INFO_FILES` (every info table),
//! the `info_ids!` list in ffh-data's registry.rs (every registry class) and the generated
//! enums.rs (every `NAMES` array).

use std::collections::BTreeSet;

use ffh_data::*;
use ffh_play::json::Json;
use ffh_play::names::{self, Kind, Named, Source, KINDS};

fn d() -> &'static GameData {
    global()
}

const REGISTRY_RS: &str = include_str!("../../ffh-data/src/registry.rs");
const ENUMS_RS: &str = include_str!("../../ffh-data/src/enums.rs");

/// `(type, class)` pairs of the `info_ids!` list in registry.rs.
fn registry_info_ids() -> Vec<(String, String)> {
    let start = REGISTRY_RS.find("info_ids! {").expect("info_ids! list in registry.rs");
    let body = &REGISTRY_RS[start..];
    let body = &body[..body.find("\n}").expect("end of info_ids! list")];
    body.lines()
        .filter_map(|l| {
            let (t, c) = l.trim().split_once(" => ")?;
            Some((t.to_string(), c.trim_end_matches(',').trim_matches('"').to_string()))
        })
        .collect()
}

/// Types of enums.rs with a `NAMES` array.
fn enums_with_names() -> Vec<String> {
    let mut out = Vec::new();
    let mut current = None;
    for l in ENUMS_RS.lines() {
        if let Some(rest) = l.strip_prefix("impl ") {
            current = rest.strip_suffix(" {").map(str::to_string);
        } else if l.contains("pub const NAMES") {
            out.push(current.clone().expect("NAMES inside an impl block"));
        }
    }
    out
}

#[test]
fn kinds_cover_every_table_class_and_enum() {
    // Every info table, in load order.
    let table_classes: Vec<&str> = KINDS.iter().filter_map(|k| match k.source {
        Source::Table { class, .. } => Some(class),
        _ => None,
    }).collect();
    let info_classes: Vec<&str> = INFO_FILES.iter().map(|(_, c)| *c).collect();
    assert_eq!(table_classes, info_classes, "names::KINDS tables vs ffh_data::INFO_FILES");

    // Every registry class (tables and GlobalTypes lists), with the right kind name.
    let ids = registry_info_ids();
    assert!(ids.len() >= 60, "parsed only {} info ids", ids.len());
    let got: BTreeSet<(String, String)> =
        KINDS.iter().filter_map(|k| Some((k.kind.to_string(), k.class()?.to_string()))).collect();
    let want: BTreeSet<(String, String)> = ids.into_iter().collect();
    assert_eq!(got, want, "names::KINDS registry kinds vs registry.rs info_ids!");

    // Every hard-coded enum with NAMES (as an Enum kind, or as a registry kind).
    let enums = enums_with_names();
    assert!(enums.len() >= 30, "parsed only {} enums", enums.len());
    for e in &enums {
        assert!(names::kind(e).is_some(), "enum {e} has NAMES but no names::KINDS entry");
    }
    for k in KINDS.iter().filter(|k| k.source == Source::Enum) {
        assert!(enums.iter().any(|e| e == k.kind), "{} is not an enums.rs type with NAMES", k.kind);
    }
}

/// Exhaustive per-index checks through the type-erased accessors (which call the typed
/// `Named` impls and `names::decode::<T>`).
fn check_kind(d: &GameData, k: &Kind) {
    let n = (k.count)(d);
    if let Source::Table { class, .. } | Source::Global { class } = k.source {
        assert_eq!(n, d.reg.count_class(class), "{}: count vs registry", k.kind);
    }
    let mut seen = BTreeSet::new();
    for i in 0..n {
        let name = (k.name)(d, i);
        assert!(!name.is_empty() && name != "?", "{}[{i}] has no name", k.kind);
        assert!(seen.insert(name), "{}: duplicate name {name}", k.kind);
        assert_eq!((k.lookup)(d, name), Some(i), "{}: lookup({name})", k.kind);
        assert_eq!((k.decode)(d, &Json::from(name)), Ok(i), "{}: decode({name:?})", k.kind);
        assert_eq!((k.decode)(d, &Json::Int(i as i64)), Ok(i), "{}: decode({i})", k.kind);
        assert_eq!((k.decode)(d, &Json::Float(i as f64)), Ok(i), "{}: decode({i}.0)", k.kind);
        assert!(!k.display(d, i).is_empty(), "{}[{i}] has no display name", k.kind);
        if k.source == Source::Enum {
            assert_eq!((k.text_key)(d, i), None);
        }
        // Names are unique across all kinds, so a bare type name is never ambiguous.
        let found = names::find(d, name);
        assert_eq!(found.len(), 1, "{name} names {:?}", found.iter().map(|(k, _)| k.kind).collect::<Vec<_>>());
        assert_eq!((found[0].0.kind, found[0].1), (k.kind, i));
    }
    assert_eq!((k.name)(d, n), "?");
    assert_eq!((k.text_key)(d, n), None);
    let err = (k.decode)(d, &Json::Int(n as i64)).unwrap_err();
    assert_eq!(err, format!("{} index {n} out of range 0..{n}", k.kind));
    assert!((k.decode)(d, &Json::Int(-1)).is_err());
    assert!((k.decode)(d, &Json::Float(0.5)).is_err());
    assert!((k.decode)(d, &Json::Bool(true)).is_err());
    assert!((k.decode)(d, &Json::Null).is_err());
    assert_eq!((k.decode)(d, &Json::from("NO_SUCH_TYPE")), Err(format!("unknown {} \"NO_SUCH_TYPE\"", k.kind)));
    assert_eq!((k.lookup)(d, ""), None);
}

#[test]
fn every_kind_resolves_every_name_and_index() {
    let d = d();
    for k in KINDS {
        check_kind(d, k);
        // Tables are non-empty except the ones FfH2 empties (no corporations).
        if k.kind != "CorporationType" {
            assert!((k.count)(d) > 0, "{} is empty", k.kind);
        }
        assert!(std::ptr::eq(names::kind(k.kind).unwrap(), k));
        if let Some(t) = k.table() {
            assert!(std::ptr::eq(names::kind(t).unwrap(), k));
        }
    }
    assert!(names::kind("NoSuchType").is_none());
    assert!(names::find(d, "NO_SUCH_TYPE").is_empty());
}

/// Info tables whose order is also a hard-coded C++ enum: the registry names (used by `Named`)
/// are the enum's `NAMES`, as `GameData::check_enum_orders` requires at load.
#[test]
fn registry_names_match_hard_coded_enums() {
    let d = d();
    macro_rules! same {
        ($($t:ident),* $(,)?) => { $(
            assert!(<$t as Named>::count(d) >= $t::NAMES.len(), "{}", stringify!($t));
            for (i, n) in $t::NAMES.iter().enumerate() {
                assert_eq!(names::name(d, $t::from_usize(i)), *n, "{}[{i}]", stringify!($t));
                assert_eq!(names::lookup::<$t>(d, n), Some($t::from_usize(i)));
            }
        )* };
    }
    same!(
        AlignmentType, AttitudeType, AutomateType, CalendarType, CitySizeType, CommandType, CommerceType,
        ContactType, DamageType, DenialType, DiplomacyPowerType, DirectionType, DomainType, GameOptionType,
        MemoryType, MissionType, UnitAIType, WorldSizeType, YieldType,
    );
}

#[test]
fn typed_decoding() {
    let d = d();
    let warrior = d.t::<UnitType>("UNIT_WARRIOR");
    assert_eq!(names::decode::<UnitType>(d, &Json::from("UNIT_WARRIOR")), Ok(warrior));
    assert_eq!(names::decode::<UnitType>(d, &Json::Int(warrior.idx() as i64)), Ok(warrior));
    // A name of another kind is not accepted.
    assert_eq!(
        names::decode::<UnitType>(d, &Json::from("TECH_NECROMANCY")),
        Err("unknown UnitType \"TECH_NECROMANCY\"".to_string())
    );
    assert_eq!(
        names::decode::<OrderType>(d, &Json::Int(4)),
        Err("OrderType index 4 out of range 0..4".to_string())
    );
    assert_eq!(
        names::decode::<TechType>(d, &Json::arr()),
        Err("expected a TechType name or index, got an array".to_string())
    );
    assert_eq!(names::decode_opt::<TechType>(d, &Json::Null), Ok(None));
    assert_eq!(names::decode_opt::<TechType>(d, &Json::Int(-1)), Ok(None));
    assert_eq!(names::decode_opt::<TechType>(d, &Json::Int(0)), Ok(Some(TechType(0))));
    assert!(names::decode_opt::<TechType>(d, &Json::Int(-2)).is_err());
    assert_eq!(names::decode::<ActivityType>(d, &Json::from("ACTIVITY_SLEEP")), Ok(ActivityType::ACTIVITY_SLEEP));
    assert_eq!(names::decode::<TaskType>(d, &Json::from("TASK_HURRY")), Ok(TaskType::TASK_HURRY));
    assert_eq!(names::decode::<MissionType>(d, &Json::from("MISSION_FORTIFY")), Ok(MissionType::MISSION_FORTIFY));
    assert_eq!(names::decode::<PlayerOptionType>(d, &Json::Int(6)), Ok(PlayerOptionType::PLAYEROPTION_QUICK_MOVES));

    // Fields of a request object.
    let req = Json::parse(
        r#"{"c":"push_order","order":"ORDER_TRAIN","data":"UNIT_WARRIOR","tech":null,"rel":-1,"civics":["CIVIC_DESPOTISM",0],"bad":[0,"X"]}"#,
    )
    .unwrap();
    assert_eq!(names::field::<OrderType>(d, &req, "order"), Ok(OrderType::ORDER_TRAIN));
    assert_eq!(names::field::<UnitType>(d, &req, "data"), Ok(warrior));
    assert_eq!(names::opt_field::<TechType>(d, &req, "tech"), Ok(None));
    assert_eq!(names::opt_field::<ReligionType>(d, &req, "rel"), Ok(None));
    assert_eq!(names::opt_field::<ReligionType>(d, &req, "absent"), Ok(None));
    let despotism = d.t::<CivicType>("CIVIC_DESPOTISM");
    assert_eq!(names::list_field::<CivicType>(d, &req, "civics"), Ok(vec![despotism, CivicType(0)]));
    let e = names::list_field::<CivicType>(d, &req, "bad").unwrap_err();
    assert_eq!(e.to_string(), "field `bad`: element 1: unknown CivicType \"X\"");
    let e = names::field::<UnitType>(d, &req, "order").unwrap_err();
    assert_eq!(e.to_string(), "field `order`: unknown UnitType \"ORDER_TRAIN\"");
    assert_eq!(names::field::<UnitType>(d, &req, "absent").unwrap_err().to_string(), "field `absent`: missing");
    assert_eq!(names::list_field::<CivicType>(d, &req, "c").unwrap_err().msg, "expected an array, got \"push_order\"");
}

#[test]
fn display_names_and_text() {
    let d = d();
    let necro = d.t::<TechType>("TECH_NECROMANCY");
    assert_eq!(names::display(d, necro), "Necromancy");
    assert_eq!(names::text(d, &d.techs[necro].base.description), "Necromancy");
    // BtS-inherited text is not vendored: the type name stands in.
    assert_eq!(names::display(d, d.t::<UnitType>("UNIT_WARRIOR")), "Warrior");
    assert_eq!(names::display(d, ActivityType::ACTIVITY_SLEEP), "Sleep");
    assert_eq!(names::display(d, d.t::<BonusType>("BONUS_MANA_FIRE")), "Fire Mana");
    assert_eq!(names::text(d, "TXT_KEY_NO_SUCH_KEY"), "TXT_KEY_NO_SUCH_KEY");

    // The full description: FfH2's English text has no gendered forms, and a ':' is prose.
    assert_eq!(names::display(d, d.t::<VoteType>("VOTE_LIBERTY")), "Global Civic: Liberty");
    assert_eq!(
        names::display(d, d.t::<VoteType>("VOTE_SINGLE_CURRENCY")),
        "Single Currency: +1 Trade Route ([ICON_TRADE]) in all cities"
    );
    // An unfilled placeholder (`%D1: "..."`) falls back to the type name, so the two Somnium
    // memories stay readable and distinct.
    let pos = names::display(d, d.t::<MemoryType>("MEMORY_SOMNIUM_POSITIVE"));
    let neg = names::display(d, d.t::<MemoryType>("MEMORY_SOMNIUM_NEGATIVE"));
    assert_eq!((pos.as_ref(), neg.as_ref()), ("Somnium Positive", "Somnium Negative"));

    // Over every kind: no display holds a placeholder, and a description with a ':' is shown
    // whole (the 9 votes, at least).
    let placeholder = |s: &str| s.as_bytes().windows(3).any(|w| w[0] == b'%' && w[1].is_ascii_alphabetic() && w[2].is_ascii_digit());
    let mut with_colon = 0;
    for k in KINDS {
        for i in 0..(k.count)(d) {
            let disp = k.display(d, i);
            assert!(!placeholder(&disp) && disp.trim() == disp, "{}[{i}] displays {disp:?}", k.kind);
            if let Some(key) = (k.text_key)(d, i) {
                let t = names::text(d, key);
                if t.contains(':') && !placeholder(t) {
                    assert_eq!(disp, t, "{}[{i}]", k.kind);
                    with_colon += 1;
                }
            }
        }
    }
    assert!(with_colon >= 9, "only {with_colon} descriptions with a ':'");
}

#[test]
fn names_json_round_trip() {
    let d = d();
    let j = names::names_json(d);
    let s = j.to_string();
    assert!(s.len() > 50_000, "names JSON is only {} bytes", s.len());
    assert_eq!(Json::parse(&s).unwrap(), j);
    assert_eq!(Json::parse(&j.to_pretty()).unwrap(), j);
    assert_eq!(j.as_obj().unwrap().len(), KINDS.len());
    let units = j.get("UnitType").and_then(Json::as_arr).unwrap();
    assert_eq!(units.len(), d.units.len());
    let warrior = d.t::<UnitType>("UNIT_WARRIOR");
    assert_eq!(units[warrior.idx()], Json::from("UNIT_WARRIOR"));
    // Deterministic: the same document twice.
    assert_eq!(names::names_json(d).to_string(), s);
}
