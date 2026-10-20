# Arm R: idle, flinch and death (rest of the set)

This agent reused the strike agent's rig: `parts/*.png`, `rig.json`, and the posing code in `make_r.py`, which is
imported and not modified. The code is in `make_r_rest.py`. Run `python3 arms/R/make_r_rest.py all` to rebuild all
three animations from the parts. A second run gave byte-identical frames. There were no image-model calls, so there
are no raws and no prompts. `work/R/rest/` holds only review crops.

---

## idle

### Method
The sprite is split into three layers using the rig's segmentation: body, sword and shield. The body is bent with
the rigid MLS map from `rig.warp_to_pose`, using the same handles: every idle-0 joint goes to that frame's idle joint,
and the knees and feet stay where they are. The sword and shield move as rigid whole-pixel layers that follow the
poses a frame late. Frame 0 is the sprite (`write_anim` forces it).

### What I did
- Layers (`idle_layers`). The sword and shield are the sprite's own pixels for those labels. The body is all other
  pixels. Under the shield's opaque pixels, the body gets the rig's code-filled sleeve, torso and thigh pixels, so a
  shield that lifts shows no hole. Under the opaque fist, the sword gets the rig's painted grip. Rebuilt in the rest
  pose, the three layers match the sprite exactly (0 differing pixels).
- Body warp (`mls_warp_sharp`). This is `rig.mls_map` with the `warp_to_pose` handles, plus one change. Plain MLS
  moves the face by 0.94 px and the chest by 0.95 px. The bilinear resample then softens frames 1-3 next to the crisp
  frame 0, and that blur pulses as the loop plays. I sharpen the displacement field: the fractional part f of each
  displacement becomes clip((f-0.5)*4+0.5, 0, 1). The head, shoulders and chest then move exactly 1 px and stay
  crisp. The legs move 0 px. The sub-pixel blend is confined to a narrow band at the belly and the near sleeve.
  sharpen=0 gives exactly `rig.mls_warp`. I compared 0, 3 and 6 at 4x and picked 3.
- Lag. The sword hilt moves with the hand, by the r_ha change (0 / -1 / 0 px on frames 1-3). The blade above the guard
  moves with the tip (0 / -2 / -1 px). It is drawn over a copy at the hand's height, so the extra rise does not open a
  gap at the guard. The first try shifted the whole sword with the tip, and green showed between the guard and the
  fist on frames 2-3. The shield moves by the pose's shield change (0 / -1 / -1 px).
- One round, plus two fixes: the guard gap, and making the rest rebuild exact. 0 calls.

### What worked
- Frame 1: the chest, head and shoulders rise 1 px; the sword and shield stay still. Frame 2: the sword tip is up
  2 px and the shield 1 px. Frame 3: the body is down and the sword and shield are still up. The loop wraps from
  frame 3 to frame 0 with a 1 px drop of the sword and shield, and no pop.
- Metrics: nonrigid mean 0.021, drift 0.02, boil 12.1. Feet jitter: bottom_std 0, feet_x_std 0. Peak frames: tunic 1,
  steel 2, wood 2, hair 2 (overlap index 0.43).
- Gate flags: none raised. Checked by eye on the strip3x and map1x: no flip, the sword in the screen-left hand and the
  shield on the near arm in every frame, and frame 0 is the sprite.

### What failed
- The breath is 1 px, as the poses specify. At 1x on the map it is a subtle bob of the head and shoulders, with the
  sword and shield following. The belly band where the displacement is fractional is about 2 rows deep and slightly
  softer on frames 1-2. It is visible at 4x and not at 1x.
- The near sleeve sits next to the shield handles, which hold still on frame 1. So it lifts only about 0.4 px (MLS),
  and the shoulder line on that side rises less than on the sword side. I saw no seam there at 4x, but the lift is
  uneven.

### Spend and time
0 calls, $0.00. Logged at 486 s (8.1 min); this includes reading the brief and the rig.

### If I had another hour
Add a 1 px chest widening on the in-breath, using a small MLS handle pair on the tunic sides, so the breath reads as
volume and not only as a lift.

---

## flinch

### Method
The rig's cut-out parts are posed to the four flinch poses with `make_r.pose_frame`. The torso uses rigid MLS from
the rest joints, and the head turns about the neck by the pose's head_tilt. The limbs are FK chains that rotate and
stretch about their joints. The eyes are squeezed shut with a code-drawn edit, in sprite skin colours, on frames 0-1.

