#!/usr/bin/env node
// The headless-Chromium smoke test of the browser client (docs/PLAY_DESIGN.md §7 P5, F21):
// starts `ffh-play serve`, loads the page in headless Chromium and plays through the UI with the
// mouse and keyboard: the new-game screen (Start double-clicked), W over the waiting groups,
// hotkeys right after a selection change, founding a city, the research and production popups,
// the city queue, selecting a unit, the path preview, moving with a right-click, ending turns
// through the end-turn gate, the turn report, saving and loading; then staged saves (`ffh-play
// stage`): the attack odds and an attack, the keep/raze decision modal, popups set aside and
// asked again after a load, the transport picker, Shift waypoints; a game started by another
// client; and a server started with `--panic-at` for the crash screen. Phase 6: the city screen
// (a worked plot clicked off and on, the citizens' automation), the tech tree (F6, a click
// researches), civics (F3, a pick, the revolution), religion (F7), the sliders (F2 and the top
// bar), diplomacy (F4, the trade screen when a player was met), the cities list (F1), the keys
// (?), the go-to mode (G, Esc), the map layers, the inspector, debug reveal, AI autoplay and the
// spectator mode (play, pause, step, stop), and the trade screen (`stage trade`: gold offered and
// accepted). The art client: the map drawn with the original pixel art (its worker, the view
// drawn in full, a frame's time, the zoom steps and the chart, the bitmap type), Tools → Options
// and `?renderer=flat` for the flat renderer; `?renderer=3d`, the experimental 3D renderer (picking,
// selection, zoom, pan, layers). Phase 7: the automate buttons (`stage automate`: a
// worker automated with a click, a scout exploring by its hotkey), the secretary general's election
// popup (`stage election`) and the Kuriotates city-or-settlement decision (`stage settlement`). It
// fails on any console error, page error, failed
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
    dblclick: (x, y) => page.mouse.dblclick(x, y),
    move: (x, y) => page.mouse.move(x, y),
    key: (key, shift = false) => page.keyboard.press(shift ? `Shift+${key}` : key),
    // A held key (Shift): the mouse events meanwhile carry it.
    keyDown: (key) => page.keyboard.down(key),
    keyUp: (key) => page.keyboard.up(key),
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
  // Modifiers held with keyDown (8 = Shift), carried by the mouse events meanwhile.
  let held = 0;
  const mouseEvent = (type, x, y, button = "none", clickCount = 0) => send("Input.dispatchMouseEvent", { type, x, y, button, clickCount, modifiers: held, buttons: button === "left" ? 1 : button === "right" ? 2 : 0 });
  const KEYS = { Enter: [13, "\r"], Escape: [27, ""], Tab: [9, ""], Delete: [46, ""], Home: [36, ""], F1: [112, ""], F2: [113, ""], F3: [114, ""], F4: [115, ""], F6: [117, ""], F7: [118, ""] };
  const MODIFIERS = { Shift: [16, 8, "ShiftLeft"] };
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
    dblclick: async (x, y) => {
      await mouseEvent("mouseMoved", x, y);
      for (const n of [1, 2]) {
        await mouseEvent("mousePressed", x, y, "left", n);
        await mouseEvent("mouseReleased", x, y, "left", n);
      }
    },
    move: (x, y) => mouseEvent("mouseMoved", x, y),
    key: async (key, shift = false) => {
      const [code, text] = KEYS[key] ?? [key.toUpperCase().charCodeAt(0), key];
      const base = { key, windowsVirtualKeyCode: code, modifiers: shift ? 8 : 0 };
      await send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", text: text || undefined, ...base });
      await send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
    },
    keyDown: async (key) => {
      const [code, bit, name] = MODIFIERS[key];
      held |= bit;
      await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key, code: name, windowsVirtualKeyCode: code, modifiers: held });
    },
    keyUp: async (key) => {
      const [code, bit, name] = MODIFIERS[key];
      held &= ~bit;
      await send("Input.dispatchKeyEvent", { type: "keyUp", key, code: name, windowsVirtualKeyCode: code, modifiers: held });
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

/** The page position of plot (x, y)'s centre. `reveal` may glide the camera there (map.js
 *  `glideTo`), so wait for it to settle before reading the screen position, as a player would. */
async function plotPos(x, y) {
  await b.eval(`window.ffh.mapView.reveal(${x}, ${y})`);
  await until2(`!window.ffh.mapView.anim`, "the camera to settle");
  return b.eval(`(() => { const m = window.ffh.mapView; const p = m.screenOf(${x}, ${y}); const r = document.querySelector("canvas.map").getBoundingClientRect(); return { x: p.x + r.left, y: p.y + r.top }; })()`);
}

async function waitHuman(what = "the player's turn", ms = 60000) {
  await until2(`(() => { const S = window.ffh.S; return S.status && S.status.phase === "human" && S.view && S.view.turn === S.status.turn; })()`, what, ms);
  await new Promise((r) => setTimeout(r, 250));
}

/** Answers what the turn start shows: the deferred popups (first enabled choice) and the
 *  blocking decision (keep / first option). */
async function answerPopups(log = true) {
  // Done once no dialog shows twice in a row (the next popup comes after the last answer's
  // command and view refresh).
  let calm = 0;
  for (let k = 0; k < 16; k++) {
    await new Promise((r) => setTimeout(r, 300));
    const titles = await modalTitles();
    if (!titles.length) {
      if (++calm >= 2) return;
      continue;
    }
    calm = 0;
    const has = (sel) => b.eval(`!!document.querySelector(${JSON.stringify(`#modals .backdrop:not(.under) ${sel}`)})`);
    if (await has(".modal.decision")) await click("#modals .modal.decision button.option:not([disabled])");
    else if (await has("button.build[data-tech]")) await click("#modals .backdrop:not(.under) button.build[data-tech]");
    else if (await has("button.build[data-order]:not([disabled])")) await click("#modals .backdrop:not(.under) button.build[data-order]:not([disabled])");
    else if (await has("button.option:not([disabled])")) await click("#modals .backdrop:not(.under) button.option:not([disabled])");
    else await b.key("Escape");
    if (log) steps.push(`answered "${titles.at(-1)}"`);
  }
}

async function newGame(url, saves) {
  await b.goto(url);
  await until2("window.ffh && window.ffh.ready", "the client to boot");
  await until(async () => (await modalTitles()).includes("New game"), "the new-game screen");
  await shot("new-game");
  await b.eval(`(() => {
    const set = (id, v) => { const e = document.getElementById(id); e.value = v; e.dispatchEvent(new Event("change")); };
    set("ng-world", "WORLDSIZE_TINY"); set("ng-seed", "${SEED}"); set("ng-opponents", "2");
  })()`);
  // A double click on Start starts one game (P5 review: it started two).
  const start = await b.eval(`(() => { const r = document.querySelector("#modals .modal-buttons button.primary").getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`);
  await b.dblclick(start.x, start.y);
  await waitHuman("the first turn");
  const games = fs.readdirSync(saves).filter((f) => fs.statSync(path.join(saves, f)).isDirectory());
  if (games.length !== 1) throw new Error(`a double click on Start made ${games.length} games: ${games}`);
  steps.push(`new game (Start double-clicked: one game): turn ${await S("S.status.turn")}`);
  await shot("first-turn");
}

/** The art client (docs/ART.md, look.js, artmap.js): the art loaded and draws the map (in its
 *  worker where there is one), the view is drawn in full, the map is not blank, a frame takes
 *  little time, the zoom goes through the art's steps and the chart, the interface uses the
 *  bitmap type and frames, and Tools → Options switches to the flat renderer and back. */
async function artClient() {
  await until2("!!(window.ffh.look.art || window.ffh.look.error)", "the art to load");
  const err = await b.eval("window.ffh.look.error ? String(window.ffh.look.error.message || window.ffh.look.error) : null");
  if (err) throw new Error(`the art did not load: ${err}`);
  if ((await b.eval("window.ffh.mapView.renderer()")) !== "art") throw new Error("the map is not drawn with the art");
  if (!(await b.eval(`document.documentElement.classList.contains("art") && !!document.querySelector("#btn-techs .ui-ptext") && !!document.querySelector("#empire canvas.ui-px")`))) {
    throw new Error("the interface has no bitmap type or icons");
  }
  await until2("window.ffh.mapView.art.settled()", "the art map drawn in full", 60000);
  const look = await b.eval(`(() => {
    const mv = window.ffh.mapView;
    const c = mv.canvas;
    const g = c.getContext("2d", { willReadFrequently: true });
    const w = Math.min(240, c.width >> 1), h = Math.min(240, c.height >> 1);
    const d = g.getImageData((c.width - w) >> 1, (c.height - h) >> 1, w, h).data;
    const colours = new Set();
    for (let i = 0; i < d.length; i += 4) colours.add((d[i] << 16) | (d[i + 1] << 8) | d[i + 2]);
    const times = [];
    for (let k = 0; k < 15; k++) { const t = performance.now(); mv.draw(); times.push(performance.now() - t); }
    times.sort((a, b) => a - b);
    return { colours: colours.size, median: times[7], worker: !!mv.art.stats.worker, ts: mv.cam.ts };
  })()`);
  if (look.colours < 8) throw new Error(`the art map is nearly blank (${look.colours} colours in the middle of the view)`);
  if (look.median > 60) throw new Error(`an art frame takes ${look.median.toFixed(1)} ms (median of 15)`);
  if (look.ts !== 64) throw new Error(`the art's default zoom is ${look.ts} px a plot (want 64)`);
  await shot("art-map");
  steps.push(`art map: ${look.colours} colours mid-view, a frame in ${look.median.toFixed(1)} ms (median), chunks drawn ${look.worker ? "by the worker" : "on the page"}`);
  // The zoom steps: 64 -> 32 (survey) -> 16 -> 8 (the chart) -> 8, then back up to 128 and to 64.
  const zooms = [];
  for (const k of ["-", "-", "-", "-", "+", "+", "+", "+", "+", "-"]) {
    await b.key(k);
    zooms.push(await b.eval("window.ffh.mapView.cam.ts"));
    if (zooms.at(-1) === 32 && k === "-") {
      await until2("window.ffh.mapView.art.settled()", "the survey zoom drawn", 60000);
      await shot("art-survey");
    }
    if (zooms.at(-1) === 16 && k === "-") await shot("art-chart");
  }
  if (JSON.stringify(zooms) !== JSON.stringify([32, 16, 8, 8, 16, 32, 64, 96, 128, 96])) throw new Error(`the zoom steps: ${zooms}`);
  await b.key("-");
  if ((await b.eval("window.ffh.mapView.cam.ts")) !== 64) throw new Error("the zoom did not come back to 64");
  steps.push(`zoom steps ${zooms.join(" ")} (the chart at 8 and 16, the art at 32-128)`);
  // Tools → Options: the flat renderer and back.
  await click("#btn-tools");
  await dialog("Tools");
  await click("#tool-options");
  await dialog("Options");
  await click("#option-flat-renderer");
  await until2(`window.ffh.mapView.renderer() === "flat"`, "the flat renderer (Options)");
  await click("#option-art-renderer");
  await until2(`window.ffh.mapView.renderer() === "art"`, "the art renderer again (Options)");
  await closeDialog("Options");
  await until2("window.ffh.mapView.art.settled()", "the art map drawn again", 60000);
  steps.push("Tools → Options: the flat renderer and back to the art");
}

/** `?renderer=flat`: the P5 renderer, kept for debugging, on the same game. */
async function flatRenderer(url) {
  await b.goto(`${url}/?renderer=flat`);
  await until2("window.ffh && window.ffh.ready", "the client to boot (flat)");
  await waitHuman("the game in the flat renderer");
  if ((await b.eval("window.ffh.mapView.renderer()")) !== "flat") throw new Error("?renderer=flat did not keep the flat renderer");
  await answerPopups(false);
  await shot("flat-renderer");
  steps.push("?renderer=flat: the flat map of P5");
}

/** `?renderer=3d`: the experimental 3D renderer (docs/R3D.md) on the same game: its WebGL frame is
 *  drawn and not blank, picking through the relief finds the plot under every plot's screen
 *  point, a click selects a unit, the hover follows the mouse, the zoom steps and a drag pan, the
 *  political layer, and back to the 2D art. Software rendering is slow, so the frame loop is idle
 *  (no animation) and the internal resolution halved. */
async function renderer3d(url) {
  const cfg = encodeURIComponent(JSON.stringify({ animate: false, renderScale: 0.5 }));
  await b.goto(`${url}/?renderer=3d&r3d=${cfg}`);
  await until2("window.ffh && window.ffh.ready", "the client to boot (3d)");
  await waitHuman("the game in the 3D renderer");
  await answerPopups(false);
  await until2(`window.ffh.mapView.renderer() === "3d" && !!window.ffh.mapView.r3d.cam`, "the 3D renderer's first frame", 180000);
  const frame = await b.eval(`(() => {
    const r = window.ffh.mapView.r3d;
    r.render(1);
    const gl = r.gl;
    const w = 48, h = 48, x = (r.canvas.width - w) >> 1, y = (r.canvas.height - h) >> 1;
    const px = new Uint8Array(w * h * 4);
    gl.readPixels(x, y, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
    const colours = new Set();
    for (let i = 0; i < px.length; i += 4) colours.add((px[i] << 16) | (px[i + 1] << 8) | px[i + 2]);
    return { colours: colours.size, props: r.staticCount, units: r.unitCount };
  })()`);
  if (frame.colours < 16) throw new Error(`the 3D frame is nearly blank (${frame.colours} colours mid-view)`);
  // Picking: the plot under each in-view plot's centre on screen is that plot (the ground's relief
  // and the camera's perspective do not shift the logical grid).
  const pick = await b.eval(`(() => {
    const mv = window.ffh.mapView, r = mv.r3d, S = window.ffh.S;
    const c = mv.center();
    let n = 0, bad = [];
    for (let dy = -3; dy <= 3; dy++) for (let dx = -4; dx <= 4; dx++) {
      const x = ((c.x + dx) % S.map.w + S.map.w) % S.map.w, y = c.y + dy;
      if (y < 0 || y >= S.map.h || S.map.vis[y * S.map.w + x] === 0) continue;
      const p = r.screenOf(x, y);
      if (p.x < 0 || p.y < 0 || p.x > mv.vw || p.y > mv.vh) continue;
      const q = r.screenToMap(p.x, p.y);
      n++;
      if (!q || q.x !== x || q.y !== y) bad.push([x, y, q && [q.x, q.y]]);
    }
    return { n, bad };
  })()`);
  if (pick.n < 10 || pick.bad.length) throw new Error(`3D picking: ${pick.bad.length} of ${pick.n} plots picked wrong: ${JSON.stringify(pick.bad.slice(0, 5))}`);
  // A click on one of our units selects it.
  const mine = await b.eval(`(() => { const S = window.ffh.S; const u = S.view.units.find((u) => u.owner === S.view.me.player); return u ? { x: u.x, y: u.y } : null; })()`);
  if (mine) {
    await b.eval("window.ffh.units.clearSelection && window.ffh.units.clearSelection()");
    const p = await plotPos(mine.x, mine.y);
    await b.mouse(p.x, p.y);
    await until2(`(() => { const s = window.ffh.S.sel; return !!s && s.x === ${mine.x} && s.y === ${mine.y}; })()`, "a click on a unit to select it (3D)");
    // The hover follows the mouse.
    await b.move(p.x + 3, p.y + 2);
    await until2(`(() => { const h = window.ffh.mapView.overlay.hover; return !!h && h.x === ${mine.x} && h.y === ${mine.y}; })()`, "the hover on the unit's plot (3D)");
  }
  // The zoom steps (pixels a plot) and back; a drag pans.
  const zooms = [];
  for (const k of ["-", "-", "+", "+", "+", "-"]) {
    await b.key(k);
    zooms.push(await b.eval("window.ffh.mapView.cam.ts"));
  }
  if (JSON.stringify(zooms) !== JSON.stringify([72, 56, 72, 96, 128, 96])) throw new Error(`3D zoom steps: ${zooms}`);
  const before = await b.eval("[window.ffh.mapView.cam.cx, window.ffh.mapView.cam.cy]");
  await b.key("ArrowLeft");
  const after = await b.eval("[window.ffh.mapView.cam.cx, window.ffh.mapView.cam.cy]");
  if (before[0] === after[0]) throw new Error("the arrow key did not pan the 3D map");
  await b.eval(`window.ffh.setLayer("political")`);
  await until2("window.ffh.mapView.r3d.scene.overlayOpts.political === 1", "the political layer in 3D");
  await b.eval(`window.ffh.setLayer("terrain")`);
  await b.eval("window.ffh.mapView.r3d.render(1)");
  await shot("renderer-3d");
  // Back to the 2D art: the 3D canvas goes.
  await b.eval(`window.ffh.setRenderer("art")`);
  await until2(`window.ffh.mapView.renderer() === "art" && !document.querySelector("canvas.map3d")`, "the art renderer after the 3D one");
  // A lost WebGL context (a driver reset) hands the map back to the 2D art.
  await b.eval(`window.ffh.setRenderer("3d")`);
  await until2(`window.ffh.mapView.renderer() === "3d" && !!window.ffh.mapView.r3d.cam`, "the 3D renderer again", 180000);
  const canLose = await b.eval(`(() => { const x = window.ffh.mapView.r3d.gl.getExtension("WEBGL_lose_context"); if (x) x.loseContext(); return !!x; })()`);
  if (canLose) await until2(`window.ffh.mapView.renderer() === "art" && !document.querySelector("canvas.map3d")`, "the art renderer after the 3D context was lost");
  else await b.eval(`window.ffh.setRenderer("art")`);
  steps.push(`?renderer=3d: a frame of ${frame.colours} colours mid-view (${frame.props} props, ${frame.units} unit figures), ${pick.n} plots picked right through the relief, click-select, hover, zoom ${zooms.join(" ")}, pan, political layer, back to the art${canLose ? ", a lost WebGL context falls back to the art" : " (no WEBGL_lose_context: context loss not checked)"}`);
}

/** Records the page's POST bodies in `window.__posts` (from now on). */
async function recordPosts() {
  await b.eval(`(() => {
    if (!window.__recording) {
      window.__recording = true;
      const f = window.fetch.bind(window);
      window.fetch = (u, o) => {
        if (o && o.method === "POST") try { window.__posts.push(JSON.parse(o.body)); } catch {}
        return f(u, o);
      };
    }
    window.__posts = [];
  })()`);
}
const posted = () => b.eval(`window.__posts.filter((p) => p.k === "cmd")`);

/** W (and Tab, "Next") reaches every group the cycle selects (`selectable`: C++
 *  `readyToSelect()`), not just the two nearest each other (P5 review). */
async function cycleAll() {
  const ready = await S("S.view.groups.filter((g) => g.selectable).map((g) => g.id)");
  if (ready.length < 2) return steps.push(`W: ${ready.length} group waiting, nothing to cycle`);
  const seen = new Set([await S("S.sel ? S.units.get(S.sel.units[0]).group : null")]);
  for (let k = 0; k < 2 * ready.length; k++) {
    await b.key("w");
    await new Promise((r) => setTimeout(r, 120));
    seen.add(await S("S.sel ? S.units.get(S.sel.units[0]).group : null"));
  }
  const missed = ready.filter((g) => !seen.has(g));
  if (missed.length) throw new Error(`W never selected groups ${missed} of ${ready}`);
  steps.push(`W selected all ${ready.length} waiting groups`);
}

/** A hotkey pressed right after the selection changed acts on the new selection, never with the
 *  previous selection's actions (P5 review): select the settler, see its Found action, then in
 *  one tick select another group and press B (nothing to found: nothing is sent) or Space
 *  (that group skips). */
async function staleSelection() {
  const pick = await S(`(() => {
    const me = S.view.me.player;
    const found = (u) => S.data.tables.UnitType[u.type].found;
    const settler = S.view.units.find((u) => u.owner === me && found(u));
    const other = settler && S.view.units.find((u) => u.owner === me && u.group !== settler.group && !found(u) && S.view.pending.ready_groups.includes(u.group));
    return other ? { settler: settler.group, other: other.group, units: S.groups.get(other.group).units } : null;
  })()`);
  if (!pick) return steps.push("no second group for the selection check");
  await b.eval(`window.ffh.units.selectGroup(${pick.settler})`);
  await until2(`!!document.querySelector('#unit-panel button.action[data-type="MISSION_FOUND"]:not([disabled])')`, "the settler's Found action");
  await recordPosts();
  const stale = await b.eval(`(() => {
    window.ffh.units.selectGroup(${pick.other});
    const shown = !!document.querySelector('#unit-panel button.action[data-type="MISSION_FOUND"]');
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "b", bubbles: true }));
    return shown;
  })()`);
  if (stale) throw new Error("the settler's Found action is drawn under the other group");
  await new Promise((r) => setTimeout(r, 500));
  let sent = await posted();
  if (sent.length) throw new Error(`B after the selection changed sent ${JSON.stringify(sent)}`);
  if (await S("S.view.cities.some((c) => c.owner === S.view.me.player)")) throw new Error("a city was founded");
  await b.eval(`window.ffh.units.selectGroup(${pick.settler})`);
  await until2(`!!document.querySelector('#unit-panel button.action[data-type="MISSION_FOUND"]:not([disabled])')`, "the settler's Found action again");
  await b.eval(`(() => {
    window.ffh.units.selectGroup(${pick.other});
    document.dispatchEvent(new KeyboardEvent("keydown", { key: " ", bubbles: true }));
  })()`);
  await until(async () => (sent = await posted()).length > 0, "the skip of the other group");
  if (sent.length !== 1 || sent[0].c !== "mission" || JSON.stringify(sent[0].units) !== JSON.stringify(pick.units)) throw new Error(`Space after the selection changed sent ${JSON.stringify(sent)} (want a mission of ${pick.units})`);
  steps.push("hotkeys right after a selection change act on the new selection");
  await new Promise((r) => setTimeout(r, 300));
  await b.eval("window.ffh.units.clearSelection()");
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

/** The city's production queue: no ✕ on the only order (the exe pops only from a longer queue,
 *  P5 review); Shift-click appends, then ✕ removes the appended order. */
async function cityQueue() {
  const city = await S("S.view.cities.find((c) => c.owner === S.view.me.player)");
  await b.eval(`window.ffh.emit("open-city", ${city.id})`);
  const count = (what) => `document.querySelectorAll("#modals .backdrop:not(.under) ol.queue ${what}").length`;
  await until2(`${count("li")} === 1 && !document.querySelector("#modals ol.queue li.dim")`, "the city's queue of one order");
  if (await b.eval(count("button"))) throw new Error("a remove button on the only order");
  await b.keyDown("Shift");
  await click("#modals .backdrop:not(.under) button.build[data-order]:not([disabled])");
  await b.keyUp("Shift");
  await until2(`${count("li")} === 2 && ${count("button")} === 2`, "the appended order with remove buttons");
  await click('#modals .backdrop:not(.under) ol.queue button[data-index="1"]');
  await until2(`${count("li")} === 1 && ${count("button")} === 0`, "the appended order removed");
  await b.key("Escape");
  await until(async () => !(await modalTitles()).length, "the city dialog closed");
  steps.push("city queue: appended with Shift, removed with ✕, no ✕ on the only order");
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

/** Enter while groups wait selects the next one and does not end the turn (Civ4: CONTROL_ENDTURN
 *  only while the end-turn message is up, else CONTROL_CYCLEUNIT; P6 review). */
async function enterCycles() {
  const waiting = await S("S.view.groups.filter((g) => g.selectable).map((g) => g.id)");
  if (!waiting.length) return steps.push("Enter: no group waiting");
  if (await S("window.ffh.S.status.phase !== 'human'")) return steps.push("Enter: not the player's turn");
  await recordPosts();
  await b.key("Enter");
  await until2(`(() => { const S = window.ffh.S; const g = S.sel && S.units.get(S.sel.units[0]); return !!g && ${JSON.stringify(waiting)}.includes(g.group); })()`, "Enter to select a waiting group");
  await new Promise((r) => setTimeout(r, 400));
  const sent = (await posted()).filter((c) => c.c === "end_turn");
  if (sent.length) throw new Error(`Enter ended the turn while ${waiting.length} groups waited`);
  if ((await modalTitles()).includes("End the turn?")) throw new Error("Enter asked to end the turn while groups waited");
  steps.push(`Enter with ${waiting.length} groups waiting: the next one selected, the turn not ended`);
}

/** Ends `n` turns with the End turn button (the gate when units wait: end anyway). */
async function endTurns(n) {
  await enterCycles();
  for (let k = 0; k < n; k++) {
    const turn = await S("S.status.turn");
    await click("#end-turn");
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

const TOP = "#modals .backdrop:not(.under)";
const q = (sel) => JSON.stringify(sel);
/** Waits for the dialog titled `title`, or one matching the regex. */
const dialog = (title) => until(async () => (await modalTitles()).some((t) => (title instanceof RegExp ? title.test(t) : t === title)), `the dialog ${title}`);
async function closeDialog(title) {
  await b.key("Escape");
  await until(async () => !(await modalTitles()).includes(title), `the dialog ${title} closed`);
}

/** The city screen (P6): a worked plot clicked stops being worked (a specialist instead, the
 *  citizens' automation off), clicked again it is worked; the automation back on. */
async function cityScreen() {
  const city = await S("S.view.cities.find((c) => c.owner === S.view.me.player) || null");
  if (!city) return steps.push("no city for the city screen");
  const p = await plotPos(city.x, city.y);
  await b.dblclick(p.x, p.y);
  await dialog(city.name);
  const screen = `${TOP} .modal.city-screen`;
  await until2(`!!document.querySelector(${q(`${screen} .cgrid .cplot.centre`)})`, "the city screen's plots");
  await shot("city-screen");
  const worked = await b.eval(`(() => { const e = document.querySelector(${q(`${screen} .cplot.worked:not(.centre)`)}); return e ? e.dataset.i : null; })()`);
  if (worked === null) {
    steps.push("city screen: no worked plot to click");
  } else {
    const sel = `${screen} .cplot[data-i="${worked}"]`;
    await click(sel);
    await until2(`(() => { const e = document.querySelector(${q(sel)}); return e && !e.classList.contains("worked"); })()`, "the plot no longer worked");
    await until2(`!!document.querySelector(${q(`${screen} .spec`)}) && !document.querySelector("#city-auto-citizens").checked`, "a specialist and the automation off");
    await shot("city-specialist");
    await click(sel);
    await until2(`(() => { const e = document.querySelector(${q(sel)}); return e && e.classList.contains("worked"); })()`, "the plot worked again");
    await click("#city-auto-citizens");
    await until2(`document.querySelector("#city-auto-citizens").checked`, "the automation back on");
    steps.push(`city screen: plot ${worked} clicked off (a specialist) and on, the automation off and on`);
  }
  await closeDialog(city.name);
}

/** The empire screens (P6) through their keys: the tech tree, civics, religion, the sliders,
 *  diplomacy, the cities list. */
async function empireScreens() {
  // F6: the tech tree; a click researches a tech.
  await b.key("F6");
  await dialog("Technology");
  await until2(`document.querySelectorAll(${q(`${TOP} button.tech`)}).length > 10`, "the techs");
  await shot("tech-tree");
  const tech = await b.eval(`(() => { const e = [...document.querySelectorAll(${q(`${TOP} button.tech.can`)})].find((x) => !x.classList.contains("researching")); return e ? Number(e.dataset.tech) : null; })()`);
  if (tech !== null) {
    await click(`${TOP} button.tech[data-tech="${tech}"]`);
    await until2(`(() => { const r = window.ffh.S.view.me.research; return !!r && r.tech === ${tech}; })()`, "the research picked in the tech tree");
    await until2(`!!document.querySelector(${q(`${TOP} button.tech.researching[data-tech="${tech}"]`)})`, "the tree showing it");
    steps.push(`tech tree (F6): researching ${tech}`);
  }
  await closeDialog("Technology");
  // F3: civics: a pick shows the change, the revolution when it is possible.
  await b.key("F3");
  await dialog("Civics");
  await until2(`!!document.querySelector(${q(`${TOP} button.civic`)})`, "the civics");
  const civic = await b.eval(`(() => { const e = document.querySelector(${q(`${TOP} button.civic:not(.current):not([disabled])`)}); return e ? Number(e.dataset.civic) : null; })()`);
  if (civic === null) steps.push("civics (F3): no civic available");
  else {
    await click(`${TOP} button.civic[data-civic="${civic}"]`);
    await until2(`(() => { const e = document.querySelector(${q(`${TOP} button.civic[data-civic="${civic}"]`)}); return !!e && e.classList.contains("chosen") && !document.querySelector(${q(`${TOP} .civic-summary.dim`)}); })()`, "the proposed change");
    await shot("civics");
    if (await b.eval(`!document.querySelector(${q(`${TOP} .modal-buttons button.primary`)}).disabled`)) {
      await click(`${TOP} .modal-buttons button.primary`);
      await until(async () => !(await modalTitles()).includes("Civics"), "the revolution");
      const now = await b.eval(`window.ffh.api.query("civics").then((c) => c.options.some((o) => o.civics.some((x) => x.civic === ${civic} && x.current)) || c.anarchy > 0)`);
      if (!now) throw new Error(`the revolution to civic ${civic} did not happen`);
      steps.push(`civics (F3): revolution to civic ${civic}`);
    } else steps.push(`civics (F3): civic ${civic} proposed, no revolution possible`);
  }
  if ((await modalTitles()).includes("Civics")) await closeDialog("Civics");
  // F7: religion.
  await b.key("F7");
  await dialog("Religion");
  await until2(`!!document.querySelector(${q(`${TOP} table.scores tr[data-religion]`)})`, "the religions");
  await closeDialog("Religion");
  // The sliders (the top bar's button; F2): research down a step and back.
  await click("#sliders-button");
  await dialog("Finances");
  const row = await b.eval(`(() => { const r = [...document.querySelectorAll(${q(`${TOP} table.sliders tr`)})].find((tr) => tr.querySelector("button.minus:not([disabled])")); return r ? Number(r.dataset.commerce) : null; })()`);
  if (row !== null) {
    const before = await S(`S.view.me.commerce_percent[${row}]`);
    await click(`${TOP} table.sliders tr[data-commerce="${row}"] button.minus`);
    await until2(`window.ffh.S.view.me.commerce_percent[${row}] < ${before}`, "the slider down");
    await until2(`!!document.querySelector(${q(`${TOP} table.sliders tr[data-commerce="${row}"] button.plus:not([disabled])`)})`, "the + button");
    await shot("sliders");
    await click(`${TOP} table.sliders tr[data-commerce="${row}"] button.plus`);
    await until2(`window.ffh.S.view.me.commerce_percent[${row}] === ${before}`, "the slider back");
    steps.push(`sliders: commerce ${row} ${before}% down a step and back`);
  }
  await closeDialog("Finances");
  await b.key("F2");
  await dialog("Finances");
  await closeDialog("Finances");
  // F4: diplomacy; the trade screen with a player met.
  await b.key("F4");
  await dialog("Foreign affairs");
  await until2(`!document.querySelector(${q(`${TOP} .modal-body p.dim`)}) || document.querySelector(${q(`${TOP} .modal-body p.dim`)}).textContent !== "…"`, "the players");
  if (await b.eval(`!!document.querySelector(${q(`${TOP} button[data-trade]`)})`)) {
    await click(`${TOP} button[data-trade]`);
    await dialog(/^Trade/);
    await until2(`!!document.querySelector(${q(`${TOP} .trade .trade-side`)})`, "the trade tables");
    await shot("trade");
    await b.key("Escape");
    steps.push("diplomacy (F4): the trade screen");
  } else steps.push("diplomacy (F4): nobody met yet");
  await until(async () => (await modalTitles()).includes("Foreign affairs"), "back to foreign affairs");
  await closeDialog("Foreign affairs");
  // F1: the cities.
  await b.key("F1");
  await dialog("Your cities");
  await closeDialog("Your cities");
  steps.push("empire screens: F6 tech tree, F3 civics, F7 religion, F2 sliders, F4 diplomacy, F1 cities");
}

/** The keys list (?), the go-to mode (G, then Esc), the map layers. */
async function keysAndLayers() {
  await b.key("?");
  await dialog("Keys");
  await closeDialog("Keys");
  if (!(await S("!!S.sel"))) await b.key("w");
  if (await S("!!S.sel")) {
    await until2(`window.ffh.units.currentActions().length > 0`, "the selection's actions");
    if (await b.eval(`window.ffh.units.currentActions().some((a) => a.kind === "target" && a.mode === "INTERFACEMODE_GO_TO" && a.enabled)`)) {
      await b.key("g");
      await until2(`(() => { const m = window.ffh.units.currentMode(); return !!m && m.kind === "target" && m.action.mode === "INTERFACEMODE_GO_TO"; })()`, "the go-to mode (G)");
      await b.key("Escape");
      await until2(`!window.ffh.units.currentMode()`, "the go-to mode left (Esc)");
      steps.push("G: the go-to mode, Esc leaves it");
    }
  }
  for (const layer of ["political", "yield", "terrain"]) {
    await click(`#mapwrap .layers button[data-layer="${layer}"]`);
    await until2(`window.ffh.mapView.layer === "${layer}" && document.querySelector('#mapwrap .layers button[data-layer="${layer}"]').classList.contains("active")`, `the ${layer} layer`);
    if (layer !== "terrain") await shot(`layer-${layer}`);
  }
  steps.push("map layers: political, yields, terrain");
}

/** The tools (P6): the inspector, debug reveal, AI autoplay, the spectator mode. */
async function toolsAndSpectate() {
  const tool = async (id, title) => {
    await click("#btn-tools");
    await dialog("Tools");
    await click(`#${id}`);
    if (title) await dialog(title);
  };
  // The inspector: our player.
  await tool("tool-inspect", "Inspector");
  await b.eval(`[...document.querySelectorAll(${q(`${TOP} .modal-body .row button.small`)})].find((x) => x.textContent === "your player").click()`);
  await until2(`/^Player \{/.test(document.querySelector(${q(`${TOP} pre.inspect`)}).textContent)`, "the player inspected");
  await shot("inspector");
  await closeDialog("Inspector");
  steps.push("inspector: the player's rules data");
  // Debug reveal on and off.
  await tool("tool-reveal");
  await until2(`window.ffh.S.status.reveal === true && !!document.querySelector("#phase .badge.reveal")`, "debug reveal on");
  await shot("debug-reveal");
  await tool("tool-reveal");
  await until2(`window.ffh.S.status.reveal === false && !document.querySelector("#phase .badge.reveal")`, "debug reveal off");
  steps.push("debug reveal on and off");
  // AI autoplay for one turn.
  let turn = await S("S.status.turn");
  await tool("tool-autoplay", "AI autoplay");
  await b.eval(`document.getElementById("autoplay-turns").value = "1"`);
  await click(`${TOP} .modal-buttons button.primary`);
  await until2(`window.ffh.S.status.turn > ${turn}`, "the autoplay turn", 90000);
  await waitHuman("control after the autoplay");
  if (await S("S.view.me.ai_auto_play")) throw new Error("still on AI autoplay after its turn");
  await answerPopups();
  steps.push(`AI autoplay: turn ${turn} played by the AI`);
  // A long AI autoplay stopped from the top bar (P6 review: nothing could stop it).
  turn = await S("S.status.turn");
  await tool("tool-autoplay", "AI autoplay");
  await b.eval(`document.getElementById("autoplay-turns").value = "5000"`);
  await click(`${TOP} .modal-buttons button.primary`);
  await until2(`!!document.querySelector("#autoplay-stop")`, "the autoplay bar's Stop", 60000);
  await shot("autoplay-running");
  await click("#autoplay-stop");
  await waitHuman("control after the stopped autoplay", 120000);
  const stoppedAt = await S("S.status.turn");
  if (await S("S.status.ai_auto_play")) throw new Error("the AI autoplay is still on after Stop");
  if (stoppedAt > turn + 100) throw new Error(`Stop took ${stoppedAt - turn} turns`);
  if (await S("!!document.querySelector('#autoplay-stop')")) throw new Error("the autoplay bar stays after Stop");
  await answerPopups();
  steps.push(`AI autoplay of 5000 turns from turn ${turn} stopped from the top bar: control at turn ${stoppedAt}`);
  // The spectator mode: plays, pauses, steps, stops.
  turn = await S("S.status.turn");
  await tool("tool-spectate");
  await until2(`!!document.querySelector("#phase .spectate") && window.ffh.S.status.reveal === true`, "the spectator bar and the reveal");
  await until2(`window.ffh.S.status.turn >= ${turn + 2}`, "two spectated turns", 120000);
  await shot("spectating");
  if ((await modalTitles()).length) throw new Error(`dialogs while spectating: ${await modalTitles()}`);
  await click("#spectate-pause");
  await until2(`!!document.querySelector("#spectate-play") && !window.ffh.tools.spectatorBusy()`, "the spectator mode paused", 60000);
  await waitHuman("the paused spectator mode");
  const paused = await S("S.status.turn");
  await new Promise((r) => setTimeout(r, 600));
  if ((await S("S.status.turn")) !== paused) throw new Error("turns went on after the pause");
  await click("#spectate-step");
  await until2(`window.ffh.S.status.turn === ${paused + 1} && !window.ffh.tools.spectatorBusy()`, "one stepped turn", 60000);
  await click("#spectate-stop");
  await until2(`!document.querySelector("#phase .spectate") && window.ffh.S.status.reveal === false`, "the spectator mode stopped (reveal off)");
  await waitHuman("control after spectating");
  await answerPopups();
  steps.push(`spectator mode: turns ${turn}-${paused} played, paused, one step, stopped`);
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
async function loadStaged(bin, saves, kind, { answer = true } = {}) {
  const st = await stage(bin, kind, path.join(saves, `staged-${kind}`));
  await loadSave(`staged-${kind}/${kind}`, st.turn);
  if (answer) await answerPopups(false);
  return st;
}

/** Loads save `name` (`GAME/SAVE`) through the load screen and waits for its first turn `turn`
 *  (a new journal: also when the page already shows a game at that turn). */
async function loadSave(name, turn) {
  const journal = await S("S.status && S.status.game && S.status.game.journal");
  await click("#btn-load");
  const sel = `#modals button.save[data-save="${name}"]`;
  await until2(`!!document.querySelector(${JSON.stringify(sel)})`, `the save ${name} in the load screen`);
  await click(sel);
  await until2(`(() => { const S = window.ffh.S; return S.status && S.status.phase === "human" && S.status.game && S.status.game.journal !== ${JSON.stringify(journal)} && S.status.turn === ${turn} && S.view && S.view.turn === ${turn}; })()`, `the game ${name}`);
  await new Promise((r) => setTimeout(r, 300));
}

/** Clicks `sel` once it has stood still for a moment (a dialog whose body loads after it
 *  opens, such as the city screen, moves its buttons). */
async function clickSteady(sel, ms = 10000) {
  const t0 = Date.now();
  let last = "";
  let still = 0;
  while (still < 3 && Date.now() - t0 < ms) {
    const r = await b.eval(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return ""; const r = e.getBoundingClientRect(); return [r.left, r.top, r.width, r.height].join(","); })()`);
    still = r && r === last ? still + 1 : 0;
    last = r;
    await new Promise((res) => setTimeout(res, 120));
  }
  await click(sel);
}

/** Presses "Later" on the popup whose title matches `re` (waiting for it). */
async function later(re) {
  await until(async () => (await modalTitles()).some((t) => re.test(t)), `a popup ${re}`);
  await clickSteady('#modals .backdrop:not(.under) .modal-buttons button[data-value="later"]');
  await until(async () => !(await modalTitles()).some((t) => re.test(t)), `the popup ${re} set aside`);
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
  // Watch for the combat flash (units.js flashCombat -> mapView.flashTile) across the resolution.
  await b.eval(`(() => { window.__fmax = 0; window.__fi = setInterval(() => { const f = window.ffh.mapView.flashes; if (Array.isArray(f) && f.length > window.__fmax) window.__fmax = f.length; }, 16); })()`);
  await b.mouse(e.x, e.y, "right");
  await until2(`(() => { const S = window.ffh.S; const me = S.units.get(${st.unit}); return !S.units.has(${st.enemy}) || !me || me.hp < me.max_hp; })()`, "the combat");
  const flashed = await b.eval(`(() => { clearInterval(window.__fi); const f = window.ffh.mapView.flashes; return Math.max(window.__fmax || 0, Array.isArray(f) ? f.length : 0); })()`);
  if (!flashed) throw new Error("no combat flash was shown on the attack");
  const won = await S(`!S.units.has(${st.enemy})`);
  steps.push(`attacked from the odds tooltip (${odds.replace(/\s+/g, " ").slice(0, 40)}…): ${won ? "won" : "lost or damaged"}; combat flash shown`);
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

/** The trade screen (P6, `stage trade`: a civilization met at peace, gold trading): F4, Trade…,
 *  the tables, 7 gold put into the deal, Propose: accepted, the gold gone. */
async function stagedTrade(bin, saves) {
  await loadStaged(bin, saves, "trade");
  const gold = await S("S.view.me.gold");
  await b.key("F4");
  await dialog("Foreign affairs");
  await until2(`!!document.querySelector(${q(`${TOP} button[data-trade]`)})`, "the Trade button");
  await click(`${TOP} button[data-trade]`);
  await dialog(/^Trade with/);
  await until2(`document.querySelectorAll(${q(`${TOP} .trade .trade-side`)}).length === 2`, "the trade tables");
  const ours = `${TOP} .trade .trade-side:first-child .trade-item[data-item="TRADE_GOLD"]`;
  await click(`${ours} button`);
  await until2(`!!document.querySelector(${q(`${ours}.on input.amount`)})`, "the gold amount");
  await b.eval(`(() => { const i = document.querySelector(${q(`${ours} input.amount`)}); i.value = "7"; i.dispatchEvent(new Event("input")); })()`);
  await shot("trade");
  await click(`${TOP} .modal-buttons button.primary`);
  await until(async () => !(await modalTitles()).some((t) => t.startsWith("Trade with")), "the deal accepted (the trade screen closes)");
  await until2(`window.ffh.S.view.me.gold === ${gold - 7}`, "7 gold given");
  await closeDialog("Foreign affairs");
  steps.push("trade screen (stage trade): 7 gold offered and accepted");
}

/** Selects the group of unit `u` with a click on its plot. */
async function selectUnit(u) {
  const unit = await S(`S.units.get(${u})`);
  const p = await plotPos(unit.x, unit.y);
  await b.mouse(p.x, p.y);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.includes(${u})`, `unit ${u} selected`);
}

const automated = (u) => `(() => { const S = window.ffh.S; const u = S.units.get(${u}); const g = u && S.groups.get(u.group); return !!g && g.automated !== null && g.automated !== undefined; })()`;

/** P7, `stage automate`: the worker's Automation section; a click automates it and the AI gives
 *  it orders at once (it waits for none); the scout explores by the hotkey E; the unit panel
 *  marks automated units. */
async function stagedAutomate(bin, saves) {
  const st = await loadStaged(bin, saves, "automate");
  await selectUnit(st.worker);
  const build = '#unit-panel button.action[data-kind="automate"][data-type="AUTOMATE_BUILD"]';
  await until2(`!!document.querySelector(${q(build)})`, "the automate buttons");
  await shot("automate-actions");
  await click(build);
  await until2(automated(st.worker), "the worker automated");
  await until2(`(() => { const S = window.ffh.S; const u = S.units.get(${st.worker}); const g = u && S.groups.get(u.group); return !!g && (g.missions.length > 0 || !g.ready); })()`, "the automated worker's orders");
  await selectUnit(st.scout);
  await until2(`!!document.querySelector('#unit-panel button.action[data-kind="automate"][data-type="AUTOMATE_EXPLORE"]')`, "the scout's explore button");
  await b.key("e");
  await until2(automated(st.scout), "the scout exploring");
  await selectUnit(st.worker);
  await until2(`!!document.querySelector("#unit-panel .badge.auto")`, "the automated badge");
  await shot("automated");
  steps.push("automate buttons (stage automate): a worker automated with a click, a scout exploring with E");
}

/** P7, `stage election`: the human secretary general's election popup (deferred, like events
 *  and votes): a resolution put to the vote. */
async function stagedElection(bin, saves) {
  const st = await loadStaged(bin, saves, "election", { answer: false });
  const opt = `${TOP} .modal button.option[data-option="0"]`;
  const shown = `!!document.querySelector(${q(opt)}) && /secretary general/.test(document.querySelector(${q(`${TOP} .modal`)}).textContent)`;
  // The popups before it in C++ order (events, votes) are answered first.
  for (let k = 0; k < 20 && !(await b.eval(shown)); k++) {
    const other = `${TOP} .modal button.option:not([disabled])`;
    if (await b.eval(`!!document.querySelector(${q(other)})`)) await click(other);
    await new Promise((r) => setTimeout(r, 400));
  }
  await until2(shown, "the election popup", 30000);
  await shot("election");
  await click(opt);
  await until2(`(window.ffh.S.view.pending.elections ?? []).every((e) => e.selection !== ${st.selection})`, "the election answered");
  steps.push("election popup (stage election): a resolution put to the vote");
  await answerPopups(false);
}

/** P7, `stage settlement`: a Kuriotates settler founds a city below the city limit: the
 *  city-or-settlement decision (BUTTONPOPUP_CONFIRMSETTLEMENT), answered "settlement". */
async function stagedSettlement(bin, saves) {
  const st = await loadStaged(bin, saves, "settlement");
  await selectUnit(st.settler);
  await b.key("b");
  await until(async () => (await modalTitles()).some((t) => /city or settlement/i.test(t)), "the city-or-settlement decision", 30000);
  await shot("settlement");
  await click('#modals .modal.decision button.option[data-option="1"]');
  await until2(`(() => { const S = window.ffh.S; return !!(S.view && S.view.cities.find((c) => c.x === ${st.x} && c.y === ${st.y} && c.owner === S.view.me.player)); })()`, "the new settlement");
  steps.push("city-or-settlement decision (stage settlement): a settlement founded");
  await answerPopups(false);
}

/** Shift waypoints, on the staged AutoMoves situation (the warrior `ready` is the only group
 *  waiting; another has orders): with Shift-right-clicks (P5 review), or (`how` "goto") in the
 *  go-to mode, G then Shift-clicks (C++ `selectionListMove` with bShift: P6 review). While Shift
 *  is held nothing is sent and the warrior stays selected (the go-to mode stays on); on release
 *  both legs are sent for the warrior, appended (the go-to mode ends), and its orders end at the
 *  second. */
async function shiftWaypoints(bin, saves, how = "right") {
  const st = await loadStaged(bin, saves, "automoves");
  const w = await S(`S.units.get(${st.ready})`);
  const p = await plotPos(w.x, w.y);
  await b.mouse(p.x, p.y);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.length === 1 && window.ffh.S.sel.units[0] === ${st.ready}`, "the waiting warrior selected");
  // Two land plots: one with a path, then a neighbour of it with a path (neither the start).
  const t = await b.eval(`(async () => {
    const S = window.ffh.S;
    const ok = async (x, y) => { const r = await window.ffh.api.query("path", { units: [${st.ready}], x, y }); return r.ok && r.declare_war_team === null && !r.attack; };
    const around = [[1,0],[-1,0],[0,1],[0,-1],[1,1],[-1,-1],[1,-1],[-1,1]];
    const at = (x, y, dx, dy) => ({ x: (x + dx + S.map.w) % S.map.w, y: y + dy });
    for (const [dx, dy] of around) {
      const a = at(${w.x}, ${w.y}, dx, dy);
      if (a.y < 0 || a.y >= S.map.h || !(await ok(a.x, a.y))) continue;
      for (const [ex, ey] of around) {
        const c = at(a.x, a.y, ex, ey);
        if (c.y < 0 || c.y >= S.map.h || (c.x === ${w.x} && c.y === ${w.y})) continue;
        if (await ok(c.x, c.y)) return [a, c];
      }
    }
    return null;
  })()`);
  if (!t) return steps.push("no two plots for the waypoints");
  const p1 = await plotPos(t[0].x, t[0].y);
  const p2 = await plotPos(t[1].x, t[1].y);
  const p1b = await plotPos(t[0].x, t[0].y);
  if (p1.x !== p1b.x || p1.y !== p1b.y) throw new Error("the map moved while placing the waypoints");
  const button = how === "goto" ? "left" : "right";
  const goTo = `(() => { const m = window.ffh.units.currentMode(); return !!m && m.kind === "target" && m.action.mode === "INTERFACEMODE_GO_TO"; })()`;
  if (how === "goto") {
    await until2(`window.ffh.units.currentActions().some((a) => a.kind === "target" && a.mode === "INTERFACEMODE_GO_TO" && a.enabled)`, "the warrior's go-to action");
    await b.key("g");
    await until2(goTo, "the go-to mode (G)");
  }
  await recordPosts();
  await b.keyDown("Shift");
  await b.mouse(p1.x, p1.y, button);
  await until2(`(() => { const w = window.ffh.units.pendingWaypoints(); return w && w.points.length === 1; })()`, "the first waypoint");
  if (how === "goto" && !(await b.eval(goTo))) throw new Error("the go-to mode ended at the first Shift-click");
  await b.mouse(p2.x, p2.y, button);
  await until2(`(() => { const w = window.ffh.units.pendingWaypoints(); return w && w.points.length === 2; })()`, "the second waypoint");
  await new Promise((r) => setTimeout(r, 300));
  const early = await posted();
  if (early.length) throw new Error(`sent while Shift was held: ${JSON.stringify(early)}`);
  if (!(await S(`S.sel && S.sel.units.length === 1 && S.sel.units[0] === ${st.ready}`))) throw new Error("the selection moved while Shift was held");
  await shot(how === "goto" ? "waypoints-goto" : "waypoints");
  await b.keyUp("Shift");
  if (how === "goto") await until2("!window.ffh.units.currentMode()", "the go-to mode to end when Shift is released");
  await until(async () => (await posted()).filter((c) => c.c === "move").length >= 2, "the waypoints sent");
  const sent = (await posted()).filter((c) => c.c === "move");
  const want = t.map(({ x, y }) => ({ units: [st.ready], x, y, append: true }));
  const got = sent.map(({ units, x, y, append }) => ({ units, x, y, append }));
  if (JSON.stringify(got) !== JSON.stringify(want)) throw new Error(`the waypoints sent: ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
  await waitHuman("the control point after the waypoints");
  await until2(`(() => {
    const S = window.ffh.S;
    const u = S.units.get(${st.ready});
    const g = u && S.groups.get(u.group);
    const last = g && g.missions.length ? g.missions[g.missions.length - 1] : null;
    return u && ((u.x === ${t[1].x} && u.y === ${t[1].y}) || (last && last.d1 === ${t[1].x} && last.d2 === ${t[1].y}));
  })()`, "the warrior's orders to end at the second waypoint", 10000);
  steps.push(`${how === "goto" ? "G, Shift-click" : "Shift-right-click"}: two waypoints held, sent on release, the warrior's orders end at (${t[1].x}, ${t[1].y})`);
}

/** A staged load: the warrior's Load action asks which of two triremes (the LOADUNIT picker);
 *  the second one picked, the warrior is in it. */
async function stagedLoad(bin, saves) {
  const st = await loadStaged(bin, saves, "load", { answer: false });
  // Popups set aside ("Later") in one game come back in the next one at the same turn (P5
  // review: C++ saves the popup queue with the player): load the same save again.
  await later(/^Choose research$/);
  await later(/^What should .* build\?$/);
  await loadSave("staged-load/load", st.turn);
  await until(async () => (await modalTitles()).includes("Choose research"), "the research popup again in the game loaded at the same turn", 10000);
  steps.push("popups set aside come back after loading a game at the same turn");
  await answerPopups(false);
  const p = await plotPos(st.x, st.y);
  await b.mouse(p.x, p.y);
  await until2(`!!document.querySelector('#unit-panel .unit-row[data-unit="${st.unit}"]')`, "the warrior in the stack list");
  await click(`#unit-panel .unit-row[data-unit="${st.unit}"]`);
  await until2(`window.ffh.S.sel && window.ffh.S.sel.units.length === 1 && window.ffh.S.sel.units[0] === ${st.unit}`, "the warrior selected");
  await until2(`!!document.querySelector('#unit-panel button.action[data-kind="load"]:not([disabled])')`, "the Load action");
  await click('#unit-panel button.action[data-kind="load"]');
  const pick = st.transports[1];
  await until(async () => (await modalTitles()).includes("Load into which transport?"), "the transport picker");
  await shot("transport-picker");
  await click(`#modals .modal-buttons button[data-value="${pick}"]`);
  await until2(`(() => { const u = window.ffh.S.units.get(${st.unit}); return u && u.transport === ${pick}; })()`, "the warrior in the picked trireme");
  steps.push(`loaded the warrior into trireme ${pick} through the transport picker`);
}

/** A game started by another client while the page shows the no-game New game dialog (which
 *  cannot be closed): the dialog goes (P5 review). */
async function newGameElsewhere(bin) {
  const saves = tempDir("smoke-elsewhere");
  const server = await startServer({ bin, saves, extra: ASSETS });
  try {
    await b.goto(server.url);
    await until2("window.ffh && window.ffh.ready", "the client to boot");
    await until(async () => (await modalTitles()).includes("New game"), "the no-game New game dialog");
    const r = await fetch(`${server.url}/api`, {
      method: "POST",
      headers: { "X-FFH": "1", "Content-Type": "application/json" },
      body: JSON.stringify({ id: 1, k: "session", op: "new", setup: { seed: 9, world: "WORLDSIZE_DUEL", opponents: 1 } }),
    });
    if (!(await r.json()).ok) throw new Error("another client could not start a game");
    await waitHuman("the game another client started");
    await until(async () => !(await modalTitles()).includes("New game"), "the New game dialog to close", 10000);
    steps.push("the no-game dialog closed when another client started a game");
  } finally {
    await b.goto("about:blank");
    await server.stop();
    fs.rmSync(saves, { recursive: true, force: true });
  }
}

async function crashScreen(bin) {
  const saves = tempDir("smoke-crash");
  const server = await startServer({ bin, saves, extra: [...ASSETS, "--new", "--world", "WORLDSIZE_DUEL", "--opponents", "1", "--seed", "2", "--panic-at", "1"] });
  try {
    await b.goto(server.url);
    await until2("window.ffh && window.ffh.ready", "the client to boot");
    await waitHuman("the crash test game");
    await answerPopups(false);
    await click("#end-turn");
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
    await newGame(server.url, saves);
    await artClient();
    await cycleAll();
    await staleSelection();
    await foundCity();
    await cityQueue();
    await moveUnit();
    await endTurns(TURNS);
    await cityScreen();
    await empireScreens();
    await keysAndLayers();
    await toolsAndSpectate();
    await saveAndLoad();
    await stagedAttack(bin, saves);
    await stagedCapture(bin, saves);
    await stagedTrade(bin, saves);
    await stagedAutomate(bin, saves);
    await stagedElection(bin, saves);
    await stagedSettlement(bin, saves);
    await stagedLoad(bin, saves);
    await shiftWaypoints(bin, saves);
    await shiftWaypoints(bin, saves, "goto");
    await flatRenderer(server.url);
    await renderer3d(server.url);
    await newGameElsewhere(bin);
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
