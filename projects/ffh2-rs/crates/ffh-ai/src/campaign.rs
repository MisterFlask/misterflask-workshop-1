//! Siege campaigns: coordinated multi-stack operations against enemy and barbarian cities.
//!
//! One operation per player at a time, recomputed every turn from the game state (nothing is
//! remembered outside `Game`, so a saved game resumed with a fresh controller plays on
//! identically):
//!
//! - **Target** (`choose_target`): at war (or preparing one), an enemy city; in peacetime, a
//!   barbarian city within 14 plots of ours; the best value (population, wonders, holy city,
//!   resources, a barbarian town near our cities) for its cost (the force it needs against what
//!   we see of its defenders, the distance). Every order of an operation carries its target as
//!   the group's mission AI (`MISSIONAI_ASSAULT`, saved with the game), and the city our stacks
//!   are committed to keeps its place while its force stays within reach: without that, stacks
//!   walked back and forth between two targets.
//! - **Commitment**: at war every stack on the landmass within 24 plots; against barbarians the
//!   nearest stacks until they hold twice the force the city needs.
//! - **Force** ([`simulate`]): the strike played out in expected values with the rules' combat
//!   odds (fortification, city defense, first strikes): collateral attackers first, then the
//!   strongest; every fight takes the defender's expected health down (a lost fight still
//!   wounds), collateral damage hits the other defenders. The force covers the city when the
//!   defenders fall with an attacker to spare; siege engines that can bring the walls down
//!   within a few turns count as having done so.
//! - **Phases** ([`Phase`]): while the committed force falls short, stacks gather in the rally
//!   city (our city nearest the target; against barbarians they stay where they are) and
//!   production builds what is missing ([`army_choice`]); once it covers the city, stacks march
//!   to an assembly plot two plots out (the best defensive ground), one turn at a time, step
//!   next to the city together from there when the force there covers it, and strike when the
//!   force next to it does: every committed stack there attacks
//!   in the same turn (`strike`), siege engines bombarding first, then collateral attackers,
//!   then the best odds (or the cheapest unit to lose), while the remaining force still covers
//!   the remaining defenders. The victor does not stay alone in the city: up to three more
//!   units follow it in. Stacks past gathering neither turn back for our cities in peril (the
//!   threat usually comes from their target) nor stop to heal just before the strike.
//! - **No hopeless camping**: a stack next to the target whose operation cannot strike steps
//!   back to the assembly plot (or the rally city while the force falls short); stacks not in
//!   the operation never campaign on their own (`military.rs` hunts only cities it can take).
//! - **Production** ([`army_choice`], one hook in `production.rs`): attackers and siege engines
//!   up to [`needed_force`], only attackers that can hurt the target's garrison.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::unit::CombatDetails;
use ffh_core::utils::{combat_odds, CombatOddsInput};
use ffh_data::*;

use crate::military::is_army_unit;
use crate::production::{best_unit, count_ai, Choice, Role};
use crate::units::{alive, skip};
use crate::view::{attack_power, View};

/// Operation phase (recomputed every turn).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The force falls short (or the war is not declared yet): gather in the rally city.
    Gather,
    /// March to the assembly plot two plots out and wait there for the others.
    March,
    /// The force at the assembly plot covers the city: step next to it together.
    StepIn,
    /// The force next to the city covers it: attack this turn.
    Strike,
}

/// One operation: a target city and the stacks committed to it.
#[derive(Clone, Debug)]
pub struct Op {
    pub city: CityId,
    pub plot: PlotIdx,
    /// A civ city (war) rather than a barbarian one.
    pub civ: bool,
    pub phase: Phase,
    /// Committed groups, nearest first.
    pub groups: Vec<GroupId>,
    /// Our city nearest the target (gathering point while the force falls short).
    pub rally: Option<PlotIdx>,
    /// Best defensive plot two plots from the target, on our side.
    pub assembly: Option<PlotIdx>,
    /// Plot next to the target the stacks step onto (next to the assembly plot).
    pub staging: Option<PlotIdx>,
    /// The strike of this turn has been made.
    pub struck: bool,
    /// At war with the target's owner (else we only prepare).
    pub at_war: bool,
    /// Units the target needs (estimate) and units committed.
    pub need: i32,
    pub have: i32,
}

/// This turn's operations (built on the first army decision of the turn, see [`plan`]).
#[derive(Default)]
pub struct Plan {
    pub built: bool,
    pub ops: Vec<Op>,
}

impl Plan {
    fn op_of(&self, grp: GroupId) -> Option<usize> {
        self.ops.iter().position(|o| o.groups.contains(&grp))
    }

    /// The city our war operation targets.
    pub fn war_target(&self) -> Option<CityId> {
        self.ops.iter().find(|o| o.civ).map(|o| o.city)
    }
}

// ----------------------------------------------------------------------------
// Fight estimates
// ----------------------------------------------------------------------------

/// Visible enemy units able to defend city plot `tp` (what we can see of the garrison; the
/// city owner's units count before the war is declared too).
pub(crate) fn visible_defenders(g: &Game, team: TeamId, tp: PlotIdx) -> Vec<UnitId> {
    if !g.plot_is_visible(tp, team) {
        return Vec::new();
    }
    let owner = g.map.plots[tp].plot_city.map(|c| g.players[g.cities[c].owner].team);
    g.map.plots[tp]
        .units
        .iter()
        .copied()
        .filter(|&e| {
            let et = g.unit_get_team(e);
            et != team && (g.at_war(team, et) || Some(et) == owner) && g.unit_can_defend(e, Some(tp)) && !g.unit_is_invisible(e, team, false)
        })
        .collect()
}

/// `apply_combat_modifier` of the rules, inverted and reapplied: a strength computed with
/// modifier `from` recomputed with modifier `to`.
fn remodify(strength: i32, from: i32, to: i32) -> i32 {
    let base = if from > 0 { strength as i64 * 100 / (from as i64 + 100) } else { strength as i64 * (100 - from as i64) / 100 };
    let s = if to > 0 { base * (to as i64 + 100) / 100 } else { base * 100 / (100 - to as i64) };
    (s as i32).max(1)
}

