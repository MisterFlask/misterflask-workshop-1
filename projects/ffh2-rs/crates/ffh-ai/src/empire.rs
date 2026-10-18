//! Empire-level choices: research queue, civics, state religion, commerce sliders.

use ffh_core::defs::*;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

pub fn manage(g: &mut Game, p: PlayerId) {
    research(g, p);
    civics(g, p);
    religion(g, p);
    sliders(g, p);
    disband(g, p);
    hurry(g, p);
}

/// Hurry production where it matters (FfH allows it under some civics: gold under Arete,
/// Caste System, Guilds, Military State; population under Slavery): a defender for a city
/// under threat, a settler or worker; with gold well above the reserve, anything nearly done
/// buys its last turns. Population only from a city at its happy cap, one citizen at a time,
/// once the anger from the last one has passed.
fn hurry(g: &mut Game, p: PlayerId) {
    let d = g.d;
    let hurries: Vec<HurryType> = d.hurries.keys().filter(|&h| g.player_can_hurry(p, h)).collect();
    if hurries.is_empty() {
        return;
    }
    let reserve = 30 + 5 * g.players[p].cities.len() as i32;
    for c in g.players[p].cities.clone() {
        let urgent = match g.city_production_unit(c) {
            Some(u) => {
                let info = &d.units[u];
                let plot = g.city_plot(c);
                let threatened = crate::production::local_threat(g, p, plot, 2) > crate::production::garrison(g, p, plot).1;
                info.found || info.work_rate > 0 && info.combat == 0 || threatened && info.combat > 0
            }
            None => false,
        };
        let turns = g.city_production_turns_left(c);
        if turns <= 1 {
            continue;
        }
        for &h in &hurries {
            if !g.city_can_hurry(c, h, false) {
                continue;
            }
            if d.hurries[h].gold_per_production > 0 {
                let cost = g.city_hurry_gold(c, h);
                let rich = g.players[p].gold - cost > reserve * 3 && turns <= 4;
                if (urgent && g.players[p].gold - cost >= reserve) || rich {
                    crate::trace!(g, p, "hurries {} with {} gold", g.cities[c].name, cost);
                    g.city_hurry(c, h);
                    break;
                }
            } else if d.hurries[h].production_per_population > 0 {
                let capped = g.city_happy_level(c) - g.city_unhappy_level(c, 0) <= 0;
                if urgent && capped && g.cities[c].population >= 4 && g.city_hurry_population(c, h) == 1 && g.cities[c].hurry_anger_timer == 0 {
                    crate::trace!(g, p, "hurries {} with a citizen", g.cities[c].name);
                    g.city_hurry(c, h);
                    break;
                }
            }
        }
    }
}

/// A bankrupt empire (research slider at the bottom and still losing gold) disbands its
/// weakest surplus garrison units: never a city's last defender (two in the capital), never a
/// unit that would defect to the barbarians, at most two per turn.
fn disband(g: &mut Game, p: PlayerId) {
    if !g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) || g.player_calculate_unit_cost(p) == 0 {
        return;
    }
    if crate::finance::disband(g, p) { return; } // bankruptcy-guard
    let pct = crate::economy::spend_pct(g, p);
    // Field units go first, as soon as research has dropped to a fifth.
    if pct > 10 && pct <= 20 {
        disband_field_units(g, p, 1);
        return;
    }
    if pct > 10 {
        return;
    }
    if g.player_calculate_gold_rate(p) >= 0 && g.players[p].gold > 30 {
        disband_field_units(g, p, 1);
        return;
    }
    let mut done = 0;
    for c in g.players[p].cities.clone() {
        if done >= 2 {
            break;
        }
        let plot = g.city_plot(c);
        if crate::production::local_threat(g, p, plot, 3) > 0 {
            continue;
        }
        // As many as production wants there (see production::decide).
        let keep = if g.city_is_capital(c) || crate::production::barbarians_near(g, p, plot, 4) { 2 } else { 1 };
        let mut garrison: Vec<UnitId> = g.map.plots[plot]
            .units
            .iter()
            .copied()
            .filter(|&u| {
                let unit = &g.units[u];
                unit.owner == p
                    && matches!(unit.ai_type, Some(UnitAIType::UNITAI_CITY_DEFENSE) | Some(UnitAIType::UNITAI_CITY_COUNTER))
                    && g.d.units[unit.unit_type].combat > 0
            })
            .collect();
        if garrison.len() as i32 <= keep {
            continue;
        }
        garrison.sort_by_key(|&u| (crate::view::defense_power(g, u), u));
        let u = garrison[0];
        let betrays = g.d.promotions.keys().any(|pr| g.units[u].has_promotion[pr] && g.d.promotions[pr].betrayal_chance != 0);
        if betrays || !g.unit_can_scrap(u) {
            continue;
        }
        crate::trace!(g, p, "bankrupt: disbanding {} in {}", g.d.units[g.units[u].unit_type].base.type_, g.cities[c].name);
        g.unit_scrap(u);
        done += 1;
    }
    disband_field_units(g, p, 2 - done);
}

