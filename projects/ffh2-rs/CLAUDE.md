# ffh2-rs: Fall from Heaven 2 (0.41) ported to Rust

Rules-faithful port of the FFH2 0.41 Civ4 mod (base mod only). The rules engine
(DLL C++ + rules Python) is ported function by function; the AI is new.

## Layout

```
data/xml/        vendored gameplay XML (FFH2 0.41 + stock BTS files FFH2 inherits)
data/text/en.tsv English text (TXT_KEY -> string)
crates/ffh-data  XML -> typed info tables (port of CvInfos.cpp / CvXMLLoadUtility)
crates/ffh-core  rules engine (port of CvGame/CvPlayer/CvTeam/CvCity/CvUnit/CvPlot/... + rules Python)
crates/ffh-ai    AI players (new design)
crates/ffh-cli   `ffh` binary: autoplay, benchmarks, inspection
tools/           fetch-reference.sh, vendor_xml.py, gen_enums.py, gen_defines.py
docs/            ARCHITECTURE.md, DATA.md, PORTING.md (status)
```

## Reference source (read-only, NOT in this repo)

`tools/fetch-reference.sh` clones the pinned sources to `/home/user/ref`:

- `/home/user/ref/ffh2/CvGameCoreDLL/*.cpp|h` -- FFH2 0.41 DLL source (the spec)
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
- **Unspecified evaluation order** (two RNG draws in one C++ expression): follow what the
  original DLL compiler (MSVC 2003, x86) does -- function-call *arguments* are evaluated
  right to left (`f(rand(a), rand(b))` draws `b` first); operands of binary operators left
  to right (`rand(a) + rand(b)` draws `a` first). Comment `// C++ quirk: MSVC evaluation order`.
- **Faithfulness first.** Port C++ semantics exactly, including quirks; if a quirk
  is an obvious bug that changes no gameplay, keep it and add a `// C++ quirk:` note.
  Never "improve" rules silently. Deliberate deviations go in docs/PORTING.md.
- Skip pure UI/graphics/sound/network code (messages may go to the game log).
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
  field per define, lowercase name). Text defines are resolved type indices (i32,
  -1 = none) -- convert with `opt_from_i32`.
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
- `CvPlayerAI::AI_foundValue` (needed by starting plots and barbarian cities) ->
  `self.player_ai_found_value(p, x, y, min_rival_range, starting_loc, &[])` (`found_value.rs`).

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
