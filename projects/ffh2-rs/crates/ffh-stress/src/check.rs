//! Cross-checks of the redundant bookkeeping the rules keep (C++ caches and counters that
//! must agree with the objects they count). A disagreement means a port bug: the C++ updates
//! all of them together.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::Idx;

/// Panics with `invariant: ...` on the first disagreement.
pub fn check(g: &Game) {
    let fail = |msg: String| -> ! { panic!("invariant: {msg} (turn {})", g.game_turn()) };

    // Cities: owner lists, plots, population, team and area counters.
    let mut team_cities = vec![0i32; g.num_teams()];
    let mut team_pop = vec![0i32; g.num_teams()];
    for p in g.player_ids() {
        let pl = &g.players[p];
        let mut pop = 0;
        for &c in &pl.cities {
            if !g.cities.contains(c) {
                fail(format!("player {} lists dead city {c:?}", p.0));
            }
            let city = &g.cities[c];
            if city.owner != p {
                fail(format!("player {} lists city {} owned by {}", p.0, city.name, city.owner.0));
            }
            let plot = g.city_plot(c);
            if g.map.plots[plot].plot_city != Some(c) {
                fail(format!("city {} of player {} is not on its plot", city.name, p.0));
            }
            pop += city.population;
        }
        if pop != pl.total_population {
            fail(format!("player {} total_population {} != sum of city populations {pop}", p.0, pl.total_population));
        }
        team_cities[pl.team.idx()] += pl.cities.len() as i32;
        team_pop[pl.team.idx()] += pop;
    }
    if g.cities.len() != g.player_ids().map(|p| g.players[p].cities.len()).sum::<usize>() {
        fail(format!("{} cities exist but players list {}", g.cities.len(), g.player_ids().map(|p| g.players[p].cities.len()).sum::<usize>()));
    }
    for t in g.team_ids() {
        if g.teams[t].num_cities != team_cities[t.idx()] {
            fail(format!("team {} num_cities {} != its players' cities {}", t.0, g.teams[t].num_cities, team_cities[t.idx()]));
        }
        if g.teams[t].total_population != team_pop[t.idx()] {
            fail(format!("team {} total_population {} != its players' {}", t.0, g.teams[t].total_population, team_pop[t.idx()]));
        }
    }
    for a in g.map.areas.ids() {
        let area = &g.map.areas[a];
        let mut n = 0;
        let mut per_player = vec![0i32; g.num_players()];
        let mut pop_per_player = vec![0i32; g.num_players()];
        for (c, city) in g.cities.iter() {
            if g.city_area(c) == a {
                n += 1;
                per_player[city.owner.idx()] += 1;
                pop_per_player[city.owner.idx()] += city.population;
            }
        }
        if area.num_cities != n {
            fail(format!("area {a:?} num_cities {} != {n}", area.num_cities));
        }
        for p in g.player_ids() {
            if area.cities_per_player[p] != per_player[p.idx()] {
                fail(format!("area {a:?} cities_per_player[{}] {} != {}", p.0, area.cities_per_player[p], per_player[p.idx()]));
            }
            if area.population_per_player[p] != pop_per_player[p.idx()] {
                fail(format!("area {a:?} population_per_player[{}] {} != {}", p.0, area.population_per_player[p], pop_per_player[p.idx()]));
            }
        }
    }

    // Units: owner lists, plot lists, groups.
    let mut listed = 0;
    for p in g.player_ids() {
        for &u in &g.players[p].units {
            listed += 1;
            if !g.units.contains(u) {
                fail(format!("player {} lists dead unit {u:?}", p.0));
            }
            let unit = &g.units[u];
            if unit.owner != p {
                fail(format!("player {} lists unit {u:?} owned by {}", p.0, unit.owner.0));
            }
            if let Some(plot) = g.map.plot_xy(unit.x, unit.y) {
                if !g.map.plots[plot].units.contains(&u) {
                    fail(format!("unit {u:?} of player {} at ({}, {}) is not in its plot's list", p.0, unit.x, unit.y));
                }
            }
            match unit.group {
                Some(grp) if g.groups.contains(grp) => {
                    if !g.groups[grp].units.contains(&u) {
                        fail(format!("unit {u:?} of player {} is not in its group's list", p.0));
                    }
                    if g.groups[grp].owner != p {
                        fail(format!("unit {u:?} of player {} is in a group of player {}", p.0, g.groups[grp].owner.0));
                    }
                }
                Some(grp) => fail(format!("unit {u:?} of player {} is in dead group {grp:?}", p.0)),
                None => {}
            }
            if let Some(t) = unit.transport_unit {
                if !g.units.contains(t) {
                    fail(format!("unit {u:?} of player {} is carried by dead unit {t:?}", p.0));
                }
            }
        }
    }
    if listed != g.units.len() {
        fail(format!("{} units exist but players list {listed}", g.units.len()));
    }
    for plot in 0..g.map.plots.len() {
        let pl = &g.map.plots[PlotIdx::from_usize(plot)];
        for &u in &pl.units {
            if !g.units.contains(u) {
                fail(format!("plot ({}, {}) lists dead unit {u:?}", pl.x, pl.y));
            }
            if (g.units[u].x, g.units[u].y) != (pl.x, pl.y) {
                fail(format!("plot ({}, {}) lists unit {u:?} at ({}, {})", pl.x, pl.y, g.units[u].x, g.units[u].y));
            }
        }
    }
    for (grp, group) in g.groups.iter() {
        for &u in &group.units {
            if !g.units.contains(u) {
                fail(format!("group {grp:?} lists dead unit {u:?}"));
            }
            if g.units[u].group != Some(grp) {
                fail(format!("group {grp:?} lists unit {u:?} of group {:?}", g.units[u].group));
            }
        }
    }
}
