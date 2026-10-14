# ffh-play: human play for ffh2-rs (final design)

This is the design of `crates/ffh-play`, implemented phase by phase (§7); the status table below says what exists. Every claim marked *verified* was checked read-only against `projects/ffh2-rs` at commit 5ee3be6 (branch ccr-17ac9b54-lbgjpn) and against the C++/Python under `/home/user/ref/ffh2`.

**Implementation status.**

| Phase | Status | Notes |
|---|---|---|
| P0 skeleton | done | `json.rs`, `names.rs`, `ffh-play kinds / names / lookup`; see "As built" under P0 in §7. |
| P1–P7 | not started | |

**Starting point.** The design starts from the **web** design, which had the highest total score: 8.5 + 7.5 + 7 = 23, against 21.5 for tui and 21.5 for engine-first. It grafts on the ideas the judges recommended (§11 lists every disagreement and how it was resolved).

**Name.** The crate is called **`ffh-play`** rather than `ffh-web`, because it serves both a browser and a JSONL agent transport.

---

## 0. Decisions at a glance

| Question | Decision |
|---|---|
| Where code lives | New crate `crates/ffh-play`: library `ffh_play` plus binary `ffh-play`. Dependencies: `ffh-data`, `ffh-core`, `ffh-ai`, std only. The MVP needs **no edits** to ffh-core, ffh-ai, ffh-cli or `controller.rs`. Shared-file edits: the workspace `members` line, docs, and `.gitignore`. |
| Who owns `Game` | One **game thread**, created by the transport, owns the `Session`, and with it `Game`. Transports talk to it. In-process tests, the bot, the soak and stdio mode are single-threaded. |
| How the human acts | Only through public rules APIs, via 1:1 ports of what C++ does for human input: `CvNet*::Execute` (CvMessageData.cpp) plus the exe-side helpers that send those messages: `CvGame::selectionListMove`, `selectionListGameNetMessage`, `cityPushOrder`, the `handleAction` confirmations, and the AutoMoves part of `updateTestEndTurn`. Commands run **between** `update()` calls, as the exe does it. |
| Synchronous Controller decisions | **Hybrid.** (a) **Deferred, C++ popup semantics**, where the port's "no answer" return is already what an unanswered C++ popup does: production, events, diplomatic votes, the research popup. The human answers later with the C++ net-message port. (b) **Blocking Answerer**, a modal that still serves read-only queries, where "no answer" loses information: free tech, keep/raze/disband city, election, offer, religion. (c) A core popup queue (`Player::popups`) is an optional, coordinated **phase 7** item. |
| `doResearch` double call | The `AI_chooseResearch` call (economy.rs:1262) is answered by the advisor, as C++ silently does for humans. The popup call (:849) is deferred to the UI. |
| Advisor | A private `ffh_ai::Ai::new()` per session. It is never the instance the AI players use. |
| Purity of reads | Five layers: (1) a session-owned interface `PathFinder`, the counterpart of `GC.getInterfacePathFinder()`; (2) an allow-list; (3) a **`QueryGuard`** that snapshots and restores every interior-mutable cache around every read of `&Game`; (4) save-bytes equality tests; (5) a noisy-client test. |
| Human front-end | A browser client: hand-written vanilla ES modules plus `<canvas>`, embedded with `include_str!`, served by a std-only HTTP/1.1 server on 127.0.0.1. |
| Agent front-end | JSONL over stdio, using the same request envelope and the same fog filter. It adds `text` and `ascii` queries so an LLM or script can play-test headless. There is no TUI. |
| Order of work | Session and decision semantics, then bot and **AI-as-human soak** (flush the human-only rules branches), then views, then transports, then browser. |

---

## 1. Verified facts that shape the design

| # | Fact | Where (verified) | Consequence |
|---|---|---|---|
| F1 | `update()` runs one turn slice: `do_turn` if no player is active, then `update_score`, `update_war`, `update_moves`, `update_timers`, governor work for human teams, `test_alive`, and a game-over check when no human is alive (unless autoplay or `autorun`). | game/turn.rs `update` | Session loop = exe loop. Set `autorun = false` so the game ends when the human dies. |
| F2 | `update_moves` calls `controller.play_turn(g, p)` **every slice** while the turn is active and `!auto_moves`. It auto-ends only non-human turns. A human's turn ends only through `player_set_end_turn(p, true)`: that sets auto moves, then `set_auto_moves(false)`, then `set_turn_active(false)`, then `player_do_turn` (which includes `player_do_research`), then the next player is activated in the same pass. | game/turn.rs `update_moves`; player/turn.rs:490-585 | `EndTurn` is one command. `HumanController::play_turn` must never block and must be cheap. |
| F3 | Mission timers: a human group gets `missions[m].time`. MOVE_TO is capped at 2 per step, or at 1 if the **active player** has `PLAYEROPTION_QUICK_MOVES`. AI groups get 0 unless *any* human has `SHOW_ENEMY_MOVES` or `SHOW_FRIENDLY_MOVES`. | group/missions.rs:23-45, :147-186 | Set `active_player = Some(h)` and QUICK_MOVES on. Force SHOW_* off (otherwise AI turns re-enter `play_turn` per step). After every human command, run `update()` until no human group is busy. |
| F4 | `group_is_busy` and `group_is_cargo_busy` take `&self`. `player_has_busy_unit` takes `&mut`. | group/misc.rs:234,258; player/turn.rs:373 | The phase predicate is pure. |
| F5 | `player_do_research`: if research is None and `active_player == Some(p)`, it calls `player_choose_tech(p,0)`, which calls `choose_research` (:849). Then, if `elapsed > 4`, it calls `choose_research` **again** and does `push_research(p,t,false)`, which appends (:1262). If that path was taken, it clears the queue at the end (:1290). Other `player_choose_tech(p,0)` callers: research-next on tech completion (team/tech.rs:624) and advanced-start exit (player/start.rs:613). | player/economy.rs:818-860, 1242-1293 | Deferring the popup and having the advisor answer the AI pick reproduces C++ exactly. Asking the human at both call sites would ask twice, and the answer would then be wiped by the clear. |
| F6 | `choose_free_tech` is only reached for humans (`discover > 0`). `None` means the tech is lost. | economy.rs:826-842 | Must block. |
| F7 | `player_trigger_data`: options are the trigger's events filtered by `can_do_event`. With no options there is no call. If the controller returns `None`, the triggered data **stays pending**. C++ `CvPlayer::trigger` for a human queues `BUTTONPOPUP_EVENT` (CvPlayer.cpp:19961). `launchEventPopup` re-checks `canDoEvent` and drops the popup if no event is available. The answer is `CvNetEventTriggered` → `applyEvent(e, id)` with `bUpdateTrigger = true` (CvMessageData.cpp:670). | player/events.rs:957-978, 1472 | Defer events. Answer with `player_apply_event(h, e, id, true)`. Re-check options when the popup is shown. |
| F8 | `add_vote_triggered` asks `diplo_vote` for every voting member and then calls `cast_vote`. `do_vote_results` gives a missing vote one more turn, then counts it as abstain. `CvNetDiploVote` is `castVote(player, id, choice)`. | game/votes.rs:927-951, 962-1000; CvMessageData.cpp:1024 | Defer votes: return `NO_PLAYER_VOTE`. Pending votes can be derived from saved state. The human answers with `cast_vote`. |
| F9 | `choose_election` feeds `set_vote_chosen(choice, id)` directly, and `None` **deletes the selection**. | votes.rs:1136-1137, 641 | Must block (or wait for a phase-7 hook). |
| F10 | `raze_city_popup` (conquest) and `disband_city_popup` (culture flip or spells) both ask `raze_conquered_city(g,p,c)` with the same arguments. The gift button has no hook. | player/cities.rs:67-95, 540-560 | Use a neutral "keep / abandon" modal. A phase-7 hook can tell the two apart. |
| F11 | Interior mutability in saved state: six `RefCell<PathFinder>` (game.rs:276-286); `Player::ai_bonus_value: Vec<Cell<i32>>` (player/mod.rs:367); **city rank caches** `population_rank`, `base_yield_rank`, `yield_rank`, `commerce_rank` and their `_valid` flags, all `Cell` (city/mod.rs:251-259). `city_largest_city_happiness` (citizens.rs:366) fills the population-rank cache, so **any happiness display mutates saved state**. A test documents base-yield ranks staying "stale until invalidated". `&self` finder users: `group_generate_path` and `group_get_path_*` (path finder), `unit_get_upgrade_city(search=true)` (promotions.rs:467-623, path finder), `player_count_unimproved_bonuses` (border finder). | as listed | A read-only UI can still change later rules results and save bytes. This motivates the `QueryGuard` (§2.5). |
| F12 | `PathFinder` is `Clone` and `pub`; `PathFinder::new(FinderKind::Path)` and `set_group`; `generate_path(&mut self, g: &Game, ..)`. | path.rs:237-353 | The interface finder can be owned by the session. |
| F13 | Saves: `Game::save` / `Game::load`; `at_turn_boundary()`; `FORMAT_VERSION = 1`. `active_player`, `autorun`, `ai_auto_play`, `Player::options` and `GameLog` are all saved. Controllers are not saved. | save/mod.rs:52-160, 194, 217-219 | Mid-turn saves at a human control point must be tested. The session drains the log at deterministic points. |
| F14 | `GameLog { enabled, entries, cap }`; `cap` evicts from the front. There are 86 `log.msg` sites (C++ has about 190 `addMessage`). Entries are tagged with the *subject* player. | log.rs; grep | Session-side message store plus a diff-based turn report. |
| F15 | ffh-ai `Ai { sites, played }` caches are keyed by game turn. It implements `choose_production`, `choose_research`, `raze_conquered_city`, `consider_offer` and `attitude` only. It does **not** implement `update_automated_group`, `choose_event`, `diplo_vote`, `choose_election` or `choose_free_tech`, so trait defaults apply. Its diplomacy skips humans (diplomacy.rs:138, 277). Core never calls `update_automated_group` except through the controller. | ffh-ai/src/lib.rs:55-120 | Automation and FfH AI-controlled human units idle until ffh-ai implements the hook (phase 7). The advisor's "pick" for events, elections and votes is the trait default. |
| F16 | No FfH2 0.41 spell has `bAllowAuto` (grep finds 0 matches; unit/ffh.rs:124 notes it). | data/xml | The human auto-cast port proposed by engine-first is dead data. Hide `COMMAND_AUTOMATE_SPELL`. |
| F17 | Setup: `PlayerSetup { civ, leader, team, handicap, slot_status, human, .. }`, `SlotStatus::Taken`, `GameSetup::from_options(d, &SetupOptions)`, `Game::new(d, setup)`, and `Game::setup_handicap`, which takes the first human slot's handicap. | setup.rs:15-237; game.rs:416 | New-game code is short. |
| F18 | C++ exe helpers: `cityPushOrder` rewrites flags when the city has stored production (CvGame.cpp:2472). `CvNetPushOrder` → `pushOrder(o, d, -1, alt, !(shift‖ctrl), shift)`. `CvNetResearch` is as at CvMessageData.cpp:364. `selectionListMove` puts up the DECLAREWARMOVE popup (CvGameInterface.cpp:828). `selectionListGameNetMessage` joins the selection into a group unless it `mirrorsSelectionGroup` (:900). `updateTestEndTurn` sends AutoMoves when nothing is ready (:386). FFH2's `cannotSelectionList*` and `cannotHandleAction` all return False. | ref C++ / CvGameUtils.py | `command.rs` ports these exactly. |
| F19 | AI cost: `ffh bench --seed 1 --civs 6 --size WORLDSIZE_STANDARD --turns 150` measured **10.51 ms/turn** (slowest 28.6 ms). | measured today | The UI adds the latency; the AI is fast enough. A synchronous end turn is fine. |
| F20 | ffh-stress `check.rs` has `pub fn check(&Game)` and depends only on ffh-core. | ffh-stress/src/check.rs | Reused by the bot and soak tests through `#[path = "../../ffh-stress/src/check.rs"] mod check;`, with no edit to ffh-stress. |
| F21 | Node 22 is installed (`/opt/node22/bin/node`). Headless Chromium is available: the Playwright browsers are in `/opt/pw-browsers` (`PLAYWRIGHT_BROWSERS_PATH`; `chromium-1194`, `chromium_headless_shell-1194`), and the `playwright` npm package (1.56) is installed globally under `/opt/node22/lib/node_modules`. Without that package, `chrome-linux/chrome --headless` can be driven directly (`--dump-dom`, `--screenshot`, DevTools protocol). Only the `x86_64-unknown-linux-gnu` target exists. | env (checked: a Playwright script launched Chromium and ran page JS) | JS gets `node --check` and `node --test`, and P5 adds a **headless-browser smoke test** (load the page against `ffh-play serve`, no console errors, play a few turns through the UI, screenshots for review). All of these are optional and skipped when node or the browser is missing. Correctness still rests mainly on server-side logic and protocol tests: the client only renders what the engine computes. |
| F22 | The FfH main HUD shows the Armageddon counter (`getGlobalCounter`) and mana counts (`getNumAvailableBonuses` over `BONUS_MANA_*`). | CvMainInterface.py:102, 855, 3386 | Both go in the top bar (`get_global_counter`, game/world.rs:1351). |

