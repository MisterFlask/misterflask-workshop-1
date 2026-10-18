// node --test: unit animation (app/r3d/anim.js, docs/ANIMATION.md). Every clip is baked from its
// recipe and must pass the validator's hard metrics (r3d-lab/anim/check.mjs) -- for the human and
// a sweep of races, whose proportions differ -- and the validator must catch every mutation of a
// good clip (r3d-lab/anim/mutate.mjs): a metric is only trusted when it fails on its defect.
import test from "node:test";
import assert from "node:assert/strict";

const R = await import("../app/r3d/assets/registry.js");
await import("../app/r3d/assets/index.js");
await import("../app/r3d/assets/anims.js");
const A = await import("../app/r3d/anim.js");
const K = await import("../../r3d-lab/anim/check.mjs");
const MU = await import("../../r3d-lab/anim/mutate.mjs");

const CLIPS = A.animatedRecipes().flatMap((recipe) => A.clipNames(recipe).map((clip) => [recipe, clip]));
/** Races whose proportions stress a clip (short, long-armed, tailed, bony). */
const SWEEP = ["dwarf", "goblin", "orc", "lizard", "demon", "undead"];
const BODY = ["ground", "footSkate", "rigid", "joints", "clearance", "balance", "continuity", "boundaries"];

test("the warrior and the archer have an idle, an attack and a death", () => {
  for (const r of ["unit.warrior", "unit.archer"]) assert.deepEqual(A.clipNames(r).sort(), ["attack", "death", "idle"], r);
});

test("a figure built with an empty pose is its rest mesh, vertex for vertex", () => {
  for (const [recipe] of CLIPS) {
    for (const param of ["human", "dwarf", "lizard"]) {
      const a = R.buildAsset(recipe, 1, param), b = R.buildPosed(recipe, 1, param, {});
      assert.equal(a.count, b.count);
      assert.deepEqual(Array.from(b.data), Array.from(a.data), `${recipe} ${param}`);
    }
  }
});

for (const [recipe, clip] of CLIPS) {
  test(`${recipe} ${clip}: every hard metric passes`, () => {
    const b = A.bakeClip(recipe, 0, "human", clip);
    const r = K.check(b);
    assert.ok(K.passes(r), `\n${K.table(r.filter((x) => x.hard && !x.pass))}`);
  });

  test(`${recipe} ${clip}: the body and motion metrics pass for other races`, () => {
    for (const param of SWEEP) {
      const b = A.bakeClip(recipe, 1, param, clip);
      const r = K.check(b, { only: BODY });
      assert.ok(K.passes(r), `${param}\n${K.table(r.filter((x) => x.hard && !x.pass))}`);
    }
  });

  test(`${recipe} ${clip}: the validator catches every mutation`, () => {
    const rows = MU.selfTest(recipe, clip);
    const bad = rows.filter((x) => !x.ok);
    assert.equal(bad.length, 0, bad.map((x) => `${x.name}: ${x.note}`).join("\n"));
    assert.ok(rows.length > 4, "mutations apply");
  });
}

test("frames are rebuilt with the rest mesh's topology, and the GPU's packing round-trips", () => {
  const b = A.bakeClip("unit.warrior", 0, "human", "attack");
  const P = A.packFrames([b]);
  assert.equal(P.count, b.count);
  const c = P.clips.attack;
  assert.equal(c.frames, b.frames);
  for (const f of [0, Math.floor(b.frames / 2), b.frames - 1]) {
    for (let v = 0; v < b.count; v += 97) {
      const o = ((c.base + f) * b.count + v) * 4, q = (f * b.count + v) * 3;
      for (let k = 0; k < 3; k++) {
        const x = P.lo[k] + (P.data[o + k] / 65535) * P.span[k];
        assert.ok(Math.abs(x - b.pos[q + k]) <= P.span[k] / 65535 + 1e-7, `position ${f}/${v}`);
      }
    }
  }
});

