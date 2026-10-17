// Shared by the web smoke tests (tools/web-api-smoke.mjs, tools/web-smoke.mjs): finding the
// `ffh-play` binary, starting `ffh-play serve` on a free port with its own saves directory, and
// command-line flags.

import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** projects/ffh2-rs. */
export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** `--name value` and `--flag` arguments: `{name: value | true}`. */
export function parseArgs(argv, known) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith("--")) throw new Error(`unexpected argument ${a}`);
    const name = a.slice(2);
    if (!(name in known)) throw new Error(`unknown flag ${a} (known: ${Object.keys(known).map((k) => `--${k}`).join(" ")})`);
    if (known[name] === "flag") out[name] = true;
    else {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} needs a value`);
      out[name] = v;
    }
  }
  return out;
}

/** The ffh-play binary: `--bin`, $FFH_PLAY, or target/release/ffh-play. */
export function findBin(given) {
  const bin = given ?? process.env.FFH_PLAY ?? path.join(ROOT, "target", "release", process.platform === "win32" ? "ffh-play.exe" : "ffh-play");
  if (!fs.existsSync(bin)) throw new Error(`no ffh-play binary at ${bin}: run \`cargo build --release -p ffh-play\` (or give --bin)`);
  return bin;
}

/** A fresh temporary directory. */
export function tempDir(name) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `ffh-${name}-`));
}

/** Starts `ffh-play serve --port 0 --saves DIR [extra]` and resolves when it serves: `{url,
 *  saves, proc, stop(), stderr()}`. The server's output is kept (the panic message of a crash
 *  test goes to stderr). */
export function startServer({ bin, saves, extra = [], timeoutMs = 30000 }) {
  return new Promise((resolve, reject) => {
    const proc = spawn(bin, ["serve", "--port", "0", "--saves", saves, ...extra], { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let err = "";
    const timer = setTimeout(() => {
      proc.kill("SIGKILL");
      reject(new Error(`ffh-play serve did not start within ${timeoutMs} ms:\n${out}${err}`));
    }, timeoutMs);
    proc.stdout.on("data", (d) => {
      out += d;
      const m = /serving (http:\/\/[^/\s]+)\//.exec(out);
      if (m) {
        clearTimeout(timer);
        resolve({
          url: m[1],
          saves,
          proc,
          stderr: () => err,
          /** The exit code once it exits by itself (a `quit` request), or "timeout". */
          exited: (ms = 10000) =>
            new Promise((res) => {
              if (proc.exitCode !== null) return res(proc.exitCode);
              const t = setTimeout(() => res("timeout"), ms);
              proc.once("exit", (code) => (clearTimeout(t), res(code)));
            }),
          stop: () =>
            new Promise((res) => {
              if (proc.exitCode !== null) return res(proc.exitCode);
              proc.once("exit", (code) => res(code));
              proc.kill("SIGTERM");
            }),
        });
      }
    });
    proc.stderr.on("data", (d) => (err += d));
    proc.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`ffh-play serve exited (${code}) before serving:\n${out}${err}`));
    });
  });
}

/** Waits until `f()` is truthy (polling every `every` ms) or throws `what` after `ms`. */
export async function until(f, what, ms = 30000, every = 50) {
  const t0 = Date.now();
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out (${ms} ms) waiting for ${what}`);
    await new Promise((r) => setTimeout(r, every));
  }
}

/** A seeded pseudo-random generator (mulberry32), so a smoke run is repeatable. */
export function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Runs `ffh-play stage KIND DIR` (a save at a staged situation) and returns its JSON line. */
export function stage(bin, kind, dir) {
  return new Promise((resolve, reject) => {
    const proc = spawn(bin, ["stage", kind, dir], { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let err = "";
    proc.stdout.on("data", (d) => (out += d));
    proc.stderr.on("data", (d) => (err += d));
    proc.on("exit", (code) => {
      if (code !== 0) return reject(new Error(`ffh-play stage ${kind} failed (${code}): ${err}`));
      try {
        resolve(JSON.parse(out.trim().split("\n").pop()));
      } catch (e) {
        reject(new Error(`ffh-play stage ${kind}: ${e.message}: ${out}`));
      }
    });
  });
}

/** Launch options for headless Chromium drawing WebGL. By default software rendering (SwiftShader:
 *  the same on every machine, no GPU needed, but slow: a 3D frame takes seconds); with `gpu` the
 *  machine's GPU, through Chromium's full headless mode and its own GL backend. Whether a GPU was
 *  really used shows in `webglRenderer` (a "SwiftShader" renderer means it was not). */
export function chromiumLaunch(gpu = false) {
  if (gpu) return { headless: true, channel: "chromium", args: ["--ignore-gpu-blocklist", "--enable-gpu"] };
  return { headless: true, args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] };
}

/** The name of the WebGL renderer a page draws with (a GPU's name, or SwiftShader). */
export function webglRenderer(page) {
  return page.evaluate(() => {
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) return "no WebGL2";
    const x = gl.getExtension("WEBGL_debug_renderer_info");
    return String(x ? gl.getParameter(x.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
  });
}
