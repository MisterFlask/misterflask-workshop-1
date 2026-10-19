#!/usr/bin/env node
// Performance of the experimental 3D renderer (docs/R3D.md "Performance") on a real game: starts
// `ffh-play serve --load SAVE` (debug reveal on, so the whole map is drawn), opens the client with
// `?renderer=3d` in headless Chromium and measures, for each zoom and pipeline configuration:
// the CPU cost of bringing the scene up to date (world, textures, dressing, instance upload,
// paint layers), the GPU time of each pass (EXT_disjoint_timer_query_webgl2), the wall time of a
// complete frame, and what was drawn (draw calls, instances, triangles).
//
//   node tools/r3d-perf.mjs --save SAVE [--x X --y Y] [--w 1280 --h 800] [--json OUT] [--runs 3]
//        [--configs painterly,plain] [--zooms 40,96] [--gpu]
//
// By default headless Chromium renders WebGL with SwiftShader (software): absolute times are those of
// a CPU rasteriser, far slower than any GPU; the relative costs of the passes are what carries over.
// --gpu measures on the machine's GPU (Chromium's full headless mode); the WebGL renderer printed
// first says which one was used.

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { ROOT, parseArgs, findBin, tempDir, startServer, chromiumLaunch, webglRenderer } from "./web-lib.mjs";

const args = parseArgs(process.argv.slice(2), { gpu: "flag", save: "v", x: "v", y: "v", w: "v", h: "v", json: "v", bin: "v", runs: "v", configs: "v", zooms: "v" });
if (!args.save) {
  console.error("r3d-perf: --save SAVE is required");
  process.exit(2);
}
const saves = tempDir("r3d-perf");
for (const ext of [".ffhsave", ".ffhsess"]) {
  const src = args.save.replace(/\.ffhsave$/, ext);
  if (fs.existsSync(src)) fs.copyFileSync(src, path.join(saves, `perf${ext}`));
}
const server = await startServer({ bin: findBin(args.bin), saves, extra: ["--load", path.join(saves, "perf.ffhsave"), "--assets", path.join(ROOT, "crates", "ffh-play", "assets")] });
const post = (body) => fetch(`${server.url}/api`, { method: "POST", headers: { "X-FFH": "1", "Content-Type": "application/json" }, body: JSON.stringify({ id: 1, ...body }) }).then((r) => r.json());
await post({ k: "session", op: "debug", reveal: true });

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
const W = Number(args.w ?? 1280), H = Number(args.h ?? 800);
const CONFIGS = [
  ["painterly", {}],
  ["no-kuwahara", { kuwahara: 0 }],
  ["no-ssao", { ssao: 0 }],
  ["no-dabs", { dabs: 0 }],
  ["no-shadows", { shadows: false }],
  ["no-outline+grade", { outline: 0, grade: 0 }],
  ["plain", "plain"],
];
const ZOOMS = args.zooms ? args.zooms.split(",").map(Number) : [40, 96, 168];
const results = [];
for (const [name, cfg] of CONFIGS.filter(([n]) => !args.configs || args.configs.split(",").includes(n))) {
  const page = await browser.newPage({ viewport: { width: W, height: H } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const q = cfg === "plain" ? "plain" : JSON.stringify({ animate: false, ...cfg });
  // force3d=1 bypasses the GPU guard (docs/R3D.md §1): this tool runs under SwiftShader on purpose.
  await page.goto(`${server.url}/?renderer=3d&r3d=${encodeURIComponent(q === "plain" ? "plain" : q)}&force3d=1`);
  await page.waitForFunction(() => window.ffh?.ready && window.ffh.S.map && window.ffh.mapView.r3d?.cam, null, { timeout: 300000 });
  for (const ts of ZOOMS) {
    const r = await page.evaluate(
      async ({ ts, x, y, runs }) => {
        const mv = window.ffh.mapView;
        const r3d = mv.r3d;
        r3d.running = false;
        cancelAnimationFrame(r3d.raf);
        r3d.cfg.animate = false;
        // The quality ladder must be off while measuring: under SwiftShader every frame is far over
        // the step-down threshold, so the warmup would drop the paint filter and render scale and the
        // per-pass costs below would be those of a degraded frame, not the full pipeline this measures.
        r3d.cfg.autoQuality = 0;
        r3d.autoLevel = 0;
        mv.cam = { ...mv.cam, ts };
        r3d.ts = ts;
        if (x !== undefined) mv.centerOn(x, y, { animate: false });
        const gl = r3d.gl;
        const px = new Uint8Array(4);
        // Warm up: chunks are built a few per frame.
        for (let k = 0; k < 6; k++) r3d.render(1);
        gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
        // A fresh sync, timed by stage.
        r3d.mapSeen = null;
        r3d.viewSeen = null;
        r3d.sync();
        const sync = { ...r3d.syncMs };
        r3d.pipeline.stats.ms = {};
        r3d.measuring = true;
        const times = [];
        for (let k = 0; k < runs; k++) {
          const t0 = performance.now();
          r3d.render(1);
          gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
          times.push(performance.now() - t0);
        }
        await new Promise((res) => setTimeout(res, 200));
        r3d.pipeline.collectTimings();
        r3d.measuring = false;
        const gpu = {};
        for (const [k, s] of Object.entries(r3d.pipeline.stats.ms)) gpu[k] = +(s.sum / Math.max(1, s.n)).toFixed(1);
        times.sort((a, b) => a - b);
        return {
          ts,
          frameMs: +times[times.length >> 1].toFixed(1),
          gpu,
          sync: Object.fromEntries(Object.entries(sync).map(([k, v]) => [k, +v.toFixed(1)])),
          drawn: r3d.scene.drawn,
          props: r3d.staticCount,
          units: r3d.unitCount,
          canvas: [r3d.canvas.width, r3d.canvas.height],
        };
      },
      { ts, x: args.x !== undefined ? Number(args.x) : undefined, y: args.y !== undefined ? Number(args.y) : undefined, runs: Number(args.runs ?? 3) },
    );
    r.config = name;
    results.push(r);
    console.log(`${name.padEnd(17)} ts ${String(ts).padStart(3)}: frame ${r.frameMs} ms; gpu ${JSON.stringify(r.gpu)}; drawn ${r.drawn.draws} draws ${r.drawn.instances} inst ${Math.round(r.drawn.tris / 1000)}k prop tris ${Math.round(r.drawn.terrainTris / 1000)}k terrain tris`);
    if (ts === ZOOMS[0] && name === "painterly") console.log(`  sync (ms): ${JSON.stringify(r.sync)}; props ${r.props}, unit figures ${r.units}`);
  }
  if (errors.length) console.log(`  page errors: ${errors.join("; ")}`);
  await page.close();
}
if (args.json) fs.writeFileSync(args.json, JSON.stringify(results, null, 1));
await browser.close();
await server.stop();
