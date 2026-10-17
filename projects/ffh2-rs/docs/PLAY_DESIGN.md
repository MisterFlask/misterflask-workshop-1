# ffh-play: human play for ffh2-rs (final design)

This is the design of `crates/ffh-play`, implemented phase by phase (§7); the status table below says what exists. Every claim marked *verified* was checked read-only against `projects/ffh2-rs` at commit 5ee3be6 (branch ccr-17ac9b54-lbgjpn) and against the C++/Python under `/home/user/ref/ffh2`.

**Implementation status.**

| Phase | Status | Notes |
|---|---|---|
| P0 skeleton | done | `json.rs`, `names.rs`, `ffh-play kinds / names / lookup`; see "As built" under P0 in §7. |
| P1 session core | done | `session.rs`, `human.rs`, `decision.rs`, `command.rs` (+ `selection.rs`), `guard.rs`, `setup.rs`, `journal.rs`, `save.rs`, `bot.rs`, `view/fog.rs` (the log's fog rules); `ffh-play bot / replay / journal print`; T1–T6, T9. See "As built" under P1 in §7. |
| P2 soak | done | `soak.rs` + `soak/batch.rs`, `ffh-play soak run / soak`; the session follows the human across slots (F23); the matrix ran clean on the release and checked builds; the P2 review's fixes, and an ffh-core assertion that longer games reach on the checked build (reported; relaxed as a C++ quirk by the rules fixes). See "As built" under P2 in §7. |
| P3 views and queries | done | `query.rs`, `view/{mod,fog,map,units,city,player,diplo,data,text}.rs`, `report.rs`; `Session::query`, debug reveal, queries inside decisions (`Answerer::ask_with`); `propose_deal` and the trade tables; the bot plays through the protocol; T7, T8, T10. See "As built" under P3 in §7. |
| P4 transports | done | `transport/{mod,stdio,http,server,shared}.rs`, `assets/index.html` (a placeholder page); `ffh-play agent` (JSONL) and `ffh-play serve` (HTTP, long-poll); T11, T12. After P4 a script or an LLM can play a whole game headless. See "As built" and "P4 review fixes" under P4 in §7. |
| P5 browser MVP | done | `assets/index.html`, `assets/app/*.js` and `style.css` (vanilla ES modules, canvas map), embedded in the binary; `src/stage.rs` (`ffh-play stage`); `tools/web-check.sh`, `tools/web-api-smoke.mjs`, `tools/web-smoke.mjs` (headless Chromium), `assets/test/*.test.mjs`, `tests/web.rs`; the manual checklist in docs/HUMAN.md. A human can play a whole game in a browser. See "As built" and "P5 review fixes" under P5 in §7. |
| P6 browser complete | done | `assets/app/{city,empire,diplo,hotkeys,tools}.js` and the rest of the client (city screen, tech tree, civics, religion, sliders, diplomacy and trade, Civ4's hotkeys and group cycle, the political and yield layers, inspector, debug reveal, autoplay, take-over, the spectator mode); map tile deltas (`view/map.rs` `MapTracker`); `view/hotkeys.rs`; `src/bench.rs` (`ffh-play bench`, T13). See "As built" and "P6 review fixes" under P6 in §7. |
| Art client | done | The browser client draws with the original pixel art of `web/art` (docs/ART.md): `build.rs` and `src/transport/art_files.rs` embed its modules and sprite bundle, served at `/art/`; `assets/app/look.js` (loading, the ui set's chrome, bitmap type, icons, heraldry), `artmap.js` (the map: a 1x chunk cache drawn by `artworker.js`, blown up to the zoom's integer scale, units, cities, marks, path, layers, minimap), `chunks.js`, `stacks.js` (tested); cities in the view carry `walls`; `?renderer=flat` (or Tools → Options) keeps the P5 renderer. See "The look" and "The map" in docs/HUMAN.md. |
| P7 | not started | Each item needs another crate's owner (§7 P7). |

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
| F23 | FfH moves the human player to another slot: `CvInitCore::reassignPlayerAdvanced` (the Impersonate Leader spell for 5–14 turns and the end of its timer, the "switch civilizations" event, the High to Low challenge at turn 50). It swaps the slots' human flag, handicap, options and (here) controllers, and moves the active player (CvInitCore.cpp:474-482; ffh-core's `reassign_player_advanced` did not until the rules fixes, and the session moved it). | ffh-core player/turn.rs, game/turn.rs, ffh/spells_a.rs, ffh/random_events_a.rs | The session follows the human after every rules call (P2); before, the old slot's turn ended as an AI's and the new human slot's never did (`Stuck`). The log since the last drain goes to the old slot after a command that moved the human (the move is its last act); after an `update()` that did, the earlier slices' entries go to the old slot and the update's to the new one (High to Low and the Impersonate timer move it before AI turns of the same update); a P7 marker would split it exactly. |
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
    self.auto_moves_tried = false;                      // the human acted: a new control episode (reset before
                                                        // applying, so an autosave in the settle records it)
    command::apply(&mut self.g, h, &plan);              // CvNet ports; may raise blocking decisions
    self.settle(h);                                     // >= 1 update(), then while a human group is busy (<= 10_000)
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
| events | `SessionShared.pending_events` for h, checked **once, when shown** at a control point (launchEventPopup): the triggered data exists and ≥1 `player_can_do_event` option, else the entry is dropped unshown. A shown popup stays open (C++ popups are modal, their buttons fixed at launch) until answered, closed by a forced end turn, or its data is gone; its options are enabled as `canDoEvent` says at each moment | `event {triggered, event}` | `player_apply_event(h, e, id, true)` |
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

*As built (P3 review):* the guard does not clone the finders. P3 cloned all six, about 1.1 MB each on a huge map (10,240 plots), so a guard cost about 0.9 ms. It now moves each of the six out of the game and puts a fresh `PathFinder::new(kind)` in its place (it initializes itself if a read searches with it), and moves the game's own back when dropped: O(1), and the finders come back exactly as they were whatever a read did. A finder a rules call holds exclusively (`try_borrow_mut` and `try_borrow` fail) is left in place and makes `finders_busy`; one it holds shared is left in place (a read cannot change it). Measured on a huge map with 8 AIs at turn 40: 1.7 µs per guard, 2.2 µs per `status` query (P3: 883 µs and 759 µs).

**Allow-list.** Queries must still not call `group_generate_path`, `group_get_path_*`, `unit_generate_path`, `unit_get_upgrade_city(_, true)`, `player_count_unimproved_bonuses`, `player_ai_found_value` or `player_ai_bonus_val`. The guard is the safety net, not a licence.

**Path previews** use `UiState.interface_finder = PathFinder::new(FinderKind::Path)`: call `set_group(Some(grp))`, then `generate_path(&g, ..)`. The game's saved finders are never touched.

### 2.6 Saves, sidecar, autosave, journal, replay, take-over

**Save bundle.** A save is two files:
* `<name>.ffhsave`: plain `Game::save()` bytes, loadable by any tool;
* `<name>.ffhsess`: a small JSON sidecar `{v:1, human, pending_events:[[p,id,launched],..], journal:"<path>", journal_line:N, messages_tail:[..], auto_moves_tried}` (`launched`: the popup was shown, a pair without it reads as not yet shown; `auto_moves_tried`, optional: the AutoMoves rule already fired at this control point).

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
* starts from the header of a new game; or, `--from`, from the save a `load` / `take_over` header names (matched by its FNV hash; loaded with its sidecar, or taken over again), or from a save at one of the journal's checkpoints;
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
    pub fn advance(&mut self, on_slice: &mut dyn FnMut(&Session)) -> Stop;   // P4: the session, not &Game (§7 P4)
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
    Mission { units: Vec<UnitId>, mission: MissionType, data1: i32, data2: i32, flags: i32, append: bool },  // flags: 0 only (§4.5)
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

A member that a command, a setup, a session operation or an answer does not define is refused (`invalid`), so a misspelt flag is an error rather than its default (P4 review); queries ignore unknown members. No query or command member is called `id` or `k` (the envelope's).

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
| `GET /api/map[?since=R&epoch=E]` | Shared | Map arrays. Full without `since`; with it (P6 as built) the tiles changed since revision `R` of epoch `E`, or the full map when they do not fit (§4.4). |
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
            "production":{"order":0,"type":312,"turns":3},"idle":false,"defense":20,"walls":false}],
 "units":[{"id":881,"owner":0,"visual_owner":0,"type":312,"x":12,"y":31,"group":402,"hp":100,"moves":60,"max_moves":120,
           "level":2,"xp":7,"promo_ready":false,"activity":2}],
 "groups":[{"id":402,"head":881,"units":[881,890],"ready":true,"missions":[{"m":0,"d1":15,"d2":33}],"automated":false}],
 "pending":{"events":[...],"votes":[...],"research_needed":false,"idle_cities":[7],"promotions_ready":[881],"ready_groups":[402,409]},
 "log_next":4211}
