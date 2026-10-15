# Playing ffh2-rs as a human (`ffh-play`)

`crates/ffh-play` is the human-play front-end: a session core that lets a person (in a browser)
or a program (JSONL over stdio) play one slot while the AI plays the others. The design is
`docs/PLAY_DESIGN.md`; this file says what you can run today and how to use it.

## Status

Phases **P0** (skeleton: JSON, type names), **P1** (session core, in-process API), **P2**
(the AI-as-human soak, `ffh-play soak`) and **P3** (views and queries, the turn report, deals)
are done. A program linked with the `ffh_play` library can play a whole game through the
protocol's queries and commands (the test bot does); there is no JSONL agent or browser yet
(P4-P5). See the status table at the top of `docs/PLAY_DESIGN.md`.

## Commands

```
cargo build --release
./target/release/ffh-play kinds                       # every named kind, its source and count
./target/release/ffh-play names UnitType              # JSON array of a kind's type names (index order)
./target/release/ffh-play names --pretty              # {kind: [names]} for all kinds
./target/release/ffh-play lookup UNIT_WARRIOR UnitType:3 ORDER_TRAIN
./target/release/ffh-play bot --seed 3 --turns 150 [--opponents 3] [--world WORLDSIZE_SMALL]
                              [--saves DIR] [--quiet] [--verbose]
./target/release/ffh-play replay DIR/journal.jsonl [--from DIR/auto/T0110.ffhsave]
./target/release/ffh-play journal print DIR/journal.jsonl
./target/release/ffh-play soak run --seed 2 --size tiny --civ kuriotates [--options raging] [--turns 300]
                              [--speed quick] [--handicap deity] [--saves DIR] [--resume DIR/auto/T0123.ffhsave]
                              [--check-every 5] [--save-check-every 25]
./target/release/ffh-play soak [--seeds 1..2] [--sizes tiny,small,standard] [--civs ..] [--options default,raging,occ,challenges]
                              [--speeds normal,quick] [--turns 300] [--jobs N] [--timeout SECS] [--saves DIR]
                              [--keep-saves] [--out results.tsv] [--filter TEXT] [--list] [--check-every N] [--save-check-every N]
```

* `lookup` prints one JSON line per reference (`kind`, `index`, `name`, `display`, `text_key`, or
  `error`) and exits with code 1 if any reference does not resolve. A kind is named by its Rust
  type (`UnitType`) or its table (`units`).
* `bot` plays the human slot with the scripted test bot (design §8 T1), which reads the game only
  through the queries below and acts with commands, and prints one line per turn and the bot's
  and session's counters; `--verbose` also prints the `text` query each turn (what an agent
  reads). With `--saves DIR` it writes the journal and the autosaves there.
* `replay` re-plays a journal from its first line (a new game), or `--from` an autosave of the
  same game; a journal that starts with a load or take-over line replays `--from` the save it was
  loaded from (its sidecar next to it) or taken over from. It stops with exit code 1 at the first
  difference (a stamp, a checkpoint checksum, a decision of another kind). Use it to reproduce a
  bug: send the journal and the last autosave (or the save the session was loaded from).
