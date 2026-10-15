// The client's look (docs/ART.md, "Monastic Pixel Noir"): loads the original pixel art the server
// embeds (`/art/`: the toolkit's modules and `/art/art.bundle.txt`, the sprites), installs the
// ui set's stylesheet (palette properties, nine-slice frames, buttons, cursors), and gives the
// screens bitmap text (Vigil for titles, Vigil Book for short labels, Tally for numbers) and
// icons. Until the art has loaded, or when it fails, the helpers fall back to plain text, so
// every screen works without it.
//
// The map renderer is chosen here too: the art renderer (`artmap.js`) unless the page was opened
// with `?renderer=flat` or the Tools → Options choice says "flat" (the P5 renderer, kept for
// debugging).

import { loadArt } from "../art/art.js";
import * as UI from "../art/ui/ui.js";
import { ownerKey } from "./stacks.js";
import { setTextHook } from "./dom.js";
import { setIconHook } from "./markup.js";

export { UI };

/** The art (an `Art` of art.js) once loaded, the error if it failed, the UI scale `u` (CSS px
 *  per art px) and the device-pixel ratio the bitmaps are drawn for. */
export const look = { art: null, error: null, u: 2, dpr: 1, ms: 0, data: null };

const RENDERER_KEY = "ffh.renderer";

/** The map renderer asked for: "flat" from `?renderer=flat` (or `?renderer=art`), else the
 *  remembered Tools → Options choice, else "art". */
export function rendererChoice() {
  let q = null;
  try {
    q = new URLSearchParams(globalThis.location?.search ?? "").get("renderer");
  } catch {
    q = null;
  }
  if (q === "flat" || q === "art") return q;
  try {
    const v = globalThis.localStorage?.getItem(RENDERER_KEY);
    if (v === "flat" || v === "art") return v;
  } catch {
    /* no storage */
  }
  return "art";
}

/** Remembers the renderer (Tools → Options). */
export function setRendererChoice(r) {
  try {
    globalThis.localStorage?.setItem(RENDERER_KEY, r);
  } catch {
    /* no storage */
  }
}

/** The art kinds and the `/api/data` tables that name their indices. */
const KIND_TABLES = {
  terrain: "TerrainType",
  feature: "FeatureType",
  bonus: "BonusType",
  improvement: "ImprovementType",
  route: "RouteType",
  unit: "UnitType",
  civ: "CivilizationType",
  religion: "ReligionType",
  building: "BuildingType",
  promotion: "PromotionType",
  unitcombat: "UnitCombatType",
  specialist: "SpecialistType",
};

/** Loads the art (once): the sprite bundle, the toolkit with its set passes, the ui stylesheet.
 *  Resolves with the `Art`, or null when it could not load (the error is in `look.error`). */
let loading = null;
export function loadLook() {
  if (loading) return loading;
  loading = (async () => {
    const t0 = performance.now();
    try {
      look.dpr = Math.max(1, Math.round(Math.min(globalThis.devicePixelRatio || 1, 3)));
      const r = await fetch("/art/art.bundle.txt");
      if (!r.ok) throw new Error(`/art/art.bundle.txt: HTTP ${r.status}`);
      const bundle = await r.text();
      const art = await loadArt({ bundle, lenient: true });
      if (art.lib.errors.length) console.warn(`art: ${art.lib.errors.length} source errors:\n${art.lib.errors.slice(0, 5).join("\n")}`);
      look.art = art;
      if (look.data) nameIndices(look.data);
      UI.installCSS(art, { u: look.u, dpr: look.dpr });
      installChrome(art);
      setTextHook(hookText);
      setIconHook(markupIcon);
      decorate(document.body);
      document.documentElement.classList.add("art");
      look.ms = Math.round(performance.now() - t0);
      return art;
    } catch (e) {
      look.error = e;
      console.warn("art: not loaded, the plain look is used:", e);
      return null;
    }
  })();
  return loading;
}

/** `/api/data` arrived: the art reads the map's and view's type indices with its names. */
export function setLookData(data) {
  look.data = data;
  if (look.art) nameIndices(data);
}

function nameIndices(data) {
  for (const [kind, table] of Object.entries(KIND_TABLES)) {
    const list = data?.tables?.[table];
    if (list) look.art.setIndexNames(kind, list.map((e) => e.type));
  }
}

// ---------------------------------------------------------------------------------------------
// The chrome: nine-slice frames and cursors as CSS properties (style.css uses them under
// `html.art`), bitmap text for buttons and headings, icons in game text.

/** The frames of the ui set (ui/chrome.spr) that style.css draws with. */
const FRAMES = [
  "frame_panel", "frame_page", "frame_modal", "frame_title", "frame_raised", "frame_card_selected", "frame_tooltip", "frame_inset",
  "frame_plate", "button", "button_hover", "button_pressed", "button_disabled", "button_primary", "button_primary_hover",
  "button_danger", "button_danger_hover", "focus", "bar",
];
const CURSORS = ["cursor", "cursor_move", "cursor_attack", "cursor_cast", "cursor_invalid", "cursor_busy"];

