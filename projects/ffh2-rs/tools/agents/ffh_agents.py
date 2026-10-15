#!/usr/bin/env python3
"""Shared infrastructure for parallel agents working on ffh2-rs (see docs/AGENTS.md).

All state lives outside the repository, in $FFH2_CACHE (default /home/user/.cache/ffh2-rs):

  locks/build.lock        one build at a time, machine-wide (flock)
  ws/<i>/src              build workspaces: synced copies of projects/ffh2-rs ...
  ws/<i>/target/<prof>    ... each with a persistent cargo target dir per profile, so a build is
  ws/<i>/state.json           incremental against whatever that workspace built last
  build/                  the workspace being built, renamed here for the build: every build runs
                            at this one path, so the same sources give byte-identical binaries
                            whichever workspace built them (the path is compiled in)
  out/<prof>/<key>/       build outputs (key = hash of the source files + profile + toolchain):
                            ffh, ffh-stress, ffh-play  wrappers that run bin/<name> on data/
                            bin/, data/ (hard links into datastore/), info.json
  revs/<prof>/<commit>    commit -> output key
  datastore/<id>/         snapshots of data/ by content
  runs/                   run cache: (binary + data + args + FFH_* env) -> stdout/stderr
  slots/                  CPU slot lock files (machine-wide limit on concurrent simulations)
  hashmemo/               file hashes memoized by (inode, size, mtime, ctime)

Every subcommand is std-only Python; the .sh files next to this one are thin wrappers.
"""
import argparse
import contextlib
import fcntl
import hashlib
import io
import json
import os
import random
import re
import shutil
import subprocess
import sys
import tarfile
import threading
import time

ROOT = os.path.abspath(os.environ.get('FFH2_CACHE') or '/home/user/.cache/ffh2-rs')
PROFILES = ('release', 'checked')
BINS = ('ffh', 'ffh-stress', 'ffh-play')
PROJECT = 'projects/ffh2-rs'
BUILD_RECIPE = 1        # bump when the way outputs are built changes (new output keys)
RUN_CACHE_VERSION = 1   # bump to invalidate every cached run
WRAPPER_MARK = 'ffh2-rs agents build output'
# Subcommands whose output is a timing, or that talk over stdin/sockets: never cached.
NO_CACHE_SUBCOMMANDS = {'bench', 'serve', 'agent'}

DEFAULT_CONFIG = {
    'workspaces': 3,        # build workspaces (each holds a cargo target dir per profile used)
    'keep_outputs': 12,     # build outputs kept per profile (least recently used pruned first)
    'runs_max_mb': 512,     # run cache size cap
    'runs_max_days': 30,    # run cache entries unused this long are dropped
    'max_total_gb': 12,     # whole cache cap: least recently used workspace target dirs go first
    'slots': 0,             # CPU slots; 0 = nproc
}


def config():
    c = dict(DEFAULT_CONFIG)
    try:
        with open(os.path.join(ROOT, 'config.json')) as f:
            c.update(json.load(f))
    except (OSError, ValueError):
        pass
    return c


def log(msg):
    print('[agents] ' + msg, file=sys.stderr, flush=True)


class Fail(Exception):
    pass


# ---------------------------------------------------------------------------------------------
# small helpers

def atomic_write(path, data, mode=0o644):
    d = os.path.dirname(path)
    os.makedirs(d, exist_ok=True)
    tmp = os.path.join(d, '.tmp-%d-%d-%s' % (os.getpid(), threading.get_ident(), os.path.basename(path)))
    with open(tmp, 'wb') as f:
        f.write(data)
    os.chmod(tmp, mode)
    os.replace(tmp, path)


def read_json(path, default=None):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def touch(path):
    try:
        os.utime(path)
    except FileNotFoundError:
        try:
            open(path, 'a').close()
        except OSError:
            pass
    except OSError:
        pass


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def blob_id(data):
    """Git's blob id of these bytes (same as `git hash-object`)."""
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


def stat_sig(st):
    return '%d %d %d %d %d' % (st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns, st.st_ctime_ns)


def memo_get(name, sig):
    try:
        with open(os.path.join(ROOT, 'hashmemo', name)) as f:
            parts = f.read().split('\n')
        if parts[0] == sig:
            return parts[1]
    except (OSError, IndexError):
        pass
    return None


def memo_put(name, sig, value):
    if os.path.isdir(ROOT):
        try:
            atomic_write(os.path.join(ROOT, 'hashmemo', name), ('%s\n%s\n' % (sig, value)).encode())
        except OSError:
            pass


def file_sha256_memo(path):
    """sha256 of a file, memoized on disk by (device, inode, size, mtime, ctime)."""
    rp = os.path.realpath(path)
    sig = stat_sig(os.stat(rp))
    name = 'f-' + hashlib.sha1(rp.encode()).hexdigest()
    h = memo_get(name, sig)
    if h is None:
        h = sha256_file(rp)
        memo_put(name, sig, h)
    return h


def dir_size(path):
    total = 0
    for dirpath, dirnames, filenames in os.walk(path):
        for fn in filenames:
            try:
                total += os.lstat(os.path.join(dirpath, fn)).st_blocks * 512
            except OSError:
                pass
    return total


def rmtree(path):
    shutil.rmtree(path, ignore_errors=True)


def fmt_bytes(n):
    for unit in ('B', 'KB', 'MB', 'GB'):
        if n < 1024 or unit == 'GB':
            return '%.1f %s' % (n, unit) if unit != 'B' else '%d B' % n
        n /= 1024.0


def git(repo, *args, check=True, binary=False):
    r = subprocess.run(['git', '-C', repo] + list(args), capture_output=True)
    if check and r.returncode != 0:
        raise Fail('git %s failed: %s' % (' '.join(args), r.stderr.decode(errors='replace').strip()))
    return r.stdout if binary else r.stdout.decode(errors='replace')


