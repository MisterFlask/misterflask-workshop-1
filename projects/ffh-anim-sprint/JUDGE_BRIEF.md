# Brief for judges

You are judging, blind, several methods for animating one unit sprite in a cheerful 16-bit-style isometric
strategy game (think SNES tactics games): a chibi **swordsman** facing the lower-left, blue tunic, round wooden
shield on his left arm (screen-right), short sword in his right hand (screen-left). Each method is shown only as
a letter. You do not know how any letter was made, and you should not guess. Judge only what you see.

Project root: `/home/user/misterflask-workshop-1/projects/ffh-anim-sprint`. Your packets are in
`judging/packets/<anim>/`. Read `beats.md` for what each animation is meant to show, frame by frame, and its
timing (it is the brief every method was given). Frames are 160×128 at 1×; the game shows them at 1× on
192×96 isometric tiles, so readability at **1×** matters most. In the strips, frame 0 of idle and strike is the
original static sprite (`ref`); every method shares it.

## What is in a packet

- `<L>_shuffled.png`: one letter's frames in a **shuffled** order, labelled p, q, r, s, …
- `compare_2x_j<YOUR NUMBER>.png`: every letter's frames in order at 2×, one row per letter, with frame number,
  duration in ms and the HIT frame marked. The top row is the reference sprite. Row order is shuffled per judge.
- `<L>_strip3x.png`: one letter's frames in order at 3×.
- `compare_1x.png`: every letter at real game scale on a grass tile, with the engine's banner and shadow.
- `<L>_onion.png`: all frames overlaid (later frames stronger) with the sword tip's path as a red trail.
- `<L>_sil.png`: silhouettes of every frame (shape change between poses).
- `metrics.txt`: computed numbers per letter. Use them as a cross-check, not instead of looking.

## What to do, per animation you are given, in this order

1. **Ordering test first.** Before you open any ordered strip of that animation, open each `<L>_shuffled.png`
   and write down the order you believe the frames play in (e.g. `"r p t q s u w v"`), using the beat sheet. A
   method whose poses read clearly should be easy to put in order.
2. Now open the ordered material: your `compare_2x_j<N>.png`, each `<L>_strip3x.png`, `compare_1x.png`, onions
   and silhouettes.
3. **Odd frame out:** for each letter, the frame number that looks most like a *different character* (size, face,
   outfit, colours, props), or `"none"`.
4. **Defects:** for each letter, a short list of concrete defects with frame numbers (sword or shield in the wrong
   hand, facing flip, missing prop, broken anatomy, size pop, colour shift, stray background pixels, cropped
   parts, floating feet).
5. **Scores**, 1–10 each, using the full range (5 = mediocre, 8 = good enough to ship, 10 = flawless):
   - `motion` (weight 35%): reads instantly as this action at 1×; clear key poses whose silhouettes differ;
     anticipation, follow-through and timing that sell the action.
   - `consistency` (30%): the same character in every frame; no drift in size, face, outfit, colours or props;
     planted feet stay planted; no visible pop between frame 0 and frame 1.
   - `style` (20%): matches the reference sprite and would sit in the same game (outline colour, pixel size, soft
     chunky shading, palette, chibi proportions).
   - `appeal` (15%): feels bespoke and hand-animated, as opposed to stiff, mechanical or puppet-like (a
     cardboard cut-out being moved or rotated).
6. **Ranking:** all letters, best first.
7. **Biggest flaw:** one sentence per letter.

Be decisive: if two letters differ, the scores should show it. Then write your result as JSON to
`judging/judge_<N>_<first anim>-<second anim>.json` in this shape, and return the same object:

```json
{"judge": 1, "animations": {
  "strike": {"ordering": {"B": "r p t q s u w v"}, "odd_frame": {"B": "5"},
             "defects": {"B": ["frame 3: shield on the sword arm"]},
             "scores": {"B": {"motion": 7, "consistency": 8, "style": 8, "appeal": 6}},
             "ranking": ["B", "D"], "flaws": {"B": "one sentence"}, "notes": "optional, short"}}}
```
