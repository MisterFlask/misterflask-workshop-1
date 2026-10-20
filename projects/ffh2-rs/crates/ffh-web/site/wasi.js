// WASI preview1 (`wasi_snapshot_preview1`) for Rust std on wasm32-wasip1, with an in-memory file
// system (docs/WEB.md; the contract's section 5). Used by the engine worker (engine.js) and by the
// node tests; no DOM, no dependencies.
//
// What it covers is what Rust std needs: files and directories (open with create / exclusive /
// truncate / append, read, write, positional I/O, seek, set_len, stat, readdir, mkdir, rmdir,
// unlink, rename), one preopened directory `/` (wasi-libc resolves absolute paths against it), the
// clocks (`realtime` = Date.now, `monotonic` = performance.now), `random_get`
// (crypto.getRandomValues), args and environment, stdout / stderr to line callbacks (line
// buffered), `proc_exit` (throws `WasiExit`), `poll_oneoff` (clock subscriptions sleep with
// Atomics.wait on a scratch SharedArrayBuffer when the page may, else return at once). Anything
// else answers ENOSYS (52). No symbolic or hard links.
//
// The file system reports what changed (`MemFS.generation`, bumped by every change, and
// `MemFS.takeDirty()`, the paths touched since the last call) so the worker can persist /saves.

/** WASI preview1 errno values (witx `errno`). */
export const E = Object.freeze({
  SUCCESS: 0, TOOBIG: 1, ACCES: 2, AGAIN: 6, BADF: 8, BUSY: 10, EXIST: 20, FAULT: 21, FBIG: 22, INVAL: 28, IO: 29,
  ISDIR: 31, LOOP: 32, NAMETOOLONG: 37, NOENT: 44, NOMEM: 48, NOSPC: 51, NOSYS: 52, NOTDIR: 54, NOTEMPTY: 55,
  NOTSUP: 58, NOTTY: 59, OVERFLOW: 61, PERM: 63, SPIPE: 70, XDEV: 75, NOTCAPABLE: 76,
});

/** `filetype`. */
export const FILETYPE = Object.freeze({ UNKNOWN: 0, CHARACTER_DEVICE: 2, DIRECTORY: 3, REGULAR_FILE: 4, SYMBOLIC_LINK: 7 });

const OFLAGS_CREAT = 1;
const OFLAGS_DIRECTORY = 2;
const OFLAGS_EXCL = 4;
const OFLAGS_TRUNC = 8;
const FDFLAGS_APPEND = 1;
const RIGHT_FD_READ = 1n << 1n;
const RIGHT_FD_WRITE = 1n << 6n;
const RIGHTS_ALL = (1n << 30n) - 1n;
const FSTFLAGS_ATIM = 1;
const FSTFLAGS_ATIM_NOW = 2;
const FSTFLAGS_MTIM = 4;
const FSTFLAGS_MTIM_NOW = 8;
const CLOCK_REALTIME = 0;
const CLOCK_MONOTONIC = 1;
const CLOCK_PROCESS_CPUTIME = 2;
const CLOCK_THREAD_CPUTIME = 3;
const EVENTTYPE_CLOCK = 0;
const SUBCLOCKFLAGS_ABSTIME = 1;

const EMPTY = new Uint8Array(0);
const enc = new TextEncoder();

/** A file system error: the WASI errno. */
export class FsError extends Error {
  constructor(errno, msg) {
    super(msg ?? `WASI errno ${errno}`);
    this.name = "FsError";
    this.errno = errno;
  }
}

/** `proc_exit`: unwinds out of the module with the exit code. */
export class WasiExit extends Error {
  constructor(code) {
    super(`the program exited with code ${code}`);
    this.name = "WasiExit";
    this.code = code;
  }
}

function nowNs() {
  return BigInt(Date.now()) * 1000000n;
}

function fail(errno) {
  throw new FsError(errno);
}

// ---------------------------------------------------------------------------------------------
// The in-memory file system

/** Directories and files in memory. A directory's `entries` map a name to `{node, seq}`; `seq`
 *  grows with every entry added, which makes the readdir cookies stable while entries are
 *  removed during a listing (`remove_dir_all` does that). A file's bytes are `data[0, size)`;
 *  the bytes of `data` past `size` are always zero (so a write past the end leaves a hole of
 *  zeros). Nodes know their parent and name (there are no hard links), so a node's path can be
 *  told for change reports. */
export class MemFS {
  constructor() {
    this.nextIno = 1;
    /** Bumped by every change. */
    this.generation = 0;
    this.dirty = new Set();
    this.root = this.#node("dir");
    this.root.parent = this.root;
  }

