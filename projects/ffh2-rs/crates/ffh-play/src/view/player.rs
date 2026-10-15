//! The empire screens (design §4.6): `techs` (the tech tree laid out by prerequisite depth),
//! `civics`, `religions` and `players` (the diplomacy overview of the players the human knows).

use ffh_core::ids::*;
use ffh_data::*;

use crate::json::Json;
use crate::view::{players_brief, Viewer};

/// The prerequisite depth of every tech: 0 without prerequisites, else one more than its
/// deepest prerequisite (the tech tree's columns).
pub fn tech_depths(d: &GameData) -> Vec<i32> {
    fn depth(d: &GameData, t: TechType, memo: &mut Vec<i32>, stack: &mut Vec<TechType>) -> i32 {
        if memo[t.idx()] >= 0 {
            return memo[t.idx()];
        }
        if stack.contains(&t) {
            return 0; // a cycle in the data: cut it
        }
        stack.push(t);
        let info = &d.techs[t];
        let deepest = info.prereq_and_techs.iter().chain(&info.prereq_or_techs).map(|&p| depth(d, p, memo, stack) + 1).max().unwrap_or(0);
        stack.pop();
        memo[t.idx()] = deepest;
        deepest
    }
    let mut memo = vec![-1; d.techs.len()];
    let mut stack = Vec::new();
    for t in d.techs.keys() {
        depth(d, t, &mut memo, &mut stack);
    }
    memo
}

/// `techs`: every tech with the human's state (has, can research, progress, cost, turns),
/// its prerequisites and its depth.
pub fn techs(v: &Viewer) -> Json {
    let g = v.g;
    let d = g.d;
    let h = v.h;
    let depths = tech_depths(d);
    let current = g.player_get_current_research(h);
    let out = d.techs.keys().map(|t| {
        let info = &d.techs[t];
        let has = g.teams[v.team].has_tech[t];
        let can = g.player_can_research(h, t, false);
        crate::jobj! {
            "tech" => Json::idx(t),
            "has" => has,
            "can_research" => can,
            "researching" => current == Some(t),
            "queued" => g.players[h].research_queue.iter().position(|&q| q == t),
            "progress" => g.teams[v.team].research_progress[t],
            "cost" => g.team_get_research_cost(v.team, t),
            "turns" => if can && !has { Json::from(g.player_get_research_turns_left(h, t, true)) } else { Json::Null },
            "era" => Json::opt_idx(info.era),
            "and" => Json::Arr(info.prereq_and_techs.iter().map(|&p| Json::idx(p)).collect()),
            "or" => Json::Arr(info.prereq_or_techs.iter().map(|&p| Json::idx(p)).collect()),
            "depth" => depths[t.idx()],
            "disabled" => info.disable,
        }
    });
    crate::jobj! {
        "current" => Json::opt_idx(current),
        "queue" => Json::Arr(g.players[h].research_queue.iter().map(|&t| Json::idx(t)).collect()),
        "rate" => g.player_calculate_research_rate(h, None),
        "techs" => Json::Arr(out.collect()),
    }
}

/// `civics`: each civic option with the current civic and every civic's availability, upkeep
/// and the anarchy a switch to it alone would cost. With `with` (civics to try, merged over the
/// current ones as `update_civics` merges them) also `proposed`: the merged civics, whether
/// `update_civics` would take them (every new civic `canDoCivics`, and `canRevolution` of the
/// set), and the set's anarchy and upkeep (the civics screen's totals, CvCivicsScreen.py).
pub fn civics(v: &Viewer, with: &[CivicType]) -> Json {
    let mut j = civics_screen(v);
    if !with.is_empty() {
        let g = v.g;
        let d = g.d;
        let h = v.h;
        let mut set = g.players[h].civics.clone();
        let mut valid = true;
        let mut seen = Vec::new();
        for &c in with {
            match d.civics[c].civic_option_type {
                Some(o) => {
                    if seen.contains(&o) || (set[o] != Some(c) && !g.player_can_do_civics(h, Some(c))) {
                        valid = false;
                    }
                    seen.push(o);
                    set[o] = Some(c);
                }
                None => valid = false,
            }
        }
        let changed = set != g.players[h].civics;
        valid = valid && changed && g.player_can_revolution(h, Some(&set));
        j.set(
            "proposed",
            crate::jobj! {
                "civics" => Json::Arr(set.iter().map(|&c| Json::opt_idx(c)).collect()),
                "changed" => changed,
                "valid" => valid,
                "anarchy" => if changed { g.player_get_civic_anarchy_length(h, &set) } else { 0 },
                "upkeep" => g.player_get_civic_upkeep(h, Some(&set), false),
            },
        );
    }
    j
}

