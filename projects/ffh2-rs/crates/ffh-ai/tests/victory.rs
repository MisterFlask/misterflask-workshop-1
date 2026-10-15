//! Victory pursuit (`victory.rs`): game states where one victory is near, built from a real
//! game (a few turns of AI play, then the techs, buildings or culture set by hand), and the
//! planner's choice and what production, research and the sliders do with it.

use std::sync::Arc;

use ffh_ai::production::Choice;
use ffh_ai::victory::{self, Goal};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::GameSetup;
use ffh_data::*;

fn attach_ai(g: &mut Game) {
    let ai = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
}

/// A 3-civ small-map game played `turns` turns by the AI.
fn played(seed: u32, turns: i32) -> Game {
    let d = ffh_data::global();
    let setup = GameSetup::standard(d, 3, "WORLDSIZE_SMALL", seed);
    let mut g = Game::new(d, setup);
    g.map_script = Arc::new(FractalMapScript::default());
    attach_ai(&mut g);
    g.autorun = true;
    g.game_init();
    while g.game_state == GameStateType::GAMESTATE_ON && g.game_turn() < turns {
        for _ in 0..200_000 {
            g.update();
            if g.at_turn_boundary() {
                break;
            }
        }
    }
    g
}

/// Jump to `turn` (late enough for every civ to plan for a victory).
fn late(g: &mut Game, turn: i32) {
    let elapsed = turn - g.game_turn();
    g.set_game_turn(turn);
    g.elapsed_game_turns += elapsed;
    assert!(g.max_turns() > turn, "the game has a time limit after turn {turn}");
}

fn give_techs(g: &mut Game, p: PlayerId, targets: &[TechType]) {
    let team = g.players[p].team;
    // Prerequisites first (each target's path, cheapest first).
    let mut path = victory::tech_path(g, team, targets);
    path.sort_by_key(|&t| (g.d.techs[t].research_cost, t));
    for t in path {
        g.team_set_has_tech(team, t, true, Some(p), false, false);
    }
}

/// The civ with the most cities (ties to the lower id).
fn biggest(g: &Game) -> PlayerId {
    g.civ_player_ids().filter(|&p| g.players[p].alive && !g.player_is_barbarian(p)).max_by_key(|&p| (g.players[p].cities.len(), std::cmp::Reverse(p))).unwrap()
}

fn tower_building(g: &Game, p: PlayerId, class: &str) -> BuildingType {
    let civ = g.player_get_civilization_type(p).unwrap();
    g.d.civ_building(civ, g.d.t::<BuildingClassType>(class)).unwrap()
}

const LESSER: [&str; 4] = ["BUILDINGCLASS_TOWER_OF_ALTERATION", "BUILDINGCLASS_TOWER_OF_DIVINATION", "BUILDINGCLASS_TOWER_OF_NECROMANCY", "BUILDINGCLASS_TOWER_OF_THE_ELEMENTS"];

/// The four lesser towers in `p`'s capital, the techs up to Strength of Will (all but
/// `except`) and most of the Tower of Mastery's hammers in `p`'s best city: the Tower of
/// Mastery is a few turns away.
fn near_tower(g: &mut Game, p: PlayerId, except: Option<&str>) {
    let d = g.d;
    let mut techs: Vec<TechType> = LESSER.iter().map(|&bc| d.buildings[tower_building(g, p, bc)].prereq_and_tech.unwrap()).collect();
    techs.push(d.t::<TechType>("TECH_STRENGTH_OF_WILL"));
    give_techs(g, p, &techs);
    if let Some(t) = except {
        let team = g.players[p].team;
        g.team_set_has_tech(team, d.t::<TechType>(t), false, Some(p), false, false);
    }
    let capital = g.players[p].capital_city.unwrap();
    for bc in LESSER {
        let b = tower_building(g, p, bc);
        g.city_set_num_real_building(capital, b, 1);
    }
    let mastery = tower_building(g, p, "BUILDINGCLASS_TOWER_OF_MASTERY");
    let best = best_city(g, p);
    let cost = g.city_production_needed_building(best, mastery);
    g.cities[best].building_production[mastery] = cost - g.city_yield_rate(best, YieldType::YIELD_PRODUCTION) * 3;
}

/// `p`'s city with the most hammers (ties to the older city), as the planner ranks them.
fn best_city(g: &Game, p: PlayerId) -> CityId {
    g.players[p].cities.iter().copied().max_by_key(|&c| (g.city_yield_rate(c, YieldType::YIELD_PRODUCTION), std::cmp::Reverse(c))).unwrap()
}

