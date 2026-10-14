//! The exe's unit selection (`gDLL->getInterfaceIFace()->getSelectionList()`): the units a
//! command acts on. The exe checks an action against the selection list before sending it
//! (`CvGame::canHandleAction`), and the selection list is itself a `CvSelectionGroup` holding the
//! selected units (not the units' own group, unless it mirrors one). These are ports of the
//! `CvSelectionGroup` checks the exe runs on that list, over a unit slice:
//! `canStartMission`, `canDoCommand`, `canEverDoCommand`, `isBusy`, `getBestBuildRoute`.
//!
//! The interface list never runs missions, so its mission timer is 0: it is busy only while a
//! selected unit fights.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// `mirrorsSelectionGroup`: the selection is exactly one group's units.
pub fn mirrors_group(g: &Game, units: &[UnitId]) -> Option<GroupId> {
    let grp = g.units[*units.first()?].group?;
    let members = &g.groups.get(grp)?.units;
    (members.len() == units.len() && units.iter().all(|&u| g.units[u].group == Some(grp))).then_some(grp)
}

/// The selection's plot (its head unit's).
pub fn plot(g: &Game, units: &[UnitId]) -> PlotIdx {
    g.unit_plot(units[0])
}

/// `CvSelectionGroup::isBusy` of the interface list.
pub fn is_busy(g: &Game, units: &[UnitId]) -> bool {
    units.iter().any(|&u| g.unit_is_combat(u))
}

/// `CvSelectionGroup::getBestBuildRoute` over the selection.
fn best_build_route(g: &Game, units: &[UnitId], plot: PlotIdx) -> Option<RouteType> {
    let d = g.d;
    let mut best_value = 0;
    let mut best_route = None;
    for &u in units {
        for build in d.builds.keys() {
            if let Some(route) = d.builds[build].route {
                if g.unit_can_build(u, plot, build, false) {
                    let value = d.routes[route].value;
                    if value > best_value {
                        best_value = value;
                        best_route = Some(route);
                    }
                }
            }
        }
    }
    best_route
}

/// `(UnitTypes)` of a MOVE_TO_UNIT mission: `data1` = owner, `data2` = unit id.
fn mission_target_unit(g: &Game, player: i32, unit: i32) -> Option<UnitId> {
    if player < 0 || player as usize >= g.num_players() || unit < 0 {
        return None;
    }
    let u = UnitId(unit as u32);
    g.units.get(u).filter(|t| t.owner == PlayerId(player as u8)).map(|_| u)
}

/// Missions a player can push (the others are engine-internal: combat, death, animations).
pub fn is_player_mission(m: MissionType) -> bool {
    m.idx() < MissionType::MISSION_DIE_ANIMATION.idx()
}

