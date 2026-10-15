//! T11: the agent transport (design §8): `ffh-play agent` spawned with piped stdin and stdout.
//! Every output line is JSON; the order is `control`, replies, `decision`, the replies served
//! inside the decision, the answer's, the command's, then `control` again; `cmd` gets
//! `decision_pending` while the decision is open; the end-of-input policy applies; a panic is
//! reported as `crashed` with a replay command that works.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use common::{staged_auto_moves, staged_capture, TempDir};
use ffh_play::json::Json;

const EXE: &str = env!("CARGO_BIN_EXE_ffh-play");

/// A running agent: requests in, lines out (read by a thread, so a hang fails the test instead
/// of blocking it).
struct Agent {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    /// Every line read, parsed.
    transcript: Vec<Json>,
}

impl Agent {
    fn spawn(args: &[&str]) -> Agent {
        let mut child = Command::new(EXE).arg("agent").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().expect("spawn ffh-play agent");
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for l in BufReader::new(stdout).lines() {
                let Ok(l) = l else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let stdin = child.stdin.take();
        Agent { child, stdin, lines, transcript: Vec::new() }
    }

    fn send(&mut self, j: &Json) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{j}").unwrap();
        stdin.flush().unwrap();
    }

    fn send_raw(&mut self, s: &str) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{s}").unwrap();
        stdin.flush().unwrap();
    }

    /// The next output line (every line must be JSON).
    fn next(&mut self) -> Json {
        let l = self.lines.recv_timeout(Duration::from_secs(300)).expect("an output line within 300 s");
        let j = Json::parse(&l).unwrap_or_else(|e| panic!("not JSON ({e}): {l}"));
        self.transcript.push(j.clone());
        j
    }

    /// Sends a request and reads its reply (the next line).
    fn ask(&mut self, j: Json) -> Json {
        self.send(&j);
        self.next()
    }

    /// Closes stdin and waits for the exit code; the lines left are read into the transcript.
    fn finish(mut self) -> (i32, Vec<Json>) {
        drop(self.stdin.take());
        let code = self.child.wait().unwrap().code().unwrap_or(-1);
        while let Ok(l) = self.lines.recv_timeout(Duration::from_secs(10)) {
            self.transcript.push(Json::parse(&l).unwrap_or_else(|e| panic!("not JSON ({e}): {l}")));
        }
        (code, self.transcript)
    }
}

/// A line's place in the order: `ev:<event>`, `ok:<id>` or `<error>:<id>`.
fn tag(j: &Json) -> String {
    if let Some(ev) = j.get("ev") {
        return format!("ev:{}", ev.as_str().unwrap());
    }
    let id = j.get("id").map_or("-".to_string(), |i| i.to_string());
    match j.get("error").and_then(Json::as_str) {
        Some(e) => format!("{e}:{id}"),
        None => format!("ok:{id}"),
    }
}

fn texts(log: &Json) -> Vec<String> {
    log.as_arr().unwrap().iter().map(|e| e.req_str("text").unwrap().to_string()).collect()
}

