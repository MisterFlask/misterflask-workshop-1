"""The pixel-over pass for cut and composed unit figures (docs/ART.md 4.3, 10.2).

A figure box-downsampled from a painted sheet and snapped to the palette pixel by pixel comes out
as salt and pepper: a third or more of its pixels have no same-colour neighbour, neighbours of one
material differ in hue but not in light, painted dark lines turn to outline black inside the
figure, the grass corners of the painted hex base survive at knee height, and the owner colour
lands as 1-3 px flecks. These steps do what a pixel artist's clean-up does, in index space:

  wings      the base's corners: in the rows just above the feet, regions outside the feet's
             column span that are mostly grass are dropped with their dark rim
  interior   specks of outline black (1-2 px) inside one material take its darkest used tone;
             longer dark lines and shadows stay, they draw the forms
  tones      at most three tones per material (colours of about one hue; skin on its own): the
             lightest and darkest kept, then the most used; the rest merge into the nearest kept
             tone in light
  coherence  a pixel alone in its family among eight neighbours of one family, at about their
             value, joins that family (the nearest tone in light): chroma noise becomes form
  despeckle  a pixel with no same-colour 4-neighbour takes its neighbours' majority colour when
             that colour is near it in light; glints and dark accents (eyes, rivets) stay
  flecks     owner-slot islands under 4 px go back to their neighbours' colour

`pre` runs before quantize.py's outline, `post` after its colour reduction, `flecks` after the
owner slots. Every step keeps the silhouette's outline pixels ('0' on an edge) as they are."""
import numpy as np
from collections import Counter

TRANS = -1
GRASS = 'qmoOAzwktT'
N4 = ((-1, 0), (1, 0), (0, -1), (0, 1))
N8 = tuple((dy, dx) for dy in (-1, 0, 1) for dx in (-1, 0, 1) if dy or dx)


class Pal:
    """The palette as the pass needs it: keys, Lab lightness and family per index."""

    def __init__(self, keys, lab, families):
        self.keys = list(keys)
        self.K = {k: i for i, k in enumerate(self.keys)}
        self.L = np.asarray(lab)[:, 0]
        self.ab = np.asarray(lab)[:, 1:]
        self.fam = list(families)
        self.void = self.K['0']


def _nbrs(idx, y, x, offs):
    H, W = idx.shape
    for dy, dx in offs:
        yy, xx = y + dy, x + dx
        yield (idx[yy, xx] if 0 <= yy < H and 0 <= xx < W else TRANS)


def wings(idx, P, band=4, feet=5):
    """Drop the painted base's corners: in the `band` rows above the feet, every region outside the
    feet's column span (the bottom `feet` rows) that is mostly grass goes, with its rim."""
    ys, xs = np.nonzero(idx != TRANS)
    if not len(ys):
        return idx
    yb = ys.max()
    H, W = idx.shape
    grass = {P.K[k] for k in GRASS}
    fr = idx[max(0, yb - feet + 1):yb + 1]
    cols = [x for x in range(W) if any(v != TRANS and v not in grass and v != P.void for v in fr[:, x])]
    if not cols:
        return idx
    x0, x1 = min(cols) - 1, max(cols) + 1
    y0, y1 = max(0, yb - feet - band + 1), yb
    out = idx.copy()
    seen = np.zeros_like(idx, bool)
    for sy in range(y0, y1 + 1):
        for sx in list(range(0, max(0, x0))) + list(range(x1 + 1, W)):
            if idx[sy, sx] == TRANS or seen[sy, sx]:
                continue
            comp, stack = [], [(sy, sx)]
            seen[sy, sx] = True
            while stack:
                y, x = stack.pop()
                comp.append((y, x))
                for dy, dx in N8:
                    yy, xx = y + dy, x + dx
                    if y0 <= yy <= y1 and 0 <= xx < W and (xx < x0 or xx > x1) and not seen[yy, xx] and idx[yy, xx] != TRANS:
                        seen[yy, xx] = True
                        stack.append((yy, xx))
            body = [p for p in comp if P.L[idx[p]] >= 34]
            if body and sum(idx[p] in grass for p in body) >= 0.4 * len(body):
                for p in comp:
                    out[p] = TRANS
    gone = (out == TRANS) & (idx != TRANS)
    if not gone.any():
        return idx
    # the wings' dark rims, left floating beside them, go too
    for y in range(y0, y1 + 1):
        for x in range(W):
            if out[y, x] == TRANS or P.L[out[y, x]] >= 34 or (x0 <= x <= x1):
                continue
            near = any(0 <= y + dy < H and 0 <= x + dx < W and gone[y + dy, x + dx] for dy, dx in N8)
            if near and not any(v != TRANS and P.L[v] >= 34 for v in _nbrs(out, y, x, N8)):
                out[y, x] = TRANS
    # never cut the figure apart: if the removal disconnects any of the body, undo it
    lost = (out != TRANS) & ~_largest(out)
    if sum(1 for v in idx[lost] if v not in grass and v != P.void) > 2:
        return idx
    out[lost] = TRANS
    return out


