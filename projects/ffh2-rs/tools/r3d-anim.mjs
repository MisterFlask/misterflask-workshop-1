#!/usr/bin/env node
// The unit animation workbench (docs/ANIMATION.md): bakes clips (crates/ffh-play/assets/app/r3d/
// anim.js), runs the validator (r3d-lab/anim/check.mjs) and draws review sheets
// (r3d-lab/anim/sheet.mjs). Node only, no browser (except `lab`).
//
//   node tools/r3d-anim.mjs check [--recipe unit.warrior] [--clip attack] [--param human]
//        [--variant 0] [--races] [--json FILE]
//     every clip (or the ones named) through the validator; --races also every race and variant
//     (hard metrics only). Exit 1 when a hard metric fails.
//   node tools/r3d-anim.mjs sheet --out DIR [--recipe ..] [--clip ..] [--param ..] [--yaw 0.25]
//        [--blind]
//     a review sheet per clip (DIR/<recipe>-<clip>-<param>.png).
//   node tools/r3d-anim.mjs mutants [--recipe ..] [--clip ..] [--out DIR]
//     the mutation self-test: each degradation (r3d-lab/anim/mutate.mjs) must trip the metric
//     it targets while the good clip passes; with --out, a sheet of each mutant.
//   node tools/r3d-anim.mjs judge-pack --out DIR [--recipe ..] [--clip ..] [--before FILE,..]
//     blind stimuli for vision judges (docs/ANIMATION.md "Vision judges"): DIR/stimuli/*.png under
//     random names, DIR/questions.json (what each judge is shown and asked; nothing names a clip,
//     a mutation or a verdict) and DIR/key.json (the answers, never shown to a judge). --before
//     names clip modules (an earlier anim-*.js) whose clips are paired against the current ones.
//   node tools/r3d-anim.mjs lab --out DIR --recipe unit.warrior --clip attack [--param human]
//        [--times 0,0.43,0.53] [--ts 200] [--gpu]
//     key frames with the real renderer (tools/r3d-lab.mjs, headless Chromium).

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { ROOT, parseArgs } from "./web-lib.mjs";

const PLAY = path.join(ROOT, "crates", "ffh-play");
const args = parseArgs(process.argv.slice(3), { recipe: "v", clip: "v", param: "v", variant: "v", races: "flag", json: "v", out: "v", yaw: "v", blind: "flag", times: "v", ts: "v", gpu: "flag", before: "v" });
const cmd = process.argv[2];

await import(path.join(PLAY, "assets/app/r3d/assets/index.js"));
await import(path.join(PLAY, "assets/app/r3d/assets/anims.js"));
const A = await import(path.join(PLAY, "assets/app/r3d/anim.js"));
const K = await import(path.join(PLAY, "r3d-lab/anim/check.mjs"));

/** Races a clip is swept over with --races: every humanoid race (and a dark one). */
export const RACES = ["human", "elf", "dwarf", "orc", "goblin", "lizard", "undead", "demon", "human-dark"];

function jobs() {
  const out = [];
  for (const recipe of A.animatedRecipes()) {
    if (args.recipe && args.recipe !== recipe) continue;
    for (const clip of A.clipNames(recipe)) {
      if (args.clip && args.clip !== clip) continue;
      out.push({ recipe, clip, param: args.param ?? "human", variant: Number(args.variant ?? 0) });
    }
  }
  return out;
}

if (cmd === "check") {
  let failed = 0;
  const report = [];
  for (const j of jobs()) {
    const b = A.bakeClip(j.recipe, j.variant, j.param, j.clip);
    const r = K.check(b);
    const ok = K.passes(r);
    if (!ok) failed++;
    report.push({ ...j, pass: ok, results: r });
    console.log(`\n${ok ? "PASS" : "FAIL"} ${j.recipe} ${j.clip} ${j.param} v${j.variant}`);
    console.log(K.table(r.filter((x) => !x.pass || !x.hard)));
    if (args.races) {
      for (const param of RACES) {
        for (let v = 0; v < 4; v++) {
          let b2;
          try {
            b2 = A.bakeClip(j.recipe, v, param, j.clip);
          } catch (e) {
            console.log(`  FAIL ${param} v${v}: ${e.message}`);
            failed++;
            continue;
          }
          const r2 = K.check(b2, { only: ["ground", "footSkate", "rigid", "joints", "clearance", "balance", "continuity", "boundaries"] });
          const bad = r2.filter((x) => x.hard && !x.pass);
          if (bad.length) failed++;
          report.push({ ...j, param, variant: v, pass: !bad.length, results: r2 });
          console.log(`  ${bad.length ? "FAIL" : "ok  "} ${param} v${v}${bad.map((x) => `  ${x.id} ${x.value} (limit ${x.limit})`).join("")}`);
        }
      }
    }
  }
  if (args.json) fs.writeFileSync(args.json, JSON.stringify(report, null, 1));
  console.log(`\n${failed ? `${failed} failing` : "all pass"}`);
  process.exit(failed ? 1 : 0);
}

