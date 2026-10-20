// engine.js `SavesPersister`: what reaches the saves store (IndexedDB in the page; a fake here)
// after the engine changed files under /saves: new, rewritten, appended, renamed (also over
// another file, and whole directories), removed; nothing when nothing changed; a failed write
// retried; and a fresh file system loaded from the store has the same tree.
import test from "node:test";
import assert from "node:assert/strict";
import { MemFS } from "../wasi.js";
import { SavesPersister, memoryStore, mountPack, parsePack } from "../engine.js";

const enc = new TextEncoder();
const dec = new TextDecoder();

/** A store that records each `apply` and can be made to fail. */
function fakeStore() {
  const inner = memoryStore();
  const calls = [];
  return {
    kind: "fake",
    entries: inner.entries,
    calls,
    failNext: false,
    load: () => inner.load(),
    async apply(puts, deletes) {
      calls.push({ puts: puts.map(([k, v]) => [k, v.kind === "dir" ? "dir" : dec.decode(v.data)]).sort(), deletes: [...deletes].sort() });
      if (this.failNext) {
        this.failNext = false;
        throw new Error("quota");
      }
      await inner.apply(puts, deletes);
    },
  };
}

/** The tree under `root`: `{relPath: "dir" | contents}`. */
function tree(fs, root) {
  const out = {};
  fs.walk(root, (rel, node) => {
    out[rel] = node.kind === "dir" ? "dir" : dec.decode(node.data.subarray(0, node.size));
  });
  return out;
}

function storeTree(store) {
  const out = {};
  for (const [k, v] of store.entries) out[k] = v.kind === "dir" ? "dir" : dec.decode(v.data);
  return out;
}

test("the store follows the saves directory, change by change", async () => {
  const fs = new MemFS();
  fs.writeFile("/data/xml/a.xml", enc.encode("<x/>"));
  const store = fakeStore();
  const p = new SavesPersister(fs, "/saves", store);
  assert.equal(await p.load(), 0);

  // Nothing changed: no write at all.
  assert.deepEqual(await p.flush(), { puts: 0, deletes: 0 });
  assert.equal(store.calls.length, 0);

  // A game directory with a journal and an autosave (written to a temporary file, renamed).
  const g = fs.mkdirp("/saves/game1/auto");
  const j = fs.createFile(fs.lookup("/saves/game1"), "journal.jsonl");
  fs.write(j, 0, enc.encode("{\"new\":1}\n"));
  const t = fs.createFile(g, "T0001.ffhsave.tmp");
  fs.write(t, 0, enc.encode("save1"));
  fs.rename(fs.root, "saves/game1/auto/T0001.ffhsave.tmp", fs.root, "saves/game1/auto/T0001.ffhsave");
  // Changes outside /saves are not the store's.
  fs.writeFile("/data/xml/b.xml", enc.encode("<y/>"));
  await p.flush();
  assert.deepEqual(store.calls.at(-1), {
    puts: [["game1", "dir"], ["game1/auto", "dir"], ["game1/auto/T0001.ffhsave", "save1"], ["game1/journal.jsonl", "{\"new\":1}\n"]],
    deletes: [],
  });

  // An append to the journal rewrites only the journal.
  fs.write(j, j.size, enc.encode("{\"cmd\":2}\n"));
  await p.flush();
  assert.deepEqual(store.calls.at(-1), { puts: [["game1/journal.jsonl", "{\"new\":1}\n{\"cmd\":2}\n"]], deletes: [] });

  // The next autosave replaces an older one by rename; the oldest goes.
  const t2 = fs.createFile(g, "T0002.ffhsave.tmp");
  fs.write(t2, 0, enc.encode("save2"));
  fs.rename(fs.root, "saves/game1/auto/T0002.ffhsave.tmp", fs.root, "saves/game1/auto/T0002.ffhsave");
  fs.unlinkFile(fs.root, "saves/game1/auto/T0001.ffhsave");
  // A manual save written over an existing name (same path, another file).
  fs.writeFile("/saves/game1/mine.ffhsave", enc.encode("m1"));
  await p.flush();
  fs.writeFile("/saves/game1/x.tmp", enc.encode("m2"));
  fs.rename(fs.root, "saves/game1/x.tmp", fs.root, "saves/game1/mine.ffhsave");
  await p.flush();
  assert.deepEqual(store.calls.at(-2), { puts: [["game1/auto/T0002.ffhsave", "save2"], ["game1/mine.ffhsave", "m1"]], deletes: ["game1/auto/T0001.ffhsave"] });
  assert.deepEqual(store.calls.at(-1), { puts: [["game1/mine.ffhsave", "m2"]], deletes: [] });

  // A truncation (File::create over a file) is a change too.
  fs.truncate(fs.lookup("/saves/game1/mine.ffhsave"), 0);
  await p.flush();
  assert.deepEqual(store.calls.at(-1), { puts: [["game1/mine.ffhsave", ""]], deletes: [] });

  // A directory renamed: its whole tree moves.
  fs.rename(fs.root, "saves/game1", fs.root, "saves/game2");
  await p.flush();
  assert.deepEqual(store.calls.at(-1), {
    puts: [["game2", "dir"], ["game2/auto", "dir"], ["game2/auto/T0002.ffhsave", "save2"], ["game2/journal.jsonl", "{\"new\":1}\n{\"cmd\":2}\n"], ["game2/mine.ffhsave", ""]],
    deletes: ["game1", "game1/auto", "game1/auto/T0002.ffhsave", "game1/journal.jsonl", "game1/mine.ffhsave"],
  });
  assert.deepEqual(storeTree(store), tree(fs, "/saves"));

  // A failed write is retried by the next flush (with what changed since, too).
  fs.writeFile("/saves/game2/a.txt", enc.encode("A"));
  store.failNext = true;
  await assert.rejects(p.flush(), /quota/);
  fs.writeFile("/saves/game2/b.txt", enc.encode("B"));
  await p.flush();
  assert.deepEqual(store.calls.at(-1), { puts: [["game2/a.txt", "A"], ["game2/b.txt", "B"]], deletes: [] });

  // A tree removed (remove_dir_all).
  fs.remove("/saves/game2/auto");
  await p.flush();
  assert.deepEqual(store.calls.at(-1), { puts: [], deletes: ["game2/auto", "game2/auto/T0002.ffhsave"] });
  assert.deepEqual(storeTree(store), tree(fs, "/saves"));

  // Another page's file system loads the same tree, empty directories included, and has
  // nothing to write back.
  fs.mkdirp("/saves/empty-game");
  await p.flush();
  const fs2 = new MemFS();
  const p2 = new SavesPersister(fs2, "/saves", store);
  const n = await p2.load();
  assert.equal(n, store.entries.size);
  assert.deepEqual(tree(fs2, "/saves"), tree(fs, "/saves"));
  const before = store.calls.length;
  assert.deepEqual(await p2.flush(), { puts: 0, deletes: 0 });
  assert.equal(store.calls.length, before);
});

