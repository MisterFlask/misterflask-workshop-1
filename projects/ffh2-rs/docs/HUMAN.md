# Playing ffh2-rs as a human (`ffh-play`)

`crates/ffh-play` is the human-play front-end: a session core that lets a person (in a browser)
or a program (JSONL over stdio) play one slot while the AI plays the others. The design is
`docs/PLAY_DESIGN.md`; this file says what you can run today and how to use it.

## Status

Phases **P0** (skeleton: JSON, type names), **P1** (session core, in-process API), **P2**
(the AI-as-human soak, `ffh-play soak`), **P3** (views and queries, the turn report, deals),
**P4** (transports), **P5** (the browser client's MVP) and **P6** (the browser client complete:
city screen, civics, religion, sliders, diplomacy and trade, tech tree, Civ4's hotkeys and unit
cycle, map layers, the testing tools, the spectator mode, map deltas; `ffh-play bench`) are done.
You can play in a browser (`ffh-play serve`, below); a script or an LLM can play a whole game
headless with `ffh-play agent` (JSON lines over stdin/stdout). What is left (P7) needs other
crates' owners: see "Known gaps" and the status table at the top of `docs/PLAY_DESIGN.md`.

## Playing in the browser

```
cargo build --release
./target/release/ffh-play serve            # then open http://127.0.0.1:8642/
```

The page starts with the new-game screen (civilization and leader or random, opponents, world
size, speed, difficulty, era, climate, sea level, seed, game options, victory conditions; "Load a
game…" lists the saves). Each game gets a directory in `./saves` (`--saves DIR` to change it).
`serve` takes the GAME flags too (`--seed 3 --world WORLDSIZE_SMALL --new` starts a game at once,
`--load saves/game-3/auto/T0040.ffhsave` resumes one). Use a current Chrome, Firefox or Safari;
nothing is fetched from the internet.

**The look.** The client draws with the game's own pixel art (docs/ART.md, "cheerful
readability", embedded in the binary like the rest of the page): a bright daylight map in the
spirit of the old Shining Force games, figures standing on team-coloured rings, and an interface of
navy panels with gold rules, gilt nails and maroon title ribbons, the Vigil
bitmap capitals for titles and numbers, the Vigil Book face for buttons and short labels (long
texts, such as events and help, stay in the system serif), the game's yield, commerce, mana and
Armageddon icons (and in the screens those of promotions, unit classes, buildings, specialists,
religions and civic categories), and an illuminated initial on events, votes and decisions. Until the art has
loaded (a moment after the page) the page shows the plain look of P5, and keeps it if the art
cannot load.

**The screen.** The top bar: the turn crescent (amber on your turn), turn and year, what is
happening (your turn; "Moving: …" with the players and the seconds while the AIs play); gold and
rate, research (click it to choose) with its progress bar and turns, the commerce split by its
icons (click it for the sliders), the Armageddon eclipse (its corona reddens at 50, 75 and 100)
with the counter, mana by sphere, anarchy, golden age, Stasis, idle AI-controlled groups, score;
below them the menu (Research, Techs, Cities, Civics, Religion, Diplomacy, Tools, Save, Load,
New, keys `?`, a raw request box `⌘` for testing) and End turn. The map fills the middle (minimap
bottom right, zoom and the map layers bottom left), the selection panel is on the right, and the
bottom panel has the turn report, the messages by turn and the game's files (directory, journal,
last autosave).

