//! Text for agents and people (design §4.6): `text` (a compact summary of the human's situation
//! for an LLM or a script playing headless), `ascii` (a map with a legend, 2 characters per
//! plot) and `inspect` (`{:#?}` of a game object or an info entry).

use std::fmt::Write as _;

use ffh_core::ids::*;
use ffh_data::*;

use crate::json::Json;
use crate::query::{invalid, Ctx, Layer, QueryErr};
use crate::view::{city_name, Viewer};

fn name<T: crate::names::Named>(d: &GameData, v: T) -> String {
    crate::names::display(d, v).into_owned()
}

/// `text`: status, empire, pending items, ready groups, cities, the last turn report and the
/// latest messages, as plain lines.
pub fn text(v: &Viewer, ctx: &Ctx) -> Json {
    let g = v.g;
    let d = g.d;
    let h = v.h;
    let pl = &g.players[h];
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Turn {} (year {}), player {} {} ({}), phase {}{}",
        g.game_turn(),
        g.game_turn_year(),
        h.0,
        crate::view::player_name(g, h),
        g.player_get_civilization_type(h).map_or("-".to_string(), |c| name(d, c)),
        crate::view::phase_name(g, h, ctx.decision.is_some()),
        if v.reveal { " [DEBUG REVEAL]" } else { "" }
    );
    let research = g.player_get_current_research(h).map_or("none".to_string(), |t| {
        format!("{} {}/{} ({} turns)", name(d, t), g.teams[v.team].research_progress[t], g.team_get_research_cost(v.team, t), g.player_get_research_turns_left(h, t, true))
    });
    let _ = writeln!(
        s,
        "Gold {} ({:+}/turn), research {research}, sliders gold {}% research {}% culture {}% espionage {}%, Armageddon {}",
        pl.gold,
        g.player_calculate_gold_rate(h),
        pl.commerce_percent[0],
        pl.commerce_percent[1],
        pl.commerce_percent[2],
        pl.commerce_percent[3],
        g.get_global_counter()
    );
    if let Some(dec) = ctx.decision {
        let _ = writeln!(s, "DECISION {} ({}): {}", dec.id, dec.kind.name(), dec.title);
        for (i, o) in dec.options.iter().enumerate() {
            let _ = writeln!(s, "  option {i}: {}{}", o.label, if o.enabled { "" } else { " (disabled)" });
        }
    }
    let p = crate::decision::pending(g, h, ctx.events);
    for e in &p.events {
        let _ = writeln!(s, "Event {}: {}", e.triggered.0, e.text);
        for o in &e.options {
            let _ = writeln!(s, "  {} {}: {}", if o.enabled { "+" } else { "-" }, crate::names::name(d, o.event), o.effects.join(", "));
        }
    }
    for vote in &p.votes {
        let choices: Vec<String> = vote.choices.iter().map(|c| crate::names::player_vote_name(*c).map_or_else(|| format!("team {}", c.0), str::to_string)).collect();
        let _ = writeln!(s, "Vote {} ({}): {} choices {}", vote.vote.0, name(d, vote.resolution), vote.text, choices.join(", "));
    }
    for e in &p.elections {
        let options: Vec<String> = e.options.iter().enumerate().map(|(i, o)| format!("{i}: {}", o.text)).collect();
        let _ = writeln!(s, "Election {} ({}): {}; or none", e.selection.0, name(d, e.source), options.join("; "));
    }
    if p.research_needed {
        let _ = writeln!(s, "Research needed");
    }
    if !p.idle_cities.is_empty() {
        let idle: Vec<String> = p.idle_cities.iter().map(|&c| format!("{} ({})", city_name(g, c), c.0)).collect();
        let _ = writeln!(s, "Idle cities: {}", idle.join(", "));
    }
    if !p.promotions_ready.is_empty() {
        let _ = writeln!(s, "Promotions ready: units {:?}", p.promotions_ready.iter().map(|u| u.0).collect::<Vec<_>>());
    }
    if !p.ai_groups_idle.is_empty() {
        let _ = writeln!(s, "{} automated or AI-controlled groups wait for the AI's moves", p.ai_groups_idle.len());
    }
    let _ = writeln!(s, "Ready groups ({}):", p.ready_groups.len());
    for &grp in &p.ready_groups {
        let group = &g.groups[grp];
        let units: Vec<String> = group
            .units
            .iter()
            .map(|&u| {
                let unit = &g.units[u];
                format!("{} {} {}hp {}/{}mv", u.0, name(d, unit.unit_type), g.unit_curr_hit_points(u), g.unit_moves_left(u) / 60, g.unit_max_moves(u) / 60)
            })
            .collect();
        let head = group.units.first().map(|&u| (g.units[u].x, g.units[u].y)).unwrap_or((-1, -1));
        let _ = writeln!(s, "  group {} at ({}, {}): {}", grp.0, head.0, head.1, units.join("; "));
    }
    let _ = writeln!(s, "Cities ({}):", pl.cities.len());
    for &c in &pl.cities {
        let city = &g.cities[c];
        let prod = city.order_queue.first().map_or("nothing".to_string(), |o| {
            let what = match o.order_type {
                OrderType::ORDER_TRAIN => d.units.get(UnitType::from_usize(o.data1.max(0) as usize)).map(|_| name(d, UnitType::from_usize(o.data1 as usize))),
                OrderType::ORDER_CONSTRUCT => d.buildings.get(BuildingType::from_usize(o.data1.max(0) as usize)).map(|_| name(d, BuildingType::from_usize(o.data1 as usize))),
                OrderType::ORDER_CREATE => d.projects.get(ProjectType::from_usize(o.data1.max(0) as usize)).map(|_| name(d, ProjectType::from_usize(o.data1 as usize))),
                _ => d.processes.get(ProcessType::from_usize(o.data1.max(0) as usize)).map(|_| name(d, ProcessType::from_usize(o.data1 as usize))),
            };
            let turns = if o.order_type == OrderType::ORDER_MAINTAIN { String::new() } else { format!(" {}t", g.city_production_turns_left(c)) };
            format!("{}{turns}", what.unwrap_or_else(|| "?".into()))
        });
        let _ = writeln!(
            s,
            "  city {} {} ({}, {}) size {} food {:+} producing {prod}{}",
            c.0,
            city_name(g, c),
            city.x,
            city.y,
            city.population,
            g.city_food_difference(c, true),
            if g.city_is_disorder(c) { " DISORDER" } else { "" }
        );
    }
    let wars: Vec<String> = g.player_ids().filter(|&q| q != h && v.player_met(q) && g.players[q].alive && g.at_war(v.team, g.players[q].team)).map(|q| format!("{} ({})", crate::view::player_name(g, q), q.0)).collect();
    if !wars.is_empty() {
        let _ = writeln!(s, "At war with: {}", wars.join(", "));
    }
    if let Some(r) = ctx.report {
        let _ = writeln!(s, "Report (turn {} -> {}):", r.from_turn, r.turn);
        for l in r.lines.iter().take(30) {
            let _ = writeln!(s, "  {l}");
        }
    }
    if let Some(m) = ctx.messages {
        let last = m.tail(8);
        if !last.is_empty() {
            let _ = writeln!(s, "Messages:");
            for e in last {
                let _ = writeln!(s, "  t{}: {}", e.turn, e.text);
            }
        }
    }
    crate::jobj! { "text" => s }
}

