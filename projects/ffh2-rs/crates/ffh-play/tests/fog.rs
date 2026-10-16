//! T8: fog (design §8). At every control point of a bot game, every unit, city, player and plot
//! the `view` and the `map` show is one the human may know (§4.4), checked against the game's
//! own state on the replies as they go over the wire (serialized and parsed back): revealed and
//! visible plots, no invisible unit, no unrevealed bonus, no unmet player; and nothing the human
//! may know is missing. Targeted cases: an enemy stack hidden in fog, an FfH invisible unit and a
//! hidden-nationality unit next to ours, debug reveal.

mod common;

use common::{bot_session, new_game, peaceful_game};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;
use ffh_play::query::{Layer, Query};
use ffh_play::{bot, Json, Session, Stop};

fn int(j: &Json, k: &str) -> i64 {
    j.get(k).and_then(Json::as_i64).unwrap_or(-1)
}

fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    j.get(k).and_then(Json::as_arr).unwrap_or(&[])
}

/// A reply as a client receives it.
fn wire(s: &Session, q: Query) -> Json {
    let r = s.query(&q).unwrap_or_else(|e| panic!("{q:?}: {e}"));
    Json::parse(&r.to_string()).expect("valid JSON")
}

/// The human may see unit `u` (§4.4).
fn may_see_unit(g: &Game, h: PlayerId, u: UnitId) -> bool {
    let team = g.players[h].team;
    let unit = &g.units[u];
    unit.owner == h || (g.map.plot_xy(unit.x, unit.y).is_some_and(|p| g.plot_is_visible(p, team)) && !g.unit_is_invisible(u, team, true))
}

