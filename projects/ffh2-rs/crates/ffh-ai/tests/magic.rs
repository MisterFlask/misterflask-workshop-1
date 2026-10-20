//! Spellcasting (`magic.rs`): constructed, deterministic scenarios on a small hand-made map --
//! a mage next to an enemy city casts Fireball before our stack attacks it, an adept that knows
//! Enchanted Blade and Haste blesses its stack rather than hasting it at the end of a quiet turn,
//! a fresh summon attacks only a fight it has a chance in, and Donal recruits once in a big city.
//! They fix no unit's stats, only what the spells are for.

use std::sync::Arc;

use ffh_ai::magic;
use ffh_ai::view::View;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::map::{Map, MapInitData};
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::GameSetup;
use ffh_data::*;

/// One block of grassland (x 2..=17, y 3..=8) in an ocean, revealed to both players: a fresh
/// two-player AI game whose starting units are gone.
fn one_landmass_game(seed: u32) -> Game {
    let d = ffh_data::global();
    let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", seed));
    g.setup.options[GameOptionType::GAMEOPTION_NO_BARBARIANS] = true;
    g.map_script = Arc::new(FractalMapScript::default());
    let ai = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
    g.autorun = true;
    g.game_init();
    for u in g.players.iter().flat_map(|pl| pl.units.clone()).collect::<Vec<_>>() {
        if g.units.contains(u) {
            g.unit_kill(u, false, None);
        }
    }
    let (w, h) = (20, 12);
    let (np, nt) = (g.num_players(), g.num_teams());
    g.map = Map::new(d, &MapInitData { grid_w: w, grid_h: h, ..Default::default() }, np, nt);
    let grass = d.t::<TerrainType>("TERRAIN_GRASS");
    let coast = d.t::<TerrainType>("TERRAIN_COAST");
    for p in g.map.plot_ids() {
        let (x, y) = g.map.xy(p);
        let land = (3..=8).contains(&y) && (2..=17).contains(&x);
        g.plot_set_plot_type(p, if land { PlotType::PLOT_LAND } else { PlotType::PLOT_OCEAN }, false);
        g.plot_set_terrain_type(p, if land { grass } else { coast }, false);
    }
    g.map_recalculate_areas();
    g.update_plot_groups();
    for t in g.team_ids() {
        for p in g.map.plot_ids() {
            g.plot_set_revealed(p, t, true, false, None, false);
        }
    }
    g.update_plot_groups();
    g
}

fn give(g: &mut Game, u: UnitId, names: &[&str]) {
    for name in names {
        let pr = g.d.t::<PromotionType>(name);
        g.unit_set_has_promotion(u, pr, true);
    }
}

