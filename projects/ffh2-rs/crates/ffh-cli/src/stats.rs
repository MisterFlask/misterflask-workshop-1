//! `ffh stats`: plays one AI game and prints one JSON object with per-civ snapshots at chosen
//! turns plus whole-game events (city captures, razes, wars, eliminations, timings). Used by
//! `crates/ffh-cli/scripts/ai_metrics.py` to compare AI versions over several seeds, and as the
//! dataset for designing the game score (every per-civ quantity a score formula might weigh).
//!
//! Output (one line): `seed civs world turns ... outcome`, `snapshots: {"<turns played>": [civ,
//! ..]}` (one row per ever-alive player, barbarians included with `barb: true`), `world:
//! {"<turns played>": {..}}` (game-wide values at the same snapshots), `eliminated_at: {"<player>":
//! <turns played>}` and `final_turn` (the last snapshot, always taken when the game stops).
//! Field meanings are listed in docs/AI.md ("Tools"). `score_version: 2`: a civ's `score` is score
//! v2, its share of the world in basis points (docs/SCORE.md); FFH2's score is `legacy_score`
//! (output without `score_version` predates v2: there `score` is FFH2's).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::utils::is_world_unit_class;
use ffh_data::*;

use crate::run::{self, RunConfig};

#[derive(Default)]
struct Events {
    /// Cities taken by a civ from another civ.
    captured_civ: i32,
    /// Cities taken by a civ from the barbarians.
    captured_from_barb: i32,
    /// Cities taken by the barbarians.
    captured_by_barb: i32,
    /// Cities that disappeared (razed or destroyed).
    destroyed: i32,
    founded: i32,
    wars_declared: i32,
    peace_made: i32,
}

/// Per-player cumulative city events, compared turn by turn (a city captured and recaptured
/// within one turn is not seen).
#[derive(Default, Clone)]
struct CivEvents {
    /// Cities this player took from another civ (owner changed to us at the end of a turn).
    taken_civ: i32,
    /// Cities this player took from the barbarians.
    taken_barb: i32,
    /// Cities that vanished during a turn with this player's units standing on the ruins
    /// (captured and razed; any former owner, barbarians included).
    razed: i32,
    /// Cities this player lost to another civ (taken, or razed with that civ's units on the plot).
    lost_civ: i32,
    /// Cities this player lost to the barbarians (taken or razed).
    lost_barb: i32,
    /// Cities of this player that vanished with no foreign unit on the plot.
    lost_other: i32,
    /// Cities this player founded.
    founded: i32,
    /// Turns played when the player was found dead (None while alive).
    elim_turn: Option<i32>,
}

fn city_owners(g: &Game) -> BTreeMap<PlotIdx, PlayerId> {
    let mut m = BTreeMap::new();
    for p in g.player_ids() {
        for &c in &g.players[p].cities {
            m.insert(g.city_plot(c), p);
        }
    }
    m
}

fn civ_war_pairs(g: &Game) -> Vec<(TeamId, TeamId)> {
    let barb = g.barbarian_team();
    let mut out = Vec::new();
    for a in g.teams.keys() {
        for b in g.teams.keys() {
            if a < b && a != barb && b != barb && g.teams[a].alive_count > 0 && g.teams[b].alive_count > 0 && g.at_war(a, b) {
                out.push((a, b));
            }
        }
    }
    out
}

/// Minimal JSON object writer (std only).
struct Obj(String);

