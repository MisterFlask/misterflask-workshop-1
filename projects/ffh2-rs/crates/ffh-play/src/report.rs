//! The human's messages ([`MessageStore`]) and the turn report (design §5.1 "Turn flow").
//!
//! ffh-core logs about 86 message sites where C++ has about 190 `addMessage` calls (F14), so a
//! log alone misses much of what a player learns at the start of a turn. The session takes a
//! fog-filtered [`Snapshot`] of what the human knows when it ends its turn, and at the next
//! control point diffs it against the game ([`Report::build`]): units lost, gained or ready to
//! promote; cities founded, acquired, lost, grown, shrunk, starving or in disorder; buildings
//! completed; techs gained; wars and peace among the teams it had met when it ended its turn
//! (a war of a team it meets during the AI turns is not new: it is reported as a contact);
//! enemies sighted near its cities; Armageddon counter thresholds crossed; borders lost. The
//! messages drained since the snapshot are merged in.
//!
//! The snapshot holds only what the human may know (its own units and cities, foreign cities it
//! has revealed, met teams, visible units), so the diff cannot leak through the fog.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::log::LogEntry;
use ffh_data::*;

use crate::json::{FieldError, FromJson, Json};
use crate::save::log_entry_json;
use crate::view::Viewer;

// ---------------------------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------------------------

/// Messages kept for the human (the oldest are dropped beyond this).
pub const MESSAGES_KEPT: usize = 10_000;

/// The human's messages, oldest first. Each message has an index (0 for the first message of the
/// session); the store keeps the last [`MESSAGES_KEPT`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageStore {
    entries: Vec<LogEntry>,
    /// Messages ever added (the index of the next one).
    total: usize,
}

impl MessageStore {
    /// A store holding `tail` (a loaded sidecar's latest messages) as messages `0..tail.len()`.
    pub fn from_tail(tail: Vec<LogEntry>) -> MessageStore {
        let total = tail.len();
        MessageStore { entries: tail, total }
    }

    pub fn push(&mut self, e: LogEntry) {
        self.entries.push(e);
        self.total += 1;
        if self.entries.len() > MESSAGES_KEPT {
            let cut = self.entries.len() - MESSAGES_KEPT;
            self.entries.drain(..cut);
        }
    }

    /// The kept messages, oldest first.
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// The index the next message will get (`view.log_next`).
    pub fn total(&self) -> usize {
        self.total
    }

    /// Index of the oldest kept message.
    pub fn first(&self) -> usize {
        self.total - self.entries.len()
    }

    /// The kept messages with index `since` or later.
    pub fn since(&self, since: usize) -> &[LogEntry] {
        let skip = since.saturating_sub(self.first()).min(self.entries.len());
        &self.entries[skip..]
    }

    /// The last `n` messages.
    pub fn tail(&self, n: usize) -> Vec<LogEntry> {
        self.entries[self.entries.len().saturating_sub(n)..].to_vec()
    }

    /// The `log` query: `{from, next, entries: [{i, t, text}]}` (`from` is the index of the
    /// first entry returned, later than `since` when older messages were dropped).
    pub fn since_json(&self, since: usize) -> Json {
        let from = since.max(self.first()).min(self.total);
        let entries = self.since(since).iter().enumerate().map(|(k, e)| crate::jobj! { "i" => from + k, "t" => e.turn, "text" => e.text.as_str() });
        crate::jobj! { "from" => from, "next" => self.total, "entries" => Json::Arr(entries.collect()) }
    }
}

// ---------------------------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------------------------

/// One of the human's units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapUnit {
    pub id: UnitId,
    pub unit_type: UnitType,
    pub x: i32,
    pub y: i32,
    pub promotion_ready: bool,
}

/// One of the human's cities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapCity {
    pub id: CityId,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub size: i32,
    pub buildings: Vec<BuildingType>,
}

/// A foreign city the human has revealed, with its owner as the human sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapForeign {
    pub id: CityId,
    pub owner: PlayerId,
    pub name: String,
    pub x: i32,
    pub y: i32,
}

/// What the human knew when it ended its turn (fog-filtered).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub turn: i32,
    /// `MessageStore::total` when taken: the report merges the messages from here on.
    pub log_mark: usize,
    pub units: Vec<SnapUnit>,
    pub cities: Vec<SnapCity>,
    pub foreign: Vec<SnapForeign>,
    pub techs: Vec<TechType>,
    /// The teams the human's team has met (its own included).
    pub met: Vec<TeamId>,
    /// Pairs of teams at war (`a < b`), among `met`.
    pub wars: Vec<(TeamId, TeamId)>,
    pub armageddon: i32,
    /// Plots the human owns.
    pub plots: Vec<PlotIdx>,
    /// Visible hostile units within 2 plots of the human's cities.
    pub enemies: Vec<UnitId>,
}

