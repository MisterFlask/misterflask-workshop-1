"""Near pose set for the swordsman as he really is: facing LOWER-RIGHT, sword in the near (screen-left) hand.

Every action points the way he looks: the strike goes to the lower-right (neighbouring tile at (+96, +48)),
the flinch and the death recoil toward the upper-left, and the corpse lies along the (96, 48) ground axis with
the head at the upper-left and the feet at the lower-right.

  python lib/poses_near_src.py     # writes poses_near.json (recipe.py picks it for lower-right units and, mirrored,
                                   # for lower-left units holding the weapon in the near, screen-right hand)

Same schema, frame counts, timings and beats as lib/poses_src.py (the "far" set). Joints are 1x canvas
coordinates (160x128, anchor (80,126)). r_* = right side = sword arm/leg (NEAR side, screen-left at rest);
l_* = left side = shield arm/leg (FAR side, drawn darker). tip = sword tip (blade from r_ha to tip) unless
"sword" gives a dropped sword as [hilt, tip]. shield = [cx, cy, rx, ry] on the l_ hand. head_tilt in degrees,
+ = tipped back, away from the facing (toward the upper-left). Every frame has facing "lower-right", far "l"
and an explicit back-to-front "order".

Made after the sprint for the strike-direction rule (ANIMATING_UNITS.md, rule 1): three agents each designed a set,
three blind judges scored the rendered guides (python lib/guides.py --poses poses_near.json --out sheet.png), and the
winner (best in strike, flinch and death) was refined from the judges' notes: weight over the rear leg in the wind-up,
arm lengths kept, a deeper lunge, a real follow-through, the far forearm drawn over the swung-back shield, limb lengths
repaired in the death, and the dropped sword lying along the ground axis.

Readability rules this set keeps (1x game scale, ~86 px figure, and an image model tracing the guide):
  - the key frames (strike 2 wind-up, strike 4 contact, flinch 0, death 1, death 5) have different outlines;
  - the blade never crosses the head or the torso at contact, and the shield never covers the blade;
  - the far shield arm moves out of the swing's path (swung back behind the shoulders, out at the upper-left)
    so the strike reads as one diagonal from shield to blade tip, along the (96, 48) attack line;
  - limbs that are drawn over one another differ in colour (blade over leg, sleeve over tunic), never leg over leg.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEAD_R = 13

# Back-to-front for a lower-right facer: far (l_) leg and arm behind the torso, the near (r_) leg, the head,
# then the near sword arm in front, its fist over the hilt. Shield in front of the torso at rest, as on the sprite.
NEAR_ORDER = ["l_leg", "l_arm", "torso", "shield", "r_leg", "head", "sword", "r_arm"]

# The sprite's real joints (identical to lib/poses_src.py REST).
REST = dict(
    head=(82, 53), neck=(82, 66), chest=(81, 74), pelvis=(82, 90),
    r_sh=(69, 69), r_el=(66, 78), r_ha=(61, 84), tip=(47, 51),
    l_sh=(95, 69), l_el=(97, 79), l_ha=(102, 87), shield=(104, 88, 9, 15),
    r_hip=(77, 94), r_kn=(73, 106), r_ft=(67, 121),
    l_hip=(88, 94), l_kn=(92, 107), l_ft=(98, 123),
    head_tilt=0, eyes="open",
)


def P(**kw):
    p = dict(REST)
    p.update(kw)
    return p


def up(p, keys, dy):
    for k in keys:
        x, y = p[k][:2]
        p[k] = (x, y + dy) + tuple(p[k][2:])
    return p


# ---------------------------------------------------------------- idle: the far set's breathing frames, unchanged
IDLE = [
    P(),
    up(P(), ["head", "neck", "chest", "r_sh", "l_sh"], -1),
    up(up(P(), ["head", "neck", "chest", "r_sh", "l_sh"], -1), ["tip"], -2) | {"shield": (104, 87, 9, 15), "l_ha": (102, 86), "r_ha": (61, 83)},
    up(P(), ["tip"], -1) | {"shield": (104, 87, 9, 15)},
]

# ---------------------------------------------------------------- strike toward the lower-right, near-hand swing
WIND = ["sword", "l_leg", "l_arm", "torso", "shield", "r_leg", "head", "r_arm"]   # blade behind everything
# shield swung back behind him: the far forearm is drawn over the shield (its inside faces the viewer)
LUNGE = ["shield", "l_arm", "l_leg", "torso", "r_leg", "head", "sword", "r_arm"]
STRIKE = [
    # 0 ready: the static sprite
    P(),
    # 1 anticipation: weight back over the rear (near, screen-left) leg, torso leaning back to the upper-left,
    #   the sword hand lifted beside the back of the head, the blade tipped up and back (upper-left)
    P(head=(74, 55), neck=(75, 68), chest=(75, 76), pelvis=(77, 92),
      r_sh=(64, 69), r_el=(59, 61), r_ha=(62, 53), tip=(41, 24),
      l_sh=(89, 71), l_el=(91, 81), l_ha=(95, 88), shield=(97, 89, 9, 15),
      r_hip=(72, 96), r_kn=(76, 108), r_ft=(67, 121),
      l_hip=(83, 96), l_kn=(90, 108), l_ft=(98, 123),
      head_tilt=6, order=WIND),
    # 2 wind-up extreme (hold): sitting back and down onto the rear leg, coiled; the fist cocked behind the head,
    #   the blade hanging down behind the shoulders toward the lower-left; shield tucked in across the belly
    P(head=(72, 61), neck=(73, 74), chest=(73, 82), pelvis=(75, 99),
      r_sh=(62, 76), r_el=(59, 67), r_ha=(59, 58), tip=(31, 81),
      l_sh=(87, 77), l_el=(92, 86), l_ha=(89, 95), shield=(90, 93, 8, 15),
      r_hip=(70, 103), r_kn=(80, 111), r_ft=(67, 121),
      l_hip=(81, 103), l_kn=(94, 107), l_ft=(98, 123),
      head_tilt=10, order=WIND),
    # 3 swing (smear): the front (far) foot lands toward the lower-right, the torso whips round, the fist comes over
    #   at shoulder height and the blade is level, pointing right; the shield is swinging back behind the shoulders
    P(head=(87, 60), neck=(87, 73), chest=(87, 80), pelvis=(84, 96),
      r_sh=(81, 76), r_el=(91, 76), r_ha=(100, 75), tip=(135, 77),
      l_sh=(95, 74), l_el=(86, 70), l_ha=(78, 73), shield=(76, 74, 6, 13),
      r_hip=(79, 100), r_kn=(76, 112), r_ft=(63, 122),
      l_hip=(90, 100), l_kn=(101, 108), l_ft=(106, 124),
      head_tilt=-4, order=LUNGE),
    # 4 contact (hit): a deep lunge toward the lower-right, the sword arm out in front of the body, the blade
    #   slashing down-right along the attack line; front knee bent over the foot, rear knee dropped; the far arm
    #   swung straight back, the shield out at the upper-left clear of the near shoulder
    P(head=(98, 66), neck=(95, 78), chest=(92, 86), pelvis=(87, 101),
      r_sh=(86, 82), r_el=(94, 87), r_ha=(101, 91), tip=(132, 109),
      l_sh=(95, 76), l_el=(86, 73), l_ha=(77, 70), shield=(74, 69, 6, 13),
      r_hip=(82, 105), r_kn=(77, 117), r_ft=(62, 122),
      l_hip=(93, 105), l_kn=(107, 108), l_ft=(110, 124),
      head_tilt=-8, order=LUNGE),
    # 5 follow-through: the hand carries on down and out, the blade turns to 45 degrees with its tip at the ground
    #   at the lower-right, outside the front knee; torso twisted further, leaning on and lower; hair trails back
    P(head=(102, 70), neck=(99, 82), chest=(96, 90), pelvis=(88, 103),
      r_sh=(95, 90), r_el=(102, 96), r_ha=(110, 100), tip=(135, 125),
      l_sh=(97, 80), l_el=(88, 76), l_ha=(79, 72), shield=(76, 71, 6, 13),
      r_hip=(83, 107), r_kn=(78, 118), r_ft=(62, 122),
      l_hip=(94, 107), l_kn=(108, 107), l_ft=(110, 124),
      head_tilt=-6, order=LUNGE),
    # 6 recover: body rises, front foot halfway back, sword lifted back up on the near side toward the ready angle;
    #   the shield comes back round in front, tucked across the belly (in-between of f5 and the rest pose)
    P(head=(84, 55), neck=(84, 68), chest=(83, 76), pelvis=(84, 92),
      r_sh=(71, 71), r_el=(68, 80), r_ha=(64, 87), tip=(58, 52),
      l_sh=(96, 71), l_el=(98, 80), l_ha=(90, 84), shield=(90, 82, 8, 14),
      r_hip=(79, 96), r_kn=(77, 108), r_ft=(67, 121),
      l_hip=(89, 96), l_kn=(96, 108), l_ft=(104, 124),
      head_tilt=-2, order=NEAR_ORDER),
    # 7 settle: the ready pose, body 1 px down, sword 1 px low (warped from the sprite)
    up(P(r_ha=(61, 85), tip=(47, 52)), ["head", "neck", "chest", "pelvis", "r_sh", "l_sh", "r_hip", "l_hip"], 1),
]

# ---------------------------------------------------------------- flinch: hit from the lower-right
FLINCH = [
    # 0 impact: head snaps back to the upper-left, eyes shut, chest and pelvis jolt back; the sword arm thrown
    #   up-out to the left (blade steeply up), the shield arm flung straight out to the right
    P(head=(73, 52), neck=(75, 65), chest=(76, 73), pelvis=(79, 90),
      r_sh=(64, 68), r_el=(56, 72), r_ha=(50, 67), tip=(27, 40),
      l_sh=(90, 68), l_el=(99, 72), l_ha=(108, 76), shield=(111, 77, 9, 15),
      r_hip=(74, 94), r_kn=(74, 107), r_ft=(67, 121),
      l_hip=(85, 94), l_kn=(91, 107), l_ft=(98, 123),
      head_tilt=14, eyes="shut", order=NEAR_ORDER),
    # 1 recoil extreme: leaning further back and down, knees bent forward, the sword dipping, the shield still out
    P(head=(70, 59), neck=(72, 72), chest=(73, 80), pelvis=(78, 96),
      r_sh=(61, 75), r_el=(56, 83), r_ha=(50, 89), tip=(25, 64),
      l_sh=(87, 75), l_el=(96, 79), l_ha=(105, 82), shield=(108, 83, 9, 15),
      r_hip=(73, 100), r_kn=(80, 111), r_ft=(67, 121),
      l_hip=(84, 100), l_kn=(96, 106), l_ft=(98, 123),
      head_tilt=10, eyes="shut", order=NEAR_ORDER),
    # 2 recovering: about two thirds of the way back
    P(head=(79, 54), neck=(79, 67), chest=(79, 75), pelvis=(81, 92),
      r_sh=(67, 70), r_el=(63, 79), r_ha=(57, 84), tip=(37, 54),
      l_sh=(93, 70), l_el=(98, 79), l_ha=(104, 85), shield=(106, 86, 9, 15),
      r_hip=(76, 96), r_kn=(75, 108), r_ft=(67, 121),
      l_hip=(87, 96), l_kn=(93, 108), l_ft=(98, 123),
      head_tilt=5, order=NEAR_ORDER),
    # 3 settle: nearly the ready pose (warped from the sprite)
    P(head=(81, 53), neck=(81, 66), chest=(80, 74), pelvis=(81, 91),
      r_sh=(68, 69), r_el=(65, 78), r_ha=(60, 84), tip=(46, 52),
      l_sh=(94, 69), l_el=(96, 79), l_ha=(101, 87), shield=(103, 88, 9, 15), head_tilt=2, order=NEAR_ORDER),
]

# ---------------------------------------------------------------- death: back toward the upper-left, corpse on the axis
# the dropped sword lies on the 2:1 ground axis at the lower-right, beyond and apart from the feet
DROPPED = ((112, 108), (140, 122))
LYING = ["l_arm", "shield", "l_leg", "torso", "r_leg", "head", "r_arm", "sword"]
DEATH = [
    # 0 the flinch impact
    dict(FLINCH[0]),
    # 1 stagger back: near (rear) foot steps back, body thrown back to the upper-left, sword arm flung up and
    #   out to the left, shield drooping
    P(head=(63, 61), neck=(67, 73), chest=(70, 80), pelvis=(75, 95),
      r_sh=(59, 76), r_el=(51, 72), r_ha=(45, 65), tip=(39, 30),
      l_sh=(82, 77), l_el=(87, 85), l_ha=(91, 92), shield=(93, 92, 8, 14),
      r_hip=(70, 99), r_kn=(70, 111), r_ft=(60, 121),
      l_hip=(81, 99), l_kn=(90, 110), l_ft=(99, 122),
      head_tilt=18, eyes="shut", order=["l_leg", "l_arm", "shield", "torso", "r_leg", "head", "sword", "r_arm"]),
    # 2 knees buckle: hips drop, knees fold forward (lower-right), torso slumps forward, head droops,
    #   sword lowering in front, its blade clear under the hanging shield
    P(head=(84, 70), neck=(81, 81), chest=(79, 87), pelvis=(76, 102),
      r_sh=(68, 84), r_el=(66, 93), r_ha=(71, 99), tip=(104, 112),
      l_sh=(91, 84), l_el=(95, 93), l_ha=(103, 96), shield=(102, 92, 7, 12),
      r_hip=(71, 105), r_kn=(77, 116), r_ft=(62, 122),
      l_hip=(82, 105), l_kn=(92, 113), l_ft=(99, 123),
      head_tilt=-12, eyes="shut", order=["l_arm", "shield", "l_leg", "torso", "r_leg", "head", "sword", "r_arm"]),
    # 3 kneeling (hold): on both knees, slumped, sword tip resting on the ground at the lower-right,
    #   shield hanging from the far hand
    P(head=(82, 74), neck=(79, 85), chest=(77, 91), pelvis=(74, 106),
      r_sh=(66, 88), r_el=(70, 97), r_ha=(76, 103), tip=(106, 123),
      l_sh=(89, 88), l_el=(93, 97), l_ha=(101, 100), shield=(101, 96, 7, 12),
      r_hip=(69, 110), r_kn=(74, 122), r_ft=(60, 115),
      l_hip=(80, 110), l_kn=(86, 121), l_ft=(72, 114),
      head_tilt=-16, eyes="shut", order=["l_leg", "l_arm", "shield", "torso", "r_leg", "head", "sword", "r_arm"]),
    # 4 falling back: torso tips back toward the upper-left, the near hand has let go and reaches back to the
    #   ground; the sword is still in the air (mid-fall), tumbling away to the lower-right above the spot where
    #   it lands on d5; legs slide forward to the lower-right
    P(head=(50, 85), neck=(58, 94), chest=(64, 99), pelvis=(76, 111),
      r_sh=(57, 104), r_el=(51, 111), r_ha=(45, 117), sword=((112, 96), (142, 108)),
      l_sh=(71, 94), l_el=(79, 89), l_ha=(87, 85), shield=(89, 85, 10, 11),
      r_hip=(72, 114), r_kn=(84, 119), r_ft=(97, 124),
      l_hip=(81, 110), l_kn=(92, 104), l_ft=(106, 115),
      head_tilt=50, eyes="shut", order=LYING),
    # 5 hits the ground: on his back along the (96, 48) axis, head upper-left, feet lower-right
    P(head=(40, 90), neck=(52, 96), chest=(59, 99), pelvis=(73, 107),
      r_sh=(50, 104), r_el=(43, 110), r_ha=(36, 115), sword=DROPPED,
      l_sh=(68, 95), l_el=(77, 91), l_ha=(86, 88), shield=(88, 88, 12, 5),
      r_hip=(68, 110), r_kn=(79, 117), r_ft=(93, 123),
      l_hip=(78, 105), l_kn=(91, 109), l_ft=(105, 118),
      head_tilt=80, eyes="shut", order=LYING),
    # 6 bounce: head and shoulders 2 px up, shield rocks
    P(head=(40, 88), neck=(52, 94), chest=(59, 98), pelvis=(73, 107),
      r_sh=(50, 102), r_el=(43, 109), r_ha=(36, 114), sword=DROPPED,
      l_sh=(68, 93), l_el=(77, 90), l_ha=(86, 87), shield=(88, 87, 12, 7),
      r_hip=(68, 110), r_kn=(79, 117), r_ft=(93, 123),
      l_hip=(78, 105), l_kn=(91, 109), l_ft=(105, 118),
      head_tilt=76, eyes="shut", order=LYING),
    # 7 corpse: settled flat (held)
    P(head=(40, 90), neck=(52, 96), chest=(59, 99), pelvis=(73, 107),
      r_sh=(50, 104), r_el=(43, 110), r_ha=(36, 115), sword=DROPPED,
      l_sh=(68, 95), l_el=(77, 91), l_ha=(86, 88), shield=(88, 88, 12, 6),
      r_hip=(68, 110), r_kn=(79, 117), r_ft=(93, 123),
      l_hip=(78, 105), l_kn=(91, 109), l_ft=(105, 118),
      head_tilt=80, eyes="shut", order=LYING),
]

POSES = {"idle": IDLE, "strike": STRIKE, "flinch": FLINCH, "death": DEATH}


def as_json():
    out = {"canvas": [160, 128], "anchor": [80, 126], "sprite_offset": [32, 8], "head_r": HEAD_R,
           "default_order": NEAR_ORDER, "smear": "cw", "anims": {}}  # smear: the blade turns clockwise on screen
    for anim, frames in POSES.items():
        fl = []
        for p in frames:
            q = {k: (list(v) if isinstance(v, tuple) else v) for k, v in p.items()}
            if "sword" in q:
                q["sword"] = [list(q["sword"][0]), list(q["sword"][1])]
                q.pop("tip", None)
            q.setdefault("order", NEAR_ORDER)
            q["order"] = list(q["order"])
            q["facing"] = "lower-right"
            q["far"] = "l"
            fl.append(q)
        out["anims"][anim] = fl
    return out


if __name__ == "__main__":
    (ROOT / "poses_near.json").write_text(json.dumps(as_json(), indent=1))
    print("wrote", ROOT / "poses_near.json")
