"""Transcribe hellrail_ad.wav with word-level timestamps via faster-whisper."""
import json
import os
import sys

from faster_whisper import WhisperModel

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WAV = os.path.join(ROOT, "public", "hellrail_ad.wav")
OUT = os.path.join(ROOT, "scripts", "whisper_words.json")

model_size = sys.argv[1] if len(sys.argv) > 1 else "small"
model = WhisperModel(model_size, device="cpu", compute_type="int8")

segments, info = model.transcribe(
    WAV,
    language="en",
    word_timestamps=True,
    beam_size=5,
    condition_on_previous_text=False,
)

words = []
for seg in segments:
    for w in seg.words or []:
        words.append({"w": w.word.strip(), "s": round(w.start, 3), "e": round(w.end, 3), "p": round(w.probability, 3)})
    print(f"[{seg.start:7.2f} - {seg.end:7.2f}] {seg.text}", flush=True)

with open(OUT, "w", encoding="utf-8") as f:
    json.dump(words, f, indent=1)
print(f"\nwrote {len(words)} words -> {OUT}")
