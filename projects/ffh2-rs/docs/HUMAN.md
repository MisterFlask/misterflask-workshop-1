# Playing ffh2-rs as a human (`ffh-play`)

`crates/ffh-play` is the human-play front-end: a session core that lets a person (in a browser)
or a program (JSONL over stdio) play one slot while the AI plays the others. The design is
`docs/PLAY_DESIGN.md`; this file says what you can run today and how to use it.

## Status

Phases **P0** (skeleton: JSON, type names) and **P1** (session core, in-process API) are done.
A program linked with the `ffh_play` library can play a whole game; there is no browser or
JSONL agent yet (P4-P5) and no views or queries beyond `Session::pending` (P3). See the status
table at the top of `docs/PLAY_DESIGN.md`.

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
```

* `lookup` prints one JSON line per reference (`kind`, `index`, `name`, `display`, `text_key`, or
  `error`) and exits with code 1 if any reference does not resolve. A kind is named by its Rust
  type (`UnitType`) or its table (`units`).
* `bot` plays the human slot with the scripted test bot (design §8 T1) and prints one line per
  turn and the bot's and session's counters. With `--saves DIR` it writes the journal and the
  autosaves there.
* `replay` re-plays a journal from its first line, or from an autosave of the same game, and
  stops with exit code 1 at the first difference (a stamp, a checkpoint checksum, a decision of
  another kind). Use it to reproduce a bug: send the journal and the last autosave.
* `journal print` shows a journal with type names.

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
* `read(|g| ..)` reads the game under a `QueryGuard` (no side effects on the saved state).
* `save(name)` / `save_bytes()` work at a control point or turn boundary; `Session::load`,
  `Session::take_over` (an AI save, debugging) resume.

### Commands

`move`, `mission`, `do_command`, `join_group`, `auto_mission`, `push_order`, `pop_order`,
`do_task`, `research`, `update_civics`, `percent_change`, `convert`, `declare_war`, `event`,
`vote`, `set_option`, `auto_moves`, `end_turn`, `autoplay` (`propose_deal` comes with P3). Each
is the port of what Civ4 sends for that input; design §4.5 and the "As built" notes of P1 in
§7 list the rules calls and checks. Their JSON form is the protocol request (`{"c": "move",
"units": [..], "x": .., "y": ..}`), with type names or indices.

### Decisions and popups

Civ4 answers some rules questions with popups for a human. `ffh-play` keeps the C++ timing:

* **Deferred** (answered at your next control point, like an unanswered C++ popup): choosing
  production for an idle city, events, diplomatic votes, the research choice. They are listed by
  `pending()` and answered with `push_order`, `event`, `vote`, `research`. Ending the turn with
  any open needs `force`; then open events are dropped, votes stay (they abstain after Civ4's
  extra turn) and empty research takes Civ4's automatic pick after turn 4.
* **Blocking** (answered during the rules call that asks, through the session's `Answerer`):
  a free tech, keeping or abandoning an acquired city, the secretary general's election, a deal
  offered to you, founding a religion. Each offers the C++ popup's options and the advisor's pick.

The advisor is a private `ffh_ai::Ai`; with `autoplay` it plays your slot for N turns.

### Saves, autosaves, journal

With `SessionOptions::saves_dir = DIR`: autosaves at every turn boundary in `DIR/auto/` (the
last 20, `T0057.ffhsave` = the end of turn 57) and `DIR/auto/start.ffhsave` at a new game's
first control point, the journal `DIR/journal.jsonl` (one line per command, decision answer and turn
checkpoint), manual saves `DIR/<name>.ffhsave`. A save is two files: `.ffhsave` (the game, any
tool loads it) and `.ffhsess` (session state: your open event popups, the journal position).

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

* No views, queries, turn report, deals (P3), transports (P4), browser (P5-P6).
* Automation and FfH AI-controlled units idle (ffh-ai has no `update_automated_group` yet); the
  automate commands are refused, and such groups do not count as ready.
* Civ4 ends your turn by itself when nothing is left to move; here you always end it.
* Human-only popups without a rules hook (Kuriotates settlements, the raze popup's gift button,
  telling disband from raze): see design §5.2 and P7.

## Checklist: a new `Controller` hook

A method added to `ffh_core::Controller` fails `tests/drift.rs` until `human::POLICIES` and
`HumanController` handle it. Decide what Civ4 does for a human there: a popup whose "no answer"
equals not answering yet is **deferred** (record it, answer at the control point with a
command); one where "no answer" loses information is **blocking** (`HumanController::block`
with a `Decision`); AI work Civ4 also runs for humans goes to the advisor.