---

## 2. Architecture and threading

### 2.1 Components

```
 Browser (index.html + ES modules)          Agent / script / LLM (stdin/stdout)
     | HTTP/1.1 JSON, 127.0.0.1, POST needs `X-FFH: 1`     | JSONL: one request per line, replies + events per line
     v                                                     v
 http.rs: acceptor + thread per connection         stdio.rs: runs ON the game thread (no extra threads)
   GET cached (status, wait, view, map, data, setup) <- Shared (Mutex+Condvar), never waits for the game thread
   POST /api  -> Envelope{req, reply_tx} --mpsc-->  GAME THREAD
                                                     Session { g: Game, human, journal, messages, ui: Arc<Mutex<UiState>> }
                                                       advance(): g.update() until the human has control / game over
                                                       execute(cmd): validate -> journal -> CvNet port -> settle
                                                       query(q): under QueryGuard, allow-listed &Game functions
                                                     HumanController (Controller on the human slot)
                                                       deferred decisions -> SessionShared (pending events)
                                                       blocking decisions -> Answerer::ask(&Game, &Decision)
                                                     Advisor = private ffh_ai::Ai (attitude, autoplay, AI pick, suggestions)
                                                     AI slots = ffh_ai::ai_controller() exactly as ffh-cli
```

The game thread creates `Game` (new or load), so `Game` never needs to be `Send`.

### 2.2 The loop

**Phase predicate.** It is pure and uses only `&self` functions:

```rust
fn phase(g: &Game, h: PlayerId) -> Phase {
    if g.game_state == GameStateType::GAMESTATE_OVER { return Phase::Over }
    let pl = &g.players[h];
    if pl.alive && pl.human && pl.turn_active && !pl.auto_moves && !pl.end_turn
        && !pl.groups.iter().any(|&grp| g.group_is_busy(grp) || g.group_is_cargo_busy(grp)) {
        return Phase::Human(h)
    }
    Phase::Running
}
```

**`advance`.** Runs the AI turns, auto moves, end-of-turn processing and blocking decisions:

```rust
pub fn advance(&mut self, on_slice: &mut dyn FnMut(&Game)) -> Stop {
    let mut slices_this_turn = 0;
    loop {
        match phase(&self.g, self.human) {
            Phase::Over => { self.drain_log(); return Stop::Over }
            Phase::Human(h) => {
                self.drain_log();
                // updateTestEndTurn port: nothing ready but queued orders -> CvNetAutoMoves (journaled).
                // At most once per control episode, so a stuck mission cannot spin.
                if self.opts.auto_moves_when_idle && !self.auto_moves_tried
                    && !self.g.player_has_ready_unit(h, true) && self.g.player_has_auto_unit(h) {
                    self.auto_moves_tried = true;
                    self.apply_journaled(Command::AutoMoves);   // = execute() without the phase check
                    continue;
                }
                self.on_control_point();                       // turn report diff, autosave-on-first-control-point
                return Stop::Human
            }
            Phase::Running => {
                self.g.update();
                if self.g.at_turn_boundary() { self.drain_log(); self.autosave(); slices_this_turn = 0; }
                on_slice(&self.g);                              // HTTP: publish status, answer queued queries
                slices_this_turn += 1;
                if slices_this_turn > 200_000 { return Stop::Stuck(diagnose(&self.g)) } // as ffh-cli run.rs
            }
        }
    }
}
```

**`execute`.** Only valid in `Phase::Human`, except `Event` and `Vote`, which are also only valid there:

```rust
pub fn execute(&mut self, cmd: &Command) -> Result<CmdOk, CmdErr> {
    let Phase::Human(h) = phase(&self.g, self.human) else { return Err(CmdErr::NotYourTurn) };
    let plan = command::validate(&self.read_guarded(), h, cmd)?;  // pure; may return NeedsConfirm
    if let Plan::NeedsConfirm(c) = plan { return Ok(CmdOk::confirm(c)) }   // not journaled, no state change
    self.journal.cmd(self.g.game_turn(), self.g.turn_slice, cmd);
    let log_mark = self.g.log.entries.len();
    command::apply(&mut self.g, h, &plan);              // CvNet ports; may raise blocking decisions
    self.settle(h);                                     // >= 1 update(), then while a human group is busy (<= 10_000)
    self.auto_moves_tried = false;
    Ok(CmdOk::applied(self.take_log_since(log_mark)))
}
```

Notes on `settle` and `EndTurn`:
* **Why at least one `update()`.** It plays the role of the exe's next frame: the governor work (`ai_update_assign_work` on dirty cities), `update_war`, `test_alive` and timers all run. The number of extra slices depends only on state, so replay is exact.
* **What `EndTurn` does.** `player_set_end_turn(h, true)` makes the phase `Running`. `advance` then completes the human's auto missions, `player_do_turn(h)`, all AI turns and `do_turn`, and returns at the human's next control point.

### 2.3 Decision policy per Controller method (`human.rs`)

**Rule when the slot isn't human.** Every method first checks `g.players[p].human`. If it is false (FfH AIAutoPlay via `set_disable_human`), the call is **delegated to the advisor** unchanged, as a non-human CvPlayer is a CvPlayerAI.

**Explicit implementation.** `HumanController` implements every trait method explicitly; the drift test in §8 enforces this.

| Method | C++ for a human | Human policy |
|---|---|---|
| `play_turn(&mut g, p)` | `CvPlayerAI::AI_unitUpdate` human branch (CvPlayerAI.cpp:1288-1305) | If `!g.player_has_busy_unit(p)`: for each `grp` in `group_cycle.clone()` that still exists and `group_is_automated(grp)`: if `advisor.update_automated_group(g, grp)` then stop. Never blocks. No-op until ffh-ai implements the hook (F15). |
| `choose_production(&mut g, c)` | CHOOSEPRODUCTION popup; `AI_chooseProduction` when `isProductionAutomated()` | `production_automated` → `advisor.choose_production`. Otherwise **no-op (deferred)**: the empty queue appears as `pending.idle_cities` and the UI opens the chooser, as the C++ popup does (its `OnFocus` cancels once production is set). |
| `choose_research(&g, p)` | :849 = CHOOSETECH popup (queued); :1262 = silent `AI_chooseResearch` | **Research-site rule.** Returns the advisor's pick only for the :1262 call, otherwise `None` (deferred; the UI prompts when `pending.research_needed`). How the :1262 call is recognised: `SessionShared` keeps `research_mark: Option<(turn, slice, p)>`. On a call with `!turn_active && research None`: if the mark equals `(game_turn, turn_slice, p)`, clear the mark and return `advisor.choose_research(g,p)` (this is :1262, which follows :849 in the same slice of `player_do_research`). Otherwise set the mark and return `None`. Calls while the turn is active (tech completed by a great person or a trade, advanced-start exit) always return `None`. The mark lives only within one slice; saves happen at control points, so nothing needs persisting. The phase-7 hook `ai_choose_research` would replace this. |
| `choose_free_tech(&g, p)` | CHOOSETECH with `iDiscover > 0` | **Blocking** `Decision::FreeTech`. Options are the techs with `player_can_research(p, t, false)`. The advisor suggestion is `advisor.choose_free_tech`. |
| `raze_conquered_city(&g, p, c)` | RAZECITY / DISBANDCITY | **Blocking** `Decision::KeepCity { city, can_raze }`. Buttons: Keep / Abandon. The title is "Disband?" when `!player_can_raze(p,c)` (only the disband popup can ask then), otherwise "Raze or keep?" with a note (F10). No gift button. |
| `choose_event(&g, p, t, options)` | EVENT popup queued (C++ `trigger`) | **Deferred.** Push `(p, t.id)` to `SessionShared.pending_events`, return `None` (F7). |
| `diplo_vote(&g, p, sub, src)` | DIPLOVOTE popup queued | **Deferred.** Return `PlayerVote::NO_PLAYER_VOTE`. The pending vote is derived from state (F8). |
| `choose_election(&g, p, vs)` | CHOOSEELECTION popup | **Blocking** `Decision::Election`. Options are `vs.vote_options[i]`. The advisor suggestion is the trait default (`None`, which means "no election", as the AI does). |
| `consider_offer(&g, p, from, ..)` | diplomacy screen | **Blocking** `Decision::Offer`. Unreachable today (F15). |
| `choose_religion(&g, p)` | FOUND_RELIGION (PICK_RELIGION is locked off in FfH) | **Blocking** `Decision::Religion`. Unreachable. |
| `keep_captured_unit`, `accept_unit` | the human keeps or accepts without asking | `true` |
| `attitude(&g, p, towards)` | `CvPlayerAI::AI_getAttitude` is computed for humans too | `advisor.attitude` (a pure function of state) |
| `update_automated_group` | `CvSelectionGroupAI::AI_update` | `advisor.update_automated_group` |

**What the human answers at a control point.** These are derived pending items, each answered with a journaled command:

