# Handoff notes (ffh2-rs)

State at commit 0e4b3f3 (branch ccr-17ac9b54-lbgjpn):

- **Rules port complete:** no `todo!()` left in ffh-core.
- **Tests:** `cargo test --release` passes, including the same-seed determinism tests in ffh-cli.
- **AI-vs-AI games:** run end to end. 300 turns, 6 civs, standard map takes 13-30 ms/turn, with no panics in a build that has debug assertions and overflow checks enabled.
- **Diagnostics:** `ffh autoplay --seed S --civs N --size WORLDSIZE_X --turns T --report K` prints per-player and per-city state every K turns.

## In-flight work (agent worktrees, uncommitted unless noted; lost if the container is reclaimed)

An audit wave compares each group of ffh-core files with the C++/Python. Each agent edits only its own files and reports cross-file issues. The prompt template is in the session transcript. Each group is: open the C++ original of every function, compare line by line (conditions, constants, defaults, rounding, RNG order, side effects, FfH blocks), fix only in assigned files, then build, test and autoplay seeds 3-5 for 200 turns.

**Finished with zero findings:**
- unit/promotions+actions
- player/units+cities
- unit/missions+group/misc+mod
- city/citizens+yields_culture+buildings
- utils+found_value

**Interrupted by the API session limit** (redo or resume):
- unit/combat+movement
- unit/ffh+lifecycle+mod
- group/missions+movement+path
- city/turn+production+mod
- player/economy+civics_religion
- player/start+turn+mod
- player/diplomacy_trade+deal+events
- plot/terrain+units+mod
- plot/ownership+geography+map+area+plot_group
- team/* (had a fix plus a test in progress in tech.rs)
- game/setup_init+turn+game.rs
- game/world+votes+mapgen/mod
- ffh/spells_*
- ffh/random_events_*
- ffh/events+custom+game_utils+dispatch+pycall
- ffh-data loaders (CvInfos read functions, defaults, load order)

**AI improvement** (owns ffh-ai, governor.rs, controller.rs, ffh-cli, docs/AI.md) was interrupted while rewriting governor.rs. Baseline problems, seed 2, 6 civs, standard map:
- 1 city per civ at T50, 1-3 at T100;
- 2-6 techs at T50;
- cities of size 2-3 with 3-6 hammers (the governor favours commerce);
- warriors built before settlers;
- a city left with an empty build queue;
- barbarians reach 120-170 units by T300;
- no conquests;
- naval units idle.

Pending from the audits:
- `ffh-ai/units.rs` `upkeep()`: after `unit_upgrade`, call `unit_do_delayed_death` (C++ CvUnitAI.cpp:820-821).

## Planned next

1. **(Dropped by user decision: slot-order emulation is not needed; document as deviation.)** Formerly: emulate FFreeListTrashArray slot order (after the audits) for per-player units, cities, groups and plot groups: freed slots are reused LIFO and iteration is in slot order. Also add a vote-container free list in game.rs. Order-dependent places:
   - the CvPlayer::doTurn city loop (RNG);
   - the unit/group turn loops;
   - Python getUnitList loops;
   - CvCity::updateTradeRoutes first-max tie;
   - killGoldenAgeUnits first-max;
   - disband draws.
2. **Save/load:** a new save.rs with exhaustive struct destructuring, and a round-trip test (save at turn N, load, same checksums as an uninterrupted run).
3. **Robustness runs:** all world sizes, 2-18 civs, 500 turns, game speeds, handicaps and options, on the checked build: `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true cargo build --release --target-dir target/checked`.
4. **Docs pass:** flip the docs/porting/*.md rows and summaries to ported, then update PORTING.md (items below).

## Items for the docs pass
- Delete stale "AI_baseBonusVal recomputed" bullet: found_value.rs caches exactly like C++ (m_aiBonusValue; reset in AI_doTurnPre position, team merge (shareItems), empire split).
- NEW deviation: per-player city/unit/group lists iterate in creation order; C++ FFreeListTrashArray iterates slot order with LIFO reuse of freed slots -> differs after deletions (RNG draw order across cities/units, first-match tie-breaks). Not a rule.
- path.rs: FAStar re-implemented: lowest total first, ties LIFO (most recently opened); children in DirectionType order; NULL-callback finders cost 1 per step + stepDistance/0 heuristic (only reachability/closed set used); bReuse only when start/info/cardinal/group match.
- getTradeDenial: AI-judgement denials -> no denial; acceptance via Controller::consider_offer.
- Random events: event choice for every player via controller.choose_event (human popup = controller); event texts stored as English templates; messages to game log.
- Votes: diplo_vote / choose_election via controller; vote slot reuse approximates FFreeListTrashArray free list (highest free slot) -- differs only if a vote waits an extra turn and two votes are added in one later turn; clearSecretaryGeneral reads uninitialized data in C++ -> id 0 here.
- Mission timers stay 0 headless (no active player); combat always quick.
- getCombatOdds: float math per source types; original x87 FPU precision could differ in rare rounding at 0.5/1000 edge (pillage odds/display only).
- AI spellcasting before groupAttack (AI_DirectDamageCast/SummonCast/BuffCast) omitted (AI).
- deleteCity clears plots' stale working-city refs (equivalent to C++ getWorkingCity() NULL).
- PLANNED (after audit wave): emulate FFreeListTrashArray slot order (LIFO reuse) for per-player units/cities/groups/plot groups (+ vote containers free list in game.rs). Order-dependent sites found: CvPlayer::doTurn city loop RNG, unit/group turn loops, Python getUnitList loops, CvCity::updateTradeRoutes first-max tie, killGoldenAgeUnits first-max, disband draws.
- Stale comment: city/citizens.rs module doc + "governor stand-ins" section (helpers now in governor.rs).
- Deliberate AI omission noted: CvUnit::found isUnitAllowedPermDefense branch (AI bookkeeping; affects AI_groupFirstVal group head).
- Audit plot/terrain+units+mod: zero findings. Note: py_can_build returns -1 (FfH CvGameUtils.canBuild AI-only bans -- no forts, Fallow no farms, workshops need Guilds, no cottages under Aristocracy, no windmills under Arete -- are AI policy, documented deviation; the new AI may adopt them).
- Audit results so far: team (fix: chooseTech human-only, merged), player/start+turn+mod (fix: AI_reset clears bonus cache, merged); zero findings: unit combat+movement, unit ffh+lifecycle, group+path, city turn+production, plot ownership/geo/map/area, game setup+turn, diplomacy/trade+deal+events.
- Deliberate AI omissions to document: AI_doFavoriteReligion never runs (favorite_religion stays None, so player_can_convert's AI restriction never applies); AI_nowHasTech production-dirty reset omitted; human-only BUTTONPOPUP_CONFIRMSETTLEMENT (Kuriotates radius/settlement) has no controller hook (suggest Controller::confirm_settlement).
- Map plot_set_plot_city debug_assert can fire when a radius-3 city dies (faithful C++ negative radius count; FAssert debug-only).