/// Field units beyond a small mobile force (1 + cities / 4) when the empire cannot pay for
/// them and is at peace: the weakest, least experienced first, never heroes or units that
/// would defect.
fn disband_field_units(g: &mut Game, p: PlayerId, max: i32) {
    if max <= 0 || g.player_calculate_unit_cost(p) == 0 {
        return;
    }
    let team = g.players[p].team;
    let at_war = g.teams.keys().any(|t| t != team && t != g.barbarian_team() && g.teams[t].alive_count > 0 && g.at_war(team, t))
        || crate::diplomacy::preparing_target(g, team).is_some();
    if at_war {
        return;
    }
    let field = |g: &Game, u: UnitId| {
        let unit = &g.units[u];
        matches!(unit.ai_type, Some(UnitAIType::UNITAI_ATTACK) | Some(UnitAIType::UNITAI_RESERVE) | Some(UnitAIType::UNITAI_COUNTER) | Some(UnitAIType::UNITAI_ATTACK_CITY) | Some(UnitAIType::UNITAI_COLLATERAL))
            && g.d.units[unit.unit_type].combat > 0
            && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND
    };
    let mut units: Vec<UnitId> = g.players[p].units.iter().copied().filter(|&u| field(g, u)).collect();
    let keep = 1 + g.players[p].cities.len() as i32 / 4;
    if units.len() as i32 <= keep {
        return;
    }
    units.sort_by_key(|&u| (crate::view::attack_power(g, u) + 50 * g.units[u].experience, u));
    let mut done = 0;
    for u in units {
        if done >= max || !g.units.contains(u) {
            break;
        }
        let n = g.players[p].units.iter().filter(|&&o| field(g, o)).count() as i32;
        if n <= keep {
            break;
        }
        let info = &g.d.units[g.units[u].unit_type];
        let hero = info.default_unit_ai_type == Some(UnitAIType::UNITAI_HERO) || info.unit_class_type.is_some_and(|c| g.d.unit_classes[c].max_global_instances > 0 || g.d.unit_classes[c].max_player_instances > 0);
        let betrays = g.d.promotions.keys().any(|pr| g.units[u].has_promotion[pr] && g.d.promotions[pr].betrayal_chance != 0);
        if hero || betrays || g.units[u].duration > 0 || !g.unit_can_scrap(u) {
            continue;
        }
        crate::trace!(g, p, "cannot pay: disbanding field unit {}", info.base.type_);
        g.unit_scrap(u);
        done += 1;
    }
}

fn research(g: &mut Game, p: PlayerId) {
    if crate::finance::escape_research(g, p) { return; } // bankruptcy-guard
    if !g.players[p].research_queue.is_empty() {
        return;
    }
    if let Some(t) = crate::research::choose(g, p) {
        g.player_push_research(p, t, true);
    }
}

/// Value of running `civic` (AI heuristic).
pub(crate) fn civic_value(g: &Game, p: PlayerId, civic: CivicType) -> i32 {
    let d = g.d;
    let info = &d.civics[civic];
    let pl = &g.players[p];
    let n = pl.cities.len() as i32;
    let pop = pl.total_population;
    // Conquest (`bMilitaryFoodProduction`): a city training a military unit puts its whole food
    // surplus into the unit and stops growing, and production trains units most of the time.
    // Its XML weight (700 a city) outbid every other economy civic, so most civs ran it and their
    // cities stalled; it gets none, and is valued like any civic by its other terms.
    let mut v = if info.military_food_production { 0 } else { info.ai_weight * n.max(1) };
    if let Some(l) = pl.personality_type.map(|l| &d.leader_heads[l]) {
        if l.favorite_civic == Some(civic) {
            v += 300 + 50 * n;
        }
    }
    let yw = [3, 3, 2];
    for i in 0..YieldType::NUM {
        v += info.yield_modifier[i] * pop * yw[i] / 10;
        v += info.capital_yield_modifier[i] * yw[i];
    }
    for i in 0..CommerceType::NUM {
        v += info.commerce_modifier[i] * pl.commerce_rate[i].max(0) / 25;
        v += info.capital_commerce_modifier[i];
    }
    v += info.largest_city_happiness * 20 + info.extra_health * 15 * n;
    if pl.state_religion_count > 0 || g.player_get_state_religion(p).is_some() {
        v += info.state_religion_happiness * 20 * n;
    }
    v += info.non_state_religion_happiness * 10 * n;
    v += info.free_experience * 15 + info.free_specialist * 30 * n;
    v += info.trade_routes * 25 * n;
    v += info.worker_speed_modifier / 2 + info.military_production_modifier / 2;
    v -= info.war_weariness_modifier / 4;
    let broke = crate::economy::spend_pct(g, p) < 50;
    let gold_w = if broke { 40 } else { 15 };
    let gold_w = gold_w.max(crate::finance::gold_value(&crate::finance::state(g, p))); // bankruptcy-guard
    v -= g.player_get_single_civic_upkeep(p, Some(civic), true) * gold_w;
    // City maintenance (the cities' distance and number-of-cities parts) and FfH's AI-only build bans.
    v -= crate::finance::civic_maintenance_x100(g, p, civic) * gold_w / 100 + crate::finance::civic_penalty(g, p, civic); // bankruptcy-guard
    // Happiness from garrisons (FfH God King / BtS Hereditary Rule), worth most to capped cities.
    let capped = pl.cities.iter().filter(|&&c| g.city_happy_level(c) - g.city_unhappy_level(c, 0) <= 0).count() as i32;
    if info.happy_per_military_unit != 0 {
        v += info.happy_per_military_unit * 2 * (40 * capped + 10 * (n - capped));
    }
    v -= info.food_consumption_per_population * pop * 20;
    // Yields on the improvements we work.
    if info.improvement_yield_changes.iter().any(|y| y.iter().any(|&x| x != 0)) {
        for &c in &pl.cities {
            for k in 0..NUM_CITY_PLOTS {
                if k == CITY_HOME_PLOT || !g.cities[c].working_plot[k] {
                    continue;
                }
                let Some(q) = g.city_index_plot(c, k) else { continue };
                if let Some(imp) = g.map.plots[q].improvement_type {
                    let y = info.improvement_yield_changes[imp];
                    v += y[0] * 20 + y[1] * 15 + y[2] * 9;
                }
            }
        }
    }
    for k in 0..CommerceType::NUM {
        v += info.specialist_extra_commerce[k] * pl.cities.iter().map(|&c| g.cities[c].specialist_population).sum::<i32>() * 8;
    }
    v
}

