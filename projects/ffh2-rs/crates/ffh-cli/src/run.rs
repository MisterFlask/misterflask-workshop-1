//! Headless game driver: builds a game, attaches AI controllers and drives `CvGame::update`
//! the way the Civ4 exe does (one call per frame), without UI.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ffh_core::controller::{Controller, ControllerRef};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::mapscript::FractalMapScript;

use ffh_core::structs::*;
use ffh_data::*;

/// Wraps a controller and accumulates the wall time spent inside it. That includes the rules
/// code executing the orders it issues (moves, combat), so it is an upper bound on AI cost.
/// Wall-clock time never feeds back into the game.
pub struct Timed {
    inner: ControllerRef,
    pub nanos: AtomicU64,
}

impl Timed {
    pub fn new(inner: ControllerRef) -> Timed {
        Timed { inner, nanos: AtomicU64::new(0) }
    }

    fn time<R>(&self, f: impl FnOnce() -> R) -> R {
        let t = Instant::now();
        let r = f();
        self.nanos.fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
        r
    }

    pub fn total(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::Relaxed))
    }
}

impl Controller for Timed {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn play_turn(&self, g: &mut Game, p: PlayerId) {
        self.time(|| self.inner.play_turn(g, p))
    }
    fn choose_production(&self, g: &mut Game, c: CityId) {
        self.time(|| self.inner.choose_production(g, c))
    }
    fn choose_research(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        self.time(|| self.inner.choose_research(g, p))
    }
    fn choose_free_tech(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        self.time(|| self.inner.choose_free_tech(g, p))
    }
    fn raze_conquered_city(&self, g: &Game, p: PlayerId, c: CityId) -> bool {
        self.time(|| self.inner.raze_conquered_city(g, p, c))
    }
    fn choose_event(&self, g: &Game, p: PlayerId, t: &EventTriggeredData, options: &[EventType]) -> Option<EventType> {
        self.time(|| self.inner.choose_event(g, p, t, options))
    }
    fn choose_election(&self, g: &Game, p: PlayerId, vs: &VoteSelectionData) -> Option<usize> {
        self.time(|| self.inner.choose_election(g, p, vs))
    }
    fn diplo_vote(&self, g: &Game, p: PlayerId, vote: &VoteSelectionSubData, source: VoteSourceType) -> PlayerVote {
        self.time(|| self.inner.diplo_vote(g, p, vote, source))
    }
    fn keep_captured_unit(&self, g: &Game, p: PlayerId, unit: UnitType, plot: PlotIdx) -> bool {
        self.time(|| self.inner.keep_captured_unit(g, p, unit, plot))
    }
    fn accept_unit(&self, g: &Game, p: PlayerId, unit: UnitId) -> bool {
        self.time(|| self.inner.accept_unit(g, p, unit))
    }
    fn consider_offer(&self, g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData]) -> bool {
        self.time(|| self.inner.consider_offer(g, p, from, they_give, we_give))
    }
    fn choose_religion(&self, g: &Game, p: PlayerId) -> Option<ReligionType> {
        self.time(|| self.inner.choose_religion(g, p))
    }
    fn attitude(&self, g: &Game, p: PlayerId, towards: PlayerId) -> AttitudeType {
        self.time(|| self.inner.attitude(g, p, towards))
    }
    fn update_automated_group(&self, g: &mut Game, grp: GroupId) -> bool {
        self.time(|| self.inner.update_automated_group(g, grp))
    }
}

pub struct RunConfig {
    pub seed: u32,
    pub civs: usize,
    pub world: String,
    pub turns: i32,
    pub verbose: bool,
    /// Print `report` every this many turns (0 = never).
    pub report_every: i32,
    /// Game options (`GAMEOPTION_*` type names) to turn on.
    pub options: Vec<String>,
    /// Stop once the game is decided (see [`SupremacyRule`]); `turns` stays the cap.
    pub until_supremacy: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig { seed: 1, civs: 4, world: "WORLDSIZE_SMALL".into(), turns: 100, verbose: false, report_every: 0, options: Vec::new(), until_supremacy: false }
    }
}

/// Per-turn record for reports and determinism checks.
pub struct TurnStat {
    pub turn: i32,
    pub elapsed: Duration,
    pub checksum: i32,
    pub units: usize,
    pub cities: usize,
}

