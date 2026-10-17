# Game score v2: a share of the world's people, land and knowledge

**For players:** your score is your share of the world: the average of your share of all people,
all land and all knowledge.

The score's only job is to show, as well as we can, how well a civ is doing in this game
relative to the other civs. FFH2's own score (`CvGameUtils.calculateScore`) was replaced on
purpose (a user decision; listed in `docs/PORTING.md` under "Changes from FFH2"). FFH2's
formula stays available as the *legacy score* for tools.

Code: `Game::world_score_shares` / `Game::world_scores` and the apportionment
`apportion_score` in `crates/ffh-core/src/game/turn.rs`; `update_score` caches the result.

## 1. Formula

**Who is scored.** S = the alive, non-barbarian players at the moment of scoring, N = |S|.
Joined Mercurians, split-off empires and Infernals are in S. Dead players and barbarians score 0.

**Inputs** (all maintained by the rules already):

| Symbol | Meaning | Rust |
|---|---|---|
| `pop_p` | sum of the player's city sizes | `players[p].total_population` |
| `land_p` | land plots the player owns (not the lagged `total_land_scored`) | `players[p].total_land` |
| `know_T` | team T's FFH2 tech score: sum over T's known techs of (era + 1) (`utils::get_tech_score`) | the largest `players[m].tech_score` among T's alive members m |

Members of a team normally have the same `tech_score` (0 mismatches in 1243 multi-member team
snapshots of the dataset). The exception is a repeatable tech (Future Tech): it adds its score
to the members on the team each time the team learns it, but is never `isHasTech`, so a member
that joins later (`addPlayerAdvanced` credits only `isHasTech` techs, as in C++) lacks those
repeats. The stress matrix found this in two DEITY late-era games (178 vs 172). Taking the
largest member value counts the team's full knowledge whichever member comes first (the design
spec said "the first alive member", identical whenever members agree). A `debug_assert` checks
that every member holds at least the team's non-repeatable known techs.

**Vassals** (applied first): a vassal keeps `floor(x / 2)` of its people and of its land; the
rest, `x - floor(x / 2)`, goes to its master team's leader (first alive member). Knowledge is not
transferred. This is a total-conserving version of Civ4's half credit: `getPopScore` /
`getLandScore(bCheckVassal)` give the vassal's half to *every* master member, which double-counts
on multi-member teams. (Untested on real games: no vassals occurred in the dataset.)

**Shares**, all `i64`, floor division, players in id order:

```
P = sum over S of pop,  L = sum over S of land
pop_T = sum of pop over T's alive members (after the vassal transfer)
K = sum of know_T over the teams with an alive member (each team once)
a_p = P > 0 ? 1_000_000 * pop_p / P   : 1_000_000 / N
b_p = L > 0 ? 1_000_000 * land_p / L  : 1_000_000 / N
c_p = K == 0      ? 1_000_000 / N
    : pop_T > 0   ? 1_000_000 * know_T * pop_p / (pop_T * K)
    :               1_000_000 * know_T / (members_T * K)
u_p = a_p + b_p + c_p
```

A team's knowledge is counted once and split among its members by population, so a team's sum
is the team's share of the world.

**Apportionment to basis points.** U = sum of u_p.
1. `bp_p = 10000 * u_p / U`.
2. The leftover `10000 - sum(bp)` goes one point each to the largest remainders
   `10000 * u_p mod U` (ties: lower player id). If U = 0, 10000 is split equally and the
   leftover goes to the lowest ids.
3. For each p with `bp_p = 0`, in id order: take 1 point from the current largest bp (ties: lower
   id) and set `bp_p = 1`.

**Guarantees.** Alive civs sum to exactly 10000 and each has at least 1 bp (so `cpp_rank_player`'s
"score > 0 = a real player" still holds; checked by a `debug_assert`). No overflow (largest
product about 1e6 x 1e3 x 5e3). Integer only and deterministic.

