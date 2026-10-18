# ANIMATION.md: unit animation in the 3D renderer, and how to judge it

**Status:** working, experimental (it lives in the experimental 3D renderer, docs/R3D.md). The
warrior and the archer have an idle, an attack and a death; every other figure keeps the 3D
renderer's small procedural idle. The workflow, the runtime and the validator are general: a new
archetype needs clips, not code.

Contents: 1 the workflow · 2 how a clip becomes motion · 3 authoring · 4 judging an animation ·
5 trusting the judge · 6 vision judges · 7 the proof of concept · 8 cost · 9 limits and next steps.

---

## 1. The workflow

```
author keys  ->  bake  ->  check  ->  look  ->  mutate  ->  judge  ->  sign off  ->  play
anim-*.js       anim.js   check.mjs  sheet.mjs  mutate.mjs   vision     the lab      map3d.js
```

1. **Author** a clip as keyed pose deltas in the archetype's file
   (`crates/ffh-play/assets/app/r3d/assets/anim-warrior.js`, `anim-archer.js`; §3).
2. **Check** it: `node tools/r3d-anim.mjs check --recipe unit.warrior [--clip attack] [--races]`
   bakes the clip from the real figure generator and runs every metric (§4). A hard failure names
   the metric, its value, its limit and the worst frame.
3. **Look** at it: `node tools/r3d-anim.mjs sheet --recipe unit.warrior --clip attack --out DIR`
   draws the review sheet: every frame at the true gameplay size and enlarged, a timing chart
   (onion skin, one dot a frame on the striking point, head and pelvis: the spacing is the
   speed), the speed and height curves, the verdicts. Numbers do not replace looking.
4. **Mutate**: `node tools/r3d-anim.mjs mutants --recipe unit.warrior` degrades the clip in the
   ways animation goes wrong and checks that each degradation trips the metric that claims to
   catch it, while the good clip passes (§5). This is the validator's own test.
5. **Judge**: `node tools/r3d-anim.mjs judge-pack --out DIR` writes blind stimuli for vision
   judges (recognition, and good-against-mutant comparisons) with a sealed key (§6).
6. **Sign off** in the real renderer: `node tools/r3d-anim.mjs lab --recipe unit.warrior --clip
   attack --out DIR` renders key frames with the painterly pipeline (headless Chromium; the lab
   page also plays clips live: `r3d-lab/lab.html?grid=unit.warrior,unit.archer&anim=attack`).
7. **Play**: the renderer bakes the clips of every animated look it shows and plays them (§2).

`node --test crates/ffh-play/assets/test/anim.test.mjs` (run by `tools/web-check.sh`) runs steps
2 and 4 for every clip, a race sweep, and the engine's own continuity tests.

## 2. How a clip becomes motion

**Figures are code.** A unit figure is built by `figure()` in `assets/units.js` from a spec (race,
body, garb, a stance, hand targets solved by two-bone IK, what each hand holds). The spec's pose
is the archetype's rest pose (the warrior braced with the axe raised, the archer at full draw).

**A clip is pose deltas.** `figure()` takes an optional `pose`: deltas from the rest pose on a
small set of channels (§3). A clip keys those channels over time; `anim.js` samples them and
**rebuilds each frame with the figure's own recipe** (`registry.js` `buildPosed`). The generator
re-solves its leg and arm IK, re-hangs the cloth from the moved torso and re-bends the bow string,
so every frame is the same model as the static one, vertex for vertex (the baker refuses a frame
whose vertex count, bone or material differs). One clip serves every race and variant of its
archetype: positions are in a human's plots and the figure scales them to its race.

Rejected alternatives (an architecture review measured them): skinning would need joint weights
threaded through a 4,400-line generator and still lose what the generator re-derives each frame
(a robe hem on the ground, a cape re-hanging, the arrow sliding with the string); the shader's
old per-bone offsets have no joints, so they open seams beyond half a pixel of motion.

