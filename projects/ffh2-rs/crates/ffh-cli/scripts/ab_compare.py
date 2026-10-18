#!/usr/bin/env python3
"""Paired A/B comparison of two `ai_metrics.py --out` files (same seeds, same settings).

`ai_metrics.py --compare` prints two reports side by side; this prints, per checkpoint turn and
per metric, the mean of each side and the paired t-statistic over seeds (the per-seed difference
of the per-civ mean), which is what docs/AI.md reports as "t". Games differ wildly from seed to
seed, so the paired t is the number to read: |t| < 2 is noise.

Per-civ means are over the civs present at game start (barbarians excluded); a dead civ counts
as 0 for the size metrics (`legacy_score`, `cities`, `pop`, `techs`, `research`, ...), as in
docs/AI.md's "dead as 0". Game-wide event counts (captures, wars, eliminations) are per game.

usage: ab_compare.py base.json new.json [--metrics legacy_score,techs,...] [--at 100,300]
"""
import argparse
import json
import math

CIV_METRICS = ['legacy_score', 'techs', 'research', 'cities', 'pop', 'prod', 'commerce', 'gold',
               'pct', 'mil', 'power', 'land', 'culture']
EVENTS = ['captured_civ', 'captured_from_barb', 'captured_by_barb', 'destroyed', 'founded',
          'wars_declared', 'peace_made', 'eliminated']


def load(path):
    with open(path) as f:
        games = json.load(f)
    return {g['seed']: g for g in games}


def civ_mean(game, turn, metric):
    snaps = game['snapshots'].get(str(turn))
    if snaps is None:
        return None
    rows = [r for r in snaps if not r.get('barb')]
    if not rows:
        return None
    vals = []
    for r in rows:
        v = r.get(metric)
        if not isinstance(v, (int, float)):
            v = 0
        if not r.get('alive', True):
            v = 0
        vals.append(v)
    return sum(vals) / len(vals)


def event(game, name):
    v = game.get(name)
    if isinstance(v, list):
        return len(v)
    if isinstance(v, dict):
        return len(v)
    return v if isinstance(v, (int, float)) else 0


def paired(a, b):
    n = len(a)
    d = [y - x for x, y in zip(a, b)]
    md = sum(d) / n
    if n < 2:
        return md, float('nan')
    var = sum((x - md) ** 2 for x in d) / (n - 1)
    if var == 0:
        return md, (0.0 if md == 0 else float('inf'))
    return md, md / math.sqrt(var / n)


def row(name, a, b):
    ma, mb = sum(a) / len(a), sum(b) / len(b)
    md, t = paired(a, b)
    pct = (100.0 * md / ma) if ma else float('nan')
    flag = '  *' if abs(t) >= 2 else ''
    return f'  {name:22s} {ma:10.2f} -> {mb:10.2f}  {md:+9.2f} ({pct:+6.1f}%)  t {t:+6.2f}{flag}'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('base')
    ap.add_argument('new')
    ap.add_argument('--metrics', default=','.join(CIV_METRICS))
    ap.add_argument('--at', default=None, help='checkpoint turns (default: all common ones)')
    args = ap.parse_args()
    A, B = load(args.base), load(args.new)
    seeds = sorted(set(A) & set(B))
    if not seeds:
        raise SystemExit('no common seeds')
    turns = sorted(set.intersection(*[set(int(t) for t in A[s]['snapshots']) & set(int(t) for t in B[s]['snapshots']) for s in seeds]))
    if args.at:
        turns = [t for t in turns if t in set(int(x) for x in args.at.split(','))]
    same = sum(1 for s in seeds if A[s].get('checksum') == B[s].get('checksum'))
    print(f'{len(seeds)} paired seeds ({seeds[0]}-{seeds[-1]}); identical games: {same}')
    print(f'  {"metric":22s} {"base":>10s}    {"new":>10s}  {"diff":>9s}            paired t')
    for t in turns:
        print(f'turn {t} (per-civ mean, dead civs as 0):')
        for m in args.metrics.split(','):
            a = [civ_mean(A[s], t, m) for s in seeds]
            b = [civ_mean(B[s], t, m) for s in seeds]
            if any(x is None for x in a + b):
                continue
            print(row(m, a, b))
    print('whole game (per game):')
    for e in EVENTS:
        a = [event(A[s], e) for s in seeds]
        b = [event(B[s], e) for s in seeds]
        print(row(e, a, b))
    a = [len(A[s].get('eliminated_at') or {}) for s in seeds]
    b = [len(B[s].get('eliminated_at') or {}) for s in seeds]
    print(row('civs_eliminated', a, b))


if __name__ == '__main__':
    main()
