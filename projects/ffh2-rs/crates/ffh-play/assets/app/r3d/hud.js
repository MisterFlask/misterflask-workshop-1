// The 3D map's interface marks (docs/R3D.md "Readability"), drawn on the map's 2D canvas over the
// WebGL frame, outside every painterly filter: city name plates (size, capital star), unit stack
// badges (count, moves, health), the Yields layer's numbers and the combat flashes. Each is
// anchored to a world point and projected with the frame's camera, so it tracks the 3D scene.

import { S, me } from "../state.js";
import * as T from "../tiles.js";

const FONT = "system-ui, 'DejaVu Sans', 'Segoe UI', sans-serif";

function roundRect(g, x, y, w, h, r) {
  g.beginPath();
  g.moveTo(x + r, y);
  g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r);
  g.arcTo(x, y, x + w, y, r);
  g.closePath();
}

function star(g, cx, cy, ro, ri) {
  g.beginPath();
  for (let i = 0; i < 10; i++) {
    const r = i % 2 ? ri : ro;
    const a = (Math.PI / 5) * i - Math.PI / 2;
    const x = cx + Math.cos(a) * r, y = cy + Math.sin(a) * r;
    i ? g.lineTo(x, y) : g.moveTo(x, y);
  }
  g.closePath();
}

