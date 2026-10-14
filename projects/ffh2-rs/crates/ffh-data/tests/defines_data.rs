//! Data spot-checks for the global defines (`defines.rs`, `defines_gen.rs`): C++
//! `ReadGlobalDefines` over GlobalDefines.xml, GlobalDefinesAlt.xml and
//! PythonCallbackDefines.xml (later files override), then `SetPostGlobalsGlobalDefines`.
//! Expected values were read by hand from `data/xml/*Defines*.xml`.

use ffh_data::*;

fn d() -> &'static GameData {
    ffh_data::global()
}

#[test]
fn int_and_float_defines() {
    let d = d();
    assert_eq!(d.defines.max_hit_points, 100);
    assert_eq!(d.defines.move_denominator, 60);
    assert_eq!(d.defines.hills_extra_defense, 25);
    assert_eq!(d.defines.food_consumption_per_population, 2);
    assert_eq!(d.defines.plot_size, 180.0);
    assert_eq!(d.defines.camera_min_yaw, -45.0);
    // FfH-only define (GlobalDefinesAlt.xml).
    assert_eq!(d.defines.flames_spread_chance, 20);
    // PythonCallbackDefines.xml
    assert_eq!(d.defines.use_cannot_found_city_callback, 0);
}

#[test]
fn global_defines_alt_overrides_global_defines() {
    // GlobalDefines.xml -> GlobalDefinesAlt.xml (C++ SetGlobalDefines reads Alt second).
    let d = d();
    assert_eq!(d.defines.start_year, 0); // -4000 in GlobalDefines.xml
    assert_eq!(d.defines.river_attack_modifier, -40); // -25
    assert_eq!(d.defines.golden_age_length, 12); // 8
    assert_eq!(d.defines.num_building_prereq_or_bonuses, 5); // 4
    assert_eq!(d.defines.max_experience_per_combat, 999); // 10
    assert_eq!(d.defines.max_national_wonders_per_city, -1); // 2
}

#[test]
fn post_globals_text_defines_resolve_to_type_indices() {
    let d = d();
    let civ = |n| d.t::<CivilizationType>(n).idx() as i32;
    assert_eq!(d.defines.barbarian_civilization, civ("CIVILIZATION_BARBARIAN"));
    assert_eq!(d.defines.random_civilization, civ("CIVILIZATION_RANDOM"));
    assert_eq!(d.defines.barbarian_leader, d.t::<LeaderHeadType>("LEADER_BARBARIAN").idx() as i32);
    assert_eq!(d.defines.land_terrain, d.t::<TerrainType>("TERRAIN_GRASS").idx() as i32);
    assert_eq!(d.defines.flames_feature, d.t::<FeatureType>("FEATURE_FLAMES").idx() as i32);
    assert_eq!(d.defines.flames_expire_effect, d.t::<FeatureType>("FEATURE_FOREST_BURNT").idx() as i32);
    assert_eq!(d.defines.flames_spread_effect, d.t::<ImprovementType>("IMPROVEMENT_SMOKE").idx() as i32);
    assert_eq!(d.defines.slave_unit, d.t::<UnitType>("UNIT_SLAVE").idx() as i32);
    assert_eq!(d.defines.weapon_promotion_tier2, d.t::<PromotionType>("PROMOTION_IRON_WEAPONS").idx() as i32);
    assert_eq!(d.defines.weapon_req_bonus_tier3, d.t::<BonusType>("BONUS_MITHRIL").idx() as i32);
    assert_eq!(d.defines.bonusclass_mana_raw, d.t::<BonusClassType>("BONUSCLASS_RAWMANA").idx() as i32);
    assert_eq!(d.defines.spell_haste, d.t::<SpellType>("SPELL_HASTE").idx() as i32);
    assert_eq!(d.defines.unitclass_scout, d.t::<UnitClassType>("UNITCLASS_SCOUT").idx() as i32);
    // TurnTimerInfos are not loaded (UI/multiplayer only), so this one cannot resolve.
    assert_eq!(d.defines.standard_turntimer, -1);
}

#[test]
fn unlisted_text_define_is_not_a_type_index() {
    // C++ quirk: FLOOD_PLAINS_FEATURE is not in SetPostGlobalsGlobalDefines, so it stays a
    // string define and getDefineINT returns the string's address -- never a feature index.
    // The CvCity::init "keep flood plains" check therefore never matches.
    let d = d();
    assert!(d.ty::<FeatureType>("FEATURE_FLOOD_PLAINS").is_some());
    assert_eq!(d.defines.flood_plains_feature, -1);
    assert_eq!(d.define_map.string("FLOOD_PLAINS_FEATURE"), "FEATURE_FLOOD_PLAINS");
    assert_eq!(d.define_map.string("GAME_CITY_SIZE_METHOD"), "METHOD_EXPONENTIAL");
}

#[test]
fn missing_define_reads_zero() {
    // C++ getDefineINT/getDefineFLOAT leave their 0 default when the name is unknown
    // (e.g. CvUnitAI reads BBAI_SKIP_BOMBARD_* that no XML defines).
    let d = d();
    assert_eq!(d.define_map.int("BBAI_SKIP_BOMBARD_BEST_ATTACK_ODDS"), 0);
    assert_eq!(d.define_map.float("NO_SUCH_DEFINE"), 0.0);
    assert_eq!(d.define_map.string("NO_SUCH_DEFINE"), "");
}