**The map** is north up and wraps east-west like the game. The ground is drawn flat (colour
fields with sparse marks, inked coasts, rivers and roads); hills, peaks, forests, improvements,
landmarks and cities stand upright; resources (only those your techs reveal) are seals at a
plot's bottom right; borders are stitched ribbons in the owner's colours. Each civilization has a
tincture and a sigil (docs/ART.md 7: two civilizations of one tincture get a divided field), on
its borders, city banners and unit standards. Fogged plots are darkened and hatched and show
what you last saw (dark windows, no foreign units), unexplored ones are black with ragged edges,
and the view's corners fade. **Cities** are drawn in their civilization's style and by size
(five tiers), in a pool of their light; the capital's spire carries a finial in the owner's
colour, walls (Walls or a palisade) show as a curtain wall, a city in disorder goes dark under a
smoke column; the plaque under it has the size, the name and, for yours, the turns of what it
builds. A **unit stack** shows its head unit (the selected one, else your unit that can move,
else the strongest), up to two shadows behind it for more units, and its owner's standard at the
top left (top right in a city): bright for your stacks that can move or are selected, quiet for
others, with a vellum pip when it can move and a gilt one with a hero in the stack; the count
hangs under the standard, a health bar shows when the head unit is hurt (or selected), and a
mark at the top right says fortified, asleep, sentry, healing, automated or promotion ready.
Units face east, and west after a move west. Zoom (`+` `-`, the wheel) goes by whole art pixels:
32, 64 (the default), 96 and 128 screen pixels a plot, and two chart steps, 16 and 8 (flat
colours, owners' washes, city dots, units as small squares). Hovering a plot shows a tooltip
(terrain, resource, improvement, owner, yields and defence while you see it, the city and units
there; with the art, the yields, defence and move cost by their icons), and with units selected
the path: dots from plot to plot that march, a roundel with the
turn of arrival at each turn's end, the part beyond this turn's moves dimmer, ochre for a route
to build; an attack ends in crimson brackets with the odds of winning on a chip, and the tooltip
gives the odds of winning and retreating with both strengths and their modifiers. The layers
(buttons bottom left, `Ctrl+1/2/3`): **Terrain**; **Political** (each owner's land washed with
its colour); **Yields** (the ground dimmed, and the food, production and commerce of the plots in
sight with their icons; fogged plots show none). The minimap is a pixel a plot in the same
colours, owned land in its owner's colour, cities as light dots.

The art is drawn in the background (a worker) a few plots at a time, the part in view first: a
part of the map you have not looked at yet shows flat colours for a moment. **The flat
renderer** of P5 (colours, glyphs and labels: lettered dots for resources, boxes for
improvements, units and cities) is kept for debugging: open the page as
`http://127.0.0.1:8642/?renderer=flat`, or choose it under Tools → Options → "This browser" (which
also offers the isometric 2.5D map in HD pixel art, docs/ISO.md, and, experimental and off unless chosen, **a fully 3D painterly
map**: `?renderer=3d`, docs/R3D.md; it needs WebGL2). The choice is remembered by this browser.

**Motion.** The interface moves quietly, in keeping with the manuscript look (docs/ART.md 4.5):

- **The camera glides.** When the view jumps to a plot it is not already near — centring on the
  selection (`C`), the next waiting group (`W`, `Enter`), a city (`Home` / `End`, `F10`, the cities
  list), a message's "Show on the map" — it glides there across the page the short way round a
  wrapping map, rather than cutting. Dragging, zooming and the minimap stay immediate (you are
  moving the view yourself), and a new game opens on its start with no glide.
- **Units slide.** A unit that moves glides from its old tile to the new one (about 8 tiles a
  second), for your own moves within your turn; a long or multi-tile hop, and every unit on turn
  rollover, jumps.
- **Combat flashes.** When a fight resolves, the struck tile flashes a brief muted crimson — a
  little brighter and rising as a unit is lost.
- **The numbers ease.** Gold, score and the research bar count to their new value rather than
  snapping.
- **The turn breathes.** While the other players move, the turn crescent and its dot breathe
  slowly; when control returns to you the crescent warms for a moment.
- **Dialogs and toasts ease in.**

All of this is off under the system's "reduce motion" setting (`prefers-reduced-motion`): the view
and units jump, no flash or breathe plays, the numbers snap, and the dialogs appear at once.

**Mouse and keys** (also under `?`). The keys are Civ4's: the orders' keys come from the game's
XML (each button shows its key), and the global keys are Civ4's controls
(`CIV4ControlInfos.xml`, what `CvGame::doControl` does with them), except the few marked
"ffh-play's own" below.

| | |
|---|---|
| left click | select your units on a plot (again: the next group there; Shift: all of them), open your city, or show a plot |
| right click | move or attack with the selection; with Shift held, each right click adds a waypoint (numbered on the map), and releasing Shift sends them (`Esc` drops them) |
| drag, wheel, arrows, `+` `-` | pan and zoom; click or drag the minimap to jump |
| double click | open your city |
| `Enter` | the next group waiting for orders, as `W`; when none waits (Civ4's "end of turn" state: the End turn button glows), end the turn |
| `Shift+Enter` | end the turn whatever waits, without the checks |
| `W` | the next group waiting for orders, in Civ4's group cycle (a stack in which a unit has used its moves is skipped, as in Civ4); when it comes round, units with orders left move (Civ4's automatic moves) |
| `Space` | with a unit selected: skip its turn; with none, end the turn when no group waits |
| `C` | centre on the selection |
| `Home` / `End` (`Num+` / `Num-`) | the next / previous of your cities (the first: the one nearest the centre of the map); in the city screen, its next / previous city |
| `F10` | your capital's city screen |
| `Tab` / `Shift+Tab` | ffh-play's own: the next / previous group waiting (in Civ4, `Tab` is chat) |
| `F` / `S` / `H` | fortify (sleep for a unit that cannot fortify) / sentry / heal |
| `B` | found a city (bombard or a ranged attack for units that cannot) |
| `G` then click, `Alt+R` then click | go to a plot (Shift-clicks add waypoints, sent when Shift is released, as with right clicks) / build a route to it |
| `R`, `I`, `M`, … | a worker's builds: road, farm, mine, … (`Shift+W` windmill, `Alt+C` chop) |
| `L` / `U` / `Shift+U` | load / unload / unload all |
| `Shift+P` | pillage |
| `Del` / `Backspace` | delete the selected units (asks first) / cancel their orders |
| `F1` `F2` `F3` `F4` `F6` `F7` | cities, finances (the sliders), civics, diplomacy, the tech tree, religion |
| `Ctrl+S` / `Ctrl+L` | save / load |
| `Ctrl+1` `Ctrl+2` `Ctrl+3`, `?` | ffh-play's own: the terrain, political and yield layers; the keys list |
| `Esc` | leave a plot pick, close a dialog, clear the selection |

**Units.** The selection panel lists your units on the plot (click a row: its group;
Ctrl-click: add or remove that unit), "All", "Group" (one group), "Split" (each unit its own
group), "Next", the group's orders, and the actions the server says the selection can take now
(greyed when not now): orders such as skip, sleep, fortify, sentry, heal, pillage, found, join,
spread, the great-person missions; go-to and route-to and the other modes that need a plot (click
it then; the valid targets are outlined); builds with their turns; spells with their help;
promotions; upgrades with their price; load, unload, wake, cancel, delete. The sections follow
Civ4's button order (Orders, Build, Great person, Religion, Cargo, Spells, Promotions,
Upgrades). "Promote…" lists the promotions with their help, "details" by Spells lists every
spell with its help and whether it can be cast now. Units in a transport are listed under it.
With the art, the panel's head shows the lead unit's figure in its owner's colours, each row its
combat class, strength, health bar, moves and level, the lead unit's promotions are seals under
the list (and each promotion button carries its seal).
When a group is done, the next one waiting for orders on the plot it left, then on its plot,
then the next in Civ4's group cycle is selected, as in Civ4 (not with the player option "No
unit cycling", Tools → Options). As in Civ4, the cycle (`W`, `Enter`, the next group after an
order) selects only groups whose units can all still move: a stack in which one unit has used its
moves still counts as waiting (the end-turn check counts it) but is not selected for you. Moving into a rival's land asks whether to declare war;
deleting, world spells and spells that start a war ask first; loading with more than one
transport asks which.

