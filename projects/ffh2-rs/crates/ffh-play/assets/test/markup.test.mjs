// node --test: Civ4 markup (app/markup.js). Rendering is checked with a minimal fake document,
// so that no text ever becomes HTML.
import test from "node:test";
import assert from "node:assert/strict";
import { tokenize, plain, render, ICONS, colorOf } from "../app/markup.js";

test("tags become tokens; other brackets stay text", () => {
  assert.deepEqual(tokenize("a[NEWLINE]b"), [{ t: "text", s: "a" }, { t: "br" }, { t: "text", s: "b" }]);
  assert.deepEqual(tokenize("[ICON_BULLET]x"), [{ t: "icon", name: "ICON_BULLET" }, { t: "text", s: "x" }]);
  assert.deepEqual(tokenize("[PARAGRAPH:1][TAB]"), [{ t: "para" }, { t: "tab" }]);
  assert.deepEqual(tokenize("[LINK=TECH_X]Tech[\\LINK]"), [{ t: "link", target: "TECH_X" }, { t: "text", s: "Tech" }, { t: "/link" }]);
  assert.deepEqual(tokenize("[LINK=X]a[/LINK]"), [{ t: "link", target: "X" }, { t: "text", s: "a" }, { t: "/link" }]);
  assert.deepEqual(tokenize("[COLOR_WARNING_TEXT]w[COLOR_REVERT]"), [{ t: "color", name: "COLOR_WARNING_TEXT" }, { t: "text", s: "w" }, { t: "/color" }]);
  assert.deepEqual(tokenize("[BOLD]b[\\BOLD][H1]h[\\H1]"), [{ t: "bold" }, { t: "text", s: "b" }, { t: "/bold" }, { t: "h1" }, { t: "text", s: "h" }, { t: "/h1" }]);
  // Footnotes, lower case and unknown words with arguments are text.
  assert.deepEqual(tokenize("see [25], [tab] and [SONG=x]"), [{ t: "text", s: "see [25], [tab] and [SONG=x]" }]);
  assert.deepEqual(tokenize(""), []);
  assert.deepEqual(tokenize(null), []);
  assert.deepEqual(tokenize("<b>&amp;</b>"), [{ t: "text", s: "<b>&amp;</b>" }]);
});

test("plain text", () => {
  assert.equal(plain("[ICON_BULLET]One[NEWLINE][ICON_BULLET]Two"), `${ICONS.ICON_BULLET}One\n${ICONS.ICON_BULLET}Two`);
  assert.equal(plain("[COLOR_HIGHLIGHT_TEXT]+2[COLOR_REVERT] [ICON_HAPPY]"), `+2 ${ICONS.ICON_HAPPY}`);
  assert.equal(plain("[ICON_SOMETHING_NEW]"), "[something new]");
  assert.equal(plain("  a [PARAGRAPH:1]b  "), "a\n\nb");
});

test("colours", () => {
  assert.notEqual(colorOf("COLOR_WARNING_TEXT"), colorOf("COLOR_POSITIVE_TEXT"));
  assert.equal(typeof colorOf("COLOR_X"), "string");
});

// A minimal document: elements keep tag, class, data, style and children; text nodes keep text.
function fakeDocument() {
  class Node {
    constructor() {
      this.childNodes = [];
    }
    appendChild(n) {
      this.childNodes.push(n);
      return n;
    }
    get textContent() {
      return this.childNodes.map((c) => c.textContent).join("");
    }
  }
  class Element extends Node {
    constructor(tag) {
      super();
      this.tagName = tag.toUpperCase();
      this.dataset = {};
      this.style = {};
      this.className = "";
      this._text = null;
    }
    set textContent(t) {
      this.childNodes = [new Text(t)];
    }
    get textContent() {
      return super.textContent;
    }
  }
  class Text {
    constructor(t) {
      this.textContent = t;
      this.tagName = "#text";
    }
  }
  return {
    createDocumentFragment: () => new Node(),
    createElement: (t) => new Element(t),
    createTextNode: (t) => new Text(t),
  };
}

test("render builds nodes, never HTML", () => {
  const doc = fakeDocument();
  const f = render("<img src=x onerror=alert(1)>[NEWLINE][COLOR_WARNING_TEXT]careful[COLOR_REVERT] [LINK=UNIT_X]unit[\\LINK]", doc);
  const kinds = f.childNodes.map((n) => n.tagName);
  assert.deepEqual(kinds, ["#text", "BR", "SPAN", "#text", "SPAN"]);
  assert.equal(f.childNodes[0].textContent, "<img src=x onerror=alert(1)>");
  assert.equal(f.childNodes[2].textContent, "careful");
  assert.equal(f.childNodes[2].style.color, colorOf("COLOR_WARNING_TEXT"));
  assert.equal(f.childNodes[4].dataset.target, "UNIT_X");
  assert.equal(f.textContent, "<img src=x onerror=alert(1)>careful unit");
  // An unclosed tag stays open to the end; a stray closing tag is ignored.
  const g = render("[BOLD]a[\\LINK]b", doc);
  assert.equal(g.childNodes.length, 1);
  assert.equal(g.childNodes[0].tagName, "B");
  assert.equal(g.childNodes[0].textContent, "ab");
  // Icons are glyphs with their name as the title.
  const i = render("[ICON_PRODUCTION]", doc);
  assert.equal(i.childNodes[0].textContent, ICONS.ICON_PRODUCTION);
});
