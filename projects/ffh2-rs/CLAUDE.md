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
Replicate C++ defaults exactly (e.g. `iMaxGlobalInstances` defaults to -1).

## Workflow

- Work in small, compiling steps; commit with clear messages.
- Update docs/PORTING.md when a C++/Python unit is ported (file, functions, status).
- Tests: unit tests next to code; data spot-checks in `crates/ffh-data/tests/`.