def _largest(idx):
    """Mask of the largest 8-connected opaque component."""
    H, W = idx.shape
    seen = np.zeros(idx.shape, bool)
    best = []
    for sy in range(H):
        for sx in range(W):
            if idx[sy, sx] == TRANS or seen[sy, sx]:
                continue
            comp, stack = [], [(sy, sx)]
            seen[sy, sx] = True
            while stack:
                y, x = stack.pop()
                comp.append((y, x))
                for dy, dx in N8:
                    yy, xx = y + dy, x + dx
                    if 0 <= yy < H and 0 <= xx < W and not seen[yy, xx] and idx[yy, xx] != TRANS:
                        seen[yy, xx] = True
                        stack.append((yy, xx))
            if len(comp) > len(best):
                best = comp
    m = np.zeros(idx.shape, bool)
    for p in best:
        m[p] = True
    return m


def interior(idx, P, max_size=2):
    """Isolated specks of outline black inside one material (at most `max_size` px, every 4-neighbour
    opaque, six of the eight neighbours of one family) take that family's darkest used tone. Longer
    dark lines and shadows stay: they draw the figure's forms."""
    used = Counter(int(v) for v in idx.ravel() if v != TRANS and v != P.void)
    darkest = {}
    for v in used:
        f = P.fam[v]
        if f not in darkest or P.L[v] < P.L[darkest[f]]:
            darkest[f] = v
    H, W = idx.shape
    blk = (idx == P.void)
    seen = np.zeros_like(blk)
    out = idx.copy()
    for sy in range(H):
        for sx in range(W):
            if not blk[sy, sx] or seen[sy, sx]:
                continue
            comp, stack = [], [(sy, sx)]
            seen[sy, sx] = True
            while stack:
                y, x = stack.pop()
                comp.append((y, x))
                for dy, dx in N4:
                    yy, xx = y + dy, x + dx
                    if 0 <= yy < H and 0 <= xx < W and blk[yy, xx] and not seen[yy, xx]:
                        seen[yy, xx] = True
                        stack.append((yy, xx))
            if len(comp) > max_size:
                continue
            if any(v == TRANS for (y, x) in comp for v in _nbrs(idx, y, x, N4)):
                continue
            fams = Counter(P.fam[v] for (y, x) in comp for v in _nbrs(idx, y, x, N8) if v != TRANS and v != P.void)
            if not fams:
                continue
            f, k = fams.most_common(1)[0]
            if k < 6 * len(comp) * 0.75 or f not in darkest or P.L[darkest[f]] >= 42:
                continue
            for p in comp:
                out[p] = darkest[f]
    return out


def tones(idx, P, cap=3, hue_tol=18.0, protect=('skin',), keep_min=2):
    """At most `cap` tones per material (4.3: two to four tones per material). A material is a run
    of used colours of about one hue (Lab a,b within `hue_tol` of its most used colour); skin is
    always its own material. Each keeps its lightest and darkest tone (when they cover `keep_min`
    px), then its most used; the rest merge into the kept tone nearest in light."""
    used = Counter(int(v) for v in idx.ravel() if v != TRANS and v != P.void and v >= 0)
    groups = []
    for v in sorted(used, key=lambda v: -used[v]):
        for gr in groups:
            ref = gr[0]
            same_class = (P.fam[ref] in protect) == (P.fam[v] in protect)
            if same_class and np.hypot(*(P.ab[ref] - P.ab[v])) < hue_tol:
                gr.append(v)
                break
        else:
            groups.append([v])
    out = idx.copy()
    for vs in groups:
        if len(vs) <= cap:
            continue
        byL = sorted(vs, key=lambda v: P.L[v])
        keep = [v for v in (byL[0], byL[-1]) if used[v] >= keep_min]
        for v in vs:
            if len(keep) >= cap:
                break
            if v not in keep:
                keep.append(v)
        for v in vs:
            if v not in keep:
                out[idx == v] = min(keep, key=lambda k: abs(P.L[k] - P.L[v]))
    return out