/// A new game with the AI on every slot; the returned `Timed` measures AI time.
pub fn new_game(cfg: &RunConfig) -> (Game, Arc<Timed>) {
    let d = ffh_data::global();
    let mut o = ffh_core::setup::SetupOptions::new(d, cfg.civs, &cfg.world, cfg.seed);
    o.options = cfg.options.iter().map(|name| d.t::<GameOptionType>(name)).collect();
    let mut g = Game::new_with_options(d, &o);
    g.map_script = Arc::new(FractalMapScript::default());
    let timed = Arc::new(Timed::new(ffh_ai::ai_controller()));
    let ai: ControllerRef = timed.clone();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
    g.autorun = true;
    // FFH_LOG=1 (all players) or FFH_LOG=<player id>: print the game log as it is written.
    if std::env::var("FFH_LOG").is_ok() {
        g.log.enabled = true;
    }
    g.game_init();
    (g, timed)
}

/// Runs `game.update()` until `cfg.turns` turns have been played or the game ends.
/// Returns one record per completed game turn.
pub fn autoplay(g: &mut Game, cfg: &RunConfig) -> Vec<TurnStat> {
    autoplay_with(g, cfg, |_, _| {})
}

/// [`autoplay`], calling `on_turn` after every completed game turn.
pub fn autoplay_with(g: &mut Game, cfg: &RunConfig, on_turn: impl FnMut(&Game, &TurnStat)) -> Vec<TurnStat> {
    play(g, cfg, on_turn).stats
}

/// When a game counts as decided for testing purposes ("clear supremacy"), on team scores (the
/// team's share of the world, score v2: docs/SCORE.md).
#[derive(Clone, Copy, Debug)]
pub struct SupremacyRule {
    /// Leader's score as a percentage of the runner-up's (200 = twice).
    pub ratio_pct: i64,
    /// Consecutive turns the leader must keep that margin.
    pub hold_turns: i32,
    /// Margins before this game turn don't count (early leads are small and still reverse).
    pub min_turn: i32,
}

/// Default: 1.5x the runner-up's score (share of the world) held 30 turns, counted from turn
/// 200: on 36 full games it fired in 15, median turn 440, and named the full-game winner in 13.
/// (Calibrated at normal speed, 4-9 civs; docs/SCORE.md has the alternatives: from turn 150 it
/// is right in 12 of 15, 1.4x held 50 turns in 15 of 18, 2x almost never fires.)
impl Default for SupremacyRule {
    fn default() -> Self {
        SupremacyRule { ratio_pct: 150, hold_turns: 30, min_turn: 200 }
    }
}

/// How an autoplay run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The turn cap was reached with no decision.
    TurnLimit,
    /// The rules ended the game (`winner` is None for a time end without a single winner).
    Victory { turn: i32, winner: Option<TeamId>, victory: Option<VictoryType> },
    /// Only one civ team (not counting barbarians) is left alive.
    LastStanding { turn: i32, team: TeamId },
    /// One team kept `ratio_pct` of the runner-up's score for `hold_turns` turns.
    Supremacy { turn: i32, team: TeamId, since: i32 },
}

/// Result of [`play`].
pub struct Played {
    pub stats: Vec<TurnStat>,
    pub outcome: Outcome,
}

/// Score of each alive civ team (sum of its alive members' `world_scores`, basis points of the
/// world; the teams sum to 10000), barbarians excluded, by team id.
pub fn team_scores(g: &Game) -> Vec<(TeamId, i64)> {
    let barb = g.barbarian_team();
    let scores = g.world_scores();
    let mut out: Vec<(TeamId, i64)> = Vec::new();
    for p in g.civ_player_ids() {
        let pl = &g.players[p];
        if !pl.alive || pl.team == barb {
            continue;
        }
        let score = scores[p.idx()] as i64;
        match out.iter_mut().find(|(t, _)| *t == pl.team) {
            Some(e) => e.1 += score,
            None => out.push((pl.team, score)),
        }
    }
    out.sort_by_key(|&(t, _)| t);
    out
}

/// Tracks [`SupremacyRule`] turn by turn.
#[derive(Default)]
pub struct SupremacyTracker {
    rule: SupremacyRule,
    leader: Option<TeamId>,
    since: i32,
}

impl SupremacyTracker {
    pub fn new(rule: SupremacyRule) -> Self {
        SupremacyTracker { rule, leader: None, since: 0 }
    }