fn civics(g: &mut Game, p: PlayerId) {
    if g.players[p].revolution_timer > 0 || g.player_is_anarchy(p) {
        return;
    }
    // Re-evaluate at most every 10 turns per player (staggered), as revolutions cost anarchy.
    if (g.game_turn() + p.0 as i32) % 10 != 0 && !crate::finance::civics_due(g, p) { // bankruptcy-guard
        return;
    }
    let d = g.d;
    let current = g.players[p].civics.clone();
    let mut new = current.clone();
    let mut gain = 0;
    for opt in d.civic_options.keys() {
        let mut best: Option<(CivicType, i32)> = current[opt].map(|c| (c, civic_value(g, p, c)));
        let cur_value = best.map_or(i32::MIN, |(_, v)| v);
        for c in d.civics.keys() {
            if d.civics[c].civic_option_type != Some(opt) || !g.player_can_do_civics(p, Some(c)) {
                continue;
            }
            let v = civic_value(g, p, c);
            if best.map_or(true, |(_, b)| v > b) {
                best = Some((c, v));
            }
        }
        if let Some((c, v)) = best {
            if Some(c) != current[opt] {
                new[opt] = Some(c);
                gain += v - cur_value.max(0);
            }
        }
    }
    if new != current && gain > 100 && g.player_can_revolution(p, Some(&new)) {
        g.player_revolution(p, &new, false);
    }
}

fn religion(g: &mut Game, p: PlayerId) {
    if !g.player_can_change_religion(p) {
        return;
    }
    let d = g.d;
    let current = g.player_get_state_religion(p);
    let fav = g.players[p].personality_type.and_then(|l| d.leader_heads[l].favorite_religion);
    let mut best: Option<(ReligionType, i32)> = None;
    for r in d.religions.keys() {
        let count = g.players[p].has_religion_count[r];
        if count == 0 || !g.player_can_convert(p, Some(r)) && current != Some(r) {
            continue;
        }
        let mut v = count * 10;
        if fav == Some(r) {
            v += 25;
        }
        if g.player_has_holy_city(p, r) {
            v += 20;
        }
        if current == Some(r) {
            v += 15; // conversion costs anarchy
        }
        if best.map_or(true, |(_, b)| v > b) {
            best = Some((r, v));
        }
    }
    if let Some((r, _)) = best {
        if current != Some(r) && g.player_can_convert(p, Some(r)) {
            g.player_convert(p, Some(r));
        }
    }
}

/// Highest research rate that keeps the treasury from running dry. A cultural victory first
/// sets the culture slider (`victory::culture_slider`), lowered in steps when even no research
/// leaves the treasury short.
fn sliders(g: &mut Game, p: PlayerId) {
    if !g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) {
        return;
    }
    let gold = g.players[p].gold;
    let reserve = 20 + 5 * g.players[p].cities.len() as i32;
    let mut culture = crate::victory::culture_slider(g, p); // victory-pursuit
    loop {
        if g.players[p].commerce_percent[CommerceType::COMMERCE_CULTURE.idx()] != culture {
            g.player_set_commerce_percent(p, CommerceType::COMMERCE_CULTURE, culture);
        }
        let mut pct = 100 - culture;
        let mut paid = false;
        while pct >= 0 {
            if g.players[p].commerce_percent[CommerceType::COMMERCE_RESEARCH.idx()] != pct {
                g.player_set_commerce_percent(p, CommerceType::COMMERCE_RESEARCH, pct);
            }
            let rate = g.player_calculate_gold_rate(p);
            if rate >= 0 || gold + rate * 10 > reserve {
                paid = true;
                break;
            }
            pct -= 10;
        }
        if paid || culture == 0 {
            break;
        }
        culture = (culture - 20).max(0);
    }
}