test("an attack turns its strike toward the target; a corpse falls across the screen, never toward its killer", () => {
  const atk = A.clipOf("unit.archer", "attack"), dth = A.clipOf("unit.warrior", "death");
  for (const h of [0, 0.7, Math.PI / 2, 2.5, Math.PI, -1.2]) {
    const yaw = A.attackYaw(atk, h);
    const own = Math.atan2(atk.strike[0], atk.strike[2]);
    assert.ok(Math.abs(Math.sin(yaw + own - h)) < 1e-9 && Math.cos(yaw + own - h) > 0, `attack heading ${h}`);
    const dy = A.deathYaw(dth, h);
    const away = Math.atan2(dth.away[0], dth.away[2]), fall = Math.atan2(dth.fall[0], dth.fall[2]);
    assert.ok(Math.cos(dy + away - h) >= -1e-9, `thrown within 90 degrees of away from the killer (${h})`);
    assert.ok(Math.abs(Math.sin(dy + fall)) > 0.99, `falls across the screen (${h})`);
  }
});

test("the animator cross-fades into a one-shot and back into the idle", () => {
  const an = new A.Animator();
  an.want("unit.warrior", 0, "human");
  while (an.queue.length) an.step(1e9);
  const it = { asset: "unit.warrior", variant: 0, param: "human" };
  const P = an.looks.get("unit.warrior#0#human").packed;
  const idle0 = an.instanceAnim(it, 7, 10, 0.3);
  assert.equal(idle0.cur[0], P.clips.idle.base);
  assert.equal(idle0.prev, null);
  an.trigger(7, "attack", 10, 1.0, it, 0.3);
  const mid = an.instanceAnim(it, 7, 10.05, 0.3);
  assert.equal(mid.cur[0], P.clips.attack.base);
  assert.ok(mid.cur[3] < 0, "a one-shot holds its last frame");
  assert.deepEqual(mid.prev, idle0.cur, "it fades from the idle it interrupted");
  const end = 10 + P.clips.attack.dur;
  const back = an.instanceAnim(it, 7, end + 0.05, 0.3);
  assert.equal(back.cur[0], P.clips.idle.base);
  assert.equal(back.cur[2], end, "the idle restarts where the attack ended");
  assert.equal(back.prev[0], P.clips.attack.base);
  assert.equal(an.instanceAnim(it, 7, end + 1, 0.3).prev, null);
});

test("the figure is continuous in every pose channel (no ring flips, IK folds or branch jumps)", () => {
  // Each channel swept over its useful range in 120 steps: no step may jump past 4x its
  // neighbours' (a vertex-correspondence pop the topology check cannot see).
  const SWEEPS = {
    pelvis: [[0, -0.1, -0.04], [0, 0.02, 0.05]], lean: [-0.5, 0.8], twist: [-0.8, 0.8], roll: [-0.4, 0.4],
    headYaw: [-1, 1], headTilt: [-0.6, 0.6], rHand: [[-0.12, -0.2, -0.12], [0.12, 0.1, 0.15]], lHand: [[-0.12, -0.15, -0.1], [0.1, 0.1, 0.12]],
    rRot: [[-1.5, -0.8, -0.8], [1.5, 0.8, 0.8]], fl: [[0, 0, 0], [0, 0.04, 0.03]], rootRot: [[-1.6, 0, -1.6], [0, 0, 1.6]],
  };
  for (const recipe of ["unit.warrior", "unit.archer"]) {
    for (const param of ["human", "orc", "goblin"]) {
      for (const [ch, [a, b]] of Object.entries(SWEEPS)) {
        const at = (u) => (Array.isArray(a) ? a.map((x, k) => x + (b[k] - x) * u) : a + (b - a) * u);
        let prev = null, steps = [];
        for (let i = 0; i <= 120; i++) {
          const m = R.buildPosed(recipe, 0, param, { [ch]: at(i / 120) });
          if (prev) {
            let d = 0;
            for (let v = 0; v < m.count; v += 3) d = Math.max(d, Math.hypot(m.data[v * 13] - prev[v * 13], m.data[v * 13 + 1] - prev[v * 13 + 1], m.data[v * 13 + 2] - prev[v * 13 + 2]));
            steps.push(d);
          }
          prev = m.data;
        }
        for (let k = 1; k + 1 < steps.length; k++) {
          const nb = Math.max(steps[k - 1], steps[k + 1], 1e-4);
          assert.ok(!(steps[k] > 0.002 && steps[k] > 4 * nb), `${recipe} ${param} ${ch}: step ${k} jumps ${steps[k].toFixed(4)} against ${nb.toFixed(4)}`);
        }
      }
    }
  }
});

test("two keys may not set one channel at one time", () => {
  assert.throws(() => A.prepareClip({ dur: 1, keys: [{ t: 0.5, pose: { lean: 0.1 } }, { t: 0.5, pose: { lean: 0.2 } }] }), /two keys/);
});
