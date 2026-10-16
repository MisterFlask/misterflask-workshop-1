//! Finance: avoid and escape the zero-research death spiral (docs/AI.md, "Finance").
//!
//! An empire whose costs (city maintenance, civic upkeep, unit support, all inflated) eat its
//! commerce pushes the research slider down to pay them; with research at zero it never
//! reaches the techs that would cut those costs (City States, Aristocracy, Courthouses), so it
//! stays there for the rest of the game. [`state`] reads the empire's finances from the rules'
//! own numbers (C++ `calculateInflatedCosts` and its parts, the commerce rates): the gold rate
//! at the research share we want to keep (60% at peace, 40% at war), the cost breakdown,
//! whether the deficit is *structural* (maintenance and civic upkeep alone eat what that share
//! leaves for gold) or *temporary* (unit support, which can be shed), and the turns of reserve
//! left. The responses, each behind a single hook in a shared module:
//! - **civics** (`empire::civic_value`, `empire::civics`): upkeep and maintenance are valued as
//!   gold at the rate of a broke empire as soon as the deficit is structural ([`gold_value`]),
//!   maintenance from the cities' actual distance and number-of-cities parts
//!   ([`civic_maintenance_x100`]); Aristocracy is charged for FfH's AI-only cottage ban
//!   ([`civic_penalty`]); a structural deficit re-checks civics outside the 10-turn cadence
//!   when a cheaper civic would win ([`civics_due`]);
//! - **unit support** (`empire::disband`): obsolete and surplus units go first while support
//!   keeps research below its target ([`disband`]);
//! - **research** (`empire::research`): the cheapest tech that unlocks a cost-cutting civic or
//!   a Courthouse-like building, while research is down to 30% or the deficit is structural
//!   ([`escape_tech`]);
//! - **citizens** (ffh-core `governor_weights`): commerce is weighted by the same measured
//!   shortfall, computed there from the rules;
//! - **expansion**: [`city_affordable`] for the settler logic (not wired in).
//!
//! Everything is a pure function of the game state: no cache, no random draw.

use ffh_core::defs::{CITY_HOME_PLOT, NUM_CITY_PLOTS};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// The empire's finances this turn (see the module docs).
#[derive(Clone, Copy, Debug, Default)]
pub struct Finance {
    /// Gold + research commerce at the current sliders.
    pub commerce: i32,
    /// City maintenance, civic upkeep and unit cost + supply (before inflation).
    pub maintenance: i32,
    pub civics: i32,
    pub units: i32,
    /// Inflation, percent.
    pub inflation: i32,
    /// All costs, inflated.
    pub costs: i32,
    /// The research share we want to keep: 60% at peace, 40% at war.
    pub target: i32,
    /// Net gold per turn with research at `target`.
    pub gold_at_target: i32,
    /// Net gold per turn now, and the treasury.
    pub rate: i32,
    pub gold: i32,
    pub research_pct: i32,
    /// The research slider can move (FfH: not for every civ).
    pub flexible: bool,
    pub at_war: bool,
}

impl Finance {
    /// Gold per turn missing to research at the target share.
    pub fn deficit(&self) -> i32 {
        (-self.gold_at_target).max(0)
    }

    /// Unit support per turn, inflated.
    pub fn unit_costs(&self) -> i32 {
        self.units * (100 + self.inflation.max(0)) / 100
    }

    /// The deficit outlives every paid unit: maintenance and civic upkeep alone eat what the
    /// target research share leaves for gold.
    pub fn structural(&self) -> bool {
        self.flexible && self.gold_at_target + self.unit_costs() < 0
    }

    /// Percent of the commerce taken by costs.
    pub fn cost_share(&self) -> i32 {
        self.costs * 100 / self.commerce.max(1)
    }

    /// Percent of the commerce taken by unit support.
    pub fn unit_share(&self) -> i32 {
        self.unit_costs() * 100 / self.commerce.max(1)
    }

    /// Turns until the treasury is empty at the current rate (`i32::MAX` when not losing gold).
    pub fn reserve_turns(&self) -> i32 {
        if self.rate >= 0 {
            i32::MAX
        } else {
            self.gold / -self.rate
        }
    }

    /// Unit support takes a tenth of the commerce while research is below its target.
    pub fn support_heavy(&self) -> bool {
        self.flexible && self.units > 0 && self.unit_share() >= 10 && self.research_pct < self.target
    }
}

/// At war with a civ, or preparing a war.
fn at_war(g: &Game, p: PlayerId) -> bool {
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t)) || crate::diplomacy::preparing_target(g, team).is_some()
}