**On the GPU** (`shaders.js` `PROP_VS`) the frames of a look (all its clips) are one RGBA16UI
texture: per vertex and frame the position quantised to 16 bits over the frames' box and the
normal octahedron-encoded. An instance carries its clip (`iAnim`: first frame, frames, start
time, rate; a negative rate plays once and holds the last frame) and the clip it cross-fades from
(`iAnimPrev`, over 0.15 s). The vertex shader lerps the two frames around the current time. The
shadow, depth and x-ray passes share the vertex shader, so shadows and ghosts animate too. Colour,
occlusion and the paint pattern stay in rest space (nothing swims).

**At run time** (`map3d.js`, `anim.js` `Animator`):

- Looks are baked a few milliseconds a frame (the idle is published first, then every clip).
  Until a look's idle is baked the figure keeps the procedural idle.
- Every unit idles at its own phase.
- A fight the client sees (`units.js` `flashCombat`: the human's attacks; the client infers
  their outcome) calls `Map3D.combat`. The attacker turns to its target over 0.15 s
  (`attackYaw`: the clip's `strike` direction at the target) and plays its attack, cross-fading
  from its idle and back. A bow looses an arrow (`fx.arrow`) at the release, from the nocked
  arrow's tip, flying to the target (`flight`). The loser keeps idling until the blow lands (or
  the arrow arrives), then turns as it is thrown (`deathYaw`: within 90 degrees of away from the
  killer, the facing whose fall runs most across the screen), plays its death, lies still and
  sinks away.
- `prefers-reduced-motion` or the 3D panel's animation switch off: rest poses, no fights.

## 3. Authoring

### 3.1 Channels