/// One fight of the simulation: attacker `a` against defender `d` (on `dp`) at `hp` hit points
/// with `wall_cut` points of city defense bombarded away. Returns (odds per mille, expected
/// damage `a` deals when it loses). The odds are the rules' `getCombatOdds` with those two
/// changes.
fn fight(g: &Game, a: UnitId, d: UnitId, dp: PlotIdx, hp: i32, wall_cut: i32) -> (i32, i32) {
    let mut det = CombatDetails::default();
    let mut d_max = g.unit_max_combat_str(d, Some(dp), Some(a), Some(&mut det));
    if wall_cut > 0 {
        d_max = remodify(d_max, det.modifier_total, det.modifier_total - wall_cut);
    }
    let max_hp = g.unit_max_hit_points(d).max(1);
    let hp = hp.clamp(1, max_hp);
    let d_str = (d_max * hp / max_hp).max(1);
    let d_fp = (d_max + d_str + 1) / 2;
    let a_str = g.unit_curr_combat_str(a, None, Some(d), None);
    if a_str <= 0 {
        return (0, 0);
    }
    let a_fp = g.unit_curr_firepower(a, None, Some(d));
    let a_hp = g.unit_curr_hit_points(a);
    let k = CombatOddsInput {
        attacker_strength: a_str,
        attacker_firepower: a_fp,
        defender_strength: d_str,
        defender_firepower: d_fp,
        attacker_hit_points: a_hp,
        defender_hit_points: hp,
        defender_max_hit_points: max_hp,
        attacker_combat_limit: g.units[a].combat_limit,
        attacker_first_strikes: g.unit_first_strikes(a),
        attacker_chance_first_strikes: g.unit_chance_first_strikes(a),
        attacker_immune_to_first_strikes: g.unit_immune_to_first_strikes(a),
        defender_first_strikes: g.unit_first_strikes(d),
        defender_chance_first_strikes: g.unit_chance_first_strikes(d),
        defender_immune_to_first_strikes: g.unit_immune_to_first_strikes(d),
    };
    let defs = &g.d.defines;
    let odds = combat_odds(&k, defs.combat_die_sides, defs.combat_damage);
    // A lost fight: the rounds the attacker wins before it has lost all its hit points.
    let sf = (a_fp + d_fp + 1) / 2;
    let to_att = (defs.combat_damage * (d_fp + sf) / (a_fp + sf)).max(1);
    let to_def = (defs.combat_damage * (a_fp + sf) / (d_fp + sf)).max(1);
    let losses = (a_hp + to_att - 1) / to_att;
    let q = a_str as i64 * 1000 / (a_str + d_str) as i64; // per mille a round goes our way
    let won = losses as i64 * q / (1000 - q).max(50);
    let mut wound = (won * to_def as i64).min(hp as i64) as i32;
    // Combat limit (siege engines stop at a share of the defender's health).
    let limit = g.units[a].combat_limit;
    if limit < max_hp {
        wound = wound.min((hp - (max_hp - limit)).max(0));
    }
    (odds, wound)
}

/// Expected outcome of a strike.
#[derive(Copy, Clone, Debug, Default)]
pub struct Sim {
    /// Attackers used until the defenders fell (`None`: they hold).
    pub used: Option<usize>,
    /// Defenders at the start.
    pub n_def: usize,
    /// Expected defender hit points left after all attacks.
    pub hp_left: i32,
    /// Best odds of one of the fights.
    pub best: i32,
}

/// Play the strike of `atts` against the defenders `defs` of plot `tp` in expected values
/// (see the module doc). `wall_cut`: city defense points bombarded away first.
pub fn simulate(g: &Game, tp: PlotIdx, defs: &[UnitId], atts: &[UnitId], wall_cut: i32) -> Sim {
    let mut ds: Vec<(UnitId, i32, i32)> = defs.iter().map(|&d| (d, g.unit_curr_hit_points(d), g.unit_max_combat_str(d, Some(tp), None, None))).collect();
    let n_def = ds.len();
    if ds.is_empty() {
        return Sim { used: Some(0), n_def, hp_left: 0, best: 1000 };
    }
    let max_hp = g.d.defines.max_hit_points.max(1);
    let dead = max_hp / 8;
    let collateral = |u: UnitId| g.unit_collateral_damage(u) > 0;
    let mut order: Vec<UnitId> = atts.iter().copied().filter(|&u| attack_power(g, u) > 0 && g.unit_can_attack(u)).collect();
    order.sort_by_key(|&u| (std::cmp::Reverse(collateral(u) && n_def >= 2), std::cmp::Reverse(attack_power(g, u)), u));
    let mut best = 0;
    for (i, &a) in order.iter().enumerate() {
        ds.retain(|d| d.1 > dead);
        if ds.is_empty() {
            return Sim { used: Some(i), n_def, hp_left: 0, best };
        }
        // The defender the rules put up: the strongest (by its plot strength and health).
        let j = (0..ds.len()).max_by_key(|&j| (ds[j].2 as i64 * ds[j].1 as i64, std::cmp::Reverse(ds[j].0))).unwrap();
        let (odds, wound) = fight(g, a, ds[j].0, tp, ds[j].1, wall_cut);
        best = best.max(odds);
        let hp = ds[j].1;
        ds[j].1 = ((1000 - odds) as i64 * (hp - wound).max(0) as i64 / 1000) as i32;
        // Collateral damage on the others (CvUnit::collateralCombat).
        if collateral(a) && ds.len() > 1 {
            let cs = g.unit_base_combat_str(a) * g.unit_collateral_damage(a) / 100;
            let limit = g.unit_collateral_damage_limit(a);
            let mut n = (ds.len() as i32 - 1).min(g.unit_collateral_damage_max_units(a));
            let mut others: Vec<usize> = (0..ds.len()).filter(|&o| o != j).collect();
            others.sort_by_key(|&o| (std::cmp::Reverse(ds[o].1), o));
            for o in others {
                if n <= 0 {
                    break;
                }
                n -= 1;
                let ts = g.unit_base_combat_str(ds[o].0);
                let sf = (cs + ts + 1) / 2;
                if ts + sf <= 0 {
                    continue;
                }
                let dmg = g.d.defines.collateral_combat_damage * (cs + sf) / (ts + sf);
                let cap = limit.min(limit * (cs + sf) / (ts + sf));
                let damage = max_hp - ds[o].1;
                let new_damage = damage.max((damage + dmg).min(cap));
                ds[o].1 = max_hp - new_damage;
            }
        }
    }
    ds.retain(|d| d.1 > dead);
    let hp_left = ds.iter().map(|d| d.1).sum();
    Sim { used: if ds.is_empty() { Some(order.len()) } else { None }, n_def, hp_left, best }
}

/// City defense our siege engines in `units` can bombard away within three turns (0 without
/// siege engines).
fn wall_cut(g: &Game, c: CityId, units: &[UnitId]) -> i32 {
    let rate: i32 = units.iter().map(|&u| g.unit_bombard_rate(u)).sum();
    if rate <= 0 {
        return 0;
    }
    let ignore = units.iter().any(|&u| g.unit_bombard_rate(u) > 0 && g.unit_ignore_building_defense(u));
    let def = g.city_defense_modifier(c, ignore);
    let per_turn = if ignore { rate } else { rate * (100 - g.cities[c].building_bombard_defense).max(0) / 100 };
    def.min(per_turn * 3).max(0)
}

/// Attackers to spare before a force counts as covering a city: a fifth of the force, at
/// least one (an expected-value estimate: fights go worse about half the time).
fn margin(n: usize) -> usize {
    (n / 5).max(1)
}

/// Does `units` take city `c` (plot `tp`, defenders `defs`) with `spare` attackers to spare?
/// With `spare`, a [`margin`] of attackers must be left over (only units able to attack count).
fn covers(g: &Game, c: CityId, tp: PlotIdx, defs: &[UnitId], units: &[UnitId], spare: bool, siege: bool) -> bool {
    let n_att = units.iter().filter(|&&u| attack_power(g, u) > 0 && g.unit_can_attack(u)).count();
    if n_att == 0 {
        return false;
    }
    let spare = if spare { margin(n_att) } else { 0 };
    let cut = if siege { wall_cut(g, c, units) } else { 0 };
    let s = simulate(g, tp, defs, units, cut);
    s.used.is_some_and(|n| n + spare <= n_att) || s.best >= 850 && defs.len() == 1
}

// ----------------------------------------------------------------------------
// Targets
// ----------------------------------------------------------------------------

/// A candidate target city.
#[derive(Copy, Clone, Debug)]
struct Cand {
    city: CityId,
    plot: PlotIdx,
    civ: bool,
    /// Defenders seen (or assumed when the city is not visible).
    n_def: i32,
    visible: bool,
    value: i32,
    /// Plot distance from our nearest city on its landmass.
    dist: i32,
}

