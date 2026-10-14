//! Headless game driver: builds a game, attaches AI controllers and drives `CvGame::update`
//! the way the Civ4 exe does (one call per frame), without UI.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ffh_core::controller::{Controller, ControllerRef};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::GameSetup;
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
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig { seed: 1, civs: 4, world: "WORLDSIZE_SMALL".into(), turns: 100, verbose: false }
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
    let setup = GameSetup::standard(d, cfg.civs, &cfg.world, cfg.seed);
    let mut g = Game::new(d, setup);
    g.map_script = Arc::new(FractalMapScript::default());
    let timed = Arc::new(Timed::new(ffh_ai::ai_controller()));
    let ai: ControllerRef = timed.clone();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
    g.autorun = true;
    g.game_init();
    (g, timed)
}

/// Runs `game.update()` until `cfg.turns` turns have been played or the game ends.
/// Returns one record per completed game turn.
pub fn autoplay(g: &mut Game, cfg: &RunConfig) -> Vec<TurnStat> {
    let mut stats = Vec::new();
    let end_turn = g.game_turn() + cfg.turns;
    let mut turn = g.game_turn();
    let mut t0 = Instant::now();
    // Safety net against a stuck turn (a player that never finishes): C++ would hang too.
    let mut updates_this_turn = 0u64;
    while g.game_state == GameStateType::GAMESTATE_ON && g.game_turn() < end_turn {
        g.update();
        updates_this_turn += 1;
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
            stats.push(stat);
            turn = g.game_turn();
            t0 = Instant::now();
            updates_this_turn = 0;
        } else if updates_this_turn > 100_000 {
            panic!("turn {turn} did not finish after {updates_this_turn} updates (a player never ends its turn)");
        }
    }
    stats
}

/// One line per player: civ, leader, cities, population, units, score.
pub fn summary(g: &Game) -> String {
    let d = g.d;
    let mut out = String::new();
    for p in g.player_ids() {
        let pl = &g.players[p];
        if !pl.ever_alive {
            continue;
        }
        let civ = g.player_get_civilization_type(p).map_or("?".to_string(), |c| d.civilizations[c].base.type_.clone());
        out += &format!(
            "  {:2} {:28} alive={:5} cities={:3} pop={:4} units={:4} gold={:5} score={}\n",
            p.0,
            civ,
            pl.alive,
            pl.cities.len(),
            pl.total_population,
            pl.units.len(),
            pl.gold,
            g.player_calculate_score(p, false, false)
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same seed, same game: every per-turn sync checksum must match.
    /// Ignored until the rules engine can play full turns (run with `--ignored`).
    #[test]
    #[ignore]
    fn same_seed_same_game() {
        let cfg = RunConfig { seed: 7, civs: 3, world: "WORLDSIZE_TINY".into(), turns: 20, verbose: false };
        let (mut a, _) = new_game(&cfg);
        let sa = autoplay(&mut a, &cfg);
        let (mut b, _) = new_game(&cfg);
        let sb = autoplay(&mut b, &cfg);
        assert_eq!(sa.len(), sb.len());
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert_eq!((x.turn, x.checksum), (y.turn, y.checksum), "games diverged at turn {}", x.turn);
        }
    }

    /// Different seeds should give different games.
    #[test]
    #[ignore]
    fn different_seed_different_game() {
        let cfg1 = RunConfig { seed: 1, civs: 3, world: "WORLDSIZE_TINY".into(), turns: 5, verbose: false };
        let cfg2 = RunConfig { seed: 2, ..RunConfig { civs: 3, world: "WORLDSIZE_TINY".into(), turns: 5, verbose: false, seed: 0 } };
        let (mut a, _) = new_game(&cfg1);
        let (mut b, _) = new_game(&cfg2);
        let sa = autoplay(&mut a, &cfg1);
        let sb = autoplay(&mut b, &cfg2);
        assert_ne!(sa.last().map(|s| s.checksum), sb.last().map(|s| s.checksum));
    }
}
