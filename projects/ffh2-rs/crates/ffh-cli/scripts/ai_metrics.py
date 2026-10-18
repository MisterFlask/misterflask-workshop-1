#!/usr/bin/env python3
"""AI quality metrics over several seeds.

Runs `ffh stats` for each seed (in parallel), saves the raw JSON lines and prints a summary:
per checkpoint turn the per-civ average/min/max of cities, population and techs, plus whole-game
events (captures, razes, wars, eliminations), barbarian strength and timings.

Score: since `score_version` 2 a civ's `score` is its share of the world in basis points
(docs/SCORE.md), so its mean is always 10000/N. The report shows instead, per checkpoint and
averaged over games, the leading team's share, its ratio to the runner-up team, the fair share
100/N %, and the per-civ legacy (FFH2) score for absolute progress. Results saved before v2 (no
`score_version`, e.g. with --compare) only have the legacy score: their share columns read '-'.

usage: ai_metrics.py [--bin target/release/ffh] [--seeds 1-6] [--civs 6]
                     [--size WORLDSIZE_STANDARD] [--turns 300] [--at 100,200,300]
                     [--out results.json] [--compare old.json] [--jobs 4] [--option GAMEOPTION_X]
                     [--until-supremacy] [--every N] [--civ CIVILIZATION_X] [--detail]
                     [--cache | --no-cache] [--no-slots]

--every N also snapshots every N turns (and `ffh stats` always adds the final state). Each saved
game carries per-civ snapshots (`snapshots`), game-wide values (`world_at`), the outcome and the
turn each civ was eliminated (`eliminated_at`); see docs/AI.md "Tools" for the fields.

Parallel agents (docs/AGENTS.md): when the shared cache /home/user/.cache/ffh2-rs exists
(tools/agents/), each game goes through its run cache -- a game already played with the same
binary, data and arguments (e.g. the integration tip's side of an A/B test, by any agent) is
replayed from the cache, and identical games started concurrently are played once -- and every
game played holds one of the machine-wide CPU slots, so all agents together run at most nproc
games at a time. --no-cache / --no-slots turn these off; cached games keep the timings
(ms/turn) of their original run.
"""
import argparse
import json
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

try:
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..', 'tools', 'agents'))
    import ffh_agents
except ImportError:
    ffh_agents = None
finally:
    del sys.path[0]

CACHED = set()  # seeds whose result came from the run cache


def parse_seeds(s):
    out = []
    for part in s.split(','):
        if '-' in part:
            a, b = part.split('-')
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def stats_cmd(args, seed):
    """The exact `ffh stats` argv for one seed. Shared with the fan-out orchestrator
    (tools/agents/fanout.py), so a game a sibling session ran is re-keyed into the local run
    cache under the same key the local binary would produce. Keep deterministic and ordered."""
    cmd = [args.bin, 'stats', '--seed', str(seed), '--civs', str(args.civs), '--size', args.size,
           '--turns', str(args.turns), '--at', args.at]
    for opt in args.option:
        cmd += ['--option', opt]
    for civ in getattr(args, 'civ', None) or []:
        cmd += ['--civ', civ]
    if getattr(args, 'detail', False):
        cmd += ['--detail']
    if args.until_supremacy:
        cmd += ['--until-supremacy']
    if args.every:
        cmd += ['--every', str(args.every)]
    return cmd


def run_one(args, seed):
    cmd = stats_cmd(args, seed)
    if ffh_agents and (args.cache or args.slots):
        rc, out, err, hit = ffh_agents.run_cached(cmd, cache=args.cache, slots=args.slots)
        out, err = out.decode('utf-8', 'replace'), err.decode('utf-8', 'replace')
        if hit:
            CACHED.add(seed)
    else:
        r = subprocess.run(cmd, capture_output=True, text=True)
        rc, out, err = r.returncode, r.stdout, r.stderr
    if rc != 0:
        return {'seed': seed, 'error': (err or out)[-2000:]}
    line = [l for l in out.splitlines() if l.startswith('{')][-1]
    return json.loads(line)


