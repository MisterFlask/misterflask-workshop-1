#!/usr/bin/env node
// The headless-Chromium smoke test of the browser client (docs/PLAY_DESIGN.md §7 P5, F21):
// starts `ffh-play serve`, loads the page in headless Chromium and plays through the UI with the
// mouse and keyboard: the new-game screen, founding a city, the research and production
// popups, selecting a unit, the path preview, moving with a right-click, ending turns through
// the end-turn gate, the turn report, saving and loading; then a staged capture (`ffh-play
// stage capture`) for the attack odds and the keep/raze decision modal, and a server started
// with `--panic-at` for the crash screen. It fails on any console error, page error, failed
// request or HTTP error, and saves screenshots for review.
//
//   node tools/web-smoke.mjs [--bin PATH] [--out DIR] [--turns 4] [--seed 5] [--cdp] [--headed]
//        [--assets DIR]   (serve the client files from DIR: crates/ffh-play/assets, live edits)
//
// The browser: the `playwright` package (resolved normally, then in node's global modules),
// else Chromium driven directly over the DevTools protocol (`--cdp` forces that): the
// executable from $CHROME, or the Playwright browsers in $PLAYWRIGHT_BROWSERS_PATH (or
// /opt/pw-browsers, ~/.cache/ms-playwright). Without a browser the test is skipped (exit 0).
// Screenshots go to --out (default target/web-smoke, not committed). Exit 1 on a failure.

import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { ROOT, parseArgs, findBin, tempDir, startServer, stage, until } from "./web-lib.mjs";

let args;
try {
  args = parseArgs(process.argv.slice(2), { bin: "v", out: "v", turns: "v", seed: "v", cdp: "flag", headed: "flag", assets: "v" });
} catch (e) {
  console.error(`web-smoke: ${e.message}`);
  process.exit(2);
}
const OUT = path.resolve(args.out ?? path.join(ROOT, "target", "web-smoke"));
const TURNS = Number(args.turns ?? 4);
const SEED = Number(args.seed ?? 5);
const VIEW = { width: 1280, height: 800 };
const ASSETS = args.assets ? ["--assets", path.resolve(args.assets)] : [];

// ------------------------------------------------------------------------------------------
// Browsers: the same small interface over Playwright or the DevTools protocol.

async function loadPlaywright() {
  if (args.cdp) return null;
  try {
    return await import("playwright");
  } catch {
    /* not installed locally */
  }
  const require = createRequire(import.meta.url);
  const roots = [path.join(path.dirname(process.execPath), "..", "lib", "node_modules"), ...(process.env.NODE_PATH ?? "").split(path.delimiter).filter(Boolean)];
  for (const r of roots) {
    try {
      return require(require.resolve("playwright", { paths: [r] }));
    } catch {
      /* next */
    }
  }
  return null;
}

/** Playwright's Chromium as the smoke test's browser. */
async function playwrightBrowser(pw, problems) {
  const browser = await pw.chromium.launch({ headless: !args.headed });
  const page = await browser.newPage({ viewport: VIEW });
  page.on("console", (m) => m.type() === "error" && problems.push(`console: ${m.text()}`));
  page.on("pageerror", (e) => problems.push(`page error: ${e.message}`));
  // A navigation aborts the page's long-poll (net::ERR_ABORTED): not a failure.
  page.on("requestfailed", (r) => r.failure()?.errorText !== "net::ERR_ABORTED" && problems.push(`request failed: ${r.url()} ${r.failure()?.errorText ?? ""}`));
  page.on("response", (r) => r.status() >= 400 && problems.push(`HTTP ${r.status()}: ${r.url()}`));
  return {
    name: `Playwright Chromium ${browser.version()}`,
    goto: (url) => page.goto(url),
    eval: (expr) => page.evaluate(expr),
    mouse: async (x, y, button = "left") => page.mouse.click(x, y, { button }),
    move: (x, y) => page.mouse.move(x, y),
    key: (key, shift = false) => page.keyboard.press(shift ? `Shift+${key}` : key),
    type: (text) => page.keyboard.type(text),
    shot: (file) => page.screenshot({ path: file }),
    close: () => browser.close(),
  };
}

