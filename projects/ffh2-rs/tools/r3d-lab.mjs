#!/usr/bin/env node
// The 3D lab's server and camera (docs/R3D.md "Lab"): serves crates/ffh-play/assets/app and
// crates/ffh-play/r3d-lab over HTTP, opens the lab in headless Chromium and saves screenshots.
// No game server is needed. Rendering is synchronous before each shot (the frame is complete).
//
//   node tools/r3d-lab.mjs --out shot.png [--query "scene=showcase&ts=88"] [--w 1280 --h 800]
//   node tools/r3d-lab.mjs --out a.png --query "grid=tree.oak,tree.pine&variants=6&ts=144"
//   node tools/r3d-lab.mjs --shots shots.json      many shots: [{"out": "a.png", "query": "..."}]
//   node tools/r3d-lab.mjs --serve [--port 8650]   just serve (open /lab/lab.html in a browser)
//
// Software rendering (SwiftShader) is slow: a 1280x800 frame takes tens of seconds.

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { ROOT, parseArgs } from "./web-lib.mjs";

const args = parseArgs(process.argv.slice(2), { out: "v", query: "v", w: "v", h: "v", shots: "v", serve: "flag", port: "v", timeout: "v", time: "v" });
const APP = path.join(ROOT, "crates", "ffh-play", "assets", "app");
const LAB = path.join(ROOT, "crates", "ffh-play", "r3d-lab");
const TYPES = { ".js": "text/javascript", ".mjs": "text/javascript", ".html": "text/html", ".json": "application/json", ".png": "image/png", ".css": "text/css" };

const server = http.createServer((req, res) => {
  const u = new URL(req.url, "http://x");
  let file = null;
  if (u.pathname.startsWith("/app/")) file = path.join(APP, u.pathname.slice(5));
  else if (u.pathname.startsWith("/lab/")) file = path.join(LAB, u.pathname.slice(5));
  else if (u.pathname === "/") file = path.join(LAB, "lab.html");
  if (!file || !(file.startsWith(APP) || file.startsWith(LAB)) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404);
    res.end("not found");
    return;
  }
  res.writeHead(200, { "Content-Type": TYPES[path.extname(file)] ?? "application/octet-stream", "Cache-Control": "no-store" });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(Number(args.port ?? 0), "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;
if (args.serve) {
  console.log(`r3d lab: ${base}/lab/lab.html`);
} else {
  const require = createRequire(import.meta.url);
  let pw;
  try {
    pw = require("playwright");
  } catch {
    pw = require(require.resolve("playwright", { paths: ["/opt/node22/lib/node_modules"] }));
  }
  const browser = await pw.chromium.launch({ headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
  const shots = args.shots ? JSON.parse(fs.readFileSync(args.shots, "utf8")) : [{ out: args.out, query: args.query ?? "", w: args.w, h: args.h }];
  let failed = 0;
  for (const s of shots) {
    const page = await browser.newPage({ viewport: { width: Number(s.w ?? args.w ?? 1280), height: Number(s.h ?? args.h ?? 800) } });
    const logs = [];
    page.on("console", (m) => (m.type() === "error" || m.type() === "warning") && logs.push(`[${m.type()}] ${m.text()}`));
    page.on("pageerror", (e) => logs.push(`[pageerror] ${e.message}`));
    const q = s.query ?? "";
    await page.goto(`${base}/lab/lab.html?${q}${q.includes("r3d=") ? "" : "&r3d=" + encodeURIComponent('{"animate":false}')}`);
    try {
      await page.waitForFunction(() => window.lab?.ready || window.lab?.error, null, { timeout: Number(args.timeout ?? 120000) });
      const err = await page.evaluate(() => window.lab.error);
      if (err) throw new Error(err);
      const t = Number(s.time ?? args.time ?? 1);
      const ms = await page.evaluate(async (time) => {
        const r3d = window.lab.r3d;
        r3d.running = false;
        cancelAnimationFrame(r3d.raf);
        const px = new Uint8Array(4);
        const t0 = performance.now();
        for (let k = 0; k < 2; k++) {
          r3d.render(time);
          r3d.gl.readPixels(0, 0, 1, 1, r3d.gl.RGBA, r3d.gl.UNSIGNED_BYTE, px);
        }
        return performance.now() - t0;
      }, t);
      await page.screenshot({ path: s.out, timeout: 300000 });
      console.log(`${s.out}: ${(ms / 2).toFixed(0)} ms/frame`);
    } catch (e) {
      failed++;
      console.log(`${s.out}: FAILED ${e.message}`);
    }
    for (const l of logs) console.log(`  ${l}`);
    await page.close();
  }
  await browser.close();
  server.close();
  process.exit(failed ? 1 : 0);
}