def score_version(r):
    return r.get('score_version', 1)


def legacy_score(r, c):
    return c.get('legacy_score', 0) if score_version(r) >= 2 else c.get('score', 0)


def leader_stats(ok, key):
    """Per game at this checkpoint: (leader's share %, leader / runner-up, fair share %) over the
    alive civ teams (team score = sum of its members' shares), for score v2 results only."""
    out = []
    for r in ok:
        if score_version(r) < 2:
            continue
        alive = [c for c in r['snapshots'].get(key, []) if not c['barb'] and c['alive']]
        if not alive:
            continue
        teams = {}
        for c in alive:
            t = c.get('team', c['p'])
            teams[t] = teams.get(t, 0) + c['score']
        top = sorted(teams.values(), reverse=True)
        ratio = top[0] / top[1] if len(top) > 1 and top[1] > 0 else None
        out.append((top[0] / 100.0, ratio, 100.0 / len(alive)))
    return out


def goal_counts(alive):
    """Victory planner goals of the alive civs ('-' = none), e.g. 'Tower 9, Culture 2, - 30'."""
    if not any('goal' in c for c in alive):
        return '-'
    n = {}
    for c in alive:
        g = c.get('goal', '-')
        n[g] = n.get(g, 0) + 1
    return ', '.join('%s %d' % kv for kv in sorted(n.items(), key=lambda kv: (-kv[1], kv[0])))


def summarize(results, at):
    rows = []
    ok = [r for r in results if 'error' not in r]
    for t in at:
        key = str(t)
        civs = [c for r in ok for c in r['snapshots'].get(key, []) if not c['barb']]
        barbs = [c for r in ok for c in r['snapshots'].get(key, []) if c['barb']]
        alive = [c for c in civs if c['alive']]
        if not civs:
            continue

        def stat(field, pool=alive):
            v = [c[field] for c in pool if field in c]
            if not v:
                return '-'
            return '%.1f [%d-%d]' % (sum(v) / len(v), min(v), max(v))

        def mean_range(v, fmt):
            v = [x for x in v if x is not None]
            if not v:
                return '-'
            return (fmt + ' [' + fmt + '-' + fmt + ']') % (sum(v) / len(v), min(v), max(v))
        lead = leader_stats(ok, key)
        legacy = [legacy_score(r, c) for r in ok for c in r['snapshots'].get(key, []) if not c['barb'] and c['alive']]
        religions = [r['world_at'][key]['religions'] for r in ok if 'religions' in r.get('world_at', {}).get(key, {})]
        rows.append((t, {
            'cities': stat('cities'), 'pop': stat('pop'), 'techs': stat('techs'), 'mil': stat('mil'),
            'workers': stat('workers'), 'impr': stat('impr'), 'research': stat('research'), 'prod': stat('prod'),
            'alive': '%d/%d' % (len(alive), len(civs)),
            'pct': stat('pct'), 'maint': stat('maint'), 'ucost': stat('ucost'),
            'lead_share': mean_range([x[0] for x in lead], '%.1f'), 'lead_ratio': mean_range([x[1] for x in lead], '%.2f'),
            'fair_share': mean_range([x[2] for x in lead], '%.1f'),
            'legacy': ('%.1f [%d-%d]' % (sum(legacy) / len(legacy), min(legacy), max(legacy))) if legacy else '-',
            'civ_upkeep': stat('civ_upkeep'), 'commerce': stat('commerce'), 'gold': stat('gold'), 'overseas': stat('overseas'), 'ships': stat('ships'),
            'alone': stat('cities', [c for c in alive if c.get('area_civs') == 0]),
            'shared': stat('cities', [c for c in alive if c.get('area_civs', 0) > 0]),
            'goals': goal_counts(alive),
            'towers': stat('towers'), 'legend3': stat('legend3_pm'), 'culture_pct': stat('culture_pct'),
            'barb_units': '%.0f' % (sum(b['units'] for b in barbs) / max(1, len(barbs))),
            'barb_cities': '%.1f' % (sum(b['cities'] for b in barbs) / max(1, len(barbs))),
            'religions': mean_range(religions, '%.1f'),
        }))
    return rows