/// Units a city of `n_def` defenders needs before we have seen a fight (rough).
pub(crate) fn rough_need(n_def: i32) -> i32 {
    n_def * 2 + 1
}

/// What taking city `c` is worth to us (`dist`: from our nearest city).
fn city_value(g: &Game, p: PlayerId, c: CityId, dist: i32, plan: &crate::victory::Plan) -> i32 {
    let city = &g.cities[c];
    let team = g.players[p].team;
    let mut v = 400 + city.population * 100 + city.num_world_wonders * 300;
    v += crate::victory::rival_city_bonus(g, plan, c); // victory-pursuit
    if g.d.religions.keys().any(|r| g.get_holy_city(r) == Some(c)) {
        v += 400;
    }
    for i in 0..NUM_CITY_PLOTS {
        let Some(q) = g.city_index_plot(c, i) else { continue };
        let plot = &g.map.plots[q];
        if !plot.revealed[team] {
            continue;
        }
        if let Some(b) = plot.bonus_type {
            if g.d.bonuses[b].tech_reveal.map_or(true, |t| g.team_is_has_tech(team, Some(t))) {
                v += 60;
            }
        }
    }
    if g.player_is_barbarian(city.owner) {
        // A barbarian town near our cities keeps sending raiders.
        v += (14 - dist).max(0) * 40;
    }
    v
}

/// Cities our army may campaign against: enemy civ cities at war (or the civ we prepare a war
/// against), else barbarian cities near our lands -- revealed, on a landmass where we have a
/// city. Returns the candidates and whether this is a war (or its preparation).
fn candidates(g: &Game, p: PlayerId) -> (Vec<Cand>, bool) {
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    let war: Vec<TeamId> = g.teams.keys().filter(|&t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t)).collect();
    let preparing = if war.is_empty() { crate::diplomacy::preparing_target(g, team) } else { None };
    let civ = !war.is_empty() || preparing.is_some();
    let plan = if civ { crate::victory::plan(g, p) } else { crate::victory::Plan::default() }; // victory-pursuit
    let ours: Vec<(i32, i32, Option<AreaId>)> = g.players[p].cities.iter().map(|&c| (g.cities[c].x, g.cities[c].y, g.map.plots[g.city_plot(c)].area)).collect();
    let mut out = Vec::new();
    for pl in g.players.iter() {
        if !pl.alive || pl.cities.is_empty() {
            continue;
        }
        // (The Clan of Embers starts at peace with the barbarians.)
        let ok = if civ { war.contains(&pl.team) || preparing == Some(pl.team) } else { pl.team == barb && g.at_war(team, barb) };
        if !ok {
            continue;
        }
        for &c in &pl.cities {
            let cp = g.city_plot(c);
            if !g.map.plots[cp].revealed[team] {
                continue;
            }
            let (cx, cy) = (g.cities[c].x, g.cities[c].y);
            let area = g.map.plots[cp].area;
            let Some(dist) = ours.iter().filter(|o| o.2 == area).map(|&(x, y, _)| g.plot_distance(x, y, cx, cy)).min() else { continue };
            if dist > if civ { 20 } else { 14 } {
                continue;
            }
            let visible = g.plot_is_visible(cp, team);
            // Unseen: what such a city usually holds. Barbarian towns arm over time (measured:
            // 0-1 defenders in their first 20 turns, 2 by turn 30, 3 by 60, 5 by 100).
            let n_def = if visible {
                visible_defenders(g, team, cp).len() as i32
            } else if civ {
                (2 + g.cities[c].population / 5).min(5)
            } else {
                (1 + (g.game_turn() - g.cities[c].game_turn_acquired) / 25).clamp(1, 6)
            };
            out.push(Cand { city: c, plot: cp, civ, n_def, visible, value: city_value(g, p, c, dist, &plan), dist });
        }
    }
    (out, civ)
}

/// Cheap score of a candidate (value for the force and the march it costs).
fn rough_score(c: &Cand) -> i64 {
    let need = rough_need(c.n_def) + i32::from(!c.visible) * 2;
    c.value as i64 * 1000 / (need as i64 * 100 + c.dist as i64 * 25 + 200)
}

/// Our army units on `area` able to attack (heroes included), strongest first.
fn force_on(g: &Game, p: PlayerId, area: Option<AreaId>) -> Vec<UnitId> {
    let mut out: Vec<UnitId> = Vec::new();
    for &grp in &g.players[p].groups {
        if !army_group(g, grp) {
            continue;
        }
        let Some(head) = g.group_get_head_unit(grp) else { continue };
        if g.map.plots[g.unit_plot(head)].area != area {
            continue;
        }
        out.extend(g.groups[grp].units.iter().copied().filter(|&u| attack_power(g, u) > 0 && g.unit_can_attack(u)));
    }
    out.sort_by_key(|&u| (std::cmp::Reverse(attack_power(g, u)), u));
    out
}

/// A field army group (what `military::army` commands), on land, not aboard a ship.
fn army_group(g: &Game, grp: GroupId) -> bool {
    let Some(head) = g.group_get_head_unit(grp) else { return false };
    if g.unit_is_cargo(head) || g.unit_get_domain_type(head) != DomainType::DOMAIN_LAND {
        return false;
    }
    is_army_unit(g, head) || g.units[head].ai_type == Some(UnitAIType::UNITAI_HERO) && attack_power(g, head) > 0
}

/// Our units in groups whose last orders aim at city plot `tp` (see [`tagged`]).
fn committed_units(g: &Game, p: PlayerId, tp: PlotIdx) -> i32 {
    g.players[p].groups.iter().filter(|&&grp| tagged(g, grp) == Some(tp)).map(|&grp| g.groups[grp].units.len() as i32).sum()
}

/// Units the city of candidate `cand` needs against what we see of it, given our `force` on
/// its landmass (strongest first): the strike played out with the whole force when the city is
/// seen (what is missing, roughly, when it falls short), else a rough guess.
fn need_for(g: &Game, team: TeamId, cand: &Cand, force: &[UnitId]) -> i32 {
    if cand.visible && !force.is_empty() {
        let defs = visible_defenders(g, team, cand.plot);
        let all: Vec<UnitId> = force.iter().copied().take(24).collect();
        let s = simulate(g, cand.plot, &defs, &all, wall_cut(g, cand.city, &all));
        match s.used {
            Some(n) => n as i32 + 1,
            // Short of the whole force: a defender's health per two more attackers.
            None => all.len() as i32 + 1 + s.hp_left * 2 / g.d.defines.max_hit_points.max(1),
        }
    } else if cand.visible {
        rough_need(cand.n_def)
    } else {
        // Enough to go and look.
        cand.n_def + 2
    }
}

