//! Unit queries (design §4.6): `group` (unit details and the actions the selection can take,
//! enumerated as the exe's `canHandleAction` does), `path` (a preview through the interface
//! finder) and `combat` (attack odds and the strength breakdown, C++ `setCombatPlotHelp`).
//!
//! A selection is a list of the human's units on one plot (as for commands). The exe checks an
//! action against the selection (a `CvSelectionGroup` of the selected units), so do these, with
//! the ports in [`crate::selection`]. Each action carries the command that performs it (`cmd`,
//! the canonical request form), so a client (or the fuzz test) can send it as is.

use ffh_core::ids::*;
use ffh_core::path::PathFinder;
use ffh_core::unit::CombatDetails;
use ffh_data::*;

use crate::command::Command;
use crate::json::Json;
use crate::query::{invalid, QueryErr};
use crate::selection::{self, InterfaceMode};
use crate::view::map::plot_at;
use crate::view::{unit_json, Viewer};

/// A selection of the human's units: non-empty, distinct, its own, on one plot.
pub fn check_selection(v: &Viewer, units: &[UnitId]) -> Result<(), QueryErr> {
    let g = v.g;
    if units.is_empty() {
        return Err(invalid("field `units`: empty selection"));
    }
    for (i, &u) in units.iter().enumerate() {
        match g.units.get(u) {
            None => return Err(QueryErr::Gone),
            Some(unit) if unit.owner != v.h => return Err(QueryErr::NotOwner),
            Some(_) => {}
        }
        if units[..i].contains(&u) {
            return Err(invalid(format!("field `units`: unit {} twice", u.0)));
        }
    }
    let plot = g.unit_plot(units[0]);
    if units.iter().any(|&u| g.unit_plot(u) != plot) {
        return Err(invalid("field `units`: the selected units are on different plots"));
    }
    Ok(())
}

/// The missions whose data is a plot the client picks: the exe sends them from an interface mode
/// ([`selection::InterfaceMode`]), never from a plain mission button.
pub const TARGET_MISSIONS: [MissionType; 8] = [
    MissionType::MISSION_MOVE_TO,
    MissionType::MISSION_ROUTE_TO,
    MissionType::MISSION_AIRLIFT,
    MissionType::MISSION_NUKE,
    MissionType::MISSION_RECON,
    MissionType::MISSION_PARADROP,
    MissionType::MISSION_AIRBOMB,
    MissionType::MISSION_RANGE_ATTACK,
];

fn mission_cmd(units: &[UnitId], m: MissionType, data1: i32, data2: i32) -> Json {
    Command::Mission { units: units.to_vec(), mission: m, data1, data2, flags: 0, append: false }.to_json()
}

fn command_cmd(units: &[UnitId], c: CommandType, data1: i32, data2: i32) -> Json {
    Command::DoCommand { units: units.to_vec(), command: c, data1, data2, alt: false, confirmed: false }.to_json()
}