/** The Chromium executable for the DevTools-protocol driver, or null. */
function findChrome() {
  if (process.env.CHROME && fs.existsSync(process.env.CHROME)) return process.env.CHROME;
  const dirs = [process.env.PLAYWRIGHT_BROWSERS_PATH, "/opt/pw-browsers", path.join(os.homedir(), ".cache", "ms-playwright")].filter(Boolean);
  for (const d of dirs) {
    if (!fs.existsSync(d)) continue;
    for (const sub of fs.readdirSync(d).filter((s) => /^chromium-\d+$/.test(s)).sort().reverse()) {
      const exe = path.join(d, sub, "chrome-linux", "chrome");
      if (fs.existsSync(exe)) return exe;
    }
  }
  for (const exe of ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"]) if (fs.existsSync(exe)) return exe;
  return null;
}

/** Chromium driven over the DevTools protocol (node's WebSocket). */
async function cdpBrowser(exe, problems) {
  const profile = tempDir("chrome");
  const proc = spawn(exe, [
    ...(args.headed ? [] : ["--headless=new"]),
    "--remote-debugging-port=0",
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-gpu",
    "--no-sandbox",
    `--window-size=${VIEW.width},${VIEW.height}`,
    "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  let err = "";
  const wsUrl = await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error(`Chromium did not start: ${err}`)), 20000);
    proc.stderr.on("data", (d) => {
      err += d;
      const m = /DevTools listening on (ws:\/\/\S+)/.exec(err);
      if (m) {
        clearTimeout(t);
        resolve(m[1]);
      }
    });
    proc.on("exit", (c) => reject(new Error(`Chromium exited (${c}): ${err}`)));
  });
  const port = new URL(wsUrl).port;
  const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const target = targets.find((t) => t.type === "page");
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = () => rej(new Error("cannot connect to the page"));
  });
  let id = 0;
  const waiting = new Map();
  const listeners = [];
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && waiting.has(m.id)) {
      const { res, rej } = waiting.get(m.id);
      waiting.delete(m.id);
      m.error ? rej(new Error(`${m.error.message} ${m.error.data ?? ""}`)) : res(m.result);
    } else if (m.method) listeners.forEach((f) => f(m));
  };
  const send = (method, params = {}) =>
    new Promise((res, rej) => {
      const n = ++id;
      waiting.set(n, { res, rej });
      ws.send(JSON.stringify({ id: n, method, params }));
    });
  listeners.push((m) => {
    const p = m.params;
    if (m.method === "Runtime.consoleAPICalled" && p.type === "error") problems.push(`console: ${p.args.map((a) => a.value ?? a.description).join(" ")}`);
    if (m.method === "Runtime.exceptionThrown") problems.push(`page error: ${p.exceptionDetails.exception?.description ?? p.exceptionDetails.text}`);
    if (m.method === "Log.entryAdded" && p.entry.level === "error") problems.push(`console: ${p.entry.text} ${p.entry.url ?? ""}`);
    if (m.method === "Network.loadingFailed" && !p.canceled) problems.push(`request failed: ${p.errorText}`);
    if (m.method === "Network.responseReceived" && p.response.status >= 400) problems.push(`HTTP ${p.response.status}: ${p.response.url}`);
  });
  for (const d of ["Page", "Runtime", "Network", "Log"]) await send(`${d}.enable`);
  await send("Emulation.setDeviceMetricsOverride", { width: VIEW.width, height: VIEW.height, deviceScaleFactor: 1, mobile: false });
  const evaluate = async (expr) => {
    const src = typeof expr === "function" ? `(${expr})()` : expr;
    const r = await send("Runtime.evaluate", { expression: src, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(`evaluate: ${r.exceptionDetails.exception?.description ?? r.exceptionDetails.text}`);
    return r.result.value;
  };
  const mouseEvent = (type, x, y, button = "none", clickCount = 0) => send("Input.dispatchMouseEvent", { type, x, y, button, clickCount, buttons: button === "left" ? 1 : button === "right" ? 2 : 0 });
  const KEYS = { Enter: [13, "\r"], Escape: [27, ""], Tab: [9, ""], Delete: [46, ""] };
  return {
    name: `Chromium over the DevTools protocol (${exe})`,
    goto: async (url) => {
      const loaded = new Promise((res) => listeners.push((m) => m.method === "Page.loadEventFired" && res()));
      await send("Page.navigate", { url });
      await loaded;
    },
    eval: evaluate,
    mouse: async (x, y, button = "left") => {
      await mouseEvent("mouseMoved", x, y);
      await mouseEvent("mousePressed", x, y, button, 1);
      await mouseEvent("mouseReleased", x, y, button, 1);
    },
    move: (x, y) => mouseEvent("mouseMoved", x, y),
    key: async (key, shift = false) => {
      const [code, text] = KEYS[key] ?? [key.toUpperCase().charCodeAt(0), key];
      const base = { key, windowsVirtualKeyCode: code, modifiers: shift ? 8 : 0 };
      await send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", text: text || undefined, ...base });
      await send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
    },
    type: (text) => send("Input.insertText", { text }),
    shot: async (file) => fs.writeFileSync(file, Buffer.from((await send("Page.captureScreenshot", { format: "png" })).data, "base64")),
    close: async () => {
      ws.close();
      proc.kill("SIGKILL");
      await new Promise((r) => (proc.exitCode !== null ? r() : proc.once("exit", r)));
      fs.rmSync(profile, { recursive: true, force: true });
    },
  };
}

// ------------------------------------------------------------------------------------------
// The test.

const problems = [];
const steps = [];
let b = null;
let shotN = 0;

async function shot(name) {
  const file = path.join(OUT, `${String(++shotN).padStart(2, "0")}-${name}.png`);
  await b.shot(file);
  steps.push(`screenshot ${path.relative(process.cwd(), file)}`);
}

const S = (expr) => b.eval(`(() => { const S = window.ffh && window.ffh.S; return ${expr}; })()`);
const phase = () => S("S && S.status && S.status.phase");
const modalTitles = () => b.eval(`[...document.querySelectorAll("#modals .backdrop:not(.under) .modal h2")].map((e) => e.textContent)`);
const until2 = (expr, what, ms = 30000) => until(() => b.eval(expr), what, ms);

/** Clicks the element `sel` (its centre) with the mouse. */
async function click(sel, button = "left") {
  const r = await b.eval(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; e.scrollIntoView({block: "nearest"}); const r = e.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, disabled: !!e.disabled }; })()`);
  if (!r) throw new Error(`no element ${sel}`);
  if (r.disabled) throw new Error(`${sel} is disabled`);
  await b.mouse(r.x, r.y, button);
}

/** The page position of plot (x, y)'s centre. */
async function plotPos(x, y) {
  return b.eval(`(() => { const m = window.ffh.mapView; m.reveal(${x}, ${y}); const p = m.screenOf(${x}, ${y}); const r = document.querySelector("canvas.map").getBoundingClientRect(); return { x: p.x + r.left, y: p.y + r.top }; })()`);
}

async function waitHuman(what = "the player's turn", ms = 60000) {
  await until2(`(() => { const S = window.ffh.S; return S.status && S.status.phase === "human" && S.view && S.view.turn === S.status.turn; })()`, what, ms);
  await new Promise((r) => setTimeout(r, 250));
}

/** Answers what the turn start shows: the deferred popups (first enabled choice) and the
 *  blocking decision (keep / first option). */
async function answerPopups(log = true) {
  for (let k = 0; k < 12; k++) {
    await new Promise((r) => setTimeout(r, 300));
    const titles = await modalTitles();
    if (!titles.length) return;
    const has = (sel) => b.eval(`!!document.querySelector(${JSON.stringify(`#modals .backdrop:not(.under) ${sel}`)})`);
    if (await has(".modal.decision")) await click("#modals .modal.decision button.option:not([disabled])");
    else if (await has("button.build[data-tech]")) await click("#modals .backdrop:not(.under) button.build[data-tech]");
    else if (await has("button.build[data-order]:not([disabled])")) await click("#modals .backdrop:not(.under) button.build[data-order]:not([disabled])");
    else if (await has("button.option:not([disabled])")) await click("#modals .backdrop:not(.under) button.option:not([disabled])");
    else await b.key("Escape");
    if (log) steps.push(`answered "${titles.at(-1)}"`);
  }
}

async function newGame(url) {
  await b.goto(url);
  await until2("window.ffh && window.ffh.ready", "the client to boot");
  await until(async () => (await modalTitles()).includes("New game"), "the new-game screen");
  await shot("new-game");
  await b.eval(`(() => {
    const set = (id, v) => { const e = document.getElementById(id); e.value = v; e.dispatchEvent(new Event("change")); };
    set("ng-world", "WORLDSIZE_TINY"); set("ng-seed", "${SEED}"); set("ng-opponents", "2");
  })()`);
  await click("#modals .modal-buttons button.primary");
  await waitHuman("the first turn");
  steps.push(`new game: turn ${await S("S.status.turn")}`);
  await shot("first-turn");
}

async function foundCity() {
  const settler = await S(`(() => { const u = S.view.units.find((u) => u.owner === S.view.me.player && S.data.tables.UnitType[u.type].found); return u || null; })()`);
  if (!settler) throw new Error("no settler");
  const p = await plotPos(settler.x, settler.y);
  await b.mouse(p.x, p.y);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.includes(${settler.id})`, "the settler selected");
  await until2(`!!document.querySelector('#unit-panel button.action[data-type="MISSION_FOUND"]:not([disabled])')`, "the Found action");
  await b.key("b");
  await until2(`window.ffh.S.view.cities.some((c) => c.owner === window.ffh.S.view.me.player)`, "a city");
  steps.push("founded a city with B");
  // The research popup, then the city's production chooser (C++ order).
  await until(async () => (await modalTitles()).length > 0, "the popups after founding");
  await shot("research-popup");
  await answerPopups();
  await shot("after-popups");
  const research = await S("S.view.me.research");
  if (!research) throw new Error("no research chosen through the popup");
  const city = await S("S.view.cities.find((c) => c.owner === S.view.me.player)");
  if (!city.production) throw new Error("no production chosen through the popup");
}

