// The GPU bench (docs/R3D.md §10): the real 3D renderer on a real game's map (an AI game at turn 200,
// exported by tools/r3d-bench.mjs as `game.json`: the server's own data, map and view replies), at
// fixed spots, resolutions and zooms, timed on this device's graphics card. Published as a claude.ai artifact by
// tools/r3d-bench.mjs, so anyone can run it by opening a link; each run is stored in the artifact's
// database (the `db` capability), where Claude reads it back -- no commands to copy, no numbers to
// paste. `node tools/r3d-bench.mjs --serve` serves the same page locally.
//
// What a run measures, per case (spot x zoom x resolution, the default painterly look, the quality
// ladder off so every case is the full pipeline):
// - frame: the wall time of one synchronous frame (render, then a 1-pixel readback), median of the
//   frames measured -- CPU and GPU in series, so a little pessimistic against a pipelined game loop;
// - gpu: the sum of the per-pass GPU times (EXT_disjoint_timer_query_webgl2), where the browser
//   offers them, and the passes themselves.
// Plus the world build (sync) of the map. The location hash takes plain tokens only (artifact
// links pass nothing else): `#quick` runs one case, `#software` runs on a software rasteriser too.

import { setData, setView, setMap } from "../app/state.js";
import { Map3D, rendererInfo } from "../app/r3d/map3d.js";
import { DEFAULTS } from "../app/r3d/config.js";

/** Bumped when the cases or what they measure change (runs of different versions do not compare). */
const BENCH_VERSION = 1;
/** The repository revision the page was built from (tools/r3d-bench.mjs fills it in). */
const BUILD = "__BUILD__";

/** Where the camera looks on the bench's map (plots; the map point looked at is the plot's centre). */
const SCENES = [
  { key: "coast", label: "Coast, farms and towns", at: [60, 40] },
  { key: "heartland", label: "River, woods and hell terrain", at: [48, 15] },
];
const ZOOMS = [
  { ts: 40, label: "far" },
  { ts: 96, label: "gameplay" },
  { ts: 150, label: "close" },
];
const SIZES = [
  [1280, 720],
  [1920, 1080],
  [2560, 1440],
];
/** The case the verdict reads: 1080p at the gameplay zoom. */
const HEADLINE = { w: 1920, h: 1080, ts: 96 };

const $ = (id) => document.getElementById(id);
const hash = (location.hash || "").slice(1);
const quick = hash.includes("quick");
const frame = () => new Promise((res) => requestAnimationFrame(() => res()));
const sleep = (ms) => new Promise((res) => setTimeout(res, ms));
const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[s.length >> 1] : null;
};
const fmt = (ms) => (ms == null ? "–" : ms < 10 ? ms.toFixed(1) : String(Math.round(ms)));
const fps = (ms) => (ms ? Math.round(1000 / ms) : null);

/** What the browser says about the device: the GPU through WEBGL_debug_renderer_info (hidden by
 *  some browsers), and what a reader of the results needs to place them. */
function deviceInfo() {
  const c = document.createElement("canvas");
  const gl = c.getContext("webgl2");
  if (!gl) return { webgl2: false };
  const { name, software } = rendererInfo(gl);
  const dbg = gl.getExtension("WEBGL_debug_renderer_info");
  const info = {
    webgl2: true,
    gpu: name || String(gl.getParameter(gl.RENDERER)),
    vendor: String((dbg ? gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR)) || ""),
    software,
    timerQuery: !!gl.getExtension("EXT_disjoint_timer_query_webgl2"),
    maxTexture: gl.getParameter(gl.MAX_TEXTURE_SIZE),
    userAgent: navigator.userAgent,
    platform: navigator.userAgentData?.platform ?? navigator.platform ?? "",
    cores: navigator.hardwareConcurrency ?? null,
    memoryGb: navigator.deviceMemory ?? null,
    screen: [screen.width, screen.height],
    dpr: devicePixelRatio,
  };
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return info;
}

/** A short "Browser on OS" from the user agent, for the results table. */
function browserOs(ua) {
  const b = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "Browser";
  const os = /Windows/.test(ua) ? "Windows" : /Mac OS X|Macintosh/.test(ua) ? "macOS" : /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iOS" : /Linux/.test(ua) ? "Linux" : "";
  return os ? `${b} on ${os}` : b;
}