/// The target of our operation and the number of units it needs. The city our stacks are
/// committed to (the most units whose last orders aim at it) keeps its place while it is within
/// reach of what we have or can build; otherwise the best value for its cost among the cities
/// we can reach (exact estimates for the few best, rough ones otherwise). Without that
/// stickiness the target changed whenever a stack came close enough to see a garrison, and the
/// stacks walked back and forth between two cities. Returns (candidate, units needed, feasible
/// with the force we have).
fn choose_target(g: &Game, p: PlayerId) -> Option<(Cand, i32, bool)> {
    let (cands, civ) = candidates(g, p);
    if cands.is_empty() {
        return None;
    }
    let team = g.players[p].team;
    let invested: Vec<i32> = cands.iter().map(|c| committed_units(g, p, c.plot)).collect();
    let kept = (0..cands.len()).filter(|&i| invested[i] > 0).max_by_key(|&i| (invested[i], std::cmp::Reverse(cands[i].city)));
    if let Some(i) = kept {
        let c = cands[i];
        let force = force_on(g, p, g.map.plots[c.plot].area);
        let need = need_for(g, team, &c, &force);
        // Within reach: twice the force we have at war; four more units than we have against
        // barbarians (production builds toward the target, but not without end).
        let reach = if civ { force.len() as i32 * 2 + 2 } else { force.len() as i32 + 4 };
        if need <= reach {
            return Some((c, need, need <= force.len() as i32));
        }
    }
    let mut order: Vec<usize> = (0..cands.len()).filter(|&i| Some(i) != kept).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(rough_score(&cands[i])), cands[i].city));
    let mut best: Option<(Cand, i32, bool, i64)> = None;
    for i in order.into_iter().take(4) {
        let c = cands[i];
        let force = force_on(g, p, g.map.plots[c.plot].area);
        let need = need_for(g, team, &c, &force);
        let feasible = need <= force.len() as i32;
        let mut score = c.value as i64 * 1000 / (need as i64 * 100 + c.dist as i64 * 25 + 200);
        if feasible {
            score += 1_000_000;
        }
        if best.map_or(true, |b| score > b.3) {
            best = Some((c, need, feasible, score));
        }
    }
    let (c, need, feasible, _) = best?;
    // A barbarian town far beyond the force we have and can build: leave it alone.
    if !civ && need > force_on(g, p, g.map.plots[c.plot].area).len() as i32 + 4 {
        return None;
    }
    Some((c, need, feasible))
}

// ----------------------------------------------------------------------------
// The plan of this turn
// ----------------------------------------------------------------------------

/// Build this turn's operation (once per `View`).
pub fn plan(g: &Game, v: &mut View) {
    if v.campaign.built {
        return;
    }
    v.campaign.built = true;
    if v.barbarian || g.players[v.p].cities.is_empty() {
        return;
    }
    if let Some(op) = build_op(g, v) {
        v.campaign.ops.push(op);
    }
}

fn build_op(g: &Game, v: &mut View) -> Option<Op> {
    let p = v.p;
    let team = v.team;
    let (cand, need, _feasible) = choose_target(g, p)?;
    let tp = cand.plot;
    let (tx, ty) = g.map.xy(tp);
    let area = g.map.plots[tp].area;
    let owner_team = g.players[g.cities[cand.city].owner].team;
    let at_war = g.at_war(team, owner_team);
    // Our stacks on the target's landmass, nearest first.
    let mut groups: Vec<(i32, GroupId)> = Vec::new();
    for &grp in &g.players[p].groups {
        if !army_group(g, grp) {
            continue;
        }
        let head = g.group_get_head_unit(grp).unwrap();
        let hp = g.unit_plot(head);
        if g.map.plots[hp].area != area {
            continue;
        }
        let (x, y) = g.map.xy(hp);
        let d = g.plot_distance(x, y, tx, ty);
        if d > if cand.civ { 24 } else { 16.max(cand.dist + 6) } {
            continue;
        }
        groups.push((d, grp));
    }
    groups.sort();
    if groups.is_empty() {
        return None;
    }
    // Commit: at war every stack; against barbarians the nearest until the force covers the
    // city twice over (stacks get delayed: healing, a city in peril), plus whatever already
    // stands near it or is on its way.
    let mut committed: Vec<GroupId> = Vec::new();
    let mut n_units = 0;
    let enough = need * 2 + 2;
    for &(d, grp) in &groups {
        if !cand.civ && n_units >= enough && d > 3 && tagged(g, grp) != Some(tp) {
            continue;
        }
        committed.push(grp);
        n_units += g.groups[grp].units.len() as i32;
    }
    let units_of = |pred: &dyn Fn(i32, GroupId) -> bool| -> Vec<UnitId> {
        let mut out = Vec::new();
        for &(d, grp) in &groups {
            if committed.contains(&grp) && pred(d, grp) {
                out.extend(g.groups[grp].units.iter().copied());
            }
        }
        out
    };
    let all = units_of(&|_, _| true);
    let near = units_of(&|d, _| d <= 2);
    let adj: Vec<UnitId> = units_of(&|d, _| d <= 1).into_iter().filter(|&u| g.unit_moves_left(u) > 0).collect();
    let defs = visible_defenders(g, team, tp);
    // We commit to marching only with a confident force (a reserve to spare: `spare = true`), so
    // stacks do not advance understrength; but once a committed force is positioned next to or
    // beside the city it strikes at ACCEPTABLE odds (`spare = false`: it expects to clear the
    // defenders, without insisting on keeping a reserve) rather than retreating to muster an
    // overwhelming force against a target that only hardens while it waits. Taking territory at
    // acceptable risk is the point; a lone unit still never qualifies (it cannot clear a city).
    let (ok_all, ok_near, ok_adj) = if cand.visible {
        (
            covers(g, cand.city, tp, &defs, &all, true, true),
            covers(g, cand.city, tp, &defs, &near, false, true),
            !adj.is_empty() && covers(g, cand.city, tp, &defs, &adj, false, false),
        )
    } else {
        // Unseen: a scouting-sized force goes to look. Capped (not the assumed defender count,
        // which grows as a barb town arms with age -- that made the needed force climb out of
        // reach so the army mustered forever and never marched to reveal the town). Once the
        // stack reaches the assembly ring and the real defenders show, the visible branch above
        // decides: strike at acceptable odds, or step back if it turns out too strong.
        let n = ((cand.n_def + 2) as usize).min(4);
        (all.len() >= n, near.len() >= n, false)
    };
    // Concentrate before a MARGINAL strike. The adjacent ring may clear the city at acceptable
    // odds (`ok_adj`, no reserve) yet bounce; `strike()` only pools units a single plot away, so a
    // committed stack one plot out is left out of the fight (measured: a city struck by 8 while
    // another 8 stood one plot back). If that farther stack can reach an adjacent plot THIS turn
    // and the combined force would then take the city decisively (with a reserve), hold the
    // adjacent stacks and step the farther one in (StepIn) instead of striking piecemeal -- they
    // strike together next turn. Strictly one turn: a stack that cannot close this turn is not
    // counted, so a city that only hardens is still struck now at acceptable odds, and a ring that
    // already wins with a reserve (`adj_decisive`) strikes now regardless.
    let adj_decisive = ok_adj && covers(g, cand.city, tp, &defs, &adj, true, false);
    let concentrate = ok_adj
        && !adj_decisive
        && {
            let mut strikeable = adj.clone();
            for &(d, grp) in &groups {
                if d != 2 || !committed.contains(&grp) {
                    continue;
                }
                let Some(h) = g.group_get_head_unit(grp) else { continue };
                let closes = v.flood.run(g, h, 2, false).into_iter().any(|r| {
                    if r.turns != 0 || g.map.plots[r.plot].plot_city.is_some() {
                        return false;
                    }
                    let (rx, ry) = g.map.xy(r.plot);
                    g.plot_distance(rx, ry, tx, ty) == 1
                        && !g.plot_is_visible_enemy_unit_for_unit(r.plot, h)
                        && g.unit_can_move_into(h, r.plot, false, false, false)
                });
                if closes {
                    strikeable.extend(g.groups[grp].units.iter().copied());
                }
            }
            strikeable.len() > adj.len() && covers(g, cand.city, tp, &defs, &strikeable, true, true)
        };
    // Strike when the adjacent ring is decisive, or when no one-turn concentration would improve
    // it; otherwise defer one turn to concentrate. `concentrate` implies `ok_near` and `ok_all`
    // (strikeable is a subset of near and all), so a deferred strike lands on StepIn, never Gather.
    let strike_now = ok_adj && (adj_decisive || !concentrate);
    let phase = if !at_war {
        Phase::Gather
    } else if strike_now {
        Phase::Strike
    } else if !ok_all {
        Phase::Gather
    } else if ok_near {
        Phase::StepIn
    } else {
        Phase::March
    };
    let rally = g.players[p].cities.iter().copied().filter(|&c| g.map.plots[g.city_plot(c)].area == area).min_by_key(|&c| (g.plot_distance(tx, ty, g.cities[c].x, g.cities[c].y), c)).map(|c| g.city_plot(c));
    let reference = committed.iter().find_map(|&grp| g.group_get_head_unit(grp));
    let assembly = reference.and_then(|r| assembly_plot(g, v, r, tp, rally));
    let staging = reference.and_then(|r| staging_plot(g, r, tp, assembly));
    crate::trace!(
        g,
        p,
        "campaign vs {} ({}, {} defenders{}): {:?}, {} stacks / {} units (need {}), near {}, adjacent {} [war {} ok {}/{}/{} age {} dist {}]",
        g.cities[cand.city].name,
        if cand.civ { "civ" } else { "barbarian" },
        cand.n_def,
        if cand.visible { "" } else { " assumed" },
        phase,
        committed.len(),
        all.len(),
        need,
        near.len(),
        adj.len(),
        at_war,
        ok_all,
        ok_near,
        ok_adj,
        g.game_turn() - g.cities[cand.city].game_turn_acquired,
        cand.dist
    );
    Some(Op { city: cand.city, plot: tp, civ: cand.civ, phase, groups: committed, rally, assembly, staging, struck: false, at_war, need, have: all.len() as i32 })
}