| channel | value | meaning |
|---|---|---|
| `pelvis` | [dx, dy, dz] | the pelvis (the legs re-solve: dy < 0 bends the knees) |
| `fl`, `fr` | [dx, dy, dz] | the left / right ankle (dy lifts the foot) |
| `rHand`, `lHand` | [dx, dy, dz] | added to the hand's IK target (figure space) |
| `rElbow`, `lElbow` | [x, y, z] | added to the elbow's pole (which way it bends) |
| `rRot`, `lRot` | [x, y, z] rad | Euler turn of what the hand holds, about the hand; `turn(from, to)` aims it |
| `lean`, `twist`, `roll` | rad | the torso (lean + forward, twist + toward the figure's left) |
| `headYaw`, `headTilt` | rad | the head (its yaw is not the torso's child) |
| `root`, `rootRot` | [dx, dy, dz], [x, y, z] rad | the whole figure, turned about the clip's `pivot` (falls) |
| `string` | 0..1 | the bowstring at the drawing hand (1) or snapped home (0) |
| `arrow` | 0..1 | the nocked arrow whole (1) or gone (0); it keeps its rest length and slides with the string |

Figure space: y up, one unit is one map plot (a human is ~0.48 tall), the figure faces +z (toward
the camera at yaw 0), its right hand is on -x.

### 3.2 Tracks and timing

A channel's track is the keys that set it; before its first key it is at rest. Between keys the
interpolation is monotone cubic (PCHIP): it never overshoots a key, eases into a turning point (an
extreme pose is a held instant) and keeps moving through a passing key (a breakdown). A key's `i`
sets the segment after it: `"hold"`, `"linear"`, `"in"` (accelerating into the next key: a fall
into the ground), `"out"` (decelerating), or per channel (`{arrow: "hold", rootRot: "in"}`).
`returns: true` ends a clip at rest; `loop: true` ends where it began; `waves` add sines with whole
cycles per loop.

### 3.3 Metadata (checked against the motion)

`kind` (idle / attack / death), `dur`, `fps` (the runtime's bake rate: 60 for fast attacks, 10-15
for idles), `events` (impact, release; hit, ground, rest), `contacts` (planted foot intervals),
`strike` (the attack's direction), `away` and `fall` (where a death is thrown and where its head
ends up), `pivot`.

### 3.4 What the validator taught the clips (and the engine)

- **Arcs, not chords.** A hand keyed from behind the shoulder to in front of the chest travels a
  straight line through the shoulder: the arm folds and the elbow whips. Add breakdown keys so the
  hand goes round.
- **Stay inside the reach.** The IK clamps at 99.5% of a limb's length (the hand stops following,
  then lurches) and, since this work, at 25% (a folded limb's joint swings wildly).
- **Falls accelerate.** PCHIP eases into every turning point, which lands a falling body on a
  cushion. Key the fall `"in"` and the rebound `"out"`.
- **Snaps are instantaneous.** A loosed arrow or a released string must change in one frame
  (`"hold"` segments); a part-grown arrow is a visible defect.
- **Fast motion needs frames.** The GPU lerps between baked frames; a fast swing's chord cuts its
  arc and shrinks the weapon. The validator compares the lerp with a bake at twice the rate.
- **Read from every side.** Fights come from eight directions and the figure turns to them. An
  overhead chop seen from behind is hidden by the body; a diagonal chop across the body reads
  from all eight.
- **Honest metadata.** Declared contacts, fall directions and events are checked against the
  motion: the renderer turns corpses by them.
- **Engine bugs surfaced as "pops".** `mesh.js` `tube()` chose its ring axis with a hard
  threshold, and arm rings took the IK pole as reference; both flipped rings between frames
  (vertex-correspondence pops the topology check cannot see). Limb tubes now take the bend
  plane's normal or the torso's side axis; a test sweeps every pose channel for jumps.

## 4. Judging an animation

The judgment has three layers, each covering what the others cannot:

1. **Numbers on the exact frames** (`r3d-lab/anim/check.mjs`). The validator sees what the GPU
   plays: the baked vertices, the joints and held points of every frame. Metrics are hard (gate)
   or scored (craft, reported). Each limit is in plots, pixels at the gameplay zoom, seconds or
   degrees, with its reason next to it (`LIMITS`).
2. **Mutation testing** (§5): every metric must fail on a degraded clip and pass on the good one.
3. **Blind vision judges** (§6), themselves calibrated on the mutants, for what numbers cannot see
   (does it read as a chop? does the fall have weight?).

The metrics, by family (the full list with limits and reasons is `check.mjs` `LIMITS`):

**Body** (the physics a viewer's eye expects):

| metric | a viewer sees | limit (reason) |
|---|---|---|
| `ground.penetration` | a part sinking into the ground | 0.006 plots below its rest depth (1.2 px close: a visible piece disappears) |
| `ground.hover` | a planted boot above the ground | 0.004 plots |
| `feet.slip`, `feet.drift` | a planted boot skating | 0.0015 a frame, 0.004 a contact (0.8 px close) |
| `rigid.props`, `rigid.arrow`, `rigid.limbs` | a weapon, bow or limb stretching; an arrow part-grown | Kabsch residual 1% of the part's radius; arrow whole or gone |
| `rigid.reach` | a limb at its IK clamp (stall, then lurch) or folded | 0.3-0.985 of its length |
| `joints.*` | a knee bending back, an elbow folded flat, an owl neck, a broken wrist | anatomical maxima; the haft at most 45 degrees below the figure's own rest grip (ulnar deviation), at most 140 |
| `clear.weapon` | the axe passing through the head or chest | 0.008 deeper than at rest; up to 0.015 for 34 ms in all (a blurred pass in a fast swing) |
| `balance` | leaning out over the feet | centre of mass (Winter's segment model) inside the support; a stepping foot's next footprint counts |
| `attack.power`, `attack.reach`, `attack.aim`, `attack.facing` | a tap, a blow behind or beside the target, a sideways swing | 1.5-7 plots/s, 0.1 ahead, 45 degrees; the chest within 60 degrees of the target |
| `death.accelerates`, `death.gravity`, `death.energy` | a floaty fall, a body lowered onto a cushion | from the final descent (a stagger that catches itself is not the fall) to the trunk's core on the ground (not a skirt's hem): peak speed at the ground; 0.4-2.5 g; 0.25-1.6 of free fall's energy |
| `death.bounce`, `death.settles`, `death.grounded`, `death.low` | a corpse bouncing, twitching, sitting up or floating | a 0.008 rebound from the lowest point so far; still within 0.5 s; trunk within 0.012 of the ground; mass at <= 0.45 of standing, the head at <= 0.3 |
| `shot.*` (body) | a string pulled by nothing, an arrow behind the bow, a chest anchor, a canted bow | nock 0.015 from the hand; tip ahead of the grip; anchor at the face; bow arm along the arrow |

**Motion** (timing and continuity):

| metric | a viewer sees | limit (reason) |
|---|---|---|
| `pop.trunk` | the body twitching | cubic residual 0.003 plots (zero for any cubic motion) |
| `pop.spike`, `pop.jitter` | a one-frame pop; a shiver | a 3 px step (closest zoom) 4x its neighbours'; two reversing 2 px steps |
| `playback.lerp` | the weapon shrinking between frames, an apex skipped | the GPU's straight in-between and the true path (a 4x bake) within 1.5 px of each other, both ways (a lerp running along the path at another pace is retimed, not misshapen) |
| `boundary.*`, `idle.seam` | a snap when a clip starts, ends or loops | rest pose at the ends; the seam no rougher than the loop; scored: cut in from any idle frame, the cross-fade adds <= 1.5 px a 60 Hz frame beyond the same cut from rest |
| `attack.windup`, `.strike`, `.contrast`, `.peak`, `.stop`, `.recover`, `.anticipation`, `.duration` | no anticipation, a push instead of a blow, a blow eased into its target, a snappy recovery | a wind-up >= 200 ms with a 67 ms hold; the blow in 67-233 ms at >= 2.5x the wind-up's speed; it arrives at the impact at >= 0.6 of its peak speed (the step into the impact frame) and halves 67 ms after; >= 270 ms of recovery; 0.6-1.8 s in all |
| `shot.aimHold`, `.release`, `.snap`, `.recoil` | a soft or unaimed release | an aim hold of 100 ms; the arrow gone in a frame; the string home in 67 ms; the hand flies back |
| `death.react`, `death.fallTime`, `death.fallDir`, `death.groundEvent` | no reaction to the blow, a slow topple, a corpse lying where the renderer does not expect | the head thrown away within 133 ms; 150-700 ms of fall; within 35 degrees of the declared fall; the declared `ground` event within 67 ms of the contact |
| `idle.breath`, `idle.jitter`, `idle.loop` | panting, trembling, a hurried loop | a 2-6.5 s breath; nothing faster than 1.5 Hz; a 2.5-8 s loop |
| `craft.commit`, `craft.arc`, `craft.rhythm` (scored) | an arm swinging on a frozen torso, a poke, a metronome idle | the chest travels 0.13 H and leads the weapon; the path bulges 12% from its chord; no repeat inside the loop |

**Reading** (what the player sees at the gameplay zoom, 96 px a plot, the figure ~31 px, through
the map camera at the facings the game uses: eight target directions, `attackYaw`, `deathYaw`):

| metric | a viewer sees | limit (reason) |
|---|---|---|
| `idle.alive`, `idle.calm`, `idle.tempo`, `idle.fromRest` | a statue; a swimming figure | head and arms move >= 1 px, the trunk >= 0.5 px; the body <= 2 px, anything <= 4 px of visible motion (across the surface: a hat's brim turning about its own axis does not count); body and head <= 4.5 px a second; scored: within 2 px of rest |
| `read.travel` | a blow too small to see from some side | the striking point travels 1-3.2 figure heights on screen at all eight facings (0.75 from behind, where the body hides a blow landing in front of it) |
| `read.keys` | two key poses that look the same | silhouette overlap (aligned IoU) of wind-up and impact <= 0.6 (0.7 from behind) |
| `read.weapon` | the weapon lost inside the body's outline | >= 20 px^2 and 40% outside the body at the wind-up and impact (the wind-up only from behind), on a canvas holding the whole clip (a cropped spear once read as hidden) |
| `read.lying` | a corpse that reads as standing (a body along the camera's axis keeps a standing outline at 48 degrees) | the body's axis within 40 degrees of horizontal on screen, its height <= 0.45 of standing |
| `shot.axis`, `shot.flight`, `shot.visible` | an arrow that leaves at an angle, too fast to see, too thin to see | within 15 degrees of the nocked arrow; 130-400 ms of flight; >= 10 px^2 |

What numbers cannot catch, and the vision judges and the owner must: appeal (a pose that is
legal and still ugly), the action's identity (a chop that reads as a wave), weight beyond the
fall's numbers, how the painterly filter treats sub-pixel motion, and the composite moments the
validator does not simulate (two units fighting, the camera moving).

## 5. Trusting the judge

A metric is trusted only when it **fails on its defect and passes on a good clip**.
`r3d-lab/anim/mutate.mjs` holds the catalogue of mutations — the ways animation goes wrong:

A mutation must express its defect however the clip was authored. The first catalogue edited
single keys, and the warrior's densely keyed clips diluted five of its mutants to nothing (the
mutation, not the metric, was the weak link). A mutation now edits a clip only with smooth
additive `bumps` and whole-clip changes (waves, metadata, every key scaled or shifted), or edits
the baked frames: retiming the motion (linear timing, an eased impact, a floaty fall, a dropped
loop end), freezing it (no recoil, a lingering arrow), scaling it about its mean (a statue, a
swimming idle), or moving parts (a stretched forearm, a broken wrist, a weapon through the head).

| mutation | the defect | must trip |
|---|---|---|
| `noAnticipation`, `linearTiming`, `slowStrike`, `easedImpact`, `timid`, `mirrorStrike` | no wind-up, robotic timing, a push, a blow eased into its target, a timid swing, a blow away from the target | the attack's phrasing, delivery and reading metrics |
| `fastSwing` | a swing too fast for the frames | `playback.lerp`, `pop.spike` |
| `freezeTorso` | an arm swinging on a frozen body | `craft.commit` |
| `rushed`, `snapBack`, `smallWeapon` | an attack over too soon, no recovery, a weapon too small to read | `attack.duration`, `attack.recover`, `read.weapon` |
| `sink`, `footSlide`, `footLift`, `wrongContacts`, `overreach`, `squat`, `stretchProp`, `stretchLimb`, `kneeBack`, `elbowFold`, `brokenWrist`, `headTurn`, `overLean`, `clipWeapon` | the body's defects | the body metrics |
| `hitch`, `jitter`, `snap`, `loopBreak`, `noReturn`, `offStart`, `shortLoop` | pops, shivers, a jump, a seam, a clip that does not start or end on rest, a hurried loop | the continuity and boundary metrics, `idle.loop` |
| `floatyFall`, `sitUp`, `rise`, `kneel`, `twitch`, `wrongFall`, `noReaction` | a floaty fall, a corpse sitting up or floating off the ground, no fall, a twitch, the wrong side, no reaction | the death metrics |
| `statue`, `swim`, `pant`, `lockstep` | a dead, swimming, panting or metronome idle | the idle metrics |
| `softRelease`, `noAimHold`, `chestAnchor`, `cantedBow`, `noRecoil`, `lingeringArrow`, `overdraw`, `underdraw` | the archer's defects | the shot metrics |

`selfTest` bakes the good clip (it must pass every hard metric) and each applicable mutant (one of
its targets must fail). The test suite runs it for every clip, and fails when a clip has a hard
metric that no mutant targeting it trips: every hard metric must be *seen* to fail on its defect
(`mutate.mjs` `RUNTIME` exempts the two that measure the runtime's projectile, not the clip:
`shot.flight`, `shot.visible`). Requiring that found a blind spot: `death.bounce` measured a rise
against the height at first contact, so a corpse that kept settling after it could float back up
3 cm unseen (it now measures from the lowest point so far).

What the mutants measured about the metrics themselves: `joints.knee` is nearly implied by
`rigid.reach`'s lower bound (a leg folded past 145 degrees is under 30% of its reach); a deep squat
trips both. The cut from the idle (`boundary.transition`) cannot fail once `idle.calm` and
`boundary.start` pass (an idle within 4 px adds at most ~0.7 px a frame through the cross-fade),
so it is scored, not a gate. A pop inside fast motion is masked by it, and `pop.spike` rightly lets
it pass: the `snap` mutant is placed where the arm moves least.

Two more tests guard the engine, since a metric can only measure what the baker reproduces
faithfully:

- the rest identity: a figure built with an empty pose is its static mesh, bit for bit;
- the continuity sweep: every pose channel swept over its range in 120 steps has no step 4x its
  neighbours' (it caught the ring flips of §3.4: re-introducing the old ring rule fails it with a
  34 mm jump).

The panel that designed the validator (four independent designs: biomechanics, perception, craft,
judge calibration; an architecture critic; skeptics trying to kill each metric) kept a metric only
if it measured something visible, had a mutation, and was hard to game. Its rejects are worth
knowing: triangle edge strain (it flags natural knee bending), the IoU of a death's first and last
frames (zero for any fall), correlations between body regions (they did not separate a lockstep
idle), ratio metrics whose denominator can vanish (report n/a rather than a number).

## 6. Vision judges

(Filled in with the results of §7.)

## 7. The proof of concept

### 7.1 The warrior and the archer

Each archetype's three clips were authored by an agent that iterated against the validator and the
review sheets (`anim-warrior.js`, `anim-archer.js`). Both pass every hard metric for the human, and
every body and motion metric for the nine races and four variants of the race sweep (108 runs per
archetype); every mutant of every clip is caught. The first drafts, checked against the final
validator, show what it made the authors fix: the warrior's attack 6 hard failures (the arm at the
IK's clamp, the GPU's lerp cutting the swing, the blade along the forearm, the axe through the
head, the blow eased into its target, the axe hidden by the body), its death 8 (sinking into the
ground, a floaty 0.17 g fall, the trunk hovering, pops), the archer's death 6 (a fall eased into
the ground at 0.15 g, the declared ground event 150 ms early).

What the authors found that was wrong in the engine rather than the clips: tube rings that flipped
between frames (vertex-correspondence pops, §3.4), a folded IK limb whose joint swung wildly, a
round shield's rim built perpendicular to its disk, a dying body that could not rest on the ground
for every race (the `settle` channel), and `settle` flattening a cape before moving the body down
onto it (the cape ended under the ground).

### 7.2 Is the validator right? A blind-authored control

Mutation testing proves each metric fails on its defect (sensitivity); it cannot show that a
metric passes good animation it was not tuned on (specificity). Two archetypes authored *against*
the validator cannot show that either. So a third archetype, the spearman (a thrust, not a chop),
was authored by an agent working from animation principles and its own eyes (review sheets, the
real renderer, its own probes) that was never shown the validator, its limits or the other clips.
The validator then reported 16 hard failures on it, and each was adjudicated
(`/workflows` adjudicate-blind-clip): three animators reviewed the clips blind, listing every
defect they saw without knowing the verdicts; a metric auditor per clip read each metric's code
against the clip and probed the frames; an adjudicator per failure argued both sides and decided.

| verdict | n | failures |
|---|---|---|
| real, visible | 1 | the death's legs slammed down a frame after the back lands (`pop.trunk`) |
| real, subtle | 5 | arms keyed past their reach (`rigid.reach`), the spear through the forearm in the recovery (`joints.wrist`, attack), a landing between baked frames (`playback.lerp`, death), a rebound too short for 30 fps (`pop.spike`), the front foot lifted before the body is back over the feet (`balance`) |
| false positive | 10 | below |

The false positives were measurement errors, each with a mechanism, most of which the warrior's and
archer's clips happened not to trigger:

- `idle.calm` counted a kettle hat's brim sliding round its own axis as 3.3 px of motion (its
  outline moved 0.45 px); its 1.5 px body limit failed a slow weight shift the blind viewers judged
  too still, not busy.
