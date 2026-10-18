// node --test: the 3D renderer's ground materials (app/r3d/materials.js, docs/R3D.md §4): painted by
// code, the same every time, and every tile meets itself at its edges (the shader repeats it over
// the map, so a seam inside a tile would show on every plot); crop rows lie where the shader's and
// the field props' rows are.
import test from "node:test";
import assert from "node:assert/strict";

const { bakeMaterials, MAT, GRAIN_ROWS, BED_ROWS } = await import("../app/r3d/materials.js");

const baked = bakeMaterials();
const N = baked.size;
const at = (l, i, j, c) => baked.data[((l * N + (j & (N - 1))) * N + (i & (N - 1))) * 4 + c];

test("the materials are a pure function of the code", () => {
  const again = bakeMaterials();
  assert.equal(again.layers, baked.layers);
  assert.ok(Buffer.from(again.data).equals(Buffer.from(baked.data)), "two bakes differ");
  assert.equal(baked.data.length, N * N * 4 * baked.layers);
  assert.equal(Object.keys(MAT).length, baked.layers);
});

test("every tile wraps: across its edges it changes no more than between neighbouring texels", () => {
  for (let l = 0; l < baked.layers; l++) {
    for (let c = 0; c < 4; c++) {
      // Mean step between neighbouring columns (and rows) inside the tile, and across its edges.
      let innerX = 0, innerY = 0, edgeX = 0, edgeY = 0;
      for (let j = 0; j < N; j++) {
        for (let i = 1; i < N; i++) {
          innerX += Math.abs(at(l, i, j, c) - at(l, i - 1, j, c));
          innerY += Math.abs(at(l, j, i, c) - at(l, j, i - 1, c));
        }
        edgeX += Math.abs(at(l, 0, j, c) - at(l, N - 1, j, c));
        edgeY += Math.abs(at(l, j, 0, c) - at(l, j, N - 1, c));
      }
      innerX /= N * (N - 1);
      innerY /= N * (N - 1);
      edgeX /= N;
      edgeY /= N;
      assert.ok(edgeX <= innerX * 1.6 + 1, `layer ${l} channel ${c}: east-west edge ${edgeX.toFixed(2)} vs ${innerX.toFixed(2)}`);
      assert.ok(edgeY <= innerY * 1.6 + 1, `layer ${l} channel ${c}: north-south edge ${edgeY.toFixed(2)} vs ${innerY.toFixed(2)}`);
    }
  }
});

test("crop rows stand where the shader's rows are (centred on each row's middle)", () => {
  // Grain rows run north-south: the height is highest along u = (k + 0.5) / GRAIN_ROWS.
  const prof = (l, along) => {
    const p = new Float64Array(N);
    for (let a = 0; a < N; a++) for (let b = 0; b < N; b++) p[a] += along ? at(l, b, a, 3) : at(l, a, b, 3);
    return p;
  };
  const check = (l, rows, along) => {
    const p = prof(l, along);
    let mid = 0, gap = 0;
    for (let k = 0; k < rows; k++) {
      mid += p[Math.floor(((k + 0.5) * N) / rows)];
      gap += p[Math.floor((k * N) / rows) % N];
    }
    assert.ok(mid > gap * 1.2, `layer ${l}: rows ${mid.toFixed(0)} vs gaps ${gap.toFixed(0)}`);
  };
  check(MAT.GRAIN, GRAIN_ROWS, false);
  check(MAT.VEG, BED_ROWS, true);
});
