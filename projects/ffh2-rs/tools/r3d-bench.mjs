#!/usr/bin/env node
// The GPU bench (docs/R3D.md §10, crates/ffh-play/r3d-lab/bench.html + bench.js): assembles it into
// one folder laid out as the published claude.ai artifact serves it -- the page (index.html), the
// bench and the lab's scenes and type tables under lab/, the renderer's modules under app/ -- with
// the repository revision filled in, then optionally serves or checks it.
//
//   node tools/r3d-bench.mjs --save SAVE [--bin FFH_PLAY] [--out DIR]
//                                                 assemble (default target/r3d-bench); prints the
//                                                 files to publish beside index.html
//
// The bench draws a real game: SAVE (e.g. `ffh-play stage corrupt DIR` -> DIR/corrupt.ffhsave, an AI
// game at turn 200, which the bench's spots in bench.js point into) is loaded by `ffh-play serve`, the
// whole map revealed, and the server's /api/data, /api/map and /api/view replies are stored as
// lab/game.json -- exactly what the client would receive.
//   node tools/r3d-bench.mjs --serve [--port N]   assemble and serve (open http://127.0.0.1:N/)
//   node tools/r3d-bench.mjs --check              assemble, run one case in headless Chromium on
//                                                 SwiftShader (#quick-software) and print the result
//
// The bench runs on whatever GPU the viewer's browser has; this machine has none, so --check only
// proves the page loads and measures, not what any card does.

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { ROOT, parseArgs, chromiumLaunch, findBin, tempDir, startServer } from "./web-lib.mjs";

const args = parseArgs(process.argv.slice(2), { out: "v", serve: "flag", port: "v", check: "flag", save: "v", bin: "v" });
if (!args.save) {
  console.error("r3d-bench: --save SAVE is required (an ffh-play save; `ffh-play stage corrupt DIR` makes one)");
  process.exit(2);
}
const OUT = path.resolve(args.out ?? path.join(ROOT, "target", "r3d-bench"));
const APP = path.join(ROOT, "crates", "ffh-play", "assets", "app");
const LAB = path.join(ROOT, "crates", "ffh-play", "r3d-lab");

/** The app modules the renderer reaches outside r3d/ (r3d imports ../state.js, ../tiles.js,
 *  ../stacks.js; state.js imports tiles.js). */
const APP_FILES = ["state.js", "tiles.js", "stacks.js"];

function build() {
  let rev = "unknown";
  try {
    rev = execFileSync("git", ["rev-parse", "--short", "HEAD"], { cwd: ROOT, encoding: "utf8" }).trim();
    if (execFileSync("git", ["status", "--porcelain", "--", "crates/ffh-play"], { cwd: ROOT, encoding: "utf8" }).trim()) rev += "+changes";
  } catch {
    /* not a git checkout */
  }
  return rev;
}

function copy(from, to) {
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(from, to);
}

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(dir, e.name)) : e.name.endsWith(".js") ? [path.join(dir, e.name)] : []));
}

/** The game the bench draws: the server's own replies for SAVE, the whole map revealed. */
async function exportGame(save) {
  const saves = tempDir("r3d-bench");
  for (const ext of [".ffhsave", ".ffhsess"]) {
    const src = save.replace(/\.ffhsave$/, ext);
    if (fs.existsSync(src)) fs.copyFileSync(src, path.join(saves, `game${ext}`));
  }
  const server = await startServer({ bin: findBin(args.bin), saves, extra: ["--load", path.join(saves, "game.ffhsave")] });
  try {
    const headers = { "X-FFH": "1" };
    await fetch(`${server.url}/api`, { method: "POST", headers: { ...headers, "Content-Type": "application/json" }, body: JSON.stringify({ id: 1, k: "session", op: "debug", reveal: true }) });
    const get = async (p) => (await fetch(`${server.url}${p}`, { headers })).json();
    return { data: await get("/api/data"), map: await get("/api/map"), view: await get("/api/view") };
  } finally {
    await server.stop();
  }
}