test("flushes run one at a time; those asked meanwhile share the next", async () => {
  const fs = new MemFS();
  let release;
  const gate = new Promise((r) => (release = r));
  const calls = [];
  const store = {
    kind: "slow",
    load: async () => [],
    async apply(puts) {
      calls.push(puts.map(([k]) => k));
      if (calls.length === 1) await gate;
    },
  };
  const p = new SavesPersister(fs, "/saves", store);
  await p.load();
  fs.writeFile("/saves/a", enc.encode("1"));
  const first = p.flush();
  await new Promise((r) => setTimeout(r, 5));
  fs.writeFile("/saves/b", enc.encode("2"));
  const second = p.flush();
  fs.writeFile("/saves/c", enc.encode("3"));
  const third = p.flush();
  assert.equal(second, third);
  release();
  await Promise.all([first, second]);
  assert.deepEqual(calls, [["a"], ["b", "c"]]);
});

test("restoring skips bad keys and keeps file times", async () => {
  const store = memoryStore([
    ["g/x.ffhsave", { kind: "file", data: enc.encode("xx"), mtime: 1700000000000 }],
    ["g", { kind: "dir" }],
    ["../evil", { kind: "file", data: enc.encode("no") }],
    ["g//y", { kind: "file", data: enc.encode("no") }],
    ["h/z", { kind: "file", data: enc.encode("parent made") }],
  ]);
  const fs = new MemFS();
  await new SavesPersister(fs, "/saves", store).load();
  assert.deepEqual(tree(fs, "/saves"), { g: "dir", "g/x.ffhsave": "xx", h: "dir", "h/z": "parent made" });
  assert.equal(fs.lookup("/saves/g/x.ffhsave").mtim, 1700000000000n * 1000000n);
  assert.equal(fs.lookup("/evil"), null);
});

test("the data pack mounts under /data", () => {
  const files = [["xml/a.xml", "<a/>"], ["xml/sub/b.xml", "<b>é</b>"], ["text/en.tsv", ""]];
  const parts = ["FFHPAK1\n"];
  for (const [p, c] of files) parts.push(`${p}\n${enc.encode(c).length}\n${c}`);
  const pack = enc.encode(parts.join(""));
  assert.deepEqual(parsePack(pack).map(([p, b]) => [p, dec.decode(b)]), files);
  const fs = new MemFS();
  assert.equal(mountPack(fs, "/data", pack), 3);
  assert.equal(dec.decode(fs.readFile("/data/xml/sub/b.xml")), "<b>é</b>");
  assert.equal(fs.readFile("/data/text/en.tsv").length, 0);
  assert.throws(() => parsePack(enc.encode("FFHPAK2\n")), /FFHPAK1/);
  assert.throws(() => parsePack(enc.encode("FFHPAK1\nxml/a\n10\nabc")), /cut/);
  assert.throws(() => parsePack(enc.encode("FFHPAK1\n../a\n1\nx")), /bad path/);
  assert.throws(() => parsePack(enc.encode("FFHPAK1\na\nx1\nx")), /bad length/);
});