| Pending item | How it is derived | Answer command | Rules call |
|---|---|---|---|
| events | `SessionShared.pending_events` for h, filtered: the triggered data still exists and ≥1 `player_can_do_event` option (launchEventPopup); entries with no options are dropped | `event {triggered, event}` | `player_apply_event(h, e, id, true)` |
| votes | `votes_triggered` with `player_is_voting_member(h, src)` and `player_vote(h,id)` ∈ {NO_PLAYER_VOTE, NO_PLAYER_VOTE_CHECKED} | `vote {vote, choice}`; the choices are the DIPLOVOTE popup's buttons (`launchDiploVotePopup`; §4.5): a team vote offers **team ids**, other votes YES, NO, NEVER (only if `player_can_defy_resolution`) and ABSTAIN | `cast_vote(h, id, choice)` |
| research | `player_get_current_research(h).is_none()` and some tech passes `player_can_research(h,t,false)` | `research {tech, shift}` | CvNetResearch port |
| idle cities | `!city_is_production(c)` for own cities | `push_order` | cityPushOrder plus the CvNetPushOrder port |

**End-turn gate.** C++ popups are modal, so `end_turn` returns `NeedsConfirm(EndTurn{events, votes, research, ready_units})` while any of these exist. With `force: true`:
* unanswered events are dropped from the pending list (C++ closing a popup with -1 does nothing);
* votes stay unanswered (the C++ extra turn, then abstain);
* empty research gets C++'s AI pick after turn 4 through the research-site rule.

### 2.4 Answerer and re-entrancy

```rust
pub trait Answerer: Send + Sync {
    /// On the game thread, inside a rules call, with the &Game the rules handed the controller.
    fn ask(&self, g: &Game, d: &Decision) -> Answer;
}
pub enum Answer { Option(usize), Advisor }
```

The implementations:

| Implementation | Used by | Behaviour |
|---|---|---|
| `ChannelAnswerer` | HTTP | Publishes `status.phase = "decision"` and the decision. Loops on the shared input receiver: `q` requests are served from `g` under a QueryGuard; `answer` with this id returns; `cmd` and `session` requests get `decision_pending`. |
| `StdioAnswerer` | agent mode | Prints `{"ev":"decision",..}` and reads lines with the same rules as `ChannelAnswerer`. |
| `BotAnswerer` | tests | A closure. |
| `AdvisorAnswerer` | soak | Returns `Answer::Advisor`. |
| `JournalAnswerer` | replay | Reads the next answer and asserts that the decision kind matches; a mismatch means desync. |

Rules for re-entrancy:
* **One input receiver, many senders.** The loop and the Answerer lock the receiver only around `recv()`. No lock is held across an ffh-core call.
* **Decisions raised inside a command are answered on the same channel.** For example, attack, then capture, then keep/raze: the first HTTP request (the `cmd`) waits for its reply while a second connection answers.
* **Answers are journaled** in the order they were asked, as `{"k":"answer","t":..,"s":..,"kind":"keep_city","option":1}`.
* **Saving is refused while a decision is open.**
* **Views built during a decision run under `catch_unwind(AssertUnwindSafe)`.** If one fails, the previous view is kept.

### 2.5 Purity of reads: interface finder, allow-list, QueryGuard

Every read of `&Game` outside rules calls goes through `QueryGuard`. That includes views, queries, the bot's own reads, Answerer modals and the turn-report snapshot.

```rust
pub struct QueryGuard<'a> { g: &'a Game, ranks: Vec<CityRankSnapshot>, bonus: Vec<Vec<i32>>,
                            path: Option<PathFinder>, border: Option<PathFinder> }
impl<'a> QueryGuard<'a> {
    pub fn new(g: &'a Game) -> Self;   // copy every city's rank Cells + _valid flags, every player's ai_bonus_value,
                                       // and clone path_finder / border_finder if not currently borrowed
                                       // (try_borrow; if borrowed, finder-using queries return `busy`)
}
impl Drop for QueryGuard<'_> { fn drop(&mut self) { /* write all of it back */ } }
```

Cost of the guard:
* Rank caches: 22 cells per city.
* Bonus values: players × bonuses.
* Finders: two `PathFinder` clones of about 0.3–0.6 MB on huge maps.
* Altogether it is well under 1 ms per query. Queries are batched per request.

**Allow-list.** Queries must still not call `group_generate_path`, `group_get_path_*`, `unit_generate_path`, `unit_get_upgrade_city(_, true)`, `player_count_unimproved_bonuses`, `player_ai_found_value` or `player_ai_bonus_val`. The guard is the safety net, not a licence.

**Path previews** use `UiState.interface_finder = PathFinder::new(FinderKind::Path)`: call `set_group(Some(grp))`, then `generate_path(&g, ..)`. The game's saved finders are never touched.

### 2.6 Saves, sidecar, autosave, journal, replay, take-over

**Save bundle.** A save is two files:
* `<name>.ffhsave`: plain `Game::save()` bytes, loadable by any tool;
* `<name>.ffhsess`: a small JSON sidecar `{v:1, human, pending_events:[[p,id],..], journal:"<path>", journal_line:N, messages_tail:[..]}`.

The sidecar holds the only session state that matters to the rules: deferred event ids. These are the C++ `m_listPopups` content, which C++ saves with the player. Pending votes, research and idle cities are derived from `Game`.

**When saving is allowed:**
* at a human control point;
* at a turn boundary.

It is refused while a decision is open.

**Why a save at a control point resumes exactly.**
* With the human in slot 0, which is the default and matches Civ4 single player, no AI has played yet in that game turn.
* ffh-ai caches are keyed by game turn.
* The research mark lives only within one slice.

Tests check this (T3).

**Autosave.**
* Written at every turn boundary as `saves/auto/T{turn:04}.ffhsave` plus `.ffhsess`; the last 20 are kept.
* The log is drained just before the save, so saves never contain log entries.
* Wall-clock time is used only to name manual saves.

**Log.**
* `g.log.enabled = true` and `cap = 0`.
* The session drains `g.log.entries` into its `MessageStore` at every control point, turn boundary and command reply. The drain points are deterministic, so save bytes match in replay.

**Journal.** `saves/<game>/journal.jsonl` gets one line per state-changing input:
* the header `{"k":"new","setup":{..}}` or `{"k":"load","fnv":"…"}`;
* `cmd` lines;
* `answer` lines;
* a `checkpoint` line `{turn, checksum: calculate_sync_checksum()}` at each autosave.

Every input line is stamped with `(game_turn, turn_slice)` for early divergence detection.

**Replay.** `ffh-play replay <journal> [--from <save>]`:
* re-applies the inputs through `JournalDriver` and `JournalAnswerer`;
* asserts the stamps and checkpoints;
* stops with a diff at the first mismatch.

`ffh-play journal print` renders the journal as human-readable lines (the tui idea), for bug reports.

**Take over** is a debug feature. It loads an AI-only save, which must be at a turn boundary. Then for slot p:
1. set `setup.players[p].human = true` and `slot_status = Taken`;
2. call `player_update_human(p)`;
3. set `active_player = Some(p)` and the player options;
4. set `autorun = false` and enable the log;
5. attach a `HumanController`.

The take-over itself is journaled. With no sidecar, pending events start empty; this is documented.

**New game.** Starting from `NewGame`:
1. `SetupOptions` → `GameSetup::from_options`.
2. Slot 0: `human = true`, `slot_status = Taken`, the chosen civ and leader (or `None`, which means random as in FfH), and the chosen `handicap`.
3. `Game::new`; then `g.handicap = Game::setup_handicap(..)`.
4. `map_script = FractalMapScript`.
5. Controllers: AI slots get one shared `ffh_ai::ai_controller()` (no wrapper, so later trait methods are not dropped silently); slot 0 gets a `HumanController`.
6. `autorun = false`, `active_player = Some(0)`.
7. Player options: QUICK_MOVES on, SHOW_FRIENDLY_MOVES and SHOW_ENEMY_MOVES off.
8. Log enabled; then `game_init()`.

`GAMEOPTION_ADVANCED_START` is refused, because there is no advanced-start UI.

### 2.7 Keeping AI turns fast

* **AI controllers.** The AI uses the same controller as ffh-cli. AI mission timers stay 0 (F3).
* **Per-slice overhead during `Running`** is small:
  * a status update (an atomic version bump plus `notify_all`, throttled to at most one per 20 ms);
  * a `try_recv` loop that answers queued queries and refuses commands;
  * log formatting (86 sites).
* **Views** are rebuilt only at control points and decisions.
* **A bench guard** compares AI time per turn in a bot-human game with `ffh bench` on the same seed; the budget is +10%.

### 2.8 Failures

**Panics.** On the HTTP server the game thread body runs under `catch_unwind`. On a rules panic:
* the status becomes `crashed {message, last_autosave, journal}`;
* the server stays up, so the tester can load the autosave;
* a copy-ready bug report shows the repro command `ffh-play replay <journal> --from <autosave>`.

In stdio mode, a panic prints `{"ev":"crashed",..}` and exits with code 3.

**Validation.** Everything is validated before any rules call:
* ids exist and are owned by the human;
* enum indices are in range;
* the rules' own `can_*` gates pass (as the exe's UI only offers legal actions).

Several mutators `expect` on valid input.

**Disconnected browser.** If the browser disconnects during a decision, the game thread keeps waiting. A reload long-polls the status, sees the decision and can answer it.

---

## 3. Engine-side API (library `ffh_play`)

