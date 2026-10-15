//! Automated groups of human players (`Controller::update_automated_group`): C++
//! `CvSelectionGroupAI::AI_update` for a human group the AI moves, with the automation branch of
//! `CvUnitAI::AI_update`. Two kinds of group come here:
//!
//! * **Automated** (`COMMAND_AUTOMATE`): `AUTOMATE_BUILD` (workers and work boats improve the
//!   empire), `AUTOMATE_NETWORK` (roads and resource connections only), `AUTOMATE_CITY` (the
//!   plots of the worker's city), `AUTOMATE_EXPLORE` (land and sea explorers), `AUTOMATE_RELIGION`
//!   (missionaries spread their religion). Each reuses the AI's own routine (`workers`,
//!   `units::explore_target`, `naval::sea_explore_target`, `units::missionary`) with the
//!   automation's restrictions; the human's `PLAYEROPTION_SAFE_AUTOMATION` keeps every improved
//!   plot as it is (C++ `AI_bestPlotBuild`). FfH's `AUTOMATE_TERRAFORMING` (Python
//!   `AI_MageTurn`) is not ported: such a group skips its turn.
//! * **FfH AI-controlled** (a unit with an `bAIControl` promotion, e.g. enraged): Sephi's block
//!   of `CvUnitAI::AI_update` -- a stack lets the unit go (`joinGroup(NULL)`), a defensive-only
//!   unit gets its control back (`changeAIControl(-1)`), any other attacks what it can reach
//!   (`AI_anyAttack`, `AI_barbAttackMove`: the AI's barbarian routine).
//!
//! As the C++ loop does (`while (readyToMove())`), the group gets decisions until it has orders,
//! and it never leaves ready: `MISSION_SKIP` is the fallback. The return value is C++'s: the group
//! is busy (a mission under way), so the human's `AI_unitUpdate` waits for the next slice. The AI
//! players never come here: their groups are played by `units::play`.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::units::{alive, move_to, nearest_city, skip};
use crate::view::View;
use crate::Ai;

/// Most decisions in one call (C++ gives up after 100 and finishes the head's moves).
const MAX_DECISIONS: usize = 8;

/// `Controller::update_automated_group` of the AI: moves human group `grp` if the AI controls
/// it. True when the group is busy afterwards.
pub fn update(_ai: &Ai, g: &mut Game, grp: GroupId) -> bool {
    if !alive(g, grp) {
        return false;
    }
    // FfH: `if (!AI_isControlled() && !isAIControl()) return false;`
    if !g.group_ai_is_controlled(grp) && !g.group_is_ai_control(grp) {
        return false;
    }
    if g.groups[grp].force_update {
        // isForceUpdate: the group re-plans (CvSelectionGroup::doTurn sets it each turn for a
        // group not on a mission), and a stack attack in progress is cancelled.
        g.group_clear_mission_queue(grp);
        g.group_set_activity_type(grp, ActivityType::ACTIVITY_AWAKE);
        g.groups[grp].force_update = false;
        g.groups[grp].ai_group_attack = false;
    }
    let owner = g.groups[grp].owner;
    let mut view: Option<View> = None;
    for _ in 0..MAX_DECISIONS {
        if !alive(g, grp) || !g.group_ready_to_move(grp, false) {
            break;
        }
        let Some(head) = g.group_get_head_unit(grp) else { break };
        if g.units[head].death_delay {
            break;
        }
        if view.is_none() {
            view = Some(View::new(g, owner));
        }
        let v = view.as_mut().expect("built above");
        let wait = act(g, v, grp, head);
        crate::trace!(g, owner, "automated group {:?} ({:?}) at {:?}: missions {:?}, activity {:?}, wait {}", grp, g.groups.get(grp).and_then(|x| x.automate_type), g.map.xy(g.unit_plot(head)), g.groups.get(grp).map(|x| x.mission_queue.iter().map(|m| m.mission_type).collect::<Vec<_>>()), g.groups.get(grp).map(|x| x.activity_type), wait);
        if !alive(g, grp) || g.group_do_delayed_death(grp) {
            return true;
        }
        if wait {
            break;
        }
    }
    // Never left ready: the turn of a group the AI still moves (not the rest of a stack an
    // AI-controlled unit left) ends with a mission, else its moves end (C++'s last resort,
    // `finishMoves`).
    if alive(g, grp) && (g.group_ai_is_controlled(grp) || g.group_is_ai_control(grp)) && g.group_ready_to_move(grp, true) {
        if g.groups[grp].mission_queue.is_empty() {
            skip(g, grp);
        }
        if alive(g, grp) && g.group_ready_to_move(grp, true) {
            for u in g.groups[grp].units.clone() {
                if g.units.contains(u) {
                    g.unit_finish_moves(u);
                }
            }
        }
    }
    alive(g, grp) && (g.group_is_busy(grp) || g.group_is_cargo_busy(grp))
}

