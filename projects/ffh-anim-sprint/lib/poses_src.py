"""Source of poses.json: 2D joint positions per frame, in 1x canvas coordinates (160x128, anchor (80,126)).

Frame-0 joints were annotated on the static sprite at 6x (sprite coords + (32, 8)). Every other frame is
written as explicit positions following beats.md. One pose spec drives both the blockout guides (guides.py)
and the rig (rig.py), so arms differ only in how the pixels are made.

  python lib/poses_src.py          # writes poses.json

Joints: head (centre; radius HEAD_R), neck, chest, pelvis; r_* = right side = sword arm/leg (FAR side, screen-left),
l_* = left side = shield arm/leg (NEAR side, screen-right); sh/el/ha shoulder/elbow/hand, hip/kn/ft hip/knee/ankle.
tip = sword tip (blade runs from r_ha to tip unless "sword" gives a dropped sword as [hilt, tip]).
shield = [cx, cy, rx, ry]. head_tilt = degrees (+ = tipped back toward the upper-right). eyes = open|shut.
order = back-to-front draw order of parts for that frame.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEAD_R = 13
DEFAULT_ORDER = ["sword", "r_arm", "r_leg", "torso", "l_leg", "head", "l_arm", "shield"]

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


IDLE = [
    P(),
    up(P(), ["head", "neck", "chest", "r_sh", "l_sh"], -1),
    up(up(P(), ["head", "neck", "chest", "r_sh", "l_sh"], -1), ["tip"], -2) | {"shield": (104, 87, 9, 15), "l_ha": (102, 86), "r_ha": (61, 83)},
    up(P(), ["tip"], -1) | {"shield": (104, 87, 9, 15)},
]

STRIKE = [
    P(),
    P(head=(86, 53), neck=(86, 66), chest=(85, 74), pelvis=(85, 91),
      r_sh=(73, 69), r_el=(70, 58), r_ha=(76, 48), tip=(96, 18),
      l_sh=(97, 69), l_el=(99, 78), l_ha=(103, 85), shield=(104, 86, 9, 15),
      r_hip=(80, 95), r_kn=(74, 106), l_hip=(91, 95), l_kn=(94, 106), head_tilt=6,
      order=["r_leg", "torso", "l_leg", "r_arm", "sword", "head", "l_arm", "shield"]),
    P(head=(89, 55), neck=(88, 68), chest=(88, 75), pelvis=(87, 92),
      r_sh=(76, 70), r_el=(80, 57), r_ha=(88, 50), tip=(118, 68),
      l_sh=(99, 70), l_el=(101, 79), l_ha=(104, 85), shield=(105, 86, 9, 15),
      r_hip=(82, 96), r_kn=(75, 107), l_hip=(93, 96), l_kn=(95, 107), head_tilt=8,
      order=["sword", "r_leg", "torso", "l_leg", "r_arm", "head", "l_arm", "shield"]),
    P(head=(74, 55), neck=(76, 68), chest=(76, 75), pelvis=(80, 92),
      r_sh=(68, 70), r_el=(60, 58), r_ha=(54, 52), tip=(22, 50),
      l_sh=(91, 70), l_el=(96, 79), l_ha=(101, 85), shield=(103, 86, 9, 15),
      r_hip=(75, 96), r_kn=(66, 108), r_ft=(58, 123), l_hip=(86, 95), l_kn=(91, 108), head_tilt=-4,
      order=["r_leg", "torso", "l_leg", "head", "r_arm", "sword", "l_arm", "shield"]),
    P(head=(70, 59), neck=(72, 71), chest=(72, 78), pelvis=(78, 94),
      r_sh=(64, 73), r_el=(56, 82), r_ha=(50, 90), tip=(20, 106),
      l_sh=(86, 72), l_el=(94, 80), l_ha=(100, 84), shield=(102, 84, 9, 15),
      r_hip=(73, 97), r_kn=(63, 110), r_ft=(58, 123), l_hip=(84, 97), l_kn=(92, 110), l_ft=(100, 123), head_tilt=-8,
      order=["r_leg", "torso", "l_leg", "head", "r_arm", "sword", "l_arm", "shield"]),
    P(head=(71, 60), neck=(73, 72), chest=(73, 79), pelvis=(78, 95),
      r_sh=(65, 74), r_el=(58, 86), r_ha=(54, 96), tip=(34, 121),
      l_sh=(87, 73), l_el=(96, 79), l_ha=(102, 82), shield=(104, 82, 9, 15),
      r_hip=(73, 98), r_kn=(63, 110), r_ft=(58, 123), l_hip=(84, 98), l_kn=(92, 110), l_ft=(100, 123), head_tilt=-6,
      order=["r_leg", "torso", "l_leg", "head", "r_arm", "sword", "l_arm", "shield"]),
    P(head=(77, 56), neck=(78, 69), chest=(77, 76), pelvis=(80, 92),
      r_sh=(67, 70), r_el=(62, 80), r_ha=(56, 86), tip=(40, 58),
      l_sh=(93, 70), l_el=(97, 79), l_ha=(102, 86), shield=(104, 87, 9, 15),
      r_hip=(76, 95), r_kn=(68, 107), r_ft=(62, 122), l_hip=(87, 95), l_kn=(92, 107), head_tilt=-2),
    up(P(r_ha=(61, 85), tip=(46, 53)), ["head", "neck", "chest", "pelvis", "r_sh", "l_sh", "r_hip", "l_hip"], 1),
]

FLINCH = [
    P(head=(86, 52), neck=(85, 65), chest=(85, 73), pelvis=(84, 90),
      r_sh=(73, 68), r_el=(68, 76), r_ha=(60, 78), tip=(44, 58),
      l_sh=(98, 68), l_el=(103, 76), l_ha=(108, 82), shield=(110, 82, 9, 15),
      r_hip=(79, 94), r_kn=(75, 106), l_hip=(90, 94), l_kn=(93, 107), head_tilt=14, eyes="shut"),
    P(head=(89, 55), neck=(88, 68), chest=(88, 76), pelvis=(86, 93),
      r_sh=(76, 71), r_el=(70, 80), r_ha=(63, 82), tip=(44, 68),
      l_sh=(99, 71), l_el=(103, 80), l_ha=(107, 86), shield=(109, 86, 9, 15),
      r_hip=(81, 97), r_kn=(77, 108), l_hip=(91, 97), l_kn=(95, 109), head_tilt=10, eyes="shut"),
    P(head=(85, 54), neck=(84, 67), chest=(84, 75), pelvis=(84, 92),
      r_sh=(72, 70), r_el=(68, 79), r_ha=(62, 84), tip=(46, 57),
      l_sh=(97, 70), l_el=(100, 79), l_ha=(104, 87), shield=(106, 88, 9, 15),
      r_hip=(79, 95), r_kn=(75, 107), l_hip=(90, 95), l_kn=(93, 108), head_tilt=5),
    P(head=(83, 53), neck=(83, 66), chest=(82, 74), pelvis=(83, 91),
      r_sh=(70, 69), r_el=(67, 78), r_ha=(61, 84), tip=(47, 52),
      l_sh=(96, 69), l_el=(98, 79), l_ha=(103, 87), shield=(105, 88, 9, 15), head_tilt=2),
]

DEATH = [
    dict(FLINCH[0]),
    P(head=(96, 58), neck=(94, 70), chest=(92, 77), pelvis=(88, 93),
      r_sh=(80, 72), r_el=(74, 62), r_ha=(68, 54), tip=(52, 34),
      l_sh=(100, 74), l_el=(104, 84), l_ha=(106, 92), shield=(107, 92, 9, 15),
      r_hip=(83, 96), r_kn=(78, 108), r_ft=(70, 122), l_hip=(92, 96), l_kn=(99, 108), l_ft=(104, 122),
      head_tilt=18, eyes="shut", order=["r_leg", "torso", "l_leg", "r_arm", "sword", "head", "l_arm", "shield"]),
    P(head=(86, 68), neck=(87, 80), chest=(88, 86), pelvis=(88, 102),
      r_sh=(80, 82), r_el=(76, 92), r_ha=(70, 100), tip=(52, 96),
      l_sh=(96, 82), l_el=(100, 92), l_ha=(102, 100), shield=(104, 100, 9, 15),
      r_hip=(83, 104), r_kn=(76, 112), r_ft=(68, 123), l_hip=(93, 104), l_kn=(98, 114), l_ft=(104, 124),
      head_tilt=-12, eyes="shut"),
    P(head=(87, 76), neck=(88, 87), chest=(88, 92), pelvis=(88, 108),
      r_sh=(80, 88), r_el=(75, 98), r_ha=(70, 106), tip=(50, 120),
      l_sh=(96, 88), l_el=(99, 98), l_ha=(101, 106), shield=(103, 106, 9, 15),
      r_hip=(83, 110), r_kn=(74, 121), r_ft=(86, 116), l_hip=(93, 110), l_kn=(96, 123), l_ft=(110, 118),
      head_tilt=-16, eyes="shut", order=["r_leg", "sword", "r_arm", "torso", "l_leg", "head", "l_arm", "shield"]),
    P(head=(110, 84), neck=(104, 93), chest=(102, 98), pelvis=(90, 112),
      r_sh=(94, 93), r_el=(90, 85), r_ha=(86, 79), sword=((60, 119), (30, 124)),
      l_sh=(108, 102), l_el=(113, 108), l_ha=(117, 112), shield=(117, 110, 10, 12),
      r_hip=(86, 114), r_kn=(76, 120), r_ft=(64, 124), l_hip=(94, 115), l_kn=(98, 122), l_ft=(84, 125),
      head_tilt=40, eyes="shut", order=["sword", "r_arm", "r_leg", "torso", "l_leg", "head", "l_arm", "shield"]),
    P(head=(112, 98), neck=(104, 103), chest=(96, 106), pelvis=(80, 115),
      r_sh=(87, 101), r_el=(80, 96), r_ha=(72, 95), sword=((60, 119), (30, 124)),
      l_sh=(105, 111), l_el=(110, 117), l_ha=(116, 121), shield=(116, 119, 12, 6),
      r_hip=(74, 112), r_kn=(63, 116), r_ft=(52, 121), l_hip=(86, 118), l_kn=(72, 121), l_ft=(60, 126),
      head_tilt=64, eyes="shut", order=["sword", "r_arm", "r_leg", "torso", "l_leg", "head", "l_arm", "shield"]),
    P(head=(112, 95), neck=(104, 101), chest=(96, 104), pelvis=(80, 115),
      r_sh=(87, 99), r_el=(80, 94), r_ha=(72, 93), sword=((60, 119), (30, 124)),
      l_sh=(105, 109), l_el=(110, 115), l_ha=(116, 119), shield=(116, 117, 12, 7),
      r_hip=(74, 112), r_kn=(63, 116), r_ft=(52, 121), l_hip=(86, 118), l_kn=(72, 121), l_ft=(60, 126),
      head_tilt=60, eyes="shut", order=["sword", "r_arm", "r_leg", "torso", "l_leg", "head", "l_arm", "shield"]),
    P(head=(113, 99), neck=(104, 104), chest=(96, 107), pelvis=(80, 115),
      r_sh=(87, 102), r_el=(80, 97), r_ha=(72, 96), sword=((60, 119), (30, 124)),
      l_sh=(105, 112), l_el=(110, 118), l_ha=(116, 122), shield=(116, 120, 12, 6),
      r_hip=(74, 112), r_kn=(63, 116), r_ft=(52, 121), l_hip=(86, 118), l_kn=(72, 121), l_ft=(60, 126),
      head_tilt=66, eyes="shut", order=["sword", "r_arm", "r_leg", "torso", "l_leg", "head", "l_arm", "shield"]),
]

POSES = {"idle": IDLE, "strike": STRIKE, "flinch": FLINCH, "death": DEATH}


def as_json():
    out = {"canvas": [160, 128], "anchor": [80, 126], "sprite_offset": [32, 8], "head_r": HEAD_R,
           "default_order": DEFAULT_ORDER, "anims": {}}
    for anim, frames in POSES.items():
        fl = []
        for p in frames:
            q = {k: (list(v) if isinstance(v, tuple) else v) for k, v in p.items()}
            if "sword" in q:
                q["sword"] = [list(q["sword"][0]), list(q["sword"][1])]
                q.pop("tip", None)
            q.setdefault("order", DEFAULT_ORDER)
            fl.append(q)
        out["anims"][anim] = fl
    return out


if __name__ == "__main__":
    (ROOT / "poses.json").write_text(json.dumps(as_json(), indent=1))
    print("wrote", ROOT / "poses.json")