    /// Call after each completed game turn; returns the outcome once the game is decided.
    pub fn observe(&mut self, g: &Game) -> Option<Outcome> {
        let turn = g.game_turn();
        let scores = team_scores(g);
        if scores.len() == 1 {
            return Some(Outcome::LastStanding { turn, team: scores[0].0 });
        }
        let mut sorted = scores.clone();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let (Some(&(lead, top)), Some(&(_, second))) = (sorted.first(), sorted.get(1)) else { return None };
        if turn >= self.rule.min_turn && top * 100 >= second.max(1) * self.rule.ratio_pct {
            if self.leader != Some(lead) {
                self.leader = Some(lead);
                self.since = turn;
            }
            if turn - self.since >= self.rule.hold_turns {
                return Some(Outcome::Supremacy { turn, team: lead, since: self.since });
            }
        } else {
            self.leader = None;
        }
        None
    }
}

/// Plays until `cfg.turns` turns, the end of the game, or (with `cfg.until_supremacy`) clear
/// supremacy, calling `on_turn` after every completed game turn.
pub fn play(g: &mut Game, cfg: &RunConfig, mut on_turn: impl FnMut(&Game, &TurnStat)) -> Played {
    let mut tracker = SupremacyTracker::new(SupremacyRule::default());
    let mut outcome = Outcome::TurnLimit;
    let mut stats = Vec::new();
    let end_turn = g.game_turn() + cfg.turns;
    let mut turn = g.game_turn();
    let mut t0 = Instant::now();
    // Safety net against a stuck turn (a player that never finishes): C++ would hang too.
    let mut updates_this_turn = 0u64;
    while g.game_state == GameStateType::GAMESTATE_ON && g.game_turn() < end_turn {
        g.update();
        updates_this_turn += 1;
        if g.log.enabled && !g.log.entries.is_empty() {
            let filter: Option<u8> = std::env::var("FFH_LOG").ok().and_then(|v| v.parse().ok());
            for e in g.log.entries.drain(..) {
                if filter.map_or(true, |f| e.player == Some(f)) {
                    eprintln!("LOG t{} p{:?}: {}", e.turn, e.player, e.text);
                }
            }
        }
        if g.game_turn() != turn {
            let stat = TurnStat { turn, elapsed: t0.elapsed(), checksum: g.calculate_sync_checksum(), units: g.units.len(), cities: g.cities.len() };
            if cfg.verbose {
                println!(
                    "turn {:4}  {:8.2} ms  units {:5}  cities {:4}  checksum {:08x}",
                    stat.turn,
                    stat.elapsed.as_secs_f64() * 1000.0,
                    stat.units,
                    stat.cities,
                    stat.checksum as u32
                );
            }
            on_turn(g, &stat);
            stats.push(stat);
            if cfg.report_every > 0 && (turn + 1) % cfg.report_every == 0 {
                print!("{}", report(g));
            }
            turn = g.game_turn();
            t0 = Instant::now();
            updates_this_turn = 0;
            if cfg.until_supremacy && g.game_state == GameStateType::GAMESTATE_ON {
                if let Some(o) = tracker.observe(g) {
                    outcome = o;
                    break;
                }
            }
        } else if updates_this_turn > 100_000 {
            for p in g.player_ids() {
                if !g.players[p].alive || !g.players[p].turn_active {
                    continue;
                }
                for &grp in &g.players[p].groups {
                    if g.group_ready_to_move(grp, true) || g.group_is_busy(grp) {
                        let head = g.group_get_head_unit(grp);
                        eprintln!(
                            "stuck: player {} group {:?} units {} head {:?} ({}) ai {:?} missions {:?} activity {:?} busy {} moves {:?}",
                            p.0,
                            grp,
                            g.groups[grp].units.len(),
                            head,
                            head.map_or("-".into(), |h| g.d.units[g.units[h].unit_type].base.type_.clone()),
                            head.and_then(|h| g.units[h].ai_type),
                            g.groups[grp].mission_queue.iter().map(|m| m.mission_type).collect::<Vec<_>>(),
                            g.groups[grp].activity_type,
                            g.group_is_busy(grp),
                            head.map(|h| g.unit_moves_left(h))
                        );
                    }
                }
            }
            panic!("turn {turn} did not finish after {updates_this_turn} updates (a player never ends its turn)");
        }
    }
    if g.game_state != GameStateType::GAMESTATE_ON {
        outcome = Outcome::Victory { turn: g.game_turn(), winner: g.winner, victory: g.victory };
    }
    Played { stats, outcome }
}

