// wasi.js: a Rust program (wasi_prog.rs) built for wasm32-wasip1 runs on the in-memory file
// system and prints what the same program prints natively in a real directory; plus direct
// checks of fd_readdir's cookies and of errno values. The program's part is skipped when rustc
// or its wasm32-wasip1 target is missing.
//   node --test crates/ffh-web/site/test/
import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { WASI, MemFS, E, FILETYPE, WasiExit } from "../wasi.js";

const here = fileURLToPath(new URL(".", import.meta.url));
const prog = join(here, "wasi_prog.rs");

/** Why the Rust part cannot run here, or null. */
function rustMissing() {
  const v = spawnSync("rustc", ["--print", "sysroot"], { encoding: "utf8" });
  if (v.error || v.status !== 0) return "rustc not found";
  if (!existsSync(join(v.stdout.trim(), "lib", "rustlib", "wasm32-wasip1"))) return "the wasm32-wasip1 target is not installed";
  return null;
}

function rustc(args) {
  const r = spawnSync("rustc", ["--edition", "2021", "-O", ...args], { encoding: "utf8" });
  assert.equal(r.status, 0, `rustc ${args.join(" ")} failed:\n${r.stderr}`);
}

async function runWasm(bytes, env) {
  const fs = new MemFS();
  fs.mkdirp(env.PROG_DIR);
  const out = [];
  const err = [];
  const wasi = new WASI({ args: ["prog"], env, fs, stdout: (l) => out.push(l), stderr: (l) => err.push(l) });
  const { instance } = await WebAssembly.instantiate(bytes, { wasi_snapshot_preview1: wasi.imports });
  const code = wasi.start(instance);
  return { code, out, err, fs };
}