export function drawHud(r3d, cam) {
  const v = r3d.view;
  const g = v.ctx;
  g.setTransform(v.dpr, 0, 0, v.dpr, 0, 0);
  g.clearRect(0, 0, v.vw, v.vh);
  if (!v.map || !r3d.world) return;
  const ts = r3d.ts;
  const mine = me()?.player;
  const inView = (p, m = 40) => p && p.x > -m && p.y > -m && p.x < v.vw + m && p.y < v.vh + m;

  // Zoomed far out the figures are too small to tell apart: each stack also gets a counter, a
  // small shield in its owner's colour on a stem over the figure (the strategy map's pieces).
  if (ts < 60) {
    const k = Math.max(0, Math.min(1, (60 - ts) / 16));
    for (const s of r3d.stacks ?? []) {
      const a = s.anchor;
      if (!a) continue;
      const base = r3d.project(cam, [r3d.nearX(a.x), a.y + a.height, a.z]);
      if (!inView(base)) continue;
      const r = 4 + 2 * k;
      const x = base.x, y = base.y - 7 - 5 * k;
      g.globalAlpha = 0.55 + 0.45 * k;
      g.strokeStyle = "rgba(15,10,8,0.85)";
      g.lineWidth = 1.5;
      g.beginPath();
      g.moveTo(base.x, base.y);
      g.lineTo(x, y + r);
      g.stroke();
      g.beginPath();
      g.moveTo(x - r, y - r);
      g.lineTo(x + r, y - r);
      g.lineTo(x + r, y + r * 0.2);
      g.quadraticCurveTo(x + r, y + r * 0.9, x, y + r * 1.5);
      g.quadraticCurveTo(x - r, y + r * 0.9, x - r, y + r * 0.2);
      g.closePath();
      g.fillStyle = s.team;
      g.fill();
      g.strokeStyle = s.picked ? "#fff2a8" : "rgba(15,10,8,0.9)";
      g.lineWidth = s.picked ? 2 : 1.2;
      g.stroke();
      g.globalAlpha = 1;
    }
  }

  // City plates: under the settlement, toward the camera; over the far-zoom counters (a city's
  // name matters more than the pins crowding round it), under the stack badges.
  for (const c of r3d.cities ?? []) {
    const city = c.city;
    // At the plot's front edge, clear of the gate where the garrison stands.
    const p = r3d.project(cam, [r3d.nearX(c.x), c.y, c.z + 0.56]);
    if (!inView(p, 120)) continue;
    const col = T.playerColor(city.owner, S.players.get(city.owner)?.barbarian);
    const seen = city.visible !== false;
    const fs = Math.max(10, Math.min(15, Math.round(ts * 0.15)));
    g.font = `600 ${fs}px ${FONT}`;
    const name = ts >= 40 ? city.name ?? "" : "";
    const tw = g.measureText(name).width;
    const sizeR = fs * 0.78;
    const pad = fs * 0.45;
    const w = (name ? tw + pad * 2 : 0) + sizeR * 2 + 4;
    const h = fs + 8;
    const x = p.x - w / 2;
    const y = p.y - h / 2;
    g.globalAlpha = seen ? 1 : 0.7;
    g.fillStyle = "rgba(18,14,12,0.82)";
    roundRect(g, x, y, w, h, h / 2);
    g.fill();
    g.lineWidth = 1.5;
    g.strokeStyle = col;
    g.stroke();
    // Size disc in the owner's colour.
    g.beginPath();
    g.arc(x + sizeR + 1, p.y, sizeR, 0, Math.PI * 2);
    g.fillStyle = col;
    g.fill();
    g.fillStyle = "#14100c";
    g.font = `700 ${Math.round(fs * 0.95)}px ${FONT}`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(String(city.size ?? ""), x + sizeR + 1, p.y + 0.5);
    if (name) {
      g.font = `600 ${fs}px ${FONT}`;
      g.fillStyle = city.owner === mine ? "#ffe9a8" : "#efe6d4";
      g.textAlign = "left";
      g.fillText(name, x + sizeR * 2 + 4 + pad, p.y + 0.5);
    }
    if (city.capital) {
      g.fillStyle = "#ffd75a";
      g.strokeStyle = "rgba(20,14,8,0.8)";
      g.lineWidth = 1;
      star(g, x + sizeR + 1, y - fs * 0.35, fs * 0.42, fs * 0.18);
      g.fill();
      g.stroke();
    }
    g.globalAlpha = 1;
  }

  // Stack badges: over the figure's head.
  for (const s of r3d.stacks ?? []) {
    const a = s.anchor;
    if (!a) continue;
    const p = r3d.project(cam, [r3d.nearX(a.x), a.y + a.height + 0.04, a.z]);
    if (!inView(p)) continue;
    const top = s.top;
    // Health bar for the wounded.
    if (top.hp != null && top.max_hp && top.hp < top.max_hp && ts >= 40) {
      const bw = Math.max(18, ts * 0.28);
      const f = Math.max(0, top.hp / top.max_hp);
      g.fillStyle = "rgba(10,8,6,0.85)";
      g.fillRect(p.x - bw / 2 - 1, p.y - 5, bw + 2, 5);
      g.fillStyle = f > 0.6 ? "#7ad04a" : f > 0.3 ? "#e8c040" : "#e04a30";
      g.fillRect(p.x - bw / 2, p.y - 4, bw * f, 3);
    }
    if (s.units.length > 1) {
      const t = String(s.units.length);
      const fs = Math.max(10, Math.min(14, Math.round(ts * 0.14)));
      g.font = `700 ${fs}px ${FONT}`;
      const bw = g.measureText(t).width + 8;
      const bh = fs + 4;
      const bx = p.x - ts * 0.16 - bw;
      const by = p.y - bh - 2;
      g.fillStyle = "rgba(18,14,12,0.88)";
      roundRect(g, bx, by, bw, bh, 3);
      g.fill();
      g.strokeStyle = s.team;
      g.lineWidth = 1.2;
      g.stroke();
      g.fillStyle = "#f4ecd8";
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(t, bx + bw / 2, by + bh / 2 + 0.5);
    }
    if (top.owner === mine) {
      g.beginPath();
      g.arc(p.x + ts * 0.14, p.y - 6, Math.max(2.5, ts * 0.035), 0, Math.PI * 2);
      g.fillStyle = top.moves > 0 ? "#4fe06a" : "#d8402c";
      g.fill();
      g.strokeStyle = "rgba(10,8,6,0.8)";
      g.lineWidth = 1;
      g.stroke();
    }
  }

  // The Yields layer: the in-sight plots' food, production and commerce.
  if (v.layer === "yield" && ts >= 40) {
    const map = v.map;
    const r = r3d.visibleRect(cam);
    g.font = `bold ${Math.max(10, Math.round(ts * 0.16))}px ${FONT}`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    for (let y = Math.max(0, Math.floor(r.y0)); y <= Math.min(map.h - 1, Math.ceil(r.y1)); y++) {
      for (let xx = Math.floor(r.x0); xx <= Math.ceil(r.x1); xx++) {
        const x = T.wrapCoord(xx, map.w, map.wrap_x);
        if (x === null) continue;
        const i = y * map.w + x;
        if (map.vis[i] !== 2) continue;
        const yl = T.unpackYield(map.yield[i]);
        if (!yl) continue;
        const p = r3d.project(cam, [xx + 0.5, r3d.world.plotHeight(x, y) + 0.02, -(y + 0.5)]);
        if (!inView(p)) continue;
        const parts = yl.map((val, j) => [val, T.YIELD_COLORS[j]]).filter(([val]) => val > 0);
        parts.forEach(([val, colr], k) => {
          const ox = (k - (parts.length - 1) / 2) * ts * 0.2;
          g.fillStyle = "rgba(0,0,0,0.7)";
          g.fillText(String(val), p.x + ox + 1, p.y + 1);
          g.fillStyle = colr;
          g.fillText(String(val), p.x + ox, p.y);
        });
      }
    }
  }

  // Combat flashes.
  const now = performance.now();
  for (const fl of v.flashes ?? []) {
    const t = Math.min(1, (now - fl.t0) / fl.dur);
    const p = r3d.project(cam, [r3d.nearX(fl.x + 0.5), r3d.world.plotHeight(fl.x, fl.y) + 0.2, -(fl.y + 0.5)]);
    if (!p) continue;
    const a = (1 - t) * 0.6;
    const rr = ts * (fl.kind === "death" ? 0.3 + t * 0.25 : 0.45);
    const grd = g.createRadialGradient(p.x, p.y, 0, p.x, p.y, rr);
    grd.addColorStop(0, `rgba(160,40,40,${a})`);
    grd.addColorStop(1, "rgba(160,40,40,0)");
    g.fillStyle = grd;
    g.beginPath();
    g.arc(p.x, p.y, rr, 0, Math.PI * 2);
    g.fill();
  }
}
