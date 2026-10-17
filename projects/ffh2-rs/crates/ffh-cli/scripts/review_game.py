#!/usr/bin/env python3
"""Make one AI game legible for holistic judgement.

Plays a game with `ffh stats` (dense snapshots) and prints, per civilization, a
readable timeline of what it did -- with the emphasis on the things that reveal
whether the AI played *in character for its civilization*: military build-up and
use, wars, cities taken and lost, the victory it aimed at, and whether it touched
its civ-defining levers (the sea, religion, wonders, casters/heroes).

This is a judgement aid, not a scorer. It surfaces facts and a few blunt "this
looks off" flags; a human (or Claude) reads it and decides what to change. It is
deliberately robust to content/stat changes: it reads whatever civs/units the
snapshot reports and never hardcodes a unit, stat threshold or civ roster.

Usage:
  ffh stats --seed 3 --civs 6 --turns 690 --every 50 | review_game.py
  review_game.py --binary <ffh> --seed 3 --civs 6 --turns 690 --every 50
"""
import argparse, json, subprocess, sys


def load(args):
    if args.binary:
        cmd = [args.binary, "stats", "--seed", str(args.seed), "--civs", str(args.civs),
               "--turns", str(args.turns), "--every", str(args.every)]
        if args.until_supremacy:
            cmd.append("--until-supremacy")
        out = subprocess.run(cmd, capture_output=True, text=True).stdout
    else:
        out = sys.stdin.read()
    for line in out.splitlines():
        line = line.strip()
        if line.startswith("{"):
            return json.loads(line)
    raise SystemExit("no JSON stats line found")


def combat_mix(uc):
    """ucombat dict -> compact 'MELEE3 ARCHER2 MOUNTED1' sorted by count desc, dropping NONE."""
    items = [(k, v) for k, v in (uc or {}).items() if k != "NONE" and v]
    items.sort(key=lambda kv: (-kv[1], kv[0]))
    return " ".join(f"{k.split('_')[-1][:5]}{v}" for k, v in items) or "-"


def num(x):
    return x if isinstance(x, (int, float)) else 0


def fmt_eta(e):
    return "-" if e is None else str(e)


