# Beat sheet (shared by every arm)

Stiffness is mostly timing and posing, not rendering, so every arm fills these same beats.
"Leads" is the body part that moves first on that frame; the others follow a frame later
(overlapping action). Joint positions for every frame are in `poses.json` (made by
`lib/poses_src.py`); the text here is the intent. `lib/beats.py` holds the same table as
data (`BEATS[anim][i]["ms"]`, `["text"]`, `["events"]`) so prompts and manifests cannot drift
from this file.

## idle — 4 frames, loop, 250 ms each (1.0 s breath)

| # | ms | Pose | Leads |
|---|---|---|---|
| 0 | 250 | the static sprite, unchanged: sword raised in the right hand, shield on the left arm | – |
| 1 | 250 | breathing in: chest and head rise 1 px, shoulders lift; the sword tip has not moved yet | chest |
| 2 | 250 | top of the breath: chest and head still up; the sword tip now rises 2 px and the shield lifts 1 px (they lag the chest) | sword, shield |
| 3 | 250 | breathing out: chest and head back down; the sword tip still 1 px high, settling | chest |

## strike — 8 frames, ~870 ms, `hit` on frame 4

| # | ms | Pose | Leads |
|---|---|---|---|
| 0 | 100 | ready: the static sprite, unchanged | – |
| 1 | 120 | anticipation: weight shifts back onto the rear (right-hand side of the screen) leg, torso leans back, the sword arm lifts the sword up over the head, tip pointing up and back | hips |
| 2 | 160 | wind-up extreme (hold): body coiled back and slightly crouched, the sword cocked behind the head with the blade hanging down behind the shoulders, shield tucked in | sword |
| 3 | 50 | swing (smear): the front foot steps toward the lower-left, the torso whips forward, the arm sweeps over the head and the blade is level, pointing to the lower-left | front foot, torso |
| 4 | 120 | **contact (`hit`)**: arm fully extended toward the lower-left, the blade slashing down-left, front knee bent, body low and leaning in, shield arm swung back for balance | sword |
| 5 | 120 | follow-through: the blade carries on down until its tip nearly touches the ground at the lower-left, torso twisted, hair trailing back | sword |
| 6 | 100 | recover: body rises, front foot pulls back halfway, the sword lifts back toward the ready angle | hips |
| 7 | 100 | settle: almost the ready pose, sword 1 px low (small overshoot), then idle resumes | – |

## flinch — 4 frames, ~440 ms

| # | ms | Pose | Leads |
|---|---|---|---|
| 0 | 80 | impact: hit from the lower-left; head snaps back toward the upper-right, eyes squeezed shut, torso jolts back, both arms jerk outward (shield flung out to the right, sword arm thrown up-out) | head |
| 1 | 120 | recoil extreme: leaning back further, knees bent, sword dipping, shield still out | hips |
| 2 | 120 | recovering: halfway back to the ready pose | chest |
| 3 | 120 | settle: nearly the ready pose, then idle resumes | – |

## death — 8 frames, ~1.18 s, last frame held

| # | ms | Pose | Leads |
|---|---|---|---|
| 0 | 80 | the flinch impact pose (same pose as flinch frame 0) | head |
| 1 | 120 | stagger back: a step back with the near (left) foot, body thrown back, sword arm flung up, shield drooping | torso |
| 2 | 140 | knees buckle: hips drop, knees fold forward, torso slumps forward, head droops, sword lowering | hips |
| 3 | 200 | kneeling (hold): on both knees, slumped, sword tip resting on the ground, shield hanging | – |
| 4 | 80 | falling back: torso tips back toward the upper-right, the sword slips out of the hand, legs slide forward | torso |
| 5 | 80 | hits the ground: lying on his back along the ground, feet toward the lower-left, head toward the upper-right; sword lying on the ground at the lower-left; shield flat beside him | – |
| 6 | 80 | bounce: head and shoulders lift 2 px off the ground, shield rocks | – |
| 7 | 400 | corpse: settled flat, eyes closed (held indefinitely) | – |

The corpse lies along the isometric ground axis (screen direction 96 right : 48 up), so it
reads as lying *on the tile*, not as a side-view figure tipped over.
