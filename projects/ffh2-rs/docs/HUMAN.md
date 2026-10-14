# Playing ffh2-rs as a human (`ffh-play`)

`crates/ffh-play` is the human-play front-end: a session core that lets a person (in a browser)
or a program (JSONL over stdio) play one slot while the AI plays the others. The design is
`docs/PLAY_DESIGN.md`; this file says what you can run today and how to use it.

## Status

Phase **P0** (skeleton) is done: the crate, its JSON layer and type-name resolution. There is
no game session, browser or agent mode yet (phases P1-P5; see the status table at the top of
`docs/PLAY_DESIGN.md`).

## Commands

```
cargo build --release
./target/release/ffh-play kinds                       # every named kind, its source and count
./target/release/ffh-play names UnitType              # JSON array of a kind's type names (index order)
./target/release/ffh-play names --pretty              # {kind: [names]} for all kinds
./target/release/ffh-play lookup UNIT_WARRIOR UnitType:3 ORDER_TRAIN
```

`lookup` prints one JSON line per reference (`kind`, `index`, `name`, `display`, `text_key`, or
`error`) and exits with code 1 if any reference does not resolve. A kind is named by its Rust type
(`UnitType`) or its table (`units`).

## Protocol conventions implemented so far

These hold for every request and reply of the protocol (design §4.1):

* **Type references** are accepted as the XML/C++ type name (`"UNIT_WARRIOR"`, `"ORDER_TRAIN"`,
  `"MISSION_FORTIFY"`) or as the integer index; replies use indices. Where a type is optional,
  `null` or `-1` means none. A name of the wrong kind is refused (`unknown UnitType "TECH_X"`).
* **Kinds** with names: all 61 info tables, the 6 GlobalTypes lists without a table, and 15
  hard-coded C++ enums (activities, orders, tasks, player options, tradeable items, ...). Vote
  choices take `PLAYER_VOTE_YES` / `YES`, `NO`, `NEVER`, `ABSTAIN`, or an integer.
* **Ids** (units, cities, groups, triggered events, votes) are plain unsigned integers; players
  and teams are 0-255.
* **JSON** is strict: no comments or trailing commas, no duplicate keys, nesting at most 128.
  Errors name the field: ``field `units`: expected an array of (a unit id (...)), got "x"``.
* **Display names** come from the game text; text FfH2 inherits from BtS is not vendored, so
  those names are derived from the type name (`UNIT_WARRIOR` -> "Warrior").

## Known gaps

Everything in `docs/PLAY_DESIGN.md` §7 from P1 on: session, decisions, commands, saves and
journal (P1); the AI-as-human soak (P2); views and queries (P3); the HTTP and JSONL transports
(P4); the browser (P5-P6).
