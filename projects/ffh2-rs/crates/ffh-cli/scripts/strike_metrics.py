#!/usr/bin/env python3
"""Siege strike metrics over several seeds, from the AI trace.

Runs `ffh stats` with FFH_AI_TRACE=all for each seed (in parallel), keeps the JSON line and the
strike lines of the trace ("strike on ...", "strike step ...", "assault on city ..."), and
prints: strikes, how many took the city, units lost (per city taken, in failed strikes), the
calibration of the strike estimate (predicted probability of taking the city at the first step
against the outcome, by bucket), city assaults outside the siege operations, and from `ffh
stats` the cities captured or razed by civs, eliminations, legacy score, techs, cities.

usage: strike_metrics.py [--bin target/release/ffh] [--seeds 1-24] [--turns 300] [--jobs 4]
                         [--out runs.json] [--compare old.json]
"""
import argparse
import json
import os
import re
import subprocess
import threading
from concurrent.futures import ThreadPoolExecutor

STRIKE = re.compile(r'^\[t(\d+) p(\d+)\] strike on (.*): (\d+) stacks, (\d+) attacks, lost (\d+), (taken|holds)(?: \(p_take ([0-9.]+)\))?')
STEP = re.compile(r'^\[t\d+ p\d+\]   strike step (\d+): .*p_take ([0-9.]+), lost ([0-9.]+)')
ASSAULT = re.compile(r'^\[t\d+ p\d+\] assault on city at .*: (\d+) attacks, lost (\d+), (taken|holds)')
KEEP = ('strike on', 'strike step', 'assault on')


def parse_seeds(s):
    out = []
    for part in s.split(','):
        if '-' in part:
            a, b = part.split('-')
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def run_one(args, seed):
    cmd = [args.bin, 'stats', '--seed', str(seed), '--civs', '6', '--size', 'WORLDSIZE_STANDARD', '--turns', str(args.turns), '--at', str(args.turns)]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=dict(os.environ, FFH_AI_TRACE='all'))
    lines = []

    def rd():
        for l in p.stderr:
            if any(k in l for k in KEEP):
                lines.append(l.rstrip('\n'))
    t = threading.Thread(target=rd)
    t.start()
    out = p.stdout.read()
    p.wait()
    t.join()
    js = [l for l in out.splitlines() if l.startswith('{')]
    if p.returncode != 0 or not js:
        return {'seed': seed, 'error': out[-2000:]}
    r = json.loads(js[-1])
    r['trace'] = lines
    return r


def summarize(results):
    ok = [r for r in results if 'error' not in r]
    s = {'games': len(ok), 'strikes': 0, 'taken': 0, 'lost': 0, 'lost_failed': 0, 'assaults': 0, 'assaults_taken': 0, 'assault_lost': 0,
         'captures': 0, 'eliminated': 0, 'legacy': 0.0, 'techs': 0.0, 'cities': 0.0}
    calib = []
    for r in ok:
        first = None
        for l in r['trace']:
            m = STEP.match(l)
            if m and m.group(1) == '0':
                first = float(m.group(2))
            m = STRIKE.match(l)
            if m and int(m.group(5)) > 0:
                taken = m.group(7) == 'taken'
                s['strikes'] += 1
                s['taken'] += taken
                s['lost'] += int(m.group(6))
                if not taken:
                    s['lost_failed'] += int(m.group(6))
                p = float(m.group(8)) if m.group(8) else first
                if p is not None:
                    calib.append((p, taken))
                first = None
            m = ASSAULT.match(l)
            if m:
                s['assaults'] += 1
                s['assaults_taken'] += m.group(3) == 'taken'
                s['assault_lost'] += int(m.group(2))
        last = r['snapshots'][str(r['final_turn'])]
        civs = [c for c in last if not c['barb']]
        alive = [c for c in civs if c['alive']]
        s['captures'] += sum(c.get('taken_civ', 0) + c.get('taken_barb', 0) + c.get('razed', 0) for c in civs)
        s['eliminated'] += r['eliminated']
        for k, f in (('legacy', 'legacy_score'), ('techs', 'techs'), ('cities', 'cities')):
            s[k] += sum(c.get(f, 0) for c in alive) / max(1, len(alive))
    for k in ('legacy', 'techs', 'cities'):
        s[k] /= max(1, len(ok))
    return s, calib


def report(results, label):
    for e in [r for r in results if 'error' in r]:
        print('seed %s FAILED: %s' % (e['seed'], e['error']))
    s, calib = summarize(results)
    print('== %s (%d games) ==' % (label, s['games']))
    print('strikes %d, taken %d (%.1f%%), units lost %d (%.2f per city taken, %d in failed strikes)' % (
        s['strikes'], s['taken'], 100.0 * s['taken'] / max(1, s['strikes']), s['lost'], s['lost'] / max(1, s['taken']), s['lost_failed']))
    print('city assaults outside operations %d, taken %d, units lost %d' % (s['assaults'], s['assaults_taken'], s['assault_lost']))
    print('cities captured or razed by civs %d, civs eliminated %d | per civ: legacy score %.1f, techs %.2f, cities %.2f' % (
        s['captures'], s['eliminated'], s['legacy'], s['techs'], s['cities']))
    if calib:
        print('estimate at the first step -> outcome:')
        for lo, hi in [(0, 0.5), (0.5, 0.65), (0.65, 0.75), (0.75, 0.85), (0.85, 0.95), (0.95, 1.01)]:
            b = [x for x in calib if lo <= x[0] < hi]
            if b:
                print('  %.2f-%.2f: %3d strikes, predicted %.3f, taken %.3f' % (lo, min(hi, 1.0), len(b), sum(x[0] for x in b) / len(b), sum(x[1] for x in b) / len(b)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', default='target/release/ffh')
    ap.add_argument('--seeds', default='1-24')
    ap.add_argument('--turns', type=int, default=300)
    ap.add_argument('--jobs', type=int, default=4)
    ap.add_argument('--out', default=None)
    ap.add_argument('--compare', default=None)
    args = ap.parse_args()
    with ThreadPoolExecutor(max_workers=args.jobs) as ex:
        results = list(ex.map(lambda s: run_one(args, s), parse_seeds(args.seeds)))
    if args.out:
        with open(args.out, 'w') as f:
            json.dump(results, f)
    if args.compare:
        with open(args.compare) as f:
            report(json.load(f), 'before: ' + args.compare)
        print()
    report(results, 'current')


if __name__ == '__main__':
    main()