/// The actions of a selection: every action the exe would show (`canHandleAction` with
/// `bTestVisible`), `enabled` when it can be done now. Kinds: `mission` (no data or a typed
/// one: build, spread, join, construct, lead), `target` (an interface mode, `mode`, whose
/// mission needs a plot: `targets` lists the valid plots in range, or `any` for go-to and
/// route-to), `command`, `spell`, `promotion`, `upgrade` (with `price`), `load` (with the
/// `transports`).
///
/// A `target` action's `cmd` carries its first listed target, or the selection's own plot as a
/// placeholder (with `any`, or when no target is in range): the client puts the picked plot in
/// (`x`, `y` of a `move`, `data1`, `data2` of a `mission`). GO_TO's command is `move`, as the exe
/// moves through `selectionListMove` (the declare-war question).
pub fn actions(v: &Viewer, units: &[UnitId]) -> Vec<Json> {
    let g = v.g;
    let d = g.d;
    let plot = selection::plot(g, units);
    let (px, py) = g.map.xy(plot);
    let mut out = Vec::new();
    let mut push = |kind: &str, enabled: bool, cmd: Json, extra: Json| {
        let mut j = crate::jobj! { "kind" => kind, "enabled" => enabled };
        if let Json::Obj(m) = extra {
            for (k, x) in m {
                j.set(&k, x);
            }
        }
        j.set("cmd", cmd);
        out.push(j);
    };
    let can = |m: MissionType, d1: i32, d2: i32, visible: bool| selection::can_start_mission(g, units, m, d1, d2, plot, visible);

    // Missions.
    for i in 0..MissionType::NUM {
        let m = MissionType::from_usize(i);
        if !selection::is_player_mission(m) || m == MissionType::MISSION_ESPIONAGE {
            continue;
        }
        let typed: Option<usize> = match m {
            MissionType::MISSION_BUILD => Some(d.builds.len()),
            MissionType::MISSION_SPREAD => Some(d.religions.len()),
            MissionType::MISSION_SPREAD_CORPORATION => Some(d.corporations.len()),
            MissionType::MISSION_JOIN => Some(d.specialists.len()),
            MissionType::MISSION_CONSTRUCT => Some(d.buildings.len()),
            _ => None,
        };
        if TARGET_MISSIONS.contains(&m) {
            continue; // the interface modes, below
        }
        match (m, typed) {
            (MissionType::MISSION_LEAD, _) => {
                // The LEADUNIT popup: one button per unit the great commander can lead.
                for &t in &g.map.plots[plot].units {
                    if g.units[t].owner == v.h && !units.contains(&t) && can(m, t.0 as i32, -1, false) {
                        push("mission", true, mission_cmd(units, m, t.0 as i32, -1), crate::jobj! { "mission" => Json::idx(m), "data1" => t });
                    }
                }
            }
            (_, Some(n)) => {
                for k in 0..n {
                    let k = k as i32;
                    if !can(m, k, -1, true) {
                        continue;
                    }
                    let enabled = can(m, k, -1, false);
                    let mut extra = crate::jobj! { "mission" => Json::idx(m), "data1" => k };
                    if m == MissionType::MISSION_BUILD {
                        let b = BuildType::from_usize(k as usize);
                        let (mut now, mut then) = (0, 0);
                        for &u in units {
                            if g.unit_get_build_type(u) != Some(b) {
                                now += g.unit_work_rate(u, false);
                                then += g.unit_work_rate(u, true);
                            }
                        }
                        extra.set("turns", g.plot_get_build_turns_left(plot, b, now, then));
                        extra.set("cost", g.player_get_build_cost(v.h, plot, b));
                    }
                    push("mission", enabled, mission_cmd(units, m, k, -1), extra);
                }
            }
            _ => {
                if can(m, -1, -1, true) {
                    push("mission", can(m, -1, -1, false), mission_cmd(units, m, -1, -1), crate::jobj! { "mission" => Json::idx(m) });
                }
            }
        }
    }

    // Interface modes (the exe's go-to, route-to, range attack, air ... buttons): shown, and so
    // enabled, when `canDoInterfaceMode`; the plot is the client's pick, among those that pass
    // `canDoInterfaceModeAt` and the mission check (any plot for go-to and route-to).
    for mode in InterfaceMode::ALL {
        if !selection::can_do_interface_mode(g, units, mode) {
            continue;
        }
        let m = mode.mission();
        let ok = |t: PlotIdx| {
            let (tx, ty) = g.map.xy(t);
            selection::can_do_interface_mode_at(g, units, mode, t) && can(m, tx, ty, false)
        };
        let targets: Option<Vec<(i32, i32)>> = match mode {
            InterfaceMode::GoTo | InterfaceMode::RouteTo => None,
            InterfaceMode::Airlift => Some(g.players[v.h].cities.iter().map(|&c| g.city_plot(c)).filter(|&t| ok(t)).map(|t| g.map.xy(t)).collect()),
            _ => {
                let range = units.iter().map(|&u| g.unit_air_range(u).max(g.unit_nuke_range(u)).max(g.unit_get_drop_range(u))).max().unwrap_or(0).max(0);
                let mut out = Vec::new();
                for dy in -range..=range {
                    for dx in -range..=range {
                        if let Some(t) = g.plot_xy(px, py, dx, dy) {
                            if v.plot_revealed(t) && ok(t) && !out.contains(&g.map.xy(t)) {
                                out.push(g.map.xy(t));
                            }
                        }
                    }
                }
                Some(out)
            }
        };
        let (tx, ty) = targets.as_ref().and_then(|t| t.first().copied()).unwrap_or((px, py));
        let cmd = if mode == InterfaceMode::GoTo {
            Command::Move { units: units.to_vec(), x: tx, y: ty, append: false, declare_war: None }.to_json()
        } else {
            mission_cmd(units, m, tx, ty)
        };
        let mut extra = crate::jobj! { "mode" => mode.name(), "mission" => Json::idx(m) };
        match targets {
            None => {
                extra.set("any", true);
            }
            Some(t) => {
                extra.set("targets", Json::Arr(t.into_iter().map(|(x, y)| crate::jarr![x, y]).collect()));
            }
        }
        push("target", true, cmd, extra);
    }

    // Commands without data.
    for i in 0..CommandType::NUM {
        let c = CommandType::from_usize(i);
        if matches!(
            c,
            CommandType::COMMAND_PROMOTION
                | CommandType::COMMAND_UPGRADE
                | CommandType::COMMAND_AUTOMATE
                | CommandType::COMMAND_AUTOMATE_SPELL
                | CommandType::COMMAND_HOTKEY
                | CommandType::COMMAND_CAST
                | CommandType::COMMAND_LOAD_UNIT
        ) {
            continue;
        }
        if !selection::can_do_command(g, units, c, -1, -1, true) {
            continue;
        }
        let enabled = selection::can_do_command(g, units, c, -1, -1, false);
        let mut extra = crate::jobj! { "command" => Json::idx(c), "confirm" => d.commands[c].confirm_command, "all" => d.commands[c].all };
        if c == CommandType::COMMAND_LOAD {
            let transports = if enabled { selection::load_targets(g, units) } else { Vec::new() };
            extra.set("transports", transports);
            push("load", enabled, command_cmd(units, c, -1, -1), extra);
        } else {
            push("command", enabled, command_cmd(units, c, -1, -1), extra);
        }
    }

    // Automation (the exe's automate buttons, one per CvAutomateInfo with `bVisible`): shown when
    // `canDoCommand(COMMAND_AUTOMATE, eAutomate, bTestVisible)`. FfH's automatic terraforming
    // (Python `AI_MageTurn`) has no AI here, so it is not offered.
    for a in d.automates.keys() {
        if a == AutomateType::AUTOMATE_TERRAFORMING {
            continue;
        }
        let k = a.idx() as i32;
        if !selection::can_do_command(g, units, CommandType::COMMAND_AUTOMATE, k, -1, true) {
            continue;
        }
        let enabled = selection::can_do_command(g, units, CommandType::COMMAND_AUTOMATE, k, -1, false);
        push("automate", enabled, command_cmd(units, CommandType::COMMAND_AUTOMATE, k, -1), crate::jobj! { "command" => Json::idx(CommandType::COMMAND_AUTOMATE), "automate" => Json::idx(a) });
    }

    // Spells.
    for s in d.spells.keys() {
        let k = s.idx() as i32;
        if !selection::can_do_command(g, units, CommandType::COMMAND_CAST, k, -1, true) {
            continue;
        }
        let enabled = selection::can_do_command(g, units, CommandType::COMMAND_CAST, k, -1, false);
        let info = &d.spells[s];
        let help = crate::names::text(d, &info.base.help);
        push(
            "spell",
            enabled,
            command_cmd(units, CommandType::COMMAND_CAST, k, -1),
            crate::jobj! { "spell" => Json::idx(s), "global" => info.global, "causes_war" => info.causes_war, "help" => if help == info.base.help { "" } else { help } },
        );
    }

    // Promotions.
    for p in d.promotions.keys() {
        let k = p.idx() as i32;
        if selection::can_do_command(g, units, CommandType::COMMAND_PROMOTION, k, -1, true) {
            let enabled = selection::can_do_command(g, units, CommandType::COMMAND_PROMOTION, k, -1, false);
            push("promotion", enabled, command_cmd(units, CommandType::COMMAND_PROMOTION, k, -1), crate::jobj! { "promotion" => Json::idx(p) });
        }
    }

    // Upgrades (with the head's price).
    for t in d.units.keys() {
        let k = t.idx() as i32;
        if selection::can_do_command(g, units, CommandType::COMMAND_UPGRADE, k, -1, true) {
            let enabled = selection::can_do_command(g, units, CommandType::COMMAND_UPGRADE, k, -1, false);
            let price: i32 = units.iter().filter(|&&u| g.unit_can_upgrade(u, Some(t), true)).map(|&u| g.unit_upgrade_price(u, t)).sum();
            push("upgrade", enabled, command_cmd(units, CommandType::COMMAND_UPGRADE, k, -1), crate::jobj! { "unit" => Json::idx(t), "price" => price });
        }
    }
    out
}