```

**Fog rules:**
* Own units are always shown.
* Foreign units are shown only on plots with `plot_is_visible(p, team)` and `!unit_is_invisible(u, team, true)` (C++'s default `bCheckCargo`: the cargo of a foreign transport is hidden; P3 as built); their owner is the `visual_owner`, which respects hidden nationality (a foreign unit's reply has no separate true owner, no group, moves or experience).
* For foreign cities, name and owner are shown when `city_is_revealed`; size, defence and walls (a Walls or Palisade building, which the city art draws) only when the plot is visible.
* Unmet players are omitted.
* Log entries tagged with a player go to that player only. An untagged entry (`player == None`) stands for C++ messages sent to several players under a condition the log does not record, so it is shown only under that condition, evaluated when the log is drained (`view/fog.rs`, `LogFog`, which recognizes the ffh-core formats): war, peace and vassal changes when h's team is one of the two teams or has met both; a city captured, razed or liberated when h's team revealed the city (at that drain or the previous one); circumnavigation by an unmet team as "an unknown civilization"; vote results to voting members; vote-source religions, special buildings and nukes to everyone. Unrecognized untagged entries (event world news, whose sender the text does not name, and new log sites) are held back. A P7 core hook (recipients per log entry) replaces the text matching.

**`map`** is flat arrays indexed by `y*w+x`, with -1 meaning none:

| Array | Contents |
|---|---|
| `vis` | 0 = unrevealed (all other arrays are -1), 1 = fogged, 2 = visible |
| `plot_type`, `terrain`, `feature` | current values |
| `bonus` | `plot_get_bonus_type(p, Some(team))` |
| `improvement` | `revealed_improvement_type[team]` |
| `route` | `revealed_route_type[team]` |
| `owner` | `revealed_owner[team]` |
| `river` | bit 0 = `n_of_river` (south edge), bit 1 = `w_of_river` (east edge), bits 2-4 = `river_ns_direction` + 1, bits 5-7 = `river_we_direction` + 1 (0 = no flow) |
| `city` | city id, for revealed cities |
| `yield` | `food \| production << 8 \| commerce << 16` of the plot's yields, for **visible** plots (P3 as built: a fogged plot's yield would show improvements built since; ffh-core's `plot_calculate_yield` has no `bDisplay`, which C++ uses to compute it from the revealed improvement) |

`map_rev` is an FNV hash of the arrays. Sizes: about 130 KB for a standard map, about 300 KB for huge.

In phase 6 the session keeps `tile_rev: Vec<u32>`, so `/api/map?since=R` returns only the tiles that changed (engine-first's delta idea). As built (P6): a `MapTracker` (`view/map.rs`; the session's in `UiState` for the `map` query, the HTTP server's own in `Shared` for the cached map) keeps the last map, a revision counter and each tile's revision of last change, under an `epoch` (a tracker's identity, unique per process and across restarts). The full map carries `epoch` and `rev`; `map {since, epoch}` / `GET /api/map?since=R&epoch=E` answers `{"full": false, "since": R, "epoch", "rev", "map_rev", "w", "h", "wrap_x", "wrap_y", "reveal", "tiles": [i, ..], "vis": [..], ..}` (each layer's values for the listed tiles, in their order), or the full map (`"full": true`) when the epoch is another (a new game, the other transport's tracker, a restarted server), `R` is not a revision of it, or more than a third of the tiles changed. A client applies a delta only to the map of revision `R` (`tiles.js` `applyMapDelta`) and asks again in full otherwise.

### 4.5 Commands

Every command is validated before any rules call (§2.8).

| `c` | C++ origin | Rules calls |
|---|---|---|
| `move {units,x,y,append,declare_war}` | `selectionListMove` + DECLAREWARMOVE `OnOk` + `selectionListGameNetMessage(PUSH_MISSION)` | 1. If any `unit_get_declare_war_move(u, plot)` returns a team, reply `confirm declare_war` unless `declare_war`; if set, `team_can_declare_war` then `team_declare_war(my, t, false, None)`. 2. If `units` doesn't equal one existing group: `unit_join_group(head, None, true)`, then `unit_join_group(u, Some(head_grp), true)` for the others. 3. `group_push_mission(grp, MISSION_MOVE_TO, x, y, 0, append, true, None, None, None)`. |
| `mission {units,mission,data1,data2,flags,append}` | `selectionListGameNetMessage(PUSH_MISSION)` → `CvNetJoinGroup` + `CvNetPushMission` | Same join step; check `group_can_start_mission(grp, m, d1, d2, None, false)`; then `group_push_mission(.., manual=true, ..)`. MOVE_TO asks the DECLAREWARMOVE question as `move` does (`confirm declare_war`, answered with `move`): every exe path that sends it goes through `selectionListMove` (P3 review). Covers skip, sleep, fortify, sentry, heal, pillage, plunder, found, build, join, construct, discover, hurry, trade, great work, golden age, spread, lead, bombard, route-to. Only what the exe sends is accepted: `flags` 0 (every exe sender passes `iFlags = 0`; the movement flags are AI-only); data as the action carries it (a plot for the interface-mode missions, the type for build / spread / join / construct with `data2` -1, the LEADUNIT unit id or -1, -1/-1 otherwise); MISSION_MOVE_TO_UNIT is refused (no visible action, hotkey or interface mode: AI-only). |
| `do_command {units,command,data1,data2,alt,confirmed}` | `handleAction` → `selectionListGameNetMessage(DO_COMMAND)` → `CvNetDoCommand` | Confirm when `commands[c].confirm_command` (delete); for COMMAND_CAST when `spells[s].global` (CONFIRMCASTGLOBAL) or `causes_war` with a target in range (the FfH/Sephi range scan in `handleAction`); COMMAND_LOAD with more than one transport (LOADUNIT picker). Then `unit_do_command(u, c, d1, d2)` for **each** selected unit. `alt` with `commands[c].all` means every unit of that type. COMMAND_AUTOMATE is hidden until automation exists (F15); COMMAND_AUTOMATE_SPELL is hidden (F16). |
| `join_group {unit,head?}` | `CvNetJoinGroup` | `unit_join_group(u, head.map(group), true)` |
| `auto_mission {unit}` | `CvNetAutoMission` | `group_auto_mission(g)` |
| `push_order {city,order,data,alt,shift,ctrl}` | `CvGame::cityPushOrder` flag rewrite, then `CvNetPushOrder` | `city_push_order(c, o, d, -1, alt, !(shift‖ctrl), shift, false)` using the rewritten flags. Checked with `city_can_train`, `construct`, `create` or `maintain` (`test_visible = false`). |
| `pop_order {city,index}` | `CvNetPopOrder` | `city_pop_order(c, i, false, false)` |
| `do_task {city,task,..}` | `CvNetDoTask` | `city_do_task(..)` for HURRY, CHANGE_SPECIALIST, CHANGE_WORKING_PLOT, EMPHASIZE, SET_AUTOMATED_*, CONSCRIPT, CLEAR_ORDERS, LIBERATE. RAZE, DISBAND and GIFT are refused (popup only). CHANGE_SPECIALIST takes `data2` = 1 or -1 only, under the city screen's +/- button conditions (`updateCitizenButtons`); LIBERATE takes `data1` = 0 only (the button and the FREE_COLONY popup; 1 is the conquest path). |
| `research {tech?,shift}` | `CvNetResearch` (`iDiscover == 0`) | `None`: `player_clear_research_queue`. Otherwise, if `player_can_ever_research`: if (has the tech or already researching it) and `!shift`, clear the queue; then `player_push_research(h, t, !shift)`. |
| `update_civics {civics}` | `CvNetUpdateCivics` | Merge into `IdxVec<CivicOptionType, Option<CivicType>>`; check `player_can_revolution(h, Some(&v))`; then `player_revolution(h, &v, false)`. |
| `percent_change {commerce,change}` | `CvNetPercentChange` | `change` must be ±`COMMERCE_PERCENT_CHANGE_INCREMENTS` (the slider buttons); the commerce must be flexible; then `player_change_commerce_percent` |
| `convert {religion?}` | `CvNetConvert` | `player_can_convert`, then `player_convert` |
| `declare_war {team}` | `CvNetChangeWar` | Met, `team_can_declare_war`, then `team_declare_war(my, t, false, None)`. Peace goes through a deal. |
| `propose_deal {to,give,get}` | diplomacy screen (reachable when `AI_isWillingToTalk`) → `AI_considerOffer` → `implementDeal` | `player_can_contact` and `AI_isWillingToTalk`; at war the peace treaty or a capitulation; every item must pass `player_can_trade_item(from, to, item, false)` with no `player_get_trade_denial`. Then `g.controller(to).consider_offer(g, to, h, give, get)`; if true, `implement_deal(h, to, give, get, false)`. Reply `{accepted}`. |
| `event {triggered,event}` | `CvNetEventTriggered` | Must be in the pending list and `player_can_do_event`; then `player_apply_event(h, e, id, true)`; remove it from the pending list. |
| `vote {vote,choice}` | DIPLOVOTE popup (`CvDLLButtonPopup::launchDiploVotePopup`) → `CvNetDiploVote` | `choice` is decoded by `names::decode_player_vote`: YES, NO, NEVER, ABSTAIN or a team id; the internal `NO_PLAYER_VOTE` (-1) and `NO_PLAYER_VOTE_CHECKED` (-6) are refused. The vote must be pending for h. Then the choice must be a button the popup offers. **Team vote** (`is_team_vote`): a team id that is alive and `is_team_vote_eligible(t, src)`; if h's team is the vassal of an eligible team V (the first in team order), only V or h's own team; ABSTAIN only when h is not such a vassal; YES/NO/NEVER refused. **Other votes:** YES, NO, NEVER only if `player_can_defy_resolution(h, src, &option)`, ABSTAIN; team ids refused. Then `cast_vote(h, id, choice)`. (A non-negative `PlayerVoteTypes` is a team id: ffh-core's `PlayerVote::for_team` / `PlayerVote::team()`.) |
| `set_option {option,value}` | `sendPlayerOption` | Allow-list: STACK_ATTACK, QUICK_MOVES, SAFE_AUTOMATION, NO_UNIT_CYCLING, WAIT_END_TURN. SHOW_* is refused (F3). |
| `auto_moves` | `CvNetAutoMoves` | `player_set_auto_moves(h, true)` |
| `end_turn {force}` | `CvNetTurnComplete` | The gate in §2.3, then `player_set_end_turn(h, true)` |
| `autoplay {turns}` | `setAIAutoPlay` (FfH `setDisableHuman`) | `set_ai_auto_play(n)`. Testing aid, journaled. (P6 review) While an autoplay runs, `turns: 0` over HTTP stops it before the next turn slice (`setAIAutoPlay(0)`, journaled at that slice and replayed there). |

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
| `inspect {kind,oid}` | `{:#?}` of a unit, group, city, plot, player, team or info entry. Own objects only unless debug-reveal is on; a plot only under debug reveal (it holds every team's data). The object is `oid` (P4 review: `id` is the envelope's request id) |
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
src/main.rs                serve | agent | bot | soak | replay | journal print | bench; kinds | names | lookup (P0); bot, replay, journal print (P1); soak run, soak (P2); bot --verbose prints the text query (P3); agent, serve (P4); stage (P5) ~220
src/json.rs                Json enum, escaping writer, recursive-descent parser, typed getters    ~400
src/names.rs               type name <-> index for every table and hard-coded enum (NAMES arrays), text lookup ~220
src/setup.rs               NewGame -> Game, controllers, active_player, options, take_over, setup lists ~250
src/human.rs               HumanController (§2.3), POLICIES, research-site rule, Answerer trait + impls ~420
src/decision.rs            Decision builders, Pending derivation, event effect summaries         ~350
src/session.rs             Session, phase, advance, settle, execute, auto moves, drain, autosave, watchdog ~600
src/command.rs             Command decode/validate/apply: CvNet ports, selectionList*, cityPushOrder,
                           handleAction confirms, deals                                           ~800
src/selection.rs           (P1) the exe's selection-list checks (canHandleAction): canStartMission,
                           canDoCommand over the selected units                                   ~170
src/guard.rs               QueryGuard                                                              ~130
src/journal.rs             JSONL writer/reader, stamps, checkpoints, JournalDriver, pretty printer ~250
src/save.rs                save bundle + sidecar, autosave ring, name sanitizing                  ~150
src/report.rs              fog-filtered snapshot + diff, MessageStore                              ~280 (P3: 610 with tests)
src/query.rs               (P3) Query decode/encode, QueryErr, Ctx, UiState, query::run             ~380
src/view/fog.rs            the only visibility predicates (Viewer methods; P1's LogFog)            ~120 (P3: 240)
src/view/{mod,map,player,units,city,diplo,data,text}.rs   views and queries (§4.4-4.7)            ~2,200 (P3: 1,850)
src/transport/mod.rs       (P4) the envelope (Request, replies), Host (the session, its game directory, the
                           session operations, advance events, status), decision_reply / running_reply,
                           the panic hook and crash report                                         ~780 (with tests)
src/transport/shared.rs    Shared {status+Condvar, cached view/map/data, rx}, wait()              ~150
src/transport/http.rs      std TcpListener, thread per connection, router, assets, X-FFH, 403/409/503 ~380
src/transport/server.rs    game-thread loop for HTTP, ChannelAnswerer, catch_unwind, crash status ~300
src/transport/stdio.rs     JSONL loop on the game thread, StdioAnswerer, EOF policy                ~250
src/bot.rs                 scripted human (protocol-level), seeded private RNG                     ~380
src/soak.rs                AI-as-human driver (P2: settings, run, checks, spawned civs, panic capture) ~800
src/soak/batch.rs          (P2) the matrix, child processes with a timeout, summary, TSV          ~400
src/stage.rs               (P5) saves at staged situations for testing clients (`ffh-play stage`)  ~240
assets/index.html ~120 (P4: a placeholder page; P5: the client's shell, 55), assets/style.css ~450 (P5: app/style.css, 200)
assets/js/api.js ~200, state.js ~250, markup.js ~120, map.js ~1,000, units.js ~500, city.js ~600,
          screens.js ~700, modals.js ~400, main.js ~250                                           ~4,600 web total
          (P5 as built: assets/app/{api,state,tiles,markup,dom,modals,commands,map,units,city,screens,hud,main}.js,
          3,800; the URL layout, /app/*)
assets/test/*.mjs          node --test: markup, wrap-aware tile maths, overlay layout, api smoke   ~200 (P5: 490)
tests/*.rs                 §8                                                                      ~1,600
tools/web-check.sh         node --check + node --test, skipped if node is absent                   ~20
tools/web-smoke.mjs        headless-Chromium smoke test against `ffh-play serve` (P5; F21), skipped without a browser ~150 (P5: 480)
tools/web-api-smoke.mjs    (P5) the node smoke client: 20 turns through the client's api.js       ~370
tools/web-lib.mjs          (P5) shared by the smoke tests: the binary, `serve --port 0`, stages    ~130
```

Totals: about **7.5k lines** of Rust library and binary code, **1.6k** of Rust tests, and **4.6k** of JS/CSS/HTML.

**Shared files edited by this workstream** (small):
* **Root `projects/ffh2-rs/Cargo.toml`:** add `"crates/ffh-play"` to `members`. This is the only manifest edit.
* **Docs:**
  * new `docs/HUMAN.md`: how to play, protocol, decision policy, journal and replay, known gaps;
  * a "Human play" section in `docs/ARCHITECTURE.md`;
  * a crate row in `README.md` and in the `projects/ffh2-rs/CLAUDE.md` layout;
  * `docs/PORTING.md` "Changes from FFH2": UI-level human timing, Kuriotates still unanswered, no gift button.
* **`.gitignore` (repo root):** add `projects/ffh2-rs/saves/`. The default `--saves` directory is `./saves`.

**Not edited in the MVP:**
* `crates/ffh-core`, `crates/ffh-ai`, `crates/ffh-cli`, `crates/ffh-stress`, `controller.rs`.
* Exception (as designed): **rules bugs** that the soak finds in core's human branches, fixed as separate small commits with an ffh-core regression test. In P2 the workstream did not own ffh-core, so its findings are reported in §7 P2 instead.

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
  * `names.rs`. `Named` trait (kind name, count, name, lookup, text key) for all **61 info tables** (`ffh_data::INFO_FILES`), the **6 GlobalTypes lists** without a table (flavors, art styles, city sizes, contacts, diplomacy powers, directions) and the **15 hard-coded C++ enums** that have `NAMES` but no XML (activity, area AI, cardinal direction, control, feat, game state, interface message, mission AI, order, player option, plot, probability, task, tradeable item, war plan). `KINDS` lists them with type-erased accessors (for `/api/data`, `inspect`, the CLI). `decode::<T>` accepts a type name or an index in range; `decode_opt` also takes `null`/`-1` as none; `field`, `opt_field`, `list_field` decode object members. `PlayerVoteTypes` (not an index type) has `decode_player_vote`, which accepts only what the C++ DIPLOVOTE popup can send: `YES`/`NO`/`NEVER`/`ABSTAIN` (C++ or short name, or the values -2, -3, -5, -4) or a **team id** in `0..MAX_CIV_TEAMS` (a non-negative `PlayerVoteTypes` is a team, used only in team votes); the internal `NO_PLAYER_VOTE` (-1) and `NO_PLAYER_VOTE_CHECKED` (-6) are refused. Which choices a given vote offers is the P1 `vote` command's check (§4.5). ffh-core's `PlayerVote::team()` reads a vote value as a team (ffh-play's own `voted_team` went with the rules fixes). Type names are unique across all kinds (tested), so a bare name is never ambiguous.
  * **Text.** The vendored text (`data/text/en.tsv`, `tools/vendor_xml.py`) holds FfH2's own text merged over the stock BtS/Civ4 English text it inherits (the base → Warlords → BtS layers, civilopedia left out; FfH2 overrides, so its own strings are unchanged). So descriptions inherited from BtS (e.g. `TXT_KEY_UNIT_WARRIOR` → "Warrior") now resolve to the real text, not a humanized type name; `names::display` falls back to a humanized name (`UNIT_WARRIOR` → "Warrior") only when a key is truly absent or empty. `display` is the description as ffh-data's `Text::name` reads it (C++ `getDescription()`, form 0), with Civ4 markup kept (`VOTE_LIBERTY` → "Global Civic: Liberty"): FfH2's own English entries have no grammatical forms (no `<Text>`/`<Gender>`), so a name is the whole text and its 11 descriptions with a ':' are all prose; a few inherited stock entries (the vanilla civ adjectives, `<Text>American:Americans</Text>`) do use forms, and `Text::name` reads form 0. (Before the rules fixes `Text::name` cut at the first ':' and `display` read the text with `Text::get`.) A description with an unfilled placeholder (the two Somnium memories, `%D1: "..."`) also falls back to the type name.
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
* **As built (done).** The API is §3 minus what belongs to P3 (`Session::query`, `UiState`, the report). No ffh-core, ffh-ai, ffh-cli or `controller.rs` edit; no engine hook was needed (the P7 list stands). Semantics pinned down, with the C++ each follows (all re-read in `/home/user/ref/ffh2`):
  * **Loop** (§2.2) as designed: `phase` is the pure predicate; `advance` drains the log at every control point and turn boundary, journals a checkpoint and autosaves at every turn boundary (`auto/T####`) and at a new game's first control point (`auto/start`: `game_init` leaves the first player's turn active, so a game starts without a boundary), and runs the 200k-slice watchdog; `execute` validates under a `QueryGuard`, journals, applies, settles (≥ 1 `update()`, then while a human group is busy, ≤ `settle_limit`). `on_slice` gets a plain `&Game` (transports must read plain fields or take a guard). The AutoMoves rule (`updateTestEndTurn`) fires once per control episode when no group is ready and some has queued orders. An episode ends when a command of the human is applied (`execute` resets the flag before applying; the AutoMoves `advance` sends itself does not), and the flag is in the sidecar, so a session resumed at that control point does not send AutoMoves again (P1 review: the flag used to be reset by the AutoMoves it guarded; `tests/decisions.rs` covers a group left ready to auto). `updateTestEndTurn`'s *automatic end of turn* (`sendTurnComplete` when nothing is ready and a unit moved) is **not** ported: the human always ends the turn, as with `PLAYEROPTION_WAIT_END_TURN`.
  * **Ready groups** (`decision::ready_groups`, used by the AutoMoves rule, the end-turn gate and `pending.ready_groups`) are `readyToMove(bAny = true)` groups **without** automated and FfH AI-controlled groups: C++ moves those in `AI_unitUpdate`, here they idle until the P7 automation hook (F15), and would otherwise block the gate and AutoMoves forever. They are listed as `pending.ai_groups_idle`.
  * **Research-site rule** (F5): the mark is `ResearchMark {turn, slice, player, team tech count, overflow research}`, not only `(turn, slice, p)`: nothing changes between `doResearch`'s two calls, so a later call in the same slice after a tech was acquired (research-next during AI turns) is not taken for the `AI_chooseResearch` call. Verified by T6 both ways.
  * **Selection checks** (`selection.rs`): the exe checks an action on the *interface selection list*, a `CvSelectionGroup` of the selected units, before sending it (`canHandleAction`). `selection.rs` ports `canStartMission`, `canDoCommand`, `canEverDoCommand`, `isBusy` and `getBestBuildRoute` over the selected units, so a selection that is not a whole group is checked like the exe does, before any join. With shift (append) and a mirrored group, the mission plot is the group's last mission plot when visible (`canHandleAction`). Selections must be on one plot, distinct, the human's.
  * **`move`**: `declare_war` is tri-state: absent asks (`Confirm::DeclareWar {team, move_anyway}`), `true` declares (`CvNetChangeWar` → `declareWar(rival, false, NO_WARPLAN)`, unchecked as in C++) and moves, `false` is the popup's "no", which still moves when the target plot is not that team's (DECLAREWARMOVE `getOption2`) and otherwise changes nothing (`Plan::Noop`, not journaled).
  * **`do_command`**: COMMAND_LOAD is never sent to units: as `handleAction` → LOADUNIT, it loads into the only possible transport (`COMMAND_LOAD_UNIT` owner/id) or answers `Confirm::LoadUnit {transports}`. Confirmations: `bConfirmCommand` (delete), global spells, war spells (Sephi's area rule from `handleAction`); COMMAND_AUTOMATE (F15), AUTOMATE_SPELL (F16) and HOTKEY are refused. `alt` applies only with `bAll`, per selected unit, as `CvNetDoCommand` (UNOFFICIAL_PATCH type read).
  * **Cities**: `push_order` applies the `cityPushOrder` flag rewrite (the CHOOSEPRODUCTION popup's `pushOrder(.., bPop = true)` gives the same result on the empty queue it is shown for). `pop_order` is refused on a queue of one order (the exe sends POP_ORDER only for longer queues). `do_task` also allows CLEAR_WORKING_OVERRIDE, RALLY_PLOT and CLEAR_RALLY_PLOT (the exe sends them); RAZE / DISBAND / GIFT are popup-only and refused; working plots must be the city's and ours (`handleCityScreenPlotPicked`).
  * **Other gates**: `update_civics` needs `canDoCivics` for each new civic and `canRevolution`; `percent_change` needs a flexible commerce and a step of ±`COMMERCE_PERCENT_CHANGE_INCREMENTS`; `convert` `canConvert`; `declare_war` `canDeclareWar`; `research` `canEverResearch` (C++ `CvNetResearch`); `autoplay` 1..=10000 turns.
  * **Parameters only as the exe sends them** (P1 review): a well-typed value no exe path sends can still reach rules code that trusts it (`alterSpecialistCount` loops `|change|` times: `data2 = i32::MAX` ran for about an hour; `changeCommercePercent` overflows). So `mission` needs `flags` 0 and the data its action carries (MOVE_TO_UNIT is refused), `do_task` CHANGE_SPECIALIST ±1 under the +/- button conditions and LIBERATE `data1` 0 (the decoder's default for it), `percent_change` ±the increment. `tests/decisions.rs` checks each refusal leaves the save bytes unchanged.
  * **Decisions**: an answer that is not an enabled option counts as "advisor" (its pick, else the first enabled option). The election offers the resolutions plus "none" (C++ TXT_KEY_NONE, which sends nothing: here the selection is deleted, the same in play). Advisor suggestions are computed under a guard (C++ never computes them for humans). Free tech and religion options are what the C++ popups list; with no option the controller returns `None` without asking.
  * **Events**: a deferred popup is checked once, when it is shown: at the first control point after it was queued (after `advance` or after the command that queued it), `launchEventPopup` drops it if its data is gone or no event can be done, else it is marked launched (`DeferredEvent::launched`, in the sidecar). A launched popup stays in `pending.events` until answered, closed by a forced end turn, or its data is gone, with its options enabled as `canDoEvent` says at each moment, so one the human's own later actions disabled (gold spent on a hurry) can be chosen again when it is doable (P1 review: the check used to run at every control point, so the human's unrelated commands could cancel an event, which a modal C++ popup prevents). `event` checks that the popup is open, the trigger's events and `canDoEvent` now. Each option of a pending event carries an effect summary from `EventInfo` (`decision::event_effects`: gold with inflation and tech cost, tech share, units, buildings, promotions, experience, happiness and health, culture, population, Armageddon counter, crime, attitude, terrain changes; an effect implemented in Python is named, not summarized).
  * **Messages** (§4.4 fog rules): the log drain keeps tagged entries for the human only and passes untagged ones through `LogFog` (`view/fog.rs`), which applies the C++ recipient condition of each recognized ffh-core message and holds back the rest (P1 review: every untagged entry used to reach the human, so wars, captures and razes among civilizations and cities it had never seen showed up in its messages and command replies). The condition is evaluated at the drain, after the slices that logged the entries; met teams and revealed cities only grow, and a city gone by then counts if it was revealed at the previous drain.
  * **Saves**: `SessionOptions::saves_dir` is the game's directory: `auto/T####.ffhsave` + `.ffhsess` (the last `autosave_keep`), `journal.jsonl` (`journal-N.jsonl` if taken), manual `<name>.ffhsave` + `.ffhsess` (names `[A-Za-z0-9_.-]{1,64}`, no leading dot). Without it the journal is kept in memory. Saving is refused while a decision is open and except at a control point, a turn boundary or game over.
  * **Journal and replay**: lines as §2.6 (cmd lines are the canonical request form, with indices; answer lines the resolved option). `session::replay` re-applies each command at its stamp and answers from the journal (`JournalAnswerer`); it runs with `auto_moves_when_idle` off, as the journaled AutoMoves are re-applied as commands, and ends at the control point after the last input. From a save it starts after the checkpoint the sidecar's `journal_line` names (else the one with the save's turn and checksum); a journal whose header is `load` or `take_over` also replays from the save the header names (its FNV hash; P1 review: before, such a journal could be replayed only from an autosave written after its first turn boundary).
  * **Bot** (`bot.rs`): the T1 policy without the peace proposal (`propose_deal` is P3). It is weak (1–4 cities in 150 turns) but exercises every P1 command path.
  * **Tests**: unit tests in every module (guard save-bytes test, setup, command decoding round trip, journal, save); `tests/bot_game.rs` (T1: seeds 1–3, 150 turns, small, 3 AIs; the blocking-decision assertion is over the three games, as only seed 3 captures a city; T2), `resume.rs` (T3: reloads before acting, mid-turn after commands, with an event pending, after a decision, and from an autosave), `replay.rs` (T4: from the header, the `start` save and an autosave; a changed stamp is reported at its line; a loaded session's journal from the loaded save and from its autosave, a taken-over session's from the AI save), `soak.rs` (T5), `decisions.rs` (T6, plus take-over and command gates), `drift.rs` (T9: also checks that `HumanController` implements every trait method itself), `cli.rs` (the new subcommands).
  * **Found for other owners** (not edited here): city names are stored as text keys (`TXT_KEY_CITY_MALAKIM_1`) and several log messages keep unfilled placeholders (`%s1_civ_adjective`); the AI playing the human slot (T5) reaches only 2 cities in 150 turns where the AI players get 1–6 (for P2's soak to look at).

**P2 — flush the human branches (about 0.6k).**
* Scope:
  * `soak.rs` and `ffh-play soak --seeds A..B --sizes .. --civs .. --options ..`;
  * invariant checks every 5 turns, through `#[path]` reuse of ffh-stress `check.rs`;
  * crashes caught per game, printing the repro command.
* Run before any person plays:
  * at least 2 seeds × {tiny, small, standard} × {Kuriotates, Infernal, Sheaim, Mercurian, Bannor, Calabim} × {default, raging barbarians, aggressive AI}, 300 turns each;
  * also on the checked build (`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`).
* Fix or document what it finds (§6). Exit: the matrix is clean.
* **As built (done).** No ffh-core, ffh-ai, ffh-cli or `controller.rs` edit; rules findings are reported for their owners (below), not fixed here.
  * **Driver** (`soak::run`, the T5 policy on the session API):
    * production: idle cities with manual production get the AI's `choose_production`; cities with odd ids are set to automated production with `do_task SET_AUTOMATED_PRODUCTION` (the human automation path: the rules ask `HumanController`, which asks its advisor);
    * research, event popups and votes are answered with the AI's choice through `research`, `event` and `vote`, except on one turn in seven (`QUIET_EVERY`): then they are left open and the end-turn gate is forced, so C++'s handling of unanswered popups runs too (closed events, the vote's extra turn then abstain, the `AI_chooseResearch` pick after turn 4). The AI's first `play_turn` of a turn fills an empty research queue with a rules call (ffh-ai `empire::research`); on a quiet turn the driver clears it again with `research {tech: null}` (CvNetResearch with NO_TECH: `clearResearchQueue`, CvMessageData.cpp:383-385) before the end turn, so the research popup really reaches the forced end turn open (P2 review: before, it never did, and the AI picks counted came from other paths);
    * the pending popups are read again before each answer: an answer can change what another popup offers (gold spent, a unit gone) or launch a new one, and `event` / `vote` check their gates when applied (P2 review: one snapshot answered all of them);
    * blocking decisions take the advisor's pick (`AdvisorAnswerer`);
    * the AI moves the human's units with `Controller::play_turn` (the rules' mission API), repeated while it moves something (at most 20 times), the session running the human's missions in between;
    * `end_turn`, forced if the gate asks.
  * **Checks**: `check.rs` every 5 turns (`#[path]` reuse, F20); at most one active player turn; `save(load(save)) == save` and the sidecar's JSON round trip every 25 turns. A command the session offered (a `pending` choice) and then refused is a failure, as is a `Stuck` turn or a panic.
  * **Repro.** The soak keeps no journal: the AI moves units through rules calls, not commands, so a journal would not replay. Every game is deterministic, so the repro is the game's own `ffh-play soak run ...` line with its check settings when they are not the defaults (`--check-every`, `--save-check-every`: a check runs only on the turns they name; a batch passes its own to every game and puts them in its repros), plus `--resume <autosave>` when `--saves` kept the last 3 autosaves; a game resumed from an autosave ends with the same save bytes as the uninterrupted one (`tests/soak.rs`, `tests/cli.rs`). A caught panic prints its message, location and the workspace frames of the backtrace (exit 101); `STUCK` exits 4, `ERROR` (a refused command, a failed check) 5. `--panic-at T` tests the report (it is not part of the repro).
  * **Infernals and Mercurians** are not playable at the start (`bPlayable` 0): FfH spawns them mid-game and offers a human founder control of them (`reassignPlayer`), which the port has no hook for (§5.2). Their soak plays an AI-only game with the same settings. At turn 30 (`--spawn-turn`) player 0 founds them through the FfH Python ports: the Infernal Pact tech (`onTechAcquired`), or the Mercurian Gate in its capital (`onBuildingBuilt`; the Mercurians join its team). The new player is taken over at the next turn boundary (`Session::take_over`), with the human's handicap (`--handicap`): FfH's `reassignPlayerAdvanced` gives the new slot the human founder's (P2 review: it kept the setup's default, Noble, against AIs at the game's handicap).
  * **Matrix change.** The design's "aggressive AI" axis is a no-op in the port. No rules or AI code reads `GAMEOPTION_AGGRESSIVE_AI`: C++ uses it only in CvPlayerAI/CvTeamAI/CvCityAI/CvUnitAI and in the Python `cf.warScript` / `dogpile`, which the new AI replaces. All 36 of its games had the checksum of the matching default game. It is replaced by two sets of game options that only act on human players, and so never ran in ffh-stress's AI-only games:
    * `occ`: one-city challenge, no healing for humans;
    * `challenges`: cut losers, high to low, increasing difficulty, flexible difficulty.

    The default matrix is 2 seeds × {tiny, small, standard} × {Kuriotates, Infernal, Sheaim, Mercurians, Bannor, Calabim} × {default, raging barbarians, `occ`, `challenges`}, 300 turns: 144 games. `ffh-play soak` runs each game in a child `soak run` with a timeout, `--jobs` at a time, then prints a summary and an optional TSV. The summary has counts per civilization, size and option set, the human's cities against the AI civs', and every distinct failure with its repro commands. After the P2 review: `--speeds` adds a game-speed axis (a quicker speed reaches the late game in fewer turns); a game listed twice runs once; each game's save directory is its description cut to 55 characters plus 8 hex digits of the FNV hash of its settings (before, two games whose descriptions shared 64 characters wrote into one directory, deleting each other's autosaves); the arguments are checked before any game runs: the `--out` file is created first, `--turns` must be at least 1, the check intervals must be numbers, and every game's settings must parse as its child will read them (before, a bad value made every game fail, or a bad `--out` path lost the whole batch's TSV); `soak run` refuses numbers out of their type's range (`--seed -1` used to become seed 4294967295).
  * **Runs** (4 cores; CPU time including the child processes):
    * release, the design's first matrix (with aggressive AI): 108 games, 11.7 CPU minutes;
    * release, `occ` and `challenges`: 72 games, 6.7 CPU minutes;
    * checked build (debug assertions and overflow checks), the 144-game default matrix: 14.9 CPU minutes; every game ended with the checksum of the same game on the release build;
    * release, seeds 3 and 4 of the default matrix: 144 games, 13.4 CPU minutes.

    In all, 468 games took 46.7 CPU minutes. Nothing was trimmed: every seed, size and civilization, 300 turns each. Every game passed: no panic, no `Stuck`, no refused command, no failed invariant or save round trip. What the three release runs exercised:
    * 3,181 event popups answered and 4 votes;
    * 1,282 popups left open, and the end-turn gate forced 1,076 times;
    * 168 `AI_chooseResearch` picks and 613 cities set to automated production;
    * 131 keep/raze and 41 free-tech decisions;
    * 108 Infernal or Mercurian players taken over;
    * the human moved to another slot in 24 of the 36 `challenges` games of each 144-game run (twice in 2 games).

    No election came up and only 4 votes, as few vote sources (the council civics) are adopted in 300 turns. The election modal is the one blocking decision the soak did not reach; a soak option that adopts a council civic would.

    These counts come from the driver before the P2 review fixes. On quiet turns the AI's own empire management answered the research popup before the end turn. So the 1,282 popups "left open" include research popups that the AI then answered, and the 168 picks came from other paths (research emptied later in the turn).

    **After the review fixes** (release build, the 144-game default matrix, seeds 1 and 2): every game passed, run twice with identical results. CPU time was 20.3 minutes over 18.6 minutes of wall time: on this host the 4 jobs got about one core, so it does not compare with the runs above. The run exercised:
    * 1,429 event popups and 2 votes answered;
    * 557 popups left open, 341 of them research popups that now reach the forced end turn open (765 forced gates);
    * 372 `AI_chooseResearch` picks: the 341, plus other paths;
    * 259 cities set to automated production;
    * 56 keep/raze and 19 free-tech decisions;
    * 48 Infernal or Mercurian take-overs;
    * the human moved to another slot in 24 of the 36 `challenges` games (twice in 2).

    The checked build played the same 144 games in 23.8 CPU minutes. Every game passed, and each ended with the checksum and save hash of its release game.

    A further checked run was a quick-speed batch that reaches the late game: `soak --seeds 1..9 --sizes tiny --civs kuriotates,infernal,sheaim,mercurians,bannor,calabim --options default --speeds quick --turns 400`. It played 54 games in 11.4 CPU minutes, and 9 of them stopped on one assertion, ffh-core's Syliven's Perfect Lyre assertion (below):
    * 5 Kuriotates, 2 Calabim, 1 Sheaim and 1 Bannor game, between turns 213 and 397;
    * in the 5 games whose last autosave already shows the cause, the Lyre has been created 2 to 5 times, against a maximum of 1.

    The summary listed the 9 games as one failure, each with a `--resume` repro.
  * **Found and fixed (ffh-play).** The session kept its `human` when FfH moved the human to another slot (F23). Once High to Low (or Impersonate Leader) moved the human, the turn of the new human slot never ended: the build before the fix stops `ffh-play soak run --seed 2 --size tiny --civ kuriotates --options challenges` with `Stuck` at turn 50. The session now follows the human after every rules call: it drains the log for the old slot and rebuilds the fog; the active player moves with the human as in C++ (in the session at first, in ffh-core's `reassign_player_advanced` since the rules fixes). `Stats::reassigned` counts the moves. Tests:
    * `decisions.rs` casts Impersonate Leader through `do_command` (a human-only spell, with no `bAllowAI`), plays the impersonated civilization until the timer moves the human back, and checks that a save of it resumes as that player;
    * `soak.rs` `human_only_options`.

    The P2 review found the follow-up's log drain misrouted when the move happens inside `update()`: High to Low reassigns slot 0 as its turn starts (`do_turn`), and slot 0 then plays that turn as an AI in the same update; the Impersonate timer runs out at the end of the impersonated slot's turn (`player_do_turn`), before the AI turns of the same update. Draining that update for the old slot passed the old slot's AI turn to the human (its production results: "Warrior completed in Torrolerial", s2 tiny Bannor `challenges`, turn 50) and dropped the new slot's messages. C++ moves the active player at once, so the old slot's later messages reach nobody. Now the session notes the log's length before each `update()`; when the update moved the human, the entries of the earlier slices are drained for the old slot and the update's own are left to the next drain, for the new slot; after a command (Impersonate cast, the switch-civilizations event) the move is the command's last act and its entries are drained for the old slot first, as before. What is still not exact without a log marker at the move (P7 item 6): the old slot's messages logged earlier in the same update are lost (the start of turn 50 for High to Low; the impersonated civ's end-of-turn production and research for the timer), and the new slot's from before the move are shown (C++ keeps them in that player's message list, which is now the human's). Test: `soak.rs` `a_slot_move_inside_an_update_drains_for_the_new_slot` (no message of the old slot from the turn of the move on).
  * **Reported for ffh-core** (not edited here; the rules fixes then fixed the active player and relaxed the Lyre assertion, see the notes after this list):
    * `reassign_player_advanced` does not move `active_player` as `CvInitCore::reassignPlayerAdvanced` does (`if active == old { active = new } else if active == new { active = old }`, CvInitCore.cpp:474-482). The port's comment calls this UI, but `active_player` gates rules here: QUICK_MOVES mission timers (the options move to the new slot, the active player stays behind), the `doResearch` popup call, and `getDeclareWarMove` visibility. Repro: a human at slot 0 with `active_player = Some(0)`; a Gibbon Goetia of it in player 1's city casts `SPELL_IMPERSONATE_LEADER`; afterwards `players[1].human` is true and `active_player` is still `Some(0)`. The session moves it after the rules call; the rest of that rules call still sees the old value.
    * Log texts with unresolved keys, in addition to P1's city names and placeholders: civilization descriptions ("was captured by the TXT_KEY_CIV_BARBARIAN_DESC") and unit names ("TXT_KEY_UNIT_WARRIOR died attacking TXT_KEY_UNIT_ARCHER"). Display only.
    * **The checked build panics in normal human play** (found by the P2 review, past the matrix's 300 normal-speed turns). `is_building_class_maxed_out` (game/world.rs:488) has a `debug_assert!` for C++'s `FAssertMsg(getBuildingClassCreatedCount(e) <= getMaxGlobalInstances())` (CvGame.cpp:5191), and normal play breaks it: a human takes the Syliven's Perfect Lyre out of a city (SPELL_TAKE_SYLIVENS_PERFECT_LYRE_BUILDING, no `bAllowAI`: human-only) and drops it again (SPELL_DROP_SYLIVENS_PERFECT_LYRE, `CreateBuildingType`); `unit_cast` (unit/ffh.rs:844-849) calls `city_set_num_real_building(c, lyre, 1)`, which counts the wonder as built again (C++ the same: CvUnit.cpp:14589 `setNumRealBuilding(.., true)`, CvCity.cpp:10444 `bFirst`, :10619 `incrementBuildingClassCreatedCount`), so `building_class_created_count` grows past `iMaxGlobalInstances` (1) with each drop, and the next `can_construct` check of the class panics. A debug C++ DLL only shows an assert dialog; release builds play on (the class already counts as maxed out). Repro, checked build (`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true cargo build --release --target-dir target/checked -p ffh-play`): `target/checked/release/ffh-play soak run --seed 9 --size tiny --civ bannor --speed quick --turns 400` panics at turn 276 (through ffh-ai `production::decide` → `city_can_construct`; resuming the turn-275 autosave panics again), and `soak run --seed 2 --size tiny --civ calabim --turns 500` at turn 420 (through `city_do_turn` → `city_pop_order` → `ai_choose_production`); both re-checked after the review fixes, and 9 of the 54 games of the checked quick-speed batch under "Runs" stop on it. Suggested fix in ffh-core: as the C++ assert is reachable, replace the `debug_assert!` by a comment (`// C++ assert (reachable: equipment drop/take re-counts the wonder)`) or a non-panicking check, with a regression test (drop, take, drop, then `city_can_construct`). Each drop also logs "<leader> completes Syliven's Perfect Lyre" to the human again.
    * Apart from that assertion, the soak found no rules crash, invariant failure or save asymmetry in the human branches; the matrix's 300 turns at normal speed did not reach it.
    * **Rules fixes (later).** The active player: `reassign_player_advanced` now moves it as C++ does, so the rest of the rules call that moved the human sees the new value; the session's own move became a `debug_assert!` (ffh-core test `reassign_player_advanced_moves_control`). The Lyre: C++ counts the wonder as created again on every drop just as the port does (the take spell's Python `setNumRealBuilding(i, 0)` does not lower the count, the drop's `setNumRealBuilding(e, true)` raises it), so the C++ assertion is reachable in C++ too; it is now a `// C++ quirk:` comment (ffh-core test `equipment_wonder_drop_counts_it_created_again`: take, drop, take, drop, then the production checks). The log line "<leader> completes Syliven's Perfect Lyre" on each drop is C++'s too (`TXT_KEY_MISC_COMPLETES_WONDER`).
  * **For the ffh-ai owners.** P1 asked P2 to look at the human's weak expansion. Over the 108 design games the AI-as-human ends with 4.8 cities and the AI civs with 5.4. Part of the gap is the rules:
    * Noble's AI-only modifiers (construct 85%, unit supply 50%, inflation 50%) do not apply to humans;
    * a human Kuriotates city stays at radius 2 (no `confirm_settlement` hook, §5.2): human Kuriotates end with 2.6 cities against 5.8.

    The rest is the AI:
    * the starting warrior does not garrison the new capital, which barbarians took at turn 7 in `s2 STANDARD SHEAIM`, killing the human;
    * 7 of the 108 games ended with a dead human, 3 of them that game;
    * ffh-ai neither reads `GAMEOPTION_AGGRESSIVE_AI` nor runs the FfH war scripts;
    * (P2 review) on a human slot the AI moves the Syliven's Perfect Lyre between a unit and the city every 2 turns to no purpose (the human-only take spell, then the drop), which is how the soak reaches the ffh-core assertion above.

    The session's own semantics do not slow the human down: production is deferred to the next control point (no turn lost) and AutoMoves runs the queued moves; traced, the AI-as-human's settler timing matched the AI players'.

**P3 — views and queries (about 2.5k plus 0.4k tests).**
* Scope:
  * `view/*`;
  * the turn report;
  * `text` and `ascii`;
  * `propose_deal` and the `trade` query.
* The bot switches to the protocol-level `Query` and `Command` API.
* Tests: T7 (fuzz, now that legal actions can be enumerated), T8 (fog), T10 (noisy client).
* **As built (done).** No ffh-core, ffh-ai, ffh-cli or `controller.rs` edit; no engine hook was needed (the P7 list stands). About 2.9k lines of library code (`query.rs` 380, `report.rs` 610 with its tests, `view/` 1,850, plus the bot's rewrite) and 1k of tests.
  * **Queries** (`query.rs`): the `Query` enum of §3 plus `setup`; the request form is `{"q": "<name>", ..}` (`Query::from_json` / `to_json`, round trip tested), errors `invalid`, `not_owner`, `gone`, `busy`. `query::run(g, &Ctx, &Query)` answers under a `QueryGuard`; `Ctx` carries what a query needs besides the game (the human, the open event popups, the message store, the last report, the open decision, the saves directory, the `UiState`). `UiState` (in `Session::ui`, an `Arc<Mutex<_>>` shared with the human's controller) holds debug reveal (`Session::set_reveal`; replies then carry `"reveal": true`), the **interface path finder** (`PathFinder::new(FinderKind::Path)`, F12) and `view_version`. `Session::query` / `query_json` answer at any time. `group`, `path` and `combat` answer `busy` when a rules call holds a game finder (`QueryGuard::finders_busy`). `map {since}` is accepted and always answered in full (the deltas are P6).
  * **Queries during a decision** (§2.4): `Answerer::ask_with(&DecisionReader, &Decision)` (default: `ask` with the game) lets an answerer read the protocol's queries while the modal is open, from the deciding player's side, with status phase `decision`; the message log, the report and the saves list are `busy` there (they live in the session, which the rules call is inside); a query that panics is caught (`catch_unwind`) and answered `busy`. The `UiState` lock tolerates poisoning (`UiState::lock`), and the path query runs its search with the interface finder taken out of the lock, so such a panic does not take the session down later (P3 review: a panic while the lock was held poisoned it, and every later query and the game thread's next `view_version` bump panicked). The transports' answerers (P4) build on it.
  * **Fog** (`view/fog.rs`, the `Viewer`'s methods; §4.4): revealed and visible plots, the bonus the team's techs reveal (`plot_get_bonus_type(p, team)`), the revealed improvement, route and owner; own units, else units on visible plots that are not invisible to the team, with **`bCheckCargo`** as C++'s default (a foreign transport's cargo is hidden: a deviation from the design's `false`), shown with their visual owner and without group, moves or experience; own cities, else revealed ones, with size and defence only while visible; met players. Plot yields, defence and culture, and a plot's units, are shown only while the plot is visible: ffh-core's `plot_calculate_yield` and `plot_defense_modifier` have no `bDisplay`/`bHelp` (C++ computes the help from the revealed improvement), so a fogged plot's values would show improvements built since.
  * **`view`** (§4.4): `me` (gold and rate, research with progress, cost, turns and rate, the queue, sliders and which are flexible, anarchy, golden age, state religion, civics, score, the Armageddon counter, the 18 mana counts in the main interface's order, the Stasis counters), `players` (met: leader, civilization, team, war, attitude towards us, open borders, pact, vassalage, score, contact), `cities`, `units` (moves in `MOVE_DENOMINATOR` units), `groups` (ready, busy, activity, mission queue, automation), `pending`, `log_next`. Player names and log messages have their text keys resolved for display (`names::resolve_keys`: ffh-core logs `TXT_KEY_CITY_*`, `TXT_KEY_UNIT_*`, ...; a key without text reads as the type it describes).
  * **`map`**: the arrays of §4.4 (`river` and `yield` encodings in its table), `map_rev` an FNV-1a 64 hash in hex. **`plot`**: terrain, feature, hills, bonus, improvement, route, owner, rivers, city, base move cost; while visible also yields, defence, culture shares of met players, upgrade time left, builds in progress and the units with their strengths.
  * **`group`** (`view/units.rs`): unit details (strength, defence, domain, unit combat, race, religion, promotions, first strikes, withdrawal, collateral, cargo) and the **actions**, each with the command that performs it (`cmd`, canonical): `mission` (no data; build with turns, from `CvDLLWidgetData`'s work rates, and cost; spread, join, construct; lead, one per unit the LEADUNIT popup lists), `target` (an interface mode, `mode`: the exe's action button sets it and the clicked plot completes it; shown, and so enabled, when `CvSelectionGroup::canDoInterfaceMode` (CvSelectionGroup.cpp:1936, ported in `selection.rs`) says so: GO_TO when the selection's domain is not air or immobile, ROUTE_TO for a `UNITAI_WORKER` unit that can build a route, range attack for a unit that can range-strike at all, the air modes as C++; the targets are the plots that pass `canDoInterfaceModeAt` (:2063) and the mission check: `any` for go-to and route-to, the plots in range for range attack and the air modes (possibly none), our cities for airlift; GO_TO's command is `move` (`selectionListMove`, the declare-war question), the others' a `mission`; a `cmd` carries the first target, or the selection's plot as a placeholder that the client replaces. P3 review: P3 offered route-to to any unit with a build (adepts, whose builds are mana nodes), hid the range modes without a target in range, sent GO_TO as a `mission` that skipped the declare-war question, and wrongly said `canDoInterfaceMode` was not in the DLL source; GO_TO_TYPE and GO_TO_ALL are GO_TO on a selection the client makes), `command`, `load` (with the transports), `spell` (with its help), `promotion`, `upgrade` (with the price). Visible actions are `canHandleAction(.., bTestVisible)` over the selection (`selection.rs`), enabled ones without it. Espionage (locked off), automation (F15) and auto-cast (F16) are not listed.
  * **`path`**: the interface finder's path for the selection's group; for a selection that is not a whole group the head's group stands in (`approximate: true`: a partial stack moves at its own speed). Nodes carry the turn of arrival and the moves left; also the war the move would declare and whether its last step attacks a visible defender.
  * **`combat`**: `setCombatPlotHelp` on a visible plot: the selection's attacker (`AI_getBestGroupAttacker(plot, false)`, then with `bForce`; for a partial selection the selected unit with the best exact odds), the defender (FfH's `isAlwaysHostile` rule; only a visible one), the odds of winning and of retreating (per mille), both current strengths (x100), first strikes, collateral, and each side's `CombatDetails` from `maxCombatStr` (true owners left out).
  * **`city`** (own cities; any under debug reveal): plots (worked, by another city, override), specialists (+/- availability), yields, commerces, food, production, happiness and health with their parts, culture, defence, maintenance, crime, trade routes, great people, buildings, religions, garrison, the queue with turns, hurries, and the buildable list (`canTrain/canConstruct/canCreate(.., bTestVisible)`, enabled, cost, turns). Its happiness reads fill the rank caches (F11): `tests/purity.rs` checks the save bytes are unchanged and that the same reads unguarded do change them.
  * **`techs`** (state, prerequisites, depth = the longest prerequisite chain), **`civics`** (availability, upkeep, the anarchy a switch to each would cost), **`religions`** (founded, revealed holy cities, conversion), **`players`** (the view's list plus each player's deals with us and whether we can declare war).
  * **`trade` and `propose_deal`** (`view/diplo.rs`): `trade_table` ports `CvPlayer::buildTradeTable` (both ways; peace, war and embargo items only for an AI side), each item with its `getTradeDenial` (ffh-core ports only the rules-based denials) and, for gold and gold per turn, the maximum: `AI_maxGoldTrade` / `AI_maxGoldPerTurnTrade`'s human formulas (`gold`; `max(0, min(rate + gold / PEACE_TREATY_LENGTH, rate))`); for an AI side the C++ AI's own limits are not ported, so its gold and gold rate bound it (this shows an AI's treasury in the trade screen, which C++ caps). `propose_deal {to, give, get}` (items `{item, data}`, names or indices) is checked before the AI is asked: a civilization we can contact (`canContact`) that is willing to talk (`diplo::ai_willing_to_talk`, a port of `CvPlayerAI::AI_isWillingToTalk`, which gates the exe's diplomacy screen: at war not before the AI's war counter reaches its leader's `iRefuseToTalkWarThreshold`, doubled for a chosen war and shortened when the human is clearly winning, and never for a vassal; P3 review: without it a 1-gold gift ended any war at once, as ffh-ai accepts every gift and `CvDeal::addTrades` makes peace for any deal at war; the peace-time branch, `MEMORY_STOPPED_TRADING_RECENT`, is not ported, as nothing decays AI memories in the port), something traded, every item in its side's table (gold and gold per turn 1 to the maximum), no item twice, `canTradeItem(.., bTestDenial = true)`, a dual item (peace treaty, open borders, pact, alliance) on both sides, as the diplomacy screen places it, and at war a peace deal: the peace treaty or a capitulation (`TRADE_SURRENDER`), as the C++ AI's own war offers and counter-proposals are (`AI_doDiplo`, `AI_counterPropose`), since any deal at war makes peace (P3 review). Then `controller(to).consider_offer(g, to, h, give, get)` runs during validation, under the guard; a declined offer answers `accepted: false` and changes and journals nothing (C++ sends no message); an accepted one is journaled and applies `implementDeal(h, to, give, get, bForce = false)` (`accepted: true`). Tested: no contact (and no gift) while the AI refuses to talk; a gift alone at war refused; a declined peace changes and journals nothing; peace made after 10 turns of war.
  * **`data`** (`view/data.rs`): one array per kind of `names::KINDS` (`{type, name}`; info tables add help, strategy and pedia texts, `null` when missing, and the gameplay fields of the main tables), built once per process. **`text`**: the compact summary (status, empire, a decision's options, events with their effects, votes, research, idle cities, promotions, ready groups with their units, cities, wars, the last report, the latest messages). **`ascii`**: 2 characters per plot, north up, `::` unrevealed, layers terrain (codes from the names, upper case visible, `^` peaks and hills), political, yield and units, with a legend; at most 256 x 256. **`inspect`**: `{:#?}` of an own unit, group, city, our player or team, a deal we are part of (anything, plots included, under debug reveal), or an info entry (`names::Kind::debug`). A plot is inspected under debug reveal only, even one we own (P3 review: its dump holds the true bonus before the tech that reveals it, invisible units and every team's visibility and revealed values; the `plot` query is the fog-filtered plot). The dumps of our own city, player and team keep their per-team bookkeeping (which teams revealed the city, espionage and contact counters): the raw inspector is a rules tester's tool.
  * **Turn report and messages** (`report.rs`): the `MessageStore` numbers the human's messages (`log {since}` answers `{from, next, entries}`; the last 10,000 are kept). When `end_turn` is applied the session takes a fog-filtered `Snapshot` (own units and cities with their buildings, revealed foreign cities, techs, the teams met and the wars among them, the Armageddon counter, owned plots, visible enemies near our cities); at the next control point, or at game over when the game ended in the AI turns (P3 review: the report kept the turn before, without what ended the game), `Report::build` diffs it (under a guard): units lost, gained, ready to promote; cities founded, acquired, lost (and who has them, if seen), grown, shrunk, starving, in disorder; buildings completed; revealed foreign cities gone; techs gained; new contacts (`contacts`); wars and peace between teams met at the snapshot (P3 review: a team met during the AI turns had each of its wars reported as new); enemies sighted near our cities; the Armageddon counter and the thresholds crossed (the units', projects' and triggers' `iPrereqGlobalCounter` plus the Python's 50, 75, 100); borders lost; with the messages since the snapshot and plain-text lines. The snapshot (with its log mark renumbered for the saved message tail) and the last report are optional sidecar members (`report_snapshot`, `report`; older sidecars still read, a P3 snapshot without `met` is dropped); a slot move drops the snapshot. A loaded snapshot whose player, team or plot ids do not fit the save (`Snapshot::fits`) is dropped too (P3 review: a corrupted or mismatched sidecar panicked at the next control point). Tested against the game's state over 80 turns.
  * **The bot** (`bot.rs`) reads only `Session::query` replies (`view`, `map`, `pending`, `group`, `combat`, `city`, `techs`, `civics`, `players`, `trade`) and the static tables, and sends the `group` query's action commands as they are. Policy changes: peace proposals from turn 120 every 5 turns until one is accepted; the capital keeps three defenders from turn 30; spare units gather in the capital and march as a stack (a multi-unit `move`, so the selection is joined) on the nearest enemy city; idle units skip instead of fortifying in the field. `ffh-play bot --verbose` prints the `text` query. **T1 seeds**: with this bot seed 2's human dies to barbarians at turn 54 and none of seeds 1-3 captures a city (a scan of seeds 1-60 found captures in 1 of 60 with the final bot), so T1 runs seeds 1, 3 and 50 (a capture, two keep/raze decisions, at turn 85), and T2, T3 and T4 use seed 50.
  * **Tests**: `tests/views.rs` (every query at control points of a 40-turn game, valid JSON and save bytes unchanged; `propose_deal`; the report against the game; after the P3 review also: the target actions against the interface modes, a team met during the AI turns, a sidecar snapshot that does not fit, the report at game over, a poisoned `UiState`), `tests/fog.rs` (T8: every control point of 120 turns, the view and the map as they go over the wire against the game's state, both ways, plus the ASCII units layer; a stack in fog, a unit invisible to land units, a hidden-nationality unit, debug reveal), `tests/fuzz.rs` (T7: 100 turns, 12 commands and 4 queries per control point; about 180 applied, every refusal kind, 700 decode errors; the slowest command took 4 ms), `tests/purity.rs` (T10: seed 50, 100 turns, about 9,800 queries, 67 inside the 2 decisions; checksums and final save equal to the quiet game; the city screen test), unit tests (query and command JSON, the message store, thresholds, text keys). The suite took about 2 minutes on this host; `views`, `fog`, `fuzz` and `purity` also pass on the checked build (debug assertions and overflow checks).
  * **P3 review fixes** (each with a regression test; no ffh-core, ffh-ai, ffh-cli or `controller.rs` edit, no engine hook): deals at war (`AI_isWillingToTalk`, peace deals only; under `trade and propose_deal`); `inspect` of a plot under debug reveal only; the report's wars among teams met at the snapshot, with contacts; the report at game over; a sidecar snapshot checked against its save; a poison-tolerant `UiState`; the target actions from `canDoInterfaceMode`, GO_TO as `move` and `mission` MOVE_TO asking the declare-war question; the `QueryGuard` swapping the finders instead of cloning them (§2.5). T7's RNG seed is now 7011: the changed action lists changed the fuzz's picks, and its game with 7007 lost the human at turn 38 (the fuzz deletes units and declares wars at random: over seeds 7007-7025 the human lived the 100 turns in 10 of 19 games before these changes and in 9 of 19 after).
  * **Found for other owners** (not edited): ffh-core's `plot_calculate_yield` and `plot_defense_modifier` dropped `bDisplay` / `bHelp`, so a front-end cannot show a fogged plot's yield or defence from the revealed improvement (ffh-play shows them only while visible); the C++ AI's trade limits and judgements (`AI_maxGoldTrade`, `AI_*Trade` denials, the memory decay that ends `AI_isWillingToTalk`'s peace-time refusal) are not ported (ffh-core documents the denials; whether an AI deals is ffh-ai's `consider_offer`).

**P4 — transports (about 1.1k plus 0.3k tests).**
* Scope: `stdio.rs` (`ffh-play agent`), `http.rs`, `server.rs`, `shared.rs`, long-poll, and a placeholder `index.html`.
* Tests: T11 (agent transcript), T12 (HTTP end to end, including the decision raised inside a command).
* After P4, an LLM or script can play a whole game headless.
* **As built (done).** No ffh-core, ffh-ai, ffh-cli or `controller.rs` edit; no engine hook was needed (the P7 list stands). About 2.1k lines with their unit tests (`transport/mod.rs` 780, `stdio.rs` 280, `http.rs` 500, `server.rs` 360, `shared.rs` 210), `assets/index.html` 160, and 0.65k of tests.
  * **The envelope and the host** (`transport/mod.rs`, a file the §6.1 list did not have): `Request::parse` / `from_json` decode a request (`k` = `cmd`, `q`, `answer`, `session`; `id`, an integer or a string, is echoed, `null` when absent; errors name the field); `ok_reply` / `err_reply` build the replies of §4.1 (a command's reply adds `log` and `status`; a confirmation is `{"confirm": ..}`). `Host` is the game-thread state both transports share: the session (or none), its game directory, the watchdog's last diagnosis, the session operations, and `advance`, which returns the event to report (`control {turn, year, human, pending, log_next}`, `over {turn, human, alive, over}`, `stuck {turn, diagnosis}`). Requests inside a blocking decision go through `decision_reply` (queries from the deciding player's side through `DecisionReader`, `answer` with the decision's id, `decision_pending` for commands and session operations, `gone` for another decision's id); requests while the game runs (HTTP only) through `running_reply` (queries answered, `not_your_turn`, `gone`, `busy`). An answer naming an option that is not enabled is refused (`invalid`, the decision stays open), where the in-process `Answerer` takes it as the advisor's pick. Without a game, `data`, `setup` and `status` are answered and everything else gets `no_game`.
  * **Session operations.** `new {setup, name?}`: the settings are checked first, then the game gets a directory `<saves>/<name>` (default `game-<seed>`; `-2`, `-3`, ... when taken). `load {name}` and `take_over {player, name}` take a save of the saves root by name: `NAME` or `auto/NAME` in the current game's directory, `GAME/NAME` or `GAME/auto/NAME` in another's (a trailing `.ffhsave` is allowed); every part must be a save name (`[A-Za-z0-9_.-]`, 1-64, no leading dot), so no name leaves the root (`invalid`); a missing save is `gone`, one that does not load `cannot`; the session continues in that game's directory with a new journal (`journal-N.jsonl`, its `load` header). `save {name}`, `debug {reveal}`, `quit`, and `saves` (every game directory of the root with its saves; the `saves` query lists the current game's). The command line can also start from any path (`--load`, `--take-over`). With `--no-saves` nothing is written (the journal is kept in memory) and operations by name answer `cannot`.
  * **Status** (§4.3): the game's status plus `game {dir, journal, autosave}` (`Host::status`, which the `status` query of both transports answers since the P4 review); the phase is `no_game` without a game, `stuck` (with the diagnosis) after the watchdog stopped a turn (commands get `not_your_turn`, session operations work: load the autosave), `crashed` (with the report, `crash`) after a panic (HTTP; the agent exits). The HTTP server adds `version`, `ms_in_phase` and `map_rev`. Inside a blocking decision the `status` query is the decision's (phase `decision`, no `game`).
  * **`Session::advance`** passes the session to its callback (`&mut dyn FnMut(&Session)`; §3 had `&Game`), so that the HTTP game thread can answer the queries queued during the AI turns with the session's whole context (messages, report, open popups) through `Session::query`; the soak and the tests were adapted.
  * **Agent** (`transport/stdio.rs`, `ffh-play agent`): on the game thread, no other thread. `Io` shares stdin and stdout between the loop and `StdioAnswerer`, each locked for one line. Events: `control`, `decision`, `over`, `stuck`, `no_game` (started without a game), `progress` (only with `--progress`; at most 4 a second), `error` (the start failed: exit code 2), `answered` (a decision the end of input gave to the advisor), `crashed` (a panic: message, location, last autosave, journal and the `ffh-play replay` command; exit code 3). It starts a new game unless `--no-game`, `--load` or `--take-over` says otherwise (`--setup JSON` and `--seed`, `--world`, `--civ`, ... set the settings). At the end of input an open decision takes the advisor's pick, then the agent quits, or with `--on-eof end-turn` ends the turn (forced), reports the next control point and quits. A reply that hands control back (`end_turn`, `auto_moves`, `autoplay`, a new or loaded game) is printed before the AI turns run, then the event.
  * **HTTP** (`transport/{http,server,shared}.rs`, `ffh-play serve`): `Server::start` binds (default 127.0.0.1:8642; port 0 for tests), starts the game thread (16 MB stack, as rules code recurses) and reports a start that failed (a bad `--setup` or save: exit code 2), then the acceptor (one thread per connection, at most 64 at once, more get 503). Requests: the head at most 16 KiB (431), `Content-Length` bodies only, at most 1 MiB (chunked 400, a POST without a length 411, larger 413), the whole request within 15 s of the connection (408; a deadline since the P4 review); every response `Connection: close`, `Cache-Control: no-store` (the data: `no-cache` with an ETag, the FNV of its JSON; `If-None-Match` gives 304). `POST /api` needs `X-FFH: 1` (403). Beyond the design, on a loopback address every request must name a loopback host (`Host: 127.0.0.1`, `localhost`, `[::1]`, with a port; 403): a page served from another name (DNS rebinding) could otherwise send the custom header same-origin. Assets: the embedded `index.html`, or `--assets DIR` (paths of 1-4 save-name parts, so nothing outside the directory). The game thread caches the view and the map (serialized, `Arc<String>`) and publishes the status before each reply (after a command or a session operation; a query changes nothing), after `advance`, and at a decision (the view and map then from the deciding player's side, `DecisionReader::query` catching a panic so a failed view keeps the previous one); during the AI turns it publishes at most every 20 ms (`PUBLISH_EVERY`), always with phase `running` (in the slice that activates the human's turn the predicate already says `human`, but `advance` may still send AutoMoves: a client acting on it got `not_your_turn`; T12 met this in a full test run), and answers the queued envelopes between turn slices. A reply that hands over to the game (`After::Advance`: an ended turn, a new or loaded game) carries phase `running` for the same reason. `/api/wait` waits on a `Condvar` until `version > since` (at most 60 s, also released when the server quits). The game thread runs every request under `catch_unwind`: a panic (its message and location recorded per thread by the panic hook, so that another thread's panic cannot take its place) drops the session, the status becomes `crashed` with the report, the request in progress gets `{"error": "crashed"}`, and the server stays up (the status stays `crashed` until a game is started or loaded). `quit` stops the server (the acceptor is woken by a connection to itself).
  * **`--panic-at T`** (agent and serve) panics once in `advance` when the game reaches turn T (also at the start of `advance` when the game is already there, e.g. a save of turn T loaded): a self-test of the crash report, as the soak's.
  * **Placeholder page** (`assets/index.html`): the status (long-poll), the `text` summary and the `ascii` map (layers), the blocking decision with its options and "AI default", new game (seed, world size), end turn (plain and forced), save, debug reveal, and a box for raw requests. Checked in headless Chromium (Playwright, F21): no console error or failed request; a new game and an ended turn through the page.
  * **Tests.** `tests/agent.rs` (T11): the transcript with a capture's keep/raze decision (staged with rules calls in a save that `tests/common` writes, as `decisions.rs` stages it): `control`, replies, `decision`, a view and the status inside it, the message log `busy`, `cmd` and `session` `decision_pending`, a wrong decision id `gone`, a disabled option and a broken line `invalid`, the answer, the command's reply with the capture message, the end of turn and `control`, and the answer journaled; the end-of-input policies (quit; end-turn) on a script that ends inside the decision; a game started and loaded over the protocol, save names that leave the root, `quit`; a panic's `crashed` event and its replay command run. `tests/http.rs` (T12): the page, the tables (with the ETag), the setup lists, no game, the header, host, length and path checks; a new game played for 10 turns through commands (found the capital from the `group` query's action, research, production, forced end turns) and long-polls, the view and map caches against the status, the journal, saves and save names that leave the root, a load, debug reveal, `quit`; the capture's decision answered on another connection while the move's request waits (a long-poll sees `phase: decision`, the view and a query are served, a command and a save get `decision_pending`), then the move's reply; a crash that leaves the server up and the autosave loaded by name. Unit tests: the envelope's decoding and errors, save-name resolution, requests while the game runs, the agent without a game and with a bad start, HTTP parsing, hosts and assets, the long-poll. The suite's new tests take about 3 seconds.
  * **Not done here:** `/api/map?since` is still answered in full (P6 deltas); the bench guard and T13 are P6; the browser client is P5.
* **P4 review fixes** (each with a regression test; no ffh-core, ffh-ai, ffh-cli or `controller.rs` edit, no engine hook):
  * **AutoMoves between commands** (major). The exe runs `updateTestEndTurn` every frame: once the human has no busy and no ready unit but an auto unit, it sends `CvNetAutoMoves`, so multi-turn orders move during the turn (and a group that arrives with moves left, or whose orders `autoMission` drops, is handed back as ready). The transports called `advance` only after a command that left the human's phase, so the rule ran only at a turn's first control point. `Session::auto_moves_due` says when the rule would fire at a control point (nothing ready, an auto unit, not yet fired in this control episode); `Host::command` then returns `After::Advance`, the reply says `running`, and the transport runs `advance` (the journaled `auto_moves`) and reports the control point after it (`control` over stdio). In-process clients (the bot, the soak) keep calling `advance` themselves. T11 and T12 skip the last ready unit of a staged save (`tests/common::staged_auto_moves`) while another group has a queued move.
  * **One status.** The `status` query is `Host::status` with or without a game (`game`, `stuck`, `crashed` with the report; `Host::crashed` drops the session and keeps the report until a game is started or loaded). Between turn slices (HTTP, `running_reply`) it says `running`, with `game`, as the published status does (`running_status`).
  * **Crash report of a loaded game.** Before a session's first autosave the save it was loaded or taken over from stands for the autosave (`Host` keeps it): the report's `repro` is `ffh-play replay <journal> --from <that save>`, which replay accepts (the journal's `load` / `take_over` header names it by FNV).
  * **`view_version`** is bumped when debug reveal changes and when a blocking decision opens and closes, so the decision's status and cached view (the decision-time state) and the view after it each carry their own version.
  * **Decision texts** (title, text, option labels and help) pass through `names::resolve_keys` (the keep/raze text showed the city's text key).
  * **Strict members.** A `setup` (session `new`, `--setup`), every command (`command::COMMAND_FIELDS`, plus the envelope's `id` and `k`), session operations and answers refuse unknown members (`invalid`, "field `X`: unknown field"): a typo no longer starts a default game or drops a flag. Queries stay lenient. `inspect` names its object `oid`.
  * **Game names.** `session new {name: "auto"}` is refused (`auto/NAME` names the current game's autosaves); `saves` skips a directory called `auto`.
  * **HTTP.** The request time is a deadline for the whole request (each read waits at most the time left, then 408), not a per-read timeout that a client trickling bytes could stretch for hours. At the connection cap the 503 is written, the write side shut and the request drained on a short-lived thread (at most 64 at once), so the client reads the 503 instead of a reset. Bound to another loopback address (`--bind 127.0.0.2:PORT`), the `Host` check also accepts that address. The placeholder page keeps an `id` typed in its raw request box.

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
* **As built (done).** No ffh-core, ffh-ai, ffh-cli or `controller.rs` edit; no engine hook and no protocol change was needed (the P7 list stands). About 3.8k lines of JS, 200 of CSS and 55 of HTML; 0.24k of Rust (`src/stage.rs`) plus 0.17k of Rust tests; 0.5k of node tests and 1k of tool scripts. Everything the client shows comes from the P3/P4 protocol as it was: the long-polled status, the cached `/api/view` and `/api/map`, `/api/data`, `/api/setup`, and `POST /api` (queries `group`, `path`, `combat`, `plot`, `city`, `techs`, `log`, `report`; the commands; `answer`; the session operations).
  * **Files.** `assets/` has the URL layout (`index.html`, `app/*.js`, `app/style.css`; the §6.1 list said `assets/js/`, `assets/style.css`), so `--assets crates/ffh-play/assets` serves the same files from disk. `http::ASSETS` embeds all 15 with `include_str!`; `tests/web.rs` fails when a file of `assets/app/` is not listed, when the embedded and on-disk contents differ, when the page or a module references a file that is not embedded (or anything but a relative module: no CDN), or when an embedded module is imported by none. Modules (one concern each): `api.js` (the envelope, GETs, the long-poll loop; no DOM, so node imports it), `state.js` (the store, names from the data tables, view lookups, the deferred-popup queue, vote labels, the gate's lines; pure), `tiles.js` (wrap-aware screen and map coordinates, the map arrays' encodings, the terrain cache's dirty chunks, colours, glyphs; pure), `markup.js` (Civ4 markup to DOM nodes, never HTML), `dom.js`, `modals.js` (a dialog stack, confirmations as promises, the blocking decision), `commands.js` (one command at a time and the exe's confirmations: declare war on a move, delete, world and war spells, the transport picker, the end-turn gate), `map.js`, `units.js`, `city.js`, `screens.js`, `hud.js` and `main.js` (the status loop). `dom.js`, `commands.js`, `hud.js` and `tiles.js` are additions to the §6.1 list.
  * **Map** (`map.js`): canvas, north up, the east-west wrap drawn as copies; the terrain (terrain colours by type with a hash fallback, hills hatched, peaks, feature glyphs, rivers on the south and east edges, roads and railroads toward their neighbours, resources as a lettered dot (mana purple), improvements as a lettered box (☠ lairs and barbarian spawns, ✦ mana nodes, ? goody huts), borders from `owner` as a tint and edge lines, fog dimmed, unrevealed black) is drawn into offscreen chunks of 8 x 8 plots per zoom level and device pixel ratio, redrawn only where the map arrays changed (a plot's chunk and its neighbours', for borders and roads) and kept within a pixel budget (least recently used first); cities (size, name, capital ★, own production and turns, idle), unit stacks (owner colour, the top unit's abbreviation, health bar, moves dot, count badge), the selection, the hovered plot, the path preview (a line with a marker and the turn at each turn's end, orange for war, red for an attack) and interface-mode targets are drawn each frame. Six zoom levels, drag to pan, the wheel zooms about the cursor, arrows; a minimap (an image of the map arrays, owner colours, the viewport) centres on click. The tooltip: the plot from the map arrays and the view at once, then the `plot` query (yields, defence, move cost), the `path` query and, over foreign units, the `combat` query (odds, retreat, strengths, the non-zero CombatDetails modifiers of both sides), debounced 90 ms. The yield and political layers are P6.
  * **Units** (`units.js`): click selects the first ready group on a plot (again: the next group there; Shift: every unit), the stack list (click a row: its group; Ctrl: toggle the unit), All, Group (`join_group` into the first unit's group), Split (`join_group` with no head), Next; right-click moves (`move`; Shift appends), `M` then click; the actions panel lists the `group` query's actions by section (orders and interface modes, builds with turns, spells with help, promotions, upgrades with price), each sending its `cmd` as is; a `target` action enters an interface mode (its targets outlined, or any plot for go-to and route-to) and puts the picked plot in. After a command on the selection, once its group no longer waits for orders, the nearest waiting group is selected. Hotkeys: Space skip, F fortify (else sleep), S sleep, H heal, B found, Del delete, M move, W/Tab next, C centre, R research, Enter end turn (Shift: forced), Esc.
  * **Turn flow** (`main.js`): the status loop (`GET /api/status`, then `/api/wait` since its version; after each command the status is read at once) refreshes the view when `view_version`, the turn, the phase or the game (journal path) change, and the map when `map_rev` does; at a new turn's control point it reads the `report` (the bottom panel's first tab) and selects the first waiting group, then shows the deferred popups one at a time in C++ order (events with each option's effects and `enabled`, votes with the popup's choices, research, idle cities; "Later" sets one aside for the turn; the end-turn gate's "Answer them" brings them back). The blocking decision is a dialog that cannot be closed (options with help, the advisor's pick marked, "AI default"), shown whenever the status carries one, also after a reload; its answer goes on its own request while the command that raised it waits. During the AI turns the top bar shows the active players and the seconds. Messages (`log` from `log_next`) are grouped by turn; a command's own messages also show as toasts. The no-game, game-over (winner, victory, scores), crash (the report with the replay command, "Load the autosave") and stuck screens follow the phase.
  * **Cities and empire**: the production dialog (`city` query: summary, the queue with ✕ = `pop_order`, the buildable list by kind with cost and turns; click replaces, Shift appends, Ctrl inserts first: `push_order`'s `shift`/`ctrl`, which the server rewrites as `cityPushOrder` does) serves both the idle-city popup and the city opened by double-click, the selection panel or the Cities list; the research dialog (`techs`: researchable techs by turns, the current one and the queue; Shift queues); the top bar (turn, year, phase, gold and rate, research with progress, the commerce split, the Armageddon counter, mana, anarchy, golden age, Stasis, idle AI-controlled groups, score); new game (every `setup` member, options and victories), save, load (every game directory and its saves), a raw request box. The full city screen, civics, religion, sliders, diplomacy, the tech tree, the inspector, debug reveal, autoplay and take-over screens are P6 (the raw request box reaches them meanwhile).
  * **`ffh-play stage capture|attack|automoves|load DIR`** (`src/stage.rs`, new): the transports' tests' staged saves (a capture's keep/raze decision, the AutoMoves rule) moved from `tests/common` into the library, plus `attack` (a warrior next to a barbarian warrior) and (P5 review) `load` (a warrior and two triremes in a coastal capital: the transport picker), written to a directory (only `DIR/KIND.ffhsave` and its sidecar since the P5 review) and printed as JSON, so that the smoke tests and a human tester can reach a decision, an attack or the picker without playing for it. `tests/common` calls them; `tests/web.rs` checks the attack's `combat` and `path` replies and the load's picker.
  * **Tests.** `tools/web-check.sh`: `node --check` of every module and script, `node --test assets/test/*.test.mjs` (23 tests: wrap and coordinates across the seam, Civ4's plot distance, the camera, river and yield encodings, dirty chunks, labels and the top unit, path markers; markup tokens, plain text and rendering with a fake document (no text becomes HTML); names, view lookups, cycling, the deferred popups' order and keys, vote labels, the gate's lines, moves, save names from paths; the envelope, ids, refusals, 409, ETags and the long-poll loop against a fake `fetch`), skipped without node; `--smoke` and `--browser` add the two below. `tools/web-api-smoke.mjs` (the node smoke client, through the page's own `api.js` and `state.js`): starts `serve --port 0`, follows the page's imports from `/` and checks every file is served with its type, `/api/data`'s ETag, no game (409); plays 20 turns as the page does (the deferred popups, research, production, founding, builds, moves along `path` previews, the end-turn gate), checking the cached view and map against the status (versions, `map_rev`, units only on revealed or visible plots, cities in the map's array) and every log index; saves, lists, loads and compares; loads `stage capture` and moves onto the city: the move's request is still open while the status shows the keep/raze decision, a command gets `decision_pending`, the answer on another request closes it, then the move's reply arrives and the city is ours and idle; quits (the server exits 0). Any refusal of something the server offered fails it. Seeds 3 and 5, tiny and small maps, and 120-turn runs (the human dies at turn 79 on seed 3 small: the game-over path) passed. `tools/web-smoke.mjs` (headless Chromium, F21; Playwright when the package resolves, normally or from node's global modules, else Chromium over the DevTools protocol with node's `WebSocket`: both drivers pass): the new-game screen, a game started through the form, founding with `B`, the research then the production popup answered by clicks, a unit selected by clicking the canvas, the path preview on hover, a right-click move, four turns ended with `Enter` through the gate, the turn report, a save through the dialog and its load through the load screen, `stage attack` (the odds in the tooltip, then the attack), `stage capture` (the keep/raze dialog answered), and the crash screen of a server started with `--panic-at 1` (its report names the replay command); it fails on any console error, page error, failed request or HTTP error (requests a navigation aborts excepted) and saves 11 screenshots under `target/web-smoke` (not committed: about 50-100 KB each). About 15 s.
  * **Deviations and refinements.** (1) Cycling goes to the nearest waiting group in `pending.ready_groups` order (not `m_groupCycle`; P6, which owns cycling, can add the cycle to the view); since the P5 review W, Tab and Next take the nearest group not yet selected in the current cycle, so they reach every waiting group. (2) The actions panel lists every action of the `group` query, spells, promotions, upgrades and load included, with confirmations; P6 refines their screens (the spell help is the XML help). (3) The commerce split is shown, not changed (sliders are P6). (4) The map is fetched whole whenever `map_rev` changes; the client redraws only the chunks whose plots changed (P6: deltas over the wire). (5) Hotkeys beyond the design's list: H heal, B found (the design's "build menu" is the Build section), R research, Del delete, M move, `?` keys. (6) `ffh-play stage` and `src/stage.rs` are new (testing aids). (7) No yield layer: the design's MVP list has it, P6's scope too; it is P6's.
* **P5 review fixes** (each with a regression test in the smoke tests, `assets/test` or `tests/web.rs`; no ffh-core, ffh-ai, ffh-cli or `controller.rs` edit, no engine hook, no protocol change):
  * **The transport picker** (major). The answer to `confirm load_unit` is `do_command COMMAND_LOAD_UNIT` with `data1` = the transport's owner and `data2` = its id, as the LOADUNIT popup sends it (CvDLLButtonPopup.cpp `OnOkClicked`: `selectionListGameNetMessage(GAMEMESSAGE_DO_COMMAND, COMMAND_LOAD_UNIT, pLoopUnit->getOwnerINLINE(), pLoopUnit->getID())`); the client sent the id as `data1`, which the server always refused, so loading with two transports never worked. `state.js` `loadUnitCommand` takes the owner from the view (a teammate's transport). New `ffh-play stage load` (a warrior and two triremes in a coastal capital); `tests/web.rs`, the node smoke client and the browser smoke test load through the picker.
  * **Popups after another game** (major). The deferred popups set aside ("Later") were remembered by key until the turn number changed, so a new game, a load or the crash screen's autosave at the same turn showed none of its own (an idle city could stay idle through the turn: the gate does not count idle cities). C++ saves the popup queue with the player (CvPlayer.cpp `m_listPopups`, read :16889, written from :17411 with the popups on display), so a load shows them again: the dismissals are now cleared whenever the game changes (a new journal, no game, `game-started`).
  * **W, Tab and Next** (major) went back and forth between the two waiting groups nearest each other (only the current group was skipped). `state.js` `cycleNext` keeps the groups the current cycle selected (per game and turn) and takes the nearest one not yet selected, starting a new cycle once every waiting group was; after a command and at a new turn the nearest waiting group is still chosen.
  * **Shift waypoints** (major). After the first Shift-right-click the group had orders, so the selection moved on and the next waypoint went to another unit; and when the group was the last one waiting, the AutoMoves rule (P4) moved it along the first leg before the next waypoint was added, where the exe holds AutoMoves while Shift is down (`CvGame::updateTestEndTurn`: `if (!(gDLL->shiftKey())) sendAutoMoves();`). The client now holds the waypoints while Shift is down (numbered on the map, a hint in the panel, Esc drops them, the selection stays) and sends them when Shift is released (or the window loses the focus) as `move` with `append`, in order, waiting for the control point between them (the first may let the AutoMoves rule run). The end state is the exe's: each leg is appended to the group's queue (`pushMission(.., bAppend = true)` does not start it), and AutoMoves runs once nothing is left waiting. The protocol needs no "Shift is down" flag, so no hook.
  * **Stale actions** (minor, two findings). After a selection change the previous selection's `group` reply was drawn under the new header and served the hotkeys until the new reply arrived (a round trip: a few ms, more behind a queued `path` query), so B, Space, F, S or Del pressed at once ordered the units no longer selected (a city founded by the previous settler). The reply now carries the units it answers; the panel and the hotkeys use it only for those units ("…" meanwhile), a hotkey waits for the selection's own reply, and an action of other units is not sent.
  * **Minors.** The production queue shows ✕ only while it holds more than one order (the exe sends POP_ORDER only then, CvGameInterface.cpp `getOrderQueueLength() > 1`; the server refuses it). `ffh-play stage` plays the staged game with no journal and no autosaves, so it writes only `DIR/KIND.ffhsave` and its sidecar (it wrote a journal and `auto/start`, `auto/T0000`.. into `DIR` and pruned the autosave ring there, which a game kept in `DIR` would lose). The New game dialog the no-game screen forces (it cannot be closed) has its own tag and closes once the phase is not `no_game` (a game started by another client, a restarted server); a game-over screen closes once the game is no longer over. A dialog button whose action returns a promise (Start, Save, "Load the autosave", "AI default") disables the dialog's buttons until it settles, the load screen loads one save at a time and Enter in the save dialog is its Save button: a double click on Start started two games.
  * **Tests.** `tools/web-smoke.mjs` now also: double-clicks Start (one game directory); presses W over every waiting group; selects another group and presses B (nothing sent) or Space (that group's skip) in the same tick as the selection change; the city queue (no ✕ on one order, Shift-click appends, ✕ removes); after `stage capture`, "Later" on the research and city popups of `stage load`, loads the same save again (the popups return) and loads the warrior through the picker; on `stage automoves` holds Shift over two right-clicks (nothing sent, the selection kept), releases it (both legs sent for the warrior, appended, its orders end at the second); a game started by another client closes the no-game dialog. 13 screenshots, about 22 s. Each new step fails on the code before its fix. `tools/web-api-smoke.mjs` answers the picker of `stage load` with `loadUnitCommand`. `assets/test/state.test.mjs`: the review's three-group cycle, `loadUnitCommand`, `clearDismissed` (25 node tests). `tests/web.rs`: `stage load` writes only its save, its `load` action offers both triremes and asks, the id as `data1` is refused and owner and id load.

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
* **As built (done).** No ffh-core, ffh-ai, ffh-cli or `controller.rs` edit; no engine hook was needed (the P7 list stands). About 1.8k lines of JS added (the client is 5.8k lines with its CSS), 1.25k of Rust (`bench.rs` 0.57k, `view/map.rs` deltas 0.27k, `view/hotkeys.rs` 0.18k, the query additions), 0.9k of tests and tool scripts.
  * **Protocol additions** (all backward compatible): map tile deltas (§4.4 as built); the view's `group_cycle` (`CvPlayer::m_groupCycle`, the player's groups in cycle order) and `has_auto_unit` (`CvPlayer::hasAutoUnit`), `me.options` (the player options, by `PlayerOptionType`) and `me.ai_auto_play`; the `city` query's `plus` / `minus` per specialist (`command::specialist_buttons`, the very check `do_task TASK_CHANGE_SPECIALIST` makes), `can_work` per plot, `emphasize` (per `EmphasizeType`) and `extra_population`; `civics {with: [civic, ..]}` adds `proposed {civics, changed, valid, anarchy, upkeep}`: the set the revolution would make, whether `update_civics` with those civics is applied (`canDoCivics` for each new civic, one per option, `canRevolution` of the set), and its anarchy and upkeep (`getCivicAnarchyLength`, `getCivicUpkeep`); trade items of `TRADE_CITIES` carry the city's `name`; `/api/data` entries of `MissionType`, `CommandType` and `BuildType` carry `keys` (`[{key: "KB_F", alt, shift, ctrl, priority}]`, the `<HotKey>` and `<HotKeyAlt>` bindings) and `order` (`iOrderPriority`), plus `interface_modes` (the modes' bindings) and `defines.commerce_percent_change_increments`. ffh-data does not load the interface fields, so `view/hotkeys.rs` reads them from the vendored XML; `CIV4InterfaceModeInfos.xml` is not vendored, so its bindings are written into `hotkeys.rs` from the reference file.
  * **Client** (new modules `empire.js`, `diplo.js`, `hotkeys.js`, `tools.js`; `http::ASSETS` embeds 19 files). *City screen* (`city.js`, rewritten): the plots of the city's radius with their yields (worked, the centre, worked by another city, unrevealed, foreign), a click sends `TASK_CHANGE_WORKING_PLOT` (Civ4's `alterWorkingPlot`: a worked plot's citizen becomes the default specialist, and working a plot by hand turns the automatic citizens off); the specialists with `−`/`+` as the server enables them; "Automate citizens"; food, production, commerce, happiness and health with their parts, culture, great people, maintenance; buildings, religions, garrison; hurry (each `HurryType` with its gold or population price), draft, the emphasis flags, "Automate production"; the queue and the chooser as in P5; previous and next city. *Empire* (`empire.js`): the tech tree (columns by prerequisite depth, rows ordered by the prerequisites' rows to cut crossings, SVG lines for the AND and OR prerequisites; click `research`, Shift-click queues), civics (picks per option, the proposal from `civics {with}`, "Revolution!" sends `update_civics`), religion (`religions`, `convert`), the sliders (`percent_change` by `COMMERCE_PERCENT_CHANGE_INCREMENTS`, flexible commerces only; redrawn with each new view). *Diplomacy* (`diplo.js`): the players met (relations, attitude, score, deals), `declare_war` (asks first), the trade screen from the `trade` tables (refusals greyed with their `DenialType`, gold with an amount up to the maximum, the dual items mirrored on both sides as the exe places them), `propose_deal` with the answer. *Units* (`units.js`): the action sections in Civ4's order and, within each, the action list's order (`iOrderPriority`, highest first: `CvXMLLoadUtility::SetGlobalActionInfo`); each button shows its key; "Promote…" and the spells' "details" open dialogs with the XML help; cargo is listed under its transport. *Map* (`map.js`): the political layer (each owner's colour over its plots, unowned land grey) and the yield layer (the packed `yield` array as three coloured numbers), cached per layer like the terrain; the map follows the deltas and redraws only the chunks of the changed tiles. *Tools* (`tools.js`): debug reveal, the inspector (kind and id, with quick picks for the selection, its group, the city, the plot under reveal, the player and team), AI autoplay (`autoplay {turns}`), take-over (`session take_over`), the player options (`set_option`), the raw request box, and the spectator mode.
  * **Cycling** (`state.js` `cycleGroups`, `units.js`): W, Enter (while groups wait), Tab and Next port `CvPlayer::cycleSelectionGroups` over `view.group_cycle` (the first group ready to select after the selection's place, going round; Shift+Tab backwards) and `CvGame::cycleSelectionGroups` (when the search went round and `has_auto_unit`, the exe sends AutoMoves: `auto_moves`); with no selection, the selectable group nearest the centre of the view (`findSelectionGroup` at the look-at plot). After a command, once the selected group no longer waits (`readyToSelect(true)`: `pending.ready_groups`), `CvGame::updateSelectionList`: the next selectable group on the plot the group left, then on its plot (`cyclePlotUnits(.., bAuto)`), then the next in the cycle, else no selection; nothing with `PLAYEROPTION_NO_UNIT_CYCLING`. "Ready to select" is C++ `readyToSelect()` with its default `bAny = false` (every unit can move, `canAllMove`), the view's per-group `selectable` (P6 review: the cycle first used `pending.ready_groups`, `readyToMove(bAny = true)`, so a stack with one unit out of moves kept coming back where Civ4 skips it); `pending.ready_groups` stays the end-turn gate's count and the "does the selected group still wait" test. This replaces P5's nearest-not-yet-visited cycle (`cycleNext`, removed), which already reached every group but in another order. A command the player gives while such an automatic command (AutoMoves, a spectated turn, grouping) is on its way now waits for it (`commands.js`); before, it was dropped like a second click, so a click right after the cycle came round did nothing (the browser smoke test met it on a production popup).
  * **Hotkeys** (`hotkeys.js`; `main.js`): a key runs, among the selection's enabled actions bound to it (the XML bindings, matched on `KeyboardEvent.code` so that a keyboard layout does not move them, with the modifiers exact), the one with the highest `iHotKeyPriority`, ties in the action list's order: F fortifies (or sleeps: both are bound to F, a unit offers one), S sentries, B founds (priority 1) before bombard or a ranged attack, R builds a road (a worker) before recon, G enters go-to, Alt+R route-to, Shift+P pillages, Space skips, Del deletes, Backspace cancels, L loads, U unloads. The exe's dispatch is not in the DLL source, so this rule is inferred from the data (it gives the bindings their intended meaning in every case of the vendored XML). The global keys are Civ4's controls (`CIV4ControlInfos.xml`, `CvGame::canDoControl` / `doControl`; P6 review: they first only claimed to be): Enter is CONTROL_ENDTURN (priority 1) while the end-turn message is up and CONTROL_CYCLEUNIT otherwise, the end-turn message being `updateTestEndTurn`'s `!hasReadyUnit(bAny)` with `bAny` when a unit is selected and unit cycling is on (`state.js` `endTurnMessage`; the End turn button glows then); Shift+Enter CONTROL_FORCEENDTURN (`end_turn {force}`); Space CONTROL_ENDTURN_ALT when the selection's actions do not take it (no selection, or a group that cannot skip; with a selection Space is MISSION_SKIP's key); W CONTROL_CYCLEUNIT_ALT; C CONTROL_CENTERONSELECTION; Home / End (and Num+ / Num-) CONTROL_NEXTCITY / PREVCITY (`cycleCities`: the next / previous own city after the one Home / End selected, else the one nearest the centre of the view, which stands in for the exe's `selectLookAtCity`; the city screen moves to that city when it is up); F10 CONTROL_SELECTCAPITAL (the capital's city screen); F1 domestic (cities), F2 financial (sliders), F3 civics, F4 foreign, F6 tech, F7 religion; Ctrl+S, Ctrl+L. ffh-play's own: Tab / Shift+Tab cycle forward / back (Civ4's Tab is team chat, Shift+Tab all chat; there is no chat), Ctrl+1/2/3 the layers, `?` the keys list, the arrows and `+`/`-` the camera. Not ported: the period and comma (the next unit on the plot: a click on the plot does it), slash and backslash (workers, last unit), the camera, globe, grid and screen controls ffh-play has no screen for. With Ctrl only keys an action uses are taken from the browser. Changed from P5: M (move) is now Civ4's G, R is the road (research: the top bar, the Research button, F6), S is sentry (P5: sleep).
  * **The spectator mode** (`tools.js`): debug reveal on, then `autoplay {turns: 1}` at each of the human's turns (FfH's `setAIAutoPlay` for the human's slot, so the AI plays every civilization; at the start of the next turn the slot is the human's again and the next step begins), with Pause, Step and Stop in the top bar (Stop restores the reveal setting); the deferred popups and the turn's first selection are not shown meanwhile. A new game can start in it (the new-game screen's "Spectate").
  * **`ffh-play stage trade DIR`** (new in `src/stage.rs`): turn 4 of a duel with the AI met at peace, Currency (gold trading) given to the human's team and 100 gold each, so the trade tables hold gold both ways; `tests/web.rs` checks its tables and an accepted gift, the browser smoke test trades through the screen.
  * **Fix in the transport.** On `quit` the acceptor waits (at most 2 s) for the connections still answering before `Server::wait` returns: the quit's own answer was sometimes lost (the process exited before the connection thread wrote it; seen in about one node smoke run in three under load).
  * **T13** (`src/bench.rs`, `ffh-play bench [--quick] [--runs R] [--json]`, `tests/bench.rs` `#[ignore]`d; `tests/screens.rs` runs the quick variant for correctness): (1) the design's check, the AI time per turn of a game the test bot plays (the AI turns after its end of turn) against `ffh bench` on the same seed, size and civilizations (budget +10%); these are two different games (`ffh bench` has an AI in slot 0, the bot plays weakly and its empire stays small), so it is loose, and (2) isolates the session's own cost on one trajectory: from a save at the first control point the AI plays every slot (`autoplay`) as a bare `Game::update()` loop with the same controllers, through `Session::advance` with the journal in memory and no autosave, and with the journal and autosaves in a directory; the three end with the same checksum. The budget (+10%) is on the session's own work measured inside its runs (`session::AdvanceProfile`, P6 review: the phase predicate, the slot-move check, the log drain and its fog, the turn-boundary checkpoint and its checksum, the per-slice callback, against the `update()` slices of the same runs, both read around each slice, so host load reaches both alike; the median of the runs); the three ways are also timed whole (the thread's CPU time, `/proc/thread-self/schedstat`), interleaved over `--runs` rounds (5), and reported with their best and slowest, not budgeted. (3) Command latency (`execute` with its settle, plus the `view` query) at a control point of a standard map at turn 200 reached by autoplay (budget 50 ms); (4) the view build on a huge map at turn 100 (budget 10 ms) and the QueryGuard (budget 1 ms). Results on this host after the P6 review (4 cores shared with other work, load average 6-7; seed 1, standard, 6 civilizations, 120 turns; three runs of `ffh-play bench`, about a minute each): (1) the test bot's game 4.55-4.75 ms of AI time a turn against `ffh bench`'s 16.9-21.8 ms (`ffh bench` reports wall time, which follows the host's load; the bot's game is much smaller); (2) inside the session's runs the `update()` slices take 17.0-17.8 ms a turn and the session's own work 0.040-0.046 ms (0.2%, budget 10%), the autosave 7.3-8.1 ms; timed whole, the bare loop's best of 5 was 15.5-16.5 ms a turn (its slowest 16.6-20.6), the session's 14.7-15.3 ms (-3.9% to -7.1%: in every run the session's best beat the bare loop's, which shows that the whole-run ratio measures the host and the run order, not the session), with autosaves 22.1-22.3 ms (+34% to +44%); the same checksum every time; (3) 8 commands timed at turn 200 (the human slot's whole offer at that control point), median 0.14-0.17 ms, slowest 0.26-0.27 ms; (4) the view on a huge map at turn 100 (10,240 plots, 9 civilizations, seed 2: seed 1's human died before) 0.14-0.23 ms median (the full map 2.5-5.0 ms), the QueryGuard 1.9-2.4 µs and the status query 2.7-4.5 µs. All budgets hold. Before the review fix the budgeted figure was that whole-run ratio, the best of two 120-turn runs: the review measured +10.5%, +26.1%, +26.6% and -9.6% on the same binary and trajectory (the bare loop alone 12.2 to 16.4 ms a turn) and +3.8% / -3.2% over 250 turns; "CPU time, so other processes do not count" was wrong: the thread's CPU time leaves out the time other processes run, not their effect on the caches, the memory bandwidth and the clock. The autosave (`Game::save` and the file write at each turn boundary) is the session's largest cost; it is reported rather than budgeted (a faster `Game::save` in ffh-core, or an autosave interval, would cut it: a note for P7, not a hook). If the human dies before turn 200 (100 on huge) the bench takes the next seed and says which.
  * **Tests.** `tests/screens.rs` (6): a client applying deltas holds the full map at every control point of a bot's 25-turn game, also across debug reveal on and off (and gets the full map for another epoch, an unknown revision or none; an empty delta when nothing changed); the city screen's `plus`/`minus` are exactly the specialists `do_task` accepts, a plot click toggles it and stops the automation, the emphasis flags follow the task; `civics {with}` is valid exactly when `update_civics` is applied (each civic alone, on copies of the game), with its anarchy and upkeep; the view's cycle, options and the data's hotkeys; trade tables name the cities; the quick bench plays the same game three ways. `tests/http.rs` follows the map by deltas during its 10 turns. Unit tests: `MapTracker` (`view/map.rs`), the hotkeys of the vendored XML (`view/hotkeys.rs`), the long-poll test with a real map (`transport/shared.rs`). Node (32 tests): Civ4's group cycle, the waiting groups on a plot, the player options, trade item labels, map deltas (applied on a copy, refused when they do not fit), the hotkeys (key names, matching, labels, the action order, which action a key runs), the tech tree's layout. `tools/web-api-smoke.mjs` also follows the map by deltas and compares it with the full map at every check, uses the screens' requests at turn 8 and at the end (a specialist's `+` and `−`, a civics proposal and its revolution, the research slider down and back, the trade tables of every player met, the inspector) and lets the AI play one turn (`autoplay`; the slot is the human's again after it). `tools/web-smoke.mjs` also drives the city screen (a worked plot clicked off and on, the automation), F6 (a click researches), F3 (a pick, the proposal, the revolution), F7, the sliders (the top bar's button, F2), F4 (the trade screen when a player was met), F1, `?`, G and Esc, the three layers, the inspector, debug reveal, an autoplay turn, the spectator mode (two turns played, pause, no turn after it, one step, stop: the reveal is off again) and the trade screen of `stage trade` (7 gold offered, accepted, the gold gone); 24 screenshots, about 40 s; both drivers (Playwright and the DevTools protocol) pass.
  * **Deviations.** (1) The spectator mode is the human slot on AI autoplay one turn at a time, not an observer slot (the rules have none): between steps the human's turn starts and is handed to the AI again, and the AI answers the human civilization's events and votes. (2) The interface modes' bindings are written into ffh-play (the XML is not vendored), and the key dispatch rule is inferred (above). (3) The AI-time check compares different games (above); the same-trajectory measurement is the one that isolates the session's cost. (4) At turn 200 of an autoplayed standard game the human slot has few groups, so the latency figure covers the commands its view offers (a dozen or fewer), not a large empire. (5) The deltas' "a third of the map" threshold and the per-transport trackers are choices of this phase. (6) Nothing for `take_over` changed: it is the P1/P4 session operation behind a dialog.
* **P6 review fixes** (no ffh-core, ffh-ai, ffh-cli or `controller.rs` edit, no engine hook):
  * **The group cycle's test** (minor). W, Enter, Tab, Next, the selection after a command and at a new turn drew from `pending.ready_groups` (`readyToMove(bAny = true)`); C++ `cycleSelectionGroups`, `cyclePlotUnits(.., bAuto)` and `findSelectionGroup` test `readyToSelect()`, whose default is `bAny = false` (CvSelectionGroup.h:108, `canAllMove`). The view's groups now carry `selectable` (`decision::is_selectable_group`: `group_ready_to_select(grp, false)` without the AI-moved groups), and `state.js` `selectableGroups`, `nextSelectableGroup`, `selectableGroupsOnPlot` replace `nextReadyGroup` and `readyGroupsOnPlot`. Tests: `tests/screens.rs` `a_partly_moved_stack_waits_but_is_not_selectable` (two warriors grouped, one out of moves: `ready` and in `pending.ready_groups`, not `selectable`), node tests of the partly moved stack skipped by the cycle, the plot cycle and the nearest-group search; the browser smoke test's W step cycles over the selectable groups.
  * **The global keys** (minor): ported as described under "Hotkeys" above (Enter cycles while groups wait and ends the turn only on the end-turn message, Space ends it with no selection, Home / End / F10 the cities), and the keys list, HUMAN.md and this section now say which keys are ffh-play's own. Tests: node tests of `endTurnMessage` (bAny with a selection, the player option) and `cycleCity`; the browser smoke test presses Enter while groups wait (a group selected, no `end_turn` sent) and ends its turns with the End turn button.
  * **Go-to waypoints** (minor). G enters the go-to mode, whose pick sent `move` with `append: false` whatever the modifiers, and the P5 move mode that turned Shift-clicks into waypoints was unreachable (M was removed). C++ sends a go-to pick through `selectionListMove(pPlot, bAlt, bShift, bCtrl)`, which pushes MISSION_MOVE_TO with `bShift` as append (CvGameInterface.cpp:828-893). Now a Shift-click in the go-to mode adds a waypoint held like a Shift-right-click's (the mode stays on until Shift is released, then the waypoints are sent and the mode ends); the dead move mode is gone. Test: the browser smoke test's waypoint step runs a second time with G and Shift-clicks on `stage automoves` (both legs sent on release, appended, the mode ended).
  * **Two-sided trade items** (minor, latent). A peace treaty, open borders, a defensive pact or a permanent alliance goes on both sides of a deal; the trade screen mirrored a click on one side to the other without checking the other side's refusal, so a deal the server refuses could be built, with the mirrored entry stuck (its button disabled). `state.js` `tradeRefusal` greys such an item on both sides when either side refuses it, with the reason ("the other side refuses: ..."). Today's ffh-core gives these four items no refusal (`player_get_trade_denial`: the AI's `AI_openBordersTrade` and the like are not ported), so it could not happen yet; it will once the AI refusals are ported. Test: a node test of `tradeRefusal`.
  * **T13's session budget** (major). The budgeted figure was the ratio of two whole runs (the best of two, 120 turns), which host load moves by a quarter or more (the review: +10.5%, +26.1%, +26.6%, -9.6% on one binary and trajectory), and the docs claimed the thread's CPU time kept other processes out. The session now measures where `advance` spends its time when `Session::profile` is set (`AdvanceProfile`: the `update()` slices, its own work around them, the autosave apart; wall time read around each part of each slice, so the share does not depend on the host's load), and T13 budgets the session's own work against the update time (median over the runs; 0.2% in three runs at load average 6-7). The whole runs are kept as a report: interleaved, `--runs` rounds (5 by default, 2 for `--quick`), best and slowest. The figures and the corrected wording are under "T13" above and in §8. Tests: `tests/screens.rs` `the_advance_profile_splits_update_own_work_and_autosave` (the profile changes nothing in the game; the autosave counted only with a saves directory), the quick bench's profile figures.
  * **A long AI autoplay could not be stopped** (minor). Tools → AI autoplay accepts up to 10,000 turns, and while the game runs commands got `not_your_turn` and session operations `busy`, so only killing the server ended it. While an AI autoplay runs, the HTTP transport now accepts `autoplay {turns: 0}` (`running_reply`; FfH `setAIAutoPlay(0)`) and answers `{"stopping": true, "turns_left"}`; the session applies it before its next turn slice (`SessionShared::request_stop_autoplay`, `Session::advance`), journaled as a `cmd` line stamped with that slice, which the replay applies at the same stamp (`Journal::take_running_cmd`; the one command a journal holds between control points). The human's slot is human again at once, so the human has control at its next turn (or at once if its turn is under way). The status carries `ai_auto_play` (the turns left) and the client's top bar shows them with Stop while an autoplay runs (`tools.js` `autoplayBar`). The agent reads no input while the game runs, so there an autoplay runs its turns. Tests: `tests/http.rs` `t12_a_running_autoplay_stops` (10,000 turns stopped within a few: `end_turn` `not_your_turn`, `save` `busy`, the stop journaled; `turns: 0` at a control point `invalid`), `tests/replay.rs` `an_autoplay_stopped_while_it_runs_replays` (the human has control at the next turn, the journal replays to the same save bytes), the browser smoke test's 5,000-turn autoplay stopped from the top bar.

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
  6. Log recipients: tag each `log.msg` with who C++ sends it to (a player, or a recipient rule such as "met both teams" / "revealed the city" / "voting members" / "everyone" with its ids), so `LogFog` (`view/fog.rs`) stops matching message texts and the event world news (held back today) reach the players who met the event's player. With it (P2 review), a marker entry at `reassign_player_advanced` (old and new slot), so the session splits the log of an `update()` in which FfH moved the human exactly at the move (today it drains such an update for the new slot, F23).
  7. Only if testers find timing differences: engine-first's opt-in C++ popup queue (`takes_popups`, `Player::popups`, save format bump).
  8. (P2) The founder's "take control" popup after the Infernal Pact and the Mercurian Gate (`reassignPlayer`), e.g. `take_control(g, p, new) -> Option<bool>` (default: no). Until then the soak reaches these civilizations through take-over.
* **Notes for other owners** (found in the P0 review and the P2 soak; not edited here):
  * (P2) ffh-core `reassign_player_advanced` does not move the active player (F23; details and repro under P2's "As built"); the session does it after the rules call. **Fixed** (rules fixes): ffh-core moves it; the session asserts it.
  * (P2 review) ffh-core `is_building_class_maxed_out`'s `debug_assert!` panics the checked build when a human takes the Syliven's Perfect Lyre out of a city and drops it again (details, repro and a suggested fix under P2's "As built"); ffh-ai moves the Lyre back and forth on a human slot. **Fixed** (rules fixes): C++ re-counts the wonder the same way, so the assertion is a `// C++ quirk:` comment now; the ffh-ai behaviour stands.
  * ffh-core `structs.rs`: `PlayerVote`'s doc ("a player id"), `PlayerVote::for_player` and `PlayerVote::player()` treat a non-negative value as a player; in C++ it is a `TeamTypes` (team votes only: `CvGame::castVote` compares it with `getTeam()`, and votes.rs's own `team_vote` builds it from a `TeamId`). ffh-play uses `names::voted_team`. **Fixed** (rules fixes): `PlayerVote::for_team` / `team()` replace them (votes.rs, the diplomatic victory check and ffh-play use them; `names::voted_team` is gone).
  * ffh-data `Text::name` cuts a text at the first ':' as FfH2's "gendered forms" separator, but English entries have no forms, so it shortens prose: ffh-core's vote option texts (votes.rs `kVoteOption.szText`, "Election cancelled: ...") show "Global Civic" for `VOTE_LIBERTY` and drop the effect of 8 other votes ("Single Currency" for "Single Currency: +1 Trade Route ..."). ffh-play's `names::display` uses the full text. **Fixed** (rules fixes): `Text` reads Civ4's format (`CvGameText::read`): only a `<Text>` child lists ':'-separated forms and `name` is form 0, so plain text keeps its colons; `names::display` uses `Text::name` again.

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

   It runs 150 turns, WORLDSIZE_SMALL, 3 AIs, seeds 1–3 (from P3, with the protocol-level bot: seeds 1, 3 and 50, see P3 "As built").

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
5. **T5 — AI-as-human soak smoke** (the short CI variant of P2, `soak::run`; `tests/soak.rs` also checks the resume from an autosave, the Infernal and Mercurian take-over and the human-only option sets). Two seeds × 150 turns:
   * `AdvisorAnswerer`;
   * at each control point: idle cities via `advisor.choose_production`, research, pending events and votes answered through the commands with the advisor's choice, then `advisor.play_turn(g, h)` repeated while it moves something (at most 20 times), then `end_turn`;
   * invariants checked.
6. **T6 — decision semantics against C++.** Uses test-setup games built with rules APIs:
   * **Capture.** A human attack captures a city. The keep/raze `Decision` is raised *inside* `execute`, and `city_choose_production` is not called before the answer. With keep, the city is idle and in `pending.idle_cities`.
   * **Research, case 1.** Empty research after turn 4, then end turn: this turn's beakers go to the advisor's pick (`player_do_research` path), the queue is empty afterwards, and the next control point has `research_needed`.
   * **Research, case 2.** With research set, the tech completes and the research-next call is deferred (no tech chosen silently).
   * **Events.** An event is pending (not applied) after the trigger. It applies through `event` with `update_trigger`. Options are checked when the popup is shown (dropped unshown if none is doable), and a shown popup stays open while none is doable. A forced end turn drops it from the list.
   * **Votes.** An unanswered vote gets the extra turn and then abstains. An answered vote calls `cast_vote`.
   * **Free techs.** A free tech blocks, and two free techs give two decisions.
   * **Autoplay.** With autoplay, every method delegates to the advisor.
7. **T7 — fuzz.** A seeded mix of well-formed commands (from the enumerated actions) and malformed or foreign-owned ones, over 100 turns. No panic. Each invalid command gets `invalid`, `not_owner`, `gone` or `cannot`, and **save bytes before == after**. The malformed ones include well-typed values no exe path sends (extreme `i32` data such as a specialist change of `i32::MAX`, slider changes, mission flags and data), run with a per-command time limit: before the P1 review fixes such a command hung the game thread or overflowed rather than being refused; the debug-assertion build must not panic on them either.
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

    As built (P4, `tests/agent.rs`): the decision is a capture's keep/raze, from a save `tests/common` stages with rules calls; the agent is driven line by line (and with a script that ends inside the decision for the EOF policies); also a game started over the protocol, and a deliberate panic's `crashed` event with a replay command that runs. P4 review: the AutoMoves rule after the last ready unit is skipped (the journal replays), and a panic before a loaded game's first autosave (the report names the loaded save).
12. **T12 — HTTP end to end.**
    * Serve on `127.0.0.1:0` in a thread; a `std::net::TcpStream` client fetches `/`, `/api/data` and `/api/setup`, starts a new game, and plays 10 turns with commands and long-polls.
    * Re-entrancy: on two connections, a pending `cmd` (a capture) while `/api/wait` shows `phase=decision`; `answer`; the `cmd` then completes.
    * A POST without `X-FFH` gets 403; path traversal in save names is rejected.

    As built (P4, `tests/http.rs`): the server runs in the test process (`Server::start` on port 0); the capture is loaded from the staged save by name; also the `Host`, length and asset-path checks, a load, debug reveal, `quit`, and a crash that leaves the server up. P4 review: `view_version` of the status and the cached view before, during and after the decision and after debug reveal; the decision text without text keys; the `status` query against `GET /api/status` (at a control point and crashed); the AutoMoves rule after a skip; a server bound to 127.0.0.2 (its own address as `Host`), 64 idle connections and a 503 the client reads.
13. **T13 — performance** (`#[ignore]`, and `ffh-play bench`):
    * AI time per turn in a bot-human game is within 10% of `ffh bench` on the same seed;
    * command latency (execute + settle + view) is under 50 ms on a standard map at turn 200;
    * a view build on a huge map is under 10 ms;
    * QueryGuard overhead is under 1 ms.

    As built (P6, `tests/bench.rs`, `src/bench.rs`): all four, plus the session's overhead on one trajectory: its own work against the `update()` time inside its runs (budget +10%; P6 review), and the bare `Game::update()` loop, the session and the session with autosaves timed whole, interleaved (reported: separate runs vary by a quarter or more on a loaded host); see P6 "As built" for the method and the figures.
14. **Optional JS** (`tools/web-check.sh`): `node --check assets/js/*.js` and `node --test assets/test/*.mjs`. As built (P5): the modules are `assets/app/*.js`, the tests `assets/test/*.test.mjs`; `--smoke` also runs the node smoke client `tools/web-api-smoke.mjs` (20 turns against `ffh-play serve` through the client's `api.js`, a staged decision, save and load), `--browser` item 15. `cargo test` checks the client's files without node (`tests/web.rs`: every file embedded as on disk, every reference and import resolves).
15. **Optional browser smoke** (`tools/web-smoke.mjs`, F21): headless Chromium against `ffh-play serve`, as in P5; screenshots for review. Not part of `cargo test`. As built: Playwright or the DevTools protocol, the staged attack and capture (`ffh-play stage`) and a `--panic-at` server for the crash screen; see P5 "As built".

---

## 9. Risks and mitigations

| Risk | Mitigation |
|---|---|
| **About 280 human-only rules branches have never run**: keep/raze/disband popups, human research paths, the human-team governor, human mission timers, `active_player` branches (quick moves, `getDeclareWarMove` visibility, `setDisableHuman`), and AI-only `isHuman()` spell checks evaluated for a human. | P2 soak over the matrix (also on the checked build) before anyone plays, including the game options that act only on humans (one-city challenge, the FfH challenges). T1/T5 in CI. `catch_unwind` plus autosave plus journal gives deterministic repro. Core fixes ship as separate commits with regression tests. |
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
| **Fog leaks** (global log lines; views built mid-rules). | A single `fog.rs`; the log filter applies each untagged message's C++ recipient condition and holds back the unrecognized ones (§4.4); T8; debug-reveal is explicit and visibly marked. |
| **Security of a local server.** | Binds 127.0.0.1 by default; `X-FFH` header against cross-site POSTs; sanitized save names; `Content-Length` bodies only. |
| **Browser disconnects during a decision.** | The game thread keeps waiting; status is published, so a reload resumes the modal. |

---

## 10. What docs say after this work

**docs/PORTING.md**, "Changes from FFH2", new "Human play" items:
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