* `journal print` shows a journal with type names.
* `soak run` lets the AI play the human slot through the session for one game (design §7 P2):
  production, research, event popups and votes are answered with the AI's choice through the
  commands (one turn in seven they are left open and the turn is ended with `force`; the
  research the AI's own turn sets meanwhile is cleared again with `research {tech: null}`, so
  Civ4's automatic research pick runs), blocking decisions take the advisor's pick, the AI moves
  the units, and ffh-stress's invariant checks run every 5 turns (`--check-every`), a save round
  trip every 25 (`--save-check-every`; 0 = never). It prints `PROGRESS`, `END`, `CITIES`,
  `STATS` lines, or `PANIC` (with the workspace frames of the backtrace, exit code 101),
  `STUCK` (exit 4) or `ERROR` (a command the session offered was refused, a check failed;
  exit 5), each with a `REPRO` line: the game's own command with its check settings when they
  are not the defaults, plus `--resume <autosave>` when `--saves` kept autosaves (every game is
  deterministic, and resuming an autosave ends exactly as the uninterrupted game). Sizes,
  civilizations, options, speeds and handicaps may be shortened (`tiny`, `mercurian`, `raging`,
  `quick`); numbers out of range are refused. `occ` (one-city challenge, no healing for humans)
  and `challenges` (cut losers, high to low, increasing and flexible difficulty) are option sets
  that act only on human players. `--civ infernal` or `mercurians` (not playable at the start)
  plays an AI-only game whose player 0 founds them at turn 30 (`--spawn-turn`) and takes the new
  player over, with your `--handicap`.
* `soak` runs the matrix (default: 2 seeds × tiny, small, standard × Kuriotates, Infernal,
  Sheaim, Mercurians, Bannor, Calabim × default, raging barbarians, `occ`, `challenges`, 300
  turns, normal speed; `--speeds` adds speeds, a game given twice runs once), each game in a
  child `soak run` with a timeout, `--jobs` at a time; it prints a summary (per civilization,
  size and option set; the human's cities against the AI civs'; every distinct failure with its
  repro commands, which include `--check-every` / `--save-check-every` when given) and exits
  with code 1 if a game failed. Its arguments are checked before any game runs (`--out` is
  created first; a bad setting is a usage error, exit 2). With `--saves DIR` each game keeps
  its last 3 autosaves in `DIR/<game>/` (the description and a hash of the settings) while it
  runs (and after a failure, for the repro). For a checked build (debug assertions and overflow
  checks): `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true
  cargo build --release --target-dir target/checked -p ffh-play`, then run
  `target/checked/release/ffh-play soak ...`. A checked build stops long games on an ffh-core
  debug assertion when the Syliven's Perfect Lyre is taken out of a city and dropped again
  (reported; design §7 P2): `soak run --seed 9 --size tiny --civ bannor --speed quick --turns 400`
  at turn 276. Release builds play on.

## The session (library API)

```rust
use ffh_play::{NewGame, Session, SessionOptions, Command, CmdOk, Stop};
let mut s = Session::new_game(&NewGame::new(ffh_data::global(), 7), answerer, SessionOptions::default())?;
loop {
    match s.advance(&mut |_| {}) {           // runs AI turns until the human has control
        Stop::Human => {}
        Stop::Over | Stop::Stuck(_) => break,
    }
    let pending = s.pending();               // events, votes, research, idle cities, ready groups
    s.execute(&Command::Research { tech: Some(t), shift: false })?;   // a CvNet* port
    s.execute(&Command::EndTurn { force: true })?;
}
```

* `advance` runs `update()` until the human's next **control point** (its turn is active and no
  group of it is busy) or the end of the game. When nothing is ready but units have queued
  orders it sends `auto_moves` once (C++ `updateTestEndTurn`), so multi-turn moves continue.