def civ_flags(rows, started=True):
    """Blunt 'this looks off for the civ' heuristics from the trajectory. Data-driven:
    they compare a civ's own signals over time, never against a fixed roster/threshold.
    `started` is False for a civ that first appears after the game's opening snapshot (a
    mid-game summon such as the Mercurians/Infernals), which begins with an army and so is
    not judged by start-to-end power growth."""
    end = rows[-1]
    flags = []
    # Reached the coast/overseas or built ships at some point but ended with none / never any.
    ever_ship = max(num(r.get("ships")) for r in rows)
    ever_overseas = max(num(r.get("overseas")) for r in rows)
    if ever_overseas and not ever_ship:
        flags.append("has overseas cities but never built a ship")
    # Fielded an army but never took a city (army that never strikes).
    peak_mil = max(num(r.get("mil")) for r in rows)
    took = num(end.get("taken_civ")) + num(end.get("taken_barb"))
    if peak_mil >= 6 and took == 0:
        flags.append(f"fielded up to {peak_mil} military units but captured 0 cities")
    # Declared/entered no war the whole game despite meeting rivals.
    if num(end.get("met")) >= 1 and num(end.get("wars")) == 0 and took == 0:
        flags.append("met rivals but never went to war or took a city")
    # Military never grew past the starting warriors.
    if peak_mil <= 2 and end.get("alive"):
        flags.append(f"military never grew past {peak_mil} units")
    # Power stagnated: an alive civ whose power barely grew over the game is not increasing its
    # power by the means available (the design goal). (We do NOT flag an idle victory planner:
    # civs greedily grow power and let domination/conquest emerge, rather than chasing a victory.)
    # Skipped for mid-game summons: they spawn with an army, so their first power is already high
    # and start-to-end growth says nothing about whether they stagnated.
    first_pow = num(rows[0].get("power"))
    end_pow = num(end.get("power"))
    if started and end.get("alive") and len(rows) >= 3 and end_pow <= max(first_pow * 3, 60):
        flags.append(f"power barely grew ({first_pow} -> {end_pow}): not increasing power by the means available")
    return flags


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--civs", type=int, default=6)
    ap.add_argument("--turns", type=int, default=690)
    ap.add_argument("--every", type=int, default=50)
    ap.add_argument("--until-supremacy", action="store_true")
    ap.add_argument("--civ", help="only show this civ (substring match on civ name)")
    args = ap.parse_args()

    d = load(args)
    snaps = d["snapshots"]
    turns = sorted(snaps.keys(), key=int)

    # group rows by player id across snapshots
    by_p = {}
    for t in turns:
        for row in snaps[t]:
            by_p.setdefault(row["p"], {})[t] = row

    print(f"# Game review: seed {d.get('seed')}, {d.get('civs')} civs, {d.get('world')}, "
          f"{d.get('turns')} turns")
    oc = d.get("outcome", {})
    print(f"  outcome: {oc.get('kind')}  final_turn={d.get('final_turn')}  "
          f"wars_declared={d.get('wars_declared')}  civ-cities-captured={d.get('captured_civ')}  "
          f"eliminated={d.get('eliminated')}")
    print()

    for p, series in sorted(by_p.items()):
        rows = [series[t] for t in turns if t in series]
        if not rows:
            continue
        first = rows[0]
        if first.get("barb"):
            continue
        if args.civ and args.civ.upper() not in str(first.get("civ", "")).upper():
            continue
        name = f"{first.get('civ')} — {first.get('leader')} ({first.get('alignment')}) [team {first.get('team')}]"
        print(f"=== {name} ===")
        hdr = f"{'t':>4} {'cit':>3} {'pop':>3} {'tech':>4} {'res':>4} {'mil':>3} {'best':>5} {'combat':<16} {'war':>3} {'takC/B':>7} {'lstC/B':>7} {'wnd':>3} {'cst':>3} {'her':>3} {'pow':>5} {'goal':<10} {'eta':>4}"
        print(hdr)
        for t in turns:
            r = series.get(t)
            if not r:
                continue
            if not r.get("alive") and num(r.get("cities")) == 0:
                print(f"{t:>4}  (eliminated ~turn {r.get('elim_turn')})")
                continue
            print(f"{t:>4} {num(r.get('cities')):>3} {num(r.get('pop')):>3} {num(r.get('techs')):>4} "
                  f"{num(r.get('research')):>4} {num(r.get('mil')):>3} {num(r.get('mil_best')):>5.0f} "
                  f"{combat_mix(r.get('ucombat')):<16} {num(r.get('wars')):>3} "
                  f"{str(num(r.get('taken_civ')))+'/'+str(num(r.get('taken_barb'))):>7} "
                  f"{str(num(r.get('lost_civ')))+'/'+str(num(r.get('lost_barb'))):>7} "
                  f"{num(r.get('wonders')):>3} {num(r.get('casters')):>3} {num(r.get('heroes')):>3} "
                  f"{num(r.get('power')):>5.0f} {str(r.get('goal')):<10} {fmt_eta(r.get('goal_eta')):>4}")
        end = rows[-1]
        print(f"  end: cities={num(end.get('cities'))} techs={num(end.get('techs'))} "
              f"religion={end.get('religion')} wonders={num(end.get('wonders'))} "
              f"heroes={num(end.get('heroes'))} casters={num(end.get('casters'))} "
              f"took civ/barb={num(end.get('taken_civ'))}/{num(end.get('taken_barb'))} "
              f"lost civ/barb={num(end.get('lost_civ'))}/{num(end.get('lost_barb'))} "
              f"conquest_eta={fmt_eta(end.get('conquest_eta'))} domination_eta={fmt_eta(end.get('domination_eta'))}")
        # A civ present only from a later snapshot spawned mid-game (a summon); it is not judged
        # by start-to-end power growth.
        started = turns[0] in series
        flags = civ_flags(rows, started=started)
        if flags:
            print("  ?? " + "\n  ?? ".join(flags))
        print()


if __name__ == "__main__":
    main()
