//! T7: fuzz (design §8). At every control point of a 100-turn bot game, a seeded mix of
//! commands is sent before the bot plays: well-formed ones taken from the actions the `group`
//! query enumerates (enabled or not, with the confirmation flag set or not), commands on foreign,
//! unknown and extreme ids, and malformed ones (wrong types, unknown names, out-of-range
//! indices) including well-typed values no exe path sends (extreme `i32` data: a specialist
//! change of `i32::MAX`, slider changes, mission flags and data, deal amounts). Then random
//! queries. No panic; a refused command answers `invalid`, `not_owner`, `gone` or `cannot` and
//! leaves the save bytes as they were (as does a confirmation and every query); every command
//! returns within a time limit. Invariants are checked every 5 turns.

mod common;

use std::time::{Duration, Instant};

use common::{bot_session, check, new_game};
use ffh_core::ids::*;
use ffh_core::rng::Rng;
use ffh_play::command::{CmdErr, CmdOk, Command, COMMAND_NAMES};
use ffh_play::query::{Query, QUERY_NAMES};
use ffh_play::{bot, jarr, jobj, Json, Session, Stop};

/// The slowest a command may take (a refused one is validated only; an applied one settles).
const LIMIT: Duration = Duration::from_secs(5);

const EXTREMES: [i64; 7] = [i32::MIN as i64, -2, -1, 0, 1, 2, i32::MAX as i64];

#[derive(Debug, Default)]
struct Tally {
    applied: usize,
    confirms: usize,
    refused: [usize; 4],
    decode_errors: usize,
    from_actions: usize,
    queries: usize,
    query_errors: usize,
    slowest: Duration,
}

struct Fuzz {
    rng: Rng,
    t: Tally,
}


fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    j.get(k).and_then(Json::as_arr).unwrap_or(&[])
}

impl Fuzz {
    fn n(&mut self, n: usize) -> usize {
        if n <= 1 {
            0
        } else {
            self.rng.get_i(n as i32, "fuzz") as usize
        }
    }

    fn pick<T: Clone>(&mut self, v: &[T]) -> T {
        v[self.n(v.len())].clone()
    }

    fn chance(&mut self, pct: usize) -> bool {
        self.n(100) < pct
    }

    /// An integer field: an extreme, a small value, something huge, or the wrong type.
    fn number(&mut self) -> Json {
        match self.n(10) {
            0..=4 => Json::Int(self.pick(&EXTREMES)),
            5..=7 => Json::Int(self.n(400) as i64 - 50),
            8 => self.pick(&[Json::Int(i64::MAX), Json::Int(i64::MIN), Json::Float(1.5), Json::Float(1e300)]),
            _ => self.pick(&[Json::Null, Json::from("x"), Json::Bool(true), jarr![1]]),
        }
    }

    /// A type reference: an index in or out of range, a name (real or not), or junk.
    fn type_ref(&mut self) -> Json {
        match self.n(8) {
            0..=3 => Json::Int(self.n(1200) as i64 - 100),
            4 => Json::Int(self.pick(&EXTREMES)),
            5 => self.pick(&[
                Json::from("UNIT_WARRIOR"),
                Json::from("MISSION_FORTIFY"),
                Json::from("TASK_HURRY"),
                Json::from("ORDER_TRAIN"),
                Json::from("COMMAND_DELETE"),
                Json::from("TECH_ARCHERY"),
                Json::from("TRADE_GOLD"),
                Json::from("COMMERCE_GOLD"),
            ]),
            6 => Json::from("NOT_A_TYPE"),
            _ => self.pick(&[Json::Null, Json::Float(0.5), Json::Bool(false)]),
        }
    }

    /// A unit id: ours, someone else's, unknown, or extreme.
    fn unit(&mut self, s: &Session) -> Json {
        let g = &s.g;
        let h = s.human;
        let ours: Vec<u32> = g.players[h].units.iter().map(|u| u.0).collect();
        let theirs: Vec<u32> = g.units.iter().filter(|(_, u)| u.owner != h).map(|(id, _)| id.0).collect();
        match self.n(6) {
            0 | 1 if !ours.is_empty() => Json::from(self.pick(&ours)),
            2 if !theirs.is_empty() => Json::from(self.pick(&theirs)),
            3 => Json::Int(1_000_000),
            4 => Json::Int(self.pick(&EXTREMES)),
            _ => self.number(),
        }
    }

    fn units(&mut self, s: &Session) -> Json {
        let n = self.n(4);
        let mut v: Vec<Json> = (0..n).map(|_| self.unit(s)).collect();
        if self.chance(10) && !v.is_empty() {
            v.push(v[0].clone()); // a duplicate
        }
        if self.chance(5) {
            return self.number();
        }
        Json::Arr(v)
    }