if (cmd === "sheet") {
  const { sheet } = await import(path.join(PLAY, "r3d-lab/anim/sheet.mjs"));
  const dir = args.out ?? ".";
  fs.mkdirSync(dir, { recursive: true });
  for (const j of jobs()) {
    const b = A.bakeClip(j.recipe, j.variant, j.param, j.clip);
    const im = sheet(b, { yaw: Number(args.yaw ?? 0.25), blind: !!args.blind });
    const file = path.join(dir, `${j.recipe}-${j.clip}-${j.param}.png`);
    fs.writeFileSync(file, im.png());
    console.log(file);
  }
  process.exit(0);
}

if (cmd === "mutants") {
  const M = await import(path.join(PLAY, "r3d-lab/anim/mutate.mjs"));
  const { sheet } = await import(path.join(PLAY, "r3d-lab/anim/sheet.mjs"));
  let bad = 0;
  for (const j of jobs()) {
    const res = M.selfTest(j.recipe, j.clip, { param: j.param });
    for (const r of res) {
      if (!r.ok) bad++;
      console.log(`${r.ok ? "ok  " : "FAIL"} ${j.recipe} ${j.clip} ${r.name.padEnd(22)} ${r.note}`);
      if (args.out && r.bake) {
        fs.mkdirSync(args.out, { recursive: true });
        fs.writeFileSync(path.join(args.out, `${j.recipe}-${j.clip}-${r.name}.png`), sheet(r.bake, {}).png());
      }
    }
  }
  console.log(bad ? `${bad} mutation checks failed` : "every mutant is caught and every original passes");
  process.exit(bad ? 1 : 0);
}