#[test]
fn a_mage_fireballs_the_city_before_the_strike() {
    let d = ffh_data::global();
    let mut g = one_landmass_game(11);
    let (us, them) = (PlayerId(0), PlayerId(1));
    if !magic::enabled(&g, us) {
        // FFH_AI_MAGIC_OFF names our civilization: nothing to test.
        return;
    }
    g.player_found(us, 4, 5);
    g.player_found(them, 12, 5);
    let target = g.players[them].capital_city.expect("the enemy founded a city");
    let tp = g.city_plot(target);
    for _ in 0..2 {
        g.player_init_unit(them, d.t::<UnitType>("UNIT_ARCHER"), 12, 5, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
    }
    // Our stack next to the city: a mage of the Fire sphere and two axemen about to attack.
    let mage = g.player_init_unit(us, d.t::<UnitType>("UNIT_MAGE"), 11, 5, Some(UnitAIType::UNITAI_MAGE), None);
    give(&mut g, mage, &["PROMOTION_FIRE1", "PROMOTION_FIRE2"]);
    let axes: Vec<UnitId> = (0..2).map(|_| g.player_init_unit(us, d.t::<UnitType>("UNIT_AXEMAN"), 11, 5, Some(UnitAIType::UNITAI_ATTACK), None)).collect();
    let (ut, tt) = (g.players[us].team, g.players[them].team);
    g.team_declare_war(ut, tt, true, None);
    assert!(g.at_war(ut, tt));
    let fireball = d.t::<SpellType>("SPELL_FIREBALL");
    assert!(g.unit_can_cast(mage, fireball, false), "the mage can cast Fireball next to the city");
    assert!(magic::attack_value(&g, mage, fireball, tp, &axes) > 0, "a Fireball on the defenders is worth casting");
    let hp_before: i32 = g.map.plots[tp].units.iter().map(|&t| g.unit_curr_hit_points(t)).sum();
    let defenders_before = g.map.plots[tp].units.len();
    let v = View::new(&g, us);
    magic::before_attack(&mut g, &v, tp, &axes);
    assert!(g.units[mage].has_casted, "the mage cast before the strike");
    let hp_after: i32 = g.map.plots[tp].units.iter().map(|&t| g.unit_curr_hit_points(t)).sum();
    assert!(
        hp_after < hp_before || g.map.plots[tp].units.len() < defenders_before,
        "the Fireball hurt the defenders ({hp_before} -> {hp_after} hit points)"
    );
}

#[test]
fn an_adept_blesses_its_stack_rather_than_hasting_it() {
    let d = ffh_data::global();
    let mut g = one_landmass_game(12);
    let us = PlayerId(0);
    if !magic::enabled(&g, us) {
        return;
    }
    g.player_found(us, 4, 5);
    // An adept that knows Enchanted Blade and Haste, with three axemen, in our city; no enemy
    // anywhere near.
    let adept = g.player_init_unit(us, d.t::<UnitType>("UNIT_ADEPT"), 4, 5, Some(UnitAIType::UNITAI_MAGE), None);
    give(&mut g, adept, &["PROMOTION_ENCHANTMENT1", "PROMOTION_BODY1"]);
    let axes: Vec<UnitId> = (0..3).map(|_| g.player_init_unit(us, d.t::<UnitType>("UNIT_AXEMAN"), 4, 5, Some(UnitAIType::UNITAI_ATTACK), None)).collect();
    let blade = d.t::<SpellType>("SPELL_ENCHANTED_BLADE");
    let haste = d.t::<SpellType>("SPELL_HASTE");
    assert!(g.unit_can_cast(adept, blade, false) && g.unit_can_cast(adept, haste, false));
    let v = View::new(&g, us);
    let (spell, _) = magic::choose_turn_spell(&g, &v, adept).expect("the adept casts something");
    assert_eq!(d.spells[spell].base.type_, "SPELL_ENCHANTED_BLADE", "a lasting blessing for the stack, not a move it will not use");
    magic::cast(&mut g, adept, spell, "test", 0);
    let eb = d.t::<PromotionType>("PROMOTION_ENCHANTED_BLADE");
    assert!(axes.iter().all(|&a| g.units[a].has_promotion[eb]), "every axeman on the plot is blessed");
    // Next quiet turn: nothing worth casting by what it does (Haste is gone before our units
    // would use it; `casting::end_turn` then leaves the cast to FfH's own choice).
    g.units[adept].has_casted = false;
    let v = View::new(&g, us);
    let next = magic::choose_turn_spell(&g, &v, adept);
    assert!(next.map_or(true, |(s, _)| s != haste), "Haste is worth nothing at the end of a quiet turn");
}

#[test]
fn a_summon_attacks_only_at_fair_odds() {
    let d = ffh_data::global();
    let mut g = one_landmass_game(13);
    let (us, them) = (PlayerId(0), PlayerId(1));
    if !magic::enabled(&g, us) {
        return;
    }
    g.player_found(us, 3, 5);
    g.player_found(them, 16, 5);
    let (ut, tt) = (g.players[us].team, g.players[them].team);
    g.team_declare_war(ut, tt, true, None);
    // Two enemies in the open, well apart: a Champion and a Warrior.
    let champion = g.player_init_unit(them, d.t::<UnitType>("UNIT_CHAMPION"), 8, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    let warrior = g.player_init_unit(them, d.t::<UnitType>("UNIT_WARRIOR"), 12, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    // Fresh summons of ours in suicide mode, as the end of the turn makes them: a Puppet next to
    // the Champion (Loki's puppets threw themselves at a Spectre every turn and left him to it),
    // an Earth Elemental next to the Warrior.
    let newest = g.players[us].units.iter().copied().max();
    let summon = |g: &mut Game, name: &str, x: i32| {
        let n = g.player_init_unit(us, d.t::<UnitType>(name), x, 5, Some(UnitAIType::UNITAI_ATTACK), None);
        g.units[n].ai_suicide_summon = true;
        n
    };
    let puppet = summon(&mut g, "UNIT_PUPPET", 7);
    summon(&mut g, "UNIT_EARTH_ELEMENTAL", 11);
    let v = View::new(&g, us);
    ffh_ai::casting::summons_attack(&mut g, &v, newest);
    assert!(g.units.contains(puppet) && !g.units[puppet].made_attack, "the Puppet stays out of a fight it cannot win");
    assert_eq!(g.units[champion].damage, 0);
    assert!(!g.units.contains(warrior) || g.units[warrior].damage > 0, "the Earth Elemental attacks the Warrior");
}

#[test]
fn a_summon_with_two_moves_reaches_an_enemy_two_plots_away() {
    let d = ffh_data::global();
    let mut g = one_landmass_game(14);
    let (us, them) = (PlayerId(0), PlayerId(1));
    if !magic::enabled(&g, us) {
        return;
    }
    g.player_found(us, 3, 5);
    g.player_found(them, 16, 5);
    let (ut, tt) = (g.players[us].team, g.players[them].team);
    g.team_declare_war(ut, tt, true, None);
    let warrior = g.player_init_unit(them, d.t::<UnitType>("UNIT_WARRIOR"), 9, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    g.units[warrior].damage = 60;
    // A fresh Spectre (two moves) two plots away, nothing next to it.
    let newest = g.players[us].units.iter().copied().max();
    let spectre = g.player_init_unit(us, d.t::<UnitType>("UNIT_SPECTRE"), 7, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    g.units[spectre].ai_suicide_summon = true;
    let v = View::new(&g, us);
    ffh_ai::casting::summons_attack(&mut g, &v, newest);
    assert!(!g.units.contains(warrior) || g.units[warrior].damage > 60, "the Spectre walked up to the Warrior and attacked");

    // The barbarians' summons attack only next to them (with this reach they took more cities).
    let barbarian = g.barbarian_player();
    assert!(g.at_war(g.players[barbarian].team, ut));
    let axeman = g.player_init_unit(us, d.t::<UnitType>("UNIT_AXEMAN"), 14, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    g.units[axeman].damage = 60;
    let newest = g.players[barbarian].units.iter().copied().max();
    let barbarian_spectre = g.player_init_unit(barbarian, d.t::<UnitType>("UNIT_SPECTRE"), 14, 7, Some(UnitAIType::UNITAI_ATTACK), None);
    g.units[barbarian_spectre].ai_suicide_summon = true;
    // (Its orders run in the barbarians' turn.)
    g.players[us].turn_active = false;
    g.players[barbarian].turn_active = true;
    let v = View::new(&g, barbarian);
    ffh_ai::casting::summons_attack(&mut g, &v, newest);
    assert!(g.units.contains(axeman) && g.units[axeman].damage == 60, "the barbarian Spectre stays out of reach");
}

#[test]
fn a_summon_is_worth_only_the_fights_it_would_take() {
    let d = ffh_data::global();
    let mut g = one_landmass_game(15);
    let (us, them) = (PlayerId(0), PlayerId(1));
    if !magic::enabled(&g, us) {
        return;
    }
    g.player_found(us, 3, 5);
    g.player_found(them, 12, 5);
    for _ in 0..2 {
        g.player_init_unit(them, d.t::<UnitType>("UNIT_CHAMPION"), 12, 5, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
    }
    let (ut, tt) = (g.players[us].team, g.players[them].team);
    g.team_declare_war(ut, tt, true, None);
    // A mage that knows Summon Spectre next to a city its Spectre could not hurt at 5% odds.
    let mage = g.player_init_unit(us, d.t::<UnitType>("UNIT_MAGE"), 11, 5, Some(UnitAIType::UNITAI_MAGE), None);
    give(&mut g, mage, &["PROMOTION_DEATH1", "PROMOTION_DEATH2"]);
    let spectre = d.t::<SpellType>("SPELL_SUMMON_SPECTRE");
    assert!(g.unit_can_cast(mage, spectre, false));
    let weight = d.spells[spectre].ai_weight;
    let v = View::new(&g, us);
    assert_eq!(magic::turn_value(&g, &v, mage, spectre), Some(weight), "a Spectre that would not attack is worth nothing");
    // A wounded Warrior two plots away: the Spectre walks up and takes it on.
    let warrior = g.player_init_unit(them, d.t::<UnitType>("UNIT_WARRIOR"), 9, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    g.units[warrior].damage = 60;
    let v = View::new(&g, us);
    assert!(magic::turn_value(&g, &v, mage, spectre).is_some_and(|value| value > weight), "the Warrior in reach is worth a Spectre");
}

#[test]
fn donal_recruits_once_in_a_big_city() {
    let d = ffh_data::global();
    let mut g = one_landmass_game(14);
    let us = PlayerId(0);
    if !magic::enabled(&g, us) {
        return;
    }
    g.player_found(us, 4, 5);
    g.player_found(us, 10, 5);
    let (big, small) = (g.players[us].cities[0], g.players[us].cities[1]);
    g.city_set_population(big, 9);
    g.city_set_population(small, 2);
    let recruit = d.t::<SpellType>("SPELL_RECRUIT");
    let donal = |g: &mut Game, x: i32| g.player_init_unit(us, d.t::<UnitType>("UNIT_DONAL"), x, 5, Some(UnitAIType::UNITAI_HERO), None);
    // In the small city: a single recruit, while the big city gives four.
    let elsewhere = donal(&mut g, 10);
    assert!(g.unit_can_cast(elsewhere, recruit, false), "Donal recruits in any city of ours");
    let v = View::new(&g, us);
    assert!(magic::turn_value(&g, &v, elsewhere, recruit).unwrap_or(0) <= 0, "not for one recruit");
    // In the big city: population 9 / 3 + 1 = 4 units, once.
    let home = donal(&mut g, 4);
    let v = View::new(&g, us);
    let (spell, value) = magic::choose_turn_spell(&g, &v, home).expect("Donal casts something");
    assert_eq!(spell, recruit, "the recruits beat Donal's other spells (value {value})");
    let before = g.players[us].units.len();
    magic::cast(&mut g, home, spell, "test", value);
    assert_eq!(g.players[us].units.len(), before + 4, "four recruits");
    g.units[home].has_casted = false;
    assert!(!g.unit_can_cast(home, recruit, false), "the Recruiter promotion is spent");
}