```rust
// setup.rs
pub struct NewGame { pub seed: u32, pub opponents: usize, pub world: WorldSizeType, pub speed: GameSpeedType,
    pub handicap: HandicapType, pub era: EraType, pub climate: ClimateType, pub sea_level: SeaLevelType,
    pub options: Vec<GameOptionType>, pub disabled_victories: Vec<VictoryType>, pub max_turns: i32,
    pub civ: Option<CivilizationType>, pub leader: Option<LeaderHeadType> }
pub fn setup_lists(d: &GameData) -> Json;   // AI-playable civs/leaders, sizes, speeds, handicaps, eras, climates, sea levels, options

// session.rs
pub enum Phase { Human(PlayerId), Running, Over }
pub enum Stop { Human, Over, Stuck(String) }
pub struct SessionOptions { pub saves_dir: PathBuf, pub autosave_keep: usize, pub auto_moves_when_idle: bool,
    pub settle_limit: u32, pub journal: bool }
pub struct Session { pub g: Game, pub human: PlayerId, pub shared: Arc<SessionShared>, pub ui: Arc<Mutex<UiState>>,
    advisor: Arc<ffh_ai::Ai>, journal: Journal, messages: MessageStore, report: ReportState, opts: SessionOptions,
    auto_moves_tried: bool }
impl Session {
    pub fn new_game(ng: &NewGame, answerer: Arc<dyn Answerer>, opts: SessionOptions) -> Session;
    pub fn load(save: &[u8], sidecar: Option<&Sidecar>, answerer: Arc<dyn Answerer>, opts: SessionOptions) -> Result<Session, PlayError>;
    pub fn take_over(save: &[u8], p: PlayerId, answerer: Arc<dyn Answerer>, opts: SessionOptions) -> Result<Session, PlayError>;
    pub fn phase(&self) -> Phase;
    pub fn advance(&mut self, on_slice: &mut dyn FnMut(&Game)) -> Stop;
    pub fn execute(&mut self, cmd: &Command) -> Result<CmdOk, CmdErr>;
    pub fn query(&self, q: &Query) -> Result<Json, QueryErr>;   // QueryGuard inside
    pub fn read<R>(&self, f: impl FnOnce(&Game) -> R) -> R;      // QueryGuard for in-process readers (bot)
    pub fn pending(&self) -> Pending;
    pub fn save(&self, name: &str) -> Result<SavePaths, PlayError>;
}
pub struct SessionShared { pub pending_events: Mutex<Vec<(PlayerId, TriggeredId)>>,
    pub research_mark: Mutex<Option<(i32, i32, PlayerId)>>, pub next_decision: AtomicU32 }

// human.rs
pub struct HumanController { advisor: Arc<ffh_ai::Ai>, answerer: Arc<dyn Answerer>, shared: Arc<SessionShared> }
pub const POLICIES: &[(&str, Policy)];   // one entry per Controller method (drift test)
pub enum Policy { Deferred, Blocking, Advisor, Fixed }

// decision.rs
pub struct Decision { pub id: u32, pub player: PlayerId, pub kind: DecisionKind, pub title: String, pub text: String,
    pub options: Vec<DecisionOption>, pub advisor: Option<usize>, pub context: DecisionContext }
pub enum DecisionKind { FreeTech, KeepCity { city: CityId, can_raze: bool }, Election { selection: VoteId },
    Offer { from: PlayerId }, Religion }
pub struct DecisionOption { pub key: String, pub label: String, pub help: String, pub enabled: bool }
pub struct Pending { pub events: Vec<PendingEvent>, pub votes: Vec<PendingVote>, pub research_needed: bool,
    pub idle_cities: Vec<CityId>, pub promotions_ready: Vec<UnitId>, pub ready_groups: Vec<GroupId> }

// command.rs
pub enum Command {
    Move { units: Vec<UnitId>, x: i32, y: i32, append: bool, declare_war: bool },
    Mission { units: Vec<UnitId>, mission: MissionType, data1: i32, data2: i32, flags: i32, append: bool },
    DoCommand { units: Vec<UnitId>, command: CommandType, data1: i32, data2: i32, alt: bool, confirmed: bool },
    JoinGroup { unit: UnitId, head: Option<UnitId> },
    AutoMission { unit: UnitId },
    PushOrder { city: CityId, order: OrderType, data: i32, alt: bool, shift: bool, ctrl: bool },
    PopOrder { city: CityId, index: i32 },
    DoTask { city: CityId, task: TaskType, data1: i32, data2: i32, option: bool, alt: bool, shift: bool, ctrl: bool },
    Research { tech: Option<TechType>, shift: bool },
    UpdateCivics { civics: Vec<CivicType> },            // merged over current choices
    PercentChange { commerce: CommerceType, change: i32 },
    Convert { religion: Option<ReligionType> },
    DeclareWar { team: TeamId },
    ProposeDeal { to: PlayerId, give: Vec<TradeData>, get: Vec<TradeData> },
    Event { triggered: TriggeredId, event: EventType },
    Vote { vote: VoteId, choice: PlayerVote },
    SetOption { option: PlayerOptionType, value: bool },
    AutoMoves,
    EndTurn { force: bool },
    AutoPlay { turns: i32 },                              // FfH setAIAutoPlay (testing; journaled)
}
pub enum CmdOk { Applied { log: Vec<LogEntry>, accepted: Option<bool> }, NeedsConfirm(Confirm) }
pub enum Confirm { DeclareWar { team: TeamId }, Command { command: CommandType }, CastGlobal { spell: SpellType },
    CastWar { spell: SpellType, team: TeamId }, LoadUnit { transports: Vec<UnitId> },
    EndTurn { events: usize, votes: usize, research: bool, ready_units: usize } }
pub enum CmdErr { NotYourTurn, DecisionPending, Invalid(String), NotOwner, Gone, Cannot(String) }

// guard.rs, query.rs
pub struct QueryGuard<'a> { .. }
pub enum Query { Status, Pending, View, Map { since: Option<u32> }, Plot { x: i32, y: i32 }, Group { units: Vec<UnitId> },
    Path { units: Vec<UnitId>, x: i32, y: i32, flags: i32 }, Combat { units: Vec<UnitId>, x: i32, y: i32 },
    City { city: CityId }, Techs, Civics, Religions, Players, Trade { with: PlayerId }, Log { since: usize },
    Report, Inspect { kind: String, id: i64 }, Text, Ascii { x: i32, y: i32, w: i32, h: i32, layer: String }, Data, Saves }
```

---

## 4. Protocol

### 4.1 Envelope

The envelope is identical for HTTP `POST /api` and for stdio lines. Type references are accepted as XML type names (`"UNIT_WARRIOR"`, resolved with `d.ty`) or as integer indices; responses use indices and `/api/data` maps them back to names. **Every** type-valued field of a response (status, view, map, queries) is an integer index, including the hard-coded C++ enums (activity, attitude, mission, order, victory, ...) and map keys (mana counts are an array of `{bonus, count}`, not an object keyed by name); `null` means none. Only protocol vocabulary that is not a game type is a string: `phase`, decision `kind` and option `key`, error codes, confirm kinds. Vote choices (`PlayerVoteTypes`, not an index type) are `YES`/`NO`/`NEVER`/`ABSTAIN` (or the values -2, -3, -5, -4) or a team id (§4.5). Ids are raw `u32` (`UnitId`, `CityId`, `GroupId`, `TriggeredId`, `VoteId`, decision ids). Players and teams are `u8`. Plots are `x,y`. Integers are `i64` on the wire; a 64-bit hash goes as a hex string.

**Requests:**

```json
{"id":12,"k":"cmd","c":"move","units":[881,890],"x":15,"y":33,"append":false}
{"id":13,"k":"q","q":"path","units":[881],"x":15,"y":33}
{"id":14,"k":"answer","decision":17,"option":1}            // or {"advisor":true}
{"id":15,"k":"session","op":"save","name":"before-war"}    // new{setup}, load{name}, save{name}, take_over{player},
                                                           // debug{reveal}, quit, saves
```

**Responses:**

```json
{"id":12,"ok":true,"r":{"applied":true,"accepted":null},"log":[{"t":57,"text":"..."}],"status":{...}}
{"id":12,"ok":true,"r":{"confirm":{"kind":"declare_war","team":3}}}       // resend with "declare_war":true
{"id":12,"ok":false,"error":"not_your_turn|decision_pending|invalid|not_owner|gone|cannot|busy|no_game","detail":"..."}
```

**Stdio events** (unsolicited, one per line):
* `{"ev":"control","turn":57,"pending":{..}}` when the human gets control;
* `{"ev":"decision",..}`;
* `{"ev":"progress","turn":57,"active":[3]}` (throttled, optional);
* `{"ev":"over",..}`;
* `{"ev":"crashed",..}`.

At EOF of stdin: answer open decisions with the advisor and either quit (default) or `--on-eof end-turn`.

### 4.2 HTTP endpoints

| Endpoint | Served by | Purpose |
|---|---|---|
| `GET /`, `/app/*.js`, `/app/*.css` | HTTP thread | `include_str!` assets, or `--assets DIR` for live editing. |
| `GET /api/status` | Shared | `Status` (§4.3). |
| `GET /api/wait?since=V&timeout_ms=25000` | Shared (Condvar) | Long-poll until `status.version > V`. |
| `GET /api/view` | Shared | Cached fog-filtered view (§4.4), carrying `view_version` and `map_rev`. |
| `GET /api/map[?since=R]` | Shared | Map arrays. Full in the MVP; per-tile deltas in phase 6. |
| `GET /api/data` | Shared (built once) | Static tables. ETag is the data fingerprint. |
| `GET /api/setup` | Shared | New-game lists. |
| `POST /api` | game thread | Envelope. A missing `X-FFH: 1` header gets 403. Bound to 127.0.0.1 unless `--bind` is given. Every response is `Connection: close` and only `Content-Length` bodies are accepted. |

### 4.3 Status

```json
{"version":812,"phase":"human|running|decision|over|no_game|crashed","turn":57,"year":-2950,"human":0,
 "active_players":[3],"view_version":40,"map_rev":"9f3a11c2","ms_in_phase":12,
 "decision":{"id":17,"kind":"keep_city","title":"...","text":"...","options":[{"key":"keep","label":"Keep","help":"","enabled":true},...],
             "advisor":0,"context":{"plot":[12,30],"city":4,"unit":null,"other_player":2}},
 "over":{"winner_team":1,"victory":2},
 "crash":{"message":"...","autosave":"saves/auto/T0056.ffhsave","journal":"..."}}
```

### 4.4 View and map

**`view`** is built from the human team's perspective, or the true state with `debug.reveal`, which the UI marks clearly. Type-valued fields are indices (§4.1); the client names them from `/api/data`. In the example, with the current data: `type` 312 = `UNIT_WARRIOR`, `order` 0 = `ORDER_TRAIN`, `attitude` 1 = `ATTITUDE_ANNOYED`, `activity` 2 = `ACTIVITY_SLEEP` (fortified units sleep: C++ has no fortify activity, `MISSION_FORTIFY` sets `ACTIVITY_SLEEP`), `m` 0 = `MISSION_MOVE_TO`, `bonus` 8 = `BONUS_MANA_FIRE`; other numbers are illustrative.

```json
{"view_version":40,"turn":57,
 "me":{"player":0,"team":0,"gold":143,"gold_rate":6,"research":{"tech":21,"progress":40,"cost":120,"turns":4,"rate":9,"queue":[21,30]},
       "commerce_percent":[60,40,0,0],"flexible":[true,true,false,false],"anarchy":0,"golden_age":0,"state_religion":2,
       "civics":[..],"score":312,"armageddon":14,"mana":[{"bonus":8,"count":1},...],"stasis":{"research":0,"production":0,"spells":0}},
 "players":[{"id":1,"leader":5,"civ":3,"team":1,"met":true,"war":true,"attitude":1,"open_borders":false,
             "defensive_pact":false,"score":290,"alive":true,"contact":true}],
 "cities":[{"id":3,"owner":0,"x":12,"y":30,"name":"Taranis","size":5,"capital":true,"visible":true,
            "production":{"order":0,"type":312,"turns":3},"idle":false,"defense":20}],
 "units":[{"id":881,"owner":0,"visual_owner":0,"type":312,"x":12,"y":31,"group":402,"hp":100,"moves":60,"max_moves":120,
           "level":2,"xp":7,"promo_ready":false,"activity":2}],
 "groups":[{"id":402,"head":881,"units":[881,890],"ready":true,"missions":[{"m":0,"d1":15,"d2":33}],"automated":false}],
 "pending":{"events":[...],"votes":[...],"research_needed":false,"idle_cities":[7],"promotions_ready":[881],"ready_groups":[402,409]},
 "log_next":4211}
```

**Fog rules:**
* Own units are always shown.
* Foreign units are shown only on plots with `plot_is_visible(p, team)` and `!unit_is_invisible(u, team, false)`; their `visual_owner` respects hidden nationality.
* For foreign cities, name and owner are shown when `city_is_revealed`; size and defence only when the plot is visible.
* Unmet players are omitted.
* Log entries are shown only where `player == Some(h)` or `None`.