#[test]
fn t11_agent_transcript_with_a_decision_inside_a_command() {
    let tmp = TempDir::new("t11-decision");
    let root = tmp.0.join("saves");
    let staged = staged_capture(&root.join("staged"));
    let mut a = Agent::spawn(&["--saves", root.to_str().unwrap(), "--load", staged.save.to_str().unwrap()]);
    let control = a.next();
    assert_eq!(control.req_str("ev"), Ok("control"));
    assert_eq!(control.req::<i32>("turn"), Ok(staged.turn));
    assert!(control.get("pending").unwrap().get("ready_groups").is_some());

    let st = a.ask(ffh_play::jobj! { "id" => 1, "k" => "q", "q" => "status" });
    assert_eq!(st.get("r").unwrap().req_str("phase"), Ok("human"));

    // The capture: the decision is raised inside the command.
    a.send(&ffh_play::jobj! { "id" => 2, "k" => "cmd", "c" => "move", "units" => vec![staged.unit], "x" => staged.x, "y" => staged.y });
    let ev = a.next();
    assert_eq!(ev.req_str("ev"), Ok("decision"), "{ev}");
    let d = ev.get("decision").unwrap();
    assert_eq!(d.req_str("kind"), Ok("keep_city"));
    let id: u32 = d.req("id").unwrap();
    let options: Vec<&str> = d.req_arr("options").unwrap().iter().map(|o| o.req_str("key").unwrap()).collect();
    assert_eq!(options, ["keep", "raze"]);

    // Inside the decision: queries are answered from the deciding player's side, the message
    // log is not available, commands and session operations wait for the answer.
    let view = a.ask(ffh_play::jobj! { "id" => 3, "k" => "q", "q" => "view" });
    assert_eq!(view.req::<bool>("ok"), Ok(true), "{view}");
    assert!(view.get("r").unwrap().get("me").is_some());
    let st = a.ask(ffh_play::jobj! { "id" => 4, "k" => "q", "q" => "status" });
    assert_eq!(st.get("r").unwrap().req_str("phase"), Ok("decision"));
    assert_eq!(st.get("r").unwrap().get("decision").unwrap().req::<u32>("id"), Ok(id));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 5, "k" => "q", "q" => "log" }).req_str("error"), Ok("busy"));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 6, "k" => "cmd", "c" => "end_turn", "force" => true }).req_str("error"), Ok("decision_pending"));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 7, "k" => "session", "op" => "save", "name" => "x" }).req_str("error"), Ok("decision_pending"));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 8, "k" => "answer", "decision" => id + 100, "option" => 0 }).req_str("error"), Ok("gone"));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 9, "k" => "answer", "decision" => id, "option" => 7 }).req_str("error"), Ok("invalid"));
    a.send_raw("{\"id\":10,\"k\":\"q\",\"q\":");
    assert_eq!(a.next().req_str("error"), Ok("invalid"));
    let answered = a.ask(ffh_play::jobj! { "id" => 11, "k" => "answer", "decision" => id, "option" => 0 });
    assert_eq!(answered.get("r").unwrap().req_str("key"), Ok("keep"), "{answered}");

    // Then the command's own reply, with the capture message.
    let moved = a.next();
    assert_eq!(moved.req::<i64>("id"), Ok(2), "{moved}");
    assert_eq!(moved.get("r").unwrap().req::<bool>("applied"), Ok(true));
    assert!(texts(moved.get("log").unwrap()).iter().any(|t| t.contains("captured")), "{moved}");
    assert_eq!(moved.get("status").unwrap().req_str("phase"), Ok("human"));
    let view = a.ask(ffh_play::jobj! { "id" => 12, "k" => "q", "q" => "view" });
    let ours = view.get("r").unwrap().req_arr("cities").unwrap().iter().filter(|c| c.req::<i32>("owner") == Ok(0)).count();
    assert_eq!(ours, 2, "the capital and the captured city");
    // The answer was journaled after the command.
    // The stage writes no journal of its own (P5 review): the session's is the first.
    let journal = root.join("staged/journal.jsonl");
    let lines = std::fs::read_to_string(&journal).unwrap();
    assert!(lines.lines().any(|l| l.contains("\"k\":\"answer\"") && l.contains("keep_city")), "{lines}");

    // The end of the turn: the reply, then the next control point.
    let ended = a.ask(ffh_play::jobj! { "id" => 13, "k" => "cmd", "c" => "end_turn", "force" => true });
    assert_eq!(ended.get("status").unwrap().req_str("phase"), Ok("running"));
    let control = a.next();
    assert_eq!((control.req_str("ev"), control.req::<i32>("turn")), (Ok("control"), Ok(staged.turn + 1)));

    let (code, transcript) = a.finish();
    assert_eq!(code, 0);
    let order: Vec<String> = transcript.iter().map(tag).collect();
    assert_eq!(
        order,
        [
            "ev:control", "ok:1", "ev:decision", "ok:3", "ok:4", "busy:5", "decision_pending:6", "decision_pending:7", "gone:8", "invalid:9", "invalid:null", "ok:11", "ok:2",
            "ok:12", "ok:13", "ev:control"
        ]
    );
}