/** Defines, for each frame `ui/<x>`, `--nine-<x>` (a border-image) and `--nine-<x>-w` (its
 *  border widths) at UI scale `u`, pre-scaled to the device pixels; and `--<cursor>` for the
 *  pointer cursors. */
function installChrome(art) {
  const u = look.u;
  const k = Math.max(1, Math.round(u * look.dpr));
  const lines = [];
  for (const f of FRAMES) {
    const n = UI.nineOf(art, `ui/${f}`);
    if (!n) continue;
    const url = art.icon(`ui/${f}`, { scale: k }).toDataURL();
    const w = `${n.T * u}px ${n.R * u}px ${n.B * u}px ${n.L * u}px`;
    const name = f.replace(/_/g, "-");
    lines.push(`  --nine-${name}: url(${url}) ${n.T * k} ${n.R * k} ${n.B * k} ${n.L * k}${n.fill ? " fill" : ""} / ${w} repeat;`);
    lines.push(`  --nine-${name}-w: ${w};`);
  }
  for (const c of CURSORS) if (art.hasSprite(`ui/${c}`)) lines.push(`  --${c.replace(/_/g, "-")}: ${UI.cursorCSS(art, `ui/${c}`, { u })};`);
  let st = document.getElementById("look-chrome");
  if (!st) {
    st = document.createElement("style");
    st.id = "look-chrome";
    document.head.append(st);
  }
  st.textContent = `:root {\n${lines.join("\n")}\n}\n`;
}

/** Whether every character of `text` has a glyph in the bitmap font (else the text stays in
 *  the system font: arrows, stars and other symbols). */
export function pixable(text, font = "book", smallCaps = false) {
  const art = look.art;
  if (!art) return false;
  const f = font === "book" ? UI.BOOK : font;
  for (const ch of UI.foldText(text)) {
    if (ch === " ") continue;
    if (!art._glyph(f, ch, smallCaps)) return false;
  }
  return true;
}

/** The text hook of dom.js: buttons in Vigil Book (the primary one, End turn, in Vigil small
 *  capitals), headings in Vigil (section headings in small capitals), hotkeys in Vigil Book. */
function hookText(tag, text, el) {
  let font = "book";
  let smallCaps = false;
  if (tag === "H2" || tag === "H3") font = "vigil";
  else if (tag === "H4") (font = "vigil"), (smallCaps = true);
  else if (tag === "BUTTON" && el?.classList?.contains("primary")) (font = "vigil"), (smallCaps = true);
  if (!text.trim() || !pixable(text, font, smallCaps)) return text;
  return pix(text, { font, smallCaps });
}

/** Converts the text of the page's static buttons and headings (index.html) as `h` converts
 *  the ones built later. */
export function decorate(root) {
  for (const el of root.querySelectorAll("button, h2, h3, h4, kbd")) {
    if (el.childNodes.length !== 1 || el.firstChild.nodeType !== 3) continue;
    const n = hookText(el.tagName, el.firstChild.textContent, el);
    if (n instanceof Node) el.replaceChildren(n);
  }
}

/** Civ4's [ICON_X] in game text as the ui set's 8 px icons (markup.js). */
const MARKUP_ICONS = {
  ICON_FOOD: "food", ICON_PRODUCTION: "production", ICON_COMMERCE: "commerce", ICON_TRADE: "commerce", ICON_GOLD: "gold",
  ICON_RESEARCH: "research", ICON_CULTURE: "culture", ICON_ESPIONAGE: "espionage", ICON_HAPPY: "happy", ICON_UNHAPPY: "unhappy",
  ICON_HEALTHY: "healthy", ICON_UNHEALTHY: "unhealthy", ICON_GREATPEOPLE: "greatpeople", ICON_GREAT_PEOPLE: "greatpeople",
  ICON_STRENGTH: "strength", ICON_MOVES: "moves", ICON_DEFENSE: "defense", ICON_ANGRYPOP: "angrypop", ICON_BULLET: "bullet",
  ICON_MANA: "mana",
};

function markupIcon(name) {
  const n = MARKUP_ICONS[name];
  return n ? iconEl(`icon8/${n}`, { title: n, cls: "inline-icon" }) : null;
}

// ---------------------------------------------------------------------------------------------
// Bitmap text and icons in the DOM

/** Text in a bitmap font as a span whose colour follows CSS (`UI.textSpan`: the real text
 *  stays inside for screen readers, `textContent` and the tests), or a plain text node before
 *  the art loaded. `font`: "book" (Vigil Book, the body face of short UI text; default),
 *  "vigil" (titles: capitals and small capitals), "tally" (digits). */
export function pix(text, { font = "book", smallCaps = false, cls } = {}) {
  const t = String(text ?? "");
  if (!look.art || t === "") return document.createTextNode(t);
  const f = font === "book" ? UI.BOOK : font;
  return UI.textSpan(look.art, t, { font: f, smallCaps, u: look.u, dpr: look.dpr, cls });
}

/** An icon sprite (`icon16/food`, `icon8/gold`, ...) as a canvas sized in CSS px, or null
 *  before the art loaded or when there is no such sprite. */