async function moveUnit() {
  const unit = await S(`(() => { const S = window.ffh.S; const me = S.view.me.player; return S.view.units.find((u) => u.owner === me && u.moves > 0 && S.data.tables.UnitType[u.type].combat > 0) || null; })()`);
  if (!unit) return steps.push("no unit to move");
  const p = await plotPos(unit.x, unit.y);
  await b.mouse(p.x, p.y);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.includes(${unit.id})`, "the unit selected");
  // A land plot two steps away with a path.
  const target = await b.eval(`(async () => {
    const S = window.ffh.S;
    for (const [dx, dy] of [[2,0],[-2,0],[0,2],[0,-2],[2,1],[-2,-1],[1,2],[-1,-2],[1,0],[-1,0],[0,1],[0,-1]]) {
      const x = (${unit.x} + dx + S.map.w) % S.map.w, y = ${unit.y} + dy;
      if (y < 0 || y >= S.map.h) continue;
      const r = await window.ffh.api.query("path", { units: [${unit.id}], x, y });
      if (r.ok && r.declare_war_team === null && !r.attack) return { x, y };
    }
    return null;
  })()`);
  if (!target) return steps.push("no reachable plot near the unit");
  const t = await plotPos(target.x, target.y);
  await b.move(t.x - 3, t.y - 3);
  await b.move(t.x, t.y);
  await until2(`(() => { const p = window.ffh.mapView.overlay.path; return p && p.ok && p.nodes.length > 1; })()`, "the path preview");
  await until2(`!document.querySelector("#mapwrap .tooltip").hidden`, "the tooltip");
  await shot("path-preview");
  await b.mouse(t.x, t.y, "right");
  await until2(`(() => { const u = window.ffh.S.units.get(${unit.id}); return !u || u.x !== ${unit.x} || u.y !== ${unit.y} || window.ffh.S.view.groups.some((g) => g.units.includes(${unit.id}) && g.missions.length); })()`, "the unit to move");
  steps.push(`moved unit ${unit.id} toward (${target.x}, ${target.y}) with a right-click`);
}

async function endTurns(n) {
  for (let k = 0; k < n; k++) {
    const turn = await S("S.status.turn");
    await b.key("Enter");
    // The end-turn gate when units wait for orders: end anyway.
    await new Promise((r) => setTimeout(r, 400));
    const titles = await modalTitles();
    if (titles.includes("End the turn?")) {
      if (k === 0) await shot("end-turn-gate");
      await click('#modals .modal-buttons button:not(.primary):last-child');
    }
    await until2(`(() => { const S = window.ffh.S; return S.status && (S.status.turn > ${turn} || S.status.phase === "decision"); })()`, `turn ${turn + 1}`, 60000);
    await waitHuman(`turn ${turn + 1}`);
    await answerPopups();
    steps.push(`ended turn ${turn}`);
  }
  await b.eval(`document.querySelector('#bottom-tabs button[data-tab="report"]').click()`);
  await shot("turn-report");
  const report = await S("S.report");
  if (!report) throw new Error("no turn report");
}

async function saveAndLoad() {
  await click("#btn-save");
  await until(async () => (await modalTitles()).includes("Save the game"), "the save dialog");
  await b.eval(`document.getElementById("save-name").select()`);
  await b.type("smoke");
  await b.key("Enter");
  await until2(`[...document.querySelectorAll("#toasts .toast")].some((t) => t.textContent.startsWith("Saved"))`, "the saved toast");
  steps.push("saved as smoke");
  const turn = await S("S.status.turn");
  await click("#btn-load");
  await until2(`!!document.querySelector('#modals button.save[data-save$="/smoke"]')`, "the save in the load screen");
  await shot("load-screen");
  await click('#modals button.save[data-save$="/smoke"]');
  await waitHuman("the loaded game");
  if ((await S("S.status.turn")) !== turn) throw new Error("the loaded game is at another turn");
  steps.push("loaded smoke");
}

/** Loads a staged save (`ffh-play stage KIND`) through the load screen. */
async function loadStaged(bin, saves, kind) {
  const st = await stage(bin, kind, path.join(saves, `staged-${kind}`));
  await click("#btn-load");
  const sel = `#modals button.save[data-save="staged-${kind}/${kind}"]`;
  await until2(`!!document.querySelector(${JSON.stringify(sel)})`, `the staged ${kind} save in the load screen`);
  await click(sel);
  await until2(`(() => { const S = window.ffh.S; return S.status && S.status.phase === "human" && S.status.turn === ${st.turn} && S.units.has(${st.unit}) && S.view.turn === ${st.turn}; })()`, `the staged ${kind} game`);
  await new Promise((r) => setTimeout(r, 300));
  await answerPopups(false);
  return st;
}

