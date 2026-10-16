"""Connected-component labelling (4-connected) via run-based union-find; numpy + python."""
import numpy as np


def label(mask):
    H, W = mask.shape
    parent = [0]

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    lab = np.zeros((H, W), np.int32)
    prev = []
    nxt = 1
    m8 = mask.astype(np.int8)
    for y in range(H):
        d = np.diff(np.concatenate([[0], m8[y], [0]]))
        starts = np.nonzero(d == 1)[0]
        ends = np.nonzero(d == -1)[0]
        runs = []
        j = 0
        for s, e in zip(starts, ends):
            l = nxt
            parent.append(l)
            nxt += 1
            while j < len(prev) and prev[j][1] <= s:
                j += 1
            k = j
            while k < len(prev) and prev[k][0] < e:
                a_, b_ = find(l), find(prev[k][2])
                if a_ != b_:
                    parent[max(a_, b_)] = min(a_, b_)
                k += 1
            runs.append((s, e, l))
            lab[y, s:e] = l
        prev = runs
    roots = np.array([find(i) for i in range(nxt)], np.int32)
    out = roots[lab]
    # compact ids
    u, inv = np.unique(out, return_inverse=True)
    return inv.reshape(out.shape).astype(np.int32)


def components(lab):
    """-> dict id -> (count, x0, y0, x1, y1) for ids > 0."""
    H, W = lab.shape
    ids = lab.ravel()
    n = ids.max() + 1
    cnt = np.bincount(ids, minlength=n)
    ys, xs = np.divmod(np.arange(H * W), W)
    x0 = np.full(n, W); y0 = np.full(n, H); x1 = np.full(n, -1); y1 = np.full(n, -1)
    np.minimum.at(x0, ids, xs); np.minimum.at(y0, ids, ys)
    np.maximum.at(x1, ids, xs); np.maximum.at(y1, ids, ys)
    return {i: (int(cnt[i]), int(x0[i]), int(y0[i]), int(x1[i]), int(y1[i])) for i in range(1, n) if cnt[i]}