- `idle.fromRest` and `boundary.transition` scored the cross-fade's own lag (any clip whose
  opening moves is retimed by it) as a defect: the cut from rest failed too.
- `playback.lerp` scored the pace along the path within a frame (a hard stop inside one) as shape.
- `attack.peak` took a centred speed across the impact frame, averaging the stop into the arrival.
- `read.weapon` rendered on a 72 px canvas that cropped a spear, and read the crop as occlusion.
- `death.gravity` took a knee-buckle 0.4 s before the fall as its onset, and `death.groundEvent`
  took a skirt's hem touching the ground for the body landing.
- `joints.wrist` had a fixed 45 degree floor where the anatomy is the figure's own rest grip less
  the wrist's deviation.

Each was fixed in its definition (`check.mjs`, with the mechanism in its comment); the only
threshold changes are the idle body limit (1.5 to 2 px) and the wrist floor (relative to the rest
grip). The warrior and the archer still pass and every mutant is still caught. Rechecked, the
spearman fails 5 metrics, all adjudicated real; the sixth real one (`balance`) is no longer
flagged: the static check now counts a stepping foot's next footprint, and what was wrong there
is dynamic (the body still moving forward when the foot lifts), which a static margin cannot see.

What the blind animators saw that no metric flags: a spear twitch in the idle with no body
involvement, the spear pitched toward the camera so the class weapon reads as a stub from the
front, the shaft passing through the hip as the spear is drawn back, the grip that never lets go
of the spear in death. These are staging and craft, the vision judges' and the owner's part.