export function iconEl(name, { title, cls, remap, u = look.u } = {}) {
  if (!look.art || !name || !look.art.hasSprite(name)) return null;
  return UI.iconEl(look.art, name, { u, dpr: look.dpr, remap, title, cls });
}

/** The icon of a game type through the art's mapping (religion, promotion, building,
 *  unitcombat, specialist, civ), 16 or 8 px; `type` is a type name or an index. */
export function typeIcon(kind, type, size = 16, opts = {}) {
  if (!look.art) return null;
  const r = UI.iconFor(look.art, kind, look.art.typeName(kind, type), size);
  return r ? iconEl(r.name, { ...opts, remap: r.remap }) : null;
}

/** A yield (0-2), commerce (0-3) or other interface icon (`UI.iconName`). */
export function uiIcon(kind, key, size = 16, opts = {}) {
  return iconEl(UI.iconName(kind, key, size), opts);
}

/** A progress bar (ui.js `drawBar`: a void-outlined track with the kind's fill, `next` the
 *  checkered part expected next turn) `w` art px wide, as a canvas; or the plain bar before the
 *  art loaded. kind: food production research culture gp xp hp hp_low doom. */
export function barEl(frac, kind = "research", { w = 30, next = null, title } = {}) {
  const f = Math.max(0, Math.min(1, Number(frac) || 0));
  if (!look.art) {
    const el = document.createElement("span");
    el.className = "bar";
    const fill = document.createElement("span");
    fill.className = "fill";
    fill.style.width = `${Math.round(100 * f)}%`;
    el.append(fill);
    if (title) el.title = title;
    return el;
  }
  const art = look.art;
  const k = Math.max(1, Math.round(look.u * look.dpr));
  const c = document.createElement("canvas");
  c.width = w * k;
  c.height = 5 * k;
  const old = art.scale;
  art.scale = k;
  try {
    UI.drawBar(art, c.getContext("2d"), 0, 0, w, f, kind, { next });
  } finally {
    art.scale = old;
  }
  c.style.width = `${w * look.u}px`;
  c.style.height = `${5 * look.u}px`;
  c.className = "ui-px ui-bar";
  if (title) c.title = title;
  return c;
}

/** A unit sprite (the map's figure, standing on nothing) as a canvas, for the unit panel, in
 *  its owner's colours. */
export function unitIcon(unitType, owner, { u = look.u, title } = {}) {
  const art = look.art;
  if (!art) return null;
  const type = art.typeName("unit", unitType);
  const r = type ? art.resolve("unit", type) : null;
  if (!r || r.sprite === "none") return null;
  const spr = art.pick(r.sprite, 0);
  if (!spr) return null;
  const k = Math.max(1, Math.round(u * look.dpr));
  const c = document.createElement("canvas");
  c.width = spr.w * k;
  c.height = spr.h * k;
  const old = art.scale;
  art.scale = k;
  try {
    art.drawSprite(c.getContext("2d"), spr.ax * k, spr.ay * k, spr.name, { remap: r.opts.remap, owner: owner ?? null, accent: r.opts.accent, rim: 1, checker: !!r.opts.ghost });
  } finally {
    art.scale = old;
  }
  c.style.width = `${spr.w * u}px`;
  c.style.height = `${spr.h * u}px`;
  c.className = "ui-px unit-figure";
  if (title) c.title = title;
  return c;
}

// ---------------------------------------------------------------------------------------------
// Owners: heraldry (docs/ART.md 7.2)

const assigned = new Map();
let assignedGame = null;

/** The owner styles of the players the client knows (`[{id, civ (type name), barbarian}]`),
 *  as a Map(id -> style). Each civilization takes its tincture plain, or the next free
 *  division of it, in the order the players became known (yours first): an assignment is kept
 *  for the game, so meeting another civilization never changes a banner already seen. A player
 *  the client does not know (not met) flies the lozenge. */
export function ownerStyles(players, game) {
  const art = look.art;
  if (!art) return new Map();
  if (game !== assignedGame) {
    assigned.clear();
    assignedGame = game;
  }
  const taken = new Set([...assigned.values()].map((s) => `${s.tinctureName}/${s.division}`));
  const out = new Map();
  for (const p of players) {
    const key = ownerKey(p);
    let st = assigned.get(key);
    if (!st) {
      if (p.barbarian || p.civ === "CIVILIZATION_BARBARIAN") st = art.ownerStyle("CIVILIZATION_BARBARIAN", { id: p.id, barbarian: true });
      else {
        const base = art.ownerStyle(p.civ ?? null, { id: p.id });
        const division = ["plain", "pale", "fess", "bend", "quarterly"].find((d) => !taken.has(`${base.tinctureName}/${d}`)) ?? "plain";
        st = art.ownerStyle(p.civ ?? null, { id: p.id, division });
        taken.add(`${st.tinctureName}/${division}`);
      }
      assigned.set(key, st);
    }
    out.set(p.id, st);
  }
  return out;
}

/** The owner style of an unknown player (not met): the lozenge on Argent. */
export function unknownOwner(id) {
  return look.art ? look.art.ownerStyle(null, { id: `unknown-${id}` }) : null;
}