/// One line describing `outcome` (civ names of the deciding team's members).
pub fn describe_outcome(g: &Game, outcome: &Outcome) -> String {
    let team_name = |t: TeamId| {
        let names: Vec<String> = g
            .player_ids()
            .into_iter()
            .filter(|&p| g.players[p].team == t && g.players[p].ever_alive)
            .map(|p| g.player_get_civilization_type(p).map_or("?".to_string(), |c| g.d.civilizations[c].base.type_.trim_start_matches("CIVILIZATION_").to_string()))
            .collect();
        format!("team {} ({})", t.0, names.join("+"))
    };
    match outcome {
        Outcome::TurnLimit => "no decision before the turn limit".to_string(),
        Outcome::Victory { turn, winner, victory } => format!(
            "game over at turn {turn}: {} by {}",
            winner.map_or("no single winner".to_string(), team_name),
            victory.map_or("time".to_string(), |v| g.d.victories[v].base.type_.clone())
        ),
        Outcome::LastStanding { turn, team } => format!("last civ standing at turn {turn}: {}", team_name(*team)),
        Outcome::Supremacy { turn, team, since } => {
            format!("clear supremacy at turn {turn}: {} has held 1.5x the runner-up's share of the world since turn {since}", team_name(*team))
        }
    }
}

/// Per alive player: research, gold, techs, units by AI role, and each city's size and
/// production (a diagnostic for AI work).
pub fn report(g: &Game) -> String {
    let d = g.d;
    let mut out = format!("== report at turn {} ==\n", g.game_turn());
    for p in g.player_ids() {
        let pl = &g.players[p];
        if !pl.alive {
            continue;
        }
        let civ = g.player_get_civilization_type(p).map_or("?".to_string(), |c| d.civilizations[c].base.type_.clone());
        let techs = g.teams[pl.team].has_tech.iter().filter(|&&b| b).count();
        let research = g.player_get_current_research(p).map_or("-".to_string(), |t| d.techs[t].base.type_.clone());
        out += &format!(
            "  {:2} {} techs={} research={} (+{}/turn) gold={} rate={:+} sliders={:?} cities={} pop={} units={}\n",
            p.0,
            civ,
            techs,
            research,
            g.player_calculate_research_rate(p, None),
            pl.gold,
            g.player_calculate_gold_rate(p),
            pl.commerce_percent,
            pl.cities.len(),
            pl.total_population,
            pl.units.len()
        );
        out += &format!(
            "     costs: units {} supply {} maintenance {} civics {} inflation {}% -> {}; commerce gold {} research {}\n",
            g.player_calculate_unit_cost(p),
            g.player_calculate_unit_supply(p),
            g.player_get_total_maintenance(p),
            g.player_get_civic_upkeep(p, None, false),
            g.player_calculate_inflation_rate(p),
            g.player_calculate_inflated_costs(p),
            g.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD),
            g.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH)
        );
        let mut roles: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for &u in &pl.units {
            let role = g.units[u].ai_type.map_or("NONE".to_string(), |a| unit_ai_name(a).to_string());
            *roles.entry(role).or_default() += 1;
        }
        out += "     units:";
        for (r, n) in &roles {
            out += &format!(" {r}={n}");
        }
        out += "\n";
        // Improvements on owned plots (worked ones counted separately) and civics.
        let mut imps: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
        for (q, plot) in g.map.plots.iter().enumerate() {
            if plot.owner != Some(p) {
                continue;
            }
            if let Some(i) = plot.improvement_type {
                let e = imps.entry(d.improvements[i].base.type_.trim_start_matches("IMPROVEMENT_").to_string()).or_default();
                e.0 += 1;
                if g.plot_is_being_worked(ffh_core::ids::PlotIdx::from_usize(q)) {
                    e.1 += 1;
                }
            }
        }
        out += "     improvements (worked):";
        for (name, (n, w)) in &imps {
            out += &format!(" {name}={n}({w})");
        }
        out += "\n     civics:";
        for o in d.civic_options.keys() {
            if let Some(c) = pl.civics[o] {
                out += &format!(" {}", d.civics[c].base.type_.trim_start_matches("CIVIC_"));
            }
        }
        out += "\n";
        for &c in &pl.cities {
            let city = &g.cities[c];
            let prod = if let Some(u) = g.city_production_unit(c) {
                d.units[u].base.type_.clone()
            } else if let Some(b) = g.city_production_building(c) {
                d.buildings[b].base.type_.clone()
            } else {
                city.order_queue.first().map_or("-".to_string(), |o| format!("{:?}", o.order_type))
            };
            let happy = g.city_happy_level(c) - g.city_unhappy_level(c, 0);
            let health = g.city_good_health(c) - g.city_bad_health(c, false, 0);
            out += &format!(
                "     city {} size={} food={:+} ({}/{}) yields={:?} commerce={:?} trade={}x{} happy={:+} health={:+} prod={} ({}/{})\n",
                city.name,
                city.population,
                g.city_food_difference(c, true),
                city.food,
                g.city_growth_threshold(c),
                [0, 1, 2].map(|y| g.city_yield_rate(c, YieldType::from_usize(y))),
                [0, 1, 2, 3].map(|k| g.city_commerce_rate(c, CommerceType::from_usize(k))),
                city.trade_cities.iter().filter(|t| t.is_some()).count(),
                city.trade_yield[2],
                happy,
                health,
                prod,
                if city.order_queue.is_empty() { 0 } else { g.city_production(c) },
                if city.order_queue.is_empty() { 0 } else { g.city_production_needed(c) }
            );
        }
    }
    out
}

