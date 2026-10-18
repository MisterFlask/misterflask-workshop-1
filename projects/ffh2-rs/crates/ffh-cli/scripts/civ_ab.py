#!/usr/bin/env python3
"""Paired A/B of one civilization's play: the civ is forced into slot 0 of every game
(`ffh stats --civ`), both sides play the same seeds, and the report follows that civ only.

Each side is a binary plus environment variables, so the same binary can be compared with and
without a civ's own logic (`FFH_AI_CIV_OFF=CIVILIZATION_X`, see crates/ffh-ai/src/civs.rs), or a
branch against its base. Games go through `ai_metrics.py` (run cache, CPU slots).

Per civ and pooled over civs, for the forced civ (player 0):
- win: its team decided the game (victory, last civ standing, or `--until-supremacy`'s clear
  supremacy); a game that reaches the turn cap undecided goes to the team with the largest share
  of the world (score v2) at the end -- `decided` counts only the first kind;
- share: its score v2 (basis points of the world, 10000 = all; dead = 0) at each checkpoint;
- legacy: FFH2's score at each checkpoint (dead = 0); alive at the end; rank by share at the end;
- usage: games where it cast its world spell, and the mean count of each unit, building and
  project type that differs between the sides (`ffh stats --detail`, on unless `--no-detail`).
Win differences use the discordant pairs (seeds won by one side only) with an exact two-sided
sign test; continuous ones the paired t over seeds (|t| < 2 is noise).

usage:
  civ_ab.py --civ CIVILIZATION_X [--civ ..|--civ all] --seeds 1-64 --base-bin B/ffh --new-bin N/ffh
            [--base-env FFH_AI_CIV_OFF=CIVILIZATION_X] [--new-env K=V] [--off-base]
            [--turns 690] [--every 50] [--no-supremacy] [--no-detail] [--out-dir DIR] [--jobs 4]
  civ_ab.py --load DIR [--civ ..]          # report on results saved by an earlier run

--off-base sets FFH_AI_CIV_OFF=<the civ> on the base side (the default base binary is then the new
one): the civ with and without its logic, against identical opponents.
"""
import argparse
import json
import math
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
AI_METRICS = os.path.join(HERE, 'ai_metrics.py')

# Civilizations a game can start with (bAIPlayable); the Infernals and Mercurians join mid-game.
PLAYABLE = ['AMURITES', 'BALSERAPHS', 'BANNOR', 'CALABIM', 'CLAN_OF_EMBERS', 'DOVIELLO', 'ELOHIM',
            'GRIGORI', 'HIPPUS', 'ILLIANS', 'KHAZAD', 'KURIOTATES', 'LANUN', 'LJOSALFAR', 'LUCHUIRP',
            'MALAKIM', 'SHEAIM', 'SIDAR', 'SVARTALFAR']


def civ_name(c):
    c = c.upper()
    return c if c.startswith('CIVILIZATION_') else 'CIVILIZATION_' + c


def short(c):
    return civ_name(c)[len('CIVILIZATION_'):]


def side_path(out_dir, side, civ):
    return os.path.join(out_dir, '%s_%s.json' % (side, short(civ).lower()))


def run_side(a, side, civ, binary, env_pairs):
    out = side_path(a.out_dir, side, civ)
    env = dict(os.environ)
    for kv in env_pairs:
        k, _, v = kv.partition('=')
        env[k] = v.replace('{civ}', civ_name(civ))
    cmd = [sys.executable, AI_METRICS, '--bin', binary, '--seeds', a.seeds, '--civs', str(a.civs),
           '--size', a.size, '--turns', str(a.turns), '--at', a.at, '--civ', civ_name(civ),
           '--jobs', str(a.jobs), '--out', out, '--label', side]
    if a.every:
        cmd += ['--every', str(a.every)]
    if not a.no_supremacy:
        cmd += ['--until-supremacy']
    if a.detail:
        cmd += ['--detail']
    for opt in a.option:
        cmd += ['--option', opt]
    r = subprocess.run(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-4000:])
        raise SystemExit('ai_metrics.py failed for %s %s' % (side, civ))
    for line in r.stdout.splitlines():
        if 'from the run cache' in line or 'games' in line and 'cache' in line:
            print('  [%s %s] %s' % (side, short(civ), line.strip()))
    return out


CRASHED = {}  # path -> seeds whose game failed (no result; left out of the pairs)


def load(path):
    with open(path) as f:
        games = json.load(f)
    CRASHED[path] = sorted(g['seed'] for g in games if 'error' in g)
    return {g['seed']: g for g in games if 'error' not in g}


def rows_at(game, turn):
    return game['snapshots'].get(str(turn))


def p0(rows):
    for r in rows or []:
        if r['p'] == 0:
            return r
    return None


def final_turn(game):
    return max(int(t) for t in game['snapshots'])