    fn city(&mut self, s: &Session) -> Json {
        let g = &s.g;
        let h = s.human;
        let ours: Vec<u32> = g.players[h].cities.iter().map(|c| c.0).collect();
        let theirs: Vec<u32> = g.cities.iter().filter(|(_, c)| c.owner != h).map(|(id, _)| id.0).collect();
        match self.n(5) {
            0 | 1 if !ours.is_empty() => Json::from(self.pick(&ours)),
            2 if !theirs.is_empty() => Json::from(self.pick(&theirs)),
            3 => Json::Int(99_999),
            _ => self.number(),
        }
    }

    fn flag(&mut self) -> Json {
        match self.n(6) {
            0..=2 => Json::Bool(false),
            3 | 4 => Json::Bool(true),
            _ => self.number(),
        }
    }

    fn item(&mut self) -> Json {
        jobj! { "item" => self.type_ref(), "data" => self.number() }
    }

    /// A command request in the protocol's shape with random fields. `end_turn`, `auto_moves` and
    /// `autoplay` are left out: applied, they end the human's turn (the bot plays after).
    fn random_command(&mut self, s: &Session) -> Json {
        let names: Vec<&str> = COMMAND_NAMES.iter().copied().filter(|c| !matches!(*c, "end_turn" | "auto_moves" | "autoplay")).collect();
        let c = if self.chance(3) { "fly" } else { self.pick(&names) };
        let mut j = jobj! { "c" => c };
        let fields: &[&str] = match c {
            "move" => &["units", "x", "y", "append", "declare_war"],
            "mission" => &["units", "mission", "data1", "data2", "flags", "append"],
            "do_command" => &["units", "command", "data1", "data2", "alt", "confirmed"],
            "join_group" => &["unit", "head"],
            "auto_mission" => &["unit"],
            "push_order" => &["city", "order", "data", "alt", "shift", "ctrl"],
            "pop_order" => &["city", "index"],
            "do_task" => &["city", "task", "data1", "data2", "option", "alt", "shift", "ctrl"],
            "research" => &["tech", "shift"],
            "update_civics" => &["civics"],
            "percent_change" => &["commerce", "change"],
            "convert" => &["religion"],
            "declare_war" => &["team"],
            "propose_deal" => &["to", "give", "get"],
            "event" => &["triggered", "event"],
            "vote" => &["vote", "choice"],
            "set_option" => &["option", "value"],
            _ => &[],
        };
        for &f in fields {
            if self.chance(5) {
                continue; // a missing field
            }
            let v = match f {
                "units" => self.units(s),
                "unit" | "head" => self.unit(s),
                "city" => self.city(s),
                "mission" | "command" | "order" | "task" | "tech" | "commerce" | "religion" | "event" | "option" => self.type_ref(),
                "data" | "data1" if self.chance(30) => self.type_ref(),
                "append" | "declare_war" | "alt" | "confirmed" | "shift" | "ctrl" | "value" => self.flag(),
                "civics" => Json::Arr((0..self.n(3)).map(|_| self.type_ref()).collect()),
                "give" | "get" => Json::Arr((0..self.n(3)).map(|_| self.item()).collect()),
                "choice" => self.pick(&[Json::from("YES"), Json::from("ABSTAIN"), Json::Int(-1), Json::Int(-6), Json::Int(3), Json::Int(i32::MAX as i64)]),
                "to" | "team" => Json::Int(self.n(12) as i64 - 1),
                "x" | "y" => {
                    if self.chance(60) {
                        Json::Int(self.n(80) as i64 - 5)
                    } else {
                        self.number()
                    }
                }
                _ => self.number(),
            };
            j.set(f, v);
        }
        j
    }

    /// One of the actions the `group` query lists for a random group of ours (enabled or not),
    /// sometimes with its confirmation flag or the append flag set.
    fn action(&mut self, s: &Session) -> Option<Json> {
        let view = s.query(&Query::View).unwrap();
        let groups = arr(&view, "groups");
        if groups.is_empty() {
            return None;
        }
        let units = self.pick(groups).get("units").cloned()?;
        let units: Vec<UnitId> = units.as_arr()?.iter().map(|u| UnitId(u.as_i64().unwrap() as u32)).collect();
        // Sometimes a part of the group.
        let units = if units.len() > 1 && self.chance(30) { units[..1].to_vec() } else { units };
        let gq = s.query(&Query::Group { units }).ok()?;
        let actions = arr(&gq, "actions");
        if actions.is_empty() {
            return None;
        }
        let mut cmd = self.pick(actions).get("cmd")?.clone();
        if cmd.get("confirmed").is_some() && self.chance(50) {
            cmd.set("confirmed", true);
        }
        if cmd.get("append").is_some() && self.chance(20) {
            cmd.set("append", true);
        }
        if self.chance(15) {
            // A target move far away, or a mission's data changed to an extreme.
            for k in ["data1", "data2", "x", "y"] {
                if cmd.get(k).is_some() && self.chance(40) {
                    let v = self.number();
                    cmd.set(k, v);
                }
            }
        }
        self.t.from_actions += 1;
        Some(cmd)
    }