/// Hostile units the human sees within 2 plots of its cities: units of teams at war with it
/// (barbarians, animals and demons included), in plot order.
fn enemies_near(v: &Viewer) -> Vec<(UnitId, CityId)> {
    let g = v.g;
    let mut out: Vec<(UnitId, CityId)> = Vec::new();
    for &c in &g.players[v.h].cities {
        let (cx, cy) = (g.cities[c].x, g.cities[c].y);
        for dy in -2..=2 {
            for dx in -2..=2 {
                let Some(p) = g.plot_xy(cx, cy, dx, dy) else { continue };
                if !v.plot_visible(p) {
                    continue;
                }
                for &u in &g.map.plots[p].units {
                    let owner = v.unit_owner(u);
                    if v.unit_visible(u) && g.at_war(v.team, g.players[owner].team) && !out.iter().any(|&(x, _)| x == u) {
                        out.push((u, c));
                    }
                }
            }
        }
    }
    out
}

/// The teams the human has met (its own included).
fn met_teams(v: &Viewer) -> Vec<TeamId> {
    v.g.team_ids().filter(|&t| v.team_met(t)).collect()
}

fn wars(v: &Viewer) -> Vec<(TeamId, TeamId)> {
    let g = v.g;
    let met = met_teams(v);
    let mut out = Vec::new();
    for (i, &a) in met.iter().enumerate() {
        for &b in &met[i + 1..] {
            if g.at_war(a, b) {
                out.push((a, b));
            }
        }
    }
    out
}