/// One of the human's units in detail (the unit panel).
pub fn unit_details(v: &Viewer, u: UnitId) -> Json {
    let g = v.g;
    let d = g.d;
    let unit = &g.units[u];
    let info = &d.units[unit.unit_type];
    let mut j = unit_json(v, u);
    j.set("name", if unit.name.is_empty() { Json::Null } else { Json::from(crate::names::text(d, &unit.name)) });
    j.set("strength", g.unit_base_combat_str(u));
    j.set("defense_strength", g.unit_base_combat_str_defense(u));
    j.set("domain", Json::idx(g.unit_get_domain_type(u)));
    j.set("unit_combat", Json::opt_idx(info.unit_combat_type));
    j.set("race", Json::opt_idx(unit.race));
    j.set("religion", Json::opt_idx(unit.religion));
    j.set("promotions", Json::Arr(d.promotions.keys().filter(|&p| unit.has_promotion[p]).map(Json::idx).collect()));
    j.set("first_strikes", crate::jarr![g.unit_first_strikes(u), g.unit_chance_first_strikes(u)]);
    j.set("withdrawal", g.unit_withdrawal_probability(u));
    j.set("collateral", g.unit_collateral_damage(u));
    j.set("cargo", Json::Arr(g.map.plots[g.unit_plot(u)].units.iter().filter(|&&c| g.units[c].transport_unit == Some(u)).map(|&c| Json::from(c)).collect()));
    j.set("cargo_space", unit.cargo_capacity);
    j.set("has_casted", unit.has_casted);
    j
}