if (cmd === "judge-pack") {
  const crypto = await import("node:crypto");
  const { sheet } = await import(path.join(PLAY, "r3d-lab/anim/sheet.mjs"));
  const M = await import(path.join(PLAY, "r3d-lab/anim/mutate.mjs"));
  const { camera } = await import(path.join(PLAY, "r3d-lab/anim/raster.mjs"));
  const dir = args.out;
  if (!dir) throw new Error("judge-pack needs --out");
  fs.mkdirSync(path.join(dir, "stimuli"), { recursive: true });
  // Deterministic shuffles and tokens (a fixed seed: the pack is reproducible).
  let seed = 0x9e3779b9;
  const rand = () => ((seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) >>> 0) / 2 ** 32);
  const token = () => Array.from({ length: 3 }, () => "BCDFGHJKLMNPQRSTVWXZ"[Math.floor(rand() * 20)]).join("");
  const save = (im) => {
    const name = crypto.createHash("sha1").update(String(rand())).digest("hex").slice(0, 10) + ".png";
    fs.writeFileSync(path.join(dir, "stimuli", name), im.png());
    return path.join(dir, "stimuli", name);
  };
  const side = (v, cam) => {
    const [dx, dy] = cam.screenDir(v);
    return Math.abs(dx) >= Math.abs(dy) ? (dx > 0 ? "right" : "left") : dy > 0 ? "down (toward the viewer)" : "up (away from the viewer)";
  };
  // The 2AFC mutants per kind: gross versions of the commonest ways animation goes wrong.
  const PAIRS = { idle: ["statue", "swim", "lockstep"], attack: ["noAnticipation", "linearTiming", "timid", "easedImpact"], death: ["floatyFall", "sitUp", "twitch"] };
  const PAIRS_BOW = { attack: ["softRelease", "noRecoil", "lingeringArrow"] };
  const questions = { recognize: [], pairs: [] }, key = { recognize: {}, pairs: {} };
  for (const j of jobs()) {
    const b = A.bakeClip(j.recipe, j.variant, j.param, j.clip);
    const def = b.clip;
    const isBow = (b.parts.weapon_l ?? b.parts.weapon_r)?.kind === "bow";
    // T1: blind recognition at the default yaw and at a facing the game gives a fight.
    const views = [["default", 0.25]];
    if (def.kind === "attack") views.push(["east", A.attackYaw(def, Math.PI / 2)]);
    if (def.kind === "death") views.push(["west attacker", A.deathYaw(def, Math.PI / 2)]);
    if (def.kind === "idle") views.push(["turned", 1.2]);
    for (const [vname, yaw] of views) {
      const im = sheet(b, { yaw, blind: true });
      const file = save(im);
      const id = token() + token();
      const cam = camera({ ts: 96, yaw });
      const ev = def.events?.impact ?? def.events?.release ?? def.events?.ground;
      const cell = ev === undefined ? null : im.frames.findIndex((f) => f >= Math.round(ev * b.fps)) + 1;
      questions.recognize.push({ id, files: [file] });
      key.recognize[id] = {
        recipe: j.recipe, clip: j.clip, view: vname,
        action: def.kind === "idle" ? "standing idle" : def.kind === "death" ? "dying" : isBow ? "shooting a bow" : "melee attack",
        direction: def.kind === "attack" ? side(def.strike ?? [0, 0, 1], cam) : def.kind === "death" ? side(def.fall ?? def.away ?? [0, 0, -1], cam) : null,
        final: def.kind === "death" ? "lying" : "standing",
        moment: cell,
        cells: im.frames.length,
      };
    }
    // T2: the good clip against each gross mutant, both orders (separate questions).
    const names = (isBow ? PAIRS_BOW[def.kind] : null) ?? PAIRS[def.kind] ?? [];
    const good = save(sheet(b, { blind: true }));
    for (const mn of names) {
      const mu = M.MUTATIONS.find((x) => x.name === mn);
      if (!mu) continue;
      const mb = M.bakeMutant(j.recipe, j.clip, mu, { param: j.param, variant: j.variant });
      const bad = save(sheet(mb, { blind: true }));
      for (const order of [0, 1]) {
        const id = token() + token();
        const [ta, tb] = [token(), token()];
        const first = order === 0 ? good : bad, second = order === 0 ? bad : good;
        questions.pairs.push({ id, group: `${j.recipe}/${j.clip}/${mn}`, labels: [ta, tb], files: [first, second] });
        key.pairs[id] = { recipe: j.recipe, clip: j.clip, mutant: mn, good: order === 0 ? ta : tb, order };
      }
    }
  }
  // T2 against an earlier version: each --before module's clips (registered over the current
  // ones once these are baked) against the current clip, both orders. The current one counts as
  // the better: the pair asks whether the revisions read as better.
  const pair = (j, label, goodFile, badFile) => {
    for (const order of [0, 1]) {
      const id = token() + token();
      const [ta, tb] = [token(), token()];
      questions.pairs.push({ id, group: `${j.recipe}/${j.clip}/${label}`, labels: [ta, tb], files: order === 0 ? [goodFile, badFile] : [badFile, goodFile] });
      key.pairs[id] = { recipe: j.recipe, clip: j.clip, mutant: label, good: order === 0 ? ta : tb, order };
    }
  };
  if (args.before) {
    const current = new Map();
    for (const j of jobs()) current.set(`${j.recipe}/${j.clip}`, { j, file: save(sheet(A.bakeClip(j.recipe, j.variant, j.param, j.clip), { blind: true })) });
    for (const [i, file] of args.before.split(",").entries()) {
      // The module imports "../anim.js": a copy beside the pack imports the engine by its path.
      const anim = new URL(`file://${path.join(PLAY, "assets/app/r3d/anim.js")}`).href;
      const src = fs.readFileSync(file, "utf8").replace(/from "\.\.\/anim\.js"/g, `from "${anim}"`);
      const copy = path.join(dir, `before-${i}.mjs`);
      fs.writeFileSync(copy, src);
      const was = new Map([...current.keys()].map((k) => [k, A.clipOf(...k.split("/"))]));
      await import(`file://${copy}`);
      for (const [k, { j }] of current) {
        const [r, c] = k.split("/");
        if (A.clipOf(r, c) === was.get(k)) continue; // not redefined by this module
        pair(j, `before${i}`, current.get(k).file, save(sheet(A.bakeClip(r, j.variant, j.param, c), { blind: true })));
      }
    }
  }
  fs.writeFileSync(path.join(dir, "questions.json"), JSON.stringify(questions, null, 1));
  fs.writeFileSync(path.join(dir, "key.json"), JSON.stringify(key, null, 1));
  console.log(`${questions.recognize.length} recognition items, ${questions.pairs.length} pair items in ${dir}`);
  process.exit(0);
}

if (cmd === "lab") {
  if (!args.recipe || !args.clip || !args.out) throw new Error("lab needs --recipe, --clip and --out");
  fs.mkdirSync(args.out, { recursive: true });
  const def = A.clipOf(args.recipe, args.clip);
  const times = (args.times ?? [0, ...Object.values(def.events ?? {}), def.dur].join(",")).split(",").map(Number);
  const param = args.param ?? "human";
  const shots = times.map((t) => ({
    out: path.join(args.out, `${args.recipe}-${args.clip}-${param}-${t.toFixed(2)}.png`),
    query: `grid=${args.recipe}&variants=1&params=${param}&anim=${args.clip}&ts=${args.ts ?? 200}&cx=1.5&cy=2.2&yaw=${args.yaw ?? 0.25}&r3d=${encodeURIComponent('{"animate":false}')}`,
    w: 360,
    h: 300,
    time: t,
  }));
  const file = path.join(args.out, "shots.json");
  fs.writeFileSync(file, JSON.stringify(shots));
  const r = spawnSync(process.execPath, [path.join(ROOT, "tools/r3d-lab.mjs"), "--shots", file, ...(args.gpu ? ["--gpu"] : [])], { stdio: "inherit" });
  process.exit(r.status ?? 1);
}

console.log("usage: node tools/r3d-anim.mjs check|sheet|mutants|lab [flags] (see the header)");
process.exit(2);