**`map`** is flat arrays indexed by `y*w+x`, with -1 meaning none:

| Array | Contents |
|---|---|
| `vis` | 0 = unrevealed (all other arrays are -1), 1 = fogged, 2 = visible |
| `plot_type`, `terrain`, `feature` | current values |
| `bonus` | `plot_get_bonus_type(p, Some(team))` |
| `improvement` | `revealed_improvement_type[team]` |
| `route` | `revealed_route_type[team]` |
| `owner` | `revealed_owner[team]` |
| `river` | bit 0 = `n_of_river` (south edge), bit 1 = `w_of_river` (east edge), bits 2-5 = directions |
| `city` | city id |
| `yield` | packed food/production/commerce from `plot_calculate_yield`, for revealed plots |

`map_rev` is an FNV hash of the arrays. Sizes: about 130 KB for a standard map, about 300 KB for huge.

In phase 6 the session keeps `tile_rev: Vec<u32>`, so `/api/map?since=R` returns only the tiles that changed (engine-first's delta idea).

### 4.5 Commands

Every command is validated before any rules call (§2.8).

| `c` | C++ origin | Rules calls |
|---|---|---|
| `move {units,x,y,append,declare_war}` | `selectionListMove` + DECLAREWARMOVE `OnOk` + `selectionListGameNetMessage(PUSH_MISSION)` | 1. If any `unit_get_declare_war_move(u, plot)` returns a team, reply `confirm declare_war` unless `declare_war`; if set, `team_can_declare_war` then `team_declare_war(my, t, false, None)`. 2. If `units` doesn't equal one existing group: `unit_join_group(head, None, true)`, then `unit_join_group(u, Some(head_grp), true)` for the others. 3. `group_push_mission(grp, MISSION_MOVE_TO, x, y, 0, append, true, None, None, None)`. |
| `mission {units,mission,data1,data2,flags,append}` | `selectionListGameNetMessage(PUSH_MISSION)` → `CvNetJoinGroup` + `CvNetPushMission` | Same join step; check `group_can_start_mission(grp, m, d1, d2, None, false)`; then `group_push_mission(.., manual=true, ..)`. Covers skip, sleep, fortify, sentry, heal, pillage, plunder, found, build, join, construct, discover, hurry, trade, great work, golden age, spread, lead, bombard, route-to. |
| `do_command {units,command,data1,data2,alt,confirmed}` | `handleAction` → `selectionListGameNetMessage(DO_COMMAND)` → `CvNetDoCommand` | Confirm when `commands[c].confirm_command` (delete); for COMMAND_CAST when `spells[s].global` (CONFIRMCASTGLOBAL) or `causes_war` with a target in range (the FfH/Sephi range scan in `handleAction`); COMMAND_LOAD with more than one transport (LOADUNIT picker). Then `unit_do_command(u, c, d1, d2)` for **each** selected unit. `alt` with `commands[c].all` means every unit of that type. COMMAND_AUTOMATE is hidden until automation exists (F15); COMMAND_AUTOMATE_SPELL is hidden (F16). |
| `join_group {unit,head?}` | `CvNetJoinGroup` | `unit_join_group(u, head.map(group), true)` |
| `auto_mission {unit}` | `CvNetAutoMission` | `group_auto_mission(g)` |
| `push_order {city,order,data,alt,shift,ctrl}` | `CvGame::cityPushOrder` flag rewrite, then `CvNetPushOrder` | `city_push_order(c, o, d, -1, alt, !(shift‖ctrl), shift, false)` using the rewritten flags. Checked with `city_can_train`, `construct`, `create` or `maintain` (`test_visible = false`). |
| `pop_order {city,index}` | `CvNetPopOrder` | `city_pop_order(c, i, false, false)` |
| `do_task {city,task,..}` | `CvNetDoTask` | `city_do_task(..)` for HURRY, CHANGE_SPECIALIST, CHANGE_WORKING_PLOT, EMPHASIZE, SET_AUTOMATED_*, CONSCRIPT, CLEAR_ORDERS, LIBERATE. RAZE, DISBAND and GIFT are refused (popup only). |
| `research {tech?,shift}` | `CvNetResearch` (`iDiscover == 0`) | `None`: `player_clear_research_queue`. Otherwise, if `player_can_ever_research`: if (has the tech or already researching it) and `!shift`, clear the queue; then `player_push_research(h, t, !shift)`. |
| `update_civics {civics}` | `CvNetUpdateCivics` | Merge into `IdxVec<CivicOptionType, Option<CivicType>>`; check `player_can_revolution(h, Some(&v))`; then `player_revolution(h, &v, false)`. |
| `percent_change {commerce,change}` | `CvNetPercentChange` | `player_change_commerce_percent` |
| `convert {religion?}` | `CvNetConvert` | `player_can_convert`, then `player_convert` |
| `declare_war {team}` | `CvNetChangeWar` | Met, `team_can_declare_war`, then `team_declare_war(my, t, false, None)`. Peace goes through a deal. |
| `propose_deal {to,give,get}` | diplomacy screen → `AI_considerOffer` → `implementDeal` | `player_can_contact`; every item must pass `player_can_trade_item(from, to, item, false)` with no `player_get_trade_denial`. Then `g.controller(to).consider_offer(g, to, h, give, get)`; if true, `implement_deal(h, to, give, get, false)`. Reply `{accepted}`. |
| `event {triggered,event}` | `CvNetEventTriggered` | Must be in the pending list and `player_can_do_event`; then `player_apply_event(h, e, id, true)`; remove it from the pending list. |
| `vote {vote,choice}` | DIPLOVOTE popup (`CvDLLButtonPopup::launchDiploVotePopup`) → `CvNetDiploVote` | `choice` is decoded by `names::decode_player_vote`: YES, NO, NEVER, ABSTAIN or a team id; the internal `NO_PLAYER_VOTE` (-1) and `NO_PLAYER_VOTE_CHECKED` (-6) are refused. The vote must be pending for h. Then the choice must be a button the popup offers. **Team vote** (`is_team_vote`): a team id that is alive and `is_team_vote_eligible(t, src)`; if h's team is the vassal of an eligible team V (the first in team order), only V or h's own team; ABSTAIN only when h is not such a vassal; YES/NO/NEVER refused. **Other votes:** YES, NO, NEVER only if `player_can_defy_resolution(h, src, &option)`, ABSTAIN; team ids refused. Then `cast_vote(h, id, choice)`. (A non-negative `PlayerVoteTypes` is a team id; ffh-core's `PlayerVote::player()` reads it as a player id, see P7.) |
| `set_option {option,value}` | `sendPlayerOption` | Allow-list: STACK_ATTACK, QUICK_MOVES, SAFE_AUTOMATION, NO_UNIT_CYCLING, WAIT_END_TURN. SHOW_* is refused (F3). |
| `auto_moves` | `CvNetAutoMoves` | `player_set_auto_moves(h, true)` |
| `end_turn {force}` | `CvNetTurnComplete` | The gate in §2.3, then `player_set_end_turn(h, true)` |
| `autoplay {turns}` | `setAIAutoPlay` (FfH `setDisableHuman`) | `set_ai_auto_play(n)`. Testing aid, journaled. |

### 4.6 Queries

All queries are pure, run under `QueryGuard` and follow the allow-list.

| `q` | Returns |
|---|---|
| `status`, `pending`, `view`, `map`, `data`, `setup`, `saves` | as above |
| `plot {x,y}` | Terrain, feature, river edges, yields, defence modifier, move cost, owner and culture, improvement (with upgrade progress), route, visible units with stats |
| `group {units}` | Unit details (strength, moves, HP, XP, level, race, promotions, summon duration, cargo) plus **actions**, enumerated as C++ `canHandleAction` does. Missions × candidate data come from `group_can_start_mission(.., true)` (visible) and `(.., false)` (enabled): builds with turns, JOIN specialists, CONSTRUCT buildings, SPREAD religions, LEAD targets. Commands come from `unit_can_do_command`, spells from `unit_can_cast(u, s, true/false)` with help text, promotions from `unit_can_promote`, upgrades from `unit_can_upgrade` with `unit_upgrade_price`, load targets from the plot. |
| `path {units,x,y,flags}` | Interface finder: nodes with turn of arrival, total turns, `declare_war_team`, `is_attack` |
| `combat {units,x,y}` | Attacker from `group_ai_get_best_group_attacker(grp, plot, true, false, false)`, defender from `plot_get_best_defender(plot, None, Some(h), Some(att), true, false, false)`, odds in per mille from `get_combat_odds`, plus a **CombatDetails breakdown** via `unit_max_combat_str(.., Some(&mut details))` (modifiers, first strikes, withdraw, collateral), as `setCombatPlotHelp` does |
| `city {city}` | The full city screen: plots with yields and worked or worked-by-other state, specialists, food/production/commerce breakdown, growth, happiness and health (with main causes), maintenance, buildings, religions, culture level, great-person progress, defence, garrison, queue, and the buildable list with cost, turns and greyed `test_visible` entries |
| `techs`, `civics`, `religions`, `players` | Their screens (tech tree laid out by prerequisite depth) |
| `trade {with}` | Tradeable items both ways, with denials |
| `log {since}`, `report` | Message store; turn report (§5) |
| `inspect {kind,id}` | `{:#?}` of a unit, group, city, plot, player, team or info entry. Own objects only unless debug-reveal is on |
| `text` | Compact text summary for LLM agents: status, pending, ready groups with positions, city list |
| `ascii {x,y,w,h,layer}` | ASCII map, 2 characters per plot, legend included; layers terrain, political, yield, units |

### 4.7 Static data (`data`)

Arrays per info table. Each entry is `{type, name, help?, strategy?, pedia?, ...gameplay fields the renderer and screens need}`, with texts from `d.text` kept in Civ4 markup; `markup.js` renders `[ICON_*]`, `[LINK=..]`, `[COLOR_*]` and `[NEWLINE]`. The tables:
* map: terrains, features, plot types, bonuses, improvements, routes, yields, commerces;
* units: units, unit combats, promotions, spells, builds, missions, commands;
* empire: techs, buildings, projects, processes, specialists, civic options, civics, religions;
* civilizations: civilizations, leaders;
* events and votes: events, triggers, votes;
* setup: handicaps, speeds, world sizes, eras, player options, trade items.

Size is about 1–1.5 MB, cached by ETag.

---

## 5. Front-end features

### 5.1 MVP: a whole game is playable

**Setup and files**
* **New game screen:**
  * civilization and leader, or random;
  * number of AI opponents, world size, speed, difficulty, era (first era by default), climate, sea level, seed;
  * a few game options; victory conditions.
* Load, save and the autosave list.
* Game-over screen: winner, victory type, scores.

**Map** (canvas, square tiles, north up, wrap honoured)
* Terrain colours keyed by `TERRAIN_*`, with hash-colour fallback; FfH hell terrains included.
* Hatching for hills, a ▲ glyph for peaks, feature glyphs.
* Rivers drawn on edges; roads as lines.
* Bonuses only when revealed by tech; improvements, including lairs and unique improvements.
* Culture borders from `revealed_owner`.
* Fog: dimmed when fogged, black when unrevealed.
* City boxes showing name, size, production and turns, and a capital mark.
* Unit stacks: owner colour, unit abbreviation, HP bar, moves pip, count badge.
* Selection highlight and hover tooltip (plot query).
* **Layers:** terrain, political, and yield digits (tui graft).
* Minimap, zoom, pan, centre on a unit or city.
* An offscreen terrain cache per zoom level.

**Units**
* Click to select; stack list with multi-select; group and split (`join_group`).
* Right-click, or `M` then click, to move.
* **Path preview** with per-turn markers; Shift adds a waypoint (append).
* **Hovering an enemy** shows odds plus the CombatDetails breakdown.
* **Actions panel** from the `group` query, covering:
  * missions, builds with turns, route-to;
  * great-person missions;
  * load and unload (with a transport picker);
  * upgrades with price, promotions;
  * spells, castable or greyed, with help;
  * delete, gift, wake, cancel.
* Confirmations: declare war on move, war spell, global spell, delete.
* Cycling over ready groups in `group_cycle` order.
* Civ4-style hotkeys:
  * `F` fortify, `S` sleep, `Space` skip, `W` wait/next;
  * `B` build menu, `C` centre;
  * `Enter` end turn, `Shift+Enter` force end turn.

**Cities**
* The city screen (§4.6) with click-to-work tiles and specialists +/-.
* Production queue: replace, append, insert at front, remove.
* Chooser covering units, buildings, projects (FfH rituals) and processes, with cost and turns.
* Hurry (gold or population), conscript, emphasize, automated citizens and production.
* A city list overview.

**Empire**
* **Top bar:** gold and rate, research and turns, sliders, turn and year, **Armageddon counter**, **mana counts**, anarchy and golden age, Stasis flags.
* Research chooser, queue and tech tree.
* Civics (current choice, availability, upkeep, anarchy, revolution).
* Religion (state religion, convert, holy cities).
* Commerce sliders.

**Diplomacy minimum**
* Met players: leader, civ, war or peace, open borders, pacts, attitude toward us, score.
* Declare war.
* **Propose a deal** to an AI: peace treaty, open borders, defensive pact, permanent alliance, gold, gold per turn, techs, maps, resources and cities.

**Popups and decisions**
* A generic blocking modal: free tech, keep/abandon city, election, offer, religion, each with an "AI default" button.
* A queue of **deferred popups** at each control point, in C++ order:
  1. events (trigger text, options with `enabled` and an effect summary computed from `EventInfo`: gold, techs, units, buildings, promotions, happiness/health, culture, attitude);
  2. votes;
  3. research;
  4. idle-city choosers.

**Turn flow**
* End turn, with the gate dialog.
* A progress line during AI turns: active player and elapsed ms.
* A **turn report** at each new turn, built by diffing a fog-filtered snapshot taken at `end_turn` against the next control point. It covers: units lost, gained or promotable; cities founded, lost, captured, grown, starving or in disorder; builds completed; techs gained; wars and peace; enemies sighted near our cities; Armageddon thresholds crossed; borders lost. Drained log entries are merged in. This makes up for F14.

**Messages**
* A log grouped by turn.
* Each command reply carries the lines it produced, for example combat results.

**Rules-testing tools**
* Raw inspector.
* Debug reveal-all, clearly marked.
* AI autoplay for N turns.
* Take over a slot from an AI save.
* Journal and save paths shown.
* A crash report with the replay command.

**Agent transport (stdio)**
* Everything above as requests, plus `text` and `ascii`.

### 5.2 Deferred

* **Automation** (worker, network, city, explore, missionary) and **FfH AI-controlled human units.** These need ffh-ai `update_automated_group` (phase 7). Until then:
  * the buttons are hidden;
  * AI-controlled units sit idle, and the top bar shows an "N AI-controlled units idle" warning;
  * this is listed in docs/HUMAN.md.
* **Human-only hooks that have no hook in core yet** (phase 7):
  * the Kuriotates city/settlement choice (a human Kuriotates city stays at radius 2 and is not a settlement, as documented in PORTING.md);
  * the raze popup's gift button;
  * telling disband apart from raze;
  * the Mercurian/Infernal "take control" choice (`reassignPlayer`).
* **More diplomacy:**
  * AI-initiated offers, demands and tribute to humans (needs ffh-ai);
  * counter-proposals ("what would you want?");
  * vassal and capitulation flows;
  * colony split;
  * cancelling deals (`deal_kill`; nice to have).
* **Full help texts** (`CvGameTextMgr` is not ported). The MVP shows XML Help, Strategy and Pedia text plus raw stats and the inspector.
* **Advanced start; less common interface modes:** range attack, air strike, recon, paradrop, airlift, nuke. Also espionage (locked off), scenarios, worldbuilder, hot seat, network play.
* **Presentation:** watching AI moves (never through SHOW_*), sounds, art, graphs, replays, the victory progress screen beyond scores, and localization.
* **Spectator mode** (an AI-only game with a god view and pause/step) is phase 6 optional.

---

## 6. Files, modules and owners

### 6.1 What this workstream creates and edits

**New files, owned by this workstream** (`crates/ffh-play`):

```
Cargo.toml                 lib + [[bin]] ffh-play; deps ffh-data, ffh-core, ffh-ai (path)        ~20
src/lib.rs                 modules, re-exports                                                    ~60
src/main.rs                serve | agent | bot | soak | replay | journal print | bench; kinds | names | lookup (P0) ~220
src/json.rs                Json enum, escaping writer, recursive-descent parser, typed getters    ~400
src/names.rs               type name <-> index for every table and hard-coded enum (NAMES arrays), text lookup ~220
src/setup.rs               NewGame -> Game, controllers, active_player, options, take_over, setup lists ~250
src/human.rs               HumanController (§2.3), POLICIES, research-site rule, Answerer trait + impls ~420
src/decision.rs            Decision builders, Pending derivation, event effect summaries         ~350
src/session.rs             Session, phase, advance, settle, execute, auto moves, drain, autosave, watchdog ~600
src/command.rs             Command decode/validate/apply: CvNet ports, selectionList*, cityPushOrder,
                           handleAction confirms, deals                                           ~800
src/guard.rs               QueryGuard                                                              ~130
src/journal.rs             JSONL writer/reader, stamps, checkpoints, JournalDriver, pretty printer ~250
src/save.rs                save bundle + sidecar, autosave ring, name sanitizing                  ~150
src/report.rs              fog-filtered snapshot + diff, MessageStore                              ~280
src/view/fog.rs            the only visibility predicates                                          ~120
src/view/{mod,map,player,units,city,diplo,data,text}.rs   views and queries (§4.4-4.7)            ~2,200
src/transport/shared.rs    Shared {status+Condvar, cached view/map/data, rx}, wait()              ~150
src/transport/http.rs      std TcpListener, thread per connection, router, assets, X-FFH, 403/409/503 ~380
src/transport/server.rs    game-thread loop for HTTP, ChannelAnswerer, catch_unwind, crash status ~300
src/transport/stdio.rs     JSONL loop on the game thread, StdioAnswerer, EOF policy                ~250
src/bot.rs                 scripted human (protocol-level), seeded private RNG                     ~380
src/soak.rs                AI-as-human driver                                                      ~200
assets/index.html ~120, assets/style.css ~450
assets/js/api.js ~200, state.js ~250, markup.js ~120, map.js ~1,000, units.js ~500, city.js ~600,
          screens.js ~700, modals.js ~400, main.js ~250                                           ~4,600 web total
assets/test/*.mjs          node --test: markup, wrap-aware tile maths, overlay layout, api smoke   ~200
tests/*.rs                 §8                                                                      ~1,600
tools/web-check.sh         node --check + node --test, skipped if node is absent                   ~20
tools/web-smoke.mjs        headless-Chromium smoke test against `ffh-play serve` (P5; F21), skipped without a browser ~150
```

Totals: about **7.5k lines** of Rust library and binary code, **1.6k** of Rust tests, and **4.6k** of JS/CSS/HTML.

**Shared files edited by this workstream** (small):
* **Root `projects/ffh2-rs/Cargo.toml`:** add `"crates/ffh-play"` to `members`. This is the only manifest edit.
* **Docs:**
  * new `docs/HUMAN.md`: how to play, protocol, decision policy, journal and replay, known gaps;
  * a "Human play" section in `docs/ARCHITECTURE.md`;
  * a crate row in `README.md` and in the `projects/ffh2-rs/CLAUDE.md` layout;
  * `docs/PORTING.md` "Deliberate deviations": UI-level human timing, Kuriotates still unanswered, no gift button.
* **`.gitignore` (repo root):** add `projects/ffh2-rs/saves/`. The default `--saves` directory is `./saves`.

**Not edited in the MVP:**
* `crates/ffh-core`, `crates/ffh-ai`, `crates/ffh-cli`, `crates/ffh-stress`, `controller.rs`.
* Exception: **rules bugs** that the soak finds in core's human branches. These are fixed as separate small commits, each with an ffh-core regression test. They are rules files, not the contested `controller.rs`; the commit message says what was found.

---

## 7. Implementation phases

Each phase is independently testable and committable. `cargo build --release && cargo test --release` must pass at each one.

**P0 — skeleton (about 0.7k).**
* Scope:
  * the crate with lib and bin;
  * `json.rs`;
  * `names.rs`;
  * the workspace member line.
* Tests: JSON round-trip and escaping; name resolution for every info table and hard-coded enum.
* **As built (done).**
  * `json.rs` (std only). `Json { Null, Bool, Int(i64), Float(f64), Str, Arr, Obj(Vec<(String, Json)>) }`: objects keep insertion order, so output is deterministic (journal byte-equality). The parser is strict RFC 8259 and, because its input comes from clients, also rejects **duplicate keys** and nesting deeper than 128; a lone surrogate escape decodes to U+FFFD; integer literals that fit are `Int`, other numbers `Float`. Integers are `i64`: there is no `u64` conversion (a value above `i64::MAX` would come back as a lossy float; hashes go as hex strings), and `usize` converts within `0..=i64::MAX`. The writer escapes `"`, `\`, control characters, U+2028/U+2029; non-finite floats become `null`. Builders: `jobj!`, `jarr!`, `Json::set/with/push`, `Json::idx/opt_idx` (type index, `null` for none), `From` for integers, strings, `Option`, `Vec` and the ffh-core entity ids. Typed getters for command decoding: `req::<T>`, `opt::<T>` (absent or `null` = none), `get_or`, `req_str`, `req_arr`, ... via `FromJson` (integers range-checked; entity ids; `Vec<T>`), all returning a `FieldError` that names the field ("field `units`: expected ..., got ..."). `to_pretty()` for people.
  * `names.rs`. `Named` trait (kind name, count, name, lookup, text key) for all **61 info tables** (`ffh_data::INFO_FILES`), the **6 GlobalTypes lists** without a table (flavors, art styles, city sizes, contacts, diplomacy powers, directions) and the **15 hard-coded C++ enums** that have `NAMES` but no XML (activity, area AI, cardinal direction, control, feat, game state, interface message, mission AI, order, player option, plot, probability, task, tradeable item, war plan). `KINDS` lists them with type-erased accessors (for `/api/data`, `inspect`, the CLI). `decode::<T>` accepts a type name or an index in range; `decode_opt` also takes `null`/`-1` as none; `field`, `opt_field`, `list_field` decode object members. `PlayerVoteTypes` (not an index type) has `decode_player_vote`, which accepts only what the C++ DIPLOVOTE popup can send: `YES`/`NO`/`NEVER`/`ABSTAIN` (C++ or short name, or the values -2, -3, -5, -4) or a **team id** in `0..MAX_CIV_TEAMS` (a non-negative `PlayerVoteTypes` is a team, used only in team votes); the internal `NO_PLAYER_VOTE` (-1) and `NO_PLAYER_VOTE_CHECKED` (-6) are refused. Which choices a given vote offers is the P1 `vote` command's check (§4.5). `voted_team` reads a vote value as a team. Type names are unique across all kinds (tested), so a bare name is never ambiguous.
  * **Text.** The vendored text (`data/text/en.tsv`) holds FfH2's own text file only; descriptions inherited from BtS (e.g. `TXT_KEY_UNIT_WARRIOR`) have no text. `names::display` therefore falls back to a humanized type name (`UNIT_WARRIOR` → "Warrior"). Vendoring the BtS text would fix it (a data change, outside this crate). Otherwise `display` is the **whole** description with Civ4 markup kept (`VOTE_LIBERTY` → "Global Civic: Liberty"), not ffh-data's `Text::name`, which cuts at the first ':' as a "gendered forms" separator: FfH2's English text has no forms (no `<Text>`/`<Gender>` in an English entry), and its 11 descriptions with a ':' are all prose. A description with an unfilled placeholder (the two Somnium memories, `%D1: "..."`) also falls back to the type name.
  * Binary: `ffh-play kinds`, `ffh-play names [KIND] [--pretty]`, `ffh-play lookup NAME|KIND:INDEX ..` (debugging aids; the §6.1 subcommands arrive with their phases).
  * Tests: `json.rs` unit tests (round trip compact and pretty, escaping, numbers, strings and surrogates, structural errors with positions, duplicate keys, depth limit, typed getters and their messages); `tests/names.rs` (every kind × every index: name, lookup, decode by name / index, out-of-range and wrong-type errors, display; coverage checked against `INFO_FILES`, the `info_ids!` list in registry.rs and every `NAMES` array in enums.rs, so a new table or enum without a `KINDS` entry fails; registry names equal the hard-coded `NAMES` where both exist); `tests/cli.rs` (binary smoke test).

**P1 — session core, in-process API only (about 3.2k plus 0.8k tests).** This is where the semantics get pinned down.
* Scope:
  * `setup.rs`, `human.rs` (all policies, Answerer, research-site rule), `decision.rs` (Decision, Pending);
  * `session.rs` (phase, advance, settle, execute, the AutoMoves rule, log drain, autosave, watchdog);
  * `command.rs` (all commands except `propose_deal`);
  * `guard.rs`, `journal.rs`, `save.rs` (sidecar);
  * a minimal bot that reads `&Game` through `Session::read`.
* Tests: T1–T6 and T9 (§8). This phase decides every semantic question; the rest is presentation.

**P2 — flush the human branches (about 0.6k).**
* Scope:
  * `soak.rs` and `ffh-play soak --seeds A..B --sizes .. --civs .. --options ..`;
  * invariant checks every 5 turns, through `#[path]` reuse of ffh-stress `check.rs`;
  * crashes caught per game, printing the repro command.
* Run before any person plays:
  * at least 2 seeds × {tiny, small, standard} × {Kuriotates, Infernal, Sheaim, Mercurian, Bannor, Calabim} × {default, raging barbarians, aggressive AI}, 300 turns each;
  * also on the checked build (`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`).
* Fix or document what it finds (§6). Exit: the matrix is clean.

**P3 — views and queries (about 2.5k plus 0.4k tests).**
* Scope:
  * `view/*`;
  * the turn report;
  * `text` and `ascii`;
  * `propose_deal` and the `trade` query.
* The bot switches to the protocol-level `Query` and `Command` API.
* Tests: T7 (fuzz, now that legal actions can be enumerated), T8 (fog), T10 (noisy client).

**P4 — transports (about 1.1k plus 0.3k tests).**
* Scope: `stdio.rs` (`ffh-play agent`), `http.rs`, `server.rs`, `shared.rs`, long-poll, and a placeholder `index.html`.
* Tests: T11 (agent transcript), T12 (HTTP end to end, including the decision raised inside a command).
* After P4, an LLM or script can play a whole game headless.

**P5 — browser MVP (about 2.6k JS).**
* Scope:
  * map, selection, move and attack with odds and path;
  * found, build, fortify, skip;
  * production and research choosers;
  * the decision modal and the deferred-popup queue;
  * end turn with the gate;
  * log and turn report;
  * new, save and load;
  * game over and crash screens.
* Tests: `tools/web-check.sh` (syntax plus pure-module tests), a node smoke client that drives `serve` for 20 turns through `fetch`, and a **headless-Chromium smoke test** (F21; `tools/web-smoke.mjs`, Playwright if its package resolves, else `chrome --headless`): load the page against `ffh-play serve`, fail on console errors or failed requests, start a game, select and move a unit, choose production and research, end a few turns through the UI, and save screenshots for review. Skipped when no browser is found.
* Also a manual checklist for the human tester in docs/HUMAN.md.

**P6 — browser complete (about 2k JS plus 0.3k Rust).**
* Scope:
  * city screen;
  * civics, religion, sliders;
  * diplomacy and deals;
  * tech tree;
  * spells, promotions, upgrades, great-person and load/unload actions;
  * hotkeys and cycling;
  * yield and political layers;
  * inspector, debug reveal, autoplay, take-over;
  * map tile deltas;
  * optional spectator mode.
* Test: T13 (performance).

**P7 — coordinated follow-ups.** Each one needs its owner's agreement and is its own commit.
* **ffh-ai owners** (also the "improve the AI" fallback task):
  * `Controller::update_automated_group` for AUTOMATE_BUILD, NETWORK, CITY, EXPLORE and RELIGION, and for FfH AI-control groups (no automate type, so it falls back to the military/army logic). It must always leave the group a mission (SKIP as fallback). Place it in a new `src/automation.rs` plus a one-method impl in `lib.rs` to keep the conflict surface small. Then un-hide the automate buttons.
  * Optional AI→human offers (peace when losing, tech trades) through `g.controller(h).consider_offer(..)`, which `HumanController` already turns into a modal.
* **ffh-core hooks**, each a defaulted Controller method plus one call site and a policy entry:
  1. `ai_choose_research` for the economy.rs:1262 site (default: `choose_research`). Removes the research-mark rule.
  2. `disband_acquired_city` for `disband_city_popup` (default: `raze_conquered_city`).
  3. `confirm_settlement(g, p, c) -> Option<bool>` for Kuriotates (already in PORTING.md next steps).
  4. The raze gift option.
  5. Election deferral (keep the selection when `None`).
  6. Only if testers find timing differences: engine-first's opt-in C++ popup queue (`takes_popups`, `Player::popups`, save format bump).
* **Notes for other owners** (found in the P0 review; no behaviour change, not edited here):
  * ffh-core `structs.rs`: `PlayerVote`'s doc ("a player id"), `PlayerVote::for_player` and `PlayerVote::player()` treat a non-negative value as a player; in C++ it is a `TeamTypes` (team votes only: `CvGame::castVote` compares it with `getTeam()`, and votes.rs's own `team_vote` builds it from a `TeamId`). ffh-play uses `names::voted_team`.
  * ffh-data `Text::name` cuts a text at the first ':' as FfH2's "gendered forms" separator, but English entries have no forms, so it shortens prose: ffh-core's vote option texts (votes.rs `kVoteOption.szText`, "Election cancelled: ...") show "Global Civic" for `VOTE_LIBERTY` and drop the effect of 8 other votes ("Single Currency" for "Single Currency: +1 Trade Route ..."). ffh-play's `names::display` uses the full text.

---

## 8. Automated tests

All run with `cargo test --release -p ffh-play`. None needs node or the network. Long variants are `#[ignore]` or CLI subcommands.

1. **T1 — scripted bot game** (`tests/bot_game.rs`; also `ffh-play bot`). The bot drives slot 0 only through `Session` commands and queries, with a private seeded RNG. Its policy:
   * research the cheapest tech; production rotation worker → warrior → settler → buildings;
   * found cities when `MISSION_FOUND` is possible, otherwise move settlers to random reachable plots;
   * workers take the first build;
   * fortify defenders; attack adjacent enemies at odds of at least 60% (exercises capture and the keep/raze modal);
   * cast a random castable spell sometimes; take promotions; revolt when a civic unlocks;
   * declare war on a met AI around turn 80 and propose peace around turn 120;
   * answer decisions and deferred popups with a seeded mix of first option, advisor and random;
   * end the turn, sometimes with `force`.

   It runs 150 turns, WORLDSIZE_SMALL, 3 AIs, seeds 1–3.

   Assertions:
   * no panic and no `Stuck`;
   * at least 2 cities founded, at least N techs, at least 1 decision and at least 1 deferred popup answered;
   * `check(&g)` every 5 turns;
   * `num_game_turn_active <= 1`.
2. **T2 — determinism.** Two runs of the same seed must give identical `calculate_sync_checksum` every turn and byte-identical final saves.
3. **T3 — mid-turn save and resume.**
   * Save at control points on turns 10, 37 and 61, including after a few commands, after an answered blocking decision, and with a deferred event pending (sidecar).
   * Save at the next turn boundary.
   * Load each with fresh controllers and continue with the same bot. Checksums and later saves must equal the uninterrupted run.
   * Also check `save(load(save)) == save`.
4. **T4 — journal replay.** Replay T1's journal from the header and from a mid-game checkpoint; the final saves must be byte-identical and the stamps must match. A deliberately corrupted journal must be detected at the first mismatched stamp.
5. **T5 — AI-as-human soak smoke** (the short CI variant of P2). Two seeds × 150 turns:
   * `AdvisorAnswerer`;
   * at each control point: idle cities via `advisor.choose_production`, research, pending events and votes answered through the commands with the advisor's choice, then `advisor.play_turn(g, h)` repeated while it moves something (at most 20 times), then `end_turn`;
   * invariants checked.
6. **T6 — decision semantics against C++.** Uses test-setup games built with rules APIs:
   * **Capture.** A human attack captures a city. The keep/raze `Decision` is raised *inside* `execute`, and `city_choose_production` is not called before the answer. With keep, the city is idle and in `pending.idle_cities`.
   * **Research, case 1.** Empty research after turn 4, then end turn: this turn's beakers go to the advisor's pick (`player_do_research` path), the queue is empty afterwards, and the next control point has `research_needed`.
   * **Research, case 2.** With research set, the tech completes and the research-next call is deferred (no tech chosen silently).
   * **Events.** An event is pending (not applied) after the trigger. It applies through `event` with `update_trigger`. Options are re-filtered at the control point. A forced end turn drops it from the list.
   * **Votes.** An unanswered vote gets the extra turn and then abstains. An answered vote calls `cast_vote`.
   * **Free techs.** A free tech blocks, and two free techs give two decisions.
   * **Autoplay.** With autoplay, every method delegates to the advisor.
7. **T7 — fuzz.** A seeded mix of well-formed commands (from the enumerated actions) and malformed or foreign-owned ones, over 100 turns. No panic. Each invalid command gets `invalid`, `not_owner`, `gone` or `cannot`, and **save bytes before == after**.
8. **T8 — fog.** Every turn of T1, for every unit, plot and city in the view and the map:
   * revealed or visible per the §4.4 rules;
   * no invisible unit, no unrevealed bonus, no unmet player.

   Targeted cases: an enemy stack hidden in fog, and an FfH hidden unit next to ours. Also checked over JSONL output.
9. **T9 — controller drift.** Parse `include_str!("../../ffh-core/src/controller.rs")`, collect the `fn` names inside `pub trait Controller`, and compare them with `human::POLICIES`. The test fails when another agent adds a hook without a human policy. It is brittle by design: it fails loudly.
10. **T10 — purity, noisy client.** Variant of T1: after every command, at every control point and inside every blocking decision, issue every allow-listed query:
    * view, map, all group details and actions;
    * 5 paths and 5 combat previews;
    * every city screen;
    * techs, civics, trade, inspect, text, ascii.

    Checksums and saves must equal the quiet run. A unit test also builds a city screen (which fills rank caches, F11) and asserts `save()` is byte-identical before and after.
11. **T11 — agent transport.** Spawn `env!("CARGO_BIN_EXE_ffh-play") agent` with a scripted stdin:
    * every output line is valid JSON;
    * the event order is `control`, then `ok`, …, `decision`, `answer` accepted, then `control`;
    * `cmd` gets `decision_pending` while a decision is open;
    * the EOF policy applies.
12. **T12 — HTTP end to end.**
    * Serve on `127.0.0.1:0` in a thread; a `std::net::TcpStream` client fetches `/`, `/api/data` and `/api/setup`, starts a new game, and plays 10 turns with commands and long-polls.
    * Re-entrancy: on two connections, a pending `cmd` (a capture) while `/api/wait` shows `phase=decision`; `answer`; the `cmd` then completes.
    * A POST without `X-FFH` gets 403; path traversal in save names is rejected.
13. **T13 — performance** (`#[ignore]`, and `ffh-play bench`):
    * AI time per turn in a bot-human game is within 10% of `ffh bench` on the same seed;
    * command latency (execute + settle + view) is under 50 ms on a standard map at turn 200;
    * a view build on a huge map is under 10 ms;
    * QueryGuard overhead is under 1 ms.
14. **Optional JS** (`tools/web-check.sh`): `node --check assets/js/*.js` and `node --test assets/test/*.mjs`.
15. **Optional browser smoke** (`tools/web-smoke.mjs`, F21): headless Chromium against `ffh-play serve`, as in P5; screenshots for review. Not part of `cargo test`.

---

## 9. Risks and mitigations

| Risk | Mitigation |
|---|---|
| **About 280 human-only rules branches have never run**: keep/raze/disband popups, human research paths, the human-team governor, human mission timers, `active_player` branches (quick moves, `getDeclareWarMove` visibility, `setDisableHuman`), and AI-only `isHuman()` spell checks evaluated for a human. | P2 soak over the matrix (also on the checked build) before anyone plays. T1/T5 in CI. `catch_unwind` plus autosave plus journal gives deterministic repro. Core fixes ship as separate commits with regression tests. |
| **Reads perturbing determinism**: saved RefCell finders, the `ai_bonus_value` memo, and **rank caches filled by happiness displays** (F11). | Session-owned interface finder; allow-list; `QueryGuard` snapshot/restore on every read (including the bot, Answerer modals and report snapshots); T10 and the save-bytes unit test. |
| **Research double call** (F5): asking twice, or the answer wiped by the forced clear. | Research-site rule with a slice-scoped mark; T6 covers both paths; the phase-7 `ai_choose_research` hook removes the heuristic. |
| **Deferred events lost on load** (they are not in `Game`). | Session sidecar (`.ffhsess`), as C++ saves `m_listPopups`; T3 saves with an event pending. Take-over of foreign saves starts with an empty list (documented). |
| **Deadlock or re-entrancy** between a command and a decision raised inside it. | One receiver, no lock held across ffh-core calls; T6 (in-process) and T12 (over two HTTP connections). |
| **The `Controller` trait changes** under concurrent edits. | Every method implemented explicitly; T9 drift test; docs/HUMAN.md checklist ("a new hook needs a human policy: deferred or blocking"). |
| **Mid-turn resume depends on ffh-ai cache discipline**, which is being edited concurrently. | Human in slot 0; turn-boundary autosaves (already guaranteed); T3 in CI catches regressions; the contract is stated in ARCHITECTURE.md. |
| **Timing deviations** for blocking decisions. | Keep/raze is immediate in C++ too. A free tech or an election is answered during processing rather than at the human's next turn start. Documented in PORTING.md; phase-7 hooks or the popup queue fix it if testers care. |
| **Automation and FfH AI-controlled units idle** (F15). | Automation is hidden; an "AI-controlled units idle" warning is shown; phase-7 request to ffh-ai (also the fallback "improve the AI" task). |
| **No AI-initiated diplomacy**: a human at war must propose peace. | `propose_deal` with a peace treaty; the `consider_offer` modal is already wired. |
| **About 4.6k lines of hand-written JS**, checked by agents only headless (F21). | The engine computes everything (legal actions, odds, turns, fog), so the client is a renderer. ES modules with one concern each; node syntax and pure-module tests; a node smoke client; a headless-Chromium smoke test with screenshots (console errors fail it); the JSONL agent exposes the same capabilities for headless verification; `--assets` live-edit mode for the human tester. |
| **Thin messages and help** (F14; `CvGameTextMgr` not ported). | Diff-based turn report; event effect summaries; XML help and pedia texts; inspector. Add `log.msg` sites upstream incrementally (no rules effect). |
| **Settle or auto-moves spinning on a stuck group.** | `settle` capped at 10k slices; AutoMoves at most once per control episode; the 200k-slice watchdog with ffh-cli-style diagnostics. |
| **Fog leaks** (global log lines; views built mid-rules). | A single `fog.rs`; log filter `player == h or None`; T8; debug-reveal is explicit and visibly marked. |
| **Security of a local server.** | Binds 127.0.0.1 by default; `X-FFH` header against cross-site POSTs; sanitized save names; `Content-Length` bodies only. |
| **Browser disconnects during a decision.** | The game thread keeps waiting; status is published, so a reload resumes the modal. |

---

## 10. What docs say after this work

**docs/PORTING.md**, "Deliberate deviations", new "Human play" items:
* Human popups whose rules answer can be deferred are answered at the next control point, as in C++: production, events, diplomatic votes, the research popup.
* Free tech, keep/raze/disband and elections are answered during the rules call that raises them.
* The keep/raze and disband popups share one modal.
* There is no gift button.
* Kuriotates settlements are still not offered to humans.
* Commands are applied only when no human group is busy.

**docs/ARCHITECTURE.md**, a "Human play (ffh-play)" section:
* the game-thread model;
* the decision policy;
* the QueryGuard purity contract ("every read of `&Game` outside rules goes through a guard");
* the save bundle and sidecar;
* the journal.

**docs/HUMAN.md:**
* how to run: `ffh-play serve`, then open `http://127.0.0.1:8642`; and `ffh-play agent`;
* keys;
* the protocol reference;
* known gaps;
* the tester checklist;
* how to file a bug (autosave plus journal plus `ffh-play replay`).

---

## 11. Disagreements resolved

1. **How decisions are answered.**
   * The options were: blocking everything (web), blocking plus queueing (tui), or a core popup queue (engine-first).
   * **Resolution:** a hybrid with no core change.
     * *Defer* exactly where the port's "no answer" already equals an unanswered C++ popup (verified F7, F8, plus the production no-op and the research popup): this removes the main timing deviation that judges 1 and 2 flagged.
     * *Block* only where "no answer" loses information (F6, F9, F10).
   * Engine-first's queue becomes optional phase 7, because it edits `controller.rs`, six audited rules files and the save format, and conflicts with the stated constraint that the rules call Controller methods synchronously.
2. **Threading.**
   * The options were: single thread (tui), game thread plus HTTP threads (web), or never blocking (engine-first).
   * **Resolution:** one game thread owns `Game`. In-process, bot, soak and stdio modes have no extra threads. HTTP adds threads only at the transport edge. Blocking decisions are served re-entrantly by an `Answerer`, which also answers queries.
3. **Path previews.**
   * tui and engine-first proposed swapping the game's path finder and restoring it; web proposed a session-owned finder.
   * **Resolution:** session-owned interface finder (F12), plus a QueryGuard restoring the game's finders as a safety net.
4. **Purity enforcement.**
   * **Resolution:** all of: allow-list (web), save-bytes test (tui and engine-first), and the new QueryGuard. The guard is needed because the city rank caches are filled by `&self` happiness functions (F11), which none of the three designs caught.
5. **Advisor instance.**
   * tui shared one `Arc` between the AI players and the human's advisor; web used a separate instance.
   * **Resolution:** separate, so the advisor cannot perturb the AI players' caches.
6. **Human auto-cast port** (engine-first).
   * **Resolution:** dropped. No FfH2 0.41 spell has `bAllowAuto` (F16); the toggle is hidden instead.
7. **`keep_captured_unit` / `accept_unit`.**
   * web used the advisor; engine-first returned `true`.
   * **Resolution:** `true`. The rules short-circuit on `human`, and C++ humans keep or accept.
8. **Settle policy.**
   * web ran zero or more updates; tui exactly one plus advance.
   * **Resolution:** at least one `update()` (the exe's next frame, so the governor reacts), then until no human group is busy. Deterministic.
9. **Commands while deferred popups are open.**
   * Judge 2 asked for end-turn gating; engine-first allowed everything.
   * **Resolution:** commands are allowed. The client presents the popups modally. `end_turn` needs `force` while events, votes or research are unanswered, with the C++ consequences of not answering (§2.3).
10. **First milestone.**
    * The options were: browser (web), REPL (tui), or headless API (engine-first).
    * **Resolution:** session semantics and the soak first (P1–P2), then headless JSONL (P4), then the browser (P5).
    * The user expects to test later, so verifiability comes first, and the browser is the human-facing deliverable.
    * There is no TUI: its value for agents is covered by the `text` and `ascii` queries.
11. **Journal format.**
    * **Resolution:** JSONL using the protocol's own JSON with `(turn, slice)` stamps (engine-first), plus `journal print` for human-readable lines (tui).
12. **Map transfer.**
    * **Resolution:** full arrays with `map_rev` in the MVP (web); per-tile revision deltas in phase 6 (engine-first).
13. **Crate name.**
    * **Resolution:** `ffh-play`, because it hosts both the browser and the agent transports.
