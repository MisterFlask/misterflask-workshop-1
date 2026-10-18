// The 3D map's interface marks (docs/R3D.md "Readability"), drawn on the map's 2D canvas over the
// WebGL frame, outside every painterly filter: city name plates (size, capital star), unit stack
// badges (count, moves, health), the Yields layer's numbers and the combat flashes. Each is
// anchored to a world point and projected with the frame's camera, so it tracks the 3D scene.

import { S, me, dname, tname } from "../state.js";
import * as T from "../tiles.js";
import { CATALOG } from "./assets/catalog.js";

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

// Relative luminance of a "#rrggbb" colour (0..1), for picking readable text on an owner disc.
function hexLum(hex) {
  const h = String(hex).replace("#", "");
  if (h.length < 6) return 0.5;
  const r = parseInt(h.slice(0, 2), 16) / 255, g = parseInt(h.slice(2, 4), 16) / 255, b = parseInt(h.slice(4, 6), 16) / 255;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
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
    g.textAlign = "center";
    g.textBaseline = "middle";
    for (const s of r3d.stacks ?? []) {
      const a = s.anchor;
      if (!a) continue;
      const base = r3d.project(cam, [r3d.nearX(a.x), a.y + a.height, a.z]);
      if (!inView(base)) continue;
      // A shield in the owner's colour on a stem, carrying the unit type (F6): far out the figures
      // are too small to tell apart, so the pin names the stack's kind.
      const r = 7 + 4 * k;
      const x = base.x, y = base.y - 9 - 6 * k;
      g.globalAlpha = 0.7 + 0.3 * k;
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
      const ab = T.abbrev(dname("UnitType", s.top.type));
      g.font = `700 ${Math.round(r * 1.15)}px ${FONT}`;
      g.lineWidth = 2;
      g.strokeStyle = "rgba(12,8,6,0.9)";
      g.fillStyle = "#f8f2e4";
      g.strokeText(ab, x, y + 1);
      g.fillText(ab, x, y + 1);
      g.globalAlpha = 1;
    }
  }

  // City plates: under the settlement, toward the camera; over the far-zoom counters (a city's
  // name matters more than the pins crowding round it), under the stack badges.
  for (const c of r3d.cities ?? []) {
    const city = c.city;
    // At the plot's front edge, clear of the gate where the garrison stands; in front of the walls
    // of a great city that spills over its plot (the catalog's cityReach).
    const reach = CATALOG.cityReach?.({ size: city.size ?? 1, walls: !!city.walls, capital: !!city.capital }) ?? 0.5;
    const p = r3d.project(cam, [r3d.nearX(c.x), c.y, c.z + Math.min(0.92, Math.max(0.56, reach + 0.05))]);
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
    // A dark owner disc (the barbarian #5a1010) gets a light rim and light digit text, so the size
    // never reads as dark-on-dark (F14); mid/bright owner colours keep the dark digit.
    const darkDisc = hexLum(col) < 0.2;
    if (darkDisc) {
      g.lineWidth = 1.2;
      g.strokeStyle = "#c8b89a";
      g.stroke();
    }
    g.font = `700 ${Math.round(fs * 0.95)}px ${FONT}`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    const sizeStr = String(city.size ?? "");
    if (darkDisc) {
      g.lineWidth = 2;
      g.strokeStyle = "rgba(10,8,6,0.85)";
      g.strokeText(sizeStr, x + sizeR + 1, p.y + 0.5);
      g.fillStyle = city.owner === mine ? "#ffe9a8" : "#f4ecd8";
    } else {
      g.fillStyle = "#14100c";
    }
    g.fillText(sizeStr, x + sizeR + 1, p.y + 0.5);
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

  // Resource chips: a small HUD marker on each revealed resource, so resources read where the tiny
  // 3D props disappear among the rocks and shrubs (F12). Fades out above ~160 px, where the prop
  // carries it; dimmed on remembered (fogged) plots, as the city plates are. Fog of war is intact:
  // the server sends -1 for unrevealed plots and the remembered bonus for fogged ones.
  if (ts >= 36 && v.map.bonus) {
    const map = v.map;
    const rr = r3d.visibleRect(cam);
    const chipA = Math.max(0, Math.min(1, (160 - ts) / 40));
    if (chipA > 0) {
      g.textAlign = "center";
      g.textBaseline = "middle";
      for (let y = Math.max(0, Math.floor(rr.y0)); y <= Math.min(map.h - 1, Math.ceil(rr.y1)); y++) {
        for (let xx = Math.floor(rr.x0); xx <= Math.ceil(rr.x1); xx++) {
          const x = T.wrapCoord(xx, map.w, map.wrap_x);
          if (x === null) continue;
          const i = y * map.w + x;
          const b = map.bonus[i];
          if (b == null || b < 0) continue;
          if (map.vis && map.vis[i] === 0) continue;
          const fogged = map.vis && map.vis[i] !== 2;
          // At the resource prop's spot (dress.js BONUS_SPOT = [-0.26, 0.2]).
          const p = r3d.project(cam, [xx + 0.5 - 0.26, r3d.world.plotHeight(x, y) + 0.18, -(y + 0.5 - 0.2)]);
          if (!inView(p)) continue;
          const mana = (tname("BonusType", b) ?? "").startsWith("BONUS_MANA");
          const rad = Math.max(9, ts * 0.15);
          g.globalAlpha = (fogged ? 0.55 : 1) * chipA;
          g.beginPath();
          g.arc(p.x, p.y, rad, 0, Math.PI * 2);
          g.fillStyle = mana ? "#8e5cd6" : "#f0e2b6";
          g.fill();
          g.lineWidth = 1.5;
          g.strokeStyle = "rgba(20,14,8,0.85)";
          g.stroke();
          g.fillStyle = mana ? "#fff" : "#3a2a10";
          g.font = `700 ${Math.round(rad)}px ${FONT}`;
          g.fillText(T.abbrev(dname("BonusType", b)), p.x, p.y + 0.5);
          g.globalAlpha = 1;
        }
      }
    }
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
    // A type tag over the figure at the gameplay zooms, so the unit's kind reads where the figures
    // are too small to tell apart (F6). The type abbreviation fades out above ~150 px, where the
    // figures speak for themselves; below 60 px the far-zoom shield carries the type instead. The
    // stack count stays solid at every zoom: only up to three figures are ever drawn, so a bigger
    // stack cannot be counted from the figures, and fading the count would hide a stack's true size
    // at the closest zooms. The abbreviation and count share one plate but keep their own alpha, so
    // the count never dims while the abbreviation fades.
    const multi = s.units.length > 1;
    const cnt = multi ? String(s.units.length) : null;
    const abbrAlpha = ts >= 60 ? Math.max(0, Math.min(1, (152 - ts) / 14)) : 0;
    const parts = [];
    if (abbrAlpha > 0) parts.push([T.abbrev(dname("UnitType", top.type)), abbrAlpha]);
    if (cnt) parts.push([cnt, 1]);
    if (parts.length) {
      const fs = Math.max(10, Math.min(14, Math.round(ts * 0.14)));
      g.font = `700 ${fs}px ${FONT}`;
      g.textAlign = "left";
      g.textBaseline = "middle";
      const gap = parts.length > 1 ? g.measureText(" ").width : 0;
      const tw = parts.reduce((w, [t]) => w + g.measureText(t).width, 0) + gap * (parts.length - 1);
      const bw = tw + 8;
      const bh = fs + 4;
      const bx = p.x - ts * 0.16 - bw;
      const by = p.y - bh - 2;
      // The plate is as opaque as its most-solid part: solid while a count rides along (so the
      // count always reads), but it fades out with a lone abbreviation, so a single-unit badge fades
      // smoothly at the gameplay-to-close zoom transition instead of popping out as a solid plate.
      const plateAlpha = Math.max(...parts.map(([, a]) => a));
      g.globalAlpha = plateAlpha;
      g.fillStyle = "rgba(18,14,12,0.88)";
      roundRect(g, bx, by, bw, bh, 3);
      g.fill();
      g.strokeStyle = s.team;
      g.lineWidth = 1.2;
      g.stroke();
      g.fillStyle = "#f4ecd8";
      let tx = bx + 4;
      const ty = by + bh / 2 + 0.5;
      for (const [t, a] of parts) {
        g.globalAlpha = a;
        g.fillText(t, tx, ty);
        tx += g.measureText(t).width + gap;
      }
      g.globalAlpha = 1;
    }
    if (top.owner === mine) {
      // A resting stack (fortified, sentried, sleeping, healing) is greyed, not green, so it is not
      // mistaken for one awaiting orders; a small letter names the state (F19).
      const resting = s.mark === "fortify" || s.mark === "sleep" || s.mark === "sentry" || s.mark === "heal";
      const dotX = p.x + ts * 0.14, dotY = p.y - 6, dotR = Math.max(2.5, ts * 0.035);
      g.beginPath();
      g.arc(dotX, dotY, dotR, 0, Math.PI * 2);
      g.fillStyle = resting ? "#9aa2ac" : top.moves > 0 ? "#4fe06a" : "#d8402c";
      g.fill();
      g.strokeStyle = "rgba(10,8,6,0.8)";
      g.lineWidth = 1;
      g.stroke();
      const glyph = { fortify: "F", sleep: "Z", sentry: "S", heal: "H", auto: "A", promote: "↑" }[s.mark];
      if (glyph && ts >= 54) {
        g.font = `700 ${Math.max(9, Math.round(ts * 0.11))}px ${FONT}`;
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.lineWidth = 2;
        g.strokeStyle = "rgba(12,8,6,0.85)";
        g.strokeText(glyph, dotX + dotR + ts * 0.06, dotY);
        g.fillStyle = s.mark === "promote" ? "#ffd75a" : "#f4ecd8";
        g.fillText(glyph, dotX + dotR + ts * 0.06, dotY);
      }
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