def winner_team(game):
    """(team, decided): the deciding team, or the top share at the end of an undecided game."""
    o = game.get('outcome') or {}
    if o.get('kind') in ('victory', 'supremacy', 'last_standing') and o.get('team', -1) >= 0:
        return o['team'], True
    rows = [r for r in rows_at(game, final_turn(game)) if not r.get('barb') and r.get('alive')]
    by_team = {}
    for r in rows:
        by_team[r['team']] = by_team.get(r['team'], 0) + r.get('score', 0)
    if not by_team:
        return -1, False
    return max(sorted(by_team), key=lambda t: by_team[t]), False


def civ_row_value(game, turn, field):
    if turn == 'end':
        turn = final_turn(game)
    else:
        # A game decided before this checkpoint keeps its final state.
        turn = min(int(turn), final_turn(game))
    r = p0(rows_at(game, turn))
    if r is None:
        return 0
    if not r.get('alive', True):
        return 0
    v = r.get(field, 0)
    return v if isinstance(v, (int, float)) and not isinstance(v, bool) else (1 if v is True else 0)


def rank_end(game):
    rows = [r for r in rows_at(game, final_turn(game)) if not r.get('barb')]
    rows.sort(key=lambda r: (-(r.get('score', 0) if r.get('alive') else -1), r['p']))
    for i, r in enumerate(rows):
        if r['p'] == 0:
            return i + 1
    return len(rows)


def game_facts(game, checkpoints):
    me = p0(rows_at(game, final_turn(game)))
    team = me['team'] if me else 0
    wt, decided = winner_team(game)
    f = {
        'win': 1 if wt == team else 0,
        'decided_win': 1 if (wt == team and decided) else 0,
        'alive': 1 if (me and me.get('alive')) else 0,
        'rank': rank_end(game),
        'world_spell': 1 if any(p0(rows).get('world_spell') for rows in game['snapshots'].values() if p0(rows)) else 0,
        'final_turn': final_turn(game),
        'leader': me.get('leader') if me else '?',
    }
    spell_turns = sorted(int(t) for t, rows in game['snapshots'].items() if p0(rows) and p0(rows).get('world_spell'))
    f['spell_turn'] = spell_turns[0] if spell_turns else None
    for t in checkpoints + ['end']:
        f['share@%s' % t] = civ_row_value(game, t, 'score')
        f['legacy@%s' % t] = civ_row_value(game, t, 'legacy_score')
    # Detail counts at the end (alive or not: what it had built by then).
    for kind in ('unit_types', 'building_types', 'project_types'):
        f[kind] = (me or {}).get(kind) or {}
    return f


def mean(v):
    return sum(v) / len(v) if v else float('nan')


def paired_t(a, b):
    d = [y - x for x, y in zip(a, b)]
    n = len(d)
    if n < 2:
        return mean(d), float('nan')
    m = mean(d)
    var = sum((x - m) ** 2 for x in d) / (n - 1)
    if var == 0:
        return m, float('inf') if m else 0.0
    return m, m / math.sqrt(var / n)


def sign_test(k, n):
    """Exact two-sided binomial p-value of k successes in n trials at p = 1/2."""
    if n == 0:
        return 1.0
    k = min(k, n - k)
    tail = sum(math.comb(n, i) for i in range(0, k + 1)) / 2 ** n
    return min(1.0, 2 * tail)


def compare(base, new, checkpoints):
    seeds = sorted(set(base) & set(new))
    fb = [game_facts(base[s], checkpoints) for s in seeds]
    fn = [game_facts(new[s], checkpoints) for s in seeds]
    return seeds, fb, fn