/// Checks `view` and `map` against the fog rules; returns how many foreign units were shown.
fn check_fog(s: &Session, turn: i32) -> usize {
    let g = &s.g;
    let h = s.human;
    let team = g.players[h].team;
    let met = |p: PlayerId| g.players[p].team == team || g.teams[team].has_met[g.players[p].team];
    let view = wire(s, Query::View);
    assert_eq!(view.get("reveal"), Some(&Json::Bool(false)));
    // Players: met, other than us.
    for p in arr(&view, "players") {
        let p = PlayerId(int(p, "id") as u8);
        assert!(p != h && met(p), "turn {turn}: unmet player {p:?} in the view");
    }
    // Cities: revealed; size and defence only when visible.
    let mut listed_cities = Vec::new();
    for c in arr(&view, "cities") {
        let id = CityId(int(c, "id") as u32);
        listed_cities.push(id);
        let city = &g.cities[id];
        assert!(city.owner == h || city.revealed[team], "turn {turn}: unrevealed city {id:?}");
        assert_eq!(int(c, "owner"), city.owner.0 as i64);
        let visible = city.owner == h || g.plot_is_visible(g.city_plot(id), team);
        assert_eq!(c.get("size").is_some_and(|v| !v.is_null()), visible, "turn {turn}: city {id:?} size shown while fogged");
        assert_eq!(c.get("production").is_some(), city.owner == h, "turn {turn}: city {id:?} production of a foreign city");
    }
    for (id, city) in g.cities.iter() {
        if city.owner == h || city.revealed[team] {
            assert!(listed_cities.contains(&id), "turn {turn}: revealed city {id:?} missing");
        }
    }
    // Units: ours, or visible and not invisible to us; owners as we see them.
    let mut shown = Vec::new();
    for u in arr(&view, "units") {
        let id = UnitId(int(u, "id") as u32);
        shown.push(id);
        assert!(may_see_unit(g, h, id), "turn {turn}: unit {id:?} of player {:?} is hidden from us", g.units[id].owner);
        if g.units[id].owner != h {
            assert_eq!(int(u, "owner"), g.unit_get_visual_owner(id, Some(team)).0 as i64, "turn {turn}: unit {id:?} shows its true owner");
            assert!(u.get("group").is_none() && u.get("moves").is_none(), "turn {turn}: foreign unit {id:?} shows its orders");
        }
    }
    let mut foreign = 0;
    for (id, unit) in g.units.iter() {
        if may_see_unit(g, h, id) {
            assert!(shown.contains(&id), "turn {turn}: visible unit {id:?} missing");
            foreign += (unit.owner != h) as usize;
        }
    }
    // Map: per plot, vis 0/1/2 as revealed/visible; nothing on unrevealed plots; bonuses the
    // team's techs reveal; owners, improvements and routes as the team last saw them.
    let map = wire(s, Query::Map { since: None, epoch: None });
    let col = |k: &str| -> Vec<i64> { arr(&map, k).iter().map(|v| v.as_i64().unwrap()).collect() };
    let (vis, bonus, owner, imp, route, city, terrain) = (col("vis"), col("bonus"), col("owner"), col("improvement"), col("route"), col("city"), col("terrain"));
    assert_eq!(vis.len(), g.map.num_plots());
    let opt = |v: Option<usize>| v.map_or(-1, |i| i as i64);
    for p in g.map.plot_ids() {
        let i = p.0 as usize;
        let pl = &g.map.plots[p];
        let revealed = pl.revealed[team];
        let visible = g.plot_is_visible(p, team);
        assert_eq!(vis[i], if !revealed { 0 } else if visible { 2 } else { 1 }, "turn {turn}: plot {i}");
        if !revealed {
            for (k, c) in [("bonus", &bonus), ("owner", &owner), ("improvement", &imp), ("route", &route), ("city", &city), ("terrain", &terrain)] {
                assert_eq!(c[i], -1, "turn {turn}: unrevealed plot {i} shows its {k}");
            }
            continue;
        }
        assert_eq!(bonus[i], opt(g.plot_get_bonus_type(p, Some(team)).map(|b| b.idx())), "turn {turn}: plot {i} bonus");
        assert_eq!(owner[i], pl.revealed_owner[team].map_or(-1, |o| o.0 as i64), "turn {turn}: plot {i} owner");
        assert_eq!(imp[i], opt(pl.revealed_improvement_type[team].map(|t| t.idx())), "turn {turn}: plot {i} improvement");
        assert_eq!(route[i], opt(pl.revealed_route_type[team].map(|t| t.idx())), "turn {turn}: plot {i} route");
        if city[i] >= 0 {
            let c = CityId(city[i] as u32);
            assert!(g.cities[c].owner == h || g.cities[c].revealed[team], "turn {turn}: unrevealed city on plot {i}");
        }
    }
    // The ASCII units layer marks exactly the plots with units we see.
    let ascii = wire(s, Query::Ascii { x: 0, y: 0, w: 0, h: 0, layer: Layer::Units });
    let rows: Vec<&str> = ascii.get("map").and_then(Json::as_str).unwrap().lines().skip(2).collect();
    let (w, ht) = (g.map.grid_width, g.map.grid_height);
    for (r, line) in rows.iter().enumerate() {
        let y = ht - 1 - r as i32;
        let cells: Vec<char> = line.chars().skip(4).collect();
        for x in 0..w {
            let p = g.map.plot_xy(x, y).unwrap();
            let seen = g.map.plots[p].units.iter().any(|&u| may_see_unit(g, h, u) && g.plot_is_visible(p, team));
            let c = cells[(2 * x) as usize];
            if !g.map.plots[p].revealed[team] {
                assert_eq!(c, ':');
            } else {
                assert_eq!(c != '.', seen, "turn {turn}: ascii units at ({x}, {y})");
            }
        }
    }
    foreign
}