def totals(results):
    ok = [r for r in results if 'error' not in r]
    keys = ['captured_civ', 'captured_from_barb', 'captured_by_barb', 'destroyed', 'founded', 'wars_declared', 'peace_made', 'eliminated']
    tot = {k: sum(r[k] for r in ok) for k in keys}
    tot['ms_per_turn'] = sum(r['ms_per_turn'] for r in ok) / max(1, len(ok))
    tot['slowest_ms'] = max([r['slowest_ms'] for r in ok] or [0])
    tot['games'] = len(ok)
    return tot


def print_report(results, at, label='', cached=()):
    errs = [r for r in results if 'error' in r]
    for e in errs:
        print('seed %s FAILED: %s' % (e['seed'], e['error']))
    print('== %s (%d games) ==' % (label, len(results) - len(errs)))
    for t, row in summarize(results, at):
        print('turn %3d: alive %s | cities %s | pop %s | techs %s' % (t, row['alive'], row['cities'], row['pop'], row['techs']))
        print('          mil %s | workers %s | impr %s | research %s | prod %s | barb units %s cities %s' % (
            row['mil'], row['workers'], row['impr'], row['research'], row['prod'], row['barb_units'], row['barb_cities']))
        print('          score: leader %s%% of the world (fair share %s%%), x%s the runner-up | legacy score %s' % (
            row['lead_share'], row['fair_share'], row['lead_ratio'], row['legacy']))
        print('          slider %s | commerce %s | maint %s | units %s | civics %s | gold %s' % (
            row['pct'], row['commerce'], row['maint'], row['ucost'], row['civ_upkeep'], row['gold']))
        print('          overseas %s | ships %s | cities alone %s, shared %s | religions founded %s' % (
            row['overseas'], row['ships'], row['alone'], row['shared'], row['religions']))
        if row['goals'] != '-':
            print('          victory goals: %s | towers %s | 3rd city culture %s per mille of legendary | culture slider %s' % (
                row['goals'], row['towers'], row['legend3'], row['culture_pct']))
    tot = totals(results)
    print('events: captured civ<-civ %(captured_civ)d, civ<-barb %(captured_from_barb)d, barb<-civ %(captured_by_barb)d, '
          'destroyed %(destroyed)d, founded %(founded)d, wars %(wars_declared)d, peace %(peace_made)d, eliminated %(eliminated)d' % tot)
    n_cached = sum(1 for r in results if 'error' not in r and r['seed'] in cached)
    print('time: %.2f ms/turn avg, slowest turn %.1f ms%s' % (
        tot['ms_per_turn'], tot['slowest_ms'],
        ' (%d of %d games from the run cache: timings of their original runs)' % (n_cached, tot['games']) if n_cached else ''))
    outs = [r['outcome'] for r in results if 'error' not in r and 'outcome' in r]
    if outs:
        decided = [o for o in outs if o['kind'] != 'turn_limit']
        kinds = {}
        for o in decided:
            k = o['kind'] if o['kind'] != 'victory' else 'victory:' + o.get('victory', '?')
            kinds[k] = kinds.get(k, 0) + 1
        turns = [o['turn'] for o in decided]
        print('outcome: %d/%d decided%s%s' % (
            len(decided), len(outs),
            (', decision turn avg %.0f [%d-%d]' % (sum(turns) / len(turns), min(turns), max(turns))) if turns else '',
            (' | ' + ', '.join('%s %d' % kv for kv in sorted(kinds.items()))) if kinds else ''))
        # Rules victories other than the time limit (a time end reports victory 'time').
        wins = [o for o in outs if o['kind'] == 'victory' and o.get('victory', 'time') not in ('time', 'VICTORY_TIME')]
        wturns = [o['turn'] for o in wins]
        print('victories before the time limit: %d/%d (%.0f%%)%s' % (
            len(wins), len(outs), 100.0 * len(wins) / len(outs),
            (', turn avg %.0f [%d-%d]' % (sum(wturns) / len(wturns), min(wturns), max(wturns))) if wturns else ''))
    for r in results:
        if 'error' in r:
            continue
        last = r['snapshots'].get(str(at[-1]), [])
        civs = ' '.join('%s:%d/%d/%d' % (c['civ'][:4], c['cities'], c['pop'], c['techs']) for c in last if not c['barb'])
        print('  seed %d (%s, %.1f ms/turn): %s' % (r['seed'], r['checksum'], r['ms_per_turn'], civs))


