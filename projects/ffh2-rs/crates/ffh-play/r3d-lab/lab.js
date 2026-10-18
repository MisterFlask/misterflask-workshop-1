// The 3D lab (docs/R3D.md "Lab"): the real 3D renderer (app/r3d/map3d.js) on a synthetic scene,
// without a game server. `tools/r3d-lab.mjs` serves it and takes screenshots; open it in a browser
// to look around (drag, wheel, the 3D panel).
//
//   lab.html?scene=showcase                     the vertical-slice scene
//   lab.html?scene=reference&ts=160             the look target's composition (docs/R3D.md §16)
//   lab.html?grid=tree.oak,tree.pine&variants=6 recipes in rows of variants on flat grass
//            &param=x (all rows) or &params=a,b,c (one per row; repeat a name for several params)
//            &ground=.  &spacing=1
//   &cx=..&cy=..&ts=..                         camera (map point, pixels per plot)
//   &r3d=painterly|plain|{json}                pipeline overrides (as in the client)
//   &float=1                                   (grid) things stand on the water, not the sea floor
//   &scale=1.4                                 (grid) every item at this scale (a catalog's scale)
//   scene=cities&civs=CIVILIZATION_A,CIVILIZATION_B&sizes=2,6,12&imps=IMPROVEMENT_COTTAGE,...
//        &walls=8&coast=1                      settlements side by side: a row per civilization
//                                              (coast=1: water south of each row)
//   rows=ffff.aaaa,jjjj.wwww                   a map from legend rows (scenes.js LEGEND, north
//                                              first), dressed by the real placement; separate the
//                                              rows with ';' when they hold ',' (plains)
//   &sel=x,y&select=1&hover=x,y&targets=x,y;x,y&path=x,y;x,y&attack=1&layer=political  overlays

import { S, setData, setView, setMap } from "/app/state.js";
import { Map3D } from "/app/r3d/map3d.js";
import { showcase, reference, grid, settlements, fromRows } from "./scenes.js";

const q = new URLSearchParams(location.search);

async function main() {
  const data = await (await fetch("/lab/data.json")).json();
  setData(data);
  let scene;
  if (q.get("grid")) {
    const names = q.get("grid").split(",").filter(Boolean);
    scene = grid(data, names, Number(q.get("variants") ?? 6), { ground: q.get("ground") ?? ".", spacing: Number(q.get("spacing") ?? 1) });
  } else if (q.get("rows")) {
    const rows = q.get("rows");
    scene = fromRows(data, rows.split(rows.includes(";") ? ";" : ","));
  } else if (q.get("scene") === "cities") {
    const list = (k, d) => (q.get(k) ?? d).split(",").filter(Boolean);
    scene = settlements(data, list("civs", "CIVILIZATION_BANNOR"), list("sizes", "2,6,12").map(Number), list("imps", ""), { walls: Number(q.get("walls") ?? 8), coast: q.get("coast") === "1" });
  } else if (q.get("scene") === "reference") {
    scene = reference(data);
  } else {
    scene = showcase(data);
  }
  setView(scene.view);
  setMap(scene.map);
  const wrap = document.getElementById("mapwrap");
  const canvas = wrap.querySelector("canvas.map");
  const view = {
    wrap,
    canvas,
    ctx: canvas.getContext("2d"),
    cam: { cx: Number(q.get("cx") ?? scene.focus[0]), cy: Number(q.get("cy") ?? scene.focus[1]), ts: Number(q.get("ts") ?? 96) },
    vw: wrap.clientWidth,
    vh: wrap.clientHeight,
    dpr: 1,
    map: scene.map,
    overlay: { path: null, targets: null, hover: null, sel: null, units: [], waypoints: null },
    layer: q.get("layer") ?? "terrain",
    flashes: [],
    reduceMotion: () => false,
    request() {
      r3d.draw();
    },
  };
  canvas.width = view.vw;
  canvas.height = view.vh;
  // Overlays: &sel=x,y &hover=x,y &targets=x,y;x,y &path=x,y;x,y (&attack=1) &select=1 (the units
  // on the sel plot are selected).
  const pts = (k) => (q.get(k) ?? "").split(";").filter(Boolean).map((p) => p.split(",").map(Number));
  if (q.get("sel")) view.overlay.sel = { x: pts("sel")[0][0], y: pts("sel")[0][1] };
  if (q.get("hover")) view.overlay.hover = { x: pts("hover")[0][0], y: pts("hover")[0][1] };
  if (q.get("targets")) view.overlay.targets = pts("targets");
  if (q.get("path")) view.overlay.path = { ok: true, attack: q.get("attack") === "1", declare_war_team: null, nodes: pts("path").map(([x, y], k) => ({ x, y, turns: k })) };
  if (q.get("select") && view.overlay.sel) {
    const here = S.view.units.filter((u) => u.x === view.overlay.sel.x && u.y === view.overlay.sel.y);
    S.sel = { units: here.map((u) => u.id), x: view.overlay.sel.x, y: view.overlay.sel.y };
    view.overlay.units = S.sel.units;
  }
  const r3d = new Map3D(view);
  if (scene.items) {
    // Grid mode: the recipes in rows, not the dressing.
    r3d.sync();
    const list = scene.items.map((it, k) => ({
      asset: it.asset,
      variant: it.variant,
      param: (q.get("params") ? q.get("params").split(",")[it.row] : null) ?? q.get("param") ?? "",
      x: it.mx,
      // &float=1: things that float (boats, whales, ice) on the water of a "-" or "~" ground.
      y: q.get("float") ? Math.max(0, r3d.world.heightAt(it.mx, it.my)) : r3d.world.heightAt(it.mx, it.my),
      z: -it.my,
      yaw: Number(q.get("yaw") ?? 0),
      sx: Number(q.get("scale") ?? 1), sy: Number(q.get("scale") ?? 1), sz: Number(q.get("scale") ?? 1),
      phase: k * 0.13,
      tint: [1, 1, 1],
      id: (k + 1) / (scene.items.length + 2),
      team: [0.75, 0.2, 0.15],
      protect: q.get("protect") ? 1 : 0,
      plot: -1,
    }));
    r3d.scene.setInstances("static", list);
    r3d.scene.setInstances("units", []);
    r3d.stacks = [];
    r3d.cities = [];
  }
  // Drag to pan, wheel to zoom.
  let down = null;
  canvas.addEventListener("pointerdown", (e) => (down = { x: e.clientX, y: e.clientY, cx: view.cam.cx, cy: view.cam.cy }));
  canvas.addEventListener("pointerup", () => (down = null));
  canvas.addEventListener("pointermove", (e) => {
    if (!down) return;
    const p = r3d.panDelta(down.cx, down.cy, e.clientX - down.x, e.clientY - down.y);
    view.cam = { ...view.cam, ...p };
    r3d.draw();
  });
  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    const zs = r3d.zooms();
    const i = zs.findIndex((z) => z >= view.cam.ts);
    view.cam = { ...view.cam, ts: zs[Math.max(0, Math.min(zs.length - 1, (i < 0 ? zs.length - 1 : i) + (e.deltaY < 0 ? 1 : -1)))] };
    r3d.draw();
  }, { passive: false });
  window.lab = { r3d, view, S, ready: true };
}

main().catch((e) => {
  document.body.append(Object.assign(document.createElement("pre"), { textContent: String(e.stack ?? e) }));
  window.lab = { error: String(e.stack ?? e) };
});