### What I did
- `shut_eyes` repaints the near eye (x 81-84, y 55-58) and the far eye (x 88-89, y 54-57) on the head layer. The
  whites and pupils become lit, mid or shade skin, and a dark lid line with a shaded crease goes across each eye. I
  checked it at 12x.
- Springs (`springs`) are the same one-frame springs as the strike, started from the rest pose: hair k 0.55 (clamped
  to 3 px), shield k 0.6 (clamped to 2 px). On the impact frame the hair trails 1.8 px left as the head snaps right.
  The shield lag is zero on frame 0, so the impact hits the pose exactly, because that frame is also death frame 0.
  On frames 1-2 the shield stays out and high for a frame ("shield still out"). Frame 3 keeps 30% of the lag, as the
  strike's settle frame does.
- Squash about the pelvis: 1.02 vertical on the impact frame (jolt) and 0.97 on the recoil frame (knees bent).
- `decrumb`. Once the far arm leaves the torso, the sprite's soft anti-aliasing between the two parts stays behind
  on the torso layer as faint lilac specks beside the belt on frames 0-1. A cleanup pass on each frame removes
  detached components under 8 px, and pixels under alpha 90 with no pixel of alpha 150 or more next to them. I
  checked at 8x that the specks are gone and the outlines kept.
- One build, one cleanup pass. 0 calls.

### What worked
- The beat reads at 1x. On frame 0 the head snaps back toward the upper-right with the chin up and eyes shut, the
  torso leans back, the sword arm is thrown out with the blade tipped toward horizontal, and the shield is flung out
  up and right. Frame 1 leans further back with the knees bent and the sword dipped. Frames 2 and 3 return to the
  ready pose.
- The character is consistent because every pixel is the sprite's: drift 0.44. Nonrigid 0.17 / 0.15 / 0.22 against
  frame 0. Ground contact: bottoms all 125, bottom_std 0.
- Gate flags: none raised. Checked by eye: no flip, the sword in the screen-left hand and the shield on the near arm in
  all 4 frames, and both props present.

### What failed
- The front boot tips up on frame 1 (feet_x_std 1.3). Shin and boot are one part, so when the knee moves, the boot
  rotates about the knee and rolls onto its heel. The toe leaves the bottom rows by 2 px. The foot does not slide.
- The revealed side of the near shorts, where the shield was, shows the thigh's outline as a dark vertical line inside
  the blue on frames 0-2. There is also a 2-3 px dark olive smudge beside the near knee: soft shield-edge pixels that
  went to the shin part. Both are visible at 3x and barely at 1x.
- The torso and face art do not change. "Torso jolts back" is a lean and a squash, not a new drawing. The closed eyes
  are the only new art.

### Spend and time
0 calls, $0.00. Logged at 167 s (2.8 min).

### If I had another hour
- Split the boot from the shin at the ankle so the foot can stay flat while the knee bends.
- Clean the shield-edge pixels out of the near thigh and shin layers in the rig.

---

## death

### Method
The cut-out parts are posed with one whole-body affine per frame, plus FK limbs. The affine is a least-squares fit
from the seven rest torso joints to the pose's torso joints. Standing frames blend it 80% toward its nearest rotation.
The lying frames use it in full, which gives the rotation plus the squash and shear that lays a body on the iso
ground. The head hangs on the neck with the same squash and shear, turned to the pose's head_tilt as an absolute
angle. Each limb is aimed in the body's own unsheared space and then mapped by the same affine, so the limbs splay
with the body and lie on the same ground plane. Frame 0 is flinch frame 0, pixel for pixel (checked).

### What I did
- `body_frame` (new). `fit_affine` fits the torso affine and `blend_rigid` blends it toward a rotation through a
  polar decomposition. `limb_matrix` computes G·(rotation+stretch toward G⁻¹·bone). `place_sword` uses the rig's
  code-drawn blade for turns over 12°, and the raster sword otherwise. The shield turns from upright to flat: a half
  turn on frame 4, then a quarter clockwise turn scaled to the pose's 12×6 ellipse on frames 5-7.
- Fitted body rotations: frame 1 -8°, frame 4 -34°, frames 5-7 -47° (plus head_tilt 64 on the head). The corpse's
  axis runs from the feet at the lower-left (about 56,124) to the head at the upper-right (about 112,98): 96 right :
  45 up, on the iso ground axis.
- First version: limbs were only rotated, not sheared with the body. On frames 5-7 the legs were a heap of boots at
  different angles, and the far arm stood up from the chest like a raised fist. Aiming the limbs in body space and
  then applying the body affine fixed both. The legs now lie along the axis with the boot toes up, and the far arm
  lies out to the upper-left.