/// `group {units}`: the selected units in detail and the selection's actions.
pub fn group(v: &Viewer, units: &[UnitId]) -> Result<Json, QueryErr> {
    check_selection(v, units)?;
    let g = v.g;
    let plot = selection::plot(g, units);
    let (x, y) = g.map.xy(plot);
    let mirrored = selection::mirrors_group(g, units);
    Ok(crate::jobj! {
        "units" => Json::Arr(units.iter().map(|&u| unit_details(v, u)).collect()),
        "plot" => crate::jarr![x, y],
        "group" => mirrored,
        // The group waits for orders (`pending.ready_groups`).
        "ready" => mirrored.is_some_and(|grp| g.group_ready_to_move(grp, true) && !crate::decision::is_ai_group(g, grp)),
        "busy" => selection::is_busy(g, units),
        "actions" => Json::Arr(actions(v, units)),
    })
}

/// `path {units,x,y,flags}`: the interface finder's path for the selection's group (the head's
/// group when the selection is not a whole group: `approximate`), with each node's turn of
/// arrival (1 = this turn) and moves left, the war it would declare and whether the last step
/// attacks.
pub fn path(v: &Viewer, finder: &mut PathFinder, units: &[UnitId], x: i32, y: i32, flags: i32) -> Result<Json, QueryErr> {
    check_selection(v, units)?;
    let to = plot_at(v, x, y)?;
    let g = v.g;
    let mirrored = selection::mirrors_group(g, units);
    let Some(grp) = mirrored.or(g.units[units[0]].group) else { return Err(invalid("the unit has no group")) };
    let from = selection::plot(g, units);
    let (fx, fy) = g.map.xy(from);
    finder.set_group(Some(grp));
    let ok = finder.generate_path(g, fx, fy, x, y, false, flags, false);
    let mut nodes = Vec::new();
    if ok {
        let mut node = finder.last_node().cloned();
        while let Some(n) = node {
            nodes.push(crate::jobj! { "x" => n.x, "y" => n.y, "turn" => n.data2, "moves" => n.data1 });
            node = finder.parent_of(&n).cloned();
        }
        nodes.reverse();
    }
    let war = units.iter().find_map(|&u| g.unit_get_declare_war_move(u, to));
    let attack = v.plot_visible(to) && units.iter().any(|&u| g.plot_is_visible_enemy_defender(to, u));
    Ok(crate::jobj! {
        "ok" => ok,
        "turns" => if ok { finder.path_turns().map(Json::from).unwrap_or(Json::Null) } else { Json::Null },
        "nodes" => Json::Arr(nodes),
        "declare_war_team" => war,
        "attack" => attack,
        "approximate" => mirrored.is_none(),
    })
}

