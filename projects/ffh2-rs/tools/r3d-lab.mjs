#!/usr/bin/env node
// The 3D lab's server and camera (docs/R3D.md "Lab"): serves crates/ffh-play/assets/app and
// crates/ffh-play/r3d-lab over HTTP, opens the lab in headless Chromium and saves screenshots.
// No game server is needed. Rendering is synchronous before each shot (the frame is complete).
//
//   node tools/r3d-lab.mjs --out shot.png [--query "scene=showcase&ts=88"] [--w 1280 --h 800]
//   node tools/r3d-lab.mjs --out a.png --query "grid=tree.oak,tree.pine&variants=6&ts=144"
//   node tools/r3d-lab.mjs --shots shots.json      many shots: [{"out": "a.png", "query": "..."}]
//   node tools/r3d-lab.mjs --serve [--port 8650]   just serve (open /lab/lab.html in a browser)
//   node tools/r3d-lab.mjs --stability [--query "scene=showcase&ts=96"] [--shift 4]
//        [--configs 'plain;{"animate":false};{"animate":false,"kuwahara":0}']
//     temporal stability under panning: renders the frame, pans the camera by `shift` pixels and
//     renders again; a stable image is the first one shifted (up to parallax). Prints, per
//     configuration, the mean difference (0-255) at the best shift and the share of pixels off by
//     more than 24 -- shimmer from screen-space effects shows as a configuration scoring worse
//     than the plain render.
//
// Software rendering (SwiftShader, the default: no GPU needed) is slow: a 1280x800 frame takes
// tens of seconds. --gpu draws with the machine's GPU instead (Chromium's full headless mode); the
// tool prints the WebGL renderer it got, so a fallback to software shows.

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { ROOT, parseArgs, chromiumLaunch, webglRenderer } from "./web-lib.mjs";

const args = parseArgs(process.argv.slice(2), { gpu: "flag", out: "v", query: "v", w: "v", h: "v", shots: "v", serve: "flag", port: "v", timeout: "v", time: "v", stability: "flag", shift: "v", configs: "v" });
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
  const browser = await pw.chromium.launch(chromiumLaunch(!!args.gpu));
  {
    // Which WebGL renderer draws (software SwiftShader unless --gpu found a GPU).
    const p = await browser.newPage();
    console.log(`WebGL renderer: ${await webglRenderer(p)}`);
    await p.close();
  }
  if (args.stability) {
    await stability(browser);
    await browser.close();
    server.close();
    process.exit(0);
  }
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

async function stability(browser) {
  const shift = Number(args.shift ?? 4);
  const q = args.query ?? "scene=showcase&ts=96";
  const configs = (args.configs ?? 'plain;{"animate":false};{"animate":false,"kuwahara":0}').split(";");
  for (const cfg of configs) {
    const page = await browser.newPage({ viewport: { width: Number(args.w ?? 900), height: Number(args.h ?? 600) } });
    await page.goto(`${base}/lab/lab.html?${q}&r3d=${encodeURIComponent(cfg)}`);
    await page.waitForFunction(() => window.lab?.ready || window.lab?.error, null, { timeout: Number(args.timeout ?? 120000) });
    const out = await page.evaluate((shift) => {
      const r = window.lab.r3d, v = window.lab.view, gl = r.gl;
      r.running = false;
      cancelAnimationFrame(r.raf);
      const W = gl.drawingBufferWidth, H = gl.drawingBufferHeight;
      const grab = () => {
        r.render(1);
        const px = new Uint8Array(W * H * 4);
        gl.readPixels(0, 0, W, H, gl.RGBA, gl.UNSIGNED_BYTE, px);
        return px;
      };
      grab();
      const a = grab();
      // The camera moves east by `shift` pixels' worth of map: the picture moves west.
      v.cam.cx += shift / v.cam.ts;
      grab();
      const b = grab();
      const m = 40;
      let best = null;
      for (let sy = -2; sy <= 2; sy++) {
        for (let sx = shift - 3; sx <= shift + 3; sx++) {
          let sum = 0, off = 0, n = 0;
          for (let y = m; y < H - m; y += 1) {
            for (let x = m; x < W - m - sx; x += 1) {
              const ia = ((y + sy) * W + x + sx) * 4, ib = (y * W + x) * 4;
              const d0 = Math.abs(a[ia] - b[ib]), d1 = Math.abs(a[ia + 1] - b[ib + 1]), d2 = Math.abs(a[ia + 2] - b[ib + 2]);
              sum += d0 + d1 + d2;
              if (Math.max(d0, d1, d2) > 24) off++;
              n++;
            }
          }
          const d = sum / (n * 3);
          if (!best || d < best.d) best = { d, sx, sy, off: off / n };
        }
      }
      return best;
    }, shift);
    console.log(`${cfg.padEnd(44)} best shift (${out.sx}, ${out.sy})  mean |diff| ${out.d.toFixed(2)}  off by >24: ${(out.off * 100).toFixed(2)}%`);
    await page.close();
  }
}