A caution the experiment taught about its own instruments: the blind sheets numbered their cells
without times, and the viewers, reading them as consecutive frames, judged every motion 2-3 times
too fast (a 1.15 s attack as "25 frames, 0.4 s"). The blind sheets now print each cell's time.

The fixed validator was calibrated on this clip, so it is no longer a held-out test; §7.3 repeats
the experiment on a fourth archetype.

### 7.3 A held-out control

(Filled in when it has run.)

## 8. Cost

Measured in node (the renderer bakes the same way, a few milliseconds a frame):

| look | vertices | frames (idle, attack, death) | CPU to bake | GPU texture |
|---|---|---|---|---|
| human warrior | 3,348 | 48 + 73 + 49 | ~0.7 s | 4.6 MB (2048 x 278, RGBA16UI) |
| dwarf warrior | 3,906 | 48 + 73 + 49 | ~0.6 s | 5.3 MB |
| human archer | 3,078 | 36 + 97 + 52 | ~0.7 s | 4.6 MB |

(Frame counts as of this writing; they follow each clip's `dur` and `fps`.) A frame is one
`figure()` build, 2-4 ms. The renderer bakes within a 5 ms budget per animation frame, so a look's
idle plays about half a second after it appears and its fights a second or two later. The vertex
shader costs two texel fetches per vertex (four during a cross-fade). The validator bakes at the
clip's rate, plus twice that rate for the playback check; `check` takes about a second a clip, the
test suite about a minute and a half.

## 9. Limits and next steps

- **Only fights the human starts are seen** (the client infers them from its own attacks); AI
  fights and other deaths (spells, starvation) need the server to report them.
- **Memory grows with looks** (~5 MB each, all clips): bake attacks and deaths on demand for the
  looks in view and evict the least recently used; store idles as harmonic shapes (the mean pose
  plus sin/cos terms: ~0.15 MB instead of ~1.5 MB, measured within 0.03 px).
- **Baking runs on the main thread**: move it to a module worker (the recipes use no DOM).
- **Hand paths are Cartesian**: a shoulder-relative (yaw, pitch, reach) arm channel, and weapon
  directions in the forearm's frame, would make arcs the default instead of a breakdown's job.
- **The root turn is applied after the figure is built**: passing it into `figure()` would let
  planted feet stay pinned through a fall, cloth hang along gravity, and held items stay above
  the ground (a dying unit cannot yet drop its weapon).
- **More clips**: the other humanoid archetypes reuse the channels as they are; creatures
  (quadrupeds, wings) need their generators to take a pose; walk and work cycles need the foot
  contact metrics already in place.
- **The vision judges are a model**: they agree with the metrics where both apply (§6), but feel
  in motion is still the owner's call in the lab and in a real fight.