/// The plot two plots from the target where the stacks gather: land we can enter, no enemy
/// unit on it; one from which the city can be seen (high ground: the stacks there see its
/// garrison and decide without stepping next to it), then the best defense, nearest our rally
/// city.
fn assembly_plot(g: &Game, v: &View, reference: UnitId, tp: PlotIdx, rally: Option<PlotIdx>) -> Option<PlotIdx> {
    let (tx, ty) = g.map.xy(tp);
    let sight = g.unit_visibility_range(reference);
    let mut best: Option<(PlotIdx, i32)> = None;
    for dx in -2..=2 {
        for dy in -2..=2 {
            let Some(q) = g.plot_xy(tx, ty, dx, dy) else { continue };
            let (qx, qy) = g.map.xy(q);
            if g.plot_distance(qx, qy, tx, ty) != 2 || !g.map.plots[q].revealed[v.team] || g.plot_is_water(q) || g.plot_is_impassable(q) {
                continue;
            }
            if g.map.plots[q].plot_city.is_some() || g.plot_is_visible_enemy_unit_for_unit(q, reference) || !g.unit_can_move_into(reference, q, false, false, false) {
                continue;
            }
            let from_rally = rally.map_or(0, |r| {
                let (rx, ry) = g.map.xy(r);
                g.plot_distance(qx, qy, rx, ry)
            });
            let sees = g.plot_can_see_plot(q, tp, v.team, sight, None);
            let score = i32::from(sees) * 1000 + g.plot_defense_modifier(q, Some(v.team), true) * 2 - from_rally * 10 - v.danger_at(q) / 200;
            if best.map_or(true, |(_, s)| score > s) {
                best = Some((q, score));
            }
        }
    }
    best.map(|(q, _)| q)
}

/// The plot next to the target the stacks step onto: next to the assembly plot, best defense.
fn staging_plot(g: &Game, reference: UnitId, tp: PlotIdx, assembly: Option<PlotIdx>) -> Option<PlotIdx> {
    let (tx, ty) = g.map.xy(tp);
    let mut best: Option<(PlotIdx, i32)> = None;
    let team = g.unit_get_team(reference);
    for dir in 0..DirectionType::NUM {
        let Some(q) = g.plot_direction(tx, ty, DirectionType::from_usize(dir)) else { continue };
        if g.plot_is_water(q) || g.plot_is_impassable(q) || g.map.plots[q].plot_city.is_some() || g.plot_is_visible_enemy_unit_for_unit(q, reference) {
            continue;
        }
        if !g.unit_can_move_into(reference, q, false, false, false) {
            continue;
        }
        let (qx, qy) = g.map.xy(q);
        let from_assembly = assembly.map_or(0, |a| {
            let (ax, ay) = g.map.xy(a);
            g.plot_distance(qx, qy, ax, ay)
        });
        let score = g.plot_defense_modifier(q, Some(team), true) - from_assembly * 100;
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((q, score));
        }
    }
    best.map(|(q, _)| q)
}

// ----------------------------------------------------------------------------
// Orders
// ----------------------------------------------------------------------------

/// End the group's turn if it is still waiting for orders.
fn finish(g: &mut Game, grp: GroupId) {
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        skip(g, grp);
    }
}

/// Orders of an operation carry its target as the group's mission AI (C++
/// `AI_setMissionAI(MISSIONAI_ASSAULT, pTargetPlot)`), saved with the game: the next turn's plan
/// knows which stacks are already committed to which city, so the target stays put.
fn order(g: &mut Game, grp: GroupId, mission: MissionType, x: i32, y: i32, flags: i32, tp: PlotIdx) {
    g.group_push_mission(grp, mission, x, y, flags, false, false, Some(MissionAIType::MISSIONAI_ASSAULT), Some(tp), None);
}

/// [`order`] a move to `dest`, one turn of it: the rest of the path is dropped, so the stack
/// decides again next turn (an AI group on a multi-turn mission gets no orders until it ends,
/// C++ `CvSelectionGroup::doTurn`: stacks walked on to plots chosen for an earlier phase while
/// the force they were part of dwindled, or past cities of ours in peril while gathering).
fn march(g: &mut Game, grp: GroupId, dest: PlotIdx, tp: PlotIdx) {
    let (x, y) = g.map.xy(dest);
    order(g, grp, MissionType::MISSION_MOVE_TO, x, y, 0, tp);
    if alive(g, grp) && !g.groups[grp].mission_queue.is_empty() && !g.group_ready_to_move(grp, false) {
        g.group_clear_mission_queue(grp);
    }
}

/// [`order`] the group to hold its ground (fortify, else sleep, else skip).
fn hold_for(g: &mut Game, grp: GroupId, tp: PlotIdx) {
    for m in [MissionType::MISSION_FORTIFY, MissionType::MISSION_SLEEP, MissionType::MISSION_SKIP] {
        if m == MissionType::MISSION_SKIP || crate::units::can_start(g, grp, m, -1, -1) {
            order(g, grp, m, -1, -1, 0, tp);
            return;
        }
    }
}

/// The target city plot of the group's last campaign order (see `order`).
pub fn tagged(g: &Game, grp: GroupId) -> Option<PlotIdx> {
    if g.groups[grp].ai_mission_ai_type != Some(MissionAIType::MISSIONAI_ASSAULT) {
        return None;
    }
    g.group_ai_get_mission_ai_plot(grp)
}