/** A staged attack: the odds and modifiers in the tooltip over the enemy, then the attack. */
async function stagedAttack(bin, saves) {
  const st = await loadStaged(bin, saves, "attack");
  const u = await S(`S.units.get(${st.unit})`);
  const p = await plotPos(u.x, u.y);
  await b.mouse(p.x, p.y);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.length === 1 && window.ffh.S.sel.units[0] === ${st.unit}`, "the warrior selected");
  const e = await plotPos(st.x, st.y);
  await b.move(e.x - 3, e.y - 3);
  await b.move(e.x, e.y);
  await until2(`!!document.querySelector("#mapwrap .tooltip .tip-combat .odds")`, "the attack odds in the tooltip");
  const odds = await b.eval(`document.querySelector("#mapwrap .tooltip .tip-combat").textContent`);
  if (!/Odds \d+(\.\d)?%/.test(odds)) throw new Error(`the odds tooltip: ${odds}`);
  await shot("attack-odds");
  await b.mouse(e.x, e.y, "right");
  await until2(`(() => { const S = window.ffh.S; const me = S.units.get(${st.unit}); return !S.units.has(${st.enemy}) || !me || me.hp < me.max_hp; })()`, "the combat");
  const won = await S(`!S.units.has(${st.enemy})`);
  steps.push(`attacked from the odds tooltip (${odds.replace(/\s+/g, " ").slice(0, 40)}…): ${won ? "won" : "lost or damaged"}`);
}

/** A staged capture: the move into the enemy city and its keep/raze decision modal. */
async function stagedCapture(bin, saves) {
  const st = await loadStaged(bin, saves, "capture");
  const u = await S(`S.units.get(${st.unit})`);
  const p = await plotPos(u.x, u.y);
  await b.mouse(p.x, p.y);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.includes(${st.unit})`, "the warrior selected");
  const c = await plotPos(st.x, st.y);
  await b.move(c.x - 3, c.y - 3);
  await b.move(c.x, c.y);
  await until2(`/size \\d/.test(document.querySelector("#mapwrap .tooltip").textContent || "")`, "the tooltip over the city");
  await b.mouse(c.x, c.y, "right");
  await until(async () => (await modalTitles()).some((t) => /keep|Disband/i.test(t)), "the keep/raze decision modal", 30000);
  await shot("decision");
  await click('#modals .modal.decision button.option[data-option="0"]');
  await until2(`(() => { const S = window.ffh.S; const c = S.view && S.view.cities.find((c) => c.x === ${st.x} && c.y === ${st.y}); return c && c.owner === S.view.me.player; })()`, "the city to be ours");
  steps.push("captured a city through the keep/raze decision modal");
  await answerPopups();
}