fn unit_ai_name(a: UnitAIType) -> &'static str {
    const NAMES: [&str; 13] = [
        "UNKNOWN", "ANIMAL", "SETTLE", "WORKER", "ATTACK", "ATTACK_CITY", "COLLATERAL", "PILLAGE", "RESERVE", "COUNTER",
        "CITY_DEFENSE", "CITY_COUNTER", "CITY_SPECIAL",
    ];
    NAMES.get(a.i() as usize).copied().unwrap_or("OTHER")
}

/// A score in basis points as a percentage with one decimal (2537 -> "25.4%").
pub fn format_score(bp: i32) -> String {
    format!("{:.1}%", bp as f64 / 100.0)
}

/// One line per player: civ, cities, population, units, gold, score (share of the world, with
/// the fair share 100/N % of the N alive civs; barbarians have no score), legacy FFH2 score.
pub fn summary(g: &Game) -> String {
    let d = g.d;
    let scores = g.world_scores();
    let alive_civs = g.civ_player_ids().filter(|&p| g.players[p].alive).count().max(1);
    let mut out = format!("  score = share of the world's people, land and knowledge; fair share {:.1}%\n", 100.0 / alive_civs as f64);
    for p in g.player_ids() {
        let pl = &g.players[p];
        if !pl.ever_alive {
            continue;
        }
        let civ = g.player_get_civilization_type(p).map_or("?".to_string(), |c| d.civilizations[c].base.type_.clone());
        let score = if g.player_is_barbarian(p) {
            String::new()
        } else {
            format!(" score={:>6} legacy={}", format_score(scores[p.idx()]), g.player_legacy_score(p))
        };
        out += &format!(
            "  {:2} {:28} alive={:5} cities={:3} pop={:4} units={:4} gold={:5}{}\n",
            p.0,
            civ,
            pl.alive,
            pl.cities.len(),
            pl.total_population,
            pl.units.len(),
            pl.gold,
            score
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same seed, same game: every per-turn sync checksum must match.
    #[test]
    fn same_seed_same_game() {
        let cfg = RunConfig { seed: 7, civs: 3, world: "WORLDSIZE_TINY".into(), turns: 20, verbose: false, report_every: 0, options: Vec::new(), until_supremacy: false };
        let (mut a, _) = new_game(&cfg);
        let sa = autoplay(&mut a, &cfg);
        let (mut b, _) = new_game(&cfg);
        let sb = autoplay(&mut b, &cfg);
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert_eq!((x.turn, x.checksum), (y.turn, y.checksum), "games diverged at turn {}", x.turn);
        }
    }

    /// A game played until supremacy stops early with a decision, and never before the rule's
    /// minimum turn.
    #[test]
    fn until_supremacy_stops_with_a_decision() {
        let cfg = RunConfig { civs: 2, world: "WORLDSIZE_DUEL".into(), turns: 400, until_supremacy: true, ..RunConfig::default() };
        let (mut g, _) = new_game(&cfg);
        let played = play(&mut g, &cfg, |_, _| {});
        match played.outcome {
            Outcome::Supremacy { turn, since, .. } => {
                assert!(since >= SupremacyRule::default().min_turn);
                assert!(turn - since >= SupremacyRule::default().hold_turns);
                assert_eq!(played.stats.last().map(|s| s.turn + 1), Some(turn));
            }
            Outcome::TurnLimit => assert_eq!(played.stats.len(), 400),
            Outcome::Victory { .. } | Outcome::LastStanding { .. } => {}
        }
    }

    /// Victory pursuit over full games (several minutes per game: `cargo test --release -p
    /// ffh-cli -- --ignored`; `scripts/victory_check.py` runs the same check over many seeds):
    /// games the victory planner decides end in a rules victory other than time, before the time
    /// limit. Seeds where the AI without the planner played to the time limit.
    #[test]
    #[ignore]
    fn full_games_end_in_victories() {
        let time = ffh_data::global().t::<VictoryType>("VICTORY_TIME");
        let mut decided = Vec::new();
        for seed in [4, 19] {
            let cfg = RunConfig { seed, civs: 6, world: "WORLDSIZE_STANDARD".into(), turns: 700, ..RunConfig::default() };
            let (mut g, _) = new_game(&cfg);
            let played = play(&mut g, &cfg, |_, _| {});
            eprintln!("seed {seed}: {}", describe_outcome(&g, &played.outcome));
            if let Outcome::Victory { turn, victory: Some(v), .. } = played.outcome {
                if v != time && turn < g.max_turns() {
                    decided.push((seed, g.d.victories[v].base.type_.clone(), turn));
                }
            }
        }
        assert_eq!(decided.len(), 2, "games decided before the time limit: {decided:?}");
    }

    /// Alive civs' scores sum to the whole world (10000) with at least 1 each, every turn.
    fn assert_world_shares(g: &Game) {
        let s = g.world_scores();
        let alive: Vec<PlayerId> = g.civ_player_ids().filter(|&p| g.players[p].alive).collect();
        assert_eq!(alive.iter().map(|&p| s[p.idx()]).sum::<i32>(), 10_000, "turn {}: {s:?}", g.game_turn());
        assert!(alive.iter().all(|&p| s[p.idx()] >= 1), "turn {}: {s:?}", g.game_turn());
        assert_eq!(team_scores(g).iter().map(|&(_, v)| v).sum::<i64>(), 10_000);
    }

    /// Score v2 in a real game: a share of the world every turn, cached by the rules at turn
    /// boundaries, and a saved game resumes with the same scores, checksums and saves.
    #[test]
    fn score_is_a_share_of_the_world_and_resumes_from_a_save() {
        let cfg = RunConfig { seed: 11, civs: 4, world: "WORLDSIZE_TINY".into(), turns: 40, ..RunConfig::default() };
        let (mut g, _) = new_game(&cfg);
        autoplay_with(&mut g, &cfg, |g, _| assert_world_shares(g));
        // Finish the turn in progress: at a turn boundary the cache is fresh after update_score.
        while !g.at_turn_boundary() {
            g.update();
        }
        g.update_score(true);
        let fresh = g.world_scores();
        for p in g.civ_player_ids() {
            assert_eq!(g.player_score[p], fresh[p.idx()]);
            assert_eq!(g.player_calculate_score(p, false, false), fresh[p.idx()]);
        }
        // The team leader holds more than a fair share.
        let n = g.civ_player_ids().filter(|&p| g.players[p].alive).count() as i32;
        assert!(g.team_score[g.rank_team[0]] >= 10_000 / n);

        let bytes = g.save();
        let mut h = Game::load(g.d, &bytes).expect("load");
        assert!(h.save() == bytes, "save(load(save)) differs");
        assert_eq!(h.world_scores(), fresh);
        assert_eq!((&h.player_score, &h.team_score, &h.rank_player), (&g.player_score, &g.team_score, &g.rank_player));
        let ai = ffh_ai::ai_controller();
        for p in h.player_ids() {
            h.controllers[p] = ai.clone();
        }
        let more = RunConfig { turns: 10, ..cfg };
        let sa = autoplay_with(&mut g, &more, |g, _| assert_world_shares(g));
        let sb = autoplay(&mut h, &more);
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert_eq!((x.turn, x.checksum), (y.turn, y.checksum), "resumed game diverged at turn {}", x.turn);
        }
        while !g.at_turn_boundary() {
            g.update();
        }
        while !h.at_turn_boundary() {
            h.update();
        }
        assert!(g.save() == h.save(), "later saves differ");
        assert_eq!(g.players[PlayerId(0)].score_history, h.players[PlayerId(0)].score_history);
    }

    /// The supremacy rule reads team sums of the share of the world.
    #[test]
    fn supremacy_rule_defaults() {
        let r = SupremacyRule::default();
        assert_eq!((r.ratio_pct, r.hold_turns, r.min_turn), (150, 30, 200));
    }

    /// Different seeds should give different games.
    #[test]
    fn different_seed_different_game() {
        let cfg1 = RunConfig { seed: 1, civs: 3, world: "WORLDSIZE_TINY".into(), turns: 5, verbose: false, report_every: 0, options: Vec::new(), until_supremacy: false };
        let cfg2 = RunConfig { seed: 2, ..RunConfig { civs: 3, world: "WORLDSIZE_TINY".into(), turns: 5, verbose: false, report_every: 0, seed: 0, options: Vec::new(), until_supremacy: false } };
        let (mut a, _) = new_game(&cfg1);
        let (mut b, _) = new_game(&cfg2);
        let sa = autoplay(&mut a, &cfg1);
        let sb = autoplay(&mut b, &cfg2);
        assert_ne!(sa.last().map(|s| s.checksum), sb.last().map(|s| s.checksum));
    }

    /// With GAMEOPTION_ADVANCED_START the AI spends its points (cities, units) and leaves
    /// advanced start on its first turn.
    #[test]
    fn ai_spends_advanced_start_points() {
        let cfg = RunConfig { seed: 3, civs: 3, world: "WORLDSIZE_TINY".into(), turns: 2, verbose: false, report_every: 0, options: vec!["GAMEOPTION_ADVANCED_START".into()], until_supremacy: false };
        let (mut g, _) = new_game(&cfg);
        autoplay(&mut g, &cfg);
        for p in g.player_ids() {
            if g.players[p].alive && !g.player_is_barbarian(p) {
                assert!(!g.players[p].cities.is_empty(), "player {p:?} has no city");
                assert_eq!(g.players[p].advanced_start_points, -1);
            }
        }
    }
}