/** Verdict from a frame time: what a player would feel at that rate. */
function verdict(ms) {
  if (ms == null) return { cls: "", text: "No result" };
  if (ms <= 16.7) return { cls: "good", text: "Smooth (60 fps or better)" };
  if (ms <= 33.4) return { cls: "warn", text: "Playable (30 to 60 fps)" };
  return { cls: "bad", text: "Struggles (under 30 fps)" };
}

function setStatus(text) {
  $("status").textContent = text;
}

/** The view object the renderer reads (the client's MapView in miniature, as in lab.js). */
function makeView(map, sc) {
  const wrap = $("stage");
  const canvas = wrap.querySelector("canvas.map");
  return {
    wrap,
    canvas,
    ctx: canvas.getContext("2d"),
    cam: { cx: sc.at[0] + 0.5, cy: sc.at[1] + 0.5, ts: HEADLINE.ts },
    vw: HEADLINE.w,
    vh: HEADLINE.h,
    dpr: 1,
    map,
    overlay: { path: null, targets: null, hover: null, sel: null, units: [], waypoints: null },
    layer: "terrain",
    flashes: [],
    reduceMotion: () => true,
    request() {},
  };
}

/** One case: warm up (chunks build over a few frames), then time frames until enough are in. */
async function measureCase(r3d, view, sc, ts, w, h, scale = 1) {
  view.vw = w;
  view.vh = h;
  view.canvas.width = w;
  view.canvas.height = h;
  view.cam = { cx: sc.at[0] + 0.5, cy: sc.at[1] + 0.5, ts };
  r3d.ts = ts;
  r3d.cfg.renderScale = scale;
  const gl = r3d.gl;
  const px = new Uint8Array(4);
  for (let k = 0; k < 4; k++) r3d.render(1);
  gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
  await frame();
  r3d.pipeline.stats.ms = {};
  r3d.measuring = true;
  const times = [];
  const start = performance.now();
  const most = quick ? 4 : 40;
  while (times.length < most && (times.length < 5 || performance.now() - start < 2500)) {
    const t0 = performance.now();
    r3d.render(1);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
    times.push(performance.now() - t0);
    // Let the page breathe (the progress shows, the tab stays responsive) and collect timers.
    if (times.length % 3 === 0) {
      r3d.pipeline.collectTimings();
      await frame();
    }
  }
  r3d.measuring = false;
  for (let k = 0; k < 20 && r3d.pipeline.queries?.length; k++) {
    await sleep(25);
    r3d.pipeline.collectTimings();
  }
  const passes = {};
  for (const [k, s] of Object.entries(r3d.pipeline.stats.ms)) passes[k] = +(s.sum / Math.max(1, s.n)).toFixed(2);
  const gpuMs = Object.keys(passes).length ? +Object.values(passes).reduce((a, b) => a + b, 0).toFixed(2) : null;
  return {
    scene: sc.key,
    ts,
    w,
    h,
    scale,
    frameMs: +median(times).toFixed(2),
    frameP90: +[...times].sort((a, b) => a - b)[Math.floor(times.length * 0.9)].toFixed(2),
    frames: times.length,
    gpuMs,
    passes,
    drawn: r3d.scene.drawn ? { ...r3d.scene.drawn } : null,
  };
}

function caseRow(c) {
  const tr = document.createElement("tr");
  const sc = SCENES.find((s) => s.key === c.scene);
  const zoom = ZOOMS.find((z) => z.ts === c.ts)?.label ?? `ts ${c.ts}`;
  const v = verdict(c.frameMs);
  const cells = [
    sc?.label ?? c.scene,
    `${zoom}${c.scale !== 1 ? `, ${Math.round(c.scale * 100)}% scale` : ""}`,
    `${c.w}×${c.h}`,
    fmt(c.frameMs),
    fmt(c.gpuMs),
    String(fps(c.frameMs) ?? "–"),
  ];
  cells.forEach((t, k) => {
    const td = document.createElement("td");
    td.textContent = t;
    if (k >= 3) td.className = "num";
    if (k === 5) td.innerHTML = `<span class="pip ${v.cls}"></span>${t}`;
    tr.append(td);
  });
  return tr;
}