def build_parser():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', default='target/release/ffh')
    ap.add_argument('--seeds', default='1-6')
    ap.add_argument('--civs', type=int, default=6)
    ap.add_argument('--size', default='WORLDSIZE_STANDARD')
    ap.add_argument('--turns', type=int, default=300)
    ap.add_argument('--at', default='100,200,300')
    ap.add_argument('--every', type=int, default=0, help='also snapshot every N turns')
    ap.add_argument('--out', default=None)
    ap.add_argument('--compare', default=None)
    ap.add_argument('--jobs', type=int, default=4)
    ap.add_argument('--label', default='current')
    ap.add_argument('--option', action='append', default=[], help='GAMEOPTION_X to turn on (repeatable)')
    ap.add_argument('--civ', action='append', default=[],
                    help='CIVILIZATION_X forced into the next civ slot (repeatable; leader picked by the seed)')
    ap.add_argument('--detail', action='store_true', help='per-player unit/building/project type counts')
    ap.add_argument('--until-supremacy', action='store_true',
                    help="stop each game at victory, last civ standing, or clear supremacy (1.5x the runner-up's score, "
                         "i.e. share of the world, held 30 turns, counted from turn 200)")
    ap.add_argument('--cache', dest='cache', action='store_true', default=None,
                    help='replay games already played (same binary, data, arguments) from the shared run cache '
                         '(default: on when %s exists)' % (ffh_agents.ROOT if ffh_agents else 'the agents cache'))
    ap.add_argument('--no-cache', dest='cache', action='store_false')
    ap.add_argument('--no-slots', dest='slots', action='store_false', default=None,
                    help='do not take machine-wide CPU slots (tools/agents/slot.sh) per game')
    return ap


def main():
    ap = build_parser()
    args = ap.parse_args()
    if ffh_agents is None:
        args.cache = args.slots = False
    else:
        if args.cache and not os.path.isdir(ffh_agents.ROOT):
            os.makedirs(ffh_agents.ROOT, exist_ok=True)
        if args.cache is None:
            args.cache = ffh_agents.cache_enabled()
        if args.slots is None:
            args.slots = ffh_agents.slots_enabled()
        if args.cache:
            key, why = ffh_agents.run_key([args.bin, 'stats'])
            if key is None:
                print('ai_metrics: run cache off for %s: %s' % (args.bin, why), file=sys.stderr)
    seeds = parse_seeds(args.seeds)
    at = [int(t) for t in args.at.split(',')]
    if args.every:
        at = sorted(set(at) | set(range(args.every, args.turns + 1, args.every)))
    with ThreadPoolExecutor(max_workers=args.jobs) as ex:
        results = list(ex.map(lambda s: run_one(args, s), seeds))
    if args.out:
        with open(args.out, 'w') as f:
            json.dump(results, f)
    if args.compare:
        with open(args.compare) as f:
            old = json.load(f)
        print_report(old, at, 'before: ' + args.compare)
        print()
    print_report(results, at, args.label, CACHED)


if __name__ == '__main__':
    main()