/// One character per name, distinct, from the names' letters (`.`-free).
fn codes(names: &[String], taken: &str) -> Vec<char> {
    let mut used: Vec<char> = taken.chars().collect();
    let mut out = Vec::with_capacity(names.len());
    for n in names {
        let candidates = n.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).chain('a'..='z').chain('0'..='9');
        let mut pick = '?';
        for c in candidates {
            if !used.contains(&c) {
                pick = c;
                break;
            }
        }
        used.push(pick);
        out.push(pick);
    }
    out
}

/// A player's code on the political and units layers: 0-9, then A-Z, a-z.
fn player_code(p: PlayerId) -> char {
    const CODES: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    CODES.get(p.0 as usize).map_or('?', |&b| b as char)
}

fn digit(n: i32) -> char {
    match n {
        n if n < 0 => '-',
        0..=9 => (b'0' + n as u8) as char,
        _ => '+',
    }
}

/// The largest ASCII map side.
pub const ASCII_MAX: i32 = 256;

/// `ascii {x,y,w,h,layer}`: `w` x `h` plots from `(x, y)` (0 = the whole map; wrapping maps
/// wrap), north up, 2 characters per plot, `::` unrevealed. Layers:
/// * `terrain`: the terrain's code (upper case visible, lower case fogged; `^` a peak), then `#`
///   a city, the feature's code, `^` hills, or a space;
/// * `political`: the owner's player code (`.` none), then `#` a city, `*` a capital;
/// * `yield`: food and production digits (`+` above 9; fogged plots `..`);
/// * `units`: the player code of the top visible unit (`.` none), then the number of visible
///   units (`+` above 9).
pub fn ascii(v: &Viewer, x: i32, y: i32, w: i32, h: i32, layer: Layer) -> Result<Json, QueryErr> {
    let g = v.g;
    let d = g.d;
    let (mw, mh) = (g.map.grid_width, g.map.grid_height);
    let w = if w <= 0 { mw } else { w };
    let h = if h <= 0 { mh } else { h };
    if !(0..mw).contains(&x) || !(0..mh).contains(&y) {
        return Err(invalid(format!("({x}, {y}) is off the map ({mw} x {mh})")));
    }
    if w > ASCII_MAX || h > ASCII_MAX {
        return Err(invalid(format!("at most {ASCII_MAX} x {ASCII_MAX} plots")));
    }
    let w = if g.map.wrap_x { w.min(mw) } else { w.min(mw - x) };
    let h = if g.map.wrap_y { h.min(mh) } else { h.min(mh - y) };
    let terrain_names: Vec<String> = d.terrains.keys().map(|t| name(d, t)).collect();
    let feature_names: Vec<String> = d.features.keys().map(|f| name(d, f)).collect();
    let tcodes = codes(&terrain_names, "^:.#");
    let taken: String = tcodes.iter().collect::<String>() + "^:.#* ";
    let fcodes = codes(&feature_names, &taken);
    let mut legend: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    // x header: tens and ones.
    let header = |f: fn(i32) -> char| -> String {
        let mut line = "    ".to_string();
        for i in 0..w {
            let px = (x + i).rem_euclid(mw);
            line.push(f(px));
            line.push(' ');
        }
        line
    };
    rows.push(header(|px| if px % 10 == 0 { digit((px / 10) % 10) } else { ' ' }));
    rows.push(header(|px| digit(px % 10)));
    for j in (0..h).rev() {
        let py = y + j;
        let mut line = format!("{:>3} ", py.rem_euclid(mh));
        for i in 0..w {
            let Some(p) = g.map.plot_xy(x + i, py) else {
                line.push_str("  ");
                continue;
            };
            if !v.plot_revealed(p) {
                line.push_str("::");
                continue;
            }
            let pl = &g.map.plots[p];
            let visible = v.plot_visible(p);
            let city = pl.plot_city.filter(|&c| v.city_revealed(c));
            let (a, b) = match layer {
                Layer::Terrain => {
                    let t = pl.terrain_type.map_or('?', |t| tcodes[t.idx()]);
                    let a = if g.plot_is_peak(p) {
                        '^'
                    } else if visible {
                        t.to_ascii_uppercase()
                    } else {
                        t
                    };
                    let b = if city.is_some() {
                        '#'
                    } else if let Some(f) = pl.feature_type {
                        fcodes[f.idx()]
                    } else if g.plot_is_hills(p) {
                        '^'
                    } else {
                        ' '
                    };
                    (a, b)
                }
                Layer::Political => {
                    let a = v.plot_owner(p).map_or('.', player_code);
                    let b = match city {
                        Some(c) if g.city_is_capital(c) => '*',
                        Some(_) => '#',
                        None => ' ',
                    };
                    (a, b)
                }
                Layer::Yield => {
                    if visible {
                        (digit(pl.yields[0]), digit(pl.yields[1]))
                    } else {
                        ('.', '.')
                    }
                }
                Layer::Units => {
                    let units: Vec<UnitId> = if visible { pl.units.iter().copied().filter(|&u| v.unit_visible(u)).collect() } else { Vec::new() };
                    match units.first() {
                        None => ('.', ' '),
                        Some(&u) => (player_code(v.unit_owner(u)), digit(units.len() as i32)),
                    }
                }
            };
            line.push(a);
            line.push(b);
        }
        rows.push(line);
    }
    match layer {
        Layer::Terrain => {
            for (t, c) in terrain_names.iter().zip(&tcodes) {
                legend.push(format!("{} {t}", c.to_ascii_uppercase()));
            }
            legend.push("^  peak (first column), hills (second)".into());
            for (f, c) in feature_names.iter().zip(&fcodes) {
                legend.push(format!(" {c} {f}"));
            }
            legend.push(" # city; upper case = visible, lower case = fogged; :: unrevealed".into());
        }
        Layer::Political | Layer::Units => {
            for p in g.player_ids().filter(|&p| v.player_met(p) && g.players[p].ever_alive) {
                legend.push(format!("{} {} (player {})", player_code(p), crate::view::player_name(g, p), p.0));
            }
            legend.push(if layer == Layer::Political { ". unowned; # city, * capital" } else { ". no visible unit; second column: unit count" }.into());
        }
        Layer::Yield => legend.push("food, production; .. fogged; + above 9".into()),
    }
    let mut map = rows.join("\n");
    map.push('\n');
    Ok(crate::jobj! { "x" => x, "y" => y, "w" => w, "h" => h, "layer" => layer.name(), "map" => map, "legend" => Json::Arr(legend.into_iter().map(Json::from).collect()) })
}

