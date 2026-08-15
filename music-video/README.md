# Stokers of the Underworld — Music Video Pipeline

Turns the Suno-generated MP3 into a synced 1080p lyric video, entirely offline
(no lyric-timing API available — timings are recovered from the audio itself).

## Is MP3 adequate?

Yes — more than adequate, because Suno embeds everything needed in ID3 tags:

| Tag | Contents |
|---|---|
| `USLT` | Full lyrics with `[Verse]`/`[Chorus]`/`[Bridge]` section headers |
| `APIC` | 360×360 cover art (used for the video background) |
| `WOAS` | Link back to the Suno song page |

The one thing MP3 lyrics *don't* carry is timing (USLT is unsynced; SYLT
would be synced but Suno doesn't write it), so alignment is the real work.

## Pipeline

```
parse_lyrics.py song.mp3      # USLT lyrics + APIC cover -> lyrics.txt, lines.json, cover.jpg
ffmpeg -i song.mp3 -ac 1 -ar 16000 -af "highpass=f=200,lowpass=f=3800" audio16k.wav
align_lyrics.py audio16k.wav <nemo-ctc-model-dir>   # -> timed_lines.json
make_subtitles.py             # -> lyrics.ass (styled karaoke track)
./render.sh out.mp4           # cover-art background + waveform + ASS overlay
```

### How alignment works

1. **CTC decode with timestamps** — a NeMo conformer CTC model (via
   `sherpa-onnx`, model fetched from the k2-fsa GitHub release assets) decodes
   overlapping 16 s windows into a timestamped token stream. Transcription of
   a screaming demon choir is predictably poor, but wherever tokens *do* come
   out, their timestamps are good.
2. **Character-level DP alignment** — Needleman-Wunsch aligns the noisy token
   stream against the known ground-truth lyrics; each lyric line collects the
   timestamps of its matched characters.
3. **Syllable-weighted interpolation** — lines with too few matches (the
   screamed choruses) get times interpolated between anchored neighbors,
   weighted by syllable count, with slack reserved for instrumental gaps.

19 of 26 lines anchored directly; 7 interpolated. Forced alignment with
pocketsphinx was tried first and fell apart on exactly those choir sections.

### Styling

`lyrics.ass` styles by section: warm white verses, molten yellow pop-in
choruses, icy blue serif whisper for the bridge (hell freezing over), an
oversized break line, plus title/end cards. `render.sh` composites a
slow-drifting blurred cover-art background, a glowing ember waveform
(`showwaves` + gaussian glow, screen-blended), film grain, vignette, and the
subtitle track.

## Dependencies

- Python: `mutagen`, `numpy`, `sherpa-onnx`
- ffmpeg (with libass), DejaVu fonts
- Model: `sherpa-onnx-nemo-ctc-en-conformer-medium` from
  https://github.com/k2-fsa/sherpa-onnx/releases/tag/asr-models