- Second fix: the head first ignored head_tilt and turned only halfway toward the head joint. It now uses head_tilt as
  the absolute angle.
- Standing frames 1-3: the poses give a shoulder width of 16 px (rest 26). The full fit made the torso 0.66× wide, too
  thin for the head. I compared 60% and 85% rigid and chose 80%.
- Sword: in the hand on frames 0-3 (raised on 1, lowered on 2, tip on the ground on 3). It leaves the hand on frame 4:
  still dropping, with the hilt 4 px above its rest spot (62,115 → 31,122). On frames 5-7 it lies on the ground at
  the lower-left, at the pose's (60,119)→(30,124).
- Overlapping action: the same hair and shield springs run through the fall, from the rest pose. The hair lag is
  converted into the head layer's own space so it trails correctly on a rotated head. The lag fades on the lying frames
  (80% / 60% / 0 on frames 5 / 6 / 7), so the corpse is settled. The flat shield lands a frame late on frame 5 and
  rocks on frame 6, together with the pose's 2 px head-and-shoulder bounce.
- Eyes shut on all 8 frames (`shut_eyes`). `decrumb` on every frame. Two rounds, 0 calls.

### What worked
- At 1x on the map strip the whole beat reads: the impact, the stagger back with the sword flung up and the near foot
  stepping back, the knees buckling with the head drooping, the slump on the knees with the sword tip on the ground,
  the fall back as the sword drops, the corpse on its back along the tile's ground axis (head upper-right, feet
  lower-left), the bounce, and the settle. In the vignette the mirrored defender dies with his feet toward the attacker.
- The ground-plane affine is what makes the corpse read as lying on the tile. Shoulders and hips run along the other
  iso axis, and the head, torso and legs are flattened together. The result does not look like a side-view figure
  tipped over.
- Drift stays low for a full fall (0.54), because every pixel is the sprite's. The flat shield is the sprite's own
  shield turned and scaled, and it reads as a round shield lying flat.
- Gate flags: none raised. Checked by eye: no flip, the sword in the screen-left hand on frames 0-3 and on the ground
  on 4-7 (allowed), the shield on the near arm on 0-4 and flat beside him on 5-7. No prop missing. All frames stay
  inside x 29-132.

### What failed
- **Frame 3 (kneel)** is the weakest standing frame. The shins point backward behind the knees, so the shin+boot parts
  rotate about 130° and sit upside down, mostly hidden behind the thigh and shield. The lower body reads as crouched
  behind the shield more than clearly on both knees.
- **Frames 2-3 do not slump forward.** The poses keep the chest straight above the pelvis, and the rig cannot draw a
  bent-over torso from the upright sprite, so "slumps forward" is only the head drooping (head_tilt -12 / -16).
- **Frame 4:** the far arm is up with an empty fist and the sword already near the ground. The release has no
  in-between: the hand opens between frames 3 and 4, which is 80 ms.
- **The corpse is sheared sprite art.** The face is the upright 3/4 face, rotated and flattened. It reads at 1x but
  at 3x looks like a skewed decal. The far sleeve and fist sit above the chest on screen. That is correct for an arm
  lying on the far side, but some viewers may read it as a raised arm. The legs are cleaner after the fix, but the
  boots and knees still overlap in a busy cluster at the lower-left.
- Overlap index 0.0. Every tracked part peaks on the last frame because a fall is monotonic. The springs add trailing
  hair and a late shield landing, but the metric cannot see them.

### Spend and time
0 calls, $0.00. Logged at 313 s (5.2 min).

### If I had another hour
- Spend part of the untouched $0.60 cap on one masked `gen.edit` of a lying head and torso: a face seen from above
  with eyes closed. Then palette-snap it to the sprite and use it for frames 5-7 in place of the sheared upright face.
- Split each boot from its shin so the kneel can fold the shins back with the soles visible instead of upside-down
  boots.
- Add an in-between for the sword release on frame 4: the fist opening, with the sword at about 45° in the air.

---

## Spend and time (all three)

0 image-model calls; $0.00 of the $0.60 anim/R cap is spent (spend.jsonl has no anim/R rows). Logged times: idle
486 s, flinch 167 s, death 313 s, about 17 minutes in all, inside the 60-minute box for each. After all four R
animations existed (the strike manifest is the strike agent's), I ran `preview.py --only sequence` and
`--only vignette`. They wrote `arms/R/sequence_{3x,1x}.{gif,mp4}` (7.99 s) and `arms/R/vignette.{gif,mp4}` (6.74 s).