impl Snapshot {
    /// What `v`'s human knows now; `log_mark` = `MessageStore::total`.
    pub fn take(v: &Viewer, log_mark: usize) -> Snapshot {
        let g = v.g;
        let d = g.d;
        let pl = &g.players[v.h];
        Snapshot {
            turn: g.game_turn(),
            log_mark,
            units: pl
                .units
                .iter()
                .map(|&u| {
                    let unit = &g.units[u];
                    SnapUnit { id: u, unit_type: unit.unit_type, x: unit.x, y: unit.y, promotion_ready: unit.promotion_ready }
                })
                .collect(),
            cities: pl
                .cities
                .iter()
                .map(|&c| {
                    let city = &g.cities[c];
                    SnapCity {
                        id: c,
                        name: crate::view::city_name(g, c),
                        x: city.x,
                        y: city.y,
                        size: city.population,
                        buildings: d.buildings.keys().filter(|&b| city.num_real_building[b] > 0).collect(),
                    }
                })
                .collect(),
            foreign: g
                .cities
                .iter()
                .filter(|(c, city)| city.owner != v.h && v.city_revealed(*c))
                .map(|(c, city)| SnapForeign { id: c, owner: city.owner, name: crate::view::city_name(g, c), x: city.x, y: city.y })
                .collect(),
            techs: d.techs.keys().filter(|&t| g.teams[v.team].has_tech[t]).collect(),
            met: met_teams(v),
            wars: wars(v),
            armageddon: g.get_global_counter(),
            plots: g.map.plot_ids().filter(|&p| g.map.plots[p].owner == Some(v.h)).collect(),
            enemies: enemies_near(v).into_iter().map(|(u, _)| u).collect(),
        }
    }

    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "turn" => self.turn,
            "log_mark" => self.log_mark,
            "units" => Json::Arr(self.units.iter().map(|u| crate::jarr![u.id, Json::idx(u.unit_type), u.x, u.y, u.promotion_ready]).collect()),
            "cities" => Json::Arr(self.cities.iter().map(|c| crate::jarr![
                c.id, c.name.as_str(), c.x, c.y, c.size, Json::Arr(c.buildings.iter().map(|&b| Json::idx(b)).collect())
            ]).collect()),
            "foreign" => Json::Arr(self.foreign.iter().map(|c| crate::jarr![c.id, c.owner, c.name.as_str(), c.x, c.y]).collect()),
            "techs" => Json::Arr(self.techs.iter().map(|&t| Json::idx(t)).collect()),
            "met" => self.met.clone(),
            "wars" => Json::Arr(self.wars.iter().map(|&(a, b)| crate::jarr![a, b]).collect()),
            "armageddon" => self.armageddon,
            "plots" => Json::Arr(self.plots.iter().map(|p| Json::from(p.0)).collect()),
            "enemies" => self.enemies.clone(),
        }
    }

    /// Whether the snapshot's player, team and plot ids exist in `g` (a sidecar's snapshot is
    /// checked against its save before it is used: a mismatched or corrupted one is dropped).
    /// Unit and city ids need no check: the report looks them up.
    pub fn fits(&self, g: &Game) -> bool {
        let team = |t: &TeamId| t.idx() < g.num_teams();
        self.foreign.iter().all(|c| c.owner.idx() < g.num_players())
            && self.met.iter().all(team)
            && self.wars.iter().all(|(a, b)| team(a) && team(b))
            && self.plots.iter().all(|p| (p.0 as usize) < g.map.plots.len())
    }

    /// Reads [`Snapshot::to_json`] (a sidecar's), checking every type index against `d` (the ids
    /// are checked against the game by [`Snapshot::fits`]).
    pub fn from_json(d: &GameData, j: &Json) -> Result<Snapshot, FieldError> {
        fn tuple<'a>(key: &str, v: &'a Json, n: usize) -> Result<&'a [Json], FieldError> {
            match v.as_arr() {
                Some(a) if a.len() == n => Ok(a),
                _ => Err(FieldError::wrong(key, &format!("an array of {n} values"), v)),
            }
        }
        fn get<T: FromJson>(key: &str, v: &Json) -> Result<T, FieldError> {
            T::from_json(v).ok_or_else(|| FieldError::wrong(key, &T::expected(), v))
        }
        fn ty<T: crate::names::Named>(d: &GameData, key: &str, v: &Json) -> Result<T, FieldError> {
            crate::names::decode::<T>(d, v).map_err(|e| FieldError::new(key, e))
        }
        let list = |key: &str| j.req_arr(key);
        Ok(Snapshot {
            turn: j.req("turn")?,
            log_mark: j.req("log_mark")?,
            units: list("units")?
                .iter()
                .map(|u| {
                    let a = tuple("units", u, 5)?;
                    Ok(SnapUnit { id: get("units", &a[0])?, unit_type: ty(d, "units", &a[1])?, x: get("units", &a[2])?, y: get("units", &a[3])?, promotion_ready: get("units", &a[4])? })
                })
                .collect::<Result<_, FieldError>>()?,
            cities: list("cities")?
                .iter()
                .map(|c| {
                    let a = tuple("cities", c, 6)?;
                    let buildings = a[5].as_arr().ok_or_else(|| FieldError::wrong("cities", "an array", &a[5]))?;
                    Ok(SnapCity {
                        id: get("cities", &a[0])?,
                        name: get("cities", &a[1])?,
                        x: get("cities", &a[2])?,
                        y: get("cities", &a[3])?,
                        size: get("cities", &a[4])?,
                        buildings: buildings.iter().map(|b| ty(d, "cities", b)).collect::<Result<_, _>>()?,
                    })
                })
                .collect::<Result<_, FieldError>>()?,
            foreign: list("foreign")?
                .iter()
                .map(|c| {
                    let a = tuple("foreign", c, 5)?;
                    Ok(SnapForeign {
                        id: get("foreign", &a[0])?,
                        owner: get("foreign", &a[1])?,
                        name: get("foreign", &a[2])?,
                        x: get("foreign", &a[3])?,
                        y: get("foreign", &a[4])?,
                    })
                })
                .collect::<Result<_, FieldError>>()?,
            techs: list("techs")?.iter().map(|t| ty(d, "techs", t)).collect::<Result<_, _>>()?,
            met: j.req("met")?,
            wars: list("wars")?
                .iter()
                .map(|w| {
                    let a = tuple("wars", w, 2)?;
                    Ok((get("wars", &a[0])?, get("wars", &a[1])?))
                })
                .collect::<Result<_, FieldError>>()?,
            armageddon: j.req("armageddon")?,
            plots: list("plots")?.iter().map(|p| get::<u32>("plots", p).map(PlotIdx)).collect::<Result<_, _>>()?,
            enemies: j.req("enemies")?,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------

/// The Armageddon counter values the rules react to: the `iPrereqGlobalCounter` of units,
/// projects and event triggers, and the FfH Python thresholds (Planar Gate 50, 75, 100).
pub fn armageddon_thresholds(d: &GameData) -> Vec<i32> {
    let mut t: Vec<i32> = d
        .units
        .iter()
        .map(|u| u.prereq_global_counter)
        .chain(d.projects.iter().map(|p| p.prereq_global_counter))
        .chain(d.event_triggers.iter().map(|e| e.prereq_global_counter))
        .chain([50, 75, 100])
        .filter(|&n| n > 0)
        .collect();
    t.sort_unstable();
    t.dedup();
    t
}

/// The turn report: what changed for the human between the end of its turn and its next
/// control point (design §5.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub from_turn: i32,
    pub turn: i32,
    /// The diff, one array per kind of change (see [`Report::build`]).
    pub items: Json,
    /// Plain-text lines of the diff, for `text` and people.
    pub lines: Vec<String>,
    /// The messages since the snapshot.
    pub messages: Vec<LogEntry>,
}

impl Report {
    /// Diffs `before` against the game as `v`'s human sees it now; `messages` are the human's
    /// messages since `before.log_mark`.
    pub fn build(before: &Snapshot, v: &Viewer, messages: Vec<LogEntry>) -> Report {
        let g = v.g;
        let d = g.d;
        let h = v.h;
        let mut lines = Vec::new();
        let unit_name = |t: UnitType| crate::names::display(d, t).into_owned();
        let own_unit = |u: UnitId| g.units.get(u).is_some_and(|unit| unit.owner == h);
        let own_city = |c: CityId| g.cities.get(c).is_some_and(|city| city.owner == h);

        // Units.
        let mut lost = Vec::new();
        for u in &before.units {
            if !own_unit(u.id) {
                lines.push(format!("lost {} at ({}, {})", unit_name(u.unit_type), u.x, u.y));
                lost.push(crate::jobj! { "unit" => u.id, "type" => Json::idx(u.unit_type), "x" => u.x, "y" => u.y });
            }
        }
        let mut gained = Vec::new();
        let mut promotable = Vec::new();
        for &u in &g.players[h].units {
            let unit = &g.units[u];
            let old = before.units.iter().find(|s| s.id == u);
            if old.is_none() {
                lines.push(format!("new {} at ({}, {})", unit_name(unit.unit_type), unit.x, unit.y));
                gained.push(crate::jobj! { "unit" => u, "type" => Json::idx(unit.unit_type), "x" => unit.x, "y" => unit.y });
            }
            if unit.promotion_ready && !old.is_some_and(|s| s.promotion_ready) {
                lines.push(format!("{} can be promoted", unit_name(unit.unit_type)));
                promotable.push(Json::from(u));
            }
        }

        // Cities.
        let (mut founded, mut acquired, mut lost_cities, mut grown, mut shrunk, mut starving, mut disorder, mut built) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for c in &before.cities {
            if !own_city(c.id) {
                // C++ makes a new city object when a city changes hands; who has it now, if seen.
                let now = g.map.plot_xy(c.x, c.y).and_then(|p| g.map.plots[p].plot_city).filter(|&n| v.city_revealed(n));
                let owner = now.map(|n| g.cities[n].owner);
                lines.push(match owner {
                    Some(o) => format!("{} now belongs to {}", c.name, crate::view::player_name(g, o)),
                    None => format!("{} is lost", c.name),
                });
                lost_cities.push(crate::jobj! { "city" => c.id, "name" => c.name.as_str(), "x" => c.x, "y" => c.y, "owner" => owner });
            }
        }
        for &c in &g.players[h].cities {
            let city = &g.cities[c];
            let name = crate::view::city_name(g, c);
            match before.cities.iter().find(|s| s.id == c) {
                None => {
                    let item = crate::jobj! { "city" => c, "name" => name.as_str(), "x" => city.x, "y" => city.y, "from" => city.previous_owner };
                    if city.previous_owner.is_some() {
                        lines.push(format!("{name} is ours"));
                        acquired.push(item);
                    } else {
                        lines.push(format!("{name} founded"));
                        founded.push(item);
                    }
                }
                Some(old) => {
                    if city.population > old.size {
                        lines.push(format!("{name} grew to size {}", city.population));
                        grown.push(crate::jobj! { "city" => c, "size" => city.population, "from" => old.size });
                    } else if city.population < old.size {
                        lines.push(format!("{name} shrank to size {}", city.population));
                        shrunk.push(crate::jobj! { "city" => c, "size" => city.population, "from" => old.size });
                    }
                    for b in d.buildings.keys().filter(|&b| city.num_real_building[b] > 0 && !old.buildings.contains(&b)) {
                        lines.push(format!("{name} completed {}", crate::names::display(d, b)));
                        built.push(crate::jobj! { "city" => c, "building" => Json::idx(b) });
                    }
                }
            }
            if g.city_food_difference(c, true) < 0 {
                lines.push(format!("{name} is starving"));
                starving.push(Json::from(c));
            }
            if g.city_is_disorder(c) {
                lines.push(format!("{name} is in disorder"));
                disorder.push(Json::from(c));
            }
        }
        // Foreign cities the human had revealed that are gone (razed, or taken: C++ makes a new
        // city object when a city changes hands); who holds the plot now, if the human sees it.
        let mut foreign_changed = Vec::new();
        for f in &before.foreign {
            if g.cities.contains(f.id) {
                continue;
            }
            let now = g.map.plot_xy(f.x, f.y).and_then(|p| g.map.plots[p].plot_city).filter(|&n| v.city_revealed(n));
            if now.is_some_and(own_city) {
                continue; // reported as acquired
            }
            let owner = now.map(|n| g.cities[n].owner);
            lines.push(match owner {
                Some(o) => format!("{} now belongs to {}", f.name, crate::view::player_name(g, o)),
                None => format!("{} ({}) is gone", f.name, crate::view::player_name(g, f.owner)),
            });
            foreign_changed.push(crate::jobj! { "city" => f.id, "name" => f.name.as_str(), "x" => f.x, "y" => f.y, "was" => f.owner, "owner" => owner });
        }

        // Techs.
        let techs: Vec<TechType> = d.techs.keys().filter(|&t| g.teams[v.team].has_tech[t] && !before.techs.contains(&t)).collect();
        for &t in &techs {
            lines.push(format!("learned {}", crate::names::display(d, t)));
        }

        // Contacts, wars and peace. A war or peace is news only between teams the human had met
        // at the snapshot: a team met since brings its wars along, which are not new.
        let contacts: Vec<TeamId> = met_teams(v).into_iter().filter(|t| !before.met.contains(t)).collect();
        let now_wars = wars(v);
        let new_wars: Vec<(TeamId, TeamId)> =
            now_wars.iter().copied().filter(|w| !before.wars.contains(w) && before.met.contains(&w.0) && before.met.contains(&w.1)).collect();
        // Peace between teams still met and alive (a team destroyed is not a peace).
        let peace: Vec<(TeamId, TeamId)> =
            before.wars.iter().copied().filter(|w| !now_wars.contains(w) && g.team_is_alive(w.0) && g.team_is_alive(w.1)).collect();
        let team_name = |t: TeamId| if t == v.team { "we".to_string() } else { format!("team {}", t.0) };
        for &t in &contacts {
            let players: Vec<String> = g.player_ids().filter(|&p| g.players[p].team == t && g.players[p].alive).map(|p| crate::view::player_name(g, p)).collect();
            lines.push(format!("met {} ({})", team_name(t), players.join(", ")));
        }
        for &(a, b) in &new_wars {
            lines.push(format!("war: {} and {}", team_name(a), team_name(b)));
        }
        for &(a, b) in &peace {
            lines.push(format!("peace: {} and {}", team_name(a), team_name(b)));
        }

        // Enemies sighted near our cities.
        let mut enemies = Vec::new();
        for (u, c) in enemies_near(v) {
            if !before.enemies.contains(&u) {
                let unit = &g.units[u];
                lines.push(format!("enemy {} near {}", unit_name(unit.unit_type), crate::view::city_name(g, c)));
                enemies.push(crate::jobj! { "unit" => u, "type" => Json::idx(unit.unit_type), "owner" => v.unit_owner(u), "x" => unit.x, "y" => unit.y, "city" => c });
            }
        }

        // Armageddon counter.
        let counter = g.get_global_counter();
        let crossed: Vec<i32> = armageddon_thresholds(d).into_iter().filter(|&t| before.armageddon < t && t <= counter).collect();
        if counter != before.armageddon {
            lines.push(format!("Armageddon counter {} -> {counter}", before.armageddon));
        }
        for t in &crossed {
            lines.push(format!("Armageddon counter reached {t}"));
        }

        // Borders lost: plots we owned that another player holds now (as we see it).
        let mut borders = Vec::new();
        for &p in &before.plots {
            let pl = &g.map.plots[p];
            if pl.owner != Some(h) {
                let owner = v.plot_owner(p);
                if owner.is_some() {
                    borders.push(crate::jobj! { "x" => pl.x, "y" => pl.y, "owner" => owner });
                }
            }
        }
        if !borders.is_empty() {
            lines.push(format!("{} plots lost to other borders", borders.len()));
        }

        let items = crate::jobj! {
            "units_lost" => Json::Arr(lost),
            "units_gained" => Json::Arr(gained),
            "promotions_ready" => Json::Arr(promotable),
            "cities_founded" => Json::Arr(founded),
            "cities_acquired" => Json::Arr(acquired),
            "cities_lost" => Json::Arr(lost_cities),
            "cities_grown" => Json::Arr(grown),
            "cities_shrunk" => Json::Arr(shrunk),
            "cities_starving" => Json::Arr(starving),
            "cities_disorder" => Json::Arr(disorder),
            "buildings_completed" => Json::Arr(built),
            "foreign_cities_gone" => Json::Arr(foreign_changed),
            "techs_gained" => Json::Arr(techs.iter().map(|&t| Json::idx(t)).collect()),
            "contacts" => contacts,
            "wars" => Json::Arr(new_wars.iter().map(|&(a, b)| crate::jarr![a, b]).collect()),
            "peace" => Json::Arr(peace.iter().map(|&(a, b)| crate::jarr![a, b]).collect()),
            "enemies_near" => Json::Arr(enemies),
            "armageddon" => crate::jobj! {
                "from" => before.armageddon, "to" => counter, "thresholds" => crossed.clone(),
            },
            "borders_lost" => Json::Arr(borders),
        };
        Report { from_turn: before.turn, turn: g.game_turn(), items, lines, messages }
    }

    pub fn to_json(&self) -> Json {
        let mut j = crate::jobj! { "from_turn" => self.from_turn, "turn" => self.turn };
        if let Json::Obj(m) = &self.items {
            for (k, v) in m {
                j.set(k, v.clone());
            }
        }
        j.with("lines", Json::Arr(self.lines.iter().map(Json::from).collect()))
            .with("messages", Json::Arr(self.messages.iter().map(log_entry_json).collect()))
    }

    /// Reads [`Report::to_json`] (a sidecar's); `None` if it does not read.
    pub fn from_json(j: &Json) -> Option<Report> {
        let messages = j
            .req_arr("messages")
            .ok()?
            .iter()
            .map(|m| Some(LogEntry { turn: m.req("t").ok()?, player: m.opt("player").ok()?, text: m.req("text").ok()? }))
            .collect::<Option<Vec<_>>>()?;
        let items = Json::Obj(j.as_obj()?.iter().filter(|(k, _)| !matches!(k.as_str(), "from_turn" | "turn" | "lines" | "messages")).cloned().collect());
        Some(Report { from_turn: j.req("from_turn").ok()?, turn: j.req("turn").ok()?, items, lines: j.req("lines").ok()?, messages })
    }
}

/// [`Snapshot::take`] under a guard, for the session.
pub fn snapshot(g: &Game, h: PlayerId, log_mark: usize) -> Snapshot {
    crate::guard::read(g, |g| Snapshot::take(&Viewer::new(g, h, false), log_mark))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_store_indices() {
        let e = |i: i32| LogEntry { turn: i, player: None, text: format!("m{i}") };
        let mut m = MessageStore::default();
        for i in 0..5 {
            m.push(e(i));
        }
        assert_eq!(m.total(), 5);
        assert_eq!(m.since(3).len(), 2);
        assert_eq!(m.since(9).len(), 0);
        let j = m.since_json(3);
        assert_eq!(j.get("from"), Some(&Json::Int(3)));
        assert_eq!(j.get("next"), Some(&Json::Int(5)));
        assert_eq!(j.get("entries").unwrap().as_arr().unwrap()[0].get("text"), Some(&Json::from("m3")));
        let mut big = MessageStore::from_tail(vec![e(0)]);
        for i in 1..(MESSAGES_KEPT as i32 + 10) {
            big.push(e(i));
        }
        assert_eq!(big.entries().len(), MESSAGES_KEPT);
        assert_eq!(big.first(), 10);
        // Asking for dropped messages starts at the oldest kept one.
        assert_eq!(big.since_json(0).get("from"), Some(&Json::Int(10)));
        assert_eq!(big.tail(2).len(), 2);
    }

    #[test]
    fn thresholds_come_from_the_data() {
        let t = armageddon_thresholds(ffh_data::global());
        for n in [10, 20, 30, 40, 50, 70, 75, 100] {
            assert!(t.contains(&n), "{t:?}");
        }
        assert!(t.windows(2).all(|w| w[0] < w[1]));
    }
}