def coherence(idx, P, tol=14.0):
    """A pixel alone in its family among neighbours of one family, at about their light, joins it."""
    used = sorted({int(v) for v in idx.ravel() if v != TRANS and v != P.void})
    out = idx.copy()
    H, W = idx.shape
    for y in range(H):
        for x in range(W):
            v = idx[y, x]
            if v == TRANS or v == P.void:
                continue
            nb = [n for n in _nbrs(idx, y, x, N8) if n != TRANS and n != P.void]
            if len(nb) < 6:
                continue
            fams = Counter(P.fam[n] for n in nb)
            f, k = fams.most_common(1)[0]
            if f == P.fam[v] or fams.get(P.fam[v], 0) or k < 5:
                continue
            mean = float(np.mean([P.L[n] for n in nb]))
            if abs(P.L[v] - mean) > tol:
                continue          # a value feature (eye, rivet, glint), not chroma noise
            cand = [c for c in used if P.fam[c] == f]
            if cand:
                out[y, x] = min(cand, key=lambda c: abs(P.L[c] - P.L[v]))
    return out


def despeckle(idx, P, tol=10.0, passes=2):
    """A pixel with no same-colour 4-neighbour takes its neighbours' majority colour if near in light."""
    out = idx.copy()
    H, W = idx.shape
    for _ in range(passes):
        src = out.copy()
        changed = 0
        for y in range(H):
            for x in range(W):
                v = src[y, x]
                if v == TRANS or v == P.void or any(n == v for n in _nbrs(src, y, x, N4)):
                    continue
                nb = [n for n in _nbrs(src, y, x, N8) if n != TRANS and n != P.void]
                if len(nb) < 4:
                    continue
                Ls = [P.L[n] for n in nb]
                if P.L[v] > max(Ls) + 8 or P.L[v] < min(Ls) - 8:
                    continue      # a glint or a dark accent
                c, k = Counter(nb).most_common(1)[0]
                if k >= 3 and abs(P.L[c] - P.L[v]) <= tol:
                    out[y, x] = c
                    changed += 1
        if not changed:
            break
    return out


def flecks(idx, before, P, slot_ids=(-2, -3), min_size=4):
    """Owner-slot islands smaller than `min_size` take their neighbours' majority colour."""
    H, W = idx.shape
    slot = np.isin(idx, slot_ids)
    seen = np.zeros_like(slot)
    out = idx.copy()
    for sy in range(H):
        for sx in range(W):
            if not slot[sy, sx] or seen[sy, sx]:
                continue
            comp, stack = [], [(sy, sx)]
            seen[sy, sx] = True
            while stack:
                y, x = stack.pop()
                comp.append((y, x))
                for dy, dx in N4:
                    yy, xx = y + dy, x + dx
                    if 0 <= yy < H and 0 <= xx < W and slot[yy, xx] and not seen[yy, xx]:
                        seen[yy, xx] = True
                        stack.append((yy, xx))
            if len(comp) >= min_size:
                continue
            for y, x in comp:
                nb = [n for n in _nbrs(idx, y, x, N8) if n != TRANS and n != P.void and n not in slot_ids]
                out[y, x] = Counter(nb).most_common(1)[0][0] if nb else before[y, x]
    return out


def pre(idx, P, opt):
    if opt.get('wings', opt.get('feet', True) and not opt.get('ship')):
        idx = wings(idx, P)
    if opt.get('interior', True):
        idx = interior(idx, P)
    return idx


def post(idx, P, opt):
    if opt.get('tones', True):
        idx = tones(idx, P, opt.get('tone_cap', 3), opt.get('hue_tol', 18.0))
    if opt.get('coherence', True):
        idx = coherence(idx, P)
    if opt.get('despeckle', True):
        idx = despeckle(idx, P)
    return idx


def speckle(idx, P):
    """Share of non-outline pixels with no same-colour 4-neighbour (the measure the pass lowers)."""
    n = t = 0
    H, W = idx.shape
    for y in range(H):
        for x in range(W):
            v = idx[y, x]
            if v == TRANS or v == P.void or v < 0:
                continue
            t += 1
            if not any(n_ == v for n_ in _nbrs(idx, y, x, N4)):
                n += 1
    return n / max(1, t)