/// The attacker the selection would send against `plot`: the group's (C++
/// `AI_getBestGroupAttacker`, the human's rule: the best odds, the first of equals); for a
/// selection that is not a whole group, the selected unit with the best exact odds.
fn best_attacker(v: &Viewer, units: &[UnitId], plot: PlotIdx, force: bool) -> Option<UnitId> {
    let g = v.g;
    if let Some(grp) = selection::mirrors_group(g, units) {
        return g.group_ai_get_best_group_attacker(grp, plot, false, force, false).0;
    }
    let mut best: Option<(i32, UnitId)> = None;
    for &u in units {
        if g.unit_is_dead(u) {
            continue;
        }
        let can_attack = if g.unit_get_domain_type(u) == DomainType::DOMAIN_AIR { g.unit_can_air_attack(u) } else { g.unit_can_attack(u) };
        if !can_attack || !(force || g.unit_can_move(u)) || !(force || g.unit_can_move_into(u, plot, true, false, false)) {
            continue;
        }
        let odds = g.plot_get_best_defender(plot, None, Some(v.h), Some(u), false, false, false).map_or(1000, |def| g.get_combat_odds(u, def));
        let value = odds.max(1);
        if best.is_none_or(|(b, _)| value > b) {
            best = Some((value, u));
        }
    }
    best.map(|(_, u)| u)
}

fn details_json(c: &CombatDetails) -> Json {
    let mut j = crate::jobj! {};
    for (k, x) in [
        ("extra_combat_percent", c.extra_combat_percent),
        ("animal_combat_modifier_ta", c.animal_combat_modifier_ta),
        ("animal_combat_modifier_aa", c.animal_combat_modifier_aa),
        ("barbarian_combat_modifier_tb", c.barbarian_combat_modifier_tb),
        ("barbarian_combat_modifier_ab", c.barbarian_combat_modifier_ab),
        ("plot_defense_modifier", c.plot_defense_modifier),
        ("fortify_modifier", c.fortify_modifier),
        ("city_defense_modifier", c.city_defense_modifier),
        ("hills_attack_modifier", c.hills_attack_modifier),
        ("hills_defense_modifier", c.hills_defense_modifier),
        ("feature_attack_modifier", c.feature_attack_modifier),
        ("feature_defense_modifier", c.feature_defense_modifier),
        ("terrain_attack_modifier", c.terrain_attack_modifier),
        ("terrain_defense_modifier", c.terrain_defense_modifier),
        ("city_attack_modifier", c.city_attack_modifier),
        ("domain_defense_modifier", c.domain_defense_modifier),
        ("city_barbarian_defense_modifier", c.city_barbarian_defense_modifier),
        ("class_defense_modifier", c.class_defense_modifier),
        ("class_attack_modifier", c.class_attack_modifier),
        ("combat_modifier_t", c.combat_modifier_t),
        ("combat_modifier_a", c.combat_modifier_a),
        ("domain_modifier_a", c.domain_modifier_a),
        ("domain_modifier_t", c.domain_modifier_t),
        ("animal_combat_modifier_a", c.animal_combat_modifier_a),
        ("animal_combat_modifier_t", c.animal_combat_modifier_t),
        ("river_attack_modifier", c.river_attack_modifier),
        ("amphib_attack_modifier", c.amphib_attack_modifier),
        ("kamikaze_modifier", c.kamikaze_modifier),
        ("modifier_total", c.modifier_total),
        ("base_combat_str", c.base_combat_str),
        ("combat", c.combat),
        ("max_combat_str", c.max_combat_str),
        ("curr_hit_points", c.curr_hit_points),
        ("max_hit_points", c.max_hit_points),
        ("curr_combat_str", c.curr_combat_str),
    ] {
        j.set(k, x);
    }
    j
}