/// T8 over a bot game: the fog rules hold at every control point of a long, varied game. The game
/// is built with [`peaceful_game`] (`GAMEOPTION_ALWAYS_PEACE` + `GAMEOPTION_NO_BARBARIANS`) so no
/// AI can declare war on or eliminate the human and no victory can fire inside 120 turns -- the
/// bot then runs the full length and reaches the until-turn with `Stop::Human` for ANY seed and
/// under any AI/content change. We assert only invariants and coverage: the fog rules at every
/// control point (`check_fog`), a full run's worth of control points, and that foreign units were
/// seen (peaceful civs are still met and their units visible) -- never a seed-specific outcome.
#[test]
fn t8_views_keep_the_fog() {
    let ng = peaceful_game(22);
    let (mut s, mut b) = bot_session(&ng, None);
    let until = s.g.game_turn() + 120;
    let mut foreign = 0;
    let mut points = 0;
    let stop = bot::run(&mut s, &mut b, until, |s, _| {
        foreign += check_fog(s, s.g.game_turn());
        points += 1;
    });
    // True by construction: an always-peace, barbarian-free game cannot end before the until-turn.
    assert_eq!(stop, Stop::Human);
    assert!(points >= 100 && foreign > 0, "{points} control points, {foreign} foreign units seen");
}

/// An AI unit of `t` at `(x, y)` (rules API).
fn ai_unit(s: &mut Session, p: PlayerId, t: UnitType, x: i32, y: i32) -> UnitId {
    s.g.player_init_unit(p, t, x, y, None, None)
}

