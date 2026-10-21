"""Align whisper word timestamps to the known poem text (Needleman-Wunsch),
emit src/lyrics.json with per-word frame times and stanza boundaries."""
import json
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FPS = 30

POEM = """
Rattatat rattling, racketing, roaring—attend to the specs of the HELLRAIL!
Blueprints in brimstone and bolted in brass, built so it never can fail!

Fourteen-foot firebox, flanged and flame-fitted, forged in the furnaces below!
Cast-iron cladding, copper-clad coils, triple-thick tungsten aglow!
Pressure at eight hundred pounds to the pipe, pig-iron pistons that pound!
Boilerplate black as the back of the Pit, banded and burnished and bound!

Wheels of wrought tungsten on tempered-steel tracks, thirty tons turning at speed!
Axles of adamant, oiled and aligned, alloys that avarice decreed!
Grinding through granite and gnashing through gneiss, groaning down gradients grim!
Sparks at the switchback and screaming on slate—Loss of traction? Just a whim!

Diamond-head drillbits that devour the dark, chewing through chalk and through char!
Borers that burrow through basalt and bone, shafts sinking steadily far!
Carbide-tipped cutters that crackle and crunch, coring the crust like a cheese!
Tunnels by Tuesday through tectonics itself—done, if you please, with such ease!

Compartments in crimson with cushions of calf, curtained in damask and down!
Gilt-fitted gaslight and gimballed champagne, the plushest perdition in town!
Lavatories lacquered in luminous lead, lustrous and lavish and loud!
Porters in pearl-buttons, punctual, precise—service to singe any crowd!

Speed? We deliver a thousand miles down in the time that you'd take for your tea!
Velocity verified, viscera-tested, validated, vivid, and free!
Faster than falling, than fear, than the future—the friction coefficient's a laugh!
Passengers pallid with pleasure arrive; check the receipts! Do the math!

Fueled on the fumes of foreclosed-upon farms, on foetor of fortunes gone foul!
Stoked with stock-Loss and shattered ambitions, promissory paper to prowl!
Combustible contracts and kindling of cares, the calorific content of greed!
One burning bushel of broken-backed bonds propels us at blistering speed!

Shares selling swiftly! Subscribe and be saved! Stamps at the station! Don't wait!
Gold by the guilder, the guinea, the groat—invest and incinerate fate!
Signed, sealed, and certified, stoked and en route, the HELLRAIL departs on the hour!
Climb on! Cash in! The conductors are calling! Come claim your combustible DOWER!
"""


def norm(w: str) -> str:
    return re.sub(r"[^a-z0-9']", "", w.lower())


def lev(a: str, b: str) -> int:
    if abs(len(a) - len(b)) > 2:
        return 3
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a):
        cur = [i + 1]
        for j, cb in enumerate(b):
            cur.append(min(prev[j + 1] + 1, cur[j] + 1, prev[j] + (ca != cb)))
        prev = cur
    return prev[-1]


# poem tokens with (stanza, line) tags
poem_words = []  # (word_display, norm, stanza_idx, line_idx_global)
line_texts = []
stanza_of_line = []
gline = 0
for s_idx, stanza in enumerate([s for s in POEM.strip().split("\n\n")]):
    for line in stanza.strip().split("\n"):
        line_clean = line.strip().rstrip(",;")
        line_texts.append(line_clean)
        stanza_of_line.append(s_idx)
        for tok in re.split(r"[\s—–]+", line_clean):
            n = norm(tok)
            if n:
                poem_words.append((tok, n, s_idx, gline))
        gline += 1

with open(os.path.join(ROOT, "scripts", "whisper_words.json"), encoding="utf-8") as f:
    hyp = json.load(f)
hyp = [h for h in hyp if norm(h["w"])]

P, H = len(poem_words), len(hyp)
GAP = -1.2