/// Move the group to `dest`; when the rules find no path, to the reached plot nearest it.
fn go(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, dest: PlotIdx, tp: PlotIdx) {
    let here = g.unit_plot(u);
    if here == dest {
        hold_for(g, grp, tp);
        return;
    }
    march(g, grp, dest, tp);
    if alive(g, grp) && g.group_ready_to_move(grp, false) && g.units.contains(u) && g.unit_moves_left(u) > 0 {
        // No path: the reached plot nearest the destination.
        let (dx, dy) = g.map.xy(dest);
        let reach = v.flood.run(g, u, 8, false);
        let (hx, hy) = g.map.xy(here);
        let now = g.plot_distance(hx, hy, dx, dy);
        if let Some(r) = reach
            .iter()
            .filter(|r| {
                let (rx, ry) = g.map.xy(r.plot);
                let d = g.plot_distance(rx, ry, dx, dy);
                d < now && d >= 1
            })
            .min_by_key(|r| {
                let (rx, ry) = g.map.xy(r.plot);
                (g.plot_distance(rx, ry, dx, dy), r.turns, r.plot)
            })
            .copied()
        {
            march(g, grp, r.plot, tp);
        }
    }
    finish(g, grp);
}

/// Move toward the ring `ring` plots from the target: the reached plot on it the group gets to
/// first (the operation's assembly or staging plot on equal turns, so stacks meet; then the
/// best defense), else toward the operation's plot along the rules' path.
fn approach(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, op: &Op, ring: i32) {
    let here = g.unit_plot(u);
    let preferred = if ring == 1 { op.staging } else { op.assembly };
    let (tx, ty) = g.map.xy(op.plot);
    let team = v.team;
    let reach = v.flood.run(g, u, 12, false);
    let best = reach
        .iter()
        .filter(|r| {
            let (rx, ry) = g.map.xy(r.plot);
            g.plot_distance(rx, ry, tx, ty) == ring && g.map.plots[r.plot].plot_city.is_none()
        })
        .min_by_key(|r| (r.turns, Some(r.plot) != preferred, std::cmp::Reverse(g.plot_defense_modifier(r.plot, Some(team), true)), r.plot))
        .copied();
    crate::trace!(g, v.p, "  approach {:?} at {:?} -> ring {} of {:?}: {:?} (reach {}, preferred {:?})", grp, g.map.xy(here), ring, (tx, ty), best.map(|r| (g.map.xy(r.plot), r.turns)), reach.len(), preferred.map(|q| g.map.xy(q)));
    match best {
        Some(r) if r.plot == here => hold_for(g, grp, op.plot),
        Some(r) => {
            march(g, grp, r.plot, op.plot);
            finish(g, grp);
        }
        None => match preferred.or(op.assembly).or(op.staging) {
            Some(dest) => go(g, v, grp, u, dest, op.plot),
            None => go(g, v, grp, u, op.rally.unwrap_or(here), op.plot),
        },
    }
}

/// Siege engines in the group bombard the target's walls.
fn bombard(g: &mut Game, grp: GroupId, op: &Op) {
    if g.cities.contains(op.city) && g.cities[op.city].defense_damage < 100 && crate::units::can_start(g, grp, MissionType::MISSION_BOMBARD, -1, -1) {
        crate::units::push(g, grp, MissionType::MISSION_BOMBARD, -1, -1, 0, false);
    }
}

/// Is the group committed to an operation past gathering (marching, stepping in, striking)?
/// Such a stack leaves our cities in peril to their garrisons: the threat usually comes from
/// the city it marches on, and turning back each time it appears stalled the sieges (measured:
/// fewer cities taken, from civs and barbarians, and no fewer lost).
pub fn on_the_move(v: &View, grp: GroupId) -> bool {
    v.campaign.op_of(grp).is_some_and(|i| v.campaign.ops[i].phase != Phase::Gather)
}

/// Is the group next to the target of an operation that strikes this turn?
pub fn striking(g: &Game, v: &View, grp: GroupId) -> bool {
    let Some(op) = v.campaign.op_of(grp).map(|i| &v.campaign.ops[i]) else { return false };
    if op.phase != Phase::Strike {
        return false;
    }
    let (tx, ty) = g.map.xy(op.plot);
    g.group_get_head_unit(grp).is_some_and(|h| {
        let (x, y) = g.map.xy(g.unit_plot(h));
        g.plot_distance(x, y, tx, ty) <= 1
    })
}

/// Orders for a stack committed to an operation. `early`: called before hunting, when only
/// war operations and barbarian operations past gathering act. True if the group acted.
pub fn act(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId, early: bool) -> bool {
    plan(g, v);
    let Some(i) = v.campaign.op_of(grp) else { return false };
    let op = v.campaign.ops[i].clone();
    if !g.cities.contains(op.city) || g.cities[op.city].owner == v.p {
        return false;
    }
    let here = g.unit_plot(u);
    let (x, y) = g.map.xy(here);
    let (tx, ty) = g.map.xy(op.plot);
    let dist = g.plot_distance(x, y, tx, ty);
    // Against barbarians, a stack still gathering its force hunts first (unless it stands next
    // to the city: it steps back); once the force is there it marches (hunting would scatter
    // it again).
    if early && !op.civ && op.phase == Phase::Gather && dist > 1 {
        return false;
    }
    if dist <= 1 && crate::trace_on(v.p) {
        let a = crate::military::assess(g, grp, op.plot);
        let ours: Vec<String> = g.groups[grp].units.iter().map(|&o| g.d.units[g.units[o].unit_type].base.type_.trim_start_matches("UNIT_").to_string()).collect();
        let theirs: Vec<String> = g.map.plots[op.plot].units.iter().map(|&o| format!("{}({}%)", g.d.units[g.units[o].unit_type].base.type_.trim_start_matches("UNIT_"), 100 - g.units[o].damage)).collect();
        // (Our army units on the plots around the city: a single-unit stack with others around
        // is part of a joint strike, not a lone camp.)
        let around: usize = (0..DirectionType::NUM)
            .filter_map(|dir| g.plot_direction(tx, ty, DirectionType::from_usize(dir)))
            .map(|q| g.map.plots[q].units.iter().filter(|&&o| g.units[o].owner == v.p && is_army_unit(g, o)).count())
            .sum();
        crate::trace!(g, v.p, "stack {:?} ({} units) next to {} (defense {}%, {:?}, {} of ours around): {:?} ours {:?} theirs {:?}", grp, g.groups[grp].units.len(), g.cities[op.city].name, g.city_defense_modifier(op.city, false), op.phase, around, a, ours, theirs);
    }
    match op.phase {
        Phase::Strike if dist <= 1 => {
            if !op.struck {
                v.campaign.ops[i].struck = true;
                strike(g, v, &op);
            }
            if alive(g, grp) {
                finish(g, grp);
            }
        }
        Phase::Strike | Phase::StepIn => {
            if dist <= 1 {
                bombard(g, grp, &op);
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    hold_for(g, grp, op.plot);
                }
            } else {
                // From the assembly ring only: a stack coming from afar would otherwise step
                // next to the city alone, ahead of the others.
                approach(g, v, grp, u, &op, if dist == 2 { 1 } else { 2 });
            }
        }
        Phase::March => {
            if dist <= 1 && g.groups[grp].units.len() >= 2 {
                // Here early: hold the ground (siege engines work on the walls).
                bombard(g, grp, &op);
                if alive(g, grp) && g.group_ready_to_move(grp, false) {
                    hold_for(g, grp, op.plot);
                }
            } else if dist == 2 {
                hold_for(g, grp, op.plot);
            } else {
                approach(g, v, grp, u, &op, 2);
            }
        }
        Phase::Gather => {
            // Too weak (or the war is not declared yet): gather in the rally city (at war; the
            // stacks committed against a barbarian town stay where they are, hunting raiders
            // and holding our cities, until the force is there: gathering them in one city bared
            // the others); a force almost strong enough that watches the city from the assembly
            // plot waits there for the rest (stepping back would lose sight of the garrison).
            let close = op.at_war && op.have * 10 >= op.need * 7;
            if dist == 2 && close && g.plot_is_visible(op.plot, v.team) {
                hold_for(g, grp, op.plot);
                return true;
            }
            if dist <= 1 {
                crate::trace!(g, v.p, "stack {:?} ({} units) steps back from {}", grp, g.groups[grp].units.len(), g.cities[op.city].name);
                if close {
                    if let Some(a) = op.assembly {
                        go(g, v, grp, u, a, op.plot);
                        return true;
                    }
                }
            }
            if !op.civ && dist > 1 {
                return false;
            }
            let Some(r) = op.rally else { return false };
            go(g, v, grp, u, r, op.plot);
        }
    }
    true
}

