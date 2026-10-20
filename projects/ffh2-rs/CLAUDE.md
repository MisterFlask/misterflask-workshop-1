# ffh2-rs: Fall from Heaven 2 (0.41) ported to Rust

**New session?** Read this file, `README.md` and the doc for the area you touch (`docs/PORTING.md`,
`docs/AI.md` + `docs/AI_LOG.md`, `docs/PLAY_DESIGN.md` + `docs/HUMAN.md`, `docs/ART.md`, `docs/R3D.md`,
`docs/SCORE.md`). What is merged is what git says (`git log origin/master`); open work and known
gaps live in the area docs, not in a session log. Out of scope for now (the owner's call):
scenarios, and porting the C++ AI.

Port of the FFH2 0.41 Civ4 mod (base mod only), the base for a game of its own. The rules
engine (DLL C++ + rules Python) was ported function by function; the AI is new. FFH2 0.41 is
the starting point, not an invariant: once the base is stable and attractive, the game will
change sweepingly (see "FFH2 is the starting point" under Hard rules).

## Layout

```
data/xml/        vendored gameplay XML (FFH2 0.41 + stock BTS files FFH2 inherits)
data/text/en.tsv English text (TXT_KEY -> string)
crates/ffh-data  XML -> typed info tables (port of CvInfos.cpp / CvXMLLoadUtility)
crates/ffh-core  rules engine (port of CvGame/CvPlayer/CvTeam/CvCity/CvUnit/CvPlot/... + rules Python)
crates/ffh-ai    AI players (new design)
crates/ffh-cli   `ffh` binary: autoplay, benchmarks, inspection
crates/ffh-stress `ffh-stress` binary: AI games over a settings matrix, crashes caught per game
crates/ffh-play  `ffh-play`: human play (docs/PLAY_DESIGN.md, docs/HUMAN.md)
crates/ffh-web   the engine as WebAssembly and the page side of the static site (docs/WEB.md)
tools/           fetch-reference.sh, vendor_xml.py, gen_enums.py, gen_defines.py,
                 agents/ (shared builds, run cache, CPU slots: docs/AGENTS.md)
docs/            ARCHITECTURE.md, DATA.md, PORTING.md (status), AGENTS.md (parallel agents)
```

## Parallel agents

Other agents share this machine (4 cores) and repository. Follow docs/AGENTS.md:
- Build with `B=$(tools/agents/build-rev.sh <rev>)` / `W=$(tools/agents/build-tree.sh)` (your
  worktree's current files; `--bins ffh` for AI work), not `cargo build` in your worktree: builds
  are shared, incremental and serialized. `tools/agents/build-tree.sh --test` is the
  `cargo build --release && cargo test --release` check below, run on your worktree's files.
- A/B against a base: `tools/agents/ab.sh <ai_metrics.py args>` (the base side's games come
  from the shared run cache). `ai_metrics.py` takes machine-wide CPU slots per game; wrap other
  game runs in `tools/agents/slot.sh` (`--exclusive` for `ffh bench`).
- If you did build in your worktree, `rm -rf target` before finishing.

## Reference source (read-only, NOT in this repo)

`tools/fetch-reference.sh` clones the pinned sources to `/home/user/ref`:

- `/home/user/ref/ffh2/CvGameCoreDLL/*.cpp|h` -- FFH2 0.41 DLL source (the reference for porting)
- `/home/user/ref/ffh2/Assets/python/**` -- FFH2 rules Python (spells, events, ...)
- `/home/user/ref/ffh2/Assets/XML/**` -- original XML (schemas: `*Schema.xml`)

When porting, read the C++/Python being ported -- never port from memory.

## Hard rules

- **No external crates.** crates.io is unreachable from this environment; std only.
- **Determinism.** Simulation state changes only through the game RNG
  (port of `CvRandom`). No `HashMap` iteration in simulation code (use `Vec`/
  `BTreeMap` or sort first). No wall-clock, no thread-dependent ordering.
- **Integer math in rules,** as in the C++ (it uses `int` with percentages).
  Port floating-point only where the C++ itself uses float, with the same ops.
- **FFH2 is the starting point, not an invariant.** The port reproduces FFH2 0.41 so the game
  starts from a known, working base; once that base is stable and attractive, the rules will
  change sweepingly. Equivalence to FFH2 is temporary: don't add tests, tools or machinery whose
  only purpose is keeping the port identical (or bit-exact) to FFH2, and a change is not wrong
  because it departs from FFH2.
- **Rules changes are deliberate and recorded.** Change rules on purpose, not as a side effect
  of other work: say what changed and why in the commit, and list gameplay changes from FFH2 0.41
  in docs/PORTING.md ("Changes from FFH2").
- **When porting C++/Python,** port what the source does, quirks included, so the port can be
  checked against it; changing the behaviour is a separate step (a rules change, above). Mark
  quirks `// C++ quirk:` so they are easy to find when the rules change. Two RNG draws in one C++
  expression follow the original DLL compiler (MSVC 2003, x86): function-call *arguments* are
  evaluated right to left (`f(rand(a), rand(b))` draws `b` first), operands of binary operators
  left to right (`rand(a) + rand(b)` draws `a` first); comment `// C++ quirk: MSVC evaluation order`.
  Skip pure UI/graphics/sound/network code (messages may go to the game log).
- `cargo build --release && cargo test --release` must pass before committing.
  `cargo run --release -p ffh-cli` must report `0 warnings` for data loading.

## Naming conventions (C++ -> Rust)

Info tables (`ffh-data`):
- Each `CvFooInfo` is `FooInfo`; table `data.foos: IdxVec<FooType, FooInfo>`.
- Field name = C++ getter name without `get`/`is`, snake_case:
  `getPrereqAndTech()` -> `prereq_and_tech`, `isAnimal()` -> `animal`,
  `getYieldChange(i)` -> `yield_change` (an array).
- C++ `int` holding an enum/type with -1 = none -> `Option<XType>`.
- Dense per-type arrays (`m_piFoo` sized `GC.getNumXInfos()`) -> `IdxVec<XType, T>`.
  2D arrays -> `IdxVec<XType, [i32; YieldType::NUM]>` or nested `IdxVec`/`Vec`.
- Yields -> `[i32; YieldType::NUM]` (3), commerces -> `[i32; CommerceType::NUM]` (4).
- Python callback strings (`PyResult`, `PyRequirement`, ...) -> `String`.
- Hard-coded C++ enum values are consts: `UnitAIType::UNITAI_ATTACK`,
  `DomainType::DOMAIN_LAND`, `YieldType::YIELD_FOOD` (see `enums.rs`, generated).
- Global defines: `data.defines.max_hit_points` (generated `defines_gen.rs`; one
  field per define, lowercase name). Text defines listed in C++ `SetPostGlobalsGlobalDefines` are
  resolved type indices (i32, -1 = none; other text defines read as -1) -- convert with `opt_from_i32`.
- Type names hard-coded in C++/Python: `data.t::<PromotionType>("PROMOTION_HERO")`
  (panics if unknown) or `data.ty::<T>(..)` (Option). Cache in hot paths.

Readers (`crates/ffh-data/src/registry.rs`, struct `Rd`) mirror `CvXMLLoadUtility`:
`GetChildXmlValByName(&i, "iX", def)` -> `rd.int_or("iX", def)`;
`FindInInfoClass` on a tag -> `rd.rf::<XType>("Tag")`;
`SetVariableListTagPair(&p, "Tag", .., GC.getNumXInfos(), def)` -> `rd.pairs_int::<XType>("Tag", def)`
(also `pairs_bool`, `pairs_string`, `pairs_ref`); `SetYields` -> `rd.yields("Tag")`;
`SetCommerce` -> `rd.commerces("Tag")`; fixed ref lists -> `rd.ref_list_fixed`.
Replicate C++ defaults exactly: use `int_or(tag, d)` only where the C++ call passes a default `d`
(most calls pass none, so absent ints are 0 even when the XML conventionally writes -1).

## Rules engine conventions (`ffh-core`)

`Game` (`game.rs`) owns all state: `d: &'static GameData`, `setup` (CvInitCore),
`map` (CvMap: plots + areas), `players`/`teams` (`IdxVec<PlayerId|TeamId, _>`, sized
to this game, barbarian last), slabs `units`/`cities`/`groups`/`plot_groups`/`deals`
(global, keyed by never-reused ids; per-owner id lists live on `Player`),
`soren_rand`/`map_rand` (`CvRandom`), `log`, `controllers`, plus CvGame's own members.

C++ -> Rust mapping:
- Methods of class `CvFoo` become `impl Game` methods named `<foo>_<method>` taking the
  object id first: `CvUnit::canMoveInto(pPlot, bAttack)` ->
  `fn unit_can_move_into(&self, u: UnitId, plot: PlotIdx, attack: bool) -> bool`.
  Prefixes: `unit_` (UnitId), `city_` (CityId), `player_` (PlayerId), `team_` (TeamId),
  `plot_` (PlotIdx), `group_` (GroupId), `area_` (AreaId), `plot_group_` (PlotGroupId),
  `deal_` (DealId), `map_` (CvMap), none for CvGame. C++ `const` -> `&self`, else `&mut self`.
- Trivial getters/setters (`return m_iFoo;`, `m_iFoo = iNewValue;` with no side effects)
  are NOT methods: use fields (`self.units[u].damage`). Anything with logic, asserts that
  clamp, or side effects IS a method (`change_damage` that kills, `set_owner`, ...).
- Struct fields: `m_iFooBar` -> `foo_bar: i32`; `m_bFoo` -> `foo: bool`; enum members ->
  `Option<XType>` (or `XType` if never NONE); `m_paiFoo` dense by X -> `IdxVec<XType, i32>`;
  per-player/per-team arrays -> `IdxVec<PlayerId|TeamId, T>` sized `num_players()/num_teams()`;
  `IDInfo` -> `Option<UnitId|CityId>`; `CLinkList<IDInfo>` -> `Vec<UnitId>` (ordered);
  coordinates stay `x: i32, y: i32` where C++ stores them.
- Pointers: `CvPlot*` -> `PlotIdx`/`Option<PlotIdx>`, `CvUnit*` -> `UnitId`/`Option<UnitId>`,
  `CvCity*` -> `CityId`/`Option<CityId>`, `CvArea*` -> `AreaId`, `CvSelectionGroup*` -> `GroupId`.
- Globals: `GET_PLAYER(e)` -> `self.players[e]`; `GET_TEAM(t)` -> `self.teams[t]`;
  `GC.getMapINLINE().plotINLINE(x, y)` -> `self.map.plot_xy(x, y)` (Option, wraps);
  `GC.getXInfo(e)` -> `self.d.xs[e]`; `GC.getDefineINT("X")` -> `self.d.defines.x`;
  `getSorenRandNum(n, "s")` -> `self.soren_rand_num(n, "s")`; `MAX_PLAYERS` loops ->
  `self.player_ids()`; `BARBARIAN_PLAYER` -> `self.barbarian_player()`.
- Iterating a player's units/cities/groups while mutating: clone the id list first
  (`for u in self.players[p].units.clone()`), and re-check `self.units.contains(u)`
  if the loop body can destroy units.
- AI calls inside rules code:
  * decisions (`AI_conquerCity`, `AI_chooseResearch`, `AI_chooseEvent`, ...) ->
    `self.controller(p).<method>(self, ..)` (see `controller.rs`; add a method with a
    sensible default if one is missing);
  * AI bookkeeping state read/written by rules (memory counts, war plans, war success,
    contact counters, unit AI type, group flag, mission AI) -> plain fields (e.g.
    `Unit::ai_type`, `Team::ai_war_plan`, `Player::ai_memory_count`); keep C++ semantics;
  * citizen assignment (`AI_assignWorkingPlots`, `AI_addBestCitizen`, `AI_removeWorstCitizen`)
    -> `self.city_assign_working_plots(c)` etc. in `governor.rs` (new code);
  * other AI (`AI_*Value`, `AI_update`, `AI_doTurn`, auto-cast...) -> omit, leaving a
    `// AI: <call>` comment where behaviour depended on it.
- Python callbacks (`CvEventReporter::*`, `gDLL->getPythonIFace()->callFunction(..)`,
  `PyResult`/`PyRequirement` spells, CvGameUtils hooks) -> functions in `crate::ffh::*`
  (ports of the FFH2 Python). Until ported, call a stub there rather than dropping the call.
- UI/graphics/sound/network (`gDLL->getInterfaceIFace()`, entities, flags, popups for
  humans) -> omit. Messages may go to `self.log.msg(turn, Some(p), || text)`.
- `GC.getGameINLINE().getActivePlayer()` -> `self.active_player` (always `None` headless, so
  "active player" UI branches never run; combat is never shown, i.e. always quick);
  `gDLL->GetAutorun()` -> `self.autorun`.
- `CvPlayerAI::AI_getAttitude(ePlayer, bForced = true)` read by rules ->
  `self.player_ai_get_attitude(p, other, forced)` (forced overrides, then the controller).
- `CvPlayerAI::AI_foundValue` (needed by starting plots and barbarian cities) ->
  `self.player_ai_found_value(p, x, y, min_rival_range, starting_loc, &[])` (`found_value.rs`).

Saved games (`src/save/`, see docs/ARCHITECTURE.md): every state struct reachable from `Game`
is saved by destructuring it exhaustively (`persist_struct!(Unit { id, group, .. every field })`
in `save/state.rs`; `Game` in `save/mod.rs`; `PathFinder` in `save/path_finder.rs`), so adding a
field to `Game`, `Unit`, `City`, `Player`, `Team`, `Plot`, `Area`, `Map`, ... breaks the build until
it is added to its save list (in declaration order) -- then bump `save::FORMAT_VERSION`. A new
field type needs a `Persist` impl (`save/io.rs` covers primitives, `Option`, `Vec`, arrays,
`IdxVec`, `BTreeMap`, tuples, `Slab`, `Cell`/`RefCell`; ids and info types in `save/state.rs`).
State must live in `Game` (not in statics or controllers) to survive save/load.

Every `todo!()` left in core code means "not yet ported"; keep the C++ function name in
the message (`todo!("CvUnit::airStrike")`) so remaining work is greppable.

## Python porting conventions (`ffh-core/src/ffh/`)

FFH2 Python talks to the DLL through the Cy* wrappers (CyUnit, CyCity, CyPlayer, ...),
which forward to the C++ methods -- so port each Cy call to the corresponding ported
`Game` method or field:
- `pUnit.getX()` -> `self.units[u].x`; `pUnit.setHasPromotion(e, True)` ->
  `self.unit_set_has_promotion(u, e, true)`; `pPlayer.initUnit(..)` -> `self.player_init_unit(..)`.
- `gc.getInfoTypeForString('PROMOTION_X')` -> `self.d.t::<PromotionType>("PROMOTION_X")`
  (hoist out of loops). `gc.getDefineINT('X')` -> `self.d.defines.x`.
- `CyMap().plot(x, y)` -> `self.map.plot_xy(x, y)`; it returns `None` off-map, and the
  Python NULL-plot then answers defaults (`isNone()` true, counts 0) -- port that explicitly.
- `CyGame().getSorenRandNum(n, "s")` -> `self.soren_rand_num(n, "s")` (same order/count).
- Python 2 integer `/` floors: use `crate::ffh::game_utils::py_div`; `%` on negative values
  follows Python (`rem_euclid` for positive divisors). `range(a, b)` -> `a..b`.
- `PyPlayer(i).getUnitList()` / `getCityList()` -> clone the id lists from `self.players[p]`.
- UI-only calls (`CyInterface().addMessage`, popups, sounds, `CyEngine` effects) -> omit
  (or `self.log.msg`). Human-only popup choices -> controller decision for that player.
- Callback names stored in XML are dispatched through tables built once from `PyCall`
  (`ffh/pycall.rs`); every name referenced by XML must resolve (a test checks this).

## Workflow

- Work in small, compiling steps; commit with clear messages.
- Update docs/PORTING.md when a C++/Python unit is ported (file, functions, status).
- Tests: unit tests next to code; data spot-checks in `crates/ffh-data/tests/`.
- Tests that rely on a seed's story (`crates/ffh-play/tests/*`: a bot survives, a city is
  captured) break when the AI changes; make them robust to that (their comments say how, e.g.
  re-pick a seed with `ffh-play bot --seed S`) rather than asserting one game's outcome.

## AI changes

- **Start from `docs/AI_LOG.md`**, the log of AI changes that did not move the win rate, for the
  area you are about to change: each entry says why the idea made sense, why it failed in the
  games, and what that implies about the right version (or about the game's balance) -- both a
  warning and a source of better ideas.
- Measure on development seeds, then validate on seeds never used to choose anything, the change
  on against the same binary with it off (`civ_ab.py --off-base` / `FFH_AI_CIV_OFF` for a
  civilization's logic; docs/AI.md, "Tools"). Keep a change only if neither the win rate nor the
  share at the end falls; development-seed gains alone are not results.
- **Civilizations use their unique mechanics** (the owner's decision, 2026-10-20): a
  civilization's own units, spells and abilities are part of how it plays, so its AI uses them
  whether or not they directly benefit it. The win-rate rule above does not decide whether a
  civilization uses its own mechanics; measure what they cost and improve how they are used.
- **Every measured change that fails gets an entry in `docs/AI_LOG.md` before it is reverted**:
  (1) why it made sense, (2) why it did not move the win rate, from the games themselves (what
  happened when it fired: counts, checkpoints, `FFH_AI_TRACE` replays), (3) what that implies.
  Leave the reverted code's module comment naming the attempt and the log entry.
- AI timing and AI-only restrictions belong in `ffh-ai`, not in the rules. The world spells'
  AI-only Python gates were moved there (docs/PORTING.md, "Changes from FFH2"); the rules' other
  AI-only branches are listed in docs/AI_ONLY_RULES.md, each group moved by its own decision.

## Verifying renderer and visual work (r3d, lab, client look)

This is a game. The owner looks at the result and reports regressions, and a regression costs a
follow-up fix, not harm. Verify in proportion: fast iteration beats exhaustive pre-checking. These
rules hold even when ultracode is on: use workflows to fan out *work* (independent areas, design
alternatives, research), not to multiply verification. Rules-engine work keeps the hard rules above.

Tiers (pick by the change):
- **Small** (a constant, a palette, a doc, a one-function fix): self-check only. One cropped render
  of what changed, or none if it isn't visual. No review agent.
- **Medium** (a new effect, a reworked area): self-check, then at most one reviewer agent working
  from the code and the renders already made (at most 2 small new frames). No refute round: judge
  its findings yourself (would the author's one-line reply win? then drop it).
- **Large** (a new look across the map, a pipeline change): one review round with 2-3 critics in
  parallel; a refute pass only for high-severity findings that would change direction. At most one
  round per batch.

Rendering (SwiftShader here: 10-30 s a frame on 4 shared cores; workflows run 2 agents at a time):
- Iterate on small renders (`--w 512 --h 320`) or crops of the changed region. Full 1024x640 sets
  and a real-game capture (`tools/r3d-compare.mjs`) only for a batch's final check.
- Review agents read existing renders and render only if they truly must.
- Never wait idle on a render, perf run or review: run it in the background and work on the next
  independent item meanwhile.

Batching:
- Group related items: one verification round and one final render set per batch, not per item.
- `tools/r3d-perf.mjs` and `tools/r3d-lab.mjs --stability` only at milestones (a batch that changes
  shader cost or anything world-anchored), as one A/B against the previous milestone: same save,
  same binary, the old tree via `git archive`. Update docs/R3D.md section 11 then.

Every commit still needs the hard-rule gate (`tools/agents/build-tree.sh --test`, `ffh data` with 0
warnings) plus `bash tools/web-check.sh`. Run `node tools/web-smoke.mjs --bin <built>/ffh-play`
before pushing to master. Regressions the owner reports get fixed in the next batch; don't try to
pre-empt every conceivable one.

A change to the transport, the web host or `crates/ffh-web` also runs the static site's smoke test
before it goes to master (`tools/web-site.sh OUT`, then `node tools/web-site-smoke.mjs --site OUT`;
docs/WEB.md): master is deployed to the public site by CI.

When a batch passes the gate, fast-forward master and push it without asking.
