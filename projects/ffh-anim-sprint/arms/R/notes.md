# Arm R: cut-out rig from the sprite's own pixels

## Method

I cut the static sprite into eleven part layers with polygon masks and painted the hidden pixels in code. I then
posed the parts for each strike frame to the poses.json joints. The limbs are rotated, and stretched a little along
the bone, about their pivots. The torso is bent with the rigid MLS warp. Parts are composited in each frame's depth
order. The test is whether a better puppet escapes the "stiff / puppet-like" look of the whole-sprite tween (A0).
There are no image-model calls: every pixel comes from the sprite or from code.

## What I did

- **Segmentation** (`make_r.py` `POLYS`, `segment`). The eleven parts are: head+hair, torso (tunic, belt, top of
  the shorts), far arm as sleeve / forearm+fist / sword, near sleeve, shield (it hides the near forearm), and each leg
  as thigh (shorts leg + knee) / shin+boot. Pixels go to parts in a fixed priority order. Tunic-blue pixels caught by
  the head or sleeve polygons go back to the torso (collar, straps). Specks smaller than 12 px go to the part they
  touch, so no crumbs float when parts separate.
- **Hidden-pixel fill** (`fill_hidden`). Each part has a target silhouette. A pixel is filled only if a part drawn
  *in front of it* hides it in the rest pose. The fill clones the nearest interior pixel of the part, then redraws a
  1 px outline on the new edge. Filled areas: the tunic under the hair, the near sleeve and the shield edge; the
  shorts legs up under the belt; the shins up under the knees; the far sleeve under the tunic; the near sleeve under
  the shield; and the sword grip under the fist (painted). Because the fill is limited this way, the rest pose rebuilt
  from the parts matches the sprite to a mean alpha difference of 0.17 (out of 255). An earlier version filled whole
  silhouettes, and its outlines showed on top of neighbouring parts (mean 0.86).
- **Posing** (`pose_frame`).
  - Torso: rigid MLS from the 7 rest joints (pelvis, hips, chest, neck, shoulders) to the frame joints, plus squash
    about the pelvis on frames 2 (0.97 v), 3 (1.03 v stretch), 4 (contact: 1.03 h / 0.95 v) and 5.
  - Head: turned about the neck by `head_tilt`, hung on the warped neck.
  - Far arm: an FK chain. The sleeve rotates about the warped shoulder, the forearm+fist about the actual elbow, the
    sword about the actual hand. Each takes its bone direction from poses.json and a clamped stretch for
    foreshortening (forearm up to 1.35, sword 0.88-1.03).
  - Legs: thigh from the warped hip, shin from the actual knee, aimed at the planted foot.
  - Near arm: the sleeve follows the l_sh→l_el bone. The shield follows the pose's shield centre and turns 0.35× the
    forearm's angle change.
- **Sampling.** Parts are transformed with a 4x nearest-supersampled affine, box-filtered in premultiplied alpha
  (area sampling). This is crisper than 1x bicubic on these soft-edged sprites, and exact for the identity. Turning
  the sprite's stair-stepped blade outline by 60-180° gave a saw-tooth edge with every sampler I tried (nearest-SS,
  bilinear-SS, bicubic). For sword turns over 12° (frames 1-5), the blade is therefore **redrawn in code**: a straight
  tapered blade whose cross-section colours are averaged from the sprite's own blade. The sprite's raster crossguard,
  grip and pommel are rotated on top. Frames 6-7 use the raster sword, so the return to idle does not pop.
- **Animation principles on top of the joints.** Hair and shield chase the head and shield positions with a
  one-frame spring (k 0.55 / 0.6, clamped to 3 / 2 px). The hair is displaced by an MLS warp of the head layer, with
  the face pinned. So the hair trails right on the whip forward (frames 3-5) and catches up on the recovery; lag below
  1 px is skipped so it does not blur the head. The fist is always drawn over the sword grip. Frame 1 puts the
  forearm and fist in front of the hair. With the poses.json order (arm behind head), the hand vanished and the hilt
  appeared to grow out of the hair. Frame 2 keeps the arm behind the head, because in front it crossed the face.
- Rounds: one build, then about six polish passes (fill limits, crumb removal, code-drawn blade, fist/grip order,
  frame-1 order, knee-seam fill). 0 image calls.