/// The research share the empire wants to keep.
pub fn target_research(at_war: bool) -> i32 {
    if at_war {
        40
    } else {
        60
    }
}

/// The empire's finances now.
pub fn state(g: &Game, p: PlayerId) -> Finance {
    let pl = &g.players[p];
    let war = at_war(g, p);
    let target = target_research(war);
    let commerce = g.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD) + g.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH);
    Finance {
        commerce,
        maintenance: g.player_get_total_maintenance(p),
        civics: g.player_get_civic_upkeep(p, None, false),
        units: g.player_calculate_unit_cost(p) + g.player_calculate_unit_supply(p),
        inflation: g.player_calculate_inflation_rate(p),
        costs: g.player_calculate_inflated_costs(p),
        target,
        gold_at_target: crate::economy::gold_at_research(g, p, target),
        rate: g.player_calculate_gold_rate(p),
        gold: pl.gold,
        research_pct: crate::economy::spend_pct(g, p),
        flexible: g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH),
        at_war: war,
    }
}

// ---------------------------------------------------------------------------------------------
// Civics
// ---------------------------------------------------------------------------------------------

/// Value of one gold per turn in `empire::civic_value` points: what it is worth to an empire
/// that has pushed research below half to pay its bills, and already before the slider has
/// fallen when the deficit is structural.
pub fn gold_value(f: &Finance) -> i32 {
    if f.flexible && (f.research_pct < 50 || f.structural()) {
        40
    } else {
        15
    }
}

/// City maintenance (x100 gold per turn, before inflation) split into the two parts civics
/// scale (C++ `calculateDistanceMaintenanceTimes100`, `calculateNumCitiesMaintenanceTimes100`),
/// with the player's current civic modifiers taken out and the cities' own modifiers
/// (Courthouses) applied.
fn maintenance_base_x100(g: &Game, p: PlayerId) -> (i64, i64) {
    let pl = &g.players[p];
    let (mut dist, mut num) = (0i64, 0i64);
    for &c in &pl.cities {
        let city = &g.cities[c];
        // C++ updateMaintenance: none in disorder, during We Love the King Day or at size 0.
        if g.city_is_disorder(c) || city.we_love_the_king_day || city.population <= 0 {
            continue;
        }
        let m = (100 + city.maintenance_modifier).max(0) as i64;
        dist += g.city_calculate_distance_maintenance_times100(c) as i64 * m / 100;
        num += g.city_calculate_num_cities_maintenance_times100(c) as i64 * m / 100;
    }
    let dm = (100 + pl.distance_maintenance_modifier).max(1) as i64;
    let nm = (100 + pl.num_cities_maintenance_modifier).max(1) as i64;
    (dist * 100 / dm, num * 100 / nm)
}

/// Gold per turn (x100, inflated) that running `civic` adds to city maintenance (negative: it
/// saves maintenance).
pub fn civic_maintenance_x100(g: &Game, p: PlayerId, civic: CivicType) -> i32 {
    let info = &g.d.civics[civic];
    if info.distance_maintenance_modifier == 0 && info.num_cities_maintenance_modifier == 0 {
        return 0;
    }
    let (dist, num) = maintenance_base_x100(g, p);
    let v = (dist * info.distance_maintenance_modifier as i64 + num * info.num_cities_maintenance_modifier as i64) / 100;
    (v * (100 + g.player_calculate_inflation_rate(p).max(0)) as i64 / 100) as i32
}

/// Gold per turn (x100, inflated) that running `civic` costs: its upkeep and its effect on city
/// maintenance.
fn civic_cost_x100(g: &Game, p: PlayerId, civic: CivicType) -> i32 {
    let upkeep = g.player_get_single_civic_upkeep(p, Some(civic), true) * (100 + g.player_calculate_inflation_rate(p).max(0));
    upkeep + civic_maintenance_x100(g, p, civic)
}