  #node(kind) {
    const t = nowNs();
    const n = { kind, ino: this.nextIno++, parent: null, name: "", atim: t, mtim: t, ctim: t, gen: 0 };
    if (kind === "dir") {
      n.entries = new Map();
      n.nextSeq = 0;
    } else {
      n.data = EMPTY;
      n.size = 0;
    }
    return n;
  }

  /** The absolute path of a node, or null when it is no longer in the tree. */
  pathOf(node) {
    const parts = [];
    let n = node;
    while (n !== this.root) {
      const p = n.parent;
      if (!p || p.entries.get(n.name)?.node !== n) return null;
      parts.push(n.name);
      n = p;
    }
    return "/" + parts.reverse().join("/");
  }

  #changed(node, path = undefined) {
    this.generation++;
    const p = path === undefined ? this.pathOf(node) : path;
    if (p !== null) this.dirty.add(p);
  }

  /** The paths changed (written, created, removed, renamed from or to) since the last call. */
  takeDirty() {
    const d = this.dirty;
    this.dirty = new Set();
    return d;
  }

  /** Resolves `path` against directory `base` (an absolute path starts at the root): the
   *  parent directory, the last name and the node there (null when absent). For a path whose last
   *  component is `.` or `..` (or empty), `parent` and `name` are null. `mustDir`: the path ends
   *  with `/`. Throws ENOENT / ENOTDIR for a missing or non-directory component. */
  resolve(base, path) {
    if (path.length === 0) fail(E.NOENT);
    if (path.includes("\0")) fail(E.INVAL);
    let dir = path.startsWith("/") ? this.root : base;
    if (dir.kind !== "dir") fail(E.NOTDIR);
    const comps = path.split("/").filter((c) => c !== "");
    const mustDir = path.endsWith("/");
    if (comps.length === 0) return { parent: null, name: null, node: dir, mustDir: true };
    for (let i = 0; i < comps.length - 1; i++) {
      const c = comps[i];
      if (c === ".") continue;
      if (c === "..") {
        dir = dir.parent;
        continue;
      }
      const e = dir.entries.get(c);
      if (!e) fail(E.NOENT);
      if (e.node.kind !== "dir") fail(E.NOTDIR);
      dir = e.node;
    }
    const last = comps[comps.length - 1];
    if (last === ".") return { parent: null, name: null, node: dir, mustDir: true };
    if (last === "..") return { parent: null, name: null, node: dir.parent, mustDir: true };
    const e = dir.entries.get(last);
    return { parent: dir, name: last, node: e ? e.node : null, mustDir };
  }

  /** Links `node` as `name` in `dir` (replacing an entry of that name). */
  #link(dir, name, node) {
    dir.entries.delete(name);
    dir.entries.set(name, { node, seq: ++dir.nextSeq });
    node.parent = dir;
    node.name = name;
    dir.mtim = dir.ctim = nowNs();
  }

  #unlink(dir, name) {
    dir.entries.delete(name);
    dir.mtim = dir.ctim = nowNs();
  }

  /** A new empty file `name` in `dir`. */
  createFile(dir, name) {
    const f = this.#node("file");
    this.#link(dir, name, f);
    this.#changed(f);
    return f;
  }

  /** A new directory `name` in `dir`. */
  createDir(dir, name) {
    const d = this.#node("dir");
    this.#link(dir, name, d);
    this.#changed(d);
    return d;
  }

  /** `path_create_directory`. */
  mkdir(base, path) {
    const r = this.resolve(base, path);
    if (r.node || !r.parent) fail(E.EXIST);
    return this.createDir(r.parent, r.name);
  }

  /** `path_remove_directory`. */
  rmdir(base, path) {
    const r = this.resolve(base, path);
    if (!r.node) fail(E.NOENT);
    if (r.node.kind !== "dir") fail(E.NOTDIR);
    if (!r.parent) fail(r.node === this.root ? E.BUSY : E.INVAL);
    if (r.node.entries.size) fail(E.NOTEMPTY);
    const p = this.pathOf(r.node);
    this.#unlink(r.parent, r.name);
    this.#changed(null, p);
  }

  /** `path_unlink_file`. */
  unlinkFile(base, path) {
    const r = this.resolve(base, path);
    if (!r.node) fail(E.NOENT);
    if (r.node.kind === "dir") fail(E.ISDIR);
    if (r.mustDir) fail(E.NOTDIR);
    const p = this.pathOf(r.node);
    this.#unlink(r.parent, r.name);
    this.#changed(null, p);
  }

  /** `path_rename`, with POSIX rename(2) semantics. */
  rename(base1, path1, base2, path2) {
    const src = this.resolve(base1, path1);
    if (!src.node) fail(E.NOENT);
    if (!src.parent) fail(E.BUSY);
    if (src.mustDir && src.node.kind !== "dir") fail(E.NOTDIR);
    const dst = this.resolve(base2, path2);
    if (!dst.parent) fail(E.BUSY);
    if (dst.node === src.node) return;
    if (src.node.kind === "dir") {
      for (let d = dst.parent; ; d = d.parent) {
        if (d === src.node) fail(E.INVAL);
        if (d === this.root) break;
      }
      if (dst.node) {
        if (dst.node.kind !== "dir") fail(E.NOTDIR);
        if (dst.node.entries.size) fail(E.NOTEMPTY);
      }
    } else {
      if (dst.node && dst.node.kind === "dir") fail(E.ISDIR);
      if (dst.mustDir) fail(E.NOTDIR);
    }
    const from = this.pathOf(src.node);
    this.#unlink(src.parent, src.name);
    this.#link(dst.parent, dst.name, src.node);
    src.node.ctim = nowNs();
    this.#changed(null, from);
    this.#changed(src.node);
  }

  /** Sets a file's size (cut or zero-extended). */
  truncate(f, size) {
    if (f.kind !== "file") fail(E.ISDIR);
    if (size < f.size) {
      f.data.fill(0, size, Math.min(f.size, f.data.length));
    } else if (size > f.data.length) {
      this.#grow(f, size);
    }
    f.size = size;
    this.#touch(f);
  }

  #grow(f, min) {
    const d = new Uint8Array(Math.max(min, f.data.length * 2, 64));
    d.set(f.data.subarray(0, f.size));
    f.data = d;
  }

  #touch(f) {
    f.gen++;
    f.mtim = f.ctim = nowNs();
    this.#changed(f);
  }

  /** Writes `bytes` at `pos` (extending the file); returns the count. */
  write(f, pos, bytes) {
    if (f.kind !== "file") fail(E.ISDIR);
    const end = pos + bytes.length;
    if (end > f.data.length) this.#grow(f, end);
    f.data.set(bytes, pos);
    if (end > f.size) f.size = end;
    this.#touch(f);
    return bytes.length;
  }

  /** At most `len` bytes from `pos` (a view, not a copy). */
  read(f, pos, len) {
    if (f.kind !== "file") fail(E.ISDIR);
    if (pos >= f.size) return EMPTY;
    return f.data.subarray(pos, Math.min(f.size, pos + len));
  }

  // --- JS-side helpers (absolute paths) ---

  /** The node at an absolute path, or null. */
  lookup(path) {
    try {
      return this.resolve(this.root, path).node;
    } catch (e) {
      if (e instanceof FsError) return null;
      throw e;
    }
  }

  /** Creates the directory and its parents (like `mkdir -p`); returns it. */
  mkdirp(path) {
    let dir = this.root;
    for (const c of path.split("/").filter((x) => x !== "" && x !== ".")) {
      if (c === "..") {
        dir = dir.parent;
        continue;
      }
      const e = dir.entries.get(c);
      if (!e) dir = this.createDir(dir, c);
      else if (e.node.kind === "dir") dir = e.node;
      else fail(E.NOTDIR);
    }
    return dir;
  }

  /** Writes a whole file at an absolute path, creating its parents. With `copy: false` the file
   *  takes `bytes` as its storage (a view into a larger buffer is fine: it is replaced, not
   *  written past its end, when the file grows). */
  writeFile(path, bytes, { copy = true, mtimeMs } = {}) {
    const i = path.lastIndexOf("/");
    const dir = this.mkdirp(path.slice(0, i));
    const name = path.slice(i + 1);
    if (!name) fail(E.INVAL);
    let f = dir.entries.get(name)?.node;
    if (f && f.kind !== "file") fail(E.ISDIR);
    if (!f) f = this.createFile(dir, name);
    f.data = copy ? new Uint8Array(bytes) : bytes;
    f.size = bytes.length;
    this.#touch(f);
    if (mtimeMs !== undefined) f.mtim = BigInt(Math.round(mtimeMs)) * 1000000n;
    return f;
  }

  /** A copy of a file's bytes at an absolute path, or null. */
  readFile(path) {
    const f = this.lookup(path);
    return f && f.kind === "file" ? f.data.slice(0, f.size) : null;
  }

  /** Removes a file or a directory tree at an absolute path; false when absent. */
  remove(path) {
    const r = this.resolve(this.root, path);
    if (!r.node || !r.parent) return false;
    const p = this.pathOf(r.node);
    this.#unlink(r.parent, r.name);
    this.#changed(null, p);
    return true;
  }

  /** Calls `fn(relPath, node)` for every node under the directory at `path` (parents first). */
  walk(path, fn) {
    const top = this.lookup(path);
    if (!top || top.kind !== "dir") return;
    const go = (dir, prefix) => {
      for (const [name, { node }] of dir.entries) {
        const rel = prefix ? `${prefix}/${name}` : name;
        fn(rel, node);
        if (node.kind === "dir") go(node, rel);
      }
    };
    go(top, "");
  }
}

