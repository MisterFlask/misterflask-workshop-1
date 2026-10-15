//! Victory planner (victory-pursuit): how close every civ is to each FFH2 victory condition,
//! the one this player pursues, and a rival about to win.
//!
//! The conditions are the rules' (`CvGame::testVictory`, `CIV4VictoryInfo.xml`):
//! - **Tower of Mastery**: the team owns one of each building class with a
//!   `VICTORY_TOWER_OF_MASTERY` threshold -- the Towers of Alteration, Divination, Necromancy and
//!   the Elements (800 hammers each, Alteration / Divination / Necromancy / Elementalism) and the
//!   Tower of Mastery itself (4000 hammers, Strength of Will, needs the other four);
//! - **Cultural**: `iNumCultureCities` (3) cities at `CULTURELEVEL_LEGENDARY` (50000 culture at
//!   normal speed);
//! - **Domination**: the team's share of the land at `getAdjustedLandPercent` (78% - 2 per civ
//!   team ever alive, at least 51%) and of the people 25 points above the runner-up's;
//! - **Conquest**: no other civ team (not our vassal) has a city;
//! - **Altar of the Luonnotar**: the final altar (4000 hammers, Omniscience) after six stages
//!   that only Great Prophets build (non-evil civs);
//! - religious (80% of the world's people in a religion whose holy city we hold), score (target
//!   score, 0 = off) and time (max turns) are not pursued.
//!
//! Every estimate is a pure function of the game state (no cache, nothing remembered), cheap:
//! O(cities + techs) per civ. [`plan`] picks, from turn ~200 (normal speed) or when we lead the
//! world clearly, the condition with the fewest estimated turns left that fits in the turns the
//! game has left; and names a rival whose own estimate is short (and shorter than ours).

use std::sync::OnceLock;

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// A victory condition the AI can pursue.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Goal {
    Tower,
    Culture,
    Domination,
    Conquest,
    Altar,
}

/// A condition and the estimated turns until a civ fulfils it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Race {
    pub goal: Goal,
    pub eta: i32,
}

/// What a player pursues, and the rival it must stop.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub ours: Option<Race>,
    pub rival: Option<(PlayerId, Race)>,
}

impl Plan {
    pub fn goal(&self) -> Option<Goal> {
        self.ours.map(|r| r.goal)
    }
}

/// Static data: victory types and the building classes they count.
pub struct Statics {
    pub tower: Option<VictoryType>,
    pub culture: Option<VictoryType>,
    pub domination: Option<VictoryType>,
    pub conquest: Option<VictoryType>,
    pub altar: Option<VictoryType>,
    /// Building classes with a Tower of Mastery threshold, and the threshold.
    pub tower_classes: Vec<(BuildingClassType, i32)>,
    /// The class of the Tower of Mastery itself (it needs the others).
    pub mastery: Option<BuildingClassType>,
    /// The altar stages, first to final (the final one has the victory threshold).
    pub altar_stages: Vec<BuildingClassType>,
    pub legendary: Option<CultureLevelType>,
    pub culture_process: Option<ProcessType>,
    /// Raw mana (`BONUS_MANA`), and the builds that turn it into a mana type (casters build
    /// these nodes: `BUILD_MANA_FIRE` -> `BONUS_MANA_FIRE`, ...).
    pub raw_mana: Option<BonusType>,
    pub mana_builds: Vec<(BuildType, BonusType)>,
}

fn build_statics(d: &GameData) -> Statics {
    let tower = d.ty::<VictoryType>("VICTORY_TOWER_OF_MASTERY");
    let tower_classes: Vec<(BuildingClassType, i32)> = match tower {
        Some(v) => d.building_classes.keys().filter(|&bc| d.building_classes[bc].victory_threshold[v] > 0).map(|bc| (bc, d.building_classes[bc].victory_threshold[v])).collect(),
        None => Vec::new(),
    };
    let altar_stages = [
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR",
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR_ANOINTED",
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR_BLESSED",
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR_CONSECRATED",
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR_DIVINE",
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR_EXALTED",
        "BUILDINGCLASS_ALTAR_OF_THE_LUONNOTAR_FINAL",
    ]
    .iter()
    .filter_map(|n| d.ty::<BuildingClassType>(n))
    .collect();
    Statics {
        tower,
        culture: d.ty::<VictoryType>("VICTORY_CULTURAL"),
        domination: d.ty::<VictoryType>("VICTORY_DOMINATION"),
        conquest: d.ty::<VictoryType>("VICTORY_CONQUEST"),
        altar: d.ty::<VictoryType>("VICTORY_ALTAR_OF_THE_LUONNOTAR"),
        tower_classes,
        mastery: d.ty::<BuildingClassType>("BUILDINGCLASS_TOWER_OF_MASTERY"),
        altar_stages,
        legendary: d.ty::<CultureLevelType>("CULTURELEVEL_LEGENDARY"),
        culture_process: d.ty::<ProcessType>("PROCESS_CULTURE"),
        raw_mana: d.ty::<BonusType>("BONUS_MANA"),
        mana_builds: d
            .builds
            .iter_enumerated()
            .filter_map(|(b, info)| info.improvement.and_then(|i| d.improvements[i].bonus_convert).map(|bonus| (b, bonus)))
            .collect(),
    }
}

