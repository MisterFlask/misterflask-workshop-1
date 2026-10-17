// DOM helpers shared by the screens: an element builder, toasts, and the busy indicator.

/** How the short texts of buttons and headings are drawn: `null` (plain text), or a function
 *  `(tagName, text, el) => Node | string` that look.js sets once the art has loaded (the bitmap
 *  fonts of docs/ART.md 11.6; the text stays the element's `textContent`). */
let textHook = null;

export function setTextHook(f) {
  textHook = f;
}

/** The icon of a game type (`(kind, index, size) => Node | null`), set by look.js once the art
 *  has loaded; `typeIconOf` answers null before that (and in the node tests). Kinds: those of
 *  look.js `typeIcon` and "civicoption" (the civic categories). */
let iconOf = null;

export function setTypeIconHook(f) {
  iconOf = f;
}

export function typeIconOf(kind, index, size = 16) {
  return iconOf?.(kind, index, size) ?? null;
}

/** The elements whose text children go through the text hook. */
const HOOKED = new Set(["BUTTON", "H2", "H3", "H4", "KBD"]);

/** Replaces an element's content with `text`, through the text hook as `h` would. */
export function setText(el, text) {
  el.replaceChildren(textHook && HOOKED.has(el.tagName) ? textHook(el.tagName, String(text), el) : String(text));
}

/** `h("button.primary#end", {onclick, title}, "End turn", child...)`: an element with classes
 *  and id from the tag string, properties or attributes from `props` (`on*` are listeners,
 *  `data-*` and `aria-*` attributes, `style` an object), and text or node children (null and
 *  false are skipped, arrays flattened). The text children of buttons and headings go through
 *  the text hook (`setTextHook`). */
export function h(tag, props = {}, ...children) {
  const m = /^([a-z0-9]+)?((?:[.#][\w-]+)*)$/i.exec(tag);
  const el = document.createElement(m?.[1] || "div");
  for (const part of (m?.[2] ?? "").match(/[.#][\w-]+/g) ?? []) {
    if (part[0] === ".") el.classList.add(part.slice(1));
    else el.id = part.slice(1);
  }
  if (props && (typeof props !== "object" || props instanceof Node || Array.isArray(props))) {
    children.unshift(props);
    props = {};
  }
  for (const [k, v] of Object.entries(props ?? {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2), v);
    else if (k === "style" && typeof v === "object") Object.assign(el.style, v);
    else if (k === "class") el.className += ` ${v}`;
    else if (k.startsWith("data-") || k.startsWith("aria-") || k === "role" || k === "for") el.setAttribute(k, v === true ? "" : v);
    else el[k] = v;
  }
  append(el, children);
  return el;
}

function append(el, children) {
  const hook = textHook && HOOKED.has(el.tagName) ? textHook : null;
  for (const c of children) {
    if (c === null || c === undefined || c === false) continue;
    if (Array.isArray(c)) append(el, c);
    else if (c instanceof Node) el.appendChild(c);
    else if (hook) {
      const n = hook(el.tagName, String(c), el);
      el.appendChild(n instanceof Node ? n : document.createTextNode(String(n)));
    } else el.appendChild(document.createTextNode(String(c)));
  }
}

export const $ = (sel, root = document) => root.querySelector(sel);

/** Replaces an element's children. */
export function fill(el, ...children) {
  el.replaceChildren();
  append(el, children);
  return el;
}

/** A transient message at the bottom right: `kind` is "info", "error" or "log". */
export function toast(text, kind = "info", ms = 4500) {
  const box = $("#toasts");
  if (!box) return;
  const el = h(`div.toast.${kind}`, { role: kind === "error" ? "alert" : "status" }, text);
  box.appendChild(el);
  while (box.children.length > 6) box.firstChild.remove();
  // Fade out before removing (CSS transition on `.leaving`); a fallback removes it even when no
  // transition runs (reduced motion, a hidden tab).
  setTimeout(() => {
    el.classList.add("leaving");
    el.addEventListener("transitionend", () => el.remove(), { once: true });
    setTimeout(() => el.remove(), 400);
  }, ms);
}

let busyCount = 0;

/** Runs `f` with the busy indicator shown. */
export async function busy(f) {
  busyCount++;
  document.body.classList.add("busy");
  try {
    return await f();
  } finally {
    if (--busyCount === 0) document.body.classList.remove("busy");
  }
}

/** Formats a signed number: +3, -2, 0. */
export function signed(n) {
  return n > 0 ? `+${n}` : `${n}`;
}