    /// Sends `req`; checks the reply and that a refusal or confirmation changed nothing.
    fn send(&mut self, s: &mut Session, req: &Json) {
        let d = s.g.d;
        let cmd = match Command::from_json(d, req) {
            Ok(c) => c,
            Err(e) => {
                assert!(matches!(e, CmdErr::Invalid(_)), "{req}: decoding gave {e}");
                self.t.decode_errors += 1;
                return;
            }
        };
        // The canonical form decodes to the same command.
        assert_eq!(Command::from_json(d, &cmd.to_json()).as_ref(), Ok(&cmd), "{req}");
        let before = s.g.save();
        let t0 = Instant::now();
        let r = s.execute(&cmd);
        let took = t0.elapsed();
        assert!(took < LIMIT, "{req}: took {took:?}");
        self.t.slowest = self.t.slowest.max(took);
        match r {
            Ok(CmdOk::Applied { accepted: Some(false), .. }) => {
                self.t.refused[3] += 1;
                assert!(before == s.g.save(), "{req}: a declined deal changed the game");
            }
            Ok(CmdOk::Applied { .. }) => self.t.applied += 1,
            Ok(CmdOk::NeedsConfirm(_)) => {
                self.t.confirms += 1;
                assert!(before == s.g.save(), "{req}: a confirmation changed the game");
            }
            Err(e) => {
                let i = match e.code() {
                    "invalid" => 0,
                    "not_owner" => 1,
                    "gone" => 2,
                    "cannot" => 3,
                    other => panic!("{req}: refused with {other} at a control point"),
                };
                self.t.refused[i] += 1;
                assert!(before == s.g.save(), "{req}: refused ({e}) but the game changed");
            }
        }
    }

    /// A random query (some malformed); nothing may change.
    fn query(&mut self, s: &Session) {
        let q = self.pick(QUERY_NAMES);
        let mut j = jobj! { "q" => q };
        for k in ["units", "x", "y", "w", "h", "city", "with", "since", "kind", "oid", "flags", "layer"] {
            if self.chance(50) {
                let v = match k {
                    "units" => self.units(s),
                    "city" => self.city(s),
                    "kind" => self.pick(&[Json::from("unit"), Json::from("city"), Json::from("plot"), Json::from("UnitType"), Json::from("units"), Json::from("nope"), Json::Int(1)]),
                    "layer" => self.pick(&[Json::from("terrain"), Json::from("units"), Json::from("sky")]),
                    "x" | "y" | "w" | "h" if self.chance(50) => Json::Int(self.n(80) as i64 - 5),
                    _ => self.number(),
                };
                j.set(k, v);
            }
        }
        let before = s.g.save();
        self.t.queries += 1;
        match Query::from_json(&j).and_then(|q| s.query(&q)) {
            Ok(r) => assert!(Json::parse(&r.to_string()).is_ok(), "{j}: invalid JSON"),
            Err(e) => {
                assert!(matches!(e.code(), "invalid" | "not_owner" | "gone"), "{j}: {e}");
                self.t.query_errors += 1;
            }
        }
        assert!(before == s.g.save(), "{j}: a query changed the game");
    }
}

#[test]
fn t7_fuzzed_commands_are_refused_cleanly() {
    let ng = new_game(7);
    let (mut s, mut b) = bot_session(&ng, None);
    // The fuzz kills the human in about half of its games (deleted units, random wars); 7011
    // plays all 100 turns (7007 did before the P3 review changed the action lists).
    let mut f = Fuzz { rng: Rng::new(7011), t: Tally::default() };
    let until = s.g.game_turn() + 100;
    let mut last_checked = -1;
    let stop = bot::run(&mut s, &mut b, until, |s, _| {
        for _ in 0..12 {
            if !matches!(s.phase(), ffh_play::Phase::Human(_)) {
                break;
            }
            let req = match f.n(10) {
                0..=3 => f.action(s).unwrap_or_else(|| f.random_command(s)),
                _ => f.random_command(s),
            };
            f.send(s, &req);
        }
        for _ in 0..4 {
            f.query(s);
        }
        let turn = s.g.game_turn();
        if turn % 5 == 0 && turn != last_checked {
            last_checked = turn;
            s.read(check::check);
        }
    });
    assert!(matches!(stop, Stop::Human | Stop::Over), "{stop:?}");
    eprintln!("fuzz: {:?}, {stop:?} at turn {}", f.t, s.g.game_turn());
    let t = &f.t;
    assert!(t.applied > 50 && t.from_actions > 200, "{t:?}");
    assert!(t.refused.iter().all(|&n| n > 10) && t.decode_errors > 50, "every refusal kind should occur: {t:?}");
    assert!(t.queries > 300 && t.query_errors > 10, "{t:?}");
}