/// Where unit `u` stands in a `view` reply.
fn unit_xy(view: &Json, u: ffh_core::ids::UnitId) -> (i32, i32) {
    let units = view.get("r").unwrap().req_arr("units").unwrap();
    let unit = units.iter().find(|j| j.req::<u32>("id") == Ok(u.0)).unwrap_or_else(|| panic!("unit {} in the view", u.0));
    (unit.req("x").unwrap(), unit.req("y").unwrap())
}

/// The AutoMoves rule between commands (C++ `updateTestEndTurn`, every frame): skipping the last
/// ready unit while another group has queued orders hands control to the game, which continues
/// the orders (`auto_moves`, journaled) before the end of the turn; then `control` again.
#[test]
fn t11_auto_moves_run_when_the_last_ready_unit_is_skipped() {
    let tmp = TempDir::new("t11-automoves");
    let root = tmp.0.join("saves");
    let staged = staged_auto_moves(&root.join("staged"));
    let mut a = Agent::spawn(&["--saves", root.to_str().unwrap(), "--load", staged.save.to_str().unwrap()]);
    let control = a.next();
    assert_eq!((control.req_str("ev"), control.req::<i32>("turn")), (Ok("control"), Ok(staged.turn)), "{control}");
    assert_eq!(control.get("pending").unwrap().req_arr("ready_groups").unwrap().len(), 1, "{control}");
    let view = a.ask(ffh_play::jobj! { "id" => 1, "k" => "q", "q" => "view" });
    assert_eq!(unit_xy(&view, staged.mover), staged.from);

    let skip = a.ask(ffh_play::jobj! { "id" => 2, "k" => "cmd", "c" => "mission", "units" => vec![staged.ready], "mission" => "MISSION_SKIP" });
    assert_eq!(skip.get("r").unwrap().req::<bool>("applied"), Ok(true), "{skip}");
    assert_eq!(skip.get("status").unwrap().req_str("phase"), Ok("running"), "nothing is ready, orders are queued: the game runs them: {skip}");
    let control = a.next();
    assert_eq!((control.req_str("ev"), control.req::<i32>("turn")), (Ok("control"), Ok(staged.turn)), "{control}");
    let view = a.ask(ffh_play::jobj! { "id" => 3, "k" => "q", "q" => "view" });
    assert_ne!(unit_xy(&view, staged.mover), staged.from, "the queued order moved its group before the end of the turn");
    let journal = skip.get("status").unwrap().get("game").unwrap().req_str("journal").unwrap().to_string();
    let lines = std::fs::read_to_string(&journal).unwrap();
    let cmds: Vec<&str> = lines.lines().filter(|l| l.contains("\"k\":\"cmd\"")).collect();
    assert!(cmds.len() == 2 && cmds[0].contains("\"c\":\"mission\"") && cmds[1].contains("\"c\":\"auto_moves\""), "{lines}");
    // The mover spent its moves (the order takes two turns or more): the next command leaves
    // nothing to continue, so the human keeps control.
    let again = a.ask(ffh_play::jobj! { "id" => 4, "k" => "cmd", "c" => "set_option", "option" => "PLAYEROPTION_QUICK_MOVES", "value" => true });
    assert_eq!(again.get("r").unwrap().req::<bool>("applied"), Ok(true), "{again}");
    assert_eq!(again.get("status").unwrap().req_str("phase"), Ok("human"), "{again}");

    let (code, transcript) = a.finish();
    assert_eq!(code, 0);
    let order: Vec<String> = transcript.iter().map(tag).collect();
    assert_eq!(order, ["ev:control", "ok:1", "ok:2", "ev:control", "ok:3", "ok:4"]);
    // The journal replays from the save its header names.
    let out = Command::new(EXE).args(["replay", &journal, "--from", staged.save.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// A scripted input that ends inside the decision: the advisor answers it, then the
/// `--on-eof` policy runs (quit, or end the turn and report the next control point).
#[test]
fn t11_end_of_input_policy() {
    let tmp = TempDir::new("t11-eof");
    let root = tmp.0.join("saves");
    let staged = staged_capture(&root.join("staged"));
    let script = format!(
        "{}\n{}\n",
        ffh_play::jobj! { "id" => 1, "k" => "q", "q" => "status" },
        ffh_play::jobj! { "id" => 2, "k" => "cmd", "c" => "move", "units" => vec![staged.unit], "x" => staged.x, "y" => staged.y }
    );
    for (policy, expected) in [
        ("quit", &["ev:control", "ok:1", "ev:decision", "ev:answered", "ok:2"][..]),
        ("end-turn", &["ev:control", "ok:1", "ev:decision", "ev:answered", "ok:2", "ok:null", "ev:control"][..]),
    ] {
        let mut a = Agent::spawn(&["--no-saves", "--load", staged.save.to_str().unwrap(), "--on-eof", policy]);
        a.stdin.as_mut().unwrap().write_all(script.as_bytes()).unwrap();
        let (code, transcript) = a.finish();
        assert_eq!(code, 0, "{policy}");
        let order: Vec<String> = transcript.iter().map(tag).collect();
        assert_eq!(order, expected, "{policy}");
        let answered = &transcript[3];
        assert_eq!(answered.req_str("by"), Ok("advisor (end of input)"));
        assert!(texts(transcript[4].get("log").unwrap()).iter().any(|t| t.contains("captured")));
        if policy == "end-turn" {
            assert_eq!(transcript.last().unwrap().req::<i32>("turn"), Ok(staged.turn + 1));
        }
    }
}

/// Without a game: `session new` starts one (`control` follows its reply); `--progress`
/// reports the AI turns; `quit` ends the agent.
#[test]
fn t11_new_game_over_the_protocol_and_quit() {
    let tmp = TempDir::new("t11-new");
    let root = tmp.0.join("saves");
    let mut a = Agent::spawn(&["--saves", root.to_str().unwrap(), "--no-game"]);
    assert_eq!(a.next().req_str("ev"), Ok("no_game"));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 1, "k" => "cmd", "c" => "end_turn" }).req_str("error"), Ok("no_game"));
    let setup = ffh_play::jobj! { "seed" => 4, "world" => "WORLDSIZE_DUEL", "opponents" => 1 };
    let r = a.ask(ffh_play::jobj! { "id" => 2, "k" => "session", "op" => "new", "setup" => setup.clone(), "name" => "duel" });
    assert_eq!(r.get("r").unwrap().req_str("game"), Ok(root.join("duel").to_str().unwrap()), "{r}");
    assert_eq!(a.next().req_str("ev"), Ok("control"));
    // A second game with the same name gets its own directory.
    let r = a.ask(ffh_play::jobj! { "id" => 3, "k" => "session", "op" => "new", "setup" => setup, "name" => "duel" });
    assert_eq!(r.get("r").unwrap().req_str("game"), Ok(root.join("duel-2").to_str().unwrap()));
    assert_eq!(a.next().req_str("ev"), Ok("control"));
    let text = a.ask(ffh_play::jobj! { "id" => 4, "k" => "q", "q" => "text" });
    assert!(text.get("r").unwrap().req_str("text").unwrap().contains("Turn 0"), "{text}");
    let saves = a.ask(ffh_play::jobj! { "id" => 5, "k" => "session", "op" => "saves" });
    let games: Vec<&str> = saves.get("r").unwrap().req_arr("games").unwrap().iter().map(|g| g.req_str("game").unwrap()).collect();
    assert_eq!(games, ["duel", "duel-2"]);
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 6, "k" => "session", "op" => "load", "name" => "../duel/auto/start" }).req_str("error"), Ok("invalid"));
    let loaded = a.ask(ffh_play::jobj! { "id" => 7, "k" => "session", "op" => "load", "name" => "duel/auto/start" });
    assert_eq!(loaded.req::<bool>("ok"), Ok(true), "{loaded}");
    assert_eq!(a.next().req_str("ev"), Ok("control"));
    assert_eq!(a.ask(ffh_play::jobj! { "id" => 8, "k" => "session", "op" => "quit" }).get("r").unwrap().req::<bool>("quit"), Ok(true));
    let (code, transcript) = a.finish();
    assert_eq!(code, 0);
    assert_eq!(tag(transcript.last().unwrap()), "ok:8", "nothing after quit");
}