/// A land plot around `(x, y)` within `r` that `ok` accepts.
fn land_near(g: &Game, x: i32, y: i32, r: i32, ok: impl Fn(PlotIdx) -> bool) -> Option<PlotIdx> {
    for d in 1..=r {
        for dy in -d..=d {
            for dx in -d..=d {
                if let Some(p) = g.plot_xy(x, y, dx, dy) {
                    if !g.plot_is_water(p) && !g.plot_is_impassable(p) && g.map.plots[p].plot_city.is_none() && ok(p) {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

/// The targeted cases: an enemy stack hidden in fog, an invisible unit and a hidden-nationality
/// unit next to ours; then debug reveal shows them, marked.
#[test]
fn hidden_units_stay_hidden() {
    let ng = new_game(5);
    let (mut s, mut b) = bot_session(&ng, None);
    let until = s.g.game_turn() + 25;
    assert_eq!(bot::run(&mut s, &mut b, until, |_, _| {}), Stop::Human);
    let d = s.g.d;
    let h = s.human;
    let team = s.g.players[h].team;
    let ai = s.g.civ_player_ids().find(|&p| p != h && s.g.players[p].alive).unwrap();
    let warrior = d.t::<UnitType>("UNIT_WARRIOR");
    let ours = s.g.players[h].units.iter().copied().find(|&u| d.units[s.g.units[u].unit_type].combat > 0).expect("a combat unit");
    let (ox, oy) = (s.g.units[ours].x, s.g.units[ours].y);

    // A stack of three in fog: revealed, not visible.
    let g = &s.g;
    let fog = g.map.plot_ids().find(|&p| {
        let pl = &g.map.plots[p];
        pl.revealed[team] && !g.plot_is_visible(p, team) && !g.plot_is_water(p) && !g.plot_is_impassable(p) && pl.units.is_empty() && pl.plot_city.is_none()
    });
    let fog = fog.expect("a fogged land plot");
    let (fx, fy) = s.g.map.xy(fog);
    let stack: Vec<UnitId> = (0..3).map(|_| ai_unit(&mut s, ai, warrior, fx, fy)).collect();

    // An invisible unit next to ours: an AI warrior made invisible to land units, as FfH's
    // hiding promotions do (`Unit::invisible_type`), on a plot we see that our units do not see
    // into for that invisible type.
    let land_invisible = d.t::<InvisibleType>("INVISIBLE_LAND");
    let g = &s.g;
    let next = land_near(g, ox, oy, 2, |p| {
        g.plot_is_visible(p, team) && g.map.plots[p].units.is_empty() && g.map.plots[p].owner.is_none() && g.map.plots[p].invisible_visibility_count[team][land_invisible] == 0
    })
    .expect("a free plot near our unit");
    let (nx, ny) = s.g.map.xy(next);
    let hidden = ai_unit(&mut s, ai, warrior, nx, ny);
    s.g.units[hidden].invisible_type = Some(land_invisible);
    assert!(s.g.unit_is_invisible(hidden, team, true), "the test needs a unit we cannot see");

    // A hidden-nationality warrior next to ours (outside its owner's cities): shown as a
    // barbarian.
    let g = &s.g;
    let next2 = land_near(g, ox, oy, 2, |p| p != next && g.plot_is_visible(p, team) && g.map.plots[p].units.is_empty()).expect("another free plot");
    let (hx, hy) = s.g.map.xy(next2);
    let masked = ai_unit(&mut s, ai, warrior, hx, hy);
    s.g.units[masked].hidden_nationality = 1;

    check_fog(&s, s.g.game_turn());
    let view = wire(&s, Query::View);
    let shown: Vec<i64> = arr(&view, "units").iter().map(|u| int(u, "id")).collect();
    for u in stack.iter().chain([&hidden]) {
        assert!(!shown.contains(&(u.0 as i64)), "{u:?} is shown");
    }
    let masked_json = arr(&view, "units").iter().find(|u| int(u, "id") == masked.0 as i64).expect("the masked unit is visible");
    assert_eq!(int(masked_json, "owner"), s.g.barbarian_player().0 as i64, "hidden nationality shows a barbarian");
    // The plot query of the fogged plot shows no units; an attack preview there has no target.
    let plot = wire(&s, Query::Plot { x: fx, y: fy });
    assert_eq!(int(&plot, "vis"), 1);
    assert!(plot.get("units").is_none());
    let next_plot = wire(&s, Query::Plot { x: nx, y: ny });
    assert!(arr(&next_plot, "units").iter().all(|u| int(u, "id") != hidden.0 as i64));
    let combat = wire(&s, Query::Combat { units: vec![ours], x: nx, y: ny });
    assert_eq!(combat.get("valid"), Some(&Json::Bool(false)), "attack preview against an invisible unit: {combat}");
    let path = wire(&s, Query::Path { units: vec![ours], x: fx, y: fy, flags: 0 });
    assert_eq!(path.get("attack"), Some(&Json::Bool(false)));
    // Their objects are not ours to inspect. A plot holds every team's data (the true bonus,
    // all its units, each team's visibility): inspected under debug reveal only, even ours.
    assert!(s.query(&Query::Inspect { kind: "unit".into(), oid: hidden.0 as i64 }).is_err());
    let own_plot = s.g.map.plot_ids().find(|&p| s.g.map.plots[p].owner == Some(h)).expect("a plot of ours");
    let inspect_plot = Query::Inspect { kind: "plot".into(), oid: own_plot.0 as i64 };
    assert_eq!(s.query(&inspect_plot).map_err(|e| e.code()), Err("not_owner"));
    assert!(s.query(&Query::Group { units: vec![hidden] }).is_err());
    // The text summary does not name players we have not met.
    let text = wire(&s, Query::Text);
    for p in s.g.player_ids() {
        if !s.g.teams[team].has_met[s.g.players[p].team] && p != h && s.g.players[p].ever_alive {
            let name = ffh_play::view::player_name(&s.g, p);
            assert!(name.is_empty() ||!text.get("text").and_then(Json::as_str).unwrap().contains(&name), "the text names unmet {name}");
        }
    }

    // Debug reveal shows everything, marked as such.
    s.set_reveal(true);
    assert!(s.query(&inspect_plot).is_ok());
    let view = wire(&s, Query::View);
    assert_eq!(view.get("reveal"), Some(&Json::Bool(true)));
    let shown: Vec<i64> = arr(&view, "units").iter().map(|u| int(u, "id")).collect();
    for u in stack.iter().chain([&hidden, &masked]) {
        assert!(shown.contains(&(u.0 as i64)), "reveal: {u:?} missing");
    }
    let masked_json = arr(&view, "units").iter().find(|u| int(u, "id") == masked.0 as i64).unwrap();
    assert_eq!(int(masked_json, "owner"), ai.0 as i64);
    s.set_reveal(false);
    check_fog(&s, s.g.game_turn());
}