/// The strike: the units of every committed stack next to the target attack this turn, one by
/// one, while the force left still covers the defenders left. Siege engines bombard first;
/// then collateral attackers while several defenders stand; then the best odds when they are
/// good, else the unit that is cheapest to lose for its strength (C++ `AI_getBestGroupSacrifice`
/// does the same in a stack attack). Each attacker is taken out of its stack first, as a human
/// selects a single unit: a stack whose other units have attacked cannot start a move.
fn strike(g: &mut Game, v: &mut View, op: &Op) {
    let p = v.p;
    let team = v.team;
    let tp = op.plot;
    let (tx, ty) = g.map.xy(tp);
    let next_to = |g: &Game, u: UnitId| {
        g.units.contains(u) && {
            let (x, y) = g.map.xy(g.unit_plot(u));
            g.plot_distance(x, y, tx, ty) == 1
        }
    };
    let groups: Vec<GroupId> = op.groups.iter().copied().filter(|&grp| alive(g, grp) && g.group_get_head_unit(grp).is_some_and(|h| next_to(g, h))).collect();
    for &grp in &groups {
        if alive(g, grp) {
            bombard(g, grp, op);
        }
    }
    let pool: Vec<UnitId> = groups.iter().filter(|&&grp| alive(g, grp)).flat_map(|&grp| g.groups[grp].units.clone()).collect();
    let name = g.cities[op.city].name.clone();
    let mut done: Vec<UnitId> = Vec::new();
    let mut fights = 0;
    for _ in 0..48 {
        if !g.cities.contains(op.city) || g.cities[op.city].owner == p {
            break;
        }
        let defs = visible_defenders(g, team, tp);
        if defs.is_empty() {
            break;
        }
        // Siege engines with walls left to batter wait (their attack would only bombard).
        let walls = g.city_defense_modifier(op.city, false) > 0;
        let units: Vec<UnitId> = pool
            .iter()
            .copied()
            .filter(|&u| next_to(g, u) && !done.contains(&u) && g.unit_moves_left(u) > 0 && !g.units[u].made_attack && attack_power(g, u) > 0 && g.unit_can_attack(u))
            .filter(|&u| !(walls && g.unit_bombard_rate(u) > 0))
            .collect();
        // Start with a margin; once started, go on while the rest still takes the city, or
        // while the defenders are worn down to half their health and we outnumber them (they
        // heal if we stop).
        let go = if fights == 0 {
            covers(g, op.city, tp, &defs, &units, true, false)
        } else {
            let hp: i32 = defs.iter().map(|&d| g.unit_curr_hit_points(d)).sum();
            let max: i32 = defs.iter().map(|&d| g.unit_max_hit_points(d)).sum();
            covers(g, op.city, tp, &defs, &units, false, false) || hp * 2 <= max && units.len() >= defs.len()
        };
        if crate::trace_on(p) {
            let s = simulate(g, tp, &defs, &units, 0);
            let hp: Vec<i32> = defs.iter().map(|&d| g.unit_curr_hit_points(d)).collect();
            crate::trace!(g, p, "  strike step {}: {} units, defenders hp {:?}, sim {:?}, go {}", fights, units.len(), hp, s, go);
        }
        if !go {
            break;
        }
        // The next attacker.
        let odds = |g: &Game, u: UnitId| g.plot_get_best_defender(tp, None, Some(p), Some(u), false, false, false).map_or(1000, |d| g.get_combat_odds(u, d));
        let with_odds: Vec<(UnitId, i32)> = units.iter().map(|&u| (u, odds(g, u))).collect();
        let best_odds = with_odds.iter().map(|&(_, o)| o).max().unwrap_or(0);
        let collateral = defs.len() >= 2 && with_odds.iter().any(|&(u, _)| g.unit_collateral_damage(u) > 0);
        let sacrifice = |u: UnitId| {
            let cost = g.d.units[g.units[u].unit_type].production_cost.max(1) as i64;
            attack_power(g, u) as i64 * (100 + g.unit_withdrawal_probability(u) as i64) * 100 / (cost * (10 + g.units[u].experience.max(0) as i64))
        };
        let pick = if collateral {
            with_odds.iter().filter(|&&(u, _)| g.unit_collateral_damage(u) > 0).max_by_key(|&&(u, o)| (o, std::cmp::Reverse(u))).map(|&(u, _)| u)
        } else if best_odds >= 680 {
            with_odds.iter().max_by_key(|&&(u, o)| (o, std::cmp::Reverse(u))).map(|&(u, _)| u)
        } else {
            with_odds.iter().max_by_key(|&&(u, _)| (sacrifice(u), std::cmp::Reverse(u))).map(|&(u, _)| u)
        };
        let Some(u) = pick else { break };
        // Out of its stack (a stack with spent units cannot start a move).
        if g.units[u].group.is_some_and(|grp| g.groups[grp].units.len() > 1) {
            g.unit_join_group(u, None, true);
        }
        let Some(grp) = g.units[u].group else { break };
        let moves = g.unit_moves_left(u);
        g.groups[grp].ai_group_attack = false;
        order(g, grp, MissionType::MISSION_MOVE_TO, tx, ty, MOVE_DIRECT_ATTACK, tp);
        fights += 1;
        if alive(g, grp) {
            g.groups[grp].ai_group_attack = false;
        }
        if g.units.contains(u) && g.unit_moves_left(u) == moves && !g.units[u].made_attack {
            // No attack happened.
            done.push(u);
        }
    }
    // The defenders are gone: move in.
    let taken = |g: &Game| !g.cities.contains(op.city) || g.cities[op.city].owner == p;
    if !taken(g) && g.cities.contains(op.city) && visible_defenders(g, team, tp).is_empty() {
        // (Workers left in the city are captured on the way in.)
        if let Some(u) = pool.iter().copied().find(|&u| next_to(g, u) && g.unit_moves_left(u) > 0 && g.unit_can_move_or_attack_into(u, tp, false)) {
            if g.units[u].group.is_some_and(|grp| g.groups[grp].units.iter().any(|&o| g.unit_moves_left(o) <= 0)) {
                g.unit_join_group(u, None, true);
            }
            if let Some(grp) = g.units[u].group {
                order(g, grp, MissionType::MISSION_MOVE_TO, tx, ty, MOVE_DIRECT_ATTACK, tp);
            }
        }
    }
    // Taken: the other units still able to move follow into the city (a lone, wounded victor
    // would lose it to the next counterattack).
    if taken(g) && g.cities.contains(op.city) {
        let mut n = 0;
        for &u in &pool {
            if n >= 3 {
                break;
            }
            if !next_to(g, u) || g.unit_moves_left(u) <= 0 || !g.unit_can_move_into(u, tp, false, false, false) {
                continue;
            }
            if g.units[u].group.is_some_and(|grp| g.groups[grp].units.len() > 1) {
                g.unit_join_group(u, None, true);
            }
            if let Some(grp) = g.units[u].group {
                march(g, grp, tp, tp);
                n += 1;
            }
        }
    }
    crate::trace!(g, p, "strike on {}: {} stacks, {} attacks, {}", name, groups.len(), fights, if taken(g) { "taken" } else { "holds" });
    // End the turn of every stack involved (including the units taken out of them).
    for u in pool {
        if let Some(grp) = g.units.contains(u).then(|| g.units[u].group).flatten() {
            finish(g, grp);
        }
    }
}

