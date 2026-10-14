//! `ffh stats`: plays one AI game and prints one JSON object with per-civ snapshots at chosen
//! turns plus whole-game events (city captures, razes, wars, eliminations, timings). Used by
//! `crates/ffh-cli/scripts/ai_metrics.py` to compare AI versions over several seeds.

use std::collections::BTreeMap;
use std::time::Instant;

use ffh_core::game::Game;
use ffh_core::ids::*;
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

fn snapshot(g: &Game) -> String {
    let d = g.d;
    let mut parts = Vec::new();
    for p in g.player_ids() {
        let pl = &g.players[p];
        if !pl.ever_alive {
            continue;
        }
        let barbarian = g.player_is_barbarian(p);
        let civ = g.player_get_civilization_type(p).map_or("?".to_string(), |c| d.civilizations[c].base.type_.trim_start_matches("CIVILIZATION_").to_string());
        let techs = g.teams[pl.team].has_tech.iter().filter(|&&b| b).count();
        let mut mil = 0;
        let mut workers = 0;
        let mut settlers = 0;
        let mut animals = 0;
        for &u in &pl.units {
            let info = &d.units[g.units[u].unit_type];
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
        }
        let mut improvements = 0;
        for &c in &pl.cities {
            for i in 0..ffh_core::defs::NUM_CITY_PLOTS {
                if let Some(q) = g.city_index_plot(c, i) {
                    let plot = &g.map.plots[q];
                    if plot.working_city == Some(c) && plot.improvement_type.is_some() && plot.owner == Some(p) {
                        improvements += 1;
                    }
                }
            }
        }
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
        let prod: i32 = pl.cities.iter().map(|&c| g.city_yield_rate(c, YieldType::YIELD_PRODUCTION)).sum();
        parts.push(format!(
            "{{\"p\":{},\"civ\":\"{}\",\"barb\":{},\"alive\":{},\"cities\":{},\"pop\":{},\"techs\":{},\"units\":{},\"mil\":{},\"workers\":{},\"settlers\":{},\"animals\":{},\"impr\":{},\"research\":{},\"prod\":{},\"gold\":{},\"score\":{},\"pct\":{},\"maint\":{},\"area_tiles\":{},\"area_civs\":{},\"overseas\":{},\"ships\":{}}}",
            p.0,
            civ,
            barbarian,
            pl.alive,
            pl.cities.len(),
            pl.total_population,
            techs,
            pl.units.len(),
            mil,
            workers,
            settlers,
            animals,
            improvements,
            research,
            prod,
            pl.gold,
            if barbarian { 0 } else { g.player_calculate_score(p, false, false) },
            pct,
            maint,
            area_tiles,
            area_civs,
            overseas,
            ships
        ));
    }
    format!("[{}]", parts.join(","))
}

pub fn cmd_stats(cfg: &RunConfig, at: &[i32]) {
    let t = Instant::now();
    let (mut g, _timed) = run::new_game(cfg);
    let setup_ms = t.elapsed().as_secs_f64() * 1000.0;
    let mut ev = Events::default();
    let mut owners = city_owners(&g);
    let mut wars = civ_war_pairs(&g);
    let mut snaps: Vec<(i32, String)> = Vec::new();
    let barb = g.barbarian_player();
    let t = Instant::now();
    let stats = run::autoplay_with(&mut g, cfg, |g, stat| {
        let now = city_owners(g);
        for (plot, &old) in &owners {
            match now.get(plot) {
                None => ev.destroyed += 1,
                Some(&new) if new != old => {
                    if new == barb {
                        ev.captured_by_barb += 1;
                    } else if old == barb {
                        ev.captured_from_barb += 1;
                    } else {
                        ev.captured_civ += 1;
                    }
                }
                _ => {}
            }
        }
        ev.founded += now.keys().filter(|k| !owners.contains_key(k)).count() as i32;
        owners = now;
        let w = civ_war_pairs(g);
        ev.wars_declared += w.iter().filter(|x| !wars.contains(x)).count() as i32;
        ev.peace_made += wars.iter().filter(|x| !w.contains(x)).count() as i32;
        wars = w;
        // `stat.turn` is the turn just completed; snapshots are labelled with the number of
        // turns played (turn 99 completed -> "100").
        let played = stat.turn + 1;
        if at.contains(&played) {
            snaps.push((played, snapshot(g)));
        }
    });
    let play_s = t.elapsed().as_secs_f64();
    let n = stats.len().max(1) as f64;
    let slowest = stats.iter().map(|s| s.elapsed.as_secs_f64() * 1000.0).fold(0.0, f64::max);
    let eliminated = g.player_ids().into_iter().filter(|&p| g.players[p].ever_alive && !g.players[p].alive && p != barb).count();
    let snaps_json: Vec<String> = snaps.iter().map(|(t, s)| format!("\"{t}\":{s}")).collect();
    println!(
        "{{\"seed\":{},\"civs\":{},\"world\":\"{}\",\"turns\":{},\"setup_ms\":{:.1},\"ms_per_turn\":{:.2},\"slowest_ms\":{:.1},\"checksum\":\"{:08x}\",\"captured_civ\":{},\"captured_from_barb\":{},\"captured_by_barb\":{},\"destroyed\":{},\"founded\":{},\"wars_declared\":{},\"peace_made\":{},\"eliminated\":{},\"snapshots\":{{{}}}}}",
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
        snaps_json.join(",")
    );
}