def report(label, seeds, fb, fn, checkpoints, detail=False):
    n = len(seeds)
    print('== %s: %d paired games ==' % (label, n))
    wb, wn = sum(f['win'] for f in fb), sum(f['win'] for f in fn)
    gained = sum(1 for x, y in zip(fb, fn) if y['win'] and not x['win'])
    lost = sum(1 for x, y in zip(fb, fn) if x['win'] and not y['win'])
    db, dn = sum(f['decided_win'] for f in fb), sum(f['decided_win'] for f in fn)
    print('  win rate      %5.1f%% -> %5.1f%%  (won only by new %d, only by base %d, sign test p %.3f)'
          % (100 * wb / max(n, 1), 100 * wn / max(n, 1), gained, lost, sign_test(gained, gained + lost)))
    print('  decided wins  %5.1f%% -> %5.1f%%' % (100 * db / max(n, 1), 100 * dn / max(n, 1)))
    for key in ['share@%s' % t for t in checkpoints + ['end']] + ['legacy@%s' % t for t in checkpoints + ['end']] + ['alive', 'rank']:
        a = [f[key] for f in fb]
        b = [f[key] for f in fn]
        m, t = paired_t(a, b)
        print('  %-13s %8.1f -> %8.1f  (diff %+8.1f, t %5.2f)' % (key, mean(a), mean(b), m, t))
    ws_b = sum(f['world_spell'] for f in fb)
    ws_n = sum(f['world_spell'] for f in fn)
    tb = [f['spell_turn'] for f in fb if f['spell_turn'] is not None]
    tn = [f['spell_turn'] for f in fn if f['spell_turn'] is not None]
    print('  world spell   %d -> %d games (first snapshot with it: %s -> %s)' % (
        ws_b, ws_n, '%.0f' % mean(tb) if tb else '-', '%.0f' % mean(tn) if tn else '-'))
    print('  game length   %.0f -> %.0f turns' % (mean([f['final_turn'] for f in fb]), mean([f['final_turn'] for f in fn])))
    if detail:
        for kind in ('unit_types', 'building_types', 'project_types'):
            keys = sorted(set(k for f in fb + fn for k in f[kind]))
            diffs = []
            for k in keys:
                a = mean([f[kind].get(k, 0) for f in fb])
                b = mean([f[kind].get(k, 0) for f in fn])
                if abs(b - a) >= 0.25 or (a == 0) != (b == 0):
                    diffs.append((abs(b - a), k, a, b))
            diffs.sort(reverse=True)
            if diffs:
                print('  %s at the end (mean per game, changed): %s' % (kind, ', '.join(
                    '%s %.1f->%.1f' % (k, a, b) for _, k, a, b in diffs[:14])))
    return {'n': n, 'wins_base': wb, 'wins_new': wn, 'gained': gained, 'lost': lost}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--civ', action='append', default=[])
    ap.add_argument('--seeds', default='1-32')
    ap.add_argument('--base-bin')
    ap.add_argument('--new-bin')
    ap.add_argument('--base-env', action='append', default=[], help='K=V for the base side ({civ} = the civ)')
    ap.add_argument('--new-env', action='append', default=[], help='K=V for the new side')
    ap.add_argument('--off-base', action='store_true', help='base side: FFH_AI_CIV_OFF=<civ>')
    ap.add_argument('--civs', type=int, default=6)
    ap.add_argument('--size', default='WORLDSIZE_STANDARD')
    ap.add_argument('--turns', type=int, default=690)
    ap.add_argument('--at', default='100,200,300')
    ap.add_argument('--every', type=int, default=50)
    ap.add_argument('--no-supremacy', action='store_true', help='play to --turns, no early stop')
    ap.add_argument('--no-detail', dest='detail', action='store_false', help='no per-type counts (on by default)')
    ap.add_argument('--option', action='append', default=[])
    ap.add_argument('--out-dir', default='civ_ab_out')
    ap.add_argument('--load', default=None, help='report on a directory from an earlier run')
    ap.add_argument('--jobs', type=int, default=4)
    a = ap.parse_args()
    civs = a.civ or ['all']
    if 'all' in [c.lower() for c in civs]:
        civs = PLAYABLE
    civs = [short(c) for c in civs]
    checkpoints = [t for t in a.at.split(',') if t]
    out_dir = a.load or a.out_dir
    a.out_dir = out_dir
    os.makedirs(out_dir, exist_ok=True)
    if not a.load:
        if a.off_base:
            a.base_env = a.base_env + ['FFH_AI_CIV_OFF={civ}']
            a.base_bin = a.base_bin or a.new_bin
        if not a.base_bin or not a.new_bin:
            raise SystemExit('need --base-bin and --new-bin (or --load DIR)')
        for civ in civs:
            run_side(a, 'base', civ, a.base_bin, a.base_env)
            run_side(a, 'new', civ, a.new_bin, a.new_env)
    pooled_b, pooled_n, totals = [], [], []
    for civ in civs:
        pb, pn = side_path(out_dir, 'base', civ), side_path(out_dir, 'new', civ)
        if not (os.path.exists(pb) and os.path.exists(pn)):
            print('== %s: no results in %s ==' % (civ, out_dir))
            continue
        seeds, fb, fn = compare(load(pb), load(pn), checkpoints)
        totals.append((civ, report(civ, seeds, fb, fn, checkpoints, a.detail)))
        if CRASHED[pb] or CRASHED[pn]:
            print('  crashed games (left out of the pairs): base seeds %s, new seeds %s' % (CRASHED[pb] or '-', CRASHED[pn] or '-'))
        pooled_b += fb
        pooled_n += fn
    if len(totals) > 1:
        report('pooled over %d civs' % len(totals), list(range(len(pooled_b))), pooled_b, pooled_n, checkpoints)
        print('  per civ wins base -> new: ' + ', '.join('%s %d->%d' % (c, t['wins_base'], t['wins_new']) for c, t in totals))


if __name__ == '__main__':
    main()