/** The headline: the median of the scenes' frame times at 1080p, gameplay zoom. */
function headlineOf(cases) {
  const hs = cases.filter((c) => c.w === HEADLINE.w && c.h === HEADLINE.h && c.ts === HEADLINE.ts && c.scale === 1);
  const frameMs = median(hs.map((c) => c.frameMs));
  const gpus = hs.map((c) => c.gpuMs).filter((x) => x != null);
  return { frameMs, gpuMs: gpus.length ? median(gpus) : null };
}

function showHeadline(h, device) {
  const v = verdict(h.frameMs);
  $("big").textContent = h.frameMs == null ? "–" : `${fmt(h.frameMs)} ms`;
  $("bigfps").textContent = h.frameMs == null ? "" : `≈ ${fps(h.frameMs)} fps`;
  const chip = $("chip");
  chip.className = `chip ${v.cls}`;
  chip.textContent = v.text;
  $("gpuline").textContent = h.gpuMs == null ? (device.timerQuery ? "GPU time: not reported" : "GPU timers not offered by this browser") : `GPU time ${fmt(h.gpuMs)} ms a frame`;
}

let db = null;

async function run(device) {
  const btn = $("run");
  btn.disabled = true;
  $("cases").querySelector("tbody").replaceChildren();
  $("saved").textContent = "";
  setStatus("Loading the map…");
  const game = await (await fetch(new URL("./game.json", import.meta.url))).json();
  setData(game.data);
  setView(game.view);
  setMap(game.map);
  const view = makeView(game.map, SCENES[0]);
  let r3d;
  const t0 = performance.now();
  try {
    r3d = new Map3D(view);
  } catch (e) {
    setStatus(e.code === "software-webgl" ? "This browser renders WebGL in software, so the results would not describe a graphics card." : `The renderer could not start: ${e.message}`);
    btn.disabled = false;
    return;
  }
  // The full pipeline as the game ships it, held still and without the quality ladder.
  r3d.running = false;
  cancelAnimationFrame(r3d.raf);
  r3d.cfg = { ...DEFAULTS, animate: false, autoQuality: 0 };
  r3d.autoLevel = 0;
  // The world build of the whole map, timed on its own (the first sync ran in the constructor's first
  // frame, if any; a fresh one here is what a game start pays).
  setStatus("Building the world…");
  await frame();
  r3d.mapSeen = null;
  r3d.viewSeen = null;
  const s0 = performance.now();
  r3d.sync();
  const sync = { totalMs: +(performance.now() - s0).toFixed(1), stages: Object.fromEntries(Object.entries(r3d.syncMs).map(([a, b]) => [a, +b.toFixed(1)])), startMs: +(s0 - t0).toFixed(1) };
  const plan = [];
  for (const sc of SCENES) for (const z of ZOOMS) for (const [w, h] of SIZES) plan.push({ sc, ts: z.ts, w, h, scale: 1 });
  // The quality ladder's last step (render scale 0.75), for the headline case.
  for (const sc of SCENES) plan.push({ sc, ts: HEADLINE.ts, w: HEADLINE.w, h: HEADLINE.h, scale: 0.75 });
  const todo = quick ? plan.filter((p) => p.sc.key === SCENES[0].key && p.ts === HEADLINE.ts && p.w === HEADLINE.w && p.scale === 1) : plan;
  const cases = [];
  for (const [k, p] of todo.entries()) {
    setStatus(`Measuring ${k + 1} of ${todo.length}: ${p.sc.label.toLowerCase()}, ${ZOOMS.find((z) => z.ts === p.ts).label} zoom, ${p.w}×${p.h}${p.scale !== 1 ? " at 75% scale" : ""}`);
    $("progress").value = k / todo.length;
    const c = await measureCase(r3d, view, p.sc, p.ts, p.w, p.h, p.scale);
    cases.push(c);
    $("cases").querySelector("tbody").append(caseRow(c));
    if (c.ts === HEADLINE.ts && c.w === HEADLINE.w && c.scale === 1) showHeadline(headlineOf(cases), device);
  }
  $("progress").value = 1;
  const headline = headlineOf(cases);
  showHeadline(headline, device);
  const result = {
    at: new Date().toISOString(),
    bench: BENCH_VERSION,
    build: BUILD,
    quick,
    durationS: +((performance.now() - t0) / 1000).toFixed(1),
    device,
    headline,
    sync,
    cases,
  };
  setStatus(`Done in ${Math.round(result.durationS)} s.`);
  globalThis.benchResult = result; // for tools/r3d-bench.mjs --check
  await save(result);
  btn.disabled = false;
  btn.textContent = "Run again";
}