@contextlib.contextmanager
def flocked(path, wait_msg=None):
    """Exclusive flock on `path` for the duration of the block (released on close/exit/crash)."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    f = open(path, 'a+')
    try:
        try:
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            if wait_msg:
                log(wait_msg() if callable(wait_msg) else wait_msg)
            fcntl.flock(f, fcntl.LOCK_EX)
        yield f
    finally:
        f.close()


# ---------------------------------------------------------------------------------------------
# source trees (the files of projects/ffh2-rs at a commit or in a worktree)

class Tree:
    """relpath (under projects/ffh2-rs) -> (kind 'f'|'x'|'l', git blob id, size), plus contents."""

    def __init__(self, label):
        self.label = label
        self.files = {}
        self._content = {}
        self._loader = None

    def add(self, rel, kind, data):
        self.files[rel] = (kind, blob_id(data), len(data))
        self._content[rel] = data

    def content(self, rel):
        if rel not in self._content:
            self._content[rel] = self._loader(rel)
        return self._content[rel]

    def manifest_hash(self):
        h = hashlib.sha256()
        for rel in sorted(self.files):
            kind, oid, size = self.files[rel]
            h.update(('%s\0%s\0%s\n' % (rel, kind, oid)).encode())
        return h.hexdigest()

    def data_id(self):
        h = hashlib.sha256()
        for rel in sorted(self.files):
            if rel.startswith('data/'):
                h.update(('%s\0%s\n' % (rel[5:], self.files[rel][1])).encode())
        return h.hexdigest()[:32]


def tree_from_rev(repo, commit):
    tar = git(repo, 'archive', '--format=tar', commit, '--', PROJECT, binary=True)
    t = Tree('rev ' + commit)
    prefix = PROJECT + '/'
    with tarfile.open(fileobj=io.BytesIO(tar)) as tf:
        for m in tf:
            if not m.name.startswith(prefix) or m.isdir():
                continue
            rel = m.name[len(prefix):]
            if m.issym():
                t.add(rel, 'l', m.linkname.encode())
            elif m.isfile():
                t.add(rel, 'x' if m.mode & 0o111 else 'f', tf.extractfile(m).read())
    if 'Cargo.toml' not in t.files:
        raise Fail('%s has no %s/Cargo.toml' % (commit, PROJECT))
    return t


def tree_from_worktree(top):
    """The files git would see in the worktree: tracked (as on disk) + untracked, not ignored."""
    out = git(top, 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', PROJECT, binary=True)
    t = Tree('tree ' + top)
    prefix = PROJECT + '/'
    for name in sorted(set(out.decode('utf-8', 'surrogateescape').split('\0'))):
        if not name.startswith(prefix):
            continue
        path = os.path.join(top, name)
        rel = name[len(prefix):]
        try:
            st = os.lstat(path)
        except FileNotFoundError:
            continue        # deleted in the worktree (or outside a sparse checkout)
        if os.path.islink(path):
            t.add(rel, 'l', os.readlink(path).encode())
        elif os.path.isfile(path):
            with open(path, 'rb') as f:
                t.add(rel, 'x' if st.st_mode & 0o111 else 'f', f.read())
    if 'Cargo.toml' not in t.files:
        raise Fail('%s has no %s/Cargo.toml' % (top, PROJECT))
    return t


def sync_tree(tree, dst):
    """Makes `dst` hold exactly the tree's files, rewriting only files whose content or kind
    differs: unchanged files keep their mtime, so cargo rebuilds only crates whose sources
    changed. Returns the changed paths."""
    os.makedirs(dst, exist_ok=True)
    existing = {}
    for dirpath, dirnames, filenames in os.walk(dst):
        for name in filenames + [d for d in dirnames if os.path.islink(os.path.join(dirpath, d))]:
            p = os.path.join(dirpath, name)
            existing[os.path.relpath(p, dst)] = p
    changed = []
    for rel, p in existing.items():
        if rel not in tree.files:
            os.unlink(p)
            changed.append(rel)
    for rel, (kind, oid, size) in tree.files.items():
        p = os.path.join(dst, rel)
        if rel in existing:
            if kind == 'l':
                if os.path.islink(p) and blob_id(os.readlink(p).encode()) == oid:
                    continue
            elif not os.path.islink(p):
                st = os.lstat(p)
                if st.st_size == size and bool(st.st_mode & 0o111) == (kind == 'x'):
                    with open(p, 'rb') as f:
                        if blob_id(f.read()) == oid:
                            continue
            os.unlink(p)
        data = tree.content(rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        if kind == 'l':
            os.symlink(data.decode(), p)
        else:
            atomic_write(p, data, 0o755 if kind == 'x' else 0o644)
        changed.append(rel)
    for dirpath, dirnames, filenames in os.walk(dst, topdown=False):
        if dirpath != dst and not os.listdir(dirpath):
            os.rmdir(dirpath)
    return changed


def crate_graph(tree):
    """crate dir name -> set of path-dependency crate dir names (from crates/*/Cargo.toml)."""
    deps = {}
    for rel in tree.files:
        m = re.match(r'crates/([^/]+)/Cargo\.toml$', rel)
        if m:
            text = tree.content(rel).decode(errors='replace')
            deps[m.group(1)] = set(re.findall(r'path\s*=\s*"\.\./([^"/]+)"', text))
    return deps


def crate_weights(tree):
    w = {}
    for rel, (kind, oid, size) in tree.files.items():
        m = re.match(r'crates/([^/]+)/.*\.rs$', rel)
        if m:
            w[m.group(1)] = w.get(m.group(1), 0) + size
    return w


def manifest_of(d):
    return {k: tuple(v) for k, v in (d or {}).items()}


def changed_paths(a, b):
    return {rel for rel in set(a) | set(b) if a.get(rel) != b.get(rel)}


def rebuild_cost(changed, graph, weights):
    """Rough cost (bytes of Rust source recompiled) of a build in which the files `changed` are
    newer than the target dir's last build (None: the target dir was never built)."""
    total = sum(weights.values())
    if changed is None:
        return total + 1
    dirty = set()
    for rel in changed:
        m = re.match(r'crates/([^/]+)/', rel)
        if m:
            dirty.add(m.group(1))
        elif rel in ('Cargo.toml', 'Cargo.lock'):
            dirty |= set(graph)
    rdeps = {}
    for c, ds in graph.items():
        for d in ds:
            rdeps.setdefault(d, set()).add(c)
    stack = list(dirty)
    while stack:
        for d in rdeps.get(stack.pop(), ()):
            if d not in dirty:
                dirty.add(d)
                stack.append(d)
    return sum(weights.get(c, 0) for c in dirty)


# ---------------------------------------------------------------------------------------------
# build service

_TOOLCHAIN = None


def toolchain_id():
    global _TOOLCHAIN
    if _TOOLCHAIN is None:
        try:
            v = subprocess.run(['rustc', '-vV'], capture_output=True, text=True).stdout
            v += subprocess.run(['cargo', '-V'], capture_output=True, text=True).stdout
        except OSError as e:
            raise Fail('rustc/cargo not found: %s' % e)
        _TOOLCHAIN = v.strip()
    return _TOOLCHAIN


def output_key(tree, profile):
    h = hashlib.sha256(('recipe %d\nprofile %s\n%s\n%s\n' % (
        BUILD_RECIPE, profile, toolchain_id(), tree.manifest_hash())).encode())
    return h.hexdigest()[:32]


def out_dir(profile, key):
    return os.path.join(ROOT, 'out', profile, key)


BIN_PACKAGE = {'ffh': 'ffh-cli', 'ffh-stress': 'ffh-stress', 'ffh-play': 'ffh-play'}


def output_ok(path, bins=BINS):
    return os.path.isfile(os.path.join(path, 'info.json')) and all(
        os.path.isfile(os.path.join(path, b)) and os.path.isfile(os.path.join(path, 'bin', b)) for b in bins)


def cargo_env(target_dir, profile):
    env = {k: v for k, v in os.environ.items()
           if not (k.startswith('CARGO_PROFILE_') or k in (
               'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS',
               'CARGO_BUILD_RUSTFLAGS', 'CARGO_INCREMENTAL', 'RUSTC_WRAPPER', 'CARGO_BUILD_JOBS'))}
    env['CARGO_TARGET_DIR'] = target_dir
    if profile == 'checked':
        env['CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS'] = 'true'
        env['CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS'] = 'true'
    return env


def ws_dir(i):
    return os.path.join(ROOT, 'ws', str(i))


def ws_state(i):
    return read_json(os.path.join(ws_dir(i), 'state.json'), {}) or {}


def active_dir():
    return os.path.join(ROOT, 'build')


def park_active():
    """Moves a workspace left at build/ (by a build that crashed) back to ws/<i>. Build lock held."""
    a = active_dir()
    if not os.path.isdir(a):
        return
    try:
        with open(os.path.join(a, '.ws')) as f:
            i = int(f.read().strip())
    except (OSError, ValueError):
        i = None
    if i is None or os.path.exists(ws_dir(i)):
        rmtree(a)
    else:
        os.makedirs(os.path.dirname(ws_dir(i)), exist_ok=True)
        os.rename(a, ws_dir(i))


@contextlib.contextmanager
def activated(i):
    """Workspace i, renamed to build/ for the duration of the block. Build lock held."""
    park_active()
    os.makedirs(ws_dir(i), exist_ok=True)
    a = active_dir()
    os.rename(ws_dir(i), a)
    atomic_write(os.path.join(a, '.ws'), b'%d\n' % i)
    try:
        yield a
    finally:
        os.rename(a, ws_dir(i))


CHEAP_BUILD = 0.5   # a build recompiling less than this share of the Rust source is "incremental"


def choose_workspace(tree, profile, label):
    """Picks the workspace to build `tree` in. Cargo rebuilds a crate when one of its files is
    newer than its last build: for this profile, every file rewritten since the profile last
    built there (built -> src now) plus every file the sync rewrites (src now -> tree).

    If some workspace makes the build incremental (an ffh-ai/cli/stress/play change against what
    it holds), take the cheapest. Otherwise (ffh-core or ffh-data changed) the build costs nearly
    a full one anywhere, so it goes where this label built last, else to the least recently used
    workspace, leaving the warm ones (e.g. at the integration tip) warm for everyone else."""
    n = max(1, int(config()['workspaces']))
    graph = crate_graph(tree)
    weights = crate_weights(tree)
    total = sum(weights.values())
    cands = []
    for i in range(n):
        st = ws_state(i)
        prof = st.get('profiles', {}).get(profile)
        changed = None
        if prof and os.path.isdir(os.path.join(ws_dir(i), 'target', profile)):
            built = manifest_of(prof.get('manifest'))
            src = manifest_of(st.get('src_manifest'))
            changed = changed_paths(built, src) | changed_paths(src, tree.files)
        cost = rebuild_cost(changed, graph, weights)
        cands.append((cost, 0 if st.get('label') == label else 1, st.get('last_used', 0), i))
    cheap = [c for c in cands if c[0] < CHEAP_BUILD * total]
    if cheap:
        cost, _, _, i = min(cheap)
    else:
        cost, _, _, i = min(cands, key=lambda c: (c[1], c[2], c[3]))
    return i, cost, total


def store_data(tree):
    data_id = tree.data_id()
    d = os.path.join(ROOT, 'datastore', data_id)
    if not os.path.isdir(d):
        tmp = d + '.tmp-%d' % os.getpid()
        rmtree(tmp)
        for rel in tree.files:
            if rel.startswith('data/'):
                atomic_write(os.path.join(tmp, rel[5:]), tree.content(rel))
        os.makedirs(tmp, exist_ok=True)
        os.replace(tmp, d)
    return data_id, d


WRAPPER = '''#!/bin/sh
# {mark} (tools/agents, docs/AGENTS.md): {name} from {source}, profile {profile}
# exe-sha256: {exe}
# data-id: {data}
d='{dir}'
touch "$d/.used" 2>/dev/null
FFH_DATA_DIR="$d/data"
export FFH_DATA_DIR
exec "$d/bin/{name}" "$@"
'''


def make_output(tree, profile, key, target_dir, label, bins, extra):
    """Creates the output dir for `key` with `bins` copied from the target dir, or adds `bins` to
    an existing (partial) one -- same key, so same sources. Writers hold the build lock."""
    final = out_dir(profile, key)
    exists = os.path.isfile(os.path.join(final, 'info.json'))
    work = final if exists else final + '.tmp-%d' % os.getpid()
    if not exists:
        rmtree(work)
    os.makedirs(os.path.join(work, 'bin'), exist_ok=True)
    info = read_json(os.path.join(work, 'info.json'), {}) if exists else {}
    data_id, store = store_data(tree)
    if not exists:
        for dirpath, dirnames, filenames in os.walk(store):
            for fn in filenames:
                s = os.path.join(dirpath, fn)
                t = os.path.join(work, 'data', os.path.relpath(s, store))
                os.makedirs(os.path.dirname(t), exist_ok=True)
                try:
                    os.link(s, t)
                except OSError:
                    shutil.copyfile(s, t)
    exe = dict(info.get('exe_sha256') or {})
    for b in bins:
        dst = os.path.join(work, 'bin', b)
        tmp = dst + '.tmp-%d' % os.getpid()
        shutil.copyfile(os.path.join(target_dir, 'release', b), tmp)
        os.chmod(tmp, 0o755)
        os.replace(tmp, dst)
        exe[b] = sha256_file(dst)
        atomic_write(os.path.join(work, b), WRAPPER.format(
            mark=WRAPPER_MARK, name=b, source=label, profile=profile, exe=exe[b], data=data_id,
            dir=final).encode(), 0o755)
    if not exists:
        info = dict(extra, key=key, profile=profile, source=label, built_at=time.time(), data_id=data_id,
                    manifest=tree.manifest_hash(), toolchain=toolchain_id(), tests=None)
    info['exe_sha256'] = exe
    atomic_write(os.path.join(work, 'info.json'), json.dumps(info, indent=1).encode())
    touch(os.path.join(work, '.used'))
    if not exists:
        os.replace(work, final)
    return final


def owner_text():
    """Who holds the build lock (probed without waiting), or 'free'."""
    p = os.path.join(ROOT, 'locks', 'build.lock')
    try:
        with open(p, 'a+') as f:
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return 'free'
    except BlockingIOError:
        pass
    except OSError:
        return 'free'
    try:
        with open(os.path.join(ROOT, 'locks', 'build.owner')) as f:
            return f.read().strip() or '?'
    except OSError:
        return '?'


def build_in(wsd, i, tree, profile, label, key, missing, test, test_args, full_suite, cost, total, jobs):
    """Syncs `tree` into the activated workspace `wsd`, builds `missing` binaries into the output
    and/or runs the tests. Returns tests_ok (None without tests)."""
    out = out_dir(profile, key)
    src = os.path.join(wsd, 'src')
    target = os.path.join(wsd, 'target', profile)
    state = read_json(os.path.join(wsd, 'state.json'), {}) or {}
    t0 = time.time()
    changed = sync_tree(tree, src)
    manifest = {k: list(v) for k, v in tree.files.items()}
    state.update(src_manifest=manifest, label=label, last_used=time.time())
    atomic_write(os.path.join(wsd, 'state.json'), json.dumps(state).encode())
    env = cargo_env(target, profile)
    jflag = ['-j', str(jobs)] if jobs else []

    def record(ok):
        state.setdefault('profiles', {})[profile] = {'manifest': manifest, 'built_at': time.time(), 'ok': ok}
        atomic_write(os.path.join(wsd, 'state.json'), json.dumps(state).encode())
    if missing:
        log('building %s of %s (%s) in workspace %d: %d files changed, ~%d%% of the Rust source to recompile' % (
            '/'.join(missing), label, profile, i, len(changed), 100 * cost // max(1, total)))
        pkgs = [] if len(missing) == len(BINS) else [a for b in missing for a in ('-p', BIN_PACKAGE[b])]
        r = subprocess.run(['cargo', 'build', '--release'] + pkgs + jflag, cwd=src, env=env, stdout=sys.stderr)
        record(r.returncode == 0)
        if r.returncode != 0:
            raise Fail('cargo build failed for %s' % label)
        secs = time.time() - t0
        make_output(tree, profile, key, target, label, missing, {'build_secs': round(secs, 1), 'workspace': i})
        log('built in %.1f s: %s' % (secs, out))
    if not test:
        return None
    cmd = ['cargo', 'test', '--release'] + jflag + list(test_args)
    log('running %s in workspace %d' % (' '.join(cmd), i))
    t1 = time.time()
    r = subprocess.run(cmd, cwd=src, env=env, stdout=sys.stderr)
    record(r.returncode == 0)
    ok = r.returncode == 0
    log('tests %s in %.1f s' % ('passed' if ok else 'FAILED', time.time() - t1))
    if ok and full_suite:
        info = read_json(os.path.join(out, 'info.json'), {})
        info['tests'] = {'passed': True, 'at': time.time()}
        atomic_write(os.path.join(out, 'info.json'), json.dumps(info, indent=1).encode())
    return ok


def build(tree, profile, label, bins=BINS, test=False, test_args=(), force_test=False, jobs=None):
    """Builds `bins` of `tree` unless an output for the same content already has them; with
    `test`, also runs `cargo test --release` there. Returns (output dir, tests_ok or None)."""
    if profile not in PROFILES:
        raise Fail('unknown profile %r (release|checked)' % profile)
    bins = [b for b in BINS if b in bins]
    key = output_key(tree, profile)
    out = out_dir(profile, key)
    full_suite = test and not test_args
    if output_ok(out, bins):
        info = read_json(os.path.join(out, 'info.json'), {})
        if not test:
            touch(os.path.join(out, '.used'))
            log('up to date: %s (built %s from %s)' % (
                out, time.strftime('%m-%d %H:%M', time.localtime(info.get('built_at', 0))), info.get('source')))
            return out, None
        if full_suite and not force_test and (info.get('tests') or {}).get('passed'):
            touch(os.path.join(out, '.used'))
            log('tests already passed for this exact content and profile (%s); --force-test reruns them' % out)
            return out, True
    lock = os.path.join(ROOT, 'locks', 'build.lock')
    with flocked(lock, wait_msg=lambda: 'waiting for the build lock (held by: %s)' % owner_text()):
        atomic_write(os.path.join(ROOT, 'locks', 'build.owner'),
                     ('%s, pid %d, since %s\n' % (label, os.getpid(), time.strftime('%H:%M:%S'))).encode())
        tests_ok = None
        missing = [b for b in bins if not output_ok(out, [b])]
        if missing or test:
            park_active()
            i, cost, total = choose_workspace(tree, profile, label)
            with activated(i) as wsd:
                tests_ok = build_in(wsd, i, tree, profile, label, key, missing, test, test_args, full_suite,
                                    cost, total, jobs)
        touch(os.path.join(out, '.used'))
        gc(quiet=True, keep=out)
    return out, tests_ok


def resolve_rev(repo, rev):
    out = git(repo, 'rev-parse', '--verify', '--quiet', '--end-of-options', rev + '^{commit}', check=False).strip()
    if not re.fullmatch(r'[0-9a-f]{40,64}', out):
        raise Fail('unknown revision %r in %s' % (rev, repo))
    return out


def repo_of(path):
    top = git(path, 'rev-parse', '--show-toplevel', check=False).strip()
    if not top:
        raise Fail('%s is not inside a git worktree' % path)
    return top


def default_repo():
    r = subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True, text=True)
    if r.returncode == 0 and r.stdout.strip():
        return r.stdout.strip()
    return repo_of(os.path.dirname(os.path.abspath(__file__)))


def build_rev(rev, profile='release', bins=BINS, test=False, test_args=(), force_test=False, repo=None, jobs=None):
    repo = repo or default_repo()
    commit = resolve_rev(repo, rev)
    idx = os.path.join(ROOT, 'revs', profile, commit)
    try:
        with open(idx) as f:
            out = out_dir(profile, f.read().strip())
        if output_ok(out, bins) and not test:
            touch(os.path.join(out, '.used'))
            log('up to date: %s (%s)' % (out, read_json(os.path.join(out, 'info.json'), {}).get('source')))
            return out, None
    except OSError:
        pass
    tree = tree_from_rev(repo, commit)
    out, ok = build(tree, profile, 'rev %s' % commit[:12], bins, test, test_args, force_test, jobs)
    atomic_write(idx, (os.path.basename(out) + '\n').encode())
    return out, ok


def build_tree(path, profile='release', bins=BINS, test=False, test_args=(), force_test=False, jobs=None):
    top = repo_of(path)
    tree = tree_from_worktree(top)
    return build(tree, profile, 'tree %s' % top, bins, test, test_args, force_test, jobs)


# ---------------------------------------------------------------------------------------------
# CPU slots

def slot_count():
    for v in (os.environ.get('FFH2_SLOTS'), config().get('slots')):
        try:
            if v and int(v) > 0:
                return int(v)
        except (TypeError, ValueError):
            pass
    return os.cpu_count() or 1


def slots_enabled():
    return os.path.isdir(ROOT) and not os.environ.get('FFH2_NO_SLOTS')


def acquire_slots(n, label='', verbose=False):
    """Blocks until this process holds `n` of the machine's CPU slots; returns the open lock
    files (closing them releases the slots). Waiters queue on queue.lock, so a request for many
    slots (e.g. --exclusive) is not starved by a stream of single-slot requests."""
    total = slot_count()
    n = max(1, min(n, total))
    d = os.path.join(ROOT, 'slots')
    os.makedirs(d, exist_ok=True)
    held = {}
    t0 = time.time()
    warned = False
    q = open(os.path.join(d, 'queue.lock'), 'a+')
    try:
        fcntl.flock(q, fcntl.LOCK_EX)
        order = list(range(total))
        random.shuffle(order)
        while True:
            for i in order:
                if len(held) == n:
                    break
                if i in held:
                    continue
                f = open(os.path.join(d, '%d.lock' % i), 'a+')
                try:
                    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    f.close()
                    continue
                f.seek(0)
                f.truncate()
                f.write('%d %d %s\n' % (os.getpid(), time.time(), label[:200]))
                f.flush()
                held[i] = f
            if len(held) == n:
                break
            if verbose and not warned and time.time() - t0 > 2:
                log('waiting for %d of %d CPU slots (%s)' % (n, total, slot_summary()))
                warned = True
            time.sleep(0.1 + random.random() * 0.1)
    except BaseException:
        for f in held.values():
            f.close()
        raise
    finally:
        q.close()
    if verbose and warned:
        log('got %d CPU slot(s) after %.0f s' % (n, time.time() - t0))
    return list(held.values())


@contextlib.contextmanager
def cpu_slots(n=1, label='', verbose=False):
    if n <= 0 or os.environ.get('FFH2_SLOT_HELD') or not slots_enabled():
        yield []
        return
    fs = acquire_slots(n, label, verbose)
    try:
        yield fs
    finally:
        for f in fs:
            f.close()


def slot_holders():
    """[(slot, holder text or None)] -- probes each slot without waiting."""
    d = os.path.join(ROOT, 'slots')
    res = []
    for i in range(slot_count()):
        p = os.path.join(d, '%d.lock' % i)
        holder = None
        try:
            with open(p, 'a+') as f:
                try:
                    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    f.seek(0)
                    holder = f.read().strip() or '?'
        except OSError:
            pass
        res.append((i, holder))
    return res


def slot_summary():
    h = slot_holders()
    return '%d busy' % sum(1 for _, x in h if x)


# ---------------------------------------------------------------------------------------------
# run cache

def cache_enabled():
    return os.path.isdir(ROOT) and not os.environ.get('FFH2_NO_CACHE')


PATH_BYTES = set(b'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-+@~/')


def baked_data_dir(path, exe_sha):
    """The data dir a cargo-built ffh binary loads by default: CARGO_MANIFEST_DIR of ffh-data
    (an absolute path compiled into the binary) + /../../data."""
    name = 'd-' + exe_sha
    hit = memo_get(name, 'baked')
    if hit is not None:
        return hit or None
    with open(path, 'rb') as f:
        data = f.read()
    cands = set()
    for m in re.finditer(rb'/crates/ffh-data', data):
        start = m.start()
        while start > 0 and data[start - 1] in PATH_BYTES:
            start -= 1
        run = data[start:m.start()].decode('latin-1')
        for i, ch in enumerate(run):
            if ch == '/' and os.path.isfile(os.path.join(run[i:], 'Cargo.toml')):
                cands.add(os.path.join(run[i:], 'data'))
                break
    found = cands.pop() if len(cands) == 1 else ''
    memo_put(name, 'baked', found)
    return found or None


def dir_data_id(d):
    """Same id as Tree.data_id() for a data dir on disk (memoized by the files' stat)."""
    entries = []
    for dirpath, dirnames, filenames in os.walk(d):
        for fn in filenames:
            p = os.path.join(dirpath, fn)
            entries.append((os.path.relpath(p, d), p))
    entries.sort()
    sig = hashlib.sha1('\n'.join('%s %s' % (rel, stat_sig(os.stat(p))) for rel, p in entries).encode()).hexdigest()
    name = 'dir-' + hashlib.sha1(os.path.realpath(d).encode()).hexdigest()
    hit = memo_get(name, sig)
    if hit:
        return hit
    h = hashlib.sha256()
    for rel, p in entries:
        with open(p, 'rb') as f:
            h.update(('%s\0%s\n' % (rel, blob_id(f.read()))).encode())
    v = h.hexdigest()[:32]
    memo_put(name, sig, v)
    return v


def bin_identity(b):
    """What determines a binary's behaviour: (identity dict, None) or (None, why not cacheable)."""
    p = b if os.sep in b else shutil.which(b)
    if not p or not os.path.isfile(p):
        return None, 'binary %s not found' % b
    with open(p, 'rb') as f:
        head = f.read(4096)
    if head.startswith(b'#!'):
        text = head.decode('utf-8', 'replace')
        exe = re.search(r'^# exe-sha256: ([0-9a-f]{64})$', text, re.M)
        data = re.search(r'^# data-id: ([0-9a-f]+)$', text, re.M)
        if WRAPPER_MARK in text and exe and data:
            return {'exe': exe.group(1), 'data': data.group(1)}, None
        return None, '%s is a script without a build identity' % p
    if head[:4] != b'\x7fELF':
        return None, '%s is not an ELF binary' % p
    exe = file_sha256_memo(p)
    ddir = os.environ.get('FFH_DATA_DIR') or baked_data_dir(p, exe)
    if not ddir or not os.path.isdir(ddir):
        return None, 'cannot tell which data dir %s loads (set FFH_DATA_DIR)' % p
    return {'exe': exe, 'data': dir_data_id(ddir)}, None


def run_key(argv):
    """Cache key of a deterministic run, or (None, reason)."""
    args = argv[1:]
    sub = next((a for a in args if not a.startswith('-')), '')
    if sub in NO_CACHE_SUBCOMMANDS:
        return None, '`%s` is not cacheable (timing or interactive)' % sub
    if any(a in ('--out', '-o') or a.startswith('--out=') for a in args):
        return None, 'writes an --out file'
    ident, why = bin_identity(argv[0])
    if ident is None:
        return None, why
    files = {}
    for a in args:
        v = a.split('=', 1)[1] if a.startswith('--') and '=' in a else a
        if v and os.path.isfile(v):
            files[v] = file_sha256_memo(v)
    env = sorted((k, v) for k, v in os.environ.items() if k.startswith('FFH_') and k != 'FFH_DATA_DIR')
    obj = {'v': RUN_CACHE_VERSION, 'bin': ident, 'args': args, 'env': env, 'files': sorted(files.items())}
    return hashlib.sha256(json.dumps(obj, sort_keys=True).encode()).hexdigest(), None


def run_path(key):
    return os.path.join(ROOT, 'runs', key[:2], key)


def cache_get(key):
    p = run_path(key)
    try:
        with open(p, 'rb') as f:
            head = json.loads(f.readline())
            out = f.read(head['out'])
            err = f.read(head['err'])
        if len(out) != head['out'] or len(err) != head['err']:
            return None
    except (OSError, ValueError, KeyError):
        return None
    touch(p)
    return out, err, head


def cache_put(key, argv, out, err, secs):
    head = {'v': RUN_CACHE_VERSION, 'argv': argv, 'created': time.time(), 'secs': round(secs, 2),
            'out': len(out), 'err': len(err)}
    atomic_write(run_path(key), json.dumps(head).encode() + b'\n' + out + err)


def stats_log(kind, key, secs):
    try:
        with open(os.path.join(ROOT, 'runs', 'stats.log'), 'a') as f:
            f.write('%d\t%s\t%s\t%.2f\n' % (time.time(), kind, key[:16], secs))
    except OSError:
        pass


def _exec(argv, stream):
    t0 = time.time()
    if not stream:
        r = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True)
        return r.returncode, r.stdout, r.stderr, time.time() - t0
    p = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    bufs = ([], [])

    def pump(src, dst, buf):
        while True:
            chunk = os.read(src.fileno(), 65536)
            if not chunk:
                break
            buf.append(chunk)
            dst.write(chunk)
            dst.flush()
    ts = [threading.Thread(target=pump, args=(p.stdout, sys.stdout.buffer, bufs[0])),
          threading.Thread(target=pump, args=(p.stderr, sys.stderr.buffer, bufs[1]))]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    rc = p.wait()
    return rc, b''.join(bufs[0]), b''.join(bufs[1]), time.time() - t0


def run_cached(argv, cache=None, slots=None, stream=False, verbose=False):
    """Runs argv through the run cache and CPU slots. Returns (returncode, stdout, stderr, hit).
    Concurrent identical runs (same key) wait for the first instead of recomputing; only runs
    that exit 0 are stored."""
    argv = list(argv)
    if cache is None:
        cache = cache_enabled()
    if slots is None:
        slots = slots_enabled()
    key = None
    if cache:
        key, why = run_key(argv)
        if key is None and verbose:
            log('not cached: ' + why)
    if key is None:
        with cpu_slots(1 if slots else 0, ' '.join(argv)):
            rc, out, err, _ = _exec(argv, stream)
        return rc, out, err, False
    hit = cache_get(key)
    if hit is None:
        with flocked(os.path.join(ROOT, 'runs', 'locks', key + '.lock')):
            hit = cache_get(key)       # an identical concurrent run may have just finished
            if hit is None:
                with cpu_slots(1 if slots else 0, ' '.join(argv)):
                    rc, out, err, secs = _exec(argv, stream)
                if rc == 0:
                    cache_put(key, argv, out, err, secs)
                    stats_log('store', key, secs)
                    maybe_gc_runs()
                return rc, out, err, False
    out, err, head = hit
    stats_log('hit', key, head.get('secs', 0))
    if verbose:
        log('cache hit %s (the original run took %.1f s)' % (key[:12], head.get('secs', 0)))
    if stream:
        sys.stdout.buffer.write(out)
        sys.stdout.buffer.flush()
        sys.stderr.buffer.write(err)
        sys.stderr.buffer.flush()
    return 0, out, err, True


def import_run(argv, out, err=b'', secs=0.0, overwrite=False):
    """Store the output of a deterministic run played elsewhere (e.g. a fan-out worker) into the
    local run cache, keyed by the LOCAL binary argv[0] -- so a later identical run here is a hit.
    Correct whenever the game is deterministic: the stored stdout equals what the local binary
    would print (bit-for-bit but for the timing fields, as with any cached game). Returns
    (key, None) on success, (None, reason) if the argv is not cacheable. Does nothing if an entry
    for the key already exists unless overwrite."""
    if not cache_enabled():
        return None, 'run cache disabled'
    key, why = run_key(list(argv))
    if key is None:
        return None, why
    if not overwrite and cache_get(key) is not None:
        return key, None
    if isinstance(out, str):
        out = out.encode()
    if isinstance(err, str):
        err = err.encode()
    cache_put(key, list(argv), out, err, secs)
    stats_log('import', key, secs)
    return key, None


# ---------------------------------------------------------------------------------------------
# garbage collection

def procs_exe_paths():
    res = set()
    for pid in os.listdir('/proc'):
        if pid.isdigit():
            try:
                res.add(os.readlink('/proc/%s/exe' % pid))
            except OSError:
                pass
    return res


def maybe_gc_runs():
    stamp = os.path.join(ROOT, 'runs', '.gc-stamp')
    try:
        if time.time() - os.stat(stamp).st_mtime < 600:
            return
    except OSError:
        pass
    touch(stamp)
    gc_runs(config())


def gc_runs(cfg, dry=False, quiet=True):
    d = os.path.join(ROOT, 'runs')
    entries = []
    now = time.time()
    removed = 0
    for sub in os.listdir(d) if os.path.isdir(d) else []:
        sd = os.path.join(d, sub)
        if len(sub) != 2 or not os.path.isdir(sd):
            continue
        for fn in os.listdir(sd):
            p = os.path.join(sd, fn)
            try:
                st = os.stat(p)
            except OSError:
                continue
            if fn.startswith('.tmp-'):
                if now - st.st_mtime > 3600 and not dry:
                    os.unlink(p)
                continue
            entries.append((st.st_mtime, st.st_size, p))
    entries.sort()                      # least recently used first
    total = sum(e[1] for e in entries)
    cap = cfg['runs_max_mb'] << 20
    goal = cap if total <= cap else cap * 0.9
    for mtime, size, p in entries:
        if now - mtime <= cfg['runs_max_days'] * 86400 and total <= goal:
            break
        if not dry:
            try:
                os.unlink(p)
            except OSError:
                pass
        total -= size
        removed += 1
    ld = os.path.join(d, 'locks')
    for fn in os.listdir(ld) if os.path.isdir(ld) else []:
        p = os.path.join(ld, fn)
        try:
            if now - os.stat(p).st_mtime < 3600:
                continue
            with open(p, 'a+') as f:
                fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
                if not dry:
                    os.unlink(p)
        except (OSError, BlockingIOError):
            pass
    log_p = os.path.join(d, 'stats.log')
    try:
        if os.stat(log_p).st_size > 8 << 20 and not dry:
            with open(log_p, 'rb') as f:
                f.seek(-(4 << 20), 2)
                tail = f.read()
            atomic_write(log_p, tail[tail.find(b'\n') + 1:])
    except OSError:
        pass
    if removed and not quiet:
        log('run cache: %s %d entries' % ('would drop' if dry else 'dropped', removed))
    return removed


def gc(quiet=False, dry=False, keep=None):
    """Enforces the cache limits. Builds hold the build lock while calling this; gc of outputs
    and workspaces otherwise takes it itself (see cmd_gc)."""
    if not os.path.isdir(ROOT):
        return
    cfg = config()
    now = time.time()
    running = procs_exe_paths()
    say = (lambda m: None) if quiet else log
    # 1. build outputs: keep the most recently used per profile; never one used in the last
    #    30 minutes or whose binary is running.
    live_data = set()
    for prof in PROFILES:
        d = os.path.join(ROOT, 'out', prof)
        if not os.path.isdir(d):
            continue
        outs = []
        for name in os.listdir(d):
            p = os.path.join(d, name)
            if '.tmp-' in name:
                if now - os.stat(p).st_mtime > 3600 and not dry:
                    rmtree(p)
                continue
            try:
                used = os.stat(os.path.join(p, '.used')).st_mtime
            except OSError:
                used = 0
            outs.append((used, p))
        outs.sort(reverse=True)
        for n, (used, p) in enumerate(outs):
            busy = any(x.startswith(p + '/') for x in running)
            if n < cfg['keep_outputs'] or now - used < 1800 or busy or p == keep:
                info = read_json(os.path.join(p, 'info.json'), {})
                live_data.add(info.get('data_id'))
                continue
            say('%s old build output %s' % ('would drop' if dry else 'dropping', p))
            if not dry:
                rmtree(p)
        rd = os.path.join(ROOT, 'revs', prof)
        for c in os.listdir(rd) if os.path.isdir(rd) else []:
            try:
                with open(os.path.join(rd, c)) as f:
                    k = f.read().strip()
            except OSError:
                continue
            if not os.path.isdir(os.path.join(d, k)) and not dry:
                os.unlink(os.path.join(rd, c))
    # 2. data snapshots no output links to
    dd = os.path.join(ROOT, 'datastore')
    for name in os.listdir(dd) if os.path.isdir(dd) else []:
        p = os.path.join(dd, name)
        if name in live_data:
            continue
        if '.tmp-' in name:
            if now - os.stat(p).st_mtime > 3600 and not dry:
                rmtree(p)
            continue
        linked = False
        for dirpath, dirnames, filenames in os.walk(p):
            if any(os.stat(os.path.join(dirpath, fn)).st_nlink > 1 for fn in filenames):
                linked = True
                break
        if not linked and now - os.stat(p).st_mtime > 600:
            say('%s unused data snapshot %s' % ('would drop' if dry else 'dropping', name))
            if not dry:
                rmtree(p)
    # 3. run cache; memoized hashes not rewritten for 30 days (recomputed when needed)
    gc_runs(cfg, dry, quiet)
    md = os.path.join(ROOT, 'hashmemo')
    for name in os.listdir(md) if os.path.isdir(md) and not dry else []:
        try:
            if now - os.stat(os.path.join(md, name)).st_mtime > 30 * 86400:
                os.unlink(os.path.join(md, name))
        except OSError:
            pass
    # 4. workspaces beyond the configured count; then the whole-cache cap: drop the target dirs
    #    of the least recently used workspaces.
    if not dry:
        park_active()
    wd = os.path.join(ROOT, 'ws')
    nws = max(1, int(cfg['workspaces']))
    for name in os.listdir(wd) if os.path.isdir(wd) else []:
        if name.isdigit() and int(name) >= nws:
            say('%s workspace %s (config: %d workspaces)' % ('would drop' if dry else 'dropping', name, nws))
            if not dry:
                rmtree(os.path.join(wd, name))
    total = dir_size(ROOT)
    cap = cfg['max_total_gb'] * (1 << 30)
    if total > cap:
        targets = []
        for i in range(nws):
            st = ws_state(i)
            for prof in PROFILES:
                t = os.path.join(ws_dir(i), 'target', prof)
                if os.path.isdir(t):
                    targets.append((st.get('last_used', 0), t, i, prof))
        targets.sort()
        for used, t, i, prof in targets[:-1]:          # never the most recently used one
            if total <= cap:
                break
            size = dir_size(t)
            say('cache is %s (cap %d GB): %s %s' % (fmt_bytes(total), cfg['max_total_gb'],
                                                   'would drop' if dry else 'dropping', t))
            if not dry:
                rmtree(t)
                st = ws_state(i)
                st.get('profiles', {}).pop(prof, None)
                atomic_write(os.path.join(ws_dir(i), 'state.json'), json.dumps(st).encode())
            total -= size


# ---------------------------------------------------------------------------------------------
# worktree pruning

def proc_refs():
    """[(pid, cmdline, [paths])]: cwd, exe, open files and absolute path arguments of every
    process (but this one)."""
    res = []
    me = os.getpid()
    for pid in os.listdir('/proc'):
        if not pid.isdigit() or int(pid) == me:
            continue
        paths = []
        for name in ('cwd', 'exe'):
            try:
                paths.append(os.readlink('/proc/%s/%s' % (pid, name)))
            except OSError:
                pass
        try:
            for fd in os.listdir('/proc/%s/fd' % pid):
                try:
                    paths.append(os.readlink('/proc/%s/fd/%s' % (pid, fd)))
                except OSError:
                    pass
        except OSError:
            pass
        try:
            with open('/proc/%s/cmdline' % pid, 'rb') as f:
                args = f.read().decode(errors='replace').split('\0')
        except OSError:
            continue
        paths += [a for a in args if a.startswith('/')]
        res.append((int(pid), ' '.join(args)[:120], paths))
    return res


def newest_activity(wt, admin):
    """Newest mtime of the worktree's files and dirs (target/ dirs excluded) and of its HEAD and
    HEAD reflog (commits, checkouts, resets)."""
    newest = 0.0
    for dirpath, dirnames, filenames in os.walk(wt):
        dirnames[:] = [d for d in dirnames if d != 'target']
        if dirpath == wt:
            filenames = [f for f in filenames if f != '.git']     # written once, at creation
        for name in filenames + [dirpath]:
            try:
                newest = max(newest, os.lstat(os.path.join(dirpath, name)).st_mtime)
            except OSError:
                pass
    for rel in ('HEAD', 'logs/HEAD'):
        try:
            newest = max(newest, os.stat(os.path.join(admin, rel)).st_mtime)
        except OSError:
            pass
    return newest


def cargo_targets(wt):
    res = []
    for dirpath, dirnames, filenames in os.walk(wt):
        if os.path.basename(dirpath) == 'target' and 'CACHEDIR.TAG' in filenames:
            res.append(dirpath)
            dirnames[:] = []
    return res


def lock_is_stale(reason):
    m = re.search(r'pid (\d+) start (\d+)', reason or '')
    if not m:
        return False
    try:
        with open('/proc/%s/stat' % m.group(1)) as f:
            fields = f.read().rsplit(')', 1)[1].split()
        return fields[19] != m.group(2)        # field 22 (starttime), counted after the comm
    except (OSError, IndexError):
        return True


def prune_worktrees(idle_minutes=90, dry=False, unlock_stale=False, repo=None):
    repo = repo or os.getcwd()
    common = os.path.realpath(os.path.join(repo, git(repo, 'rev-parse', '--git-common-dir').strip()))
    main = os.path.dirname(common)
    base = os.path.join(main, '.claude', 'worktrees') + '/'
    me = os.path.realpath(git(repo, 'rev-parse', '--show-toplevel').strip())
    entries = []
    cur = {}
    for line in git(main, 'worktree', 'list', '--porcelain').split('\n') + ['']:
        if not line:
            if cur:
                entries.append(cur)
            cur = {}
            continue
        k, _, v = line.partition(' ')
        cur[k] = v or ('(no reason)' if k == 'locked' else '')
    procs = proc_refs()
    now = time.time()
    freed = 0
    for e in entries:
        wt = e.get('worktree', '')
        if not wt.startswith(base):
            continue
        name = os.path.basename(wt)
        if os.path.realpath(wt) == me:
            print('%-28s skip: the worktree this runs from' % name)
            continue
        if 'prunable' in e or not os.path.isdir(wt):
            print('%-28s skip: directory missing (git worktree prune cleans the entry)' % name)
            continue
        users = [(pid, cmd) for pid, cmd, paths in procs
                 if any(p == wt or p.startswith(wt + '/') for p in paths)]
        if users:
            print('%-28s skip: in use by pid %d (%s)' % (name, users[0][0], users[0][1]))
            continue
        admin = os.path.realpath(os.path.join(wt, git(wt, 'rev-parse', '--git-dir').strip()))
        idle = (now - newest_activity(wt, admin)) / 60
        if idle < idle_minutes:
            print('%-28s skip: active %.0f min ago' % (name, idle))
            continue
        targets = cargo_targets(wt)
        tsize = sum(dir_size(t) for t in targets)
        reasons = []
        locked = e.get('locked')
        stale = locked and lock_is_stale(locked)
        if locked and not (stale and unlock_stale):
            reasons.append('locked%s: %s' % (' (stale; --unlock-stale removes it)' if stale else '', locked))
        dirty = git(wt, '--no-optional-locks', 'status', '--porcelain', check=False).strip()
        if dirty:
            reasons.append('uncommitted changes (%d paths)' % len(dirty.split('\n')))
        head = e.get('HEAD', '')
        if not git(main, 'for-each-ref', '--count=1', '--contains', head, 'refs/heads/', check=False).strip():
            reasons.append('HEAD %s is on no branch' % head[:10])
        what = []
        if targets:
            what.append('%s %d target dir(s), %s' % ('would delete' if dry else 'deleting', len(targets), fmt_bytes(tsize)))
        if not reasons:
            br = e.get('branch', '').replace('refs/heads/', '')
            what.append('%s the worktree (%s)' % ('would remove' if dry else 'removing',
                                                  'branch %s kept' % br if br else 'detached HEAD, on a branch'))
        print('%-28s idle %.0f min: %s%s' % (name, idle, '; '.join(what) or 'nothing to do',
                                             ('; kept: ' + '; '.join(reasons)) if reasons else ''))
        if dry:
            freed += tsize + (0 if reasons else dir_size(wt) - tsize)
            continue
        for t in targets:
            rmtree(t)
        freed += tsize
        if not reasons:
            size = dir_size(wt)
            if locked:
                git(main, 'worktree', 'unlock', wt)
            r = subprocess.run(['git', '-C', main, 'worktree', 'remove', wt], capture_output=True, text=True)
            if r.returncode != 0:
                print('%-28s   git worktree remove failed: %s' % (name, r.stderr.strip()))
            else:
                freed += size
    print('%s %s' % ('would free' if dry else 'freed', fmt_bytes(freed)))


# ---------------------------------------------------------------------------------------------
# status

def cmd_status():
    if not os.path.isdir(ROOT):
        print('no cache at %s (it is created by the first build-rev/build-tree)' % ROOT)
        return
    cfg = config()
    print('cache %s: %s (cap %d GB)' % (ROOT, fmt_bytes(dir_size(ROOT)), cfg['max_total_gb']))
    for i in range(max(1, int(cfg['workspaces']))):
        st = ws_state(i)
        if not st:
            print('  workspace %d: empty' % i)
            continue
        profs = ', '.join('%s %s' % (p, fmt_bytes(dir_size(os.path.join(ws_dir(i), 'target', p))))
                          for p in PROFILES if os.path.isdir(os.path.join(ws_dir(i), 'target', p)))
        print('  workspace %d: last %s at %s; targets: %s' % (
            i, st.get('label'), time.strftime('%m-%d %H:%M', time.localtime(st.get('last_used', 0))), profs or 'none'))
    for prof in PROFILES:
        d = os.path.join(ROOT, 'out', prof)
        outs = [n for n in os.listdir(d) if '.tmp-' not in n] if os.path.isdir(d) else []
        if outs:
            print('  outputs %s: %d (%s)' % (prof, len(outs), fmt_bytes(dir_size(d))))
            for n in sorted(outs, key=lambda n: -os.stat(os.path.join(d, n)).st_mtime)[:5]:
                info = read_json(os.path.join(d, n, 'info.json'), {})
                print('    %s  %s%s' % (os.path.join(d, n), info.get('source'),
                                        ', tests passed' if (info.get('tests') or {}).get('passed') else ''))
    rd = os.path.join(ROOT, 'runs')
    n = sum(len(os.listdir(os.path.join(rd, s))) for s in os.listdir(rd)
            if len(s) == 2 and os.path.isdir(os.path.join(rd, s))) if os.path.isdir(rd) else 0
    hits = saved = stores = 0
    try:
        with open(os.path.join(rd, 'stats.log')) as f:
            for line in f:
                p = line.split('\t')
                if len(p) == 4:
                    if p[1] == 'hit':
                        hits += 1
                        saved += float(p[3])
                    elif p[1] == 'store':
                        stores += 1
    except OSError:
        pass
    print('  run cache: %d entries (%s); %d stored, %d hits saving %.1f CPU-hours' % (
        n, fmt_bytes(dir_size(rd)) if os.path.isdir(rd) else '0', stores, hits, saved / 3600))
    holders = slot_holders()
    print('  CPU slots: %d of %d busy' % (sum(1 for _, h in holders if h), len(holders)))
    for i, h in holders:
        if h:
            print('    slot %d: %s' % (i, h))
    print('  build lock: %s' % owner_text())


# ---------------------------------------------------------------------------------------------
# command line

def split_dashdash(argv):
    if '--' in argv:
        i = argv.index('--')
        return argv[:i], argv[i + 1:]
    return argv, []


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    if not argv or argv[0] in ('-h', '--help'):
        print(__doc__)
        print('commands: build-rev, build-tree, run, slot, gc, status, prune-worktrees (each takes -h)')
        return 0
    cmd, rest = argv[0], argv[1:]
    try:
        if cmd in ('build-rev', 'build-tree'):
            own, test_args = split_dashdash(rest)
            ap = argparse.ArgumentParser(prog=cmd + '.sh', description=(
                'Print the directory holding release binaries (ffh, ffh-stress, ffh-play) of a git revision'
                if cmd == 'build-rev' else
                "Print the directory holding release binaries built from a worktree's current files "
                '(uncommitted and untracked files included, ignored ones not)'),
                epilog='Arguments after -- go to cargo test (and imply --test).')
            if cmd == 'build-rev':
                ap.add_argument('rev')
            else:
                ap.add_argument('worktree', nargs='?', default='.')
            ap.add_argument('--profile', default='release', choices=PROFILES,
                            help="'checked' = release + debug assertions + overflow checks")
            ap.add_argument('--test', action='store_true', help='also run cargo test --release (same profile)')
            ap.add_argument('--force-test', action='store_true',
                            help='rerun the tests even if they passed for this exact content before')
            ap.add_argument('--bins', default=','.join(BINS),
                            help='binaries to build (default all: %(default)s); `--bins ffh` skips ffh-play, '
                                 'by far the slowest to rebuild after an ffh-ai change')
            ap.add_argument('-j', '--jobs', type=int, default=None, help='cargo -j')
            a = ap.parse_args(own)
            bins = [b for b in a.bins.split(',') if b]
            bad = [b for b in bins if b not in BINS]
            if bad or not bins:
                ap.error('--bins: choose from %s' % ','.join(BINS))
            os.makedirs(ROOT, exist_ok=True)
            test = a.test or a.force_test or bool(test_args)
            if cmd == 'build-rev':
                out, ok = build_rev(a.rev, a.profile, bins, test, test_args, a.force_test, jobs=a.jobs)
            else:
                out, ok = build_tree(a.worktree, a.profile, bins, test, test_args, a.force_test, jobs=a.jobs)
            print(out)
            return 0 if ok in (None, True) else 1
        if cmd == 'run':
            ap = argparse.ArgumentParser(prog='cached-run.sh', description=(
                'Run a deterministic command (e.g. `ffh stats ...`) through the run cache: the same '
                'binary + data + arguments + FFH_* environment replays the stored stdout/stderr. '
                'Failed runs are never stored. Holds one CPU slot while running.'))
            ap.add_argument('--no-cache', action='store_true')
            ap.add_argument('--no-slot', action='store_true')
            ap.add_argument('-v', '--verbose', action='store_true')
            ap.add_argument('argv', nargs=argparse.REMAINDER)
            a = ap.parse_args(rest)
            run = a.argv[1:] if a.argv[:1] == ['--'] else a.argv
            if not run:
                ap.error('no command')
            rc, out, err, hit = run_cached(run, cache=False if a.no_cache else None,
                                           slots=False if a.no_slot else None, stream=True, verbose=a.verbose)
            return rc
        if cmd == 'slot':
            ap = argparse.ArgumentParser(prog='slot.sh', description=(
                'Run a command holding machine-wide CPU slots (default 1 of nproc; see docs/AGENTS.md). '
                'Nested use (a command already under slot.sh) does not take more slots.'))
            g = ap.add_mutually_exclusive_group()
            g.add_argument('-n', type=int, default=1, help='slots to hold (e.g. the --jobs of a batch)')
            g.add_argument('--exclusive', action='store_true', help='hold every slot (timing benchmarks)')
            ap.add_argument('-q', '--quiet', action='store_true')
            ap.add_argument('argv', nargs=argparse.REMAINDER)
            a = ap.parse_args(rest)
            run = a.argv[1:] if a.argv[:1] == ['--'] else a.argv
            if not run:
                ap.error('no command')
            n = slot_count() if a.exclusive else a.n
            if os.environ.get('FFH2_SLOT_HELD') or not slots_enabled():
                os.execvp(run[0], run)
            fs = acquire_slots(n, ' '.join(run), verbose=not a.quiet)
            for f in fs:
                os.set_inheritable(f.fileno(), True)
            os.environ['FFH2_SLOT_HELD'] = str(len(fs))
            os.execvp(run[0], run)
        if cmd == 'gc':
            ap = argparse.ArgumentParser(prog='cache-gc.sh')
            ap.add_argument('--dry-run', action='store_true')
            a = ap.parse_args(rest)
            if a.dry_run:
                gc(quiet=False, dry=True)
            else:
                with flocked(os.path.join(ROOT, 'locks', 'build.lock'),
                             wait_msg=lambda: 'waiting for the build lock (held by: %s)' % owner_text()):
                    gc(quiet=False)
            return 0
        if cmd == 'status':
            cmd_status()
            return 0
        if cmd == 'prune-worktrees':
            ap = argparse.ArgumentParser(prog='prune-worktrees.sh', description=(
                'Remove agent worktrees under .claude/worktrees that are clean, idle and whose HEAD is '
                'on a branch (git worktree remove, never --force); delete cargo target dirs of every '
                'idle worktree. Worktrees used by a running process are never touched.'))
            ap.add_argument('--idle-minutes', type=float, default=90)
            ap.add_argument('-n', '--dry-run', action='store_true')
            ap.add_argument('--unlock-stale', action='store_true',
                            help='also remove worktrees whose lock names a process that no longer exists')
            a = ap.parse_args(rest)
            prune_worktrees(a.idle_minutes, a.dry_run, a.unlock_stale)
            return 0
        print('unknown command %r' % cmd, file=sys.stderr)
        return 2
    except Fail as e:
        log('error: %s' % e)
        return 1


if __name__ == '__main__':
    sys.exit(main())
