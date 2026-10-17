//! Amphibious invasion (`naval.rs` + `campaign.rs`/`diplomacy.rs`/`production.rs`): a
//! constructed, deterministic scenario where a civ must ship an army across water to take an
//! enemy city on a landmass where it has no city.
//!
//! The map is built by hand: two land blocks separated by open ocean, revealed to both teams.
//! The attacker (an AI player) has a coastal capital with a transport and a few attackers; the
//! enemy has one weakly defended coastal city across the water. We are at war. The AI, left to
//! run, boards the army, sails it over, lands it beside the enemy city and takes the city.

use std::sync::Arc;

use ffh_ai::naval;
use ffh_ai::production::{best_unit, Role};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::map::{Map, MapInitData};
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::GameSetup;
use ffh_data::*;

fn attach_ai(g: &mut Game) {
    let ai = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
}

/// Plays `g` to the next turn boundary.
fn play_turn(g: &mut Game) {
    for _ in 0..200_000 {
        g.update();
        if g.at_turn_boundary() || g.game_state != GameStateType::GAMESTATE_ON {
            return;
        }
    }
    panic!("turn {} does not end", g.game_turn());
}

/// Two land blocks (A: x in 2..=7, B: x in 12..=17; both y 3..=8) in one big ocean. A fresh AI
/// game with the players/teams/turn machinery of `game_init`, its map replaced by this one.
fn two_landmass_game(seed: u32) -> Game {
    let d = ffh_data::global();
    let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", seed));
    // No barbarians: the scenario is just the two civs.
    g.setup.options[GameOptionType::GAMEOPTION_NO_BARBARIANS] = true;
    g.map_script = Arc::new(FractalMapScript::default());
    attach_ai(&mut g);
    g.autorun = true;
    g.game_init();

    // Clear the generated starting units: we place our own scenario.
    for u in g.players.iter().flat_map(|pl| pl.units.clone()).collect::<Vec<_>>() {
        if g.units.contains(u) {
            g.unit_kill(u, false, None);
        }
    }

    // Replace the map with two land blocks in one ocean.
    let (w, h) = (24, 12);
    let (np, nt) = (g.num_players(), g.num_teams());
    g.map = Map::new(d, &MapInitData { grid_w: w, grid_h: h, ..Default::default() }, np, nt);
    let grass = d.t::<TerrainType>("TERRAIN_GRASS");
    let coast = d.t::<TerrainType>("TERRAIN_COAST");
    let is_land = |x: i32, y: i32| (3..=8).contains(&y) && ((2..=7).contains(&x) || (12..=17).contains(&x));
    for p in g.map.plot_ids() {
        let (x, y) = g.map.xy(p);
        let land = is_land(x, y);
        g.plot_set_plot_type(p, if land { PlotType::PLOT_LAND } else { PlotType::PLOT_OCEAN }, false);
        g.plot_set_terrain_type(p, if land { grass } else { coast }, false);
    }
    g.map_recalculate_areas();
    g.update_plot_groups();
    // Reveal the whole map to both teams (so both cities and the sea route are known).
    for t in g.team_ids() {
        for p in g.map.plot_ids() {
            g.plot_set_revealed(p, t, true, false, None, false);
        }
    }
    g.update_plot_groups();
    g
}