/// What `empire::civic_value` cannot see from the civic's own numbers: FfH's AI-only
/// `canBuild` (CvGameUtils.py) bans cottages under Aristocracy, the main commerce improvement
/// once Education is known. Charged when we know Education, research it or can research it
/// next: the cottage sites our cities would use (a third of their citizens), each worth about
/// two commerce.
pub fn civic_penalty(g: &Game, p: PlayerId, civic: CivicType) -> i32 {
    let d = g.d;
    if d.civics[civic].base.type_ != "CIVIC_ARISTOCRACY" {
        return 0;
    }
    let Some((_, build)) = d.builds.iter_enumerated().find(|(_, b)| b.base.type_ == "BUILD_COTTAGE") else { return 0 };
    let Some(imp) = build.improvement else { return 0 };
    let team = g.players[p].team;
    let soon = match build.tech_prereq {
        None => true,
        Some(t) => g.team_is_has_tech(team, Some(t)) || g.player_get_current_research(p) == Some(t) || g.player_can_research(p, t, false),
    };
    if !soon {
        return 0;
    }
    let mut sites = 0;
    for &c in &g.players[p].cities {
        let mut n = 0;
        for i in 0..NUM_CITY_PLOTS {
            if i == CITY_HOME_PLOT {
                continue;
            }
            let Some(q) = g.city_index_plot(c, i) else { continue };
            let plot = &g.map.plots[q];
            if plot.owner != Some(p) || plot.improvement_type.is_some() || plot.bonus_type.is_some() {
                continue;
            }
            if g.plot_can_have_improvement(q, imp, Some(team), false) {
                n += 1;
            }
        }
        sites += n.min(1 + g.cities[c].population / 3);
    }
    // 9 points per commerce, as `civic_value` counts improvement yields.
    sites * 2 * 9
}