/// A panic on the game thread: a `crashed` event with the message, the last autosave and the
/// journal, exit code 3; its replay command reproduces the game up to the autosave.
#[test]
fn t11_a_panic_is_reported_with_its_repro() {
    let tmp = TempDir::new("t11-crash");
    let root = tmp.0.join("saves");
    let mut a = Agent::spawn(&["--saves", root.to_str().unwrap(), "--seed", "6", "--world", "WORLDSIZE_DUEL", "--opponents", "1", "--panic-at", "1"]);
    assert_eq!(a.next().req_str("ev"), Ok("control"));
    let ended = a.ask(ffh_play::jobj! { "id" => 1, "k" => "cmd", "c" => "end_turn", "force" => true });
    assert_eq!(ended.req::<bool>("ok"), Ok(true));
    let crashed = a.next();
    assert_eq!(crashed.req_str("ev"), Ok("crashed"), "{crashed}");
    assert!(crashed.req_str("message").unwrap().contains("--panic-at 1"));
    assert!(crashed.req_str("location").unwrap().contains("transport"));
    let autosave = crashed.req_str("autosave").unwrap().to_string();
    let journal = crashed.req_str("journal").unwrap().to_string();
    assert!(autosave.contains("/auto/T0") && std::path::Path::new(&autosave).is_file(), "{autosave}");
    assert_eq!(crashed.req_str("repro"), Ok(format!("ffh-play replay {journal} --from {autosave}").as_str()));
    let (code, _) = a.finish();
    assert_eq!(code, 3);
    let out = Command::new(EXE).args(["replay", &journal, "--from", &autosave]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// A panic before a loaded game's first autosave: the report names the loaded save as the
/// autosave (the journal's `load` header names it) and its replay command runs (P4 review).
/// `--panic-at` the save's turn fires as the loaded game starts.
#[test]
fn t11_a_panic_after_a_load_is_reported_with_the_loaded_save() {
    let tmp = TempDir::new("t11-crash-load");
    let root = tmp.0.join("saves");
    let staged = staged_capture(&root.join("staged"));
    let turn = staged.turn.to_string();
    let mut a = Agent::spawn(&["--saves", root.to_str().unwrap(), "--load", staged.save.to_str().unwrap(), "--panic-at", &turn]);
    let crashed = a.next();
    assert_eq!(crashed.req_str("ev"), Ok("crashed"), "{crashed}");
    let save = staged.save.to_str().unwrap();
    assert_eq!(crashed.req_str("autosave"), Ok(save), "no autosave yet: the loaded save");
    let journal = crashed.req_str("journal").unwrap().to_string();
    assert_eq!(crashed.req_str("repro"), Ok(format!("ffh-play replay {journal} --from {save}").as_str()));
    let (code, _) = a.finish();
    assert_eq!(code, 3);
    let out = Command::new(EXE).args(["replay", &journal, "--from", save]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