impl Obj {
    fn new() -> Obj {
        Obj(String::from("{"))
    }
    fn key(&mut self, k: &str) {
        if self.0.len() > 1 {
            self.0.push(',');
        }
        let _ = write!(self.0, "\"{k}\":");
    }
    fn int(&mut self, k: &str, v: impl Into<i64>) -> &mut Obj {
        self.key(k);
        let _ = write!(self.0, "{}", v.into());
        self
    }
    fn num(&mut self, k: &str, v: f64) -> &mut Obj {
        self.key(k);
        let _ = write!(self.0, "{:.1}", v);
        self
    }
    fn boolean(&mut self, k: &str, v: bool) -> &mut Obj {
        self.key(k);
        let _ = write!(self.0, "{v}");
        self
    }
    fn str(&mut self, k: &str, v: &str) -> &mut Obj {
        self.key(k);
        let _ = write!(self.0, "\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""));
        self
    }
    fn opt_int(&mut self, k: &str, v: Option<i32>) -> &mut Obj {
        match v {
            Some(v) => self.int(k, v),
            None => self.raw(k, "null"),
        }
    }
    fn raw(&mut self, k: &str, json: &str) -> &mut Obj {
        self.key(k);
        self.0.push_str(json);
        self
    }
    fn done(&mut self) -> String {
        self.0.push('}');
        std::mem::take(&mut self.0)
    }
}

/// Combat power of one unit: max(attack, defence) base strength (with damage-type strength) x
/// (100 + promotion combat percent) / 100 x current hit-point fraction.
fn unit_power(g: &Game, u: UnitId) -> f64 {
    let base = g.unit_base_combat_str(u).max(g.unit_base_combat_str_defense(u)) as f64;
    let pct = (100 + g.units[u].extra_combat_percent).max(0) as f64 / 100.0;
    let hp = g.unit_curr_hit_points(u).max(0) as f64 / g.unit_max_hit_points(u).max(1) as f64;
    base * pct * hp
}

fn short<'a>(name: &'a str, prefix: &str) -> &'a str {
    name.strip_prefix(prefix).unwrap_or(name)
}