// ---------------------------------------------------------------------------------------------
// Console sinks

class LineSink {
  constructor(fn) {
    this.fn = fn;
    this.dec = new TextDecoder();
    this.buf = "";
  }

  write(bytes) {
    this.buf += this.dec.decode(bytes, { stream: true });
    let i;
    while ((i = this.buf.indexOf("\n")) >= 0) {
      const line = this.buf.slice(0, i);
      this.buf = this.buf.slice(i + 1);
      this.fn(line);
    }
  }

  flush() {
    this.buf += this.dec.decode();
    if (this.buf) {
      const line = this.buf;
      this.buf = "";
      this.fn(line);
    }
  }
}

// ---------------------------------------------------------------------------------------------
// WASI

/** The scratch word `poll_oneoff` sleeps on (null when the page may not share memory). */
let sleepWord;
function sleepMs(ms) {
  if (!(ms > 0)) return;
  if (sleepWord === undefined) {
    try {
      sleepWord = typeof SharedArrayBuffer === "function" ? new Int32Array(new SharedArrayBuffer(4)) : null;
    } catch {
      sleepWord = null;
    }
  }
  if (!sleepWord) return;
  try {
    Atomics.wait(sleepWord, 0, 0, ms);
  } catch {
    // The main thread of a page may not block: return at once.
    sleepWord = null;
  }
}