/// A structural deficit, outside the 10-turn civics cadence: is some civic cheaper (upkeep +
/// maintenance) than the one we run in its option by at least 2 gold per turn, and better
/// overall by `empire::civic_value` (as the revolution test judges it)? Then civics are
/// re-checked now.
pub fn civics_due(g: &Game, p: PlayerId) -> bool {
    let f = state(g, p);
    if !f.structural() || g.players[p].revolution_timer > 0 || g.player_is_anarchy(p) {
        return false;
    }
    let d = g.d;
    let current = &g.players[p].civics;
    for opt in d.civic_options.keys() {
        let Some(cur) = current[opt] else { continue };
        let cur_cost = civic_cost_x100(g, p, cur);
        let mut cur_value = None;
        for c in d.civics.keys() {
            if c == cur || d.civics[c].civic_option_type != Some(opt) || !g.player_can_do_civics(p, Some(c)) {
                continue;
            }
            if civic_cost_x100(g, p, c) + 200 > cur_cost {
                continue;
            }
            let cur_v = *cur_value.get_or_insert_with(|| crate::empire::civic_value(g, p, cur));
            if crate::empire::civic_value(g, p, c) > cur_v.max(0) + 100 {
                crate::trace!(g, p, "finance: structural deficit {} (commerce {}, costs {}%), {} is cheaper than {}", f.deficit(), f.commerce, f.cost_share(), d.civics[c].base.type_, d.civics[cur].base.type_);
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Unit support
// ---------------------------------------------------------------------------------------------

/// A unit's strength relative to the best its owner can now train for the same job (percent):
/// low values are obsolete units.
fn relative_power(g: &Game, u: UnitId, best_def: i64, best_att: i64, garrison: bool) -> i64 {
    let info = &g.d.units[g.units[u].unit_type];
    if garrison {
        crate::production::type_city_defense(info) * 100 / best_def.max(1)
    } else {
        crate::view::type_attack_power(g.d, g.units[u].unit_type) as i64 * 100 / best_att.max(1)
    }
}

/// While unit support keeps research below its target share ([`Finance::support_heavy`]) and
/// we are not at war: disband up to two units a turn (one when the deficit is a single gold),
/// obsolete ones first (weakest relative to what we can train now; every unit costs the same
/// gold), from the surplus: field units beyond a mobile force of 1 + cities/4, garrison units
/// beyond one per city (two in the capital and near barbarians) in cities with no threat in
/// sight, then idle workers beyond twice what the worked plots need. Never heroes, units that
/// would defect, summons, a city's last defender. Returns whether it disbanded anything
/// (`empire::disband` then stops).
pub fn disband(g: &mut Game, p: PlayerId) -> bool {
    let f = state(g, p);
    if !f.support_heavy() || f.at_war || g.player_calculate_unit_cost(p) == 0 {
        return false;
    }
    let d = g.d;
    // Best defender / attacker we can train now (in any city).
    let mut best_def = 0i64;
    let mut best_att = 0i64;
    for &c in &g.players[p].cities {
        if let Some(u) = crate::production::best_unit(g, c, crate::production::Role::Defense) {
            best_def = best_def.max(crate::production::type_city_defense(&d.units[u]));
        }
        if let Some(u) = crate::production::best_unit(g, c, crate::production::Role::Attack) {
            best_att = best_att.max(crate::view::type_attack_power(d, u) as i64);
        }
    }
    let no_scrap = |g: &Game, u: UnitId| {
        let unit = &g.units[u];
        let info = &g.d.units[unit.unit_type];
        let hero = info.default_unit_ai_type == Some(UnitAIType::UNITAI_HERO)
            || info.unit_class_type.is_some_and(|c| g.d.unit_classes[c].max_global_instances > 0 || g.d.unit_classes[c].max_player_instances > 0);
        let betrays = g.d.promotions.keys().any(|pr| unit.has_promotion[pr] && g.d.promotions[pr].betrayal_chance != 0);
        hero || betrays || unit.duration > 0 || !g.unit_can_scrap(u)
    };
    // (relative power, unit): the weakest go first.
    let mut cands: Vec<(i64, UnitId)> = Vec::new();
    // Field units beyond the mobile force.
    let field = |g: &Game, u: UnitId| {
        let unit = &g.units[u];
        matches!(unit.ai_type, Some(UnitAIType::UNITAI_ATTACK) | Some(UnitAIType::UNITAI_RESERVE) | Some(UnitAIType::UNITAI_COUNTER) | Some(UnitAIType::UNITAI_ATTACK_CITY) | Some(UnitAIType::UNITAI_COLLATERAL))
            && g.d.units[unit.unit_type].combat > 0
            && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND
    };
    let mut fields: Vec<UnitId> = g.players[p].units.iter().copied().filter(|&u| field(g, u) && !no_scrap(g, u)).collect();
    let keep_field = 1 + g.players[p].cities.len() as i32 / 4;
    fields.sort_by_key(|&u| (relative_power(g, u, best_def, best_att, false) + 10 * g.units[u].experience as i64, u));
    let surplus_field = (fields.len() as i32 - keep_field).max(0) as usize;
    for &u in fields.iter().take(surplus_field) {
        cands.push((relative_power(g, u, best_def, best_att, false), u));
    }
    // Garrison units beyond the quota production keeps when it cannot afford more.
    for &c in &g.players[p].cities {
        let plot = g.city_plot(c);
        if crate::production::local_threat(g, p, plot, 3) > 0 {
            continue;
        }
        let keep = if g.city_is_capital(c) || crate::production::barbarians_near(g, p, plot, 4) { 2 } else { 1 };
        let mut garrison: Vec<UnitId> = g.map.plots[plot]
            .units
            .iter()
            .copied()
            .filter(|&u| {
                let unit = &g.units[u];
                unit.owner == p && matches!(unit.ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER)) && g.d.units[unit.unit_type].combat > 0
            })
            .collect();
        if garrison.len() as i32 <= keep {
            continue;
        }
        garrison.sort_by_key(|&u| (crate::view::defense_power(g, u), u));
        let surplus = garrison.len() - keep as usize;
        for &u in garrison.iter().take(surplus) {
            if !no_scrap(g, u) {
                cands.push((relative_power(g, u, best_def, best_att, true), u));
            }
        }
    }
    // Idle workers (no job this turn) beyond twice what the worked plots need (as production
    // counts them): that need moves as cities grow and switch tiles.
    let n = g.players[p].cities.len() as i32;
    let unimproved = crate::production::unimproved_worked_plots(g, p);
    let need = 2 * (n.min(3) + (unimproved + 1) / 3).min(n * 2).max(1);
    let workers: Vec<UnitId> = g.players[p]
        .units
        .iter()
        .copied()
        .filter(|&u| g.units[u].ai_type == Some(UnitAIType::UNITAI_WORKER) && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND)
        .collect();
    if workers.len() as i32 > need {
        let idle = workers.iter().copied().filter(|&u| !no_scrap(g, u) && g.units[u].group.is_some_and(|grp| g.groups[grp].mission_queue.is_empty()));
        for u in idle.take((workers.len() as i32 - need) as usize) {
            cands.push((150, u));
        }
    }
    if cands.is_empty() {
        return false;
    }
    cands.sort();
    let max = if f.deficit() > 1 { 2 } else { 1 };
    let mut done = 0;
    for (key, u) in cands {
        if done >= max || !g.units.contains(u) {
            break;
        }
        crate::trace!(g, p, "finance: support {}% of commerce, disbanding {} ({}% of our best)", f.unit_share(), d.units[g.units[u].unit_type].base.type_, key);
        g.unit_scrap(u);
        done += 1;
    }
    done > 0
}

// ---------------------------------------------------------------------------------------------
// Research
// ---------------------------------------------------------------------------------------------

/// The cheapest way out of a deficit through research: among the techs that unlock a civic we
/// would adopt (by `empire::civic_value`) and that costs at least 3 gold per turn less than the
/// one we run (City States or Aristocracy out of Despotism), or a building that cuts our
/// cities' maintenance by that much (Courthouse; counted at half, it is built one city at a
/// time), the one with the most gold saved per beaker on the way (prerequisites included).
/// `None` unless there is a deficit at the target share and research is down to 30% or the
/// deficit is structural.
pub fn escape_tech(g: &Game, p: PlayerId) -> Option<TechType> {
    let f = state(g, p);
    if !f.flexible || f.deficit() == 0 || !(f.structural() || f.research_pct <= 30) {
        return None;
    }
    let d = g.d;
    let team = g.players[p].team;
    let civ = g.player_get_civilization_type(p)?;
    // (tech, gold saved per turn x100)
    let mut gains: Vec<(TechType, i32)> = Vec::new();
    let mut add = |t: TechType, v: i32| {
        if let Some(e) = gains.iter_mut().find(|(o, _)| *o == t) {
            e.1 = e.1.max(v);
        } else {
            gains.push((t, v));
        }
    };
    let current = &g.players[p].civics;
    for c in d.civics.keys() {
        let info = &d.civics[c];
        let (Some(t), Some(opt)) = (info.tech_prereq, info.civic_option_type) else { continue };
        if g.team_is_has_tech(team, Some(t)) || !g.player_can_ever_research(p, t) {
            continue;
        }
        let Some(cur) = current[opt] else { continue };
        let saved = civic_cost_x100(g, p, cur) - civic_cost_x100(g, p, c);
        if saved < 300 || crate::empire::civic_value(g, p, c) <= crate::empire::civic_value(g, p, cur) {
            continue;
        }
        add(t, saved);
    }
    for b in d.buildings.keys() {
        let info = &d.buildings[b];
        if info.maintenance_modifier >= 0 || info.prereq_religion.is_some() {
            continue;
        }
        let Some(t) = info.prereq_and_tech else { continue };
        if g.team_is_has_tech(team, Some(t)) || !g.player_can_ever_research(p, t) || !info.prereq_and_techs.iter().flatten().all(|&o| o == t || g.team_is_has_tech(team, Some(o))) {
            continue;
        }
        let Some(class) = info.building_class_type else { continue };
        if d.civ_building(civ, class) != Some(b) || d.building_classes[class].max_global_instances > 0 || d.building_classes[class].max_player_instances > 0 {
            continue;
        }
        let saved: i32 = g.players[p].cities.iter().map(|&c| -info.maintenance_modifier * g.cities[c].maintenance.max(0) / 100).sum();
        if saved / 2 >= 300 {
            add(t, saved / 2);
        }
    }
    let mut best: Option<(TechType, i64)> = None;
    for (t, saved) in gains {
        let cost = g.player_find_path_length(p, t, true).max(1) as i64;
        let score = saved as i64 * 10000 / cost;
        if best.map_or(true, |(_, s)| score > s) {
            best = Some((t, score));
        }
    }
    best.map(|(t, _)| t)
}

/// `empire::research` hook: research [`escape_tech`] now (the beakers already spent on the
/// current tech stay with it in the rules). True when the escape tech is being researched.
pub fn escape_research(g: &mut Game, p: PlayerId) -> bool {
    let Some(t) = escape_tech(g, p) else { return false };
    if g.players[p].researching_tech[t] {
        return true;
    }
    let f = state(g, p);
    crate::trace!(g, p, "finance: research {} to cut costs (deficit {}, costs {}% of commerce, {} turns of reserve)", g.d.techs[t].base.type_, f.deficit(), f.cost_share(), f.reserve_turns());
    g.player_push_research(p, t, true)
}

// ---------------------------------------------------------------------------------------------
// Expansion
// ---------------------------------------------------------------------------------------------

/// Can the empire carry one more city and still research at the target share? A new city
/// costs about one and a half times the average city's maintenance (its own, plus the
/// number-of-cities part it adds to every other city) and half the average civic upkeep per
/// city; it brings half an average city's commerce, of which the gold share counts. Settlers
/// already on their way count as cities. For the settler logic; not wired in yet (measured
/// wired into `production::settler_choice` on seeds 1-12: one civ at 20% research or less at
/// turn 200, none at turn 300, minimum techs 11 at turn 300, but 7% fewer cities and no better
/// score).
pub fn city_affordable(g: &Game, p: PlayerId) -> bool {
    let f = state(g, p);
    if !f.flexible {
        return true;
    }
    let n = g.players[p].cities.len() as i32;
    if n == 0 {
        return true;
    }
    let settlers = crate::production::count_ai(g, p, UnitAIType::UNITAI_SETTLE);
    let cost = (f.maintenance * 3 / 2 + f.civics / 2) * (100 + f.inflation.max(0)) / 100;
    let gain = f.commerce / 2 * (100 - f.target) / 100;
    f.gold_at_target * n + gain >= cost * (1 + settlers)
}