/** Assembles the bench into OUT; returns the published paths besides index.html. */
async function assemble() {
  fs.rmSync(OUT, { recursive: true, force: true });
  const files = [];
  const put = (from, rel) => {
    copy(from, path.join(OUT, rel));
    files.push(rel);
  };
  for (const f of APP_FILES) put(path.join(APP, f), `app/${f}`);
  for (const f of walk(path.join(APP, "r3d"))) put(f, `app/${path.relative(APP, f).split(path.sep).join("/")}`);
  fs.mkdirSync(path.join(OUT, "lab"), { recursive: true });
  fs.writeFileSync(path.join(OUT, "lab", "game.json"), JSON.stringify(await exportGame(path.resolve(args.save))));
  files.push("lab/game.json");
  const rev = build();
  fs.writeFileSync(path.join(OUT, "lab", "bench.js"), fs.readFileSync(path.join(LAB, "bench.js"), "utf8").replace("__BUILD__", rev));
  files.push("lab/bench.js");
  copy(path.join(LAB, "bench.html"), path.join(OUT, "index.html"));
  return { files: files.sort(), rev };
}

/** Serves OUT, the page wrapped in the skeleton the artifact host adds at publish time. */
async function serve(port) {
  const TYPES = { ".js": "text/javascript", ".json": "application/json", ".html": "text/html" };
  const server = http.createServer((req, res) => {
    const u = new URL(req.url, "http://x");
    const rel = u.pathname === "/" ? "index.html" : decodeURIComponent(u.pathname.slice(1));
    const file = path.join(OUT, rel);
    if (!file.startsWith(OUT) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      res.writeHead(404);
      res.end("not found");
      return;
    }
    res.writeHead(200, { "Content-Type": TYPES[path.extname(file)] ?? "application/octet-stream", "Cache-Control": "no-store" });
    if (rel === "index.html") {
      res.end(`<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover"><style>body{margin:0}</style></head><body>${fs.readFileSync(file, "utf8")}</body></html>`);
    } else fs.createReadStream(file).pipe(res);
  });
  await new Promise((r) => server.listen(port, "127.0.0.1", r));
  return server;
}

const { files, rev } = await assemble();
console.log(`r3d-bench: assembled ${files.length + 1} files in ${OUT} (build ${rev})`);
if (!args.serve && !args.check) console.log(JSON.stringify(files));

if (args.serve) {
  const server = await serve(Number(args.port ?? 8660));
  console.log(`r3d-bench: http://127.0.0.1:${server.address().port}/`);
} else if (args.check) {
  const server = await serve(0);
  const require = createRequire(import.meta.url);
  let pw;
  try {
    pw = require("playwright");
  } catch {
    pw = require(require.resolve("playwright", { paths: ["/opt/node22/lib/node_modules"] }));
  }
  const browser = await pw.chromium.launch(chromiumLaunch(false));
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await page.goto(`http://127.0.0.1:${server.address().port}/#quick-software`);
  await page.waitForFunction(() => globalThis.benchResult || /failed|could not|no WebGL2/i.test(document.getElementById("status")?.textContent ?? ""), null, { timeout: 600000 });
  const out = await page.evaluate(() => ({ status: document.getElementById("status").textContent, saved: document.getElementById("saved").textContent, result: globalThis.benchResult ?? null }));
  console.log(`status: ${out.status}`);
  console.log(`saved: ${out.saved}`);
  if (out.result) {
    const r = out.result;
    console.log(`device: ${r.device.gpu} (timer queries ${r.device.timerQuery ? "yes" : "no"})`);
    for (const c of r.cases) console.log(`case ${c.scene} ts ${c.ts} ${c.w}x${c.h} scale ${c.scale}: frame ${c.frameMs} ms over ${c.frames} frames, gpu ${c.gpuMs ?? "-"} ms, ${c.drawn?.draws ?? "?"} draws`);
    console.log(`sync: ${JSON.stringify(r.sync)}`);
  }
  if (errors.length) console.log(`page errors: ${errors.join(" | ")}`);
  await page.screenshot({ path: path.join(OUT, "check.png") });
  console.log(`screenshot: ${path.join(OUT, "check.png")}`);
  await browser.close();
  server.close();
  process.exitCode = out.result && !errors.length ? 0 : 1;
}