#[test]
fn tower_of_mastery_near_is_pursued() {
    let mut g = played(11, 80);
    let p = biggest(&g);
    late(&mut g, 400);
    near_tower(&mut g, p, None);
    let team = g.players[p].team;
    let st = victory::tower_state(&g, p).expect("the Tower of Mastery victory is on");
    let mastery = tower_building(&g, p, "BUILDINGCLASS_TOWER_OF_MASTERY");
    assert_eq!(st.missing, vec![mastery], "only the Tower of Mastery is missing");
    assert!(st.techs.is_empty(), "no tech is missing: {:?}", st.techs);
    assert!(victory::tower_eta(&g, p).unwrap() <= 5);
    let plan = victory::plan(&g, p);
    assert_eq!(plan.goal(), Some(Goal::Tower), "{}", victory::describe(&g, p, &plan));
    assert!(plan.ours.unwrap().eta <= victory::turns_left(&g));

    // Production: the best city builds the Tower of Mastery (the rules allow it now).
    let best = best_city(&g, p);
    assert!(g.city_can_construct(best, mastery, false, false, false));
    assert_eq!(victory::production(&g, best, false), Some(Choice::Construct(mastery)));
    // ... through the city's whole decision: the garrison, the first army, the workers, settlers
    // and the buildings that unlock better units come first (each is completed at once here),
    // all of them quick, then the Tower of Mastery.
    let (x, y) = (g.cities[best].x, g.cities[best].y);
    let mut chosen = Vec::new();
    for _ in 0..16 {
        g.city_clear_order_queue(best);
        g.controller(p).choose_production(&mut g, best);
        let order = g.cities[best].head_order().expect("an order");
        let turns = g.city_production_turns_left(best);
        if order.order_type == OrderType::ORDER_TRAIN {
            let u = UnitType::from_usize(order.data1 as usize);
            g.player_init_unit(p, u, x, y, Some(UnitAIType::from_usize(order.data2 as usize)), None);
            chosen.push((g.d.units[u].base.type_.clone(), turns));
        } else if order.order_type == OrderType::ORDER_CONSTRUCT && order.data1 != mastery.i() {
            let b = BuildingType::from_usize(order.data1 as usize);
            g.city_set_num_real_building(best, b, 1);
            chosen.push((g.d.buildings[b].base.type_.clone(), turns));
        } else {
            break;
        }
        assert!(turns <= 25, "{} before the Tower of Mastery: {chosen:?}", g.cities[best].name);
    }
    assert_eq!(g.city_production_building(best), Some(mastery), "{} builds {:?} after {:?}", g.cities[best].name, g.cities[best].head_order(), chosen);
    // Other cities do not compete for it (only one may build it).
    for &c in &g.players[p].cities {
        if c != best {
            assert_ne!(victory::production(&g, c, false), Some(Choice::Construct(mastery)));
        }
    }
    // Owning all five is the rules' victory.
    g.city_set_num_real_building(best, mastery, 1);
    let v = g.d.t::<VictoryType>("VICTORY_TOWER_OF_MASTERY");
    assert_eq!(g.test_victory_team(v, team), (true, false));
}

#[test]
fn tower_victory_makes_the_last_tower_tech_the_goal() {
    let mut g = played(11, 80);
    let p = biggest(&g);
    late(&mut g, 400);
    near_tower(&mut g, p, Some("TECH_STRENGTH_OF_WILL"));
    // Strength of Will 60 turns of research away (most of it done): within reach.
    let sow = g.d.t::<TechType>("TECH_STRENGTH_OF_WILL");
    let team = g.players[p].team;
    let rate = g.player_calculate_research_rate(p, None).max(1);
    let cost = g.team_get_research_cost(team, sow);
    g.teams[team].research_progress[sow] = (cost - rate * 60).max(0);
    let plan = victory::plan(&g, p);
    assert_eq!(plan.goal(), Some(Goal::Tower), "{}", victory::describe(&g, p, &plan));
    // The last Tower tech is the victory goal, so research (research.rs) gives it the goal-tech
    // bonus. The exact tech chosen now balances that against the goal-directed economic plan
    // (ai3b-beelines' goal_bonus): with both merged the AI wins MORE full games than victory
    // pursuit alone (13/24 vs 10/24 cultural on seeds 601-624; docs/AI.md), reaching the more
    // achievable cultural victory rather than beelining the rare Tower, so the final pick is
    // judged by that measurement, not asserted here.
    assert_eq!(victory::goal_techs(&g, p, &plan), vec![sow]);
    assert!(g.controller(p).choose_research(&g, p).is_some());
}