/// `combat {units,x,y}`: the attack preview of C++ `setCombatPlotHelp` on a plot the human sees:
/// the attacker the selection would send, the defender it would meet, the odds of winning (per
/// mille), of retreating (per mille), both current strengths (x100), and each side's strength
/// breakdown (`CombatDetails` from `maxCombatStr`).
pub fn combat(v: &Viewer, units: &[UnitId], x: i32, y: i32) -> Result<Json, QueryErr> {
    check_selection(v, units)?;
    let plot = plot_at(v, x, y)?;
    let g = v.g;
    let none = || crate::jobj! { "valid" => false };
    if !v.plot_visible(plot) {
        return Ok(none());
    }
    let first = best_attacker(v, units, plot, false);
    // The selection's domain (its head's) decides where it can fight.
    let valid = match g.unit_get_domain_type(units[0]) {
        DomainType::DOMAIN_SEA => g.plot_is_water(plot),
        DomainType::DOMAIN_AIR => true,
        DomainType::DOMAIN_LAND => !g.plot_is_water(plot) || first.is_some_and(|a| g.unit_can_move_all_terrain(a) || g.unit_is_boarding(a)),
        _ => false,
    };
    if !valid {
        return Ok(none());
    }
    let Some(att) = first.or_else(|| best_attacker(v, units, plot, true)) else { return Ok(none()) };
    let def = if g.unit_is_always_hostile(att, Some(plot)) {
        g.plot_get_best_defender(plot, None, Some(v.h), Some(att), false, false, false)
    } else {
        g.plot_get_best_defender(plot, None, Some(v.h), Some(att), false, g.unit_get_declare_war_move(att, plot).is_none(), false)
    };
    let Some(def) = def.filter(|&d| d != att && v.unit_visible(d) && g.unit_can_defend(d, Some(plot)) && g.unit_can_attack_unit(att, d)) else {
        return Ok(none());
    };
    let air = g.unit_get_domain_type(att) == DomainType::DOMAIN_AIR;
    let odds = (!air).then(|| g.get_combat_odds(att, def));
    let max_hp = g.d.defines.max_hit_points;
    let withdraw = odds.map(|o| {
        let mut w = 0;
        if g.units[att].combat_limit < max_hp {
            w += 100 * o;
        }
        w += g.unit_withdrawal_probability(att).min(100) * (1000 - o);
        w / 100
    });
    let mut ad = CombatDetails::default();
    let mut dd = CombatDetails::default();
    let att_str = if air { g.unit_air_curr_combat_str(att, Some(def)) } else { g.unit_curr_combat_str(att, None, Some(def), Some(&mut ad)) };
    let def_str = g.unit_curr_combat_str(def, Some(plot), Some(att), Some(&mut dd));
    Ok(crate::jobj! {
        "valid" => true,
        "attacker" => att,
        "defender" => def,
        "defender_owner" => v.unit_owner(def),
        "defender_type" => Json::idx(g.units[def].unit_type),
        "odds" => odds,
        "withdraw" => withdraw,
        "attacker_strength" => att_str,
        "defender_strength" => def_str,
        "attacker_first_strikes" => crate::jarr![g.unit_first_strikes(att), g.unit_chance_first_strikes(att)],
        "defender_first_strikes" => crate::jarr![g.unit_first_strikes(def), g.unit_chance_first_strikes(def)],
        "collateral" => g.unit_collateral_damage(att),
        "attacker_details" => details_json(&ad),
        "defender_details" => details_json(&dd),
        "declare_war_team" => g.unit_get_declare_war_move(att, plot),
    })
}