pub fn statics(d: &GameData) -> &'static Statics {
    static S: OnceLock<Statics> = OnceLock::new();
    S.get_or_init(|| build_statics(d))
}

/// Estimates beyond this many turns count as out of reach.
const FAR: i32 = 10_000;

/// Game turns left before the time limit (`FAR` without one).
pub fn turns_left(g: &Game) -> i32 {
    if g.max_turns() > 0 {
        (g.max_turns() - g.elapsed_game_turns).max(0)
    } else {
        FAR
    }
}

/// The turn (at this game's length) from which every civ plans for a victory: 200 of 690 at
/// normal speed.
pub fn start_turn(g: &Game) -> i32 {
    let end = if g.max_turns() > 0 { g.max_turns() } else { g.estimate_end_turn };
    (end.max(1) * 200 / 690).max(1)
}

// ----------------------------------------------------------------------------
// Technology paths
// ----------------------------------------------------------------------------

/// Technologies `team` lacks for `targets`, prerequisites included (every AND prerequisite; of
/// the OR prerequisites none when one is known, else the cheapest), deduplicated.
pub fn tech_path(g: &Game, team: TeamId, targets: &[TechType]) -> Vec<TechType> {
    let mut out: Vec<TechType> = Vec::new();
    let mut stack: Vec<TechType> = targets.to_vec();
    while let Some(t) = stack.pop() {
        if g.team_is_has_tech(team, Some(t)) || out.contains(&t) {
            continue;
        }
        out.push(t);
        let info = &g.d.techs[t];
        stack.extend(info.prereq_and_techs.iter().copied());
        if !info.prereq_or_techs.is_empty() && !info.prereq_or_techs.iter().any(|&o| g.team_is_has_tech(team, Some(o)) || out.contains(&o)) {
            if let Some(&o) = info.prereq_or_techs.iter().min_by_key(|&&o| (g.team_get_research_left(team, o), o)) {
                stack.push(o);
            }
        }
    }
    out
}

fn beakers(g: &Game, team: TeamId, techs: &[TechType]) -> i64 {
    techs.iter().map(|&t| g.team_get_research_left(team, t).max(0) as i64).sum()
}

/// Techs a building needs.
fn building_techs(info: &BuildingInfo) -> Vec<TechType> {
    let mut v: Vec<TechType> = info.prereq_and_tech.into_iter().collect();
    v.extend(info.prereq_and_techs.iter().flatten().copied());
    v
}

fn research_rate(g: &Game, p: PlayerId) -> i64 {
    g.player_calculate_research_rate(p, None).max(1) as i64
}

/// Hammers per turn of our best (and our `n` best summed) production cities.
fn hammers_per_turn(g: &Game, p: PlayerId, n: usize) -> (i64, i64) {
    let mut prods: Vec<i64> = g.players[p].cities.iter().map(|&c| g.city_yield_rate(c, YieldType::YIELD_PRODUCTION).max(1) as i64).collect();
    prods.sort_unstable_by(|a, b| b.cmp(a));
    (prods.first().copied().unwrap_or(1), prods.iter().take(n.max(1)).sum::<i64>().max(1))
}

// ----------------------------------------------------------------------------
// Tower of Mastery
// ----------------------------------------------------------------------------

/// Progress toward the Tower of Mastery victory.
#[derive(Clone, Debug, Default)]
pub struct TowerState {
    /// Towers the team still lacks (this civ's buildings), the Tower of Mastery last.
    pub missing: Vec<BuildingType>,
    /// Techs still to research for them (prerequisites included).
    pub techs: Vec<TechType>,
    pub beakers: i64,
    /// Hammers still to put into the lesser towers and into the Tower of Mastery.
    pub lesser_hammers: i64,
    pub mastery_hammers: i64,
    /// Manas the missing lesser towers need that we lack. FfH makes a building's
    /// `PrereqBonuses` AND requirements (`CvCity::canConstruct`): the Tower of the Elements
    /// needs Air, Earth, Fire and Water mana, and so on, 16 manas for the four towers.
    pub mana: Vec<BonusType>,
    /// Raw mana nodes in our cities' radius, to build those manas on.
    pub raw_nodes: Vec<PlotIdx>,
}

/// Raw mana nodes (`BONUS_MANA`) on our plots in our cities' radius, in plot order.
pub fn raw_mana_plots(g: &Game, p: PlayerId) -> Vec<PlotIdx> {
    let Some(raw) = statics(g.d).raw_mana else { return Vec::new() };
    let mut out: Vec<PlotIdx> = Vec::new();
    for &c in &g.players[p].cities {
        for i in 0..ffh_core::defs::NUM_CITY_PLOTS {
            let Some(q) = g.city_index_plot(c, i) else { continue };
            let plot = &g.map.plots[q];
            if plot.owner == Some(p) && plot.bonus_type == Some(raw) && !out.contains(&q) {
                out.push(q);
            }
        }
    }
    out.sort_unstable();
    out
}