**Fixed test vectors** (rows `(player, team, pop, land, know)`):
`(0,0,20,100,50) (1,1,11,41,30) (2,2,9,59,30) (3,0,0,0,50)` give `{4848, 2509, 2642, 1}`; with
team 1 a vassal of team 0, `{5698, 1659, 2642, 1}`. The Rust tests also check 24 random states
against the Python reference (`world_scores()` in the design's `judge/final_score.py`).

## 2. Display

- Stored: integer basis points (`player_score`, `team_score`, `score_history`); 10000 is the
  whole world.
- Shown: a percentage with one decimal (2537 -> "25.4%"), next to the fair share 100/N %
  (e.g. "fair share 16.7%"). `ffh autoplay` prints both, plus the legacy score.
- Detail views can show the three shares (`world_score_shares`; `ffh stats` reports them as
  `score_pop_share`, `score_land_share`, `score_know_share`, parts per million).

Typical team-leader values in the 36 games (median):

| turn | leader's share | leader / runner-up |
|---|---|---|
| 100 | 21.3% (13.6-32.8) | 1.12 |
| 200 | 23.4% | 1.18 |
| 300 | 26.2% | 1.19 |
| 500 | 29.3% | 1.24 |
| final | 34.1% (17.3-54.9) | 1.23 |

## 3. Teams, joins and splits

- **Team score** = the sum of its alive members' bp, as in the rules before; team scores also sum
  to 10000.
- **Joins and splits** (a Mercurian joining, `formEmpire`) move points without creating them: at a
  join the team's share changes by a median of x1.00 (max x1.13), against x1.28 (max x1.51) with
  FFH2's score. A 0-city member scores 1 bp (with FFH2's score it held about 77% of an even split,
  through the team's shared techs).
- **Infernals** arrive at about 9% of the world (FFH2's score: about 14%), because their granted
  techs count as knowledge.

## 4. When it is recomputed

The score is game-level and costs O(players + teams) per refresh. It uses the existing
`score_dirty` / `update_score(force)` machinery (on `update`, `doTurn`, `testVictory`; forced on
`addTeam`). Every input change marks the score dirty: population (`player_change_pop_score`;
the population score is the identity), techs (`player_change_tech_score`), alive, game turn, and
-- added for v2 -- land plots (`plot_set_owner`, `plot_set_plot_type`), vassal changes
(`team_set_vassal`) and team changes (`player_set_team`). Mid-turn readers (Barbarian-trait war,
Ascension, the switch-civs event) read the cached values, as in C++.

## 5. Consumers

All rules consumers read order or ratios and keep their code; only the score-victory target's
unit changes.

| Consumer | Behaviour with v2 |
|---|---|
| Time victory (`test_victory_team`, end score) | the team with the largest share of the world's people, land and knowledge at max turns wins; a tie means no winner, as in C++. It differs from FFH2's winner in 10 of the 26 time games of the dataset; the closest final margin was 3 bp, no exact ties. |
| Score victory (`target_score`) | the target is in bp: 5000 = half the world; 0 = off. |
| ffh-stress `victory` matrix (`target_score: 150`) | still fires as soon as victory tests start: a leader always holds >= 10000/N >= 150 bp for N <= 66. |
| Ranks (`update_score`; ties to the higher index) | unchanged, computed from `world_scores()`. |
| Cut Losers, High to Low, Flexible Difficulty | cut, demote or adjust by real standing. |
| Barbarian-trait war (human Barbarian-trait civs: own >= 1.5x the rank-1 player) | identical up to turn 100; somewhat more frequent mid-game: player-level leader >= 1.5x the runner-up in 6/11/11/14% of snapshots at turns 150/200/300/500, against 3/8/6/11% with FFH2's score. |
| Ascension's Godslayer target, switch-civs event | follow the new ranks. |
| Sync checksum (reads `player_score`) | values change once; no stored goldens. |
| `score_history` | a history of world share. |
| Saves | `save::FORMAT_VERSION` 2: same fields, new meaning. |

The C++ AI's uses (attitude, votes, tech trade, surrender, FINAL_WAR at 90%) read ranks and ratios;
they are not ported and would work unchanged. Hall of Fame and replay final scores are not ported;
the final/victory branches of `getScoreComponent` were deleted.

## 6. --until-supremacy

`SupremacyRule::default()` = 1.5x the runner-up's team score held 30 turns, **counted from turn
200** (was 150 with FFH2's score). Simulated on team sums of v2 at 10-turn snapshots; "right" =
names the team that wins the full game under v2 (the cultural winner, or the v2 time-victory
winner):

| rule | games | fired | median turn | right |
|---|---|---|---|---|
| **1.5x / 30 / from 200 (chosen)** | fit (24) | 10 | 495 | 10/10 |
| **1.5x / 30 / from 200 (chosen)** | held-out (12) | 5 | 360 | 3/5 |
| **1.5x / 30 / from 200 (chosen)** | all (36) | 15 (42%) | 440 (mean 424) | 13/15 (87%); 108 turns saved per game |
| 1.5x / 30 / from 150 (old rule on v2) | all | 15 | 400 | 12/15 (80%) |
| 1.4x / 50 / from 200 | all | 18 | 390 | 15/18 (83%) |
| 1.35x / 50 / from 150 | all | 19 | 290 | 15/19 (79%) |
| 2.0x / 30 / from 150 | all | 2 | | |
| FFH2 score, old rule | all | 26 | 380 | 42% vs the final control leader |

On held-out every shortlisted rule fired in 5/12 games with 3/5 right, so the start turn was
chosen on fit: moving it from 150 to 200 removes an early false fire (std6 205). The two misses are
real reversals: small4 306 (the Dovie held 2x the people at turns 200-300, then the Ljosalfar
overtook them by turn 400) and std6 206 (a cultural victory by a trailing civ). Other game speeds
could scale `min_turn` by `estimate_end_turn / 690` (not done).

## 7. Why this design

Three designs were evaluated on 36 full AI games (std6 x24, small4 x6, large9 x6; normal speed,
played to the rules' end; per-civ snapshots every 10 turns), split 24 fit / 12 held-out
(stratified by map group and outcome). Targets, none of which reads a score:
`control` = 0.5 x pop share + 0.5 x land share;
**T1** = final control share (top-1 accuracy of the leader, Spearman rho, lock-in turn),
**T1x** = final share of pop, land, production, commerce and military (rho);
**T2** = elimination AUC (within 100 turns, or ever);
**T3** = the non-time (cultural) winner team leads;
**T4** = control 100 turns later (top-1 and rho);
W-L = games where a candidate's per-game rho beats FFH2's score minus games where it loses.
Headline window: turns 100-499, games weighted equally.

**Held-out headline:**

| candidate | T1top | T1rho | T1x | T4top | T4rho | T2<=100 | T2any | T3 | W-L T1rho / T4rho |
|---|---|---|---|---|---|---|---|---|---|
| FFH2 score | 40% | .63 | .67 | 60% | .74 | .95 | .84 | 40% | - |
| design 1, world share (7 inputs) | 37% | .66 | .68 | 60% | .79 | .82 | .75 | 47% | 6-6 / 8-4 |
| design 2, forward power (fitted) | 42% | .70 | .71 | 71% | .85 | .83 | .76 | 46% | 9-3 / 12-0 |
| **design 3, people/land/knowledge (chosen)** | 45% | .69 | .70 | 71% | .82 | .80 | .77 | 37% | 11-1 / 11-1 |

T1 lock-in on held-out (median turn, games locked by turn 400): FFH2 490 (6/12); design 1 530
(4/12); design 2 525 (4/12); design 3 370 (6/12).

All 36 games, design 3 vs FFH2's score: T1top 50% vs 48%, T1rho .65 vs .63, T4top 67% vs 60%,
T4rho .81 vs .75; W-L 22-14 / 33-3; T1 lock-in 355 (21/36) vs 365 (20/36).

**Paired bootstrap on held-out (90% intervals of per-game window means):**
- design 3 minus design 1: T1rho +.031 [+.011, +.052], T1x +.025 [+.002, +.048], T4rho +.036
  [+.015, +.057], T1top +8 points [+0.5, +18], T4top +11 points [+4.5, +20] -- design 3 is
  significantly better on every prediction metric.
- design 2 minus design 3: only T4rho differs, +.029 [+.012, +.053] -- the target design 2 was
  fitted to; T1rho +.014 [-.011, +.037], T1x +.011 [-.013, +.035], T1top -3 points [-8, +2],
  T4top 0 [-5, +4]. At horizon 200 the T4rho gap shrinks to .773 vs .758.

**Simplicity breaks the near-tie with design 2:** three stocks the rules already maintain, equal
untuned weights and about 40 lines of integer code, against a fitted softmax over a Cobb-Douglas
index with fixed-point log/exp tables, a turn ramp and five fitted constants. Design 3 is also
ahead on T1top (45% vs 42%) and on lock-in (turn 370, 6/12 locked by 400, vs 525, 4/12).

**Rejected combination:** adding a culture-stock share at 10% (30/30/30/10) looked better on fit
(T3 73% vs 64%, T3 lock-in 230 with 7/7 vs 310 with 5/7) but lost on held-out (T1top 40% vs 45%,
T4top 65% vs 71%, T1 lock-in 510 vs 370, W-L 7-5 vs 11-1, T3 no better: 36% vs 37%). A military
term at 10% was neutral. Plain people/land/knowledge stays.

Why FFH2's score was a poor measure: before turn 150 its leader was the eventual (time-victory)
winner in only 25-33% of games; tech share is nearly equal across civs and compresses the ratios;
and a 0-city team member (a joined Mercurian) carried a large score through the team's shared
techs, so team sums jumped when such members arrived.

## 8. Tooling

- `Game::player_legacy_score(p)`: FFH2's formula (`calculateScore(p, false, false)`), computed
  on demand, never stored or saved, read by no rule. The `max_*` / `init_*` counters stay for it.
- `ffh stats`: top-level `score_version: 2`; per civ `score` (bp), the three shares and
  `legacy_score`. Output without `score_version` predates v2 (its `score` is FFH2's).
- `ai_metrics.py`: instead of the mean of `score` (always 10000/N) it reports the leading team's
  share, its ratio to the runner-up, the fair share 100/N %, and the legacy score for absolute
  progress. Pre-v2 results (e.g. `--compare old.json`) show only the legacy score, so results from
  before and after the switch are not mixed.
- `run::team_scores` sums `world_scores()` by team; `ffh autoplay`'s summary shows each civ's
  share (none for the barbarians) and legacy score.

## 9. Known limits

- **Zero-sum.** Absolute progress is invisible; use the legacy score for it.
- **No military, culture or wonder term.** A large army counts only once it conquers. The
  cultural winner leads a little less often late (T3: 56% vs 61% on all games).
- **Early game.** Before cities exist, civs differ only by starting techs: the leader /
  runner-up ratio at turn 10 has a median of 1.08 (FFH2's score: 1.00).
- **Ties.** The leader is tied at bp resolution in 19-20 of 2430 snapshots, mostly early. Time
  victory treats a tie as no winner, as C++ does.
- **Vassal rule untested** on real games; a small deviation from `getPopScore`/`getLandScore`.
- **Elimination AUC** is lower in the harness (T2<=100 .80 vs .95) only because 0-city Mercurians
  survive through their team while holding nothing; among starting civs it is .957 vs .941 for
  FFH2's score (T2any .841 vs .830).