- Extra deliverable: `make_sequence_r.py` writes `sequence_strike_{3x,1x}.{gif,mp4}`: ready hold → strike → ready,
  twice, on a grass tile with banner and shadow, at the manifest timings (3.84 s). It uses preview.py's scene, GIF and
  MP4 helpers. Once idle/flinch/death exist, it also writes preview.py's standard full `sequence_{3x,1x}` and a
  per-animation sequence for each.

## What worked

- The character stays the same in every frame, because all frames reuse the sprite's own pixels: drift 0.41 ΔE
  (A0 0.47), and no redraw or boil of the face or outfit.
- Moving parts separately makes the motion clearly non-rigid: nonrigid 0.33-0.36 on frames 1-5 (A0 strike mean
  0.06). It also brings overlapping action: the peak frames are steel 2 / tunic 5 / hair 4 / wood 2, overlap index
  1.30 (A0 0.0). At 1x on the map strip, the strike reads well: wind-up behind the head, a level swing, a low lunge
  with a bent front knee on contact, and a follow-through with the tip near the ground.
- The code-drawn blade turns cleanly at any angle and keeps the sprite's shading, so frames 0 and 1 do not jar.
- Metric gates: none raised. By eye: no facing flip, the sword stays in the far screen-left hand and the shield on the
  near arm in all 8 frames, the sword is present in every frame (in frame 2 the blade is partly hidden behind the head
  but visible at the right), and frame 0 is the sprite (`write_anim` forces it).
- The rig can be reused for the next round: `parts/*.png`, `rig.json` (polygons, fill silhouettes, pivots, bones,
  stretch limits, depth groups) and `pose_frame(parts, pose)` take any poses.json pose.

## What failed

- **No new drawing where the pose needs it.** The face, expression and torso art never change, so frame 5's "torso
  twisted" is only a lean and a squash. The cream sleeve rotated about 180° in frames 1-3 reads as a lump beside the
  face rather than a raised upper arm. The far forearm in frames 1, 3 and 4 is the sprite's short foreshortened
  forearm stretched up to 1.35×, so it is thin.
- **Frame 2** (wind-up hold): the hand and most of the forearm are hidden behind the head. The pose reads mostly from
  the blade showing to the right of the head and the sleeve to the left. It reads at 1x but is the weakest frame.
- **Knees (frames 3-6):** a dark crease line remains where the thigh and shin meet on the far leg. The gap itself is
  filled.
- **Joint mismatch with the poses.** poses.json limb lengths vary by up to 30% between frames (foreshortening). With
  the stretch clamps (sleeve ≤1.25, forearm ≤1.35, both at the limit in frames 1-5), the far hand lands 1.1 / 1.6 /
  2.2 px from the pose's r_ha in frames 1 / 2 / 5 (≤0.5 px elsewhere), and the sword tip lands
  within 2 px (tip trail: 95,16 / 119,66 / 22,50 / 20,106 / 34,121 against the pose's 96,18 / 118,68 / 22,50 /
  20,106 / 34,121).
- **Stiffness:** better than A0, but the torso and head are still rigid cut-outs with uniform shading. A viewer will
  still see a puppet in the body, but not in the limbs or sword.

## Spend and time

0 image-model calls, $0.00 of the $0.60 cap. I did not need inpainting, because the code fills cover everything that
gets revealed. The strike took 13.8 minutes wall-clock, including segmentation, the rig, polish and the sequence
files (`timings.jsonl`).

## If I had another hour

- Paint a proper raised-arm sleeve and a longer forearm in code (capsule with sprite palette and outline) for frames
  1-3, instead of stretching the sprite's foreshortened pieces.
- Spend one or two masked `gen.edit` calls ($0.60 cap) on a turned torso for frame 5 and on a shoulder/arm-pit fill
  for the raised frames, then palette-snap the result to the sprite.
- Use a two-bone IK for the arm, so the hand lands exactly on r_ha with lengths split between sleeve and forearm,
  and a per-frame elbow crease.
- Reuse the rig for idle (MLS breath plus a 1 px lag on sword and shield) and flinch, where the poses are small and
  cut-out parts are most convincing. Death will need lying-down art the rig cannot make from a standing sprite beyond
  rotating the whole assembly.