/// `None` when the victory is off or this civ cannot build one of the towers.
pub fn tower_state(g: &Game, p: PlayerId) -> Option<TowerState> {
    let d = g.d;
    let s = statics(d);
    let v = s.tower?;
    if !g.is_victory_valid(v) || s.tower_classes.is_empty() {
        return None;
    }
    let team = g.players[p].team;
    let civ = g.player_get_civilization_type(p)?;
    let mut st = TowerState::default();
    let mut needed: Vec<TechType> = Vec::new();
    let mut mastery: Option<BuildingType> = None;
    for &(bc, threshold) in &s.tower_classes {
        if g.teams[team].building_class_count[bc] >= threshold {
            continue;
        }
        let b = d.civ_building(civ, bc)?;
        let info = &d.buildings[b];
        if info.production_cost <= 0 {
            return None;
        }
        needed.extend(building_techs(info));
        if Some(bc) != s.mastery {
            for &bn in info.prereq_or_bonuses.iter().flatten() {
                if !st.mana.contains(&bn) && !g.player_has_bonus(p, bn) {
                    st.mana.push(bn);
                }
            }
        }
        let progress = g.players[p].cities.iter().map(|&c| g.cities[c].building_production[b]).max().unwrap_or(0);
        let left = (g.player_get_production_needed_building(p, b) - progress).max(0) as i64;
        if Some(bc) == s.mastery {
            st.mastery_hammers = left;
            mastery = Some(b);
        } else {
            st.lesser_hammers += left;
            st.missing.push(b);
        }
    }
    st.missing.extend(mastery);
    st.techs = tech_path(g, team, &needed);
    st.beakers = beakers(g, team, &st.techs);
    if !st.mana.is_empty() {
        st.raw_nodes = raw_mana_plots(g, p);
    }
    Some(st)
}

/// Turns a caster takes to reach and build a mana node, roughly.
const MANA_NODE_TURNS: i64 = 15;

/// Turns until `p` could have the Tower of Mastery: research of the missing techs (the lesser
/// towers are built meanwhile, in our best cities), then the Tower of Mastery in the best city.
pub fn tower_eta(g: &Game, p: PlayerId) -> Option<i32> {
    let st = tower_state(g, p)?;
    if st.missing.is_empty() {
        return Some(0);
    }
    // A tower without its manas cannot be built: each mana we lack needs a raw node of ours to
    // convert (the AI does not trade for mana).
    if g.players[p].cities.is_empty() || st.mana.len() > st.raw_nodes.len() {
        return None;
    }
    let (best, top4) = hammers_per_turn(g, p, 4);
    let research = st.beakers / research_rate(g, p);
    let lesser = st.lesser_hammers / top4 + if st.mana.is_empty() { 0 } else { MANA_NODE_TURNS };
    let mastery = st.mastery_hammers / best;
    Some((research.max(lesser) + mastery).min(FAR as i64) as i32)
}

// ----------------------------------------------------------------------------
// Culture
// ----------------------------------------------------------------------------

/// Culture slider steps a cultural victory may run (the lowest that finishes in time).
pub const CULTURE_SLIDERS: [i32; 4] = [40, 60, 80, 100];

/// Per city of `p`: turns until legendary culture at the current culture rate, and with the
/// culture slider at `boost` percent (replacing its current share) plus, when the city could
/// run it, culture from production (`PROCESS_CULTURE`). Sorted by the boosted estimate, then
/// by city id.
pub fn culture_turns(g: &Game, p: PlayerId, boost: i32) -> Vec<(CityId, i32, i32)> {
    let s = statics(g.d);
    let Some(legendary) = s.legendary else { return Vec::new() };
    let target = g.culture_threshold(legendary) as i64 * 100;
    let current_pct = g.players[p].commerce_percent[CommerceType::COMMERCE_CULTURE.idx()];
    let process = s.culture_process.map_or(0, |pr| g.d.processes[pr].production_to_commerce_modifier[CommerceType::COMMERCE_CULTURE.idx()]) as i64;
    let mut out: Vec<(CityId, i32, i32)> = Vec::new();
    for &c in &g.players[p].cities {
        let have = g.cities[c].culture[p] as i64;
        let left = (target - have).max(0);
        let rate = g.city_commerce_rate_times100(c, CommerceType::COMMERCE_CULTURE).max(0) as i64;
        let modifier = g.city_total_commerce_rate_modifier(c, CommerceType::COMMERCE_CULTURE) as i64;
        let extra_pct = (boost - current_pct) as i64;
        let mut extra = g.city_yield_rate(c, YieldType::YIELD_COMMERCE).max(0) as i64 * extra_pct * modifier / 100;
        if boost > 0 && s.culture_process.is_some_and(|pr| g.city_can_maintain(c, pr, false)) && !g.city_is_production_process(c) {
            extra += g.city_yield_rate(c, YieldType::YIELD_PRODUCTION).max(0) as i64 * process * modifier / 100;
        }
        let turns = |r: i64| if left == 0 { 0 } else if r <= 0 { FAR } else { ((left + r - 1) / r).min(FAR as i64) as i32 };
        out.push((c, turns(rate), turns((rate + extra).max(0))));
    }
    out.sort_by_key(|&(c, _, b)| (b, c));
    out
}

/// Cities a cultural victory needs (`None` when it is off).
fn culture_cities_needed(g: &Game) -> Option<usize> {
    let v = statics(g.d).culture?;
    g.is_victory_valid(v).then(|| g.d.victories[v].num_culture_cities.max(1) as usize)
}