* `execute` applies one `Command` at a control point. It validates first (ids yours, types in
  range, the rules' own `can*` gates, as the Civ4 interface only offers legal actions); refusals
  (`not_your_turn`, `decision_pending`, `invalid`, `not_owner`, `gone`, `cannot`) change nothing.
  Where Civ4 asks for confirmation the reply is `NeedsConfirm` (declare war to move, delete,
  global or war spells, which transport to load into, ending the turn with things open); resend
  with the flag set.
* `query(&Query)` (or `query_json` of a `{"q": ..}` object) answers a protocol query from your
  side of the fog (below); `set_reveal(true)` switches debug reveal on (the true state, every
  reply marked `"reveal": true`).
* `read(|g| ..)` reads the game under a `QueryGuard` (no side effects on the saved state).
* `save(name)` / `save_bytes()` work at a control point or turn boundary; `Session::load`,
  `Session::take_over` (an AI save, debugging) resume.
* FfH can move the human to another slot (`reassignPlayerAdvanced`: the Impersonate Leader
  spell for its 5-14 turns, the "switch civilizations" event, the High to Low challenge). The
  session follows: `Session::human` is the slot you play now, and the active player moves with
  you as in Civ4. Messages follow you too: when the move happens during the turn processing
  (High to Low at turn 50, the end of the Impersonate Leader timer), the log of that game update
  is read as the new slot's, so the old slot's messages from its turn as an AI reach nobody, as
  in Civ4; its messages from just before the move in that update are lost (design §7 P2).

### Commands

`move`, `mission`, `do_command`, `join_group`, `auto_mission`, `push_order`, `pop_order`,
`do_task`, `research`, `update_civics`, `percent_change`, `convert`, `declare_war`,
`propose_deal`, `event`, `vote`, `set_option`, `auto_moves`, `end_turn`, `autoplay`. Each is
the port of what Civ4 sends for that input; design §4.5 and the "As built" notes of P1 and P3 in
§7 list the rules calls and checks. The `group` query lists the actions of a selection with the
command that performs each, ready to send. Their JSON form is the protocol request (`{"c": "move",
"units": [..], "x": .., "y": ..}`), with type names or indices. Parameters are accepted only in
the form the Civ4 interface sends them: missions without movement `flags` (0) and with the data
their button carries (a plot, a build/religion/specialist/building, else -1), no
`MISSION_MOVE_TO_UNIT`, specialist changes of +1/-1 where the city screen shows the button,
slider steps of ±`COMMERCE_PERCENT_CHANGE_INCREMENTS` (10), `TASK_LIBERATE` with `data1` 0.

### Deals

`propose_deal {"to": 2, "give": [{"item": "TRADE_PEACE_TREATY"}], "get": [{"item":
"TRADE_PEACE_TREATY"}]}` offers a deal to an AI, as the diplomacy screen does. The `trade {with}`
query lists what each side can give (Civ4's trade table: gold and gold per turn up to a maximum,
maps, open borders, pacts, a peace treaty while at war, techs, resources, cities, ...) with
Civ4's refusals. A deal must use those items (gold as an amount), not twice, and put a peace
treaty, open borders, a defensive pact or a permanent alliance on both sides. At war an AI
refuses to talk at all until the war has lasted its leader's refuse-to-talk threshold (5 to 20
turns, doubled for a war it chose, shorter when you are clearly winning; Civ4's
`AI_isWillingToTalk`): `trade` then answers `contact: false`, `why_not: "... refuses to talk"`,
and `propose_deal` `cannot`. As every deal between enemies ends the war (Civ4's `CvDeal`), a deal
at war must carry the peace treaty (both sides) or a capitulation; a gift alone is `invalid`.
The AI answers at once: `accepted: false` changes nothing; `accepted: true` signs it. Today's
AI (ffh-ai's `consider_offer`) accepts gifts, peace after 10 turns of war unless it is clearly
winning, open borders when it likes you, and a fair tech swap, and refuses anything else.

### Queries

Requests are `{"q": "<name>", ..}`; replies are JSON with type indices (`data` names them).
Errors: `invalid`, `not_owner`, `gone`, `busy`.

| `q` | Reply |
|---|---|
| `status` | phase (`human`, `running`, `decision`, `over`), turn, year, you, the active players, the open decision, the winner |
| `pending` | open events (with each option's effects), votes, research needed, idle cities, promotions, ready groups, idle AI-controlled groups |
| `view` | `me` (gold and rate, research, sliders, civics, score, Armageddon counter, mana, Stasis), the players you met, the cities you revealed, the units you see, your groups, `pending`, `log_next` |
| `map` | flat arrays `y * w + x`: `vis` (0 unrevealed, 1 fogged, 2 visible), `plot_type`, `terrain`, `feature`, `bonus`, `improvement`, `route`, `owner`, `river` (bit 0 river on the south edge, bit 1 on the east edge, bits 2-4 north/south flow + 1, bits 5-7 west/east flow + 1), `city`, `yield` (`food \| production << 8 \| commerce << 16`, visible plots only), and `map_rev` |
| `plot {x,y}` | a plot as you know it; while visible also yields, defence, culture and the units |
| `group {units}` | the units in detail and the actions: `mission`, `target` (needs a plot), `command`, `load`, `spell`, `promotion`, `upgrade`, each `enabled` or not, each with its `cmd` |
| `path {units,x,y}` | the path preview: nodes with the turn of arrival and moves left, total turns, a war it would declare, whether it ends in an attack |
| `combat {units,x,y}` | the attack preview: attacker, defender, odds of winning and of retreating (per mille), strengths, the modifiers of both sides |
| `city {city}` | the city screen of one of your cities: plots, specialists, yields, happiness, health, buildings, queue, the buildable list with cost and turns |
| `techs`, `civics`, `religions`, `players` | the tech tree (prerequisites, depth, turns), civics (availability, upkeep, anarchy), religions, the players you met with your deals |
| `trade {with}` | the trade tables both ways (see Deals) |
| `log {since}` | your messages from index `since` (`view.log_next` is the next one) |
| `report` | the turn report: what changed between your end of turn and now (units lost and gained, cities founded, lost, grown, starving, buildings, techs, wars and peace, enemies near your cities, the Armageddon counter, borders), with the messages |
| `text` | a plain-text summary for agents |
| `ascii {x,y,w,h,layer}` | an ASCII map, 2 characters per plot, north up, `::` unrevealed; layers `terrain`, `political`, `yield`, `units`; a legend |
| `inspect {kind,id}` | `{:#?}` of your unit, group, city, plot, player, team, deal, or of an info entry (`UnitType`, `units`, ...) |
| `data` | the static tables (every kind: type, name, texts, the main gameplay fields) |
| `setup`, `saves` | the new-game lists; the saves of the game's directory |

What you see is what Civ4 shows the player: your units, and others only on plots you see and
when they are not invisible to you (hidden-nationality units show as barbarians; the cargo of a
foreign ship is hidden); cities you revealed (size only while you see them); players you met;
bonuses your techs reveal; improvements, roads and borders as you last saw them.

### Decisions and popups

Civ4 answers some rules questions with popups for a human. `ffh-play` keeps the C++ timing:

* **Deferred** (answered at your next control point, like an unanswered C++ popup): choosing
  production for an idle city, events, diplomatic votes, the research choice. They are listed by
  `pending()` and answered with `push_order`, `event`, `vote`, `research`. An event popup is
  checked when it is shown (it is dropped if none of its choices is possible then); once shown
  it stays until you answer it, even while your own actions make its choices impossible for a
  while (C++ event popups are modal). Ending the turn with
  any open needs `force`; then open events are dropped, votes stay (they abstain after Civ4's
  extra turn) and empty research takes Civ4's automatic pick after turn 4.
* **Blocking** (answered during the rules call that asks, through the session's `Answerer`):
  a free tech, keeping or abandoning an acquired city, the secretary general's election, a deal
  offered to you, founding a religion. Each offers the C++ popup's options and the advisor's pick.
  An answerer that implements `ask_with` can run the queries meanwhile (through the
  `DecisionReader`; messages, the report and the saves list are not available then).

The advisor is a private `ffh_ai::Ai`; with `autoplay` it plays your slot for N turns.

### Saves, autosaves, journal

With `SessionOptions::saves_dir = DIR`: autosaves at every turn boundary in `DIR/auto/` (the
last 20, `T0057.ffhsave` = the end of turn 57) and `DIR/auto/start.ffhsave` at a new game's
first control point, the journal `DIR/journal.jsonl` (one line per command, decision answer and turn
checkpoint), manual saves `DIR/<name>.ffhsave`. A save is two files: `.ffhsave` (the game, any
tool loads it) and `.ffhsess` (session state: your open event popups, the journal position, the
latest messages and the turn report).

## Protocol conventions

These hold for every request and reply of the protocol (design §4.1):

* **Type references** are accepted as the XML/C++ type name (`"UNIT_WARRIOR"`, `"ORDER_TRAIN"`,
  `"MISSION_FORTIFY"`) or as the integer index. Replies use indices for every type-valued field,
  hard-coded C++ enums included (an `activity` is `2`, not `"ACTIVITY_SLEEP"`); `ffh-play names`
  (later `/api/data`) maps them back. Where a type is optional, `null` or `-1` means none. A name
  of the wrong kind is refused (`unknown UnitType "TECH_X"`).
* **Kinds** with names: all 61 info tables, the 6 GlobalTypes lists without a table, and 15
  hard-coded C++ enums (activities, orders, tasks, player options, tradeable items, ...).
* **Vote choices** are what the Civ4 vote popup offers: `YES`, `NO`, `NEVER`, `ABSTAIN` (also
  `PLAYER_VOTE_YES`, ..., or the values -2, -3, -5, -4), or, in a team vote (secretary general,
  victory), the **team id** voted for (0-34). `NO_PLAYER_VOTE` and `NO_PLAYER_VOTE_CHECKED`
  (-1, -6) are vote state, not choices, and are refused. The `vote` command also refuses a
  button the popup would not show for that vote: team ids in a yes/no vote, YES/NO/NEVER in a
  team vote, a team that is not eligible, NEVER when you cannot defy the resolution, and, for a
  vassal, anything but its master or its own team.
* **Ids** (units, cities, groups, triggered events, votes) are plain unsigned integers; players
  and teams are 0-255. JSON integers are 64-bit signed; a 64-bit hash is sent as a hex string.
* **JSON** is strict: no comments or trailing commas, no duplicate keys, nesting at most 128.
  Errors name the field: ``field `units`: expected an array of (a unit id (...)), got "x"``.
* **Display names** are the whole description text, with Civ4 markup such as `[ICON_TRADE]` kept
  (`VOTE_LIBERTY` -> "Global Civic: Liberty"). Text FfH2 inherits from BtS is not vendored, so
  those names are derived from the type name (`UNIT_WARRIOR` -> "Warrior"), as are descriptions
  with an unfilled placeholder (`MEMORY_SOMNIUM_POSITIVE` -> "Somnium Positive").

## Known gaps

* No transports (P4: `ffh-play agent` and `serve`), no browser (P5-P6).
* AI players never offer you deals or ask anything (ffh-ai); you propose. The trade screen shows
  an AI's whole gold and gold rate as what it can give (Civ4's AI caps it; that AI code is not
  ported), and Civ4's AI refusals ("we don't want to trade that") are mostly not ported either.
* A path preview for part of a stack uses the whole stack's speed (`approximate: true`).
* Fogged plots show no yield, defence or culture (the rules engine computes them from the real
  improvement, not the one you last saw).
* Automation and FfH AI-controlled units idle (ffh-ai has no `update_automated_group` yet); the
  automate commands are refused, and such groups do not count as ready.
* Civ4 ends your turn by itself when nothing is left to move; here you always end it.
* Messages Civ4 sends only to some players (wars between others, captured or razed cities) are
  recognized by their text and shown under Civ4's condition (you met both teams, you revealed
  the city); event world news are not shown yet, as their sender is not recorded. When FfH moves
  you to another slot during the turn processing, the old slot's messages from just before the
  move are lost (the log has no marker at the move yet, design P7).
* A checked build (debug assertions) panics in ffh-core when the Syliven's Perfect Lyre is taken
  out of a city (a human-only spell) and dropped again: the wonder is counted as built once more
  each time (as in C++, which only asserts). Reported to ffh-core's owners; release builds play
  on.
* Human-only popups without a rules hook (Kuriotates settlements, the raze popup's gift button,
  telling disband from raze, taking control of the Infernals or Mercurians you summoned): see
  design §5.2 and P7. To play the Infernals or Mercurians, take over their slot from an AI save.

## Checklist: a new `Controller` hook

A method added to `ffh_core::Controller` fails `tests/drift.rs` until `human::POLICIES` and
`HumanController` handle it. Decide what Civ4 does for a human there: a popup whose "no answer"
equals not answering yet is **deferred** (record it, answer at the control point with a
command); one where "no answer" loses information is **blocking** (`HumanController::block`
with a `Decision`); AI work Civ4 also runs for humans goes to the advisor.