async function crashScreen(bin) {
  const saves = tempDir("smoke-crash");
  const server = await startServer({ bin, saves, extra: [...ASSETS, "--new", "--world", "WORLDSIZE_DUEL", "--opponents", "1", "--seed", "2", "--panic-at", "1"] });
  try {
    await b.goto(server.url);
    await until2("window.ffh && window.ffh.ready", "the client to boot");
    await waitHuman("the crash test game");
    await answerPopups(false);
    await b.key("Enter");
    await new Promise((r) => setTimeout(r, 400));
    if ((await modalTitles()).includes("End the turn?")) await click('#modals .modal-buttons button:not(.primary):last-child');
    await until(async () => (await modalTitles()).includes("The game crashed"), "the crash screen", 60000);
    const text = await b.eval(`document.querySelector("#modals pre.report").textContent`);
    if (!/deliberate panic/.test(text) || !/ffh-play replay/.test(text)) throw new Error(`the crash report: ${text}`);
    await shot("crash");
    steps.push("crash screen with the replay command");
  } finally {
    // Leave the page before the server goes, or its long-poll fails.
    await b.goto("about:blank");
    await server.stop();
    fs.rmSync(saves, { recursive: true, force: true });
  }
}

async function main() {
  const pw = await loadPlaywright();
  const chrome = pw ? null : findChrome();
  if (!pw && !chrome) {
    console.log("web-smoke: no browser (neither the playwright package nor a Chromium executable); skipped");
    return 0;
  }
  const bin = findBin(args.bin);
  fs.mkdirSync(OUT, { recursive: true });
  for (const f of fs.readdirSync(OUT)) if (f.endsWith(".png")) fs.rmSync(path.join(OUT, f));
  const saves = tempDir("smoke");
  const server = await startServer({ bin, saves, extra: ASSETS });
  try {
    b = pw ? await playwrightBrowser(pw, problems) : await cdpBrowser(chrome, problems);
    console.log(`web-smoke: ${b.name}; server ${server.url}; screenshots in ${OUT}`);
    const t0 = Date.now();
    await newGame(server.url);
    await foundCity();
    await moveUnit();
    await endTurns(TURNS);
    await saveAndLoad();
    await stagedAttack(bin, saves);
    await stagedCapture(bin, saves);
    await crashScreen(bin);
    console.log(`web-smoke: ${steps.length} steps in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  } catch (e) {
    problems.push(`step failed: ${e.message}`);
    try {
      await b?.shot(path.join(OUT, "failure.png"));
      problems.push(`see ${path.join(OUT, "failure.png")}`);
    } catch {
      /* the browser is gone */
    }
  } finally {
    await b?.goto("about:blank").catch(() => {});
    await b?.close();
    await server.stop();
    fs.rmSync(saves, { recursive: true, force: true });
  }
  for (const s of steps) console.log(`  ${s}`);
  if (problems.length) {
    for (const p of problems) console.error(`FAIL: ${p}`);
    return 1;
  }
  console.log("web-smoke: ok");
  return 0;
}

process.exit(await main());