/// Turns until the cities a cultural victory needs reach legendary culture with the slider at
/// `boost` (see [`culture_turns`]).
fn culture_eta_at(g: &Game, p: PlayerId, need: usize, boost: i32) -> Option<i32> {
    let turns = culture_turns(g, p, boost);
    (turns.len() >= need).then(|| turns[need - 1].2)
}

/// Turns until `p` has the cities a cultural victory needs at legendary culture, all in: the
/// culture slider at 100% and the culture process in those cities (once the slider can move,
/// i.e. after Drama, whose research is added). `None` when the victory is off or we have too
/// few cities.
pub fn culture_eta(g: &Game, p: PlayerId) -> Option<i32> {
    let need = culture_cities_needed(g)?;
    if g.player_is_commerce_flexible(p, CommerceType::COMMERCE_CULTURE) {
        return culture_eta_at(g, p, need, 100);
    }
    // Before Drama: its research first.
    let team = g.players[p].team;
    let culture = CommerceType::COMMERCE_CULTURE.idx();
    let d = g.d;
    let research = d
        .techs
        .keys()
        .filter(|&t| d.techs[t].commerce_flexible[culture])
        .map(|t| beakers(g, team, &tech_path(g, team, &[t])))
        .min()?
        / research_rate(g, p);
    let eta = culture_eta_at(g, p, need, 0)?.min(culture_eta_at(g, p, need, 100)? + research as i32);
    Some(eta.min(FAR))
}

// ----------------------------------------------------------------------------
// Domination and conquest
// ----------------------------------------------------------------------------

/// Turns a conquest of one city takes us, roughly (gathering, marching, the strike: the siege
/// operations take about one city per war of 20-40 turns).
const TURNS_PER_CITY: i32 = 30;

/// Alive civ teams other than ours and our vassals that hold cities: (team, cities, power).
fn rivals(g: &Game, team: TeamId) -> Vec<(TeamId, i32, i64)> {
    let barb = g.barbarian_team();
    g.civ_team_ids()
        .filter(|&t| t != team && t != barb && g.team_is_alive(t) && !g.teams[t].vassal[team] && g.teams[t].num_cities > 0)
        .map(|t| (t, g.teams[t].num_cities, g.team_get_power(t, true) as i64))
        .collect()
}

/// Domination: when we already hold half the land needed, the cities to take for the rest at
/// [`TURNS_PER_CITY`] each (0 when both shares are met).
pub fn domination_eta(g: &Game, p: PlayerId) -> Option<i32> {
    let v = statics(g.d).domination?;
    if !g.is_victory_valid(v) || g.map.land_plots <= 0 {
        return None;
    }
    let team = g.players[p].team;
    let land = g.team_get_total_land(team, true) as i64;
    let need_land = g.map.land_plots as i64 * g.adjusted_land_percent(v) as i64 / 100;
    let pop = g.team_get_total_population(team, true) as i64;
    let need_pop = g.total_population as i64 * g.adjusted_population_percent(v) as i64 / 100;
    if land >= need_land && pop >= need_pop {
        return Some(0);
    }
    if land * 2 < need_land {
        return None;
    }
    let rivals = rivals(g, team);
    let their_cities: i64 = rivals.iter().map(|r| r.1 as i64).sum();
    let their_land: i64 = rivals.iter().map(|r| g.team_get_total_land(r.0, false) as i64).sum();
    if their_cities == 0 {
        return None;
    }
    let per_city = (their_land / their_cities).max(1);
    let cities = ((need_land - land).max(0) + per_city - 1) / per_city;
    let their_pop: i64 = rivals.iter().map(|r| g.teams[r.0].total_population as i64).sum();
    let pop_per_city = (their_pop / their_cities).max(1);
    let cities = cities.max(((need_pop - pop).max(0) + pop_per_city - 1) / pop_per_city);
    Some((cities.max(1) * TURNS_PER_CITY as i64).min(FAR as i64) as i32)
}

/// Conquest: once our power is at least twice all the rivals' together, their cities at
/// [`TURNS_PER_CITY`] each.
pub fn conquest_eta(g: &Game, p: PlayerId) -> Option<i32> {
    let v = statics(g.d).conquest?;
    if !g.is_victory_valid(v) {
        return None;
    }
    let team = g.players[p].team;
    let rivals = rivals(g, team);
    let ours = g.team_get_power(team, true) as i64;
    let theirs: i64 = rivals.iter().map(|r| r.2).sum();
    if ours < theirs * 2 {
        return None;
    }
    let cities: i64 = rivals.iter().map(|r| r.1 as i64).sum();
    Some((cities * TURNS_PER_CITY as i64).min(FAR as i64) as i32)
}

// ----------------------------------------------------------------------------
// Altar of the Luonnotar
// ----------------------------------------------------------------------------

/// The altar stage the team has reached (index into `altar_stages`, `None` before the first).
pub fn altar_stage(g: &Game, team: TeamId) -> Option<usize> {
    let s = statics(g.d);
    s.altar_stages.iter().rposition(|&bc| g.teams[team].building_class_count[bc] > 0)
}

