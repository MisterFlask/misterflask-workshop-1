#!/usr/bin/env python3
"""Victory pursuit check: do full AI games end in rules victories before the time limit?

Plays full games (normal speed: to the rules' end at turn 690, or an earlier victory) over many
seeds with `ffh stats` (through ai_metrics.py's runner), or reads games saved by
`ai_metrics.py --out`, and reports:
- the share of games decided by a victory other than time, per victory type, the decision turn;
- the winners' victory-planner goals and the goals all civs held at each snapshot;
- economy at fixed turns (per alive civ: legacy FFH2 score, techs, cities, population, research)
  and eliminations, to make sure the pursuit does not wreck the empires.

usage: victory_check.py [--bin target/release/ffh] [--seeds 1-24] [--jobs 4] [--out games.json]
                        [--load games.json] [--compare base.json] [--min-share 0.4]

--min-share S exits with status 1 when fewer than S of the games end in a non-time victory
(and, with --compare, when the share is not above the compared run's), so the check can gate a
change. Each full game takes a few minutes of one core (6 civs, standard map).
"""
import argparse
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ai_metrics  # noqa: E402

CHECKPOINTS = [300, 400, 500, 600]


def play(args):
    ns = argparse.Namespace(bin=args.bin, civs=args.civs, size=args.size, turns=700,
                            at=','.join(str(t) for t in CHECKPOINTS + [690]), option=[],
                            until_supremacy=False, every=0)
    with ThreadPoolExecutor(max_workers=args.jobs) as ex:
        return list(ex.map(lambda s: ai_metrics.run_one(ns, s), ai_metrics.parse_seeds(args.seeds)))


def wins(results):
    """Per game: (seed, victory type or 'time', turn, winning team)."""
    out = []
    for r in results:
        if 'error' in r:
            continue
        o = r.get('outcome', {})
        v = o.get('victory', 'time') if o.get('kind') == 'victory' else o.get('kind', '?')
        out.append((r['seed'], v, o.get('turn', r.get('final_turn')), o.get('team', -1)))
    return out


def economy(results, turn):
    civs = [c for r in results if 'error' not in r for c in r['snapshots'].get(str(turn), []) if not c['barb'] and c['alive']]
    if not civs:
        return None

    def avg(k):
        v = [c.get(k, 0) for c in civs]
        return sum(v) / len(v)
    games = sum(1 for r in results if 'error' not in r and str(turn) in r['snapshots'])
    return {'games': games, 'civs': len(civs), 'legacy': avg('legacy_score'), 'techs': avg('techs'), 'cities': avg('cities'),
            'pop': avg('pop'), 'research': avg('research'), 'pct': avg('pct')}


def goals(results, turn):
    n = {}
    for r in results:
        if 'error' in r:
            continue
        for c in r['snapshots'].get(str(turn), []):
            if not c['barb'] and c['alive'] and 'goal' in c:
                n[c['goal']] = n.get(c['goal'], 0) + 1
    return n


def report(results, label):
    w = wins(results)
    n = len(w)
    decided = [x for x in w if x[1] not in ('time', 'VICTORY_TIME', 'turn_limit')]
    kinds = {}
    for x in decided:
        kinds[x[1]] = kinds.get(x[1], 0) + 1
    turns = [x[2] for x in decided]
    print('== %s: %d games ==' % (label, n))
    print('victories before the time limit: %d/%d (%.0f%%)%s' % (
        len(decided), n, 100.0 * len(decided) / max(1, n),
        ', decision turn avg %.0f [%d-%d]' % (sum(turns) / len(turns), min(turns), max(turns)) if turns else ''))
    for k, v in sorted(kinds.items(), key=lambda kv: (-kv[1], kv[0])):
        print('  %-32s %d' % (k, v))
    elim = sum(r.get('eliminated', 0) for r in results if 'error' not in r)
    print('civs eliminated: %d (%.1f per game)' % (elim, elim / max(1, n)))
    print('turn  games civs  legacy  techs cities    pop research slider | planner goals')
    for t in CHECKPOINTS:
        e = economy(results, t)
        if e:
            g = goals(results, t)
            print('%4d  %5d %4d  %6.0f  %5.1f  %5.1f  %5.1f  %7.1f  %5.1f | %s' % (
                t, e['games'], e['civs'], e['legacy'], e['techs'], e['cities'], e['pop'], e['research'], e['pct'],
                ', '.join('%s %d' % kv for kv in sorted(g.items(), key=lambda kv: (-kv[1], kv[0]))) or '-'))
    print('per game: ' + ' '.join('%d:%s@%s' % (s, v.replace('VICTORY_', '')[:5], t) for s, v, t, _ in w))
    return len(decided) / max(1, n)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', default='target/release/ffh')
    ap.add_argument('--seeds', default='1-24')
    ap.add_argument('--civs', type=int, default=6)
    ap.add_argument('--size', default='WORLDSIZE_STANDARD')
    ap.add_argument('--jobs', type=int, default=4)
    ap.add_argument('--out', default=None)
    ap.add_argument('--load', default=None, help='games saved by ai_metrics.py --out or this script')
    ap.add_argument('--compare', default=None)
    ap.add_argument('--min-share', type=float, default=None)
    args = ap.parse_args()
    if args.load:
        with open(args.load) as f:
            results = json.load(f)
    else:
        results = play(args)
        if args.out:
            with open(args.out, 'w') as f:
                json.dump(results, f)
    for r in results:
        if 'error' in r:
            print('seed %s FAILED: %s' % (r['seed'], r['error']))
    base_share = None
    if args.compare:
        with open(args.compare) as f:
            base_share = report(json.load(f), 'before: ' + args.compare)
        print()
    share = report(results, 'current')
    if args.min_share is not None:
        ok = share >= args.min_share and (base_share is None or share > base_share)
        print('check: %.0f%% decided before the time limit (minimum %.0f%%%s): %s' % (
            100 * share, 100 * args.min_share, ', above %.0f%%' % (100 * base_share) if base_share is not None else '',
            'ok' if ok else 'FAILED'))
        sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