#[test]
fn ai_ships_an_army_overseas_and_takes_an_enemy_city() {
    let d = ffh_data::global();
    let mut g = two_landmass_game(7);
    let (attacker, enemy) = (PlayerId(0), PlayerId(1));
    let (at, et) = (g.players[attacker].team, g.players[enemy].team);

    // Attacker's coastal capital on landmass A (east edge, coast to the east); enemy city on
    // landmass B across the water.
    g.player_found(attacker, 7, 5);
    g.player_found(enemy, 12, 5);
    let capital = g.players[attacker].capital_city.expect("attacker founded a capital");
    let target = g.players[enemy].capital_city.expect("enemy founded a city");
    assert_ne!(g.map.plots[g.city_plot(capital)].area, g.map.plots[g.city_plot(target)].area, "cities on different landmasses");
    assert!(naval::is_coastal_city(&g, capital), "the attacker's capital is coastal");

    // The attacker has the whole tech tree (so its capital can train a real attacker and
    // `can_take_city` can judge the assault); the enemy stays primitive.
    for tech in d.techs.keys() {
        g.team_set_has_tech(at, tech, true, Some(attacker), false, false);
    }

    // Place the assault force at the capital: a transport (the one that carries the most land
    // units), a city-defense garrison so the attackers are not drafted into it, and attackers.
    let transport = d.units.keys().filter(|&u| naval::is_transport_type(&d.units[u])).max_by_key(|&u| (d.units[u].cargo_space, std::cmp::Reverse(u))).expect("a land-cargo transport exists");
    let cargo = d.units[transport].cargo_space.min(3).max(2);
    let attacker_unit = best_unit(&g, capital, Role::CityAttack).or_else(|| best_unit(&g, capital, Role::Attack)).unwrap_or_else(|| d.t::<UnitType>("UNIT_CHAMPION"));
    g.player_init_unit(attacker, d.t::<UnitType>("UNIT_WARRIOR"), 7, 5, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
    g.player_init_unit(attacker, transport, 7, 5, Some(UnitAIType::UNITAI_ASSAULT_SEA), None);
    for _ in 0..cargo {
        g.player_init_unit(attacker, attacker_unit, 7, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    }
    // The enemy city holds one weak defender.
    g.player_init_unit(enemy, d.t::<UnitType>("UNIT_SCOUT"), 12, 5, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);

    g.team_declare_war(at, et, true, None);
    assert!(g.at_war(at, et));

    // The target function finds the enemy city as the amphibious target, launching from the
    // capital.
    let amph = naval::amphibious_target(&g, attacker).expect("an amphibious target across the water");
    assert_eq!(amph.city, target, "the overseas enemy city is the target");
    assert_eq!(amph.staging, capital, "the coastal capital is the staging port");

    // Run the AI: it should board the army, sail it over, land it and take the city.
    let start = g.game_turn();
    let mut struck = false;
    let mut captured = false;
    for _ in 0..20 {
        play_turn(&mut g);
        if g.cities.contains(target) && g.cities[target].defense_damage > 0 {
            struck = true;
        }
        // Capture: the enemy city is now ours (same city, new owner).
        if g.cities.contains(target) && g.cities[target].owner == attacker {
            captured = true;
            break;
        }
        // Or the enemy was wiped out (its city taken, then it died).
        if !g.players[enemy].alive {
            captured = true;
            break;
        }
        if g.game_state != GameStateType::GAMESTATE_ON {
            break;
        }
    }
    let _ = struck;
    assert!(
        captured,
        "the attacker took the overseas enemy city by turn {} (started {start}); enemy city owner {:?}, alive {}",
        g.game_turn(),
        g.cities.contains(target).then(|| g.cities[target].owner),
        g.players[enemy].alive,
    );
}

#[test]
fn no_amphibious_target_without_a_coastal_city_or_an_enemy() {
    let g = two_landmass_game(7);
    // No cities founded, no war: nothing to invade.
    assert!(naval::amphibious_target(&g, PlayerId(0)).is_none());
}

/// A small transport (least cargo) against a garrison bigger than one load: the invasion must
/// ferry several loads, and they must land on the *same* plot beside the city and pile into a
/// stack that can strike (`transport_assault` lands on the reachable plot already holding the most
/// of our army). If each trip chose a fresh plot the force would scatter and the city would never
/// fall. Robust to content: it fixes no stats, only that one small load < the garrison < the
/// attackers provided, and asserts the overseas city is taken.
#[test]
fn ai_takes_a_defended_overseas_city_with_a_small_transport() {
    let d = ffh_data::global();
    let mut g = two_landmass_game(7);
    let (attacker, enemy) = (PlayerId(0), PlayerId(1));
    let (at, et) = (g.players[attacker].team, g.players[enemy].team);

    g.player_found(attacker, 7, 5);
    g.player_found(enemy, 12, 5);
    let capital = g.players[attacker].capital_city.expect("attacker founded a capital");
    let target = g.players[enemy].capital_city.expect("enemy founded a city");
    assert_ne!(g.map.plots[g.city_plot(capital)].area, g.map.plots[g.city_plot(target)].area, "cities on different landmasses");

    for tech in d.techs.keys() {
        g.team_set_has_tech(at, tech, true, Some(attacker), false, false);
    }

    // The least-cargo transport, so each trip lands only a few units.
    let transport = d
        .units
        .keys()
        .filter(|&u| naval::is_transport_type(&d.units[u]) && d.units[u].cargo_space > 0)
        .min_by_key(|&u| (d.units[u].cargo_space, u))
        .expect("a land-cargo transport exists");
    let cargo = d.units[transport].cargo_space.max(1);
    let attacker_unit = best_unit(&g, capital, Role::CityAttack).or_else(|| best_unit(&g, capital, Role::Attack)).unwrap_or_else(|| d.t::<UnitType>("UNIT_CHAMPION"));

    // A garrison larger than one load, and attackers enough for several loads.
    let defenders = cargo + 2;
    let attackers = (cargo + 2) * 3;
    g.player_init_unit(attacker, d.t::<UnitType>("UNIT_WARRIOR"), 7, 5, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
    g.player_init_unit(attacker, transport, 7, 5, Some(UnitAIType::UNITAI_ASSAULT_SEA), None);
    for _ in 0..attackers {
        g.player_init_unit(attacker, attacker_unit, 7, 5, Some(UnitAIType::UNITAI_ATTACK), None);
    }
    for _ in 0..defenders {
        g.player_init_unit(enemy, d.t::<UnitType>("UNIT_WARRIOR"), 12, 5, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
    }

    g.team_declare_war(at, et, true, None);
    assert_eq!(naval::amphibious_target(&g, attacker).map(|a| a.city), Some(target), "the overseas enemy city is the target");

    let mut captured = false;
    for _ in 0..90 {
        play_turn(&mut g);
        if (g.cities.contains(target) && g.cities[target].owner == attacker) || !g.players[enemy].alive {
            captured = true;
            break;
        }
        if g.game_state != GameStateType::GAMESTATE_ON {
            break;
        }
    }
    assert!(
        captured,
        "the attacker took the defended overseas city by turn {} (cargo {cargo}, {defenders} defenders); enemy city owner {:?}, alive {}",
        g.game_turn(),
        g.cities.contains(target).then(|| g.cities[target].owner),
        g.players[enemy].alive,
    );
}