/// Altar of the Luonnotar: only once the sixth stage (Exalted) stands, the final altar's
/// research and hammers. (The stages before are Great Prophets' work.)
pub fn altar_eta(g: &Game, p: PlayerId) -> Option<i32> {
    let d = g.d;
    let s = statics(d);
    let v = s.altar?;
    if !g.is_victory_valid(v) || s.altar_stages.len() < 2 {
        return None;
    }
    let team = g.players[p].team;
    let stage = altar_stage(g, team)?;
    let last = s.altar_stages.len() - 1;
    if stage == last {
        return Some(0);
    }
    if stage + 1 < last {
        return None;
    }
    let civ = g.player_get_civilization_type(p)?;
    let b = d.civ_building(civ, s.altar_stages[last])?;
    let techs = tech_path(g, team, &building_techs(&d.buildings[b]));
    let (best, _) = hammers_per_turn(g, p, 1);
    let research = beakers(g, team, &techs) / research_rate(g, p);
    let hammers = g.player_get_production_needed_building(p, b).max(0) as i64 / best;
    Some((research + hammers).min(FAR as i64) as i32)
}

// ----------------------------------------------------------------------------
// The plan
// ----------------------------------------------------------------------------

/// Every condition `p` could fulfil, with its estimate (unsorted).
pub fn races(g: &Game, p: PlayerId) -> Vec<Race> {
    let mut out = Vec::new();
    let mut add = |goal: Goal, eta: Option<i32>| {
        if let Some(eta) = eta.filter(|&e| e < FAR) {
            out.push(Race { goal, eta });
        }
    };
    add(Goal::Tower, tower_eta(g, p));
    add(Goal::Culture, culture_eta(g, p));
    add(Goal::Domination, domination_eta(g, p));
    add(Goal::Conquest, conquest_eta(g, p));
    add(Goal::Altar, altar_eta(g, p));
    out
}

/// The condition `p` is nearest to (fewest turns, ties by goal order).
pub fn best_race(g: &Game, p: PlayerId) -> Option<Race> {
    races(g, p).into_iter().min_by_key(|r| (r.eta, r.goal))
}

/// Our team's share of the world (score v2) against the best other team's: (ours, best other),
/// in basis points.
fn shares(g: &Game, team: TeamId) -> (i64, i64) {
    let scores = g.world_scores();
    let barb = g.barbarian_team();
    let mut teams: Vec<(TeamId, i64)> = Vec::new();
    for q in g.civ_player_ids() {
        let pl = &g.players[q];
        if !pl.alive || pl.team == barb {
            continue;
        }
        match teams.iter_mut().find(|(t, _)| *t == pl.team) {
            Some(e) => e.1 += scores[q.idx()] as i64,
            None => teams.push((pl.team, scores[q.idx()] as i64)),
        }
    }
    let ours = teams.iter().find(|(t, _)| *t == team).map_or(0, |e| e.1);
    let other = teams.iter().filter(|(t, _)| *t != team).map(|e| e.1).max().unwrap_or(0);
    (ours, other)
}

/// Is `p` planning for a victory yet: from [`start_turn`], or from half of it when our team
/// holds 1.5x the share of the world of the next team.
pub fn active(g: &Game, p: PlayerId) -> bool {
    let start = start_turn(g);
    let turn = g.game_turn();
    if turn >= start {
        return true;
    }
    if turn < start / 2 {
        return false;
    }
    let (ours, other) = shares(g, g.players[p].team);
    ours * 2 >= other * 3
}

/// A rival is a threat when its estimate is at most this many turns (normal speed: 60).
pub fn threat_horizon(g: &Game) -> i32 {
    (start_turn(g) * 60 / 200).max(10)
}

/// Our part of the plan only (no rival): what production, research and the sliders need. Six
/// times cheaper than [`plan`].
pub fn plan_ours(g: &Game, p: PlayerId) -> Plan {
    let pl = &g.players[p];
    if !pl.alive || g.player_is_barbarian(p) || pl.cities.is_empty() || !active(g, p) {
        return Plan::default();
    }
    let left = turns_left(g);
    let ours = races(g, p).into_iter().filter(|r| r.eta <= left && (r.goal != Goal::Culture || culture_front_runner(g, p, r.eta))).min_by_key(|r| (r.eta, r.goal));
    Plan { ours, rival: None }
}

/// Is no other civ clearly ahead of us (an estimate under 4/5 of ours, 10 turns more) in the
/// race for a cultural victory? Only one civ wins it: the others' culture is research lost.
fn culture_front_runner(g: &Game, p: PlayerId, eta: i32) -> bool {
    let team = g.players[p].team;
    g.civ_player_ids().all(|q| {
        let ql = &g.players[q];
        q == p || !ql.alive || ql.team == team || g.player_is_barbarian(q) || ql.cities.is_empty() || culture_eta(g, q).map_or(true, |e| e * 5 >= (eta - 10) * 4)
    })
}

/// The plan of player `p` now (see the module comment).
pub fn plan(g: &Game, p: PlayerId) -> Plan {
    let pl = &g.players[p];
    if !pl.alive || g.player_is_barbarian(p) || pl.cities.is_empty() || !active(g, p) {
        return Plan::default();
    }
    let left = turns_left(g);
    let ours = plan_ours(g, p).ours;
    let team = pl.team;
    let horizon = threat_horizon(g);
    let mut rival: Option<(PlayerId, Race)> = None;
    for q in g.civ_player_ids() {
        let ql = &g.players[q];
        if q == p || !ql.alive || ql.team == team || g.player_is_barbarian(q) || ql.cities.is_empty() || g.teams[ql.team].vassal[team] {
            continue;
        }
        let Some(r) = best_race(g, q) else { continue };
        if r.eta > horizon || r.eta > left || ours.is_some_and(|o| o.eta <= r.eta) {
            continue;
        }
        if rival.map_or(true, |(rq, rr)| (r.eta, q) < (rr.eta, rq)) {
            rival = Some((q, r));
        }
    }
    Plan { ours, rival }
}