#[test]
fn mana_nodes_are_planned_for_towers_without_mana() {
    let mut g = played(11, 80);
    let p = biggest(&g);
    late(&mut g, 400);
    let d = g.d;
    // The techs for all five towers, no tower built, no mana of any kind.
    let mut techs: Vec<TechType> = LESSER.iter().map(|&bc| d.buildings[tower_building(&g, p, bc)].prereq_and_tech.unwrap()).collect();
    techs.push(d.t::<TechType>("TECH_STRENGTH_OF_WILL"));
    give_techs(&mut g, p, &techs);
    let st = victory::tower_state(&g, p).unwrap();
    let lacking = st.mana.len();
    assert!(lacking > 0, "a tower lacks its mana");
    assert!(victory::tower_eta(&g, p).is_none(), "not enough raw nodes: the towers are out of reach");
    assert!(victory::mana_jobs(&g, p).is_empty());
    // Raw mana on free plots of our cities' radius until there is a node per mana the towers
    // lack.
    let raw = d.t::<BonusType>("BONUS_MANA");
    let mut placed = st.raw_nodes.len();
    for c in g.players[p].cities.clone() {
        for i in 1..ffh_core::defs::NUM_CITY_PLOTS {
            if placed >= lacking {
                break;
            }
            let Some(q) = g.city_index_plot(c, i) else { continue };
            let plot = &g.map.plots[q];
            if plot.owner != Some(p) || plot.bonus_type.is_some() || plot.improvement_type.is_some() || plot.feature_type.is_some() || g.plot_is_water(q) || plot.plot_type == PlotType::PLOT_PEAK {
                continue;
            }
            g.plot_set_bonus_type(q, Some(raw));
            placed += 1;
        }
    }
    assert!(placed >= lacking, "{placed} free plots for {lacking} manas");
    let st = victory::tower_state(&g, p).unwrap();
    assert!(st.raw_nodes.len() >= lacking);
    assert!(victory::tower_eta(&g, p).is_some(), "enough raw nodes: the towers are reachable");
    let jobs = victory::mana_jobs(&g, p);
    assert_eq!(jobs.len(), lacking);
    // Each job turns a raw node into a mana a tower needs.
    for (mana, (q, b)) in st.mana.iter().zip(&jobs) {
        let improvement = d.builds[*b].improvement.unwrap();
        assert_eq!(d.improvements[improvement].bonus_convert, Some(*mana));
        assert_eq!(g.map.plots[*q].bonus_type, Some(raw));
        assert!(g.player_can_build(p, *q, *b, false, false));
    }
}

#[test]
fn a_rival_near_the_tower_is_named() {
    let mut g = played(11, 80);
    let p = biggest(&g);
    let q = g.civ_player_ids().find(|&o| o != p && g.players[o].alive && !g.player_is_barbarian(o) && !g.players[o].cities.is_empty()).unwrap();
    late(&mut g, 400);
    near_tower(&mut g, q, None);
    let plan = victory::plan(&g, p);
    let (rival, race) = plan.rival.expect("a rival one building away from the Tower of Mastery");
    assert_eq!(rival, q);
    assert_eq!(race.goal, Goal::Tower);
    assert!(race.eta <= victory::threat_horizon(&g));
}

#[test]
fn culture_near_legendary_is_pursued() {
    let mut g = played(12, 120);
    let p = biggest(&g);
    assert!(g.players[p].cities.len() >= 3, "a civ with three cities");
    late(&mut g, 300);
    // Drama: the culture slider can move.
    let drama = g.d.t::<TechType>("TECH_DRAMA");
    give_techs(&mut g, p, &[drama]);
    assert!(g.player_is_commerce_flexible(p, CommerceType::COMMERCE_CULTURE));
    let legendary = g.culture_threshold(g.d.t::<CultureLevelType>("CULTURELEVEL_LEGENDARY"));
    let cities: Vec<CityId> = g.players[p].cities.iter().copied().take(3).collect();
    for &c in &cities {
        g.city_set_culture(c, p, legendary * 999 / 1000, false, false);
    }
    let plan = victory::plan(&g, p);
    assert_eq!(plan.goal(), Some(Goal::Culture), "{}", victory::describe(&g, p, &plan));
    // The cities nearest legendary make culture (a culture building, a wonder or the process).
    for &c in &cities {
        let choice = victory::production(&g, c, false).expect("culture production");
        match choice {
            Choice::Maintain(pr) => assert_eq!(g.d.processes[pr].base.type_, "PROCESS_CULTURE"),
            Choice::Construct(b) => {
                let info = &g.d.buildings[b];
                let i = CommerceType::COMMERCE_CULTURE.idx();
                assert!(info.commerce_change[i] + info.obsolete_safe_commerce_change[i] > 0 || info.commerce_modifier[i] > 0, "{}", info.base.type_);
            }
            other => panic!("{other:?}"),
        }
    }
    // They keep two more defenders: the other civs will come for them.
    for &c in &cities {
        assert_eq!(victory::guard(&g, c), 2, "{}", g.cities[c].name);
    }
    for &c in &g.players[p].cities {
        if !cities.contains(&c) {
            assert_eq!(victory::guard(&g, c), 0);
        }
    }
    // The empire puts commerce into culture, and the culture slider is not taken for a
    // shortfall (research and culture count together: nothing is disbanded for it).
    assert!(victory::culture_slider(&g, p) > 0);
    ffh_ai::empire::manage(&mut g, p);
    let culture = g.players[p].commerce_percent[CommerceType::COMMERCE_CULTURE.idx()];
    assert!(culture > 0);
    assert_eq!(ffh_ai::economy::spend_pct(&g, p), g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] + culture);
    // Three legendary cities are the rules' victory.
    for &c in &cities {
        g.city_set_culture(c, p, legendary, false, false);
    }
    let v = g.d.t::<VictoryType>("VICTORY_CULTURAL");
    assert_eq!(g.test_victory_team(v, g.players[p].team), (true, false));
}

#[test]
fn nothing_is_pursued_early_or_out_of_reach() {
    let g = played(11, 80);
    for p in g.civ_player_ids() {
        assert_eq!(victory::plan(&g, p), victory::Plan::default(), "no plan at turn 80");
    }
}
