#!/usr/bin/env python3
"""Fan out a batch of deterministic games to sibling cloud sessions (docs/AGENTS.md "Fan-out").

The orchestrator is the agent driving this: the create_session / archive_session MCP tools are the
agent's, not callable from here, so this script does the mechanical, deterministic half and prints
exactly what the agent must do between steps.

  prepare  push the commit under test to fanout/<job>, slice the seeds into N workers, write a job
           file under $FFH2_CACHE/fanout/<job>.json, and print the command each worker session runs
  status   show which workers have pushed results and how many games each has
  collect  fetch the result branches and re-key every game into the LOCAL run cache (so a later
           `ai_metrics.py --seeds ...` with the same binary and args is all cache hits); report
           which seeds are still missing
  cleanup  delete the job and result branches on origin and the local job file

Only cross-container DETERMINISM is relied on: `collect` re-keys each worker's stdout under the key
the local binary produces, so it is correct whether or not builds are byte-identical across
containers. `prepare --check` first plays a couple of seeds locally and has the worker compare, to
confirm same seed -> same checksum before trusting a batch (see docs/AGENTS.md).
"""
import argparse
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import ffh_agents  # noqa: E402
sys.path.insert(0, os.path.join(HERE, '..', '..', 'crates', 'ffh-cli', 'scripts'))
import ai_metrics  # noqa: E402

JOB_DIR = os.path.join(ffh_agents.ROOT, 'fanout')
WORKER_REL = 'projects/ffh2-rs/tools/agents/fanout-worker.sh'


def sh(*args, cwd=None, check=True):
    r = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise SystemExit('command failed: %s\n%s' % (' '.join(args), r.stderr.strip()))
    return r.stdout.strip()


def repo_root():
    return sh('git', 'rev-parse', '--show-toplevel')


def parse_seeds(s):
    return ai_metrics.parse_seeds(s)


def slice_seeds(seeds, n):
    """n contiguous slices, as balanced as possible."""
    n = max(1, min(n, len(seeds)))
    out, base, rem = [], len(seeds) // n, len(seeds) % n
    i = 0
    for k in range(n):
        size = base + (1 if k < rem else 0)
        out.append(seeds[i:i + size])
        i += size
    return [s for s in out if s]


def compact(seeds):
    """[201,202,203,207] -> '201-203,207' (contiguous runs)."""
    seeds = sorted(seeds)
    parts, i = [], 0
    while i < len(seeds):
        j = i
        while j + 1 < len(seeds) and seeds[j + 1] == seeds[j] + 1:
            j += 1
        parts.append('%d-%d' % (seeds[i], seeds[j]) if j > i else '%d' % seeds[i])
        i = j + 1
    return ','.join(parts)


def job_path(job):
    return os.path.join(JOB_DIR, job + '.json')


def load_job(job):
    spec = ffh_agents.read_json(job_path(job))
    if not spec:
        raise SystemExit('no such job %r (looked in %s)' % (job, JOB_DIR))
    return spec


def cmd_prepare(a):
    root = repo_root()
    commit = ffh_agents.resolve_rev(root, a.rev)
    seeds = parse_seeds(a.seeds)
    slices = slice_seeds(seeds, a.workers)
    job = a.job or '%s-%d' % (commit[:12], int(time.time()))
    branch = 'fanout/%s' % job
    game_args = a.game_args
    print('[fanout] pushing %s -> origin %s' % (commit[:12], branch), file=sys.stderr)
    sh('git', '-C', root, 'push', '-f', 'origin', '%s:refs/heads/%s' % (commit, branch))
    workers = []
    for k, sl in enumerate(slices):
        rb = 'fanout/res/%s/w%d' % (job, k)
        wcmd = 'git fetch origin %s && git checkout %s && bash %s --job %s --worker %d --result-branch %s --seeds %s -- %s' % (
            branch, branch, WORKER_REL, job, k, rb, compact(sl), ' '.join(game_args))
        workers.append({'worker': k, 'seeds': compact(sl), 'count': len(sl), 'result_branch': rb, 'command': wcmd})
    spec = {'job': job, 'commit': commit, 'branch': branch, 'seeds': a.seeds, 'game_args': game_args,
            'workers': workers, 'created': time.time()}
    os.makedirs(JOB_DIR, exist_ok=True)
    ffh_agents.atomic_write(job_path(job), json.dumps(spec, indent=1).encode())
    print('\njob %s: %d seeds over %d workers, commit %s on %s' % (
        job, len(seeds), len(workers), commit[:12], branch))
    print('start one sibling cloud session per worker: create_session(source_url=<this repo>,')
    print('source_revision=%s), and send it a prompt like this (it MUST run in the foreground):' % branch)
    for w in workers:
        print('\n  # worker %d: seeds %s (%d) -- result branch %s' % (
            w['worker'], w['seeds'], w['count'], w['result_branch']))
        print('  Run this one command in the FOREGROUND and wait for it to finish in this turn (do')
        print('  not background it); then report its final stderr line. Do not edit files or open a PR:')
        print('      %s' % w['command'])
    print('\nthen: tools/agents/fanout.sh collect --job %s --bin <local-build-dir>/ffh' % job)
    print('(MACHINE %s)' % json.dumps({'job': job, 'branch': branch, 'commit': commit,
                                       'workers': workers}))


def existing_result_branches(root, job):
    out = sh('git', '-C', root, 'ls-remote', '--heads', 'origin', 'fanout/res/%s/*' % job, check=False)
    return [ln.split('\t')[1].replace('refs/heads/', '') for ln in out.splitlines() if ln]