**Each turn.** When your turn starts the turn report fills the bottom panel (units and cities
gained or lost, growth, starvation, disorder, buildings, techs, contacts, wars and peace,
enemies near your cities, the Armageddon counter, borders, and the messages), the first unit
waiting for orders is selected, and the open questions come up one at a time in Civ4's order:
events (each choice with its effects; greyed when not possible now), votes, research (click:
research it; Shift-click: add it to the queue), then each city with nothing to build (click:
build it now; Shift-click: add to the queue; Ctrl-click: put it first). "Later" sets one aside
for this turn. End turn (the button, or `Enter` once no group waits) asks first when an event, a vote or the research is
open or units wait for orders: answer them, select a unit, or end the turn anyway (Civ4's
consequences: open events are dropped, votes get one more turn and then abstain, empty research
takes the automatic pick). A question the rules must have answered at once (keep or raze a
captured city, a free tech, an election) appears as a dialog that cannot be closed, with the
choices, the advisor's pick marked, and "AI default".

**Cities.** Double-click your city, use the button in the selection panel when your units
stand in it, or the Cities list (`F1`). The city screen: the plots of the city's radius with
their yields (worked ones green, the city's own plot gold, plots another city works dashed;
click a plot to work it or to stop working it, which turns the citizen into a specialist, as in
Civ4, and stops the automatic citizens), the specialists with `−`/`+` (greyed when the rules
would refuse), "Automate citizens", food and growth, production, commerce, happiness and health
with their parts, culture, great people, maintenance, buildings, religions and the garrison;
hurry (gold or population, with the price), draft, the emphasis flags and "Automate
production"; the queue (✕ removes an entry while there are several; the only one is replaced by
choosing another) and everything it can build with turns and cost (click: build it; Shift:
append; Ctrl: put it first). "‹ Previous city" and "Next city ›" go through your cities.