fn civics_screen(v: &Viewer) -> Json {
    let g = v.g;
    let d = g.d;
    let h = v.h;
    let pl = &g.players[h];
    let options = d.civic_options.keys().map(|o| {
        let civics = d.civics.keys().filter(|&c| d.civics[c].civic_option_type == Some(o)).map(|c| {
            let mut changed = pl.civics.clone();
            changed[o] = Some(c);
            crate::jobj! {
                "civic" => Json::idx(c),
                "current" => pl.civics[o] == Some(c),
                "available" => g.player_can_do_civics(h, Some(c)),
                "upkeep" => g.player_get_single_civic_upkeep(h, Some(c), false),
                "anarchy" => if pl.civics[o] == Some(c) { 0 } else { g.player_get_civic_anarchy_length(h, &changed) },
            }
        });
        crate::jobj! { "option" => Json::idx(o), "current" => Json::opt_idx(pl.civics[o]), "civics" => Json::Arr(civics.collect()) }
    });
    crate::jobj! {
        "options" => Json::Arr(options.collect()),
        "can_revolution" => g.player_can_revolution(h, None),
        "revolution_timer" => pl.revolution_timer,
        "anarchy" => pl.anarchy_turns,
        "upkeep" => g.player_get_civic_upkeep(h, None, false),
    }
}

/// `religions`: the state religion, conversion, and every religion's founding and holy city (as
/// far as the human has revealed it).
pub fn religions(v: &Viewer) -> Json {
    let g = v.g;
    let d = g.d;
    let h = v.h;
    let pl = &g.players[h];
    let out = d.religions.keys().map(|r| {
        let holy = g.get_holy_city(r).filter(|&c| v.city_revealed(c));
        crate::jobj! {
            "religion" => Json::idx(r),
            "founded" => g.is_religion_founded(r),
            "holy_city" => holy,
            "our_holy_city" => g.player_has_holy_city(h, r),
            "our_cities" => pl.cities.iter().filter(|&&c| g.cities[c].has_religion[r]).count(),
            "can_convert" => g.player_can_convert(h, Some(r)),
        }
    });
    crate::jobj! {
        "state_religion" => Json::opt_idx(g.player_get_state_religion(h)),
        "can_convert_none" => g.player_can_convert(h, None),
        "conversion_timer" => pl.conversion_timer,
        "anarchy" => g.player_get_religion_anarchy_length(h),
        "religions" => Json::Arr(out.collect()),
    }
}

/// The items of a deal as `[{item, data}]`.
pub fn trades_json(list: &[ffh_core::structs::TradeData]) -> Json {
    Json::Arr(list.iter().map(|t| crate::jobj! { "item" => Json::idx(t.item_type), "data" => t.data }).collect())
}

/// `players`: the view's player list, plus each player's deals with the human and whether the
/// human can declare war on it.
pub fn players(v: &Viewer) -> Json {
    let g = v.g;
    let Json::Arr(mut list) = players_brief(v) else { unreachable!() };
    for j in &mut list {
        let p = PlayerId(j.get("id").and_then(Json::as_i64).unwrap_or(0) as u8);
        let t = g.players[p].team;
        let deals: Vec<Json> = g
            .deals
            .iter()
            .filter_map(|(id, deal)| {
                let (ours, theirs) = if (deal.first_player, deal.second_player) == (v.h, p) {
                    (&deal.first_trades, &deal.second_trades)
                } else if (deal.first_player, deal.second_player) == (p, v.h) {
                    (&deal.second_trades, &deal.first_trades)
                } else {
                    return None;
                };
                Some(crate::jobj! { "deal" => id, "we_give" => trades_json(ours), "they_give" => trades_json(theirs), "turn" => deal.initial_game_turn })
            })
            .collect();
        j.set("deals", Json::Arr(deals));
        j.set("can_declare_war", t != v.team && g.team_can_declare_war(v.team, t));
    }
    crate::jobj! { "me" => v.h, "players" => Json::Arr(list) }
}