async function save(result) {
  const out = $("saved");
  if (!db) {
    out.textContent = "Not recorded: results are saved only when this page is opened on claude.ai.";
    return;
  }
  if (result.device.software) {
    out.textContent = "Not recorded: a software renderer says nothing about a graphics card.";
    return;
  }
  try {
    await db.collection("runs").add(result);
    out.textContent = "Recorded in the results below.";
  } catch (e) {
    out.textContent = e?.code === "invalid_argument" ? "Not recorded: this link lets you run the bench but not save results. Ask the page's owner to invite you as an editor." : `Not recorded (${e?.code ?? "error"}). Run it again to retry.`;
  }
}

function showRuns(snap) {
  const body = $("runs").querySelector("tbody");
  body.replaceChildren();
  $("runs-empty").hidden = !snap.empty;
  for (const d of snap.docs) {
    const r = d.data();
    if (!r?.device) continue;
    const tr = document.createElement("tr");
    const v = verdict(r.headline?.frameMs);
    const when = new Date(r.at);
    const cells = [
      r.device.gpu || "Unknown GPU",
      browserOs(r.device.userAgent ?? ""),
      fmt(r.headline?.frameMs),
      fmt(r.headline?.gpuMs),
      String(fps(r.headline?.frameMs) ?? "–"),
      Number.isNaN(when.getTime()) ? "" : when.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
    ];
    cells.forEach((t, k) => {
      const td = document.createElement("td");
      td.textContent = t;
      if (k >= 2 && k <= 4) td.className = "num";
      if (k === 4) td.innerHTML = `<span class="pip ${v.cls}"></span>${t}`;
      if (k === 0) td.className = "gpu";
      tr.append(td);
    });
    if (r.quick) tr.title = "A one-case quick run";
    body.append(tr);
  }
}

async function main() {
  if (hash.includes("software")) globalThis.FFH_FORCE3D = true;
  const device = deviceInfo();
  $("gpu").textContent = device.webgl2 ? device.gpu || "Hidden by this browser" : "WebGL2 is not available";
  $("gpu-meta").textContent = device.webgl2 ? `${browserOs(device.userAgent)} · ${device.cores ?? "?"} CPU threads · ${device.timerQuery ? "GPU timers available" : "no GPU timers (frame times only)"}` : "";
  $("run").addEventListener("click", () => run(device));
  $("build").textContent = BUILD;
  // Recorded runs, live (shared by everyone who opens the page).
  const pending = globalThis.claude?.use?.("db");
  db = pending ? await pending.catch(() => null) : null;
  if (db) {
    db.collection("runs").orderBy("at", "desc").limit(100).onSnapshot(showRuns, () => ($("runs-note").textContent = "The recorded runs could not be loaded."));
  } else {
    $("runs-empty").hidden = false;
    $("runs-empty").textContent = "Recorded runs appear here when the page is opened on claude.ai.";
  }
  if (!device.webgl2) {
    setStatus("This browser has no WebGL2, which the 3D map needs.");
    $("run").disabled = true;
    return;
  }
  if (device.software && !globalThis.FFH_FORCE3D) {
    setStatus("This browser renders WebGL in software, so there is no graphics card to measure. Open the page in a browser with hardware acceleration on.");
    $("run").disabled = true;
    return;
  }
  // Start on its own: opening the page is the whole procedure.
  setStatus("Starting…");
  await sleep(800);
  run(device);
}

main().catch((e) => setStatus(`The bench failed: ${e?.message ?? e}`));