/// `CvSelectionGroup::canStartMission(eMission, iData1, iData2, pPlot, bTestVisible)` over the
/// selection. `mission` must satisfy [`is_player_mission`] and its data must be in range for
/// its type (`command::validate` checks both first).
pub fn can_start_mission(g: &Game, units: &[UnitId], mission: MissionType, data1: i32, data2: i32, plot: PlotIdx, test_visible: bool) -> bool {
    if is_busy(g, units) {
        return false;
    }
    let typed = |n: usize| (data1 >= 0 && (data1 as usize) < n).then_some(data1 as usize);
    for &u in units {
        let ok = match mission {
            MissionType::MISSION_MOVE_TO => !g.plot_at(plot, data1, data2),
            MissionType::MISSION_ROUTE_TO => !g.plot_at(plot, data1, data2) || best_build_route(g, units, plot).is_some(),
            MissionType::MISSION_MOVE_TO_UNIT => mission_target_unit(g, data1, data2).is_some_and(|t| !g.unit_at_plot(t, plot)),
            MissionType::MISSION_SKIP => g.unit_can_hold(u, plot),
            MissionType::MISSION_SLEEP => g.unit_can_sleep(u, plot),
            MissionType::MISSION_FORTIFY => g.unit_can_fortify(u, plot),
            MissionType::MISSION_AIRPATROL => g.unit_can_air_patrol(u, plot),
            MissionType::MISSION_SEAPATROL => g.unit_can_sea_patrol(u, plot),
            MissionType::MISSION_HEAL => g.unit_can_heal(u, plot),
            MissionType::MISSION_SENTRY => g.unit_can_sentry(u, plot),
            MissionType::MISSION_AIRLIFT => g.unit_can_airlift_at(u, plot, data1, data2),
            MissionType::MISSION_NUKE => g.unit_can_nuke_at(u, plot, data1, data2),
            MissionType::MISSION_RECON => g.unit_can_recon_at(u, plot, data1, data2),
            MissionType::MISSION_PARADROP => g.unit_can_paradrop_at(u, plot, data1, data2),
            MissionType::MISSION_AIRBOMB => g.unit_can_air_bomb_at(u, plot, data1, data2),
            MissionType::MISSION_BOMBARD => g.unit_can_bombard(u, plot),
            MissionType::MISSION_RANGE_ATTACK => g.unit_can_range_strike_at(u, plot, data1, data2),
            MissionType::MISSION_PLUNDER => g.unit_can_plunder(u, plot, test_visible),
            MissionType::MISSION_PILLAGE => g.unit_can_pillage(u, plot),
            MissionType::MISSION_SABOTAGE => g.unit_can_sabotage(u, plot, test_visible),
            MissionType::MISSION_DESTROY => g.unit_can_destroy(u, plot, test_visible),
            MissionType::MISSION_STEAL_PLANS => g.unit_can_steal_plans(u, plot, test_visible),
            MissionType::MISSION_FOUND => g.unit_can_found(u, plot, test_visible),
            MissionType::MISSION_SPREAD => g.unit_can_spread(u, plot, typed(g.d.religions.len()).map(ReligionType::from_usize), test_visible),
            MissionType::MISSION_SPREAD_CORPORATION => {
                g.unit_can_spread_corporation(u, plot, typed(g.d.corporations.len()).map(CorporationType::from_usize), test_visible)
            }
            MissionType::MISSION_JOIN => g.unit_can_join(u, plot, typed(g.d.specialists.len()).map(SpecialistType::from_usize)),
            MissionType::MISSION_CONSTRUCT => g.unit_can_construct(u, plot, typed(g.d.buildings.len()).map(BuildingType::from_usize), test_visible),
            MissionType::MISSION_DISCOVER => g.unit_can_discover(u, plot),
            MissionType::MISSION_HURRY => g.unit_can_hurry(u, plot, test_visible),
            MissionType::MISSION_TRADE => g.unit_can_trade(u, plot, test_visible),
            MissionType::MISSION_GREAT_WORK => g.unit_can_great_work(u, plot),
            MissionType::MISSION_INFILTRATE => g.unit_can_infiltrate(u, plot, test_visible),
            // "this means to play the animation only"
            MissionType::MISSION_GOLDEN_AGE => data1 != -1 || g.unit_can_golden_age(u, plot, test_visible),
            MissionType::MISSION_BUILD => typed(g.d.builds.len()).is_some_and(|b| g.unit_can_build(u, plot, BuildType::from_usize(b), test_visible)),
            MissionType::MISSION_LEAD => g.unit_can_lead(u, plot, (data1 >= 0).then_some(UnitId(data1 as u32))) != 0,
            MissionType::MISSION_ESPIONAGE => g.unit_can_espionage(u, plot, test_visible),
            _ => false,
        };
        if ok {
            return true;
        }
    }
    false
}

/// `CvSelectionGroup::canEverDoCommand` over the selection.
pub fn can_ever_do_command(g: &Game, units: &[UnitId], command: CommandType) -> bool {
    if command == CommandType::COMMAND_LOAD {
        let plot = plot(g, units);
        return g.map.plots[plot].units.iter().any(|&u| !g.unit_is_full(u));
    }
    if command == CommandType::COMMAND_UNLOAD {
        return units.iter().any(|&u| g.unit_is_cargo(u));
    }
    true
}

/// `CvSelectionGroup::canDoCommand(eCommand, iData1, iData2, bTestVisible)` over the selection.
pub fn can_do_command(g: &Game, units: &[UnitId], command: CommandType, data1: i32, data2: i32, test_visible: bool) -> bool {
    if is_busy(g, units) || !can_ever_do_command(g, units, command) {
        return false;
    }
    units.iter().any(|&u| g.unit_can_do_command(u, command, data1, data2, test_visible, false))
}

/// The transports `launchLoadUnitPopup` offers: units on the selection's plot the selection
/// can load into (`canDoCommand(COMMAND_LOAD_UNIT, owner, id)`), in plot order.
pub fn load_targets(g: &Game, units: &[UnitId]) -> Vec<UnitId> {
    let plot = plot(g, units);
    g.map.plots[plot]
        .units
        .iter()
        .copied()
        .filter(|&t| can_do_command(g, units, CommandType::COMMAND_LOAD_UNIT, g.units[t].owner.i(), t.0 as i32, false))
        .collect()
}
