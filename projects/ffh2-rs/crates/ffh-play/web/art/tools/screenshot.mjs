#!/usr/bin/env node
// screenshot.mjs: screenshot every preview page with headless Chromium.
//
//   node tools/screenshot.mjs <outdir> [page ...]
//
// Pages (default: all): sheet-terrain sheet-units sheet-cities sheet-ui sheet-all coverage scene
// scene-1x scene-cached (the scene through a terrain cache: should look the same as scene).
// Uses the playwright package when it resolves (local, or the global node_modules next to node),
// else runs the chrome binary under $PLAYWRIGHT_BROWSERS_PATH (or /opt/pw-browsers) with
// --headless --screenshot (fixed window size, no console check). Exits 1 if a page reports an
// error (console error, uncaught exception, or a failed art load).
import { mkdirSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { startServer } from './serve.mjs';

export const PAGES = {
  'sheet-terrain': ['preview/sheet.html?set=terrain', 1400, 900],
  'sheet-units': ['preview/sheet.html?set=units', 1400, 900],
  'sheet-cities': ['preview/sheet.html?set=cities', 1400, 900],
  'sheet-ui': ['preview/sheet.html?set=ui', 1400, 900],
  'sheet-all': ['preview/sheet.html?set=all', 1400, 900],
  coverage: ['preview/sheet.html?set=coverage', 1400, 900],
  scene: ['preview/scene.html', 1400, 1000],
  'scene-1x': ['preview/scene.html?scale=1', 760, 560],
  'scene-cached': ['preview/scene.html?cache=1', 1400, 1000],
};

function loadPlaywright() {
  const req = createRequire(import.meta.url);
  const tries = ['playwright', join(dirname(process.execPath), '../lib/node_modules/playwright'), '/opt/node22/lib/node_modules/playwright'];
  for (const t of tries) { try { return req(t); } catch { /* next */ } }
  return null;
}

function findChrome() {
  const roots = [process.env.PLAYWRIGHT_BROWSERS_PATH, '/opt/pw-browsers'].filter(Boolean);
  for (const r of roots) {
    if (!existsSync(r)) continue;
    for (const d of readdirSync(r).filter((n) => /^chromium-\d+$/.test(n)).sort().reverse()) {
      const p = join(r, d, 'chrome-linux', 'chrome');
      if (existsSync(p)) return p;
    }
  }
  return null;
}

async function main() {
  const [outArg, ...only] = process.argv.slice(2);
  if (!outArg) { console.error('usage: node tools/screenshot.mjs <outdir> [page ...]'); process.exit(2); }
  const out = resolve(outArg);
  mkdirSync(out, { recursive: true });
  const names = only.length ? only : Object.keys(PAGES);
  const server = await startServer(0);
  let failed = false;
  try {
    const pw = loadPlaywright();
    if (pw) {
      const browser = await pw.chromium.launch();
      for (const name of names) {
        const [path, w, h] = PAGES[name];
        const page = await browser.newPage({ viewport: { width: w, height: h } });
        const errors = [];
        page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
        page.on('pageerror', (e) => errors.push(String(e)));
        const t0 = Date.now();
        await page.goto(server.url + path);
        await page.waitForFunction(() => window.__artReady !== undefined, null, { timeout: 120000 });
        const status = await page.evaluate(() => window.__artReady);
        if (status !== true) errors.push(`page status: ${status}`);
        const file = join(out, `${name}.png`);
        await page.screenshot({ path: file, fullPage: true });
        console.log(`${errors.length ? 'FAIL' : 'ok  '} ${name} -> ${file} (${Date.now() - t0} ms)`);
        for (const e of errors) console.log(`     ${e}`);
        if (errors.length) failed = true;
        await page.close();
      }
      await browser.close();
    } else {
      const chrome = findChrome();
      if (!chrome) { console.error('screenshot: neither playwright nor a chromium binary found'); process.exit(2); }
      for (const name of names) {
        const [path, w, h] = PAGES[name];
        const file = join(out, `${name}.png`);
        const r = spawnSync(chrome, ['--headless', '--no-sandbox', '--disable-gpu', '--hide-scrollbars', `--window-size=${w},${h}`,
          '--virtual-time-budget=30000', `--screenshot=${file}`, server.url + path], { encoding: 'utf8' });
        console.log(`${r.status === 0 ? 'ok  ' : 'FAIL'} ${name} -> ${file}`);
        if (r.status !== 0) { failed = true; console.log(r.stderr.slice(-2000)); }
      }
    }
  } finally {
    await server.close();
  }
  process.exit(failed ? 1 : 0);
}

main();
