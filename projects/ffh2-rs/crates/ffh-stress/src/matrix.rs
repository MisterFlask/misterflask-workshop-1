//! Settings matrices: each group varies one dimension around the standard game (small map,
//! 5 civs, normal speed, Noble, first era, temperate, medium sea level); `random` draws every
//! dimension at once.

use ffh_data::*;

use crate::settings::Settings;

/// Options left out of the matrices: world-builder scenario flags (scenarios are out of scope)
/// and options with no rules effect in a single-process game.
fn skipped_option(name: &str) -> bool {
    name.starts_with("GAMEOPTION_WB_") || name == "GAMEOPTION_LOCK_MODS" || name == "GAMEOPTION_NEW_RANDOM_SEED"
}

/// Type names of every row of an info table.
fn names<T: InfoId>(d: &GameData, n: usize) -> Vec<String> {
    (0..n).map(|i| d.type_name(T::from_usize(i)).to_string()).collect()
}

pub const GROUPS: &[&str] = &["speed", "handicap", "era", "world", "civs", "climate", "sea", "options", "teams", "victory", "random"];

pub fn build(d: &GameData, group: &str, seeds: &[u32], turns: i32, n_random: usize) -> Result<Vec<Settings>, String> {
    if group == "all" {
        let mut v = Vec::new();
        for g in GROUPS {
            v.extend(build(d, g, seeds, turns, n_random)?);
        }
        return Ok(v);
    }
    let speeds = names::<GameSpeedType>(d, d.game_speeds.len());
    let handicaps = names::<HandicapType>(d, d.handicaps.len());
    let eras = names::<EraType>(d, d.eras.len());
    let worlds = names::<WorldSizeType>(d, d.worlds.len());
    let climates = names::<ClimateType>(d, d.climates.len());
    let seas = names::<SeaLevelType>(d, d.sea_levels.len());
    let options: Vec<String> = names::<GameOptionType>(d, d.game_options.len()).into_iter().filter(|o| !skipped_option(o)).collect();
    let victories = names::<VictoryType>(d, d.victories.len());
    let mut v = Vec::new();
    let base = |seed: u32| Settings::base(group, seed, turns);
    match group {
        "speed" => {
            for sp in &speeds {
                for &seed in seeds {
                    v.push(Settings { speed: sp.clone(), ..base(seed) });
                }
            }
        }
        "handicap" => {
            for h in &handicaps {
                for &seed in seeds {
                    v.push(Settings { handicap: h.clone(), ..base(seed) });
                }
                // AI slots at that handicap too (as for a human player's slot).
                v.push(Settings { handicap: h.clone(), player_handicap: h.clone(), ..base(seeds[0]) });
            }
        }
        "era" => {
            for e in &eras {
                for &seed in seeds {
                    v.push(Settings { era: e.clone(), ..base(seed) });
                }
            }
        }
        "world" => {
            for (i, w) in worlds.iter().enumerate() {
                let default = d.worlds[WorldSizeType::from_usize(i)].default_players.max(2) as usize;
                for &seed in seeds {
                    v.push(Settings { world: w.clone(), civs: default, ..base(seed) });
                }
                v.push(Settings { world: w.clone(), civs: 2, ..base(seeds[0]) });
            }
        }
        "civs" => {
            // Crowded maps up to MAX_CIV_PLAYERS (no open slot left for mid-game players).
            let max = ffh_core::GameSetup::MAX_CIVS;
            for (w, counts) in [
                ("WORLDSIZE_DUEL", vec![4, 8]),
                ("WORLDSIZE_TINY", vec![2, 6, 12]),
                ("WORLDSIZE_SMALL", vec![10, 18]),
                ("WORLDSIZE_STANDARD", vec![14, 24]),
                ("WORLDSIZE_LARGE", vec![18, max]),
                ("WORLDSIZE_HUGE", vec![22, max - 4, max]),
            ] {
                for c in counts {
                    for &seed in seeds {
                        v.push(Settings { world: w.into(), civs: c, ..base(seed) });
                    }
                }
            }
        }
        "climate" => {
            for c in &climates {
                for &seed in seeds {
                    v.push(Settings { climate: c.clone(), ..base(seed) });
                }
            }
        }
        "sea" => {
            for s in &seas {
                for &seed in seeds {
                    v.push(Settings { sea: s.clone(), ..base(seed) });
                }
            }
        }
        "options" => {
            for o in &options {
                for &seed in seeds {
                    v.push(Settings { options: vec![o.clone()], ..base(seed) });
                }
            }
        }
        "teams" => {
            for &seed in seeds {
                v.push(Settings { civs: 6, teams: 2, ..base(seed) });
                v.push(Settings { civs: 6, teams: 3, ..base(seed) });
                v.push(Settings { civs: 4, teams: 2, options: vec!["GAMEOPTION_PERMANENT_ALLIANCES".into()], ..base(seed) });
                v.push(Settings { world: "WORLDSIZE_STANDARD".into(), civs: 8, teams: 4, ..base(seed) });
            }
        }
        "victory" => {
            for &seed in seeds {
                // Time victory soon after the start.
                v.push(Settings { max_turns: 60, ..base(seed) });
                // Time victory at the end of a quick game (480 turns; no max turns given).
                v.push(Settings { speed: "GAMESPEED_QUICK".into(), turns: 500, ..base(seed) });
                // Score target, in basis points of the world (score v2): the leader always holds
                // at least 10000/N >= 150, so the score victory fires as soon as victory tests
                // start (turn 11).
                v.push(Settings { target_score: 150, ..base(seed) });
                // City elimination: a player that lost N cities is out.
                v.push(Settings { max_city_elimination: 1, civs: 6, options: vec!["GAMEOPTION_AGGRESSIVE_AI".into()], ..base(seed) });
                // Conquest / domination on a crowded small map with warmongering options.
                v.push(Settings {
                    world: "WORLDSIZE_DUEL".into(),
                    civs: 2,
                    options: vec!["GAMEOPTION_ALWAYS_WAR".into(), "GAMEOPTION_AGGRESSIVE_AI".into()],
                    ..base(seed)
                });
                // Only the FfH altar/tower victories (and time), late start era.
                v.push(Settings {
                    era: eras.get(2).cloned().unwrap_or_else(|| eras[0].clone()),
                    no_victory: victories.iter().filter(|x| !x.contains("ALTAR") && !x.contains("TOWER") && !x.contains("TIME")).cloned().collect(),
                    ..base(seed)
                });
                // The FfH building victories, forced: the first civ's capital gets the victory
                // buildings at turn 100.
                for vic in victories.iter().filter(|x| x.contains("ALTAR") || x.contains("TOWER")) {
                    v.push(Settings { grant_victory: Some(format!("{vic}@100")), ..base(seed) });
                }
                // No victory at all.
                v.push(Settings { no_victory: victories.clone(), ..base(seed) });
            }
        }
        "random" => {
            let mut rng = Lcg(0x9e37_79b9 ^ seeds.iter().fold(0u64, |a, &s| a.wrapping_mul(31).wrapping_add(s as u64)));
            for _ in 0..n_random {
                let wi = rng.below(worlds.len());
                let default = d.worlds[WorldSizeType::from_usize(wi)].default_players.max(2) as usize;
                let civs = match rng.below(4) {
                    0 => 2,
                    1 => default,
                    2 => default + 1 + rng.below(default + 2),
                    _ => 2 + rng.below(default.max(2)),
                }
                .min(ffh_core::GameSetup::MAX_CIVS);
                let mut opts = Vec::new();
                for _ in 0..rng.below(4) {
                    let o = options[rng.below(options.len())].clone();
                    if !opts.contains(&o) {
                        opts.push(o);
                    }
                }
                let h = handicaps[rng.below(handicaps.len())].clone();
                v.push(Settings {
                    seed: 1000 + rng.below(100_000) as u32,
                    world: worlds[wi].clone(),
                    civs,
                    speed: speeds[rng.below(speeds.len())].clone(),
                    handicap: h.clone(),
                    player_handicap: if rng.below(4) == 0 { h } else { "HANDICAP_NOBLE".into() },
                    era: eras[if rng.below(2) == 0 { 0 } else { rng.below(eras.len()) }].clone(),
                    climate: climates[rng.below(climates.len())].clone(),
                    sea: seas[rng.below(seas.len())].clone(),
                    options: opts,
                    teams: if civs >= 4 && rng.below(5) == 0 { 2 } else { 0 },
                    ..base(0)
                });
            }
        }
        _ => return Err(format!("unknown matrix {group} (groups: {} all)", GROUPS.join(" "))),
    }
    Ok(v)
}

/// Deterministic generator for the random matrix (64-bit LCG, high bits).
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n.max(1) as u64) as usize
    }
}