/// `inspect {kind,oid}`: `{:#?}` of a game object of the human's (`unit`, `group`, `city`,
/// `player`, `team`, a `deal` it is part of; any object, and any `plot`, under debug reveal), or
/// of info entry `oid` of an info table (a kind name such as `UnitType` or a table name such as
/// `units`). The dumps of the human's own city, player and team keep their per-team
/// bookkeeping (which teams revealed the city, espionage and contact counters): this is the
/// rules tester's raw inspector.
pub fn inspect(v: &Viewer, kind: &str, id: i64) -> Result<Json, QueryErr> {
    let g = v.g;
    let d = g.d;
    let id32 = u32::try_from(id).map_err(|_| invalid(format!("field `oid`: {id} out of range")))?;
    let mine = |owner: PlayerId| v.reveal || owner == v.h;
    let debug = match kind {
        "unit" => {
            let unit = g.units.get(UnitId(id32)).ok_or(QueryErr::Gone)?;
            if !mine(unit.owner) {
                return Err(QueryErr::NotOwner);
            }
            format!("{unit:#?}")
        }
        "group" => {
            let group = g.groups.get(GroupId(id32)).ok_or(QueryErr::Gone)?;
            if !mine(group.owner) {
                return Err(QueryErr::NotOwner);
            }
            format!("{group:#?}")
        }
        "city" => {
            let city = g.cities.get(CityId(id32)).ok_or(QueryErr::Gone)?;
            if !mine(city.owner) {
                return Err(QueryErr::NotOwner);
            }
            format!("{city:#?}")
        }
        "plot" => {
            // Under debug reveal only: a plot holds every team's data (its true bonus before the
            // tech that reveals it, all its units, each team's visibility and revealed values),
            // so even a plot the human owns is not its own object (P3 review). The `plot` query
            // is the fog-filtered plot.
            let plot = g.map.plots.get(id32 as usize).ok_or(QueryErr::Gone)?;
            if !v.reveal {
                return Err(QueryErr::NotOwner);
            }
            format!("{plot:#?}")
        }
        "player" => {
            if id32 as usize >= g.num_players() {
                return Err(QueryErr::Gone);
            }
            let p = PlayerId(id32 as u8);
            if !mine(p) {
                return Err(QueryErr::NotOwner);
            }
            format!("{:#?}", g.players[p])
        }
        "team" => {
            if id32 as usize >= g.num_teams() {
                return Err(QueryErr::Gone);
            }
            let t = TeamId(id32 as u8);
            if !(v.reveal || t == v.team) {
                return Err(QueryErr::NotOwner);
            }
            format!("{:#?}", g.teams[t])
        }
        "deal" => {
            let deal = g.deals.get(DealId(id32)).ok_or(QueryErr::Gone)?;
            if !(mine(deal.first_player) || mine(deal.second_player)) {
                return Err(QueryErr::NotOwner);
            }
            format!("{deal:#?}")
        }
        other => {
            let k = crate::names::kind(other).ok_or_else(|| invalid(format!("field `kind`: unknown kind {other:?} (unit, group, city, plot, player, team, deal, or an info table)")))?;
            (k.debug)(d, id32 as usize).ok_or_else(|| invalid(format!("{} has no info entry {id}", k.kind)))?
        }
    };
    Ok(crate::jobj! { "kind" => kind, "oid" => id, "debug" => debug })
}