/** One instance of the WASI imports over a `MemFS`.
 *
 *  new WASI({ args, env, fs, preopens, stdout, stderr })
 *    args      the program's arguments (argv[0] first)
 *    env       the environment, an object
 *    fs        the MemFS (a new one by default)
 *    preopens  the preopened directories, absolute paths (default ["/"]); fd 3, 4, ...
 *    stdout, stderr  called with each line written (without the newline)
 *
 *  `imports` is the `wasi_snapshot_preview1` import object (functions this module does not
 *  implement answer ENOSYS); `initialize(instance)` binds the memory and calls `_initialize` if
 *  the module exports it (a reactor); `start(instance)` runs `_start` (a command) and returns the
 *  exit code. */
export class WASI {
  constructor({ args = ["wasm"], env = {}, fs = new MemFS(), preopens = ["/"], stdout = (l) => console.log(l), stderr = (l) => console.warn(l) } = {}) {
    this.args = args.map(String);
    this.env = Object.entries(env).map(([k, v]) => `${k}=${v}`);
    this.fs = fs;
    this.memory = null;
    this.sinks = [null, new LineSink(stdout), new LineSink(stderr)];
    /** fd -> { kind: "stdio" | "file" | "dir", node, pos, append, rights, rightsInh, preopen } */
    this.fds = new Map();
    for (let i = 0; i < 3; i++) this.fds.set(i, { kind: "stdio", n: i, pos: 0, append: false, rights: RIGHTS_ALL, rightsInh: 0n });
    let fd = 3;
    for (const p of preopens) {
      const node = fs.mkdirp(p);
      this.fds.set(fd++, { kind: "dir", node, pos: 0, append: false, rights: RIGHTS_ALL, rightsInh: RIGHTS_ALL, preopen: p });
    }
    this._buf = null;
    this.imports = this.#makeImports();
  }

  /** Binds the instance's memory and runs `_initialize` when exported (a reactor). */
  initialize(instance) {
    this.memory = instance.exports.memory;
    if (typeof instance.exports._initialize === "function") instance.exports._initialize();
  }

  /** Runs a command's `_start`; returns its exit code. */
  start(instance) {
    this.memory = instance.exports.memory;
    try {
      instance.exports._start();
      return 0;
    } catch (e) {
      if (e instanceof WasiExit) return e.code;
      throw e;
    } finally {
      this.flush();
    }
  }

  /** Writes out the partial lines of stdout and stderr. */
  flush() {
    this.sinks[1].flush();
    this.sinks[2].flush();
  }