/// One line for traces.
pub fn describe(g: &Game, p: PlayerId, plan: &Plan) -> String {
    let ours = plan.ours.map_or("none".to_string(), |r| format!("{:?} in {}", r.goal, r.eta));
    let rival = plan.rival.map_or(String::new(), |(q, r)| format!(", rival p{} {:?} in {}", q.0, r.goal, r.eta));
    let detail = match (plan.goal(), tower_state(g, p)) {
        (Some(Goal::Tower), Some(st)) => format!(
            "; towers missing {}, techs {:?}, {} beakers, hammers {}+{}, without mana {}, raw nodes {}, mana jobs {}",
            st.missing.len(),
            st.techs.iter().map(|&t| g.d.techs[t].base.type_.as_str()).collect::<Vec<_>>(),
            st.beakers,
            st.lesser_hammers,
            st.mastery_hammers,
            st.mana.len(),
            st.raw_nodes.len(),
            mana_jobs(g, p).len()
        ),
        _ => String::new(),
    };
    format!("victory plan: {ours}{rival} ({} turns left){detail}", turns_left(g))
}

// ----------------------------------------------------------------------------
// Pursuit: research, production, sliders
// ----------------------------------------------------------------------------

/// Technologies our goal still needs (research gives them a bonus): the towers' techs; Drama
/// (the culture slider) for a cultural victory; the final altar's techs.
pub fn goal_techs(g: &Game, p: PlayerId, plan: &Plan) -> Vec<TechType> {
    let d = g.d;
    let team = g.players[p].team;
    match plan.goal() {
        Some(Goal::Tower) => tower_state(g, p).map_or(Vec::new(), |s| s.techs),
        Some(Goal::Culture) => {
            let culture = CommerceType::COMMERCE_CULTURE.idx();
            if g.player_is_commerce_flexible(p, CommerceType::COMMERCE_CULTURE) {
                return Vec::new();
            }
            let flex: Vec<TechType> = d.techs.keys().filter(|&t| d.techs[t].commerce_flexible[culture]).collect();
            flex.iter().map(|&t| tech_path(g, team, &[t])).min_by_key(|path| (beakers(g, team, path), path.len())).unwrap_or_default()
        }
        Some(Goal::Altar) => {
            let s = statics(d);
            let Some(civ) = g.player_get_civilization_type(p) else { return Vec::new() };
            match s.altar_stages.last().and_then(|&bc| d.civ_building(civ, bc)) {
                Some(b) => tech_path(g, team, &building_techs(&d.buildings[b])),
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// Our cities by production, best first (ties to the older city).
fn ranked_by_production(g: &Game, p: PlayerId) -> Vec<CityId> {
    let mut v: Vec<(CityId, i32)> = g.players[p].cities.iter().map(|&c| (c, g.city_yield_rate(c, YieldType::YIELD_PRODUCTION))).collect();
    v.sort_by_key(|&(c, prod)| (std::cmp::Reverse(prod), c));
    v.into_iter().map(|(c, _)| c).collect()
}

/// Culture (x100) a building adds to city `c` per turn: flat culture, and its culture modifier
/// on what the city makes now.
fn building_culture(g: &Game, c: CityId, b: BuildingType) -> i32 {
    let info = &g.d.buildings[b];
    let i = CommerceType::COMMERCE_CULTURE.idx();
    let rate = g.city_commerce_rate(c, CommerceType::COMMERCE_CULTURE).max(0);
    (info.commerce_change[i] + info.obsolete_safe_commerce_change[i]) * 100 + info.commerce_modifier[i] * rate
}

/// A goal this close (turns) comes before expansion and the army in production.
pub const URGENT: i32 = 30;

/// What city `c` builds for our goal, if anything (asked by production after the garrison, the
/// first army and the workers; with `urgent_only`, before expansion, and only when the goal is
/// [`URGENT`]): the towers in our best cities (the Tower of Mastery in the best one; the rules
/// let only one city build each); for a cultural victory, in the cities nearest legendary,
/// culture buildings and wonders, else culture from production (`PROCESS_CULTURE`); the final
/// altar where the Exalted altar stands.
pub fn production(g: &Game, c: CityId, urgent_only: bool) -> Option<crate::production::Choice> {
    use crate::production::Choice;
    let d = g.d;
    let p = g.cities[c].owner;
    let plan = plan_ours(g, p);
    if urgent_only && plan.ours.map_or(true, |r| r.eta > URGENT) {
        return None;
    }
    match plan.goal()? {
        Goal::Tower => {
            let st = tower_state(g, p)?;
            // A caster to build the mana nodes the towers need, when we have none.
            let casters = crate::production::count_ai(g, p, UnitAIType::UNITAI_MAGE) + crate::production::count_ai(g, p, UnitAIType::UNITAI_WARWIZARD);
            if casters == 0 && !mana_jobs(g, p).is_empty() {
                if let Some(u) = crate::production::best_unit(g, c, crate::production::Role::Mage) {
                    return Some(Choice::Train(u, UnitAIType::UNITAI_MAGE));
                }
            }
            let ranked = ranked_by_production(g, p);
            let rank = ranked.iter().position(|&o| o == c)?;
            // The last entry is the Tower of Mastery: the best city (it needs the other four,
            // so the rules refuse it until they stand).
            let mastery = statics(d).mastery;
            for &b in st.missing.iter().rev() {
                let is_mastery = d.buildings[b].building_class_type == mastery;
                let slots = if is_mastery { 1 } else { st.missing.len() };
                if rank < slots && g.city_can_construct(c, b, false, false, false) {
                    return Some(Choice::Construct(b));
                }
            }
            crate::trace!(g, p, "{}: no tower to build (rank {} of {})", g.cities[c].name, rank, ranked.len());
            None
        }
        Goal::Culture => {
            let v = statics(d).culture?;
            let need = d.victories[v].num_culture_cities.max(1) as usize;
            let top = culture_turns(g, p, 100);
            if !top.iter().take(need).any(|&(o, _, _)| o == c) {
                return None;
            }
            let mut best: Option<(BuildingType, i64)> = None;
            for b in d.buildings.keys() {
                let gain = building_culture(g, c, b);
                if gain <= 0 || !g.city_can_construct(c, b, false, false, false) {
                    continue;
                }
                let turns = g.city_production_turns_left_building(c, b, 0).max(1);
                if turns > 40 {
                    continue;
                }
                let score = gain as i64 * 100 / (turns as i64 + 3);
                if best.map_or(true, |(_, s)| score > s) {
                    best = Some((b, score));
                }
            }
            if let Some((b, _)) = best {
                return Some(Choice::Construct(b));
            }
            statics(d).culture_process.filter(|&pr| g.city_can_maintain(c, pr, false)).map(Choice::Maintain)
        }
        Goal::Altar => {
            let s = statics(d);
            let civ = g.player_get_civilization_type(p)?;
            let b = d.civ_building(civ, *s.altar_stages.last()?)?;
            g.city_can_construct(c, b, false, false, false).then_some(Choice::Construct(b))
        }
        Goal::Domination | Goal::Conquest => None,
    }
}

/// A cultural victory aims to be won within this many turns (normal speed), or before the time
/// limit if sooner.
pub fn culture_horizon(g: &Game) -> i32 {
    (start_turn(g) / 2).max(10)
}

/// The culture slider our goal wants: 0 unless a cultural victory (and the slider can move);
/// then the lowest of [`CULTURE_SLIDERS`] that brings the cities there within
/// [`culture_horizon`] turns (and 10 turns before the time limit), else all of it.
pub fn culture_slider(g: &Game, p: PlayerId) -> i32 {
    if plan_ours(g, p).goal() != Some(Goal::Culture) || !g.player_is_commerce_flexible(p, CommerceType::COMMERCE_CULTURE) {
        return 0;
    }
    let Some(need) = culture_cities_needed(g) else { return 0 };
    let left = (turns_left(g) - 10).min(culture_horizon(g));
    for pct in CULTURE_SLIDERS {
        if culture_eta_at(g, p, need, pct).is_some_and(|e| e <= left) {
            return pct;
        }
    }
    100
}

/// Extra defenders city `c` keeps for our victory: once our goal is near enough for the other
/// civs to come for it (within [`threat_horizon`] plus 30 turns), two in the cities that carry
/// it -- those nearest legendary culture, those with or building a tower.
pub fn guard(g: &Game, c: CityId) -> i32 {
    let p = g.cities[c].owner;
    let Some(race) = plan_ours(g, p).ours else { return 0 };
    if race.eta > threat_horizon(g) + 30 {
        return 0;
    }
    let d = g.d;
    let carries = match race.goal {
        Goal::Culture => {
            let need = culture_cities_needed(g).unwrap_or(3);
            culture_turns(g, p, 100).iter().take(need).any(|&(o, _, _)| o == c)
        }
        Goal::Tower => {
            let civ = g.city_civilization_type(c);
            statics(d).tower_classes.iter().any(|&(bc, _)| {
                d.civ_building(civ, bc).is_some_and(|b| g.cities[c].num_real_building[b] > 0 || g.city_production_building(c) == Some(b))
            })
        }
        _ => false,
    };
    if carries {
        2
    } else {
        0
    }
}

/// A great person with a great work (the Great Bard: 4000 culture) pursuing a cultural victory
/// performs it in the city among those nearest legendary that is furthest from it. Returns
/// whether it got orders.
pub fn great_work(g: &mut Game, v: &mut crate::view::View, grp: GroupId, u: UnitId) -> bool {
    use crate::units::{alive, move_to, push, skip};
    let p = v.p;
    if g.d.units[g.units[u].unit_type].great_work_culture <= 0 || plan_ours(g, p).goal() != Some(Goal::Culture) {
        return false;
    }
    let Some(need) = culture_cities_needed(g) else { return false };
    let top = culture_turns(g, p, 100);
    let Some(&(c, _, _)) = top.iter().take(need).filter(|&&(_, _, t)| t > 0).max_by_key(|&&(c, _, t)| (t, std::cmp::Reverse(c))) else {
        return false;
    };
    let tp = g.city_plot(c);
    crate::trace!(g, p, "great work in {} for a cultural victory", g.cities[c].name);
    if g.unit_plot(u) != tp {
        move_to(g, grp, tp, ffh_core::defs::MOVE_SAFE_TERRITORY);
        if !alive(g, grp) {
            return true;
        }
        push(g, grp, MissionType::MISSION_GREAT_WORK, -1, -1, 0, true);
    } else if g.unit_can_great_work(u, tp) {
        push(g, grp, MissionType::MISSION_GREAT_WORK, -1, -1, 0, false);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, false) {
        skip(g, grp);
    }
    true
}

// ----------------------------------------------------------------------------
// War: our domination or conquest, a rival about to win
// ----------------------------------------------------------------------------

/// Does our goal call for wars of conquest (domination, conquest)?
pub fn wants_conquest(plan: &Plan) -> bool {
    matches!(plan.goal(), Some(Goal::Domination) | Some(Goal::Conquest))
}

/// The rival team our plan says to stop, if any.
pub fn rival_team(g: &Game, plan: &Plan) -> Option<TeamId> {
    plan.rival.map(|(q, _)| g.players[q].team)
}

/// Extra campaign value of city `c` for us: a rival's cities that carry its victory (the
/// culture cities nearest legendary, the cities with its towers).
pub fn rival_city_bonus(g: &Game, plan: &Plan, c: CityId) -> i32 {
    let Some((q, race)) = plan.rival else { return 0 };
    if g.cities[c].owner != q {
        return 0;
    }
    let d = g.d;
    match race.goal {
        Goal::Culture => {
            let need = culture_cities_needed(g).unwrap_or(3);
            if culture_turns(g, q, 100).iter().take(need).any(|&(o, _, _)| o == c) {
                3000
            } else {
                0
            }
        }
        Goal::Tower => {
            let towers = statics(d).tower_classes.iter().filter(|&&(bc, _)| d.civ_building(g.city_civilization_type(c), bc).is_some_and(|b| g.cities[c].num_real_building[b] > 0)).count() as i32;
            3000 * towers.min(1) + 500 * towers
        }
        Goal::Altar => {
            let stages = statics(d).altar_stages.iter().filter(|&&bc| d.civ_building(g.city_civilization_type(c), bc).is_some_and(|b| g.cities[c].num_real_building[b] > 0)).count() as i32;
            3000 * stages.min(1)
        }
        // Their land and people: every city counts, the big ones most (city_value already).
        Goal::Domination | Goal::Conquest => 500,
    }
}

// ----------------------------------------------------------------------------
// Mana nodes for the towers
// ----------------------------------------------------------------------------

/// The mana nodes the Tower of Mastery still needs: per mana the towers lack, a raw node of
/// ours and the build (whose tech we have) that turns it into that mana. Only while the raw
/// nodes cover every mana we lack (a partial set builds no tower).
pub fn mana_jobs(g: &Game, p: PlayerId) -> Vec<(PlotIdx, BuildType)> {
    let d = g.d;
    let Some(st) = tower_state(g, p) else { return Vec::new() };
    if st.mana.len() > st.raw_nodes.len() {
        return Vec::new();
    }
    let team = g.players[p].team;
    let mut out: Vec<(PlotIdx, BuildType)> = Vec::new();
    let mut nodes = st.raw_nodes.clone();
    for &mana in &st.mana {
        let build = statics(d).mana_builds.iter().find(|&&(b, bonus)| bonus == mana && d.builds[b].tech_prereq.map_or(true, |t| g.team_is_has_tech(team, Some(t))));
        let Some(&(b, _)) = build else { continue };
        if nodes.is_empty() {
            break;
        }
        out.push((nodes.remove(0), b));
    }
    out
}

/// A caster of a civ pursuing the Tower of Mastery builds the mana node a tower needs (the
/// nearest one it can reach within 12 turns and build). Returns whether it got orders.
pub fn mana_node(g: &mut Game, v: &mut crate::view::View, grp: GroupId, u: UnitId) -> bool {
    use crate::units::{alive, move_to, push, skip};
    let p = v.p;
    if plan_ours(g, p).goal() != Some(Goal::Tower) {
        return false;
    }
    let jobs: Vec<(PlotIdx, BuildType)> = mana_jobs(g, p).into_iter().filter(|&(q, b)| !v.is_claimed(q) && g.unit_can_build(u, q, b, false)).collect();
    if jobs.is_empty() {
        return false;
    }
    let reach = v.flood.run(g, u, 12, false);
    let Some((q, b, _)) = jobs.iter().filter_map(|&(q, b)| reach.iter().find(|r| r.plot == q).map(|r| (q, b, r.turns))).min_by_key(|&(q, _, t)| (t, q)) else {
        return false;
    };
    v.claimed.push(q);
    crate::trace!(g, p, "caster {:?}: {} at {:?} for the Tower of Mastery", u, g.d.builds[b].base.type_, g.map.xy(q));
    if q != g.unit_plot(u) {
        move_to(g, grp, q, ffh_core::defs::MOVE_SAFE_TERRITORY);
        if !alive(g, grp) {
            return true;
        }
        push(g, grp, MissionType::MISSION_BUILD, b.i(), -1, 0, true);
    } else {
        push(g, grp, MissionType::MISSION_BUILD, b.i(), -1, 0, false);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, false) {
        skip(g, grp);
    }
    true
}