**Empire.** **Techs** (`F6`): the tech tree by prerequisite depth, what you know, research and
can research now, with costs, turns and progress; click a tech to research it (with what it
needs first), Shift-click to queue it. **Civics** (`F3`): each civic option with its civics (the
current ones marked, the ones you cannot adopt greyed); pick civics and the screen shows the
revolution's anarchy and upkeep (computed by the rules), then "Revolution!". **Religion** (`F7`):
the religions, their holy cities as far as you know them, your cities with each, and "Convert"
(with the anarchy). **Finances** (`F2`, or click the commerce split in the top bar): each
commerce's share with `−`/`+` by Civ4's step (only the ones you may change).
**Diplomacy** (`F4`): every player you met (war or peace, open borders, pacts, vassals, attitude,
score, your deals with them) with "Trade…" and "Declare war" (it asks first). The trade screen
lists what each side can give (refusals greyed with Civ4's reason); click items into the deal
(gold with an amount; peace, open borders and pacts go both ways, so they are greyed on both
sides when either side refuses them), "Propose": the AI accepts or declines at once.

**Tools** (the Tools menu): **Debug reveal** (the whole map and every unit and city; the top
bar says DEBUG REVEAL); the **Inspector** (the rules engine's own data of your unit, group, city,
player or team, and of anything under debug reveal, plots too; an info entry by kind and index);
**AI autoplay** (the AI plays your civilization for N turns, then you have control again; while
it runs the top bar counts the turns left and "Stop" ends it: you have control at your next turn);
**Spectate** (below); **Take over…** (play another slot of an AI-only save); **Options** (Civ4's
player options); the raw request box.

**Spectating.** Tools → Spectate (or "Spectate" in the new-game screen) lets the AI play every
civilization, yours too, one turn at a time with the whole map shown: the top bar has Pause,
Step (one turn while paused) and Stop (debug reveal goes back off and you have control). No
popup asks you anything meanwhile; the AI answers them.

**Saving.** Save asks for a name and writes it to the game's directory; the game also autosaves
at every turn boundary (`auto/T0057` is the end of turn 57; the last 20 are kept). Load lists
every game directory with its saves (yours first, then the autosaves, newest first).

**When something breaks.** If the rules crash, the page shows the message and a bug report
(the last autosave, the journal and the `ffh-play replay` command that reproduces it; Copy puts
it on the clipboard) and offers to load the autosave: the server stays up. If a turn does not
finish (the watchdog), load an autosave. If the page loses the server it says so and retries;
a reload is always safe, also in the middle of a decision.

**For testers.** `ffh-play serve --assets crates/ffh-play/assets` serves the client from disk,
so an edit shows on reload; the art then comes from `crates/ffh-play/web/art` too (the
directory beside the assets: its modules, and the sprite bundle made on each request), so an
edit to a sprite file shows on reload as well. `window.ffh.mapView.art.stats` has the last
frame's times by phase and the chunks drawn. `ffh-play stage capture|attack|automoves|load|trade DIR` writes a save at a
staged situation into `DIR` (put it under the saves directory, e.g. `saves/staged`, then load
`staged/capture`): `capture`, a warrior next to an undefended enemy city (moving in raises the
keep/raze dialog); `attack`, a warrior next to a barbarian warrior (the odds tooltip, an attack);
`automoves`, one unit waiting while another has a long move queued (skip the first: the second
moves on, Civ4's AutoMoves); `load`, a warrior and two triremes in a coastal city (Load asks
which trireme); `trade`, a civilization met at peace with gold trading and 100 gold on each side
(Diplomacy, Trade…: a gift of gold is accepted). It writes only `DIR/KIND.ffhsave` and its sidecar (no journal, no autosaves), so
`DIR` may be a game's directory. See "Testing the browser client" below for the checklist.

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
./target/release/ffh-play agent [GAME] [--on-eof quit|end-turn] [--progress]
./target/release/ffh-play serve [GAME] [--port 8642] [--bind ADDR] [--assets DIR]
./target/release/ffh-play stage capture|attack|automoves|load|trade DIR
./target/release/ffh-play bench [--quick] [--seed N] [--turns T] [--runs R] [--world W] [--opponents N] [--ffh PATH] [--json]
    GAME: [--saves DIR | --no-saves] [--autosaves N] [--setup JSON] [--seed N] [--opponents N] [--world W]
          [--civ C] [--leader L] [--handicap H] [--speed S] [--new] [--no-game] [--load SAVE]
          [--take-over SAVE --player P] [--panic-at T]
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
* `agent` plays one game over JSON lines: each line you write to its stdin is a request, each
  line it prints is a reply or an event (see "Transports" below). It starts a new game (the
  defaults, or `--setup '{"seed":3,"world":"WORLDSIZE_DUEL","opponents":1}'`, or the flags
  `--seed`, `--world`, ... which set the same members) unless `--no-game`, `--load SAVE` or
  `--take-over SAVE --player P`. Example: `printf '%s\n' '{"k":"q","q":"text"}' | ffh-play agent --seed 3`.
* `serve` starts the HTTP server on `http://127.0.0.1:8642/` with the browser client (open it in a
  browser; "Playing in the browser" above). No game is started unless the GAME flags say so; the
  page or a client starts one. `--port 0` picks a free port (the address is printed). `--bind
  0.0.0.0` listens on every interface (then the `Host` check below is off: only do this on a
  network you trust). `--assets DIR` serves the client files from `DIR` (`crates/ffh-play/assets`:
  live editing).
* `stage` writes a save at a staged situation into `DIR` for testing clients (`capture`: a warrior
  next to an undefended enemy city, moving in raises the keep/raze decision; `attack`: a warrior
  next to a barbarian warrior; `automoves`: skipping the one ready unit runs the AutoMoves rule;
  `load`: a warrior and two triremes in a coastal city, loading asks which trireme; `trade`: a
  civilization met at peace, Currency known and 100 gold each, so the trade tables hold gold),
  and prints
  its path and the ids as JSON. It writes only `DIR/KIND.ffhsave` and its sidecar (the staged
  game keeps no journal and no autosaves). Nothing in play uses it.
* `bench` measures the session's performance (design T13): the AI time per turn of a game the
  test bot plays against `ffh bench` (the `ffh` binary next to `ffh-play`, or `--ffh`), the
  session's own cost on one AI-played trajectory (its own work against the `update()` time
  inside its runs, which is budgeted; and, reported only, a bare update loop, the session and the
  session with its autosaves timed whole, `--runs` interleaved rounds, the same game each time:
  separate runs differ by a quarter or more on a busy host, even in the thread's CPU time),
  command latency on a standard map at turn 200, the view on a huge map and the QueryGuard; it
  prints each against its budget (`--json`: the figures) and exits 1 when one is missed.
  `--quick` is a small variant (seconds).
  The full run takes about a minute.
* GAME flags: each game gets a directory in `--saves DIR` (default `./saves`), `game-<seed>`
  (`-2`, ... when taken), with its autosaves (`auto/`, `--autosaves N` kept, default 20), journal
  and manual saves; `--no-saves` writes nothing. `--panic-at T` panics once when the game reaches
  turn T (or as a loaded save of turn T or later starts), a self-test of the crash report.
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
  Civ4 checks this every frame: after a command that leaves nothing ready while orders are
  queued, `auto_moves_due()` is true and a client calls `advance` again (the transports do).
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
command that performs each, ready to send, except a `target` action's plot: its command carries
the first of its `targets`, or your units' own plot as a placeholder (with `any`, or when no
target is in range), and you put in the plot you pick (`x`, `y` of a `move`, `data1`, `data2` of
a `mission`). Go-to is a `move`; a `mission` `MISSION_MOVE_TO` into a rival's borders asks the
declare-war question too (answer it with `move` and `declare_war`). Their JSON form is the
protocol request (`{"c": "move", "units": [..], "x": .., "y": ..}`), with type names or indices.
Parameters are accepted only in
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
| `status` | phase (`human`, `running`, `decision`, `over`; over the transports also `stuck`, `no_game`, `crashed` with the report), turn, year, you, the active players, the open decision, the winner, `view_version` (changes whenever the view does: a command, a decision opening or closing, debug reveal), `ai_auto_play` (the AI autoplay's turns left, 0 when off), and over the transports `game {dir, journal, autosave}` |
| `pending` | open events (with each option's effects), votes, research needed, idle cities, promotions, ready groups, idle AI-controlled groups |
| `view` | `me` (gold and rate, research, sliders, civics, score, Armageddon counter, mana, Stasis, the player options, AI autoplay), the players you met, the cities you revealed, the units you see, your groups (`ready`: some unit can move, as `pending.ready_groups`; `selectable`: every unit can, what Civ4's cycle selects), `group_cycle` (your groups in Civ4's cycle order) and `has_auto_unit`, `pending`, `log_next` |
| `map` | flat arrays `y * w + x`: `vis` (0 unrevealed, 1 fogged, 2 visible), `plot_type`, `terrain`, `feature`, `bonus`, `improvement`, `route`, `owner`, `river` (bit 0 river on the south edge, bit 1 on the east edge, bits 2-4 north/south flow + 1, bits 5-7 west/east flow + 1), `city`, `yield` (`food \| production << 8 \| commerce << 16`, visible plots only), `map_rev`, and `epoch` and `rev`. `map {since: R, epoch: E}` answers only the tiles changed since revision `R` (`full: false`, `tiles` = their indices, each array the values of those tiles), or the full map (`full: true`) when `E` is not the current epoch, `R` is unknown or much of the map changed |
| `plot {x,y}` | a plot as you know it; while visible also yields, defence, culture and the units |
| `group {units}` | the units in detail and the actions: `mission`, `target` (an interface mode, `mode`: go-to, route-to for workers, range attack, the air modes; needs a plot), `command`, `load`, `spell`, `promotion`, `upgrade`, each `enabled` or not, each with its `cmd` |
| `path {units,x,y}` | the path preview: nodes with the turn of arrival and moves left, total turns, a war it would declare, whether it ends in an attack |
| `combat {units,x,y}` | the attack preview: attacker, defender, odds of winning and of retreating (per mille), strengths, the modifiers of both sides |
| `city {city}` | the city screen of one of your cities: plots (with `can_work`), specialists (with `plus` and `minus`: what `do_task TASK_CHANGE_SPECIALIST` accepts now), yields, happiness, health, buildings, hurry, emphasis, queue, the buildable list with cost and turns |
| `techs`, `civics`, `religions`, `players` | the tech tree (prerequisites, depth, turns), civics (availability, upkeep, anarchy; `civics {with: [civic, ..]}` adds `proposed`: the set a revolution to those civics makes, whether `update_civics` would be applied, its anarchy and upkeep), religions, the players you met with your deals |
| `trade {with}` | the trade tables both ways (see Deals) |
| `log {since}` | your messages from index `since` (`view.log_next` is the next one) |
| `report` | the turn report: what changed between your end of turn and now, or the end of the game (units lost and gained, cities founded, lost, grown, starving, buildings, techs, civilizations met, wars and peace between those you had met, enemies near your cities, the Armageddon counter, borders), with the messages |
| `text` | a plain-text summary for agents |
| `ascii {x,y,w,h,layer}` | an ASCII map, 2 characters per plot, north up, `::` unrevealed; layers `terrain`, `political`, `yield`, `units`; a legend |
| `inspect {kind,oid}` | `{:#?}` of your unit, group, city, player, team, deal, or of an info entry (`UnitType`, `units`, ...) with id or index `oid`; any object, and plots, under debug reveal (a plot holds every team's data: use `plot`) |
| `data` | the static tables (every kind: type, name, texts, the main gameplay fields; missions, commands and builds with their Civ4 hotkeys `keys` and button order `order`), the interface modes' hotkeys, a few global defines |
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

## Transports: `ffh-play agent` and `ffh-play serve`

Both speak the same envelope (design §4.1). A request is a JSON object with `k` and an optional
`id` (an integer or a string) that the reply echoes:

```
{"id":1,"k":"q","q":"view"}                                     a query (the table above)
{"id":2,"k":"cmd","c":"move","units":[881],"x":15,"y":33}       a command
{"id":3,"k":"answer","decision":17,"option":1}                  or {"advisor":true}
{"id":4,"k":"session","op":"save","name":"before-war"}          a session operation
```

Replies are `{"id":1,"ok":true,"r":{..}}` (a command's adds `log`, the messages it produced, and
`status`; a command Civ4 would confirm answers `{"confirm":{"kind":"declare_war",..}}`, resend
it with the flag), or `{"id":1,"ok":false,"error":"<code>","detail":".."}` with the codes
`invalid`, `not_your_turn`, `decision_pending`, `not_owner`, `gone`, `cannot`, `busy`,
`no_game`, and over HTTP `crashed`.

Session operations (`op`):

| `op` | |
|---|---|
| `new {setup, name?}` | a new game (`setup` as `--setup`; `name`: its directory, default `game-<seed>`; not `auto`, which names autosaves) |
| `load {name}` | a save by name: `NAME` or `auto/NAME` in the current game's directory, `GAME/NAME` or `GAME/auto/NAME` in another game's (as the `saves` query and operation list them); nothing outside the saves directory |
| `take_over {player, name}` | take over slot `player` of an AI-only save (debugging) |
| `save {name}` | save the game in its directory (`[A-Za-z0-9_.-]`, 1-64, not starting with a dot) |
| `debug {reveal}` | debug reveal on or off |
| `saves` | every game directory with its saves |
| `quit` | end the agent / stop the server |

**Blocking decisions** (keep or abandon a captured city, a free tech, an election, ...) are
asked in the middle of a command or of the AI turns. While one is open, queries are answered
(from your side; the message log, report and saves list are `busy`), commands and session
operations get `decision_pending`, and `answer` with the decision's id and an enabled option (or
`advisor`) closes it; then the command that raised it gets its reply.

**The agent** prints events as well as replies, one per line:

* `{"ev":"control","turn","year","human","pending","log_next"}`: you have control (at the start,
  after your end of turn, after a load);
* `{"ev":"decision","decision":{id, kind, title, text, options, advisor, context}}`: the next
  lines you send go to the decision;
* `over` (the game ended), `stuck` (a turn did not finish: load an autosave), `no_game`,
  `progress` (with `--progress`, during the AI turns), `answered` (the end of input gave a
  decision to the advisor), `error` (the start failed, exit code 2), `crashed` (a panic, with the
  message, the last autosave, the journal and the `ffh-play replay` command; exit code 3).

A reply that hands control back (`end_turn`, `auto_moves`, `autoplay`, `new`, `load`, or any
command after which nothing is ready while some group has orders to continue: the last ready
unit skipped, fortified or sent on a long move) says `running` and comes first, then the game
runs (the AI turns, or your groups' orders, as Civ4 continues them as soon as nothing is left to
move), then the event. At the end of your input an open decision takes the
advisor's pick, then the agent quits, or with `--on-eof end-turn` ends the turn and prints the
next `control` first.

**The server** answers:

| Request | |
|---|---|
| `GET /`, `/app/*` | the browser client (`index.html`, `app/*.js` modules, `app/style.css`; embedded in the binary, or from `--assets DIR`) |
| `GET /api/status` | the status: `version`, `phase` (`human`, `running`, `decision`, `over`, `stuck`, `no_game`, `crashed`), `turn`, `year`, `human`, `active_players`, `view_version`, `decision`, `over`, `reveal`, `ai_auto_play`, `ms_in_phase`, `map_rev`, `game {dir, journal, autosave}`, `crash` |
| `GET /api/wait?since=V&timeout_ms=T` | the status as soon as its `version` is above `V` (long-poll, at most 60 s) |
| `GET /api/view`, `/api/map` | the `view` and `map` of the last control point or decision (409 without a game); `/api/map?since=R&epoch=E` the tiles changed since (as the `map` query) |
| `GET /api/data`, `/api/setup` | the static tables (with an ETag) and the new-game lists |
| `POST /api` | the envelope; needs the header `X-FFH: 1` |

The server listens on 127.0.0.1 only (unless `--bind`), refuses requests whose `Host` is not
`127.0.0.1`, `localhost`, `[::1]` or the loopback address it is bound to, and refuses a `POST`
without `X-FFH: 1`, so other web pages cannot drive it. A request must arrive whole within 15 s
(408); at most 64 connections are served at once (more get 503). A command's request waits while a decision it raised is open: answer the
decision on another connection (the status shows `phase: decision`). While the game runs (the
AI turns), queries are answered, session operations get `busy` and commands `not_your_turn`,
except `autoplay {turns: 0}` while an AI autoplay runs: it answers `{"stopping": true,
"turns_left": N}` and the autoplay stops before the next turn slice (FfH `setAIAutoPlay(0)`,
journaled at that slice; your slot is human again at once, and you have control at your next
turn, or at once during yours). The agent reads no input while the game runs, so there an
autoplay runs all its turns. If the rules crash, the
status becomes `crashed` with the report and the server stays up: load the autosave. Report the
bug with the journal and that autosave (`ffh-play replay JOURNAL --from AUTOSAVE`; before a loaded
game's first autosave, the report names the save it was loaded from).

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
  A member that a command, a `setup`, a session operation or an answer does not have is refused
  (``field `oponents`: unknown field (known: ...)``), so a typo cannot start a default game or drop
  a flag; queries ignore members they do not use.
* **Display names** are the whole description text, with Civ4 markup such as `[ICON_TRADE]` kept
  (`VOTE_LIBERTY` -> "Global Civic: Liberty"). Text FfH2 inherits from BtS is not vendored, so
  those names are derived from the type name (`UNIT_WARRIOR` -> "Warrior"), as are descriptions
  with an unfilled placeholder (`MEMORY_SOMNIUM_POSITIVE` -> "Somnium Positive").

## Testing the browser client

Automated (none of these is part of `cargo test`; `tests/web.rs` there checks that every client
file is embedded and every module import resolves):

```
tools/web-check.sh                       # node --check of every module, node --test of the pure modules
tools/web-check.sh --smoke --browser     # also the two smoke tests below
node tools/web-api-smoke.mjs             # 20 turns through the client's api.js against `ffh-play serve`
node tools/web-smoke.mjs                 # headless Chromium plays through the page; screenshots in target/web-smoke
```

* `web-check.sh` needs node (it is skipped without it); the pure modules (map maths and map
  deltas, markup, state and Civ4's group cycle, the hotkeys, the tech tree's layout, the API layer
  against a fake `fetch`) are tested in `crates/ffh-play/assets/test/*.test.mjs`.
* `web-api-smoke.mjs [--turns 20] [--seed 5] [--world WORLDSIZE_TINY] [--opponents 2] [--url URL]
  [--keep] [--verbose]` starts `target/release/ffh-play serve --port 0` (or `--bin`), checks the
  page and its modules are served, plays the turns as the page does (popups, research,
  production, found, builds, moves along path previews, the end-turn gate), checks the cached
  view and map against the status, saves, lists, loads, plays a staged capture whose decision is
  answered while the move's request waits, answers a staged transport picker (`stage load`) as
  the page does, and quits. It also follows the map by tile deltas (`/api/map?since=&epoch=`)
  and compares it with the full map at every check, and at turn 8 and at the end uses the
  screens' requests (a specialist's `+` and `−`, a civics proposal and its revolution, the
  research slider down and back, the trade tables, the inspector) and lets the AI play one
  turn (`autoplay`). A refusal of anything the server offered fails it.
* `web-smoke.mjs [--turns 4] [--seed 5] [--out DIR] [--assets DIR] [--cdp] [--headed]` drives the
  page with the mouse and keyboard in headless Chromium (the `playwright` package if it resolves,
  else Chromium over the DevTools protocol; skipped without a browser): new game (Start
  double-clicked: one game), `W` over every group the cycle selects, hotkeys pressed in the same tick as a
  selection change, found a city with `B`, the research and production popups, the city queue
  (Shift-click appends, ✕ removes), select a unit, the path preview, a right-click move, `Enter`
  while groups wait (it selects one and does not end the turn), end turns with the button through
  the gate, the turn report, save and load through the dialogs, the odds tooltip and an
  attack (`stage attack`), the keep/raze dialog (`stage capture`), popups set aside and asked
  again after loading the same turn, the transport picker (`stage load`), Shift-right-click
  waypoints and go-to (`G`) Shift-click waypoints held until Shift is released (`stage
  automoves`), a game started by another client
  while the page shows the new-game screen, and the crash screen of a server started with
  `--panic-at`; and (P6) the city screen (a worked plot clicked off and on, the automation), the
  tech tree (`F6`, a click researches), civics (`F3`, a pick and the revolution), religion
  (`F7`), the sliders (top bar and `F2`), diplomacy (`F4`; the trade screen when a player was
  met), the cities list (`F1`), the keys (`?`), the go-to mode (`G`, `Esc`), the map layers, the
  inspector, debug reveal, an AI autoplay turn and a 5,000-turn one stopped from the top bar, the spectator mode (play, pause, step, stop),
  and the trade screen (`stage trade`: 7 gold offered and accepted); and the art client: the art
  loads and draws the map (by its worker), the view is drawn in full and is not blank, a frame
  takes under 60 ms (median of 15), the zoom goes through the chart (8, 16) and the art's steps
  (32-128) with `+` and `-`, the interface has its bitmap type and icons, Tools → Options switches
  to the flat renderer and back, and `?renderer=flat` keeps the flat one. Any console error, page
  error, failed request or HTTP error fails it. The screenshots (`target/web-smoke/*.png`: the art
  map, the survey zoom and the chart, the flat renderer and every screen above) are for review,
  not committed.
* Frame times of the art map (headless Chromium 141, software rendering, 1280x800, a 4-core host
  shared with other work; `window.ffh.mapView.art.stats`): on a standard map at 2x a frame with
  everything drawn takes 3-5 ms (the vignette 2-4 ms of it, the units about 1 ms) and a frame of a
  slow pan 4-9 ms; on a huge map (128x80) with debug reveal, so that every plot is drawn, a frame
  takes 2.6-4 ms still, 3-8 ms panning (14-20 at most), and a sweep at two plots a frame 3-4 ms
  (16-36 at most: the worker draws the chunks, the page shows their chart meanwhile); the chart
  steps take under 1 ms a frame. A jump to an undrawn part of the map is drawn in full within
  about half a second on a quiet host, 1-2 s with the host's 4 cores loaded 11 deep (single frames
  of 40-100 ms then show up in any phase). A chunk of 4x4 plots takes the worker 15-100 ms (up to
  250 ms for the first ones, which fill the sprite atlas).

**Manual checklist** (a human tester, `ffh-play serve` and a real browser; also try a narrow
window):

1. New game: change the civilization (the leader list follows), world size, difficulty, a game
   option and a victory condition; start. The map centres on your units; the top bar shows the
   turn, gold and "choose research".
2. Map: drag to pan across the east-west seam (no seam in the ground, borders or rivers), zoom
   with the wheel and `+`/`-` through every step (the chart at 8 and 16, then 32 to 128: the art
   stays crisp, never blurred), click and drag the minimap; pan fast to a part you have not seen
   (flat colours for a moment, then the art). Hover plots: the tooltip names the terrain,
   feature, resource, improvement, owner, and yields while visible. Check hills, peaks, forests,
   rivers on edges, roads, lairs and goody huts, resource seals, borders, fog (darkened and
   hatched) and unexplored (black, ragged) plots, a city's light, plaque and banner, the stack
   standards (bright on your units that can move), and the political and yield layers. Reload
   with `?renderer=flat`: the flat map of P5 comes back (and Tools → Options toggles it).
3. Select the settler (click), found a city (`B` or the Found button): the research popup comes
   first, then the production chooser; choose with a click, try Shift-click (queue) and "Later".
4. Select a warrior: the path preview follows the mouse with turn numbers; right-click moves;
   hold Shift and right-click two or three plots: they are numbered on the map and the warrior
   stays selected; release Shift: the warrior gets them as orders in turn (it moves once nothing
   else waits for orders); `G` then click goes there, `G` then Shift-clicks add waypoints the
   same way (the go-to mode stays on until Shift is released); `Esc` cancels. A move into a
   rival's land asks to declare war (try "Cancel").
5. Group units on one plot (`All`, `Group`), split them, Ctrl-click rows; `W`, `Enter` and `Tab`
   go through every waiting group before any comes up again (`Shift+Tab` backwards); move one unit
   of a two-unit stack with different moves (a warrior and a scout grouped, one plot): `W` skips
   the stack from then on, as Civ4 does; once nothing waits the End turn button glows and `Enter`
   (or `Space` with nothing selected) ends the turn; `Home`/`End` go through your cities, `F10`
   opens the capital; `Space`, `F`, `S`, `G`
   work on the selection (also pressed right after clicking another unit); a worker's build
   buttons show turns and their keys (`R` road, `I` farm, `M` mine); compare a few keys with
   Civ4's.
6. End the turn with units waiting (the End turn button; `Enter` selects the next unit then): the
   gate lists them; "Select a unit" goes back, "End turn anyway" ends it. During the AI turns the top bar says who is moving. The next turn's report
   and messages appear in the bottom panel.
7. Open a city (double-click, the Cities list, or the button when your units stand in it):
   replace, append (Shift), insert first (Ctrl) and remove (✕) production; click worked and
   unworked plots, `+`/`−` a specialist, hurry, toggle an emphasis and the automation; Next city.
   Then the empire screens: `F6` research from the tree, `F3` pick civics and read the anarchy,
   `F7`, `F2` move a slider, `F4` open a trade with a player you met and propose something
   (`ffh-play stage trade saves/staged-trade`, load `staged-trade/trade`, if you met nobody).
   Switch the map layers (political, yields).
8. Save under a name, load it back (Load lists it and the autosaves); reload the page in the
   middle of a turn and while a decision is open: the page comes back in the same state.
9. `ffh-play stage attack saves/staged-attack`, load `staged-attack/attack`: hover the barbarian
   with the warrior selected (odds, strengths, modifiers), right-click to attack, read the result.
   `ffh-play stage capture saves/staged-capture`, load `staged-capture/capture`: move onto the
   city; the "Raze or keep?" dialog cannot be closed; answer it (or "AI default").
   `ffh-play stage load saves/staged-load`, load `staged-load/load`: "Later" on the research and
   city popups, load the same save again (they come back); select the warrior in the city's
   stack list, Load: it asks which trireme; pick one, and the warrior is in it.
10. Play 20-50 turns of a normal game: events and votes when they come (each choice's effects,
    "Later"), research done, cities growing, a war with an AI; note anything that looks wrong
    with the turn, the autosave and the journal shown under "Game files".
11. Crash: `ffh-play serve --new --panic-at 3`, end turns until it crashes: the crash screen shows
    the replay command and loads the autosave.
12. Tools: debug reveal on and off, the inspector on your unit, city and player, AI autoplay for
    3 turns, Spectate (pause, step, stop), and a new game with "Spectate" ticked.

Report a bug with the save or autosave, the journal (`Game files` tab), a screenshot and what you
did.

## Known gaps

* The art map: two civilizations of one tincture are told apart by a divided field given in the
  order this page met them (docs/ART.md 7.2 gives it in slot order at the start, which needs every
  civilization, also those you have not met); it holds while the page is open, and a reload may
  give the divisions the other way round. Units face west after a move west this page saw (a
  reload faces them all east). A holy city's glyph is not on its plaque (the view has no holy-city
  flag for foreign cities), disorder shows only on your own cities (the view gives it only for
  them). Animations are limited to the marching path: fire and water frames are drawn once into
  the cached terrain.
* Shift-right-click waypoints are sent when Shift is released, not one by one as Civ4 does (it
  holds the automatic moves while Shift is down instead): the end result is the same.
* The spectator mode is the AI autoplay of one turn at a time (FfH's "AI autoplay" for your slot,
  then your turn starts and the next step begins), not an observer slot: between two steps your
  turn starts and ends without you, and the events and votes of your civilization are answered by
  the AI.
* The keys of the interface modes (go to, route to, the targeted missions) are written into
  ffh-play from Civ4's `CIV4InterfaceModeInfos.xml` (not vendored); the other orders' keys are read
  from the vendored XML.
* The autosave at each turn boundary adds about half to the time of an AI turn on a standard
  map (`ffh-play bench`: `Game::save` and the file write, about 7-8 ms a turn against 17 ms of
  `update()`); it is the session's largest cost (its other work is about 0.2%).
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
