#!/usr/bin/env python3
"""Align known lyric lines to the audio, producing per-line start/end times.

The USLT lyrics have no timestamps, so timing is recovered in three steps:
 1. Decode overlapping 16s windows with a NeMo conformer CTC model
    (sherpa-onnx) to get a timestamped token stream. The transcript is noisy —
    screamed choir sections barely decode — but timestamps are accurate where
    tokens do come out.
 2. Character-level DP alignment (Needleman-Wunsch) of that token stream
    against the ground-truth lyrics; each lyric line collects the timestamps
    of its matched characters (8th/92nd percentile = start/end).
 3. Lines with too few matches (the screamed ones) are interpolated between
    anchored neighbors, weighted by syllable count, with slack reserved for
    instrumental gaps.

Usage: align_lyrics.py <audio16k.wav> <model_dir>
  audio16k.wav: 16 kHz mono (ffmpeg -ac 1 -ar 16000 -af "highpass=f=200,lowpass=f=3800")
  model_dir:    sherpa-onnx NeMo CTC model dir (model.int8.onnx + tokens.txt)
Reads lines.json, writes timed_lines.json.
"""
import json
import re
import sys
import wave

import numpy as np
import sherpa_onnx


def norm(text):
    text = text.replace("—", " ").replace("’", "'").lower()
    text = re.sub(r"[^a-z ]", " ", text)
    return re.sub(r"\s+", " ", text).strip()


def syllables(text):
    return sum(
        max(1, len(re.findall(r"[aeiouy]+", w)))
        for w in re.findall(r"[a-zA-Z']+", text.lower())
    )


def decode_tokens(wav_path, model_dir):
    rec = sherpa_onnx.OfflineRecognizer.from_nemo_ctc(
        model=f"{model_dir}/model.int8.onnx",
        tokens=f"{model_dir}/tokens.txt",
        num_threads=4,
    )
    w = wave.open(wav_path)
    sr = w.getframerate()
    audio = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
    audio = audio.astype(np.float32) / 32768.0
    total = len(audio) / sr

    events = []
    for start in np.arange(0.0, total, 8.0):
        seg = audio[int(start * sr): int((start + 16.0) * sr)]
        if len(seg) < sr:
            break
        s = rec.create_stream()
        s.accept_waveform(sr, seg)
        rec.decode_stream(s)
        r = s.result
        for tok, ts in zip(r.tokens, r.timestamps):
            events.append((round(start + ts, 2), tok))

    events.sort()
    merged = []
    for ts, tok in events:  # dedupe overlap: same token within 0.25s
        if any(abs(ts - mts) <= 0.25 and tok == mtok for mts, mtok in merged[-8:]):
            continue
        merged.append((ts, tok))
    return merged, total


def dp_align(lines, events):
    """Match hypothesis chars to reference chars; return per-line matched timestamps."""
    hyp_chars, hyp_ts = [], []
    for ts, tok in events:
        for ch in tok.replace("▁", " ").lower():
            if ch.isalpha() or ch == " ":
                hyp_chars.append(ch)
                hyp_ts.append(ts)

    ref_chars, ref_line = [], []
    for i, l in enumerate(lines):
        for ch in norm(l["text"]) + " ":
            ref_chars.append(ch)
            ref_line.append(i)

    n, m = len(ref_chars), len(hyp_chars)
    S = np.zeros((n + 1, m + 1), dtype=np.int32)
    P = np.zeros((n + 1, m + 1), dtype=np.int8)  # 1=diag 2=up 3=left
    for i in range(1, n + 1):
        S[i, 0] = S[i - 1, 0] - 1
        P[i, 0] = 2
    for i in range(1, n + 1):
        rc = ref_chars[i - 1]
        for j in range(1, m + 1):
            d = S[i - 1, j - 1] + (2 if rc == hyp_chars[j - 1] else -1)
            u = S[i - 1, j] - 1
            l_ = S[i, j - 1] - 1
            best = max(d, u, l_)
            S[i, j] = best
            P[i, j] = 1 if best == d else (2 if best == u else 3)

    match_ts = [[] for _ in lines]
    i, j = n, int(np.argmax(S[n]))
    while i > 0 and j > 0:
        p = P[i, j]
        if p == 1:
            if ref_chars[i - 1] == hyp_chars[j - 1] and ref_chars[i - 1] != " ":
                match_ts[ref_line[i - 1]].append(hyp_ts[j - 1])
            i -= 1
            j -= 1
        elif p == 2:
            i -= 1
        else:
            j -= 1
    return match_ts


def main(wav_path, model_dir):
    lines = json.load(open("lines.json"))
    events, total = decode_tokens(wav_path, model_dir)
    match_ts = dp_align(lines, events)

    for l, ts in zip(lines, match_ts):
        ts = sorted(ts)
        if len(ts) >= 5:
            l["start"] = round(float(np.percentile(ts, 8)), 2)
            l["end"] = round(float(np.percentile(ts, 92)), 2)
        else:
            l["start"] = l["end"] = None

    # interpolate unanchored runs, syllable-weighted with instrumental slack
    n = len(lines)
    i = 0
    while i < n:
        if lines[i]["start"] is None:
            j = i
            while j < n and lines[j]["start"] is None:
                j += 1
            left = lines[i - 1]["end"] if i > 0 else 0.5
            right = lines[j]["start"] if j < n else total - 1.5
            span = right - left
            syls = [syllables(lines[k]["text"]) for k in range(i, j)]
            slack = min(0.25 * span, 3.0)
            t = left + slack * 0.6
            usable = right - t - slack * 0.4
            for k, s in zip(range(i, j), syls):
                dur = usable * s / sum(syls)
                lines[k]["start"] = round(t + 0.15, 2)
                lines[k]["end"] = round(t + dur - 0.15, 2)
                t += dur
            i = j
        else:
            i += 1

    for k in range(n):  # monotonic, min 1s duration
        if k > 0 and lines[k]["start"] < lines[k - 1]["end"]:
            lines[k]["start"] = lines[k - 1]["end"] + 0.05
        lines[k]["end"] = max(lines[k]["end"], lines[k]["start"] + 1.0)

    json.dump(lines, open("timed_lines.json", "w"), indent=1)
    for k, l in enumerate(lines):
        print(f"{k:2d} {l['start']:7.2f}-{l['end']:7.2f} [{l['section']}] {l['text'][:55]}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