fn snapshot(g: &Game, civ_events: &BTreeMap<PlayerId, CivEvents>) -> String {
    let d = g.d;
    let hero = d.ty::<PromotionType>("PROMOTION_HERO");
    let people = d.ty::<SpecialUnitType>("SPECIALUNIT_PEOPLE");
    let adept = d.ty::<UnitCombatType>("UNITCOMBAT_ADEPT");
    let disciple = d.ty::<UnitCombatType>("UNITCOMBAT_DISCIPLE");
    let legendary = d.ty::<CultureLevelType>("CULTURELEVEL_LEGENDARY");
    let barb_team = g.barbarian_team();
    // Score v2 (share of the world, basis points) and its three shares (parts per million).
    let scores = g.world_scores();
    let shares = g.world_score_shares();
    let mut parts = Vec::new();
    for p in g.player_ids() {
        let pl = &g.players[p];
        if !pl.ever_alive {
            continue;
        }
        let barbarian = g.player_is_barbarian(p);
        let civ = g.player_get_civilization_type(p).map_or("?".to_string(), |c| d.civilizations[c].base.type_.trim_start_matches("CIVILIZATION_").to_string());
        let leader = g.player_get_leader_type(p).map_or("?".to_string(), |l| short(&d.leader_heads[l].base.type_, "LEADER_").to_string());
        let team = pl.team;
        let known: Vec<TechType> = d.techs.keys().filter(|&t| g.teams[team].has_tech[t]).collect();
        let techs = known.len();
        let tech_cost: i64 = known.iter().map(|&t| g.team_get_research_cost(team, t) as i64).sum();
        let tech_progress: i64 = d.techs.keys().filter(|&t| !g.teams[team].has_tech[t]).map(|t| g.teams[team].research_progress[t] as i64).sum();

        // Units.
        let mut mil = 0;
        let mut workers = 0;
        let mut settlers = 0;
        let mut animals = 0;
        let mut heroes = 0;
        let mut world_units = 0;
        let mut gp_units = 0;
        let mut summons = 0;
        let mut casters = 0;
        let mut levels = 0i64;
        let mut xp = 0i64;
        let mut mil_str = 0.0;
        let mut mil_best: f64 = 0.0;
        let mut naval_str = 0.0;
        let mut ucombat: BTreeMap<&str, i32> = BTreeMap::new();
        for &u in &pl.units {
            let unit = &g.units[u];
            let info = &d.units[unit.unit_type];
            if info.animal {
                animals += 1;
            }
            if info.found {
                settlers += 1;
            } else if info.builds.iter().any(|&b| b) && info.combat == 0 {
                workers += 1;
            } else if info.combat > 0 {
                mil += 1;
            }
            if hero.is_some_and(|h| unit.has_promotion[h]) {
                heroes += 1;
            }
            if info.unit_class_type.is_some_and(|c| is_world_unit_class(d, c)) {
                world_units += 1;
            }
            if people.is_some() && info.special_unit_type == people {
                gp_units += 1;
            }
            if unit.summoner.is_some() || unit.duration > 0 {
                summons += 1;
            }
            let uc = info.unit_combat_type;
            if uc.is_some() && (uc == adept || uc == disciple) {
                casters += 1;
            }
            *ucombat.entry(uc.map_or("NONE", |c| short(&d.unit_combats[c].base.type_, "UNITCOMBAT_"))).or_insert(0) += 1;
            if g.unit_can_fight(u) {
                let power = unit_power(g, u);
                levels += unit.level as i64;
                xp += unit.experience as i64;
                if g.unit_get_domain_type(u) == DomainType::DOMAIN_SEA {
                    naval_str += power;
                } else {
                    mil_str += power;
                    mil_best = mil_best.max(power);
                }
            }
        }
        let ucombat_json = {
            let mut o = Obj::new();
            for (k, v) in &ucombat {
                o.int(k, *v);
            }
            o.done()
        };

        // Cities.
        let mut improvements = 0;
        let mut food = 0;
        let mut food_surplus = 0;
        let mut yield_commerce = 0;
        let mut wonders = 0;
        let mut team_wonders = 0;
        let mut nat_wonders = 0;
        let mut buildings = 0;
        let mut gp_settled = 0;
        let mut happy_margin = 0;
        let mut min_happy_margin: Option<i32> = None;
        let mut unhappy_cities = 0;
        let mut angry_pop = 0;
        let mut disorder_cities = 0;
        let mut health_margin = 0;
        let mut min_health_margin: Option<i32> = None;
        let mut unhealthy_cities = 0;
        let mut holy_cities = 0;
        let mut legendary_cities = 0;
        let mut max_culture_level = -1;
        for &c in &pl.cities {
            for i in 0..ffh_core::defs::NUM_CITY_PLOTS {
                if let Some(q) = g.city_index_plot(c, i) {
                    let plot = &g.map.plots[q];
                    if plot.working_city == Some(c) && plot.improvement_type.is_some() && plot.owner == Some(p) {
                        improvements += 1;
                    }
                }
            }
            let city = &g.cities[c];
            food += g.city_yield_rate(c, YieldType::YIELD_FOOD);
            food_surplus += g.city_food_difference(c, true);
            yield_commerce += g.city_yield_rate(c, YieldType::YIELD_COMMERCE);
            wonders += city.num_world_wonders;
            team_wonders += city.num_team_wonders;
            nat_wonders += city.num_national_wonders;
            buildings += city.num_buildings;
            gp_settled += city.num_great_people;
            let happy = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
            happy_margin += happy;
            min_happy_margin = Some(min_happy_margin.map_or(happy, |m| m.min(happy)));
            unhappy_cities += (happy < 0) as i32;
            angry_pop += g.city_angry_population(c, 0);
            disorder_cities += g.city_is_disorder(c) as i32;
            let health = g.city_good_health(c) - g.city_bad_health(c, false, 0);
            health_margin += health;
            min_health_margin = Some(min_health_margin.map_or(health, |m| m.min(health)));
            unhealthy_cities += (health < 0) as i32;
            holy_cities += d.religions.keys().any(|r| g.city_is_holy_city(c, r)) as i32;
            legendary_cities += (city.culture_level.is_some() && legendary.is_some() && city.culture_level >= legendary) as i32;
            max_culture_level = max_culture_level.max(opt_to_i32(city.culture_level));
        }
        let projects: i32 = d.projects.keys().map(|j| g.teams[team].project_count[j]).sum();
        let prod: i32 = pl.cities.iter().map(|&c| g.city_yield_rate(c, YieldType::YIELD_PRODUCTION)).sum();

        let research = g.player_calculate_research_rate(p, None);
        // Geography: the capital's landmass, civs sharing it, our cities elsewhere, ships.
        let home = pl.capital_city.filter(|&c| g.cities.contains(c)).and_then(|c| g.map.plots[g.city_plot(c)].area);
        let area_tiles = home.map_or(0, |a| g.map.areas[a].num_tiles);
        let area_civs = g
            .player_ids()
            .into_iter()
            .filter(|&o| o != p && g.players[o].alive && !g.player_is_barbarian(o))
            .filter(|&o| home.is_some() && g.players[o].cities.iter().any(|&c| g.map.plots[g.city_plot(c)].area == home))
            .count();
        let overseas = pl.cities.iter().filter(|&&c| g.map.plots[g.city_plot(c)].area != home).count();
        let ships = pl.units.iter().filter(|&&u| g.unit_get_domain_type(u) == DomainType::DOMAIN_SEA).count();
        let pct = pl.commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()];
        let maint = if barbarian { 0 } else { g.player_get_total_maintenance(p) };
        let ucost = if barbarian { 0 } else { g.player_calculate_unit_cost(p) + g.player_calculate_unit_supply(p) };
        let civ_upkeep = if barbarian { 0 } else { g.player_get_civic_upkeep(p, None, false) };
        let gold_commerce = g.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD);
        let research_commerce = g.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH);
        let commerce = gold_commerce + research_commerce;
        let (gold_rate, commerce_total) = if barbarian { (0, 0) } else { (g.player_calculate_gold_rate(p), g.player_calculate_total_commerce(p)) };

        // Diplomacy.
        let mut wars = 0;
        let mut met = 0;
        for o in g.teams.keys() {
            if o == team || o == barb_team || g.teams[o].alive_count == 0 {
                continue;
            }
            met += g.teams[team].has_met[o] as i32;
            wars += g.at_war(team, o) as i32;
        }
        let ev = civ_events.get(&p).cloned().unwrap_or_default();

        let mut o = Obj::new();
        o.int("p", p.0)
            .str("civ", &civ)
            .boolean("barb", barbarian)
            .boolean("alive", pl.alive)
            .int("cities", pl.cities.len() as i64)
            .int("pop", pl.total_population)
            .int("techs", techs as i64)
            .int("units", pl.units.len() as i64)
            .int("mil", mil)
            .int("workers", workers)
            .int("settlers", settlers)
            .int("animals", animals)
            .int("impr", improvements)
            .int("research", research)
            .int("prod", prod)
            .int("gold", pl.gold)
            .int("score", if barbarian { 0 } else { scores[p.idx()] })
            .int("pct", pct)
            .int("maint", maint)
            .int("area_tiles", area_tiles)
            .int("area_civs", area_civs as i64)
            .int("overseas", overseas as i64)
            .int("ships", ships as i64)
            .int("ucost", ucost)
            .int("civ_upkeep", civ_upkeep)
            .int("commerce", commerce)
            // Identity.
            .int("team", team.0)
            .str("leader", &leader)
            .str("alignment", &pl.alignment.map_or("-".to_string(), |a| short(&d.alignments[a].base.type_, "ALIGNMENT_").to_string()))
            .int("era", pl.current_era.i())
            .str("religion", &g.player_get_state_religion(p).map_or("-".to_string(), |r| short(&d.religions[r].base.type_, "RELIGION_").to_string()))
            .opt_int("elim_turn", ev.elim_turn)
            // Land and people.
            .int("land", pl.total_land)
            .int("land_scored", pl.total_land_scored)
            // Score v2 (`score`, basis points of the world): its people, land and knowledge
            // shares in parts per million. The legacy (FFH2) score and its raw components.
            .int("score_pop_share", if barbarian { 0 } else { shares[p.idx()][0] })
            .int("score_land_share", if barbarian { 0 } else { shares[p.idx()][1] })
            .int("score_know_share", if barbarian { 0 } else { shares[p.idx()][2] })
            .int("legacy_score", if barbarian { 0 } else { g.player_legacy_score(p) })
            .int("score_pop_raw", if barbarian { 0 } else { g.player_get_pop_score(p, true) })
            .int("score_land_raw", if barbarian { 0 } else { g.player_get_land_score(p, true) })
            .int("score_tech_raw", pl.tech_score)
            .int("score_wonders_raw", pl.wonders_score)
            .int("power", pl.power)
            .int("assets", pl.assets)
            // Technology.
            .int("tech_cost", tech_cost)
            .int("tech_progress", tech_progress)
            .int("research_commerce", research_commerce)
            // Economy.
            .int("food", food)
            .int("food_surplus", food_surplus)
            .int("yield_commerce", yield_commerce)
            .int("commerce_total", commerce_total)
            .int("gold_commerce", gold_commerce)
            .int("gold_rate", gold_rate)
            .int("culture", g.player_count_total_culture(p))
            .int("culture_rate", g.player_get_commerce_rate(p, CommerceType::COMMERCE_CULTURE))
            .int("max_culture_level", max_culture_level)
            .int("legendary_cities", legendary_cities)
            .int("espionage_rate", g.player_get_commerce_rate(p, CommerceType::COMMERCE_ESPIONAGE))
            .int("golden_age_turns", pl.golden_age_turns)
            .int("anarchy_turns", pl.anarchy_turns)
            // Buildings and great people.
            .int("wonders", wonders)
            .int("team_wonders", team_wonders)
            .int("nat_wonders", nat_wonders)
            .int("buildings", buildings)
            .int("projects", projects)
            .int("holy_cities", holy_cities)
            .int("gp_created", pl.great_people_created)
            .int("gg_created", pl.great_generals_created)
            .int("gp_units", gp_units)
            .int("gp_settled", gp_settled)
            // Military.
            .num("mil_str", mil_str)
            .num("mil_best", mil_best)
            .num("naval_str", naval_str)
            .int("heroes", heroes)
            .int("world_units", world_units)
            .int("summons", summons)
            .int("casters", casters)
            .int("levels", levels)
            .int("xp", xp)
            .raw("ucombat", &ucombat_json)
            // Happiness and health.
            .int("happy_margin", happy_margin)
            .opt_int("min_happy_margin", min_happy_margin)
            .int("unhappy_cities", unhappy_cities)
            .int("angry_pop", angry_pop)
            .int("disorder_cities", disorder_cities)
            .int("health_margin", health_margin)
            .opt_int("min_health_margin", min_health_margin)
            .int("unhealthy_cities", unhealthy_cities)
            // Diplomacy and conquest.
            .int("met", met)
            .int("wars", wars)
            .boolean("vassal", !barbarian && g.team_is_a_vassal(team))
            .int("taken_civ", ev.taken_civ)
            .int("taken_barb", ev.taken_barb)
            .int("razed", ev.razed)
            .int("lost_civ", ev.lost_civ)
            .int("lost_barb", ev.lost_barb)
            .int("lost_other", ev.lost_other)
            .int("founded", ev.founded)
            .int("cities_lost_rule", pl.cities_lost)
            .int("players_killed", pl.players_killed);
        // Victory progress (ffh-ai's victory planner; reads the game only).
        if !barbarian && pl.alive {
            use ffh_ai::victory;
            let vs = victory::statics(d);
            let towers: i32 = vs.tower_classes.iter().filter(|&&(bc, n)| g.teams[team].building_class_count[bc] >= n).count() as i32;
            let plan = victory::plan(g, p);
            // The third-best city's culture as a share of legendary (per mille).
            let mut culture: Vec<i64> = pl.cities.iter().map(|&c| g.cities[c].culture[p] as i64 / 100).collect();
            culture.sort_unstable_by(|a, b| b.cmp(a));
            let legend = legendary.map_or(1, |l| g.culture_threshold(l).max(1) as i64);
            o.int("towers", towers)
                .opt_int("tower_eta", victory::tower_eta(g, p))
                .opt_int("culture_eta", victory::culture_eta(g, p))
                .int("legend3_pm", culture.get(2).map_or(0, |&c| c * 1000 / legend))
                .opt_int("domination_eta", victory::domination_eta(g, p))
                .opt_int("conquest_eta", victory::conquest_eta(g, p))
                .int("altar_stage", victory::altar_stage(g, team).map_or(-1, |s| s as i64))
                .int("mana_raw", victory::raw_mana_plots(g, p).len() as i64)
                .int("mana_types", vs.mana_builds.iter().filter(|&&(_, bn)| g.player_has_bonus(p, bn)).count() as i64)
                .int("culture_pct", pl.commerce_percent[CommerceType::COMMERCE_CULTURE.idx()])
                .str("goal", &plan.ours.map_or("-".to_string(), |r| format!("{:?}", r.goal)))
                .opt_int("goal_eta", plan.ours.map(|r| r.eta))
                .opt_int("rival", plan.rival.map(|(q, _)| q.0 as i32));
        }
        parts.push(o.done());
    }
    format!("[{}]", parts.join(","))
}

