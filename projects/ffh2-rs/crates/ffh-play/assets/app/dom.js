// DOM helpers shared by the screens: an element builder, toasts, and the busy indicator.

/** `h("button.primary#end", {onclick, title}, "End turn", child...)`: an element with classes
 *  and id from the tag string, properties or attributes from `props` (`on*` are listeners,
 *  `data-*` and `aria-*` attributes, `style` an object), and text or node children (null and
 *  false are skipped, arrays flattened). */
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
  for (const c of children) {
    if (c === null || c === undefined || c === false) continue;
    if (Array.isArray(c)) append(el, c);
    else el.appendChild(c instanceof Node ? c : document.createTextNode(String(c)));
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
  setTimeout(() => el.remove(), ms);
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