test("a Rust std program runs on wasi.js as it does natively", { timeout: 120000 }, async (t) => {
  const why = rustMissing();
  if (why) return t.skip(why);
  const dir = mkdtempSync(join(tmpdir(), "ffh-wasi-"));
  try {
    rustc(["--target", "wasm32-wasip1", prog, "-o", join(dir, "prog.wasm")]);
    rustc([prog, "-o", join(dir, "prog-native")]);
    const work = join(dir, "work");
    spawnSync("mkdir", ["-p", work]);
    const native = spawnSync(join(dir, "prog-native"), [], { encoding: "utf8", env: { PROG_DIR: work, PROG_GREETING: "hi" } });
    assert.equal(native.status, 0, native.stderr);
    const nativeOut = native.stdout.trimEnd().split("\n");

    const bytes = readFileSync(join(dir, "prog.wasm"));
    const a = await runWasm(bytes, { PROG_DIR: "/work", PROG_GREETING: "hi" });
    assert.equal(a.code, 0, a.err.join("\n"));
    // Everything but the checksum line's value is compared; then the checksum too (it covers
    // the same lines).
    assert.deepEqual(a.out, nativeOut);
    assert.match(a.out.at(-1), /^checksum [0-9a-f]{16} over \d+ lines$/);
    // stderr: line buffered, the last line without a newline flushed at exit.
    assert.equal(a.err.length, 2, a.err.join("\n"));
    assert.match(a.err[0], /^random hash [0-9a-f]{16}$/);
    assert.equal(a.err[1], "stderr without newline");
    // random_get seeds HashMap's hasher: another run hashes differently.
    const b = await runWasm(bytes, { PROG_DIR: "/work", PROG_GREETING: "hi" });
    assert.notEqual(a.err[0], b.err[0]);
    // What the program left, seen from JS.
    assert.equal(new TextDecoder().decode(a.fs.readFile("/work/a/b2/moved.txt")), "replacement");
    assert.equal(a.fs.lookup("/work/many"), null);
    const dirty = a.fs.takeDirty();
    // (a directory renamed is reported by its old and new paths, not its files')
    assert.ok(dirty.has("/work/a/b2") && dirty.has("/work/a/b") && dirty.has("/work/many") && dirty.has("/work/a/copy.txt"), `${dirty.size} paths`);
    t.diagnostic(`${a.out.length} lines match the native run; ${a.out.at(-1)}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// --- Direct calls, with a memory of our own ----------------------------------------------------

function harness(fs = new MemFS()) {
  const wasi = new WASI({ fs, stdout: () => {}, stderr: () => {} });
  const memory = new WebAssembly.Memory({ initial: 2 });
  wasi.memory = memory;
  const u8 = () => new Uint8Array(memory.buffer);
  const dv = () => new DataView(memory.buffer);
  const w = wasi.imports;
  const PATH = 1024;
  const OUT = 2048;
  const putPath = (p) => {
    const b = new TextEncoder().encode(p);
    u8().set(b, PATH);
    return [PATH, b.length];
  };
  return {
    fs,
    wasi,
    w,
    u8,
    dv,
    open(path, oflags = 0, rights = (1n << 30n) - 1n, fdflags = 0) {
      const r = w.path_open(3, 0, ...putPath(path), oflags, rights, rights, fdflags, OUT);
      return r === E.SUCCESS ? dv().getUint32(OUT, true) : -r;
    },
    call(name, path, ...rest) {
      return w[name](3, ...putPath(path), ...rest);
    },
    /** fd_readdir into a buffer of `len` bytes from `cookie`: the entries wholly in it, the
     *  bytes used. */
    readdir(fd, len, cookie) {
      const BUF = 8192;
      assert.equal(w.fd_readdir(fd, BUF, len, cookie, OUT), E.SUCCESS);
      const used = dv().getUint32(OUT, true);
      const ents = [];
      let off = 0;
      while (off + 24 <= used) {
        const d = new DataView(memory.buffer, BUF + off);
        const namlen = d.getUint32(16, true);
        if (off + 24 + namlen > used) break;
        ents.push({ next: d.getBigUint64(0, true), ino: d.getBigUint64(8, true), type: d.getUint8(20), name: new TextDecoder().decode(u8().slice(BUF + off + 24, BUF + off + 24 + namlen)) });
        off += 24 + namlen;
      }
      return { ents, used };
    },
  };
}

test("fd_readdir: dot entries, cookies, truncated entries, removal while listing", () => {
  const h = harness();
  h.fs.mkdirp("/d/sub");
  for (const n of ["a", "bb", "ccc", "dddd"]) h.fs.writeFile(`/d/${n}`, new Uint8Array([1]));
  const fd = h.open("d", 2, (1n << 30n) - 1n);
  assert.ok(fd > 3);
  // All at once: . and .. first, then the entries in creation order with their types.
  const all = h.readdir(fd, 4096, 0n);
  assert.deepEqual(all.ents.map((e) => e.name), [".", "..", "sub", "a", "bb", "ccc", "dddd"]);
  assert.equal(all.ents[2].type, FILETYPE.DIRECTORY);
  assert.equal(all.ents[3].type, FILETYPE.REGULAR_FILE);
  assert.ok(all.used < 4096);
  // A small buffer: full, the last entry cut; resuming from the last whole entry's cookie
  // gives the rest.
  const first = h.readdir(fd, 24 * 3 + 6, 0n);
  assert.equal(first.used, 24 * 3 + 6);
  assert.deepEqual(first.ents.map((e) => e.name), [".", "..", "sub"]);
  const rest = h.readdir(fd, 4096, first.ents.at(-1).next);
  assert.deepEqual(rest.ents.map((e) => e.name), ["a", "bb", "ccc", "dddd"]);
  // Removing entries during a listing does not skip the others (the cookies are stable).
  const part = h.readdir(fd, 4096, 0n).ents.slice(0, 4); // ., .., sub, a
  h.fs.remove("/d/a");
  h.fs.remove("/d/sub");
  const after = h.readdir(fd, 4096, part.at(-1).next);
  assert.deepEqual(after.ents.map((e) => e.name), ["bb", "ccc", "dddd"]);
  // A new entry comes after the cookie it was made after.
  h.fs.writeFile("/d/e", new Uint8Array(0));
  assert.deepEqual(h.readdir(fd, 4096, after.ents.at(-1).next).ents.map((e) => e.name), ["e"]);
  // Not a directory.
  const f = h.open("d/bb");
  assert.equal(h.w.fd_readdir(f, 8192, 100, 0n, 2048), E.NOTDIR);
});

test("errno values (WASI preview1)", () => {
  const h = harness();
  h.fs.mkdirp("/x/y");
  h.fs.writeFile("/x/f", new Uint8Array([1, 2, 3]));
  const CREAT = 1;
  const DIR = 2;
  const EXCL = 4;
  const TRUNC = 8;
  assert.equal(E.NOENT, 44);
  assert.equal(E.NOSYS, 52);
  assert.equal(h.open("x/none"), -44);
  assert.equal(h.open("x/f", CREAT | EXCL), -20);
  assert.equal(h.open("x/f", DIR), -54);
  assert.equal(h.open("x/f/g"), -54);
  assert.equal(h.open("x/y", TRUNC), -31);
  assert.equal(h.open("x/new/", CREAT), -31);
  assert.equal(h.call("path_create_directory", "x/y"), 20);
  assert.equal(h.call("path_remove_directory", "x"), 55);
  assert.equal(h.call("path_remove_directory", "x/f"), 54);
  assert.equal(h.call("path_unlink_file", "x/y"), 31);
  assert.equal(h.call("path_unlink_file", "x/none"), 44);
  assert.equal(h.w.fd_close(99), 8);
  assert.equal(h.w.fd_prestat_get(4, 2048), 8);
  assert.equal(h.w.fd_prestat_get(3, 2048), 0);
  assert.equal(h.dv().getUint32(2052, true), 1);
  // Not implemented: ENOSYS, and the module still links.
  assert.equal(h.w.sock_send(0, 0, 0, 0, 0), 52);
  assert.equal(h.w.path_symlink(0, 0, 3, 0, 0), 52);
  // A read-only fd refuses writes, a write-only one reads.
  const ro = h.open("x/f", 0, 1n << 1n);
  const iov = 3000;
  h.dv().setUint32(iov, 3100, true);
  h.dv().setUint32(iov + 4, 2, true);
  assert.equal(h.w.fd_write(ro, iov, 1, 3200), 8);
  const wo = h.open("x/f", 0, 1n << 6n);
  assert.equal(h.w.fd_read(wo, iov, 1, 3200), 8);
  // Seeking before the start.
  assert.equal(h.w.fd_seek(ro, -1n, 0, 3200), 28);
  assert.equal(h.w.fd_seek(1, 0n, 0, 3200), 70);
  // proc_exit unwinds.
  assert.throws(() => h.w.proc_exit(3), (e) => e instanceof WasiExit && e.code === 3);
});

test("MemFS: writes past the end leave zeros, truncation clears, rename over, change reports", () => {
  const fs = new MemFS();
  const f = fs.writeFile("/s/a.bin", new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]));
  fs.truncate(f, 2);
  fs.write(f, 6, new Uint8Array([9]));
  assert.deepEqual([...fs.readFile("/s/a.bin")], [1, 2, 0, 0, 0, 0, 9]);
  fs.takeDirty();
  const g0 = fs.generation;
  fs.writeFile("/s/b.bin", new Uint8Array([5]));
  fs.rename(fs.root, "s/b.bin", fs.root, "s/a.bin");
  assert.deepEqual([...fs.readFile("/s/a.bin")], [5]);
  assert.equal(fs.lookup("/s/b.bin"), null);
  assert.ok(fs.generation > g0);
  assert.deepEqual([...fs.takeDirty()].sort(), ["/s/a.bin", "/s/b.bin"]);
  // Paths: absolute, `..` above the root stays at the root, `.`.
  assert.equal(fs.lookup("/../s/./a.bin"), fs.lookup("/s/a.bin"));
  assert.equal(fs.pathOf(fs.lookup("/s/a.bin")), "/s/a.bin");
});