  #mem() {
    const b = this.memory.buffer;
    if (this._buf !== b) {
      this._buf = b;
      this._dv = new DataView(b);
      this._u8 = new Uint8Array(b);
    }
    return this._dv;
  }

  #u8() {
    this.#mem();
    return this._u8;
  }

  #str(ptr, len) {
    return new TextDecoder().decode(this.#u8().slice(ptr, ptr + len));
  }

  #fd(fd) {
    const f = this.fds.get(fd);
    if (!f) fail(E.BADF);
    return f;
  }

  #dirFd(fd) {
    const f = this.#fd(fd);
    if (f.kind !== "dir") fail(E.NOTDIR);
    return f;
  }

  #allocFd(entry) {
    let fd = 3;
    while (this.fds.has(fd)) fd++;
    this.fds.set(fd, entry);
    return fd;
  }

  /** The byte ranges of an iovec array. */
  #iovs(ptr, n) {
    const dv = this.#mem();
    const out = [];
    for (let i = 0; i < n; i++) out.push([dv.getUint32(ptr + 8 * i, true), dv.getUint32(ptr + 8 * i + 4, true)]);
    return out;
  }

  #filestat(ptr, node, stdio = false) {
    const u8 = this.#u8();
    u8.fill(0, ptr, ptr + 64);
    const dv = this.#mem();
    if (stdio) {
      dv.setUint8(ptr + 16, FILETYPE.CHARACTER_DEVICE);
      dv.setBigUint64(ptr + 24, 1n, true);
      return;
    }
    dv.setBigUint64(ptr, 1n, true);
    dv.setBigUint64(ptr + 8, BigInt(node.ino), true);
    dv.setUint8(ptr + 16, node.kind === "dir" ? FILETYPE.DIRECTORY : FILETYPE.REGULAR_FILE);
    dv.setBigUint64(ptr + 24, 1n, true);
    dv.setBigUint64(ptr + 32, BigInt(node.kind === "dir" ? 0 : node.size), true);
    dv.setBigUint64(ptr + 40, node.atim, true);
    dv.setBigUint64(ptr + 48, node.mtim, true);
    dv.setBigUint64(ptr + 56, node.ctim, true);
  }

  #setTimes(node, atim, mtim, flags) {
    const now = nowNs();
    if (flags & FSTFLAGS_ATIM_NOW) node.atim = now;
    else if (flags & FSTFLAGS_ATIM) node.atim = atim;
    if (flags & FSTFLAGS_MTIM_NOW) node.mtim = now;
    else if (flags & FSTFLAGS_MTIM) node.mtim = mtim;
    node.ctim = now;
  }

  #clock(id) {
    switch (id) {
      case CLOCK_REALTIME:
        return BigInt(Date.now()) * 1000000n;
      case CLOCK_MONOTONIC:
      case CLOCK_PROCESS_CPUTIME:
      case CLOCK_THREAD_CPUTIME:
        return BigInt(Math.round(performance.now() * 1e6));
      default:
        fail(E.INVAL);
    }
  }

  #writeTo(f, bytesList) {
    if (f.kind === "stdio") {
      if (f.n === 0) fail(E.BADF);
      for (const b of bytesList) this.sinks[f.n].write(b);
      return bytesList.reduce((s, b) => s + b.length, 0);
    }
    if (f.kind === "dir") fail(E.ISDIR);
    if (!(f.rights & RIGHT_FD_WRITE)) fail(E.BADF);
    let n = 0;
    for (const b of bytesList) {
      const pos = f.append ? f.node.size : f.pos;
      this.fs.write(f.node, pos, b);
      f.pos = pos + b.length;
      n += b.length;
    }
    return n;
  }

  #makeImports() {
    const self = this;
    const fs = this.fs;
    const impl = {
      args_sizes_get(argc, size) {
        const dv = self.#mem();
        dv.setUint32(argc, self.args.length, true);
        dv.setUint32(size, self.args.reduce((s, a) => s + enc.encode(a).length + 1, 0), true);
      },
      args_get(argv, buf) {
        self.#putStrings(self.args, argv, buf);
      },
      environ_sizes_get(count, size) {
        const dv = self.#mem();
        dv.setUint32(count, self.env.length, true);
        dv.setUint32(size, self.env.reduce((s, a) => s + enc.encode(a).length + 1, 0), true);
      },
      environ_get(environ, buf) {
        self.#putStrings(self.env, environ, buf);
      },
      clock_res_get(id, ptr) {
        if (id > CLOCK_THREAD_CPUTIME) fail(E.INVAL);
        self.#mem().setBigUint64(ptr, id === CLOCK_REALTIME ? 1000000n : 1000n, true);
      },
      clock_time_get(id, _precision, ptr) {
        self.#mem().setBigUint64(ptr, self.#clock(id), true);
      },
      random_get(buf, len) {
        for (let off = 0; off < len; off += 65536) {
          const tmp = new Uint8Array(Math.min(65536, len - off));
          crypto.getRandomValues(tmp);
          self.#u8().set(tmp, buf + off);
        }
      },
      sched_yield() {},
      proc_exit(code) {
        self.flush();
        throw new WasiExit(code);
      },
      poll_oneoff(inPtr, outPtr, nsubs, neventsPtr) {
        if (nsubs === 0) fail(E.INVAL);
        const dv = self.#mem();
        const clocks = [];
        const ready = [];
        for (let i = 0; i < nsubs; i++) {
          const s = inPtr + 48 * i;
          const userdata = dv.getBigUint64(s, true);
          const tag = dv.getUint8(s + 8);
          if (tag === EVENTTYPE_CLOCK) {
            const id = dv.getUint32(s + 16, true);
            const timeout = dv.getBigUint64(s + 24, true);
            const flags = dv.getUint16(s + 40, true);
            let wait;
            try {
              wait = flags & SUBCLOCKFLAGS_ABSTIME ? timeout - self.#clock(id) : timeout;
            } catch (e) {
              ready.push({ userdata, tag, error: e.errno ?? E.INVAL });
              continue;
            }
            clocks.push({ userdata, tag, wait: wait < 0n ? 0n : wait });
          } else {
            // fd_read / fd_write: the in-memory files and the console never block.
            const fd = dv.getUint32(s + 16, true);
            ready.push({ userdata, tag, error: self.fds.has(fd) ? E.SUCCESS : E.BADF });
          }
        }
        if (!ready.length && clocks.length) {
          const min = clocks.reduce((m, c) => (c.wait < m ? c.wait : m), clocks[0].wait);
          sleepMs(Number(min) / 1e6);
          for (const c of clocks) if (c.wait <= min) ready.push({ userdata: c.userdata, tag: c.tag, error: E.SUCCESS });
        } else {
          for (const c of clocks) if (c.wait === 0n) ready.push({ userdata: c.userdata, tag: c.tag, error: E.SUCCESS });
        }
        const u8 = self.#u8();
        ready.forEach((ev, i) => {
          const o = outPtr + 32 * i;
          u8.fill(0, o, o + 32);
          dv.setBigUint64(o, ev.userdata, true);
          dv.setUint16(o + 8, ev.error, true);
          dv.setUint8(o + 10, ev.tag);
        });
        dv.setUint32(neventsPtr, ready.length, true);
      },

      fd_prestat_get(fd, ptr) {
        const f = self.fds.get(fd);
        if (!f || f.preopen === undefined) fail(E.BADF);
        const dv = self.#mem();
        dv.setUint32(ptr, 0, true);
        dv.setUint32(ptr + 4, enc.encode(f.preopen).length, true);
      },
      fd_prestat_dir_name(fd, ptr, len) {
        const f = self.fds.get(fd);
        if (!f || f.preopen === undefined) fail(E.BADF);
        const name = enc.encode(f.preopen);
        if (len < name.length) fail(E.NAMETOOLONG);
        self.#u8().set(name, ptr);
      },
      fd_fdstat_get(fd, ptr) {
        const f = self.#fd(fd);
        const dv = self.#mem();
        self.#u8().fill(0, ptr, ptr + 24);
        dv.setUint8(ptr, f.kind === "stdio" ? FILETYPE.CHARACTER_DEVICE : f.kind === "dir" ? FILETYPE.DIRECTORY : FILETYPE.REGULAR_FILE);
        dv.setUint16(ptr + 2, f.append ? FDFLAGS_APPEND : 0, true);
        dv.setBigUint64(ptr + 8, f.rights, true);
        dv.setBigUint64(ptr + 16, f.rightsInh, true);
      },
      fd_fdstat_set_flags(fd, flags) {
        self.#fd(fd).append = !!(flags & FDFLAGS_APPEND);
      },
      fd_fdstat_set_rights(fd, base, inh) {
        const f = self.#fd(fd);
        f.rights &= base;
        f.rightsInh &= inh;
      },
      fd_filestat_get(fd, ptr) {
        const f = self.#fd(fd);
        self.#filestat(ptr, f.node, f.kind === "stdio");
      },
      fd_filestat_set_size(fd, size) {
        const f = self.#fd(fd);
        if (f.kind !== "file") fail(f.kind === "dir" ? E.ISDIR : E.INVAL);
        fs.truncate(f.node, Number(size));
      },
      fd_filestat_set_times(fd, atim, mtim, flags) {
        const f = self.#fd(fd);
        if (f.kind !== "stdio") self.#setTimes(f.node, atim, mtim, flags);
      },
      fd_advise(fd) {
        self.#fd(fd);
      },
      fd_allocate(fd, offset, len) {
        const f = self.#fd(fd);
        if (f.kind !== "file") fail(E.BADF);
        const end = Number(offset + len);
        if (end > f.node.size) fs.truncate(f.node, end);
      },
      fd_datasync(fd) {
        self.#fd(fd);
      },
      fd_sync(fd) {
        self.#fd(fd);
      },
      fd_close(fd) {
        self.#fd(fd);
        self.fds.delete(fd);
      },
      fd_renumber(from, to) {
        const f = self.#fd(from);
        self.#fd(to);
        self.fds.set(to, f);
        self.fds.delete(from);
      },
      fd_seek(fd, offset, whence, ptr) {
        const f = self.#fd(fd);
        if (f.kind === "stdio") fail(E.SPIPE);
        const base = whence === 0 ? 0n : whence === 1 ? BigInt(f.pos) : whence === 2 ? BigInt(f.kind === "file" ? f.node.size : 0) : fail(E.INVAL);
        const pos = base + offset;
        if (pos < 0n) fail(E.INVAL);
        if (pos > BigInt(Number.MAX_SAFE_INTEGER)) fail(E.OVERFLOW);
        f.pos = Number(pos);
        self.#mem().setBigUint64(ptr, pos, true);
      },
      fd_tell(fd, ptr) {
        const f = self.#fd(fd);
        if (f.kind === "stdio") fail(E.SPIPE);
        self.#mem().setBigUint64(ptr, BigInt(f.pos), true);
      },
      fd_read(fd, iovs, n, nreadPtr) {
        const f = self.#fd(fd);
        let total = 0;
        if (f.kind === "file") {
          if (!(f.rights & RIGHT_FD_READ)) fail(E.BADF);
          for (const [ptr, len] of self.#iovs(iovs, n)) {
            const got = fs.read(f.node, f.pos, len);
            self.#u8().set(got, ptr);
            f.pos += got.length;
            total += got.length;
            if (got.length < len) break;
          }
        } else if (f.kind === "dir") {
          fail(E.ISDIR);
        } else if (f.n !== 0) {
          fail(E.BADF);
        }
        self.#mem().setUint32(nreadPtr, total, true);
      },
      fd_pread(fd, iovs, n, offset, nreadPtr) {
        const f = self.#fd(fd);
        if (f.kind === "stdio") fail(E.SPIPE);
        if (f.kind === "dir") fail(E.ISDIR);
        if (!(f.rights & RIGHT_FD_READ)) fail(E.BADF);
        let pos = Number(offset);
        let total = 0;
        for (const [ptr, len] of self.#iovs(iovs, n)) {
          const got = fs.read(f.node, pos, len);
          self.#u8().set(got, ptr);
          pos += got.length;
          total += got.length;
          if (got.length < len) break;
        }
        self.#mem().setUint32(nreadPtr, total, true);
      },
      fd_write(fd, iovs, n, nwrittenPtr) {
        const f = self.#fd(fd);
        const u8 = self.#u8();
        const chunks = self.#iovs(iovs, n).map(([ptr, len]) => u8.slice(ptr, ptr + len));
        self.#mem().setUint32(nwrittenPtr, self.#writeTo(f, chunks), true);
      },
      fd_pwrite(fd, iovs, n, offset, nwrittenPtr) {
        const f = self.#fd(fd);
        if (f.kind === "stdio") fail(E.SPIPE);
        if (f.kind === "dir") fail(E.ISDIR);
        if (!(f.rights & RIGHT_FD_WRITE)) fail(E.BADF);
        const u8 = self.#u8();
        let pos = Number(offset);
        let total = 0;
        for (const [ptr, len] of self.#iovs(iovs, n)) {
          fs.write(f.node, pos, u8.slice(ptr, ptr + len));
          pos += len;
          total += len;
        }
        self.#mem().setUint32(nwrittenPtr, total, true);
      },
      fd_readdir(fd, buf, len, cookie, usedPtr) {
        const f = self.#dirFd(fd);
        const dir = f.node;
        const u8 = self.#u8();
        const list = [[".", dir, 1n], ["..", dir.parent, 2n]];
        for (const [name, e] of dir.entries) list.push([name, e.node, BigInt(e.seq) + 2n]);
        let off = 0;
        for (const [name, node, next] of list) {
          if (next <= cookie) continue;
          if (off >= len) break;
          const nb = enc.encode(name);
          const ent = new Uint8Array(24 + nb.length);
          const dv = new DataView(ent.buffer);
          dv.setBigUint64(0, next, true);
          dv.setBigUint64(8, BigInt(node.ino), true);
          dv.setUint32(16, nb.length, true);
          dv.setUint8(20, node.kind === "dir" ? FILETYPE.DIRECTORY : FILETYPE.REGULAR_FILE);
          ent.set(nb, 24);
          const k = Math.min(ent.length, len - off);
          u8.set(ent.subarray(0, k), buf + off);
          off += k;
        }
        self.#mem().setUint32(usedPtr, off, true);
      },

      path_open(dirfd, _dirflags, pathPtr, pathLen, oflags, rightsBase, rightsInh, fdflags, fdPtr) {
        const d = self.#dirFd(dirfd);
        const r = fs.resolve(d.node, self.#str(pathPtr, pathLen));
        let node = r.node;
        if (node) {
          if (oflags & OFLAGS_CREAT && oflags & OFLAGS_EXCL) fail(E.EXIST);
          if ((r.mustDir || oflags & OFLAGS_DIRECTORY) && node.kind !== "dir") fail(E.NOTDIR);
          if (node.kind === "dir" && (oflags & OFLAGS_TRUNC || (rightsBase & RIGHT_FD_WRITE && !(oflags & OFLAGS_DIRECTORY)))) fail(E.ISDIR);
          if (node.kind === "file" && oflags & OFLAGS_TRUNC && node.size > 0) fs.truncate(node, 0);
        } else {
          if (!(oflags & OFLAGS_CREAT)) fail(E.NOENT);
          if (oflags & OFLAGS_DIRECTORY) fail(E.INVAL);
          if (r.mustDir || !r.parent) fail(E.ISDIR);
          node = fs.createFile(r.parent, r.name);
        }
        const fd = self.#allocFd({ kind: node.kind, node, pos: 0, append: !!(fdflags & FDFLAGS_APPEND), rights: rightsBase, rightsInh });
        self.#mem().setUint32(fdPtr, fd, true);
      },
      path_create_directory(fd, pathPtr, pathLen) {
        fs.mkdir(self.#dirFd(fd).node, self.#str(pathPtr, pathLen));
      },
      path_remove_directory(fd, pathPtr, pathLen) {
        fs.rmdir(self.#dirFd(fd).node, self.#str(pathPtr, pathLen));
      },
      path_unlink_file(fd, pathPtr, pathLen) {
        fs.unlinkFile(self.#dirFd(fd).node, self.#str(pathPtr, pathLen));
      },
      path_rename(fd1, p1, l1, fd2, p2, l2) {
        fs.rename(self.#dirFd(fd1).node, self.#str(p1, l1), self.#dirFd(fd2).node, self.#str(p2, l2));
      },
      path_filestat_get(fd, _flags, pathPtr, pathLen, ptr) {
        const r = fs.resolve(self.#dirFd(fd).node, self.#str(pathPtr, pathLen));
        if (!r.node) fail(E.NOENT);
        if (r.mustDir && r.node.kind !== "dir") fail(E.NOTDIR);
        self.#filestat(ptr, r.node);
      },
      path_filestat_set_times(fd, _flags, pathPtr, pathLen, atim, mtim, flags) {
        const r = fs.resolve(self.#dirFd(fd).node, self.#str(pathPtr, pathLen));
        if (!r.node) fail(E.NOENT);
        self.#setTimes(r.node, atim, mtim, flags);
      },
      path_readlink(fd, pathPtr, pathLen) {
        const r = fs.resolve(self.#dirFd(fd).node, self.#str(pathPtr, pathLen));
        fail(r.node ? E.INVAL : E.NOENT);
      },
    };
    const wrapped = {};
    for (const [name, fn] of Object.entries(impl)) {
      wrapped[name] = (...a) => {
        try {
          return fn(...a) ?? E.SUCCESS;
        } catch (e) {
          if (e instanceof FsError) return e.errno;
          if (e instanceof WasiExit) throw e;
          console.error(`wasi ${name}:`, e);
          return E.IO;
        }
      };
    }
    // Every other function of the module (sockets, links, proc_raise, ...) answers ENOSYS, so
    // that a module importing one still links.
    return new Proxy(wrapped, {
      get(target, prop) {
        if (prop in target || typeof prop !== "string") return target[prop];
        return () => E.NOSYS;
      },
    });
  }

  #putStrings(list, ptrs, buf) {
    const dv = this.#mem();
    const u8 = this.#u8();
    let off = buf;
    list.forEach((s, i) => {
      const b = enc.encode(s);
      dv.setUint32(ptrs + 4 * i, off, true);
      u8.set(b, off);
      u8[off + b.length] = 0;
      off += b.length + 1;
    });
  }
}
