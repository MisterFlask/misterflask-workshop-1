"""The beat sheet as data (mirrors beats.md). Prompts and manifests read timings and pose text from here."""

BEATS = {
    "idle": [
        {"ms": 250, "text": "the ready pose exactly as in the reference: sword raised in the right hand, shield on the left arm"},
        {"ms": 250, "text": "breathing in: chest and head rise by one pixel, shoulders lift slightly; the sword has not moved yet"},
        {"ms": 250, "text": "top of the breath: chest and head still raised; the sword tip now rises a little and the shield lifts slightly"},
        {"ms": 250, "text": "breathing out: chest and head back down; the sword tip still slightly raised, settling"},
    ],
    "strike": [
        {"ms": 100, "text": "ready: the reference pose exactly"},
        {"ms": 120, "text": "anticipation: weight shifts back onto the rear leg, torso leans back, the sword is lifted up over the head with the tip pointing up and back"},
        {"ms": 160, "text": "wind-up extreme: body coiled back and slightly crouched, the sword cocked behind the head with the blade hanging down behind the shoulders, shield tucked in"},
        {"ms": 50, "text": "swing: the front foot steps toward the lower-left, the torso whips forward, the arm sweeps over the head and the blade is level, pointing to the lower-left"},
        {"ms": 120, "text": "contact: sword arm fully extended toward the lower-left, the blade slashing down-left, front knee bent, body low and leaning in, shield arm swung back for balance", "events": ["hit"]},
        {"ms": 120, "text": "follow-through: the blade carries on down until its tip nearly touches the ground at the lower-left, torso twisted, hair trailing back"},
        {"ms": 100, "text": "recover: body rises, front foot pulls back halfway, the sword lifts back toward the ready angle"},
        {"ms": 100, "text": "settle: almost the ready pose, sword a touch lower than in the reference"},
    ],
    "flinch": [
        {"ms": 80, "text": "impact: hit from the lower-left; head snaps back toward the upper-right, eyes squeezed shut, torso jolts back, both arms jerk outward (shield flung out to the right, sword arm thrown up and out)"},
        {"ms": 120, "text": "recoil extreme: leaning back further, knees bent, sword dipping, shield still held out"},
        {"ms": 120, "text": "recovering: halfway back to the ready pose"},
        {"ms": 120, "text": "settle: nearly the ready pose"},
    ],
    "death": [
        {"ms": 80, "text": "impact: hit from the lower-left; head snaps back, eyes squeezed shut, torso jolts back, arms jerk outward"},
        {"ms": 120, "text": "stagger back: a step back with the near foot, body thrown back, sword arm flung up, shield drooping"},
        {"ms": 140, "text": "knees buckle: hips drop, knees fold forward, torso slumps forward, head droops, sword lowering"},
        {"ms": 200, "text": "kneeling: on both knees, slumped forward, sword tip resting on the ground, shield arm hanging"},
        {"ms": 80, "text": "falling back: torso tips back toward the upper-right, the sword slips out of the hand, legs slide forward"},
        {"ms": 80, "text": "hits the ground: lying on his back along the ground, feet toward the lower-left and head toward the upper-right; the sword lies on the ground at the lower-left; the shield lies flat beside him"},
        {"ms": 80, "text": "bounce: head and shoulders lift slightly off the ground, the shield rocks"},
        {"ms": 400, "text": "corpse: lying flat and still on his back, eyes closed, sword and shield on the ground beside him"},
    ],
}
LOOP = {"idle": True, "strike": False, "flinch": False, "death": False}
HOLD_LAST = {"idle": False, "strike": False, "flinch": False, "death": True}
COUNTS = {k: len(v) for k, v in BEATS.items()}


def ms(anim):
    return [b["ms"] for b in BEATS[anim]]


def events(anim):
    return [b.get("events", []) for b in BEATS[anim]]