/// Detailed city view for one player: every plot of every city (worked or not) with terrain,
/// feature, bonus, improvement, route and yields.
pub fn inspect(g: &Game, p: PlayerId) -> String {
    let d = g.d;
    let mut out = String::new();
    let name = |s: &str| s.split('_').skip(1).collect::<Vec<_>>().join("_");
    for &c in &g.players[p].cities {
        let city = &g.cities[c];
        out += &format!(
            "city {} size {} yields {:?} food {:+} happy {:+} health {:+} prod {:?} specialists {:?}\n",
            city.name,
            city.population,
            [0, 1, 2].map(|y| g.city_yield_rate(c, YieldType::from_usize(y))),
            g.city_food_difference(c, true),
            g.city_happy_level(c) - g.city_unhappy_level(c, 0),
            g.city_good_health(c) - g.city_bad_health(c, false, 0),
            city.order_queue.first().map(|o| (o.order_type, o.data1)),
            city.specialist_count.iter().enumerate().filter(|(_, &n)| n > 0).map(|(s, &n)| (name(&d.specialists[SpecialistType::from_usize(s)].base.type_), n)).collect::<Vec<_>>()
        );
        for i in 0..ffh_core::defs::NUM_CITY_PLOTS {
            let Some(q) = g.city_index_plot(c, i) else { continue };
            let pl = &g.map.plots[q];
            if pl.working_city != Some(c) {
                continue;
            }
            out += &format!(
                "   {} {:>2} {:?} {:?} {:<10} {:<14} {:<12} {:<18} {}\n",
                if city.working_plot[i] { "W" } else { " " },
                i,
                pl.yields,
                pl.plot_type,
                pl.terrain_type.map_or("-".into(), |t| name(&d.terrains[t].base.type_)),
                pl.feature_type.map_or("-".into(), |f| name(&d.features[f].base.type_)),
                pl.bonus_type.map_or("-".into(), |b| name(&d.bonuses[b].base.type_)),
                pl.improvement_type.map_or("-".into(), |m| name(&d.improvements[m].base.type_)),
                if pl.route_type.is_some() { "road" } else { "" }
            );
        }
    }
    out

}