/// One decision for the group of `head` (C++ `CvUnitAI::AI_update` for an AI-moved human
/// group). True: stop for this slice (the group was split, or is no longer automated).
fn act(g: &mut Game, v: &mut View, grp: GroupId, head: UnitId) -> bool {
    // FfH (Kael): a unit that can neither move nor attack into any plot around it skips.
    let here = g.unit_plot(head);
    let (x, y) = g.map.xy(here);
    let can_step = (-1..=1).any(|dx| (-1..=1).any(|dy| g.plot_xy(x, y, dx, dy).is_some_and(|q| g.unit_can_move_or_attack_into(head, q, false))));
    if !can_step {
        skip(g, grp);
        return false;
    }
    // A land unit at sea waits for its ship.
    if g.unit_get_domain_type(head) == DomainType::DOMAIN_LAND && g.plot_is_water(here) && !g.unit_can_move_all_terrain(head) {
        skip(g, grp);
        return false;
    }

    // FfH AI control (Sephi's BUGFIX block).
    if g.unit_is_ai_control(head) {
        if g.groups[grp].units.len() > 1 {
            g.unit_join_group(head, None, false);
            return true;
        }
        if g.unit_is_only_defensive(head) {
            // Defensive-only units do not stay AI-controlled.
            g.unit_change_ai_control(head, -1);
            skip(g, grp);
            return false;
        }
        // AI_anyAttack(3, 0), AI_anyAttack(10, 0), AI_barbAttackMove.
        crate::units::barbarian(g, v, grp, head);
        return false;
    }
    let Some(automate) = g.groups[grp].automate_type else {
        // AI-controlled through another unit of the stack: those units leave it, and the rest
        // is the human's again.
        for u in g.groups[grp].units.clone() {
            if g.units.contains(u) && g.unit_is_ai_control(u) {
                g.unit_join_group(u, None, false);
            }
        }
        return true;
    };

    let ai_type = g.units[head].ai_type;
    match automate {
        AutomateType::AUTOMATE_BUILD => match ai_type {
            Some(UnitAIType::UNITAI_WORKER) => worker(g, v, grp, head, Restrict::None),
            Some(UnitAIType::UNITAI_WORKER_SEA) => crate::workers::worker(g, v, grp, head, true),
            _ => skip(g, grp),
        },
        AutomateType::AUTOMATE_NETWORK => worker(g, v, grp, head, Restrict::Network),
        AutomateType::AUTOMATE_CITY => {
            // The city whose plot the worker stands on, else the nearest (C++ `findCity`).
            let p = v.p;
            let city = g.map.plots[here].working_city.filter(|&c| g.map.plots[here].owner == Some(p) && g.cities[c].owner == p).or_else(|| nearest_city(g, p, here));
            match city {
                Some(c) => worker(g, v, grp, head, Restrict::City(c)),
                None => skip(g, grp),
            }
        }
        AutomateType::AUTOMATE_EXPLORE => explore(g, v, grp, head),
        AutomateType::AUTOMATE_RELIGION => {
            if ai_type == Some(UnitAIType::UNITAI_MISSIONARY) {
                crate::units::missionary(g, v, grp, head);
            } else {
                skip(g, grp);
            }
        }
        // AUTOMATE_TERRAFORMING: FfH's Python terraformer (AI_MageTurn) is not ported.
        _ => skip(g, grp),
    }
    // "if no longer automated, then we want to bail"
    !alive(g, grp) || !g.group_is_automated(grp)
}

/// What an automated worker may build.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Restrict {
    /// AUTOMATE_BUILD: anything the AI's workers would.
    None,
    /// AUTOMATE_NETWORK: routes, and improvements that connect a resource.
    Network,
    /// AUTOMATE_CITY: the plots this city works (and roads among them).
    City(CityId),
}

fn worker(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, restrict: Restrict) {
    let p = v.p;
    let safe = g.players[p].options[PlayerOptionType::PLAYEROPTION_SAFE_AUTOMATION];
    let ruins = g.d.defines.ruins_improvement;
    let allow = move |g: &Game, q: PlotIdx, b: BuildType| -> bool {
        let pl = &g.map.plots[q];
        // C++ AI_bestPlotBuild: with safe automation an improved plot (ruins aside) is left alone.
        if safe && pl.improvement_type.is_some_and(|i| i.idx() as i32 != ruins) {
            return false;
        }
        let info = &g.d.builds[b];
        match restrict {
            Restrict::None => true,
            Restrict::Network => {
                (info.route.is_some() && info.improvement.is_none())
                    || pl.bonus_type.is_some_and(|bonus| info.improvement.is_some_and(|imp| g.d.improvements[imp].improvement_bonus[bonus].bonus_trade))
            }
            Restrict::City(c) => pl.working_city == Some(c),
        }
    };
    crate::workers::worker_with(g, v, grp, u, false, &allow);
}

/// AUTOMATE_EXPLORE (C++ `AI_exploreMove` / `AI_exploreSeaMove` for a human: no attacks): the
/// AI's next exploration goal; with nothing left to explore, home to a city, where it waits a
/// turn and looks again (the group stays automated).
fn explore(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) {
    let here = g.unit_plot(u);
    let target = match g.unit_get_domain_type(u) {
        DomainType::DOMAIN_LAND => crate::units::explore_target(g, v, u),
        DomainType::DOMAIN_SEA => crate::naval::sea_explore_target(g, v, u),
        _ => None,
    };
    match target {
        Some(q) => {
            v.claimed.push(q);
            move_to(g, grp, q, 0);
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
        }
        None if g.unit_get_domain_type(u) == DomainType::DOMAIN_SEA => crate::naval::port(g, v.p, grp, here),
        None => {
            let p = v.p;
            if !g.map.plots[here].plot_city.is_some_and(|c| g.cities[c].owner == p) {
                if let Some(c) = nearest_city(g, p, here) {
                    move_to(g, grp, g.city_plot(c), 0);
                }
            }
            if alive(g, grp) && g.group_ready_to_move(grp, false) {
                skip(g, grp);
            }
        }
    }
}