def cmd_status(a):
    root = repo_root()
    spec = load_job(a.job)
    have = set(existing_result_branches(root, a.job))
    print('job %s (commit %s): %d workers' % (a.job, spec['commit'][:12], len(spec['workers'])))
    for w in spec['workers']:
        rb = w['result_branch']
        mark = 'pushed' if rb in have else 'waiting'
        extra = ''
        if rb in have:
            man = sh('git', '-C', root, 'show', 'origin/%s:%s' % (
                rb, 'fanout-res/%s/w%d/manifest.json' % (a.job, w['worker'])), check=False)
            try:
                extra = ' (%d/%d games)' % (json.loads(man)['seeds_ok'], w['count'])
            except (ValueError, KeyError):
                extra = ' (manifest unreadable)'
        print('  w%d seeds %s: %s%s' % (w['worker'], w['seeds'], mark, extra))


def cmd_collect(a):
    root = repo_root()
    spec = load_job(a.job)
    ap = ai_metrics.build_parser()
    args = ap.parse_args(['--bin', a.bin, '--seeds', '1'] + spec['game_args'])
    ident, why = ffh_agents.bin_identity(a.bin)
    if ident is None:
        raise SystemExit('cannot key runs for %s: %s (point --bin at a build output wrapper)' % (a.bin, why))
    sh('git', '-C', root, 'fetch', '-f', 'origin',
       *['%s:refs/remotes/origin/%s' % (w['result_branch'], w['result_branch']) for w in spec['workers']],
       check=False)
    imported = missing = failed = 0
    missing_seeds = []
    for w in spec['workers']:
        rb = w['result_branch']
        seeds = parse_seeds(w['seeds'])
        for s in seeds:
            path = 'fanout-res/%s/w%d/%d.out' % (a.job, w['worker'], s)
            blob = subprocess.run(['git', '-C', root, 'show', 'origin/%s:%s' % (rb, path)],
                                  capture_output=True)
            if blob.returncode != 0:
                missing += 1
                missing_seeds.append(s)
                continue
            argv = ai_metrics.stats_cmd(args, s)
            key, kwhy = ffh_agents.import_run(argv, blob.stdout, overwrite=a.overwrite)
            if key is None:
                failed += 1
                print('  seed %d not cacheable: %s' % (s, kwhy), file=sys.stderr)
            else:
                imported += 1
    print('job %s: imported %d games into the run cache, %d missing, %d not cacheable' % (
        a.job, imported, missing, failed))
    if missing_seeds:
        print('  missing seeds: %s' % compact(missing_seeds))
    print('now: python3 crates/ffh-cli/scripts/ai_metrics.py --bin %s --seeds %s %s %s' % (
        a.bin, spec['seeds'], ' '.join(spec['game_args']),
        '[--compare base.json] (all cache hits)' if not missing_seeds else '(run the missing seeds live)'))


def cmd_cleanup(a):
    root = repo_root()
    spec = ffh_agents.read_json(job_path(a.job))
    branches = []
    if spec:
        branches.append(spec['branch'])
    for rb in existing_result_branches(root, a.job):
        branches.append(rb)
    for w in (spec['workers'] if spec else []):
        if w['result_branch'] not in branches:
            branches.append(w['result_branch'])
    if branches:
        print('[fanout] deleting on origin: %s' % ', '.join(branches))
        r = subprocess.run(['git', '-C', root, 'push', 'origin',
                            *[':refs/heads/%s' % b for b in branches]], capture_output=True, text=True)
        if r.returncode != 0:
            print('  could not delete the remote branches (%s). This environment forbids branch '
                  'deletion over the git proxy (HTTP 403); they are harmless under fanout/ and can '
                  'be removed later by someone with delete rights.' % (
                      '403' if '403' in r.stderr else r.stderr.strip().splitlines()[-1:] or 'error'),
                  file=sys.stderr)
    for w in (spec['workers'] if spec else []):
        sh('git', '-C', root, 'update-ref', '-d', 'refs/remotes/origin/%s' % w['result_branch'], check=False)
    try:
        os.unlink(job_path(a.job))
    except OSError:
        pass
    print('job %s cleaned up locally (archive the worker sessions with archive_session)' % a.job)


def main():
    ap = argparse.ArgumentParser(prog='fanout.sh', description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='cmd', required=True)
    p = sub.add_parser('prepare', help='push the job branch and print worker commands')
    p.add_argument('--rev', default='HEAD')
    p.add_argument('--seeds', required=True, help='e.g. 601-648')
    p.add_argument('--workers', type=int, default=4, help='sibling sessions (<=4; user limit)')
    p.add_argument('--job', default=None, help='job id (default: <commit>-<time>)')
    p.add_argument('--game-args', nargs=argparse.REMAINDER, default=[],
                   help='everything after it is the ffh stats game args (--civs .. --size .. --turns .. --at ..)')
    p.set_defaults(fn=cmd_prepare)
    p = sub.add_parser('status', help='which workers have pushed results')
    p.add_argument('--job', required=True)
    p.set_defaults(fn=cmd_status)
    p = sub.add_parser('collect', help='fetch results and import them into the local run cache')
    p.add_argument('--job', required=True)
    p.add_argument('--bin', required=True, help='the LOCAL build output ffh wrapper (keys the cache)')
    p.add_argument('--overwrite', action='store_true', help='replace cache entries that already exist')
    p.set_defaults(fn=cmd_collect)
    p = sub.add_parser('cleanup', help='delete the job and result branches on origin')
    p.add_argument('--job', required=True)
    p.set_defaults(fn=cmd_cleanup)
    a = ap.parse_args()
    if a.cmd == 'prepare' and a.game_args and a.game_args[0] == '--':
        a.game_args = a.game_args[1:]
    a.fn(a)


if __name__ == '__main__':
    main()
