// Civ4 text markup (design §4.7): the data's help, strategy and pedia texts and the event texts
// keep it. `tokenize` and `plain` are pure (tested); `render` builds DOM nodes. Nothing in a
// text becomes HTML: only the tags below are recognized, everything else stays literal text.
//
// Tags: [NEWLINE], [PARAGRAPH:n], [TAB], [ICON_X], [LINK=target]..[\LINK], [COLOR_X]..
// [COLOR_REVERT] (also [\COLOR]), [BOLD]..[\BOLD], [H1]..[\H1].

/** Icon glyphs; an unknown icon shows its humanized name in brackets. */
export const ICONS = {
  ICON_BULLET: "•",
  ICON_HAPPY: "☺",
  ICON_UNHAPPY: "☹",
  ICON_HEALTHY: "♥",
  ICON_UNHEALTHY: "♡",
  ICON_FOOD: "♨",
  ICON_PRODUCTION: "⚒",
  ICON_COMMERCE: "¤",
  ICON_GOLD: "¤",
  ICON_RESEARCH: "⚗",
  ICON_CULTURE: "♪",
  ICON_ESPIONAGE: "◉",
  ICON_TRADE: "⇄",
  ICON_STRENGTH: "⚔",
  ICON_MOVES: "➜",
  ICON_GREATPEOPLE: "☆",
  ICON_GREAT_PEOPLE: "☆",
  ICON_RELIGION: "✚",
  ICON_STAR: "★",
  ICON_SILVER_STAR: "☆",
  ICON_POWER: "ϟ",
  ICON_DEFENSE: "⛨",
  ICON_ANGRYPOP: "☹",
  ICON_OCCUPATION: "⚑",
  ICON_MANA: "✦",
};

const TAG = /\[([\\/]?[A-Z][A-Z0-9_]*)(?:[=:]([^\]]*))?\]/g;

/** The tokens of a text: `{t: "text", s}`, `br`, `para`, `tab`, `{t: "icon", name}`,
 *  `{t: "link", target}`, `/link`, `{t: "color", name}`, `/color`, `bold`, `/bold`, `h1`,
 *  `/h1`. Unknown bracketed words stay text. */
export function tokenize(text) {
  const out = [];
  const s = String(text ?? "");
  let last = 0;
  const push = (t) => {
    if (t.t === "text" && out.length && out[out.length - 1].t === "text") out[out.length - 1].s += t.s;
    else out.push(t);
  };
  for (const m of s.matchAll(TAG)) {
    const [whole, raw, arg] = m;
    const tok = tagToken(raw.replace(/^\//, "\\"), arg);
    if (!tok) continue;
    if (m.index > last) push({ t: "text", s: s.slice(last, m.index) });
    push(tok);
    last = m.index + whole.length;
  }
  if (last < s.length) push({ t: "text", s: s.slice(last) });
  return out;
}

function tagToken(name, arg) {
  switch (name) {
    case "NEWLINE":
      return { t: "br" };
    case "PARAGRAPH":
      return { t: "para" };
    case "TAB":
      return { t: "tab" };
    case "BOLD":
      return { t: "bold" };
    case "\\BOLD":
      return { t: "/bold" };
    case "H1":
      return { t: "h1" };
    case "\\H1":
      return { t: "/h1" };
    case "LINK":
      return arg !== undefined ? { t: "link", target: arg } : null;
    case "\\LINK":
      return { t: "/link" };
    case "COLOR_REVERT":
    case "\\COLOR":
      return { t: "/color" };
    default:
      if (arg !== undefined) return null;
      if (name.startsWith("ICON_")) return { t: "icon", name };
      if (name.startsWith("COLOR_")) return { t: "color", name };
      return null;
  }
}

/** How [ICON_X] is drawn in the DOM: `null` (a glyph from ICONS), or a function `(name) => Node
 *  | null` that look.js sets once the art has loaded (the ui set's 8 px icons). */
let iconHook = null;

export function setIconHook(f) {
  iconHook = f;
}

function iconText(name) {
  return ICONS[name] ?? `[${name.slice(5).toLowerCase().replace(/_/g, " ")}]`;
}

/** The text without markup (icons as glyphs, line breaks as "\n"). */
export function plain(text) {
  return tokenize(text)
    .map((t) => {
      switch (t.t) {
        case "text":
          return t.s;
        case "br":
          return "\n";
        case "para":
          return "\n\n";
        case "tab":
          return "    ";
        case "icon":
          return iconText(t.name);
        default:
          return "";
      }
    })
    .join("")
    .replace(/[ \t]+\n/g, "\n")
    .trim();
}

/** The CSS colour of a COLOR_ tag: the palette colours docs/ART.md 11.2 gives Civ4's text
 *  colours (highlight vellum, positive lichen, negative scarlet, warning amber, links sand,
 *  Somnium frost, the rest bone). */
export function colorOf(name) {
  if (/WARNING|YELLOW/.test(name)) return "#e2913c";
  if (/NEGATIVE|RED/.test(name)) return "#b33a3a";
  if (/POSITIVE|GREEN/.test(name)) return "#6c724a";
  if (/HIGHLIGHT/.test(name)) return "#ddcfb4";
  if (/UNIT|BUILDING|LINK/.test(name)) return "#8c8068";
  if (/SOMNIUM/.test(name)) return "#717f86";
  if (/ALT|GREY|GRAY/.test(name)) return "#7d7366";
  return "#b3a28c";
}

/** The text as DOM nodes (a DocumentFragment of `doc`). */
export function render(text, doc = globalThis.document) {
  const frag = doc.createDocumentFragment();
  const stack = [frag];
  const top = () => stack[stack.length - 1];
  const open = (el) => {
    top().appendChild(el);
    stack.push(el);
  };
  const close = (tag) => {
    for (let i = stack.length - 1; i > 0; i--) {
      if (stack[i].dataset?.tag === tag) {
        stack.length = i;
        return;
      }
    }
  };
  for (const t of tokenize(text)) {
    switch (t.t) {
      case "text":
        top().appendChild(doc.createTextNode(t.s));
        break;
      case "br":
        top().appendChild(doc.createElement("br"));
        break;
      case "para":
        top().appendChild(doc.createElement("br"));
        top().appendChild(doc.createElement("br"));
        break;
      case "tab":
        top().appendChild(doc.createTextNode("    "));
        break;
      case "icon": {
        const art = iconHook?.(t.name);
        if (art) {
          top().appendChild(art);
          break;
        }
        const el = doc.createElement("span");
        el.className = "icon";
        el.title = t.name;
        el.textContent = iconText(t.name);
        top().appendChild(el);
        break;
      }
      case "link": {
        const el = doc.createElement("span");
        el.className = "link";
        el.dataset.tag = "link";
        el.dataset.target = t.target;
        open(el);
        break;
      }
      case "color": {
        const el = doc.createElement("span");
        el.dataset.tag = "color";
        el.style.color = colorOf(t.name);
        open(el);
        break;
      }
      case "bold":
      case "h1": {
        const el = doc.createElement(t.t === "bold" ? "b" : "strong");
        el.dataset.tag = t.t;
        if (t.t === "h1") el.className = "h1";
        open(el);
        break;
      }
      case "/link":
      case "/color":
      case "/bold":
      case "/h1":
        close(t.t.slice(1));
        break;
    }
  }
  return frag;
}
