#!/usr/bin/env node
// Side-by-side captures of one real game scene in the map's renderers (docs/R3D.md
// "Comparisons"): starts `ffh-play serve --load SAVE` (debug reveal on unless --fog), centres every
// renderer on the same plot and saves `art.png` (the 2D pixel art), `iso.png` (the isometric
// 2.5D), `3d-plain.png` (the 3D renderer with every stylisation off: an ordinary real-time render
// of the same geometry), `3d-painterly.png` (the art-directed pipeline) and the painterly 3D at
// the close and far zooms (`3d-close.png`, `3d-far.png`).
//
//   node tools/r3d-compare.mjs --save SAVE --x X --y Y [--out DIR] [--w 1440 --h 900] [--fog]
//        [--only 3d-painterly,3d-far] [--cfg '{"clouds":0}'] (merged into the painterly shots)
//        [--gpu] (draw with the machine's GPU instead of software SwiftShader)
//
// The page's side and bottom panels stay (this is the game's screen); the bottom panel is folded.
// The 3D frames are rendered synchronously before each capture (software WebGL is slow).

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { ROOT, parseArgs, findBin, tempDir, startServer, chromiumLaunch, webglRenderer } from "./web-lib.mjs";

const args = parseArgs(process.argv.slice(2), { gpu: "flag", save: "v", x: "v", y: "v", out: "v", w: "v", h: "v", fog: "flag", bin: "v", only: "v", cfg: "v" });
if (!args.save || args.x === undefined || args.y === undefined) {
  console.error("r3d-compare: --save SAVE --x X --y Y are required");
  process.exit(2);
}
const OUT = path.resolve(args.out ?? path.join(ROOT, "target", "r3d-compare"));
fs.mkdirSync(OUT, { recursive: true });
const X = Number(args.x), Y = Number(args.y);
const saves = tempDir("r3d-compare");
for (const ext of [".ffhsave", ".ffhsess"]) {
  const src = args.save.replace(/\.ffhsave$/, ext);
  if (fs.existsSync(src)) fs.copyFileSync(src, path.join(saves, `scene${ext}`));
}
const server = await startServer({ bin: findBin(args.bin), saves, extra: ["--load", path.join(saves, "scene.ffhsave"), "--assets", path.join(ROOT, "crates", "ffh-play", "assets")] });
if (!args.fog) {
  await fetch(`${server.url}/api`, { method: "POST", headers: { "X-FFH": "1", "Content-Type": "application/json" }, body: JSON.stringify({ id: 1, k: "session", op: "debug", reveal: true }) });
}
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
const W = Number(args.w ?? 1440), H = Number(args.h ?? 900);
const enc = (o) => encodeURIComponent(typeof o === "string" ? o : JSON.stringify(o));
const SHOTS = [
  // The 2D renderers at the 3D's gameplay zoom (96 px a plot): the same piece of the map in each.
  { name: "art", renderer: "art", ts: 96 },
  { name: "iso", renderer: "iso", ts: 96 },
  { name: "3d-plain", renderer: "3d", ts: 96, cfg: "plain" },
  { name: "3d-painterly", renderer: "3d", ts: 96, cfg: {} },
  { name: "3d-close", renderer: "3d", ts: 168, cfg: {} },
  { name: "3d-far", renderer: "3d", ts: 40, cfg: {} },
];
const only = args.only ? new Set(args.only.split(",")) : null;
for (const s of SHOTS) {
  if (only && !only.has(s.name)) continue;
  const page = await browser.newPage({ viewport: { width: W, height: H } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  const cfg = s.cfg === "plain" ? "plain" : s.cfg ? { animate: false, ...s.cfg, ...JSON.parse(args.cfg ?? "{}") } : null;
  await page.goto(`${server.url}/?renderer=${s.renderer}${cfg ? `&r3d=${enc(cfg)}` : ""}`);
  await page.waitForFunction(() => window.ffh?.ready && window.ffh.S.map, null, { timeout: 300000 });
  await page.evaluate(() => {
    document.body.classList.add("bottom-closed");
    for (const m of document.querySelectorAll("#modals > *")) m.remove();
  });
  await page.waitForTimeout(500);
  if (s.renderer === "3d") await page.waitForFunction(() => window.ffh.mapView.r3d?.cam, null, { timeout: 300000 });
  await page.evaluate(([x, y, ts]) => {
    const mv = window.ffh.mapView;
    mv.cam = { ...mv.cam, ts };
    if (mv.r3d) mv.r3d.ts = ts;
    mv.centerOn(x, y, { animate: false });
    mv.resize();
  }, [X, Y, s.ts]);
  await page.waitForTimeout(s.renderer === "3d" ? 300 : 2500);
  // The client may have moved the camera meanwhile (it centres on the next unit waiting for
  // orders): centre again just before the capture.
  await page.evaluate(([x, y]) => window.ffh.mapView.centerOn(x, y, { animate: false }), [X, Y]);
  if (s.renderer === "art") await page.waitForFunction(() => window.ffh.mapView.art?.settled?.() ?? true, null, { timeout: 120000 }).catch(() => {});
  if (s.renderer !== "3d") await page.waitForTimeout(1500);
  if (s.renderer === "3d") {
    await page.evaluate(() => {
      const r = window.ffh.mapView.r3d;
      r.running = false;
      cancelAnimationFrame(r.raf);
      // Full quality for the comparison shots, never the degraded ladder levels.
      r.cfg.autoQuality = 0;
      r.autoLevel = 0;
      const px = new Uint8Array(4);
      for (let k = 0; k < 3; k++) {
        r.render(1.5);
        r.gl.readPixels(0, 0, 1, 1, r.gl.RGBA, r.gl.UNSIGNED_BYTE, px);
      }
    });
  }
  const file = path.join(OUT, `${s.name}.png`);
  await page.screenshot({ path: file, timeout: 600000 });
  console.log(`${file}${errors.length ? `  (page errors: ${errors.join("; ").slice(0, 300)})` : ""}`);
  await page.close();
}
await browser.close();
await server.stop();