/// Game-wide values at a snapshot: the score maxima the FFH2 score divides by, the Armageddon
/// counter, religions founded, map size.
fn world_snapshot(g: &Game) -> String {
    let alive_civs = g.player_ids().into_iter().filter(|&p| g.players[p].alive && !g.player_is_barbarian(p)).count();
    let mut o = Obj::new();
    o.int("game_turn", g.game_turn())
        .int("alive_civs", alive_civs as i64)
        .int("war_pairs", civ_war_pairs(g).len() as i64)
        .int("armageddon", g.get_global_counter())
        .int("religions", g.religion_game_turn_founded.iter().filter(|&&t| t != -1).count() as i64)
        .int("land_plots", g.map.land_plots)
        .int("estimate_end_turn", g.estimate_end_turn)
        .int("max_population", g.max_population)
        .int("max_land", g.max_land)
        .int("max_tech", g.max_tech)
        .int("max_wonders", g.max_wonders)
        .int("init_population", g.init_population)
        .int("init_land", g.init_land)
        .int("init_tech", g.init_tech)
        .int("init_wonders", g.init_wonders);
    o.done()
}

pub fn cmd_stats(cfg: &RunConfig, at: &[i32]) {
    let t = Instant::now();
    let (mut g, _timed) = run::new_game(cfg);
    let setup_ms = t.elapsed().as_secs_f64() * 1000.0;
    let mut ev = Events::default();
    let mut civ_ev: BTreeMap<PlayerId, CivEvents> = BTreeMap::new();
    let mut owners = city_owners(&g);
    let mut wars = civ_war_pairs(&g);
    let mut snaps: Vec<(i32, String, String)> = Vec::new();
    let barb = g.barbarian_player();
    let t = Instant::now();
    let played = run::play(&mut g, cfg, |g, stat| {
        // `stat.turn` is the turn just completed; snapshots are labelled with the number of
        // turns played (turn 99 completed -> "100").
        let played = stat.turn + 1;
        let now = city_owners(g);
        for (&plot, &old) in &owners {
            match now.get(&plot) {
                None => {
                    ev.destroyed += 1;
                    // Razed: credit the owner of a foreign unit standing on the ruins.
                    let razer = g.map.plots[plot].units.iter().map(|&u| g.units[u].owner).find(|&o| o != old);
                    match razer {
                        Some(r) => {
                            civ_ev.entry(r).or_default().razed += 1;
                            let e = civ_ev.entry(old).or_default();
                            if r == barb {
                                e.lost_barb += 1;
                            } else {
                                e.lost_civ += 1;
                            }
                        }
                        None => civ_ev.entry(old).or_default().lost_other += 1,
                    }
                }
                Some(&new) if new != old => {
                    if new == barb {
                        ev.captured_by_barb += 1;
                        civ_ev.entry(old).or_default().lost_barb += 1;
                    } else if old == barb {
                        ev.captured_from_barb += 1;
                        civ_ev.entry(new).or_default().taken_barb += 1;
                    } else {
                        ev.captured_civ += 1;
                        civ_ev.entry(new).or_default().taken_civ += 1;
                        civ_ev.entry(old).or_default().lost_civ += 1;
                    }
                }
                _ => {}
            }
        }
        for (plot, &p) in &now {
            if !owners.contains_key(plot) {
                ev.founded += 1;
                civ_ev.entry(p).or_default().founded += 1;
            }
        }
        owners = now;
        for p in g.player_ids() {
            if g.players[p].ever_alive && !g.players[p].alive {
                let e = civ_ev.entry(p).or_default();
                if e.elim_turn.is_none() {
                    e.elim_turn = Some(played);
                }
            }
        }
        let w = civ_war_pairs(g);
        ev.wars_declared += w.iter().filter(|x| !wars.contains(x)).count() as i32;
        ev.peace_made += wars.iter().filter(|x| !w.contains(x)).count() as i32;
        wars = w;
        if at.contains(&played) {
            snaps.push((played, snapshot(g, &civ_ev), world_snapshot(g)));
        }
    });
    let stats = played.stats;
    // Always snapshot the final state (a game decided between two `--at` turns, or the cap).
    let final_turn = stats.last().map_or(0, |s| s.turn + 1);
    if snaps.last().map_or(true, |s| s.0 != final_turn) {
        snaps.push((final_turn, snapshot(&g, &civ_ev), world_snapshot(&g)));
    }
    let outcome_json = match played.outcome {
        run::Outcome::TurnLimit => "{\"kind\":\"turn_limit\"}".to_string(),
        run::Outcome::Victory { turn, winner, victory } => format!(
            "{{\"kind\":\"victory\",\"turn\":{turn},\"team\":{},\"victory\":\"{}\"}}",
            winner.map_or(-1, |t| t.0 as i64),
            victory.map_or("time".to_string(), |v| g.d.victories[v].base.type_.clone())
        ),
        run::Outcome::LastStanding { turn, team } => format!("{{\"kind\":\"last_standing\",\"turn\":{turn},\"team\":{}}}", team.0),
        run::Outcome::Supremacy { turn, team, since } => {
            format!("{{\"kind\":\"supremacy\",\"turn\":{turn},\"team\":{},\"since\":{since}}}", team.0)
        }
    };
    let play_s = t.elapsed().as_secs_f64();
    let n = stats.len().max(1) as f64;
    let slowest = stats.iter().map(|s| s.elapsed.as_secs_f64() * 1000.0).fold(0.0, f64::max);
    let eliminated = g.player_ids().into_iter().filter(|&p| g.players[p].ever_alive && !g.players[p].alive && p != barb).count();
    let snaps_json: Vec<String> = snaps.iter().map(|(t, s, _)| format!("\"{t}\":{s}")).collect();
    let world_json: Vec<String> = snaps.iter().map(|(t, _, w)| format!("\"{t}\":{w}")).collect();
    let elim_json: Vec<String> = civ_ev.iter().filter_map(|(p, e)| e.elim_turn.map(|t| format!("\"{}\":{t}", p.0))).collect();
    println!(
        "{{\"score_version\":2,\"seed\":{},\"civs\":{},\"world\":\"{}\",\"turns\":{},\"setup_ms\":{:.1},\"ms_per_turn\":{:.2},\"slowest_ms\":{:.1},\"checksum\":\"{:08x}\",\"captured_civ\":{},\"captured_from_barb\":{},\"captured_by_barb\":{},\"destroyed\":{},\"founded\":{},\"wars_declared\":{},\"peace_made\":{},\"eliminated\":{},\"outcome\":{},\"final_turn\":{},\"eliminated_at\":{{{}}},\"snapshots\":{{{}}},\"world_at\":{{{}}}}}",
        cfg.seed,
        cfg.civs,
        cfg.world,
        stats.len(),
        setup_ms,
        play_s * 1000.0 / n,
        slowest,
        stats.last().map_or(0, |s| s.checksum) as u32,
        ev.captured_civ,
        ev.captured_from_barb,
        ev.captured_by_barb,
        ev.destroyed,
        ev.founded,
        ev.wars_declared,
        ev.peace_made,
        eliminated,
        outcome_json,
        final_turn,
        elim_json.join(","),
        snaps_json.join(","),
        world_json.join(",")
    );
}