// ----------------------------------------------------------------------------
// Production
// ----------------------------------------------------------------------------

/// The force our operation wants: attackers and siege engines.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Force {
    pub attackers: i32,
    pub siege: i32,
    /// Strength (x100) of the strongest defender seen at the target, in its city (fortified,
    /// city defense, terrain); 0 when nothing is seen.
    pub defense: i32,
    /// Team that owns the target city (barbarian or a rival civ): who our operation is fighting,
    /// so callers can weigh our strength against theirs.
    pub target: Option<TeamId>,
}

/// The force the operation of `p` wants (what production builds toward), from the cheap
/// estimate of its target (the city our stacks are committed to, else the best value for its
/// cost): about two attackers per defender, siege engines when the city has walls or several
/// defenders.
pub fn needed_force(g: &Game, p: PlayerId) -> Option<Force> {
    let (cands, civ) = candidates(g, p);
    let c = cands.iter().max_by_key(|c| (committed_units(g, p, c.plot) > 0, rough_score(c), std::cmp::Reverse(c.city)))?;
    let walls = g.city_defense_modifier(c.city, false);
    let mut attackers = rough_need(c.n_def);
    let siege = if c.n_def >= 3 || walls >= 25 { (c.n_def / 2).clamp(1, 3) } else { 0 };
    attackers -= siege;
    attackers = attackers.min(if civ { 16 } else { 10 });
    let team = g.players[p].team;
    let defense = visible_defenders(g, team, c.plot).iter().map(|&d| g.unit_max_combat_str(d, Some(c.plot), None, None)).max().unwrap_or(0);
    let target = Some(g.players[g.cities[c.city].owner].team);
    Some(Force { attackers, siege, defense, target })
}

/// Can a unit of type `u` hurt a defender of strength `defense` (x100) in a city? At least half
/// its strength (with the unit's city attack bonus): a stack of them grinds the garrison
/// down; weaker ones only feed it experience.
fn useful_attacker(g: &Game, u: UnitType, defense: i32) -> bool {
    let info = &g.d.units[u];
    let att = crate::view::type_attack_power(g.d, u) * (100 + info.city_attack_modifier.max(-50)) / 100;
    defense <= 0 || att * 2 >= defense || info.collateral_damage > 0 || info.bombard_rate > 0
}

/// Production hook (army composition): a siege engine, the building that trains them, or an
/// attacker when our operation wants more than we have. `attackers`/`siege`: what we have
/// (alive or in training).
pub fn army_choice(g: &Game, c: CityId, attackers: i32, siege: i32, afford: bool) -> Option<Choice> {
    let p = g.cities[c].owner;
    if !afford || g.player_is_barbarian(p) {
        return None;
    }
    let f = needed_force(g, p)?;
    let war = candidates_war(g, p);
    // In peacetime the army takes every other slot (buildings matter too).
    if !war && (g.game_turn() + c.0 as i32) % 2 != 0 {
        return None;
    }
    let mut choice = None;
    if siege < f.siege {
        choice = best_unit(g, c, Role::Siege).map(|u| Choice::Train(u, UnitAIType::UNITAI_ATTACK_CITY)).or_else(|| siege_building(g, c).map(Choice::Construct));
    }
    if choice.is_none() && attackers + siege < f.attackers + f.siege {
        let role = if war || f.defense > 0 { Role::CityAttack } else { Role::Attack };
        choice = best_unit(g, c, role).filter(|&u| useful_attacker(g, u, f.defense)).map(|u| Choice::Train(u, UnitAIType::UNITAI_ATTACK));
    }
    if let Some(ch) = choice {
        crate::trace!(g, p, "{}: the campaign wants {:?} (have {} + {} siege): {}", g.cities[c].name, f, attackers, siege, crate::production::choice_name(g, ch));
    }
    choice
}

/// At war with a civ (or preparing one).
fn candidates_war(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t)) || crate::diplomacy::preparing_target(g, team).is_some()
}

/// The building that lets city `c` train our best siege engine (FfH Siege Workshop), when no
/// city of ours can train one yet, none is building it, and `c` is our most productive city.
fn siege_building(g: &Game, c: CityId) -> Option<BuildingType> {
    let p = g.cities[c].owner;
    let d = g.d;
    let civ = g.player_get_civilization_type(p)?;
    let pl = &g.players[p];
    if pl.cities.iter().any(|&o| best_unit(g, o, Role::Siege).is_some()) {
        return None;
    }
    let prod = |o: CityId| g.city_yield_rate(o, YieldType::YIELD_PRODUCTION);
    if pl.cities.iter().any(|&o| o != c && !g.city_is_occupation(o) && (prod(o), std::cmp::Reverse(o)) > (prod(c), std::cmp::Reverse(c))) {
        return None;
    }
    for u in d.units.keys() {
        let info = &d.units[u];
        if info.domain_type != Some(DomainType::DOMAIN_LAND) || info.bombard_rate <= 0 && info.collateral_damage <= 0 {
            continue;
        }
        if !(info.unit_ai_type[UnitAIType::UNITAI_ATTACK_CITY] || info.unit_ai_type[UnitAIType::UNITAI_COLLATERAL]) {
            continue;
        }
        let Some(class) = info.unit_class_type else { continue };
        if d.civ_unit(civ, class) != Some(u) || !g.player_can_train(p, u, false, false, false) {
            continue;
        }
        let Some(bc) = info.prereq_building_class else { continue };
        let Some(b) = d.civ_building(civ, bc) else { continue };
        if g.city_is_has_building_class(c, bc) || !g.city_can_construct(c, b, false, false, false) {
            continue;
        }
        if pl.cities.iter().any(|&o| g.city_production_building(o) == Some(b)) {
            return None;
        }
        return Some(b);
    }
    None
}

/// Siege engines and attackers we have (alive or in training), for traces.
pub fn army_counts(g: &Game, p: PlayerId) -> (i32, i32) {
    let att = count_ai(g, p, UnitAIType::UNITAI_ATTACK);
    let siege = count_ai(g, p, UnitAIType::UNITAI_ATTACK_CITY) + count_ai(g, p, UnitAIType::UNITAI_COLLATERAL);
    (att, siege)
}