def score(pi: int, hi: int) -> float:
    a = poem_words[pi][1]
    b = norm(hyp[hi]["w"])
    if a == b:
        return 3.0
    d = lev(a, b)
    if d == 1:
        return 1.6
    if d == 2:
        return 0.5
    return -1.5


# DP
import numpy as np

dp = np.zeros((P + 1, H + 1), dtype=np.float32)
bt = np.zeros((P + 1, H + 1), dtype=np.int8)  # 1 diag, 2 up (skip poem), 3 left (skip hyp)
for i in range(1, P + 1):
    dp[i][0] = dp[i - 1][0] + GAP
    bt[i][0] = 2
for j in range(1, H + 1):
    dp[0][j] = dp[0][j - 1] + GAP
    bt[0][j] = 3
for i in range(1, P + 1):
    for j in range(1, H + 1):
        d = dp[i - 1][j - 1] + score(i - 1, j - 1)
        u = dp[i - 1][j] + GAP
        l = dp[i][j - 1] + GAP
        best = max(d, u, l)
        dp[i][j] = best
        bt[i][j] = 1 if best == d else (2 if best == u else 3)

# traceback
match_t = [None] * P  # timestamp for each poem word
i, j = P, H
n_match = 0
while i > 0 or j > 0:
    m = bt[i][j]
    if m == 1:
        if score(i - 1, j - 1) > 0:
            match_t[i - 1] = (hyp[j - 1]["s"], hyp[j - 1]["e"])
            n_match += 1
        i, j = i - 1, j - 1
    elif m == 2:
        i -= 1
    else:
        j -= 1

print(f"matched {n_match}/{P} poem words ({100*n_match/P:.0f}%)")

# interpolate gaps
idx_known = [k for k, t in enumerate(match_t) if t is not None]
for k in range(P):
    if match_t[k] is None:
        prev = max([x for x in idx_known if x < k], default=None)
        nxt = min([x for x in idx_known if x > k], default=None)
        if prev is None:
            match_t[k] = (match_t[nxt][0] - 0.3, match_t[nxt][0])
        elif nxt is None:
            match_t[k] = (match_t[prev][1], match_t[prev][1] + 0.3)
        else:
            t0, t1 = match_t[prev][1], match_t[nxt][0]
            frac = (k - prev) / (nxt - prev)
            w = (t1 - t0) * 1.0 / (nxt - prev)
            st = t0 + (t1 - t0) * frac
            match_t[k] = (st, st + max(0.12, w * 0.8))

# monotonicity fix
for k in range(1, P):
    if match_t[k][0] < match_t[k - 1][0]:
        match_t[k] = (match_t[k - 1][0] + 0.05, max(match_t[k][1], match_t[k - 1][0] + 0.2))

# build lines
lines = []
wi = 0
for li, text in enumerate(line_texts):
    toks = [t for t in re.split(r"[\s—–]+", text) if norm(t)]
    words = []
    for tok in toks:
        t0, t1 = match_t[wi]
        words.append({"w": tok, "f": round(t0 * FPS)})
        wi += 1
    start = words[0]["f"]
    end = round(match_t[wi - 1][1] * FPS)
    lines.append({
        "text": text,
        "stanza": stanza_of_line[li],
        "start": start,
        "end": end,
        "words": words,
    })

stanzas = []
for s in range(max(stanza_of_line) + 1):
    ls = [l for l in lines if l["stanza"] == s]
    stanzas.append({"start": ls[0]["start"], "end": ls[-1]["end"]})

out = {"lines": lines, "stanzas": stanzas}
with open(os.path.join(ROOT, "src", "lyrics.json"), "w", encoding="utf-8") as f:
    json.dump(out, f, indent=1)

for s_i, s in enumerate(stanzas):
    print(f"stanza {s_i+1}: {s['start']/FPS:6.1f}s - {s['end']/FPS:6.1f}s  (frames {s['start']}-{s['end']})")
print(f"wrote src/lyrics.json with {len(lines)} lines")
