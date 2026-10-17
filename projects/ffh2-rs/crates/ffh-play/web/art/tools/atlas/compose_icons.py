"""Redraw the UI icons in the atlas look (docs/ART.md 11.5): python3 compose_icons.py ART_DIR [PREVIEW_DIR]

Each recipe draws an icon once in 16x16 coordinates with maplib.Px (crisp lit shapes posterized to
palette ramps, one void outline) and is rendered into every size that exists: icon16/NAME at
16x16 and icon8/NAME at 8x8 (the 16x16 reduced by 2x2 blocks, or drawn by hand in HAND8 where that
loses the subject). Promotion seals keep their family
rim slot (#, recoloured by the mapping's remap) around a shaded navy face. The mana sphere runes
are symbols (a notation, like the font) and keep their shapes; the geometric chrome of marks.spr
(path dots, tile brackets, odds chips) is untouched."""
import math, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from maplib import Px, parse, rewrite, preview, KEYS

ART = sys.argv[1]
PREV = sys.argv[2] if len(sys.argv) > 2 else None
ICONS = {}      # base name -> (fn, files it may live in)
EXTRA = {}      # full sprite name -> fn (marks, cursors: one size)


def icon(fn):
    ICONS[fn.__name__] = fn
    return fn


def named(full):
    def deco(fn):
        EXTRA[full] = fn
        return fn
    return deco


GOLD = 'ylKL'
STEEL = '3456'
BRIGHT = '5678'
WOOD = 'hriu'
SLOT = 'J'      # placeholder for the seal rim, written back as '#'


# ---------------------------------------------------------------- shared shapes
def sheaf(p):
    for dx in (-3.4, 0, 3.4):
        p.add(lambda s, dx=dx: s.stroke([(8 + dx * 0.3, 15), (8 + dx, 6)], 0.6, 0.6, smooth=False), 'iyK', flat=1, sep=False)
    for dx, dy in ((-3.4, 0.4), (0, -0.6), (3.4, 0.4)):
        p.add(lambda s, dx=dx, dy=dy: s.ellipse(8 + dx * 1.05, 4 + dy, 1.5, 3.2, rot=dx * 6), GOLD, sigma=0.9, gloss='L')
    p.add(lambda s: s.rect(5, 10, 11, 11.6), 'riu', flat=1, sep=False)


def coin(p, cx, cy, r, hole=False):
    p.add(lambda s: s.ellipse(cx, cy, r, r), GOLD, sigma=r * 0.6, gloss='8')
    p.add(lambda s: s.ellipse(cx, cy, r * 0.62, r * 0.62), 'yK', sigma=r * 0.4, sep=False, outline=False)
    if hole:
        p.solid(lambda s: s.rect(cx - r * 0.25, cy - r * 0.25, cx + r * 0.25, cy + r * 0.25), '0')


def candle(p, lit=True):
    p.add(lambda s: s.rect(6, 7, 10, 14.4), '678', sigma=1.0)
    p.add(lambda s: s.ellipse(8, 14.6, 5, 1.4), BRIGHT, sigma=0.8)
    if lit:
        p.add(lambda s: s.stroke([(8, 6.6), (8.4, 4), (7.8, 1.2)], 1.4, 0.3), 'ElLL', sigma=0.8, okey='e')
    else:
        p.add(lambda s: s.stroke([(8, 6), (7, 4), (9, 2.4), (8, 0.6)], 0.5, 0.4), '45', flat=1, sep=False, outline=False)
        p.solid(lambda s: s.rect(7.6, 6, 8.4, 7), 'C')


def book(p, cover='nNJ'.replace('J', 'g'), pages='G78'):
    p.add(lambda s: s.poly([(0.6, 13.4), (0.6, 4), (8, 5.4), (15.4, 4), (15.4, 13.4), (8, 14.6)]), cover, sigma=1.0)
    p.add(lambda s: s.poly([(1.6, 12), (1.8, 3.4), (8, 4.8), (8, 13.4)]), pages, sigma=1.2, sep=False)
    p.add(lambda s: s.poly([(8, 13.4), (8, 4.8), (14.2, 3.4), (14.4, 12)]), pages, sigma=1.2, sep=False)
    for y in (6.4, 8.4, 10.4):
        p.add(lambda s, y=y: (s.stroke([(3, y), (6.8, y + 0.4)], 0.3, 0.3, smooth=False), s.stroke([(9.2, y + 0.4), (13, y)], 0.3, 0.3, smooth=False)), '55', flat=0, sep=False, outline=False)


def shield(p, keys=BRIGHT, boss='yK'):
    p.add(lambda s: s.poly([(1.6, 1.4), (14.4, 1.4), (14.4, 7), (8, 15), (1.6, 7)]), keys, sigma=2.4, gloss='8')
    p.add(lambda s: s.ellipse(8, 6.4, 2, 2), boss, sigma=1.0)


def sword(p, x0=4, y0=13, x1=12.6, y1=1.6, blade=BRIGHT, hilt='yK'):
    p.add(lambda s: s.stroke([(x0 + (x1 - x0) * 0.18, y0 - (y0 - y1) * 0.18), (x1, y1)], 1.3, 0.5, smooth=False), blade, sigma=0.8, gloss='8')
    cx, cy = x0 + (x1 - x0) * 0.2, y0 - (y0 - y1) * 0.2
    p.add(lambda s: s.stroke([(cx - 2.6, cy - 2.2), (cx + 2.6, cy + 2.2)], 0.8, 0.8, smooth=False), hilt, flat=1, sep=False)
    p.add(lambda s: s.stroke([(x0, y0), (cx, cy)], 0.8, 0.8, smooth=False), 'hr', flat=1, sep=False)


def boot(p):
    p.add(lambda s: s.poly([(4, 1), (10, 1), (10, 9), (15, 11), (15, 15), (3, 15), (4, 9)]), 'riuj', sigma=1.6)
    p.add(lambda s: s.rect(3.6, 13.6, 15.2, 15), 'hr', flat=1, sep=False)


def crown(p, keys=GOLD, gem='cCR'):
    p.add(lambda s: s.poly([(1, 13), (1, 4), (4.6, 8), (8, 2), (11.4, 8), (15, 4), (15, 13)]), keys, sigma=1.6, gloss='8')
    p.add(lambda s: s.ellipse(8, 10, 1.6, 1.6), gem, sigma=0.8)


def star(p, keys=GOLD, cx=8, cy=8, r=7):
    pts = []
    for i in range(10):
        a = -math.pi / 2 + i * math.pi / 5
        rr = r if i % 2 == 0 else r * 0.45
        pts.append((cx + math.cos(a) * rr, cy + math.sin(a) * rr))
    p.add(lambda s: s.poly(pts), keys, sigma=r * 0.4, gloss='8')


def eye(p, iris='nNg'):
    p.add(lambda s: s.poly([(0.6, 8), (4, 4), (8, 3), (12, 4), (15.4, 8), (12, 12), (8, 13), (4, 12)]), 'G78', sigma=1.6)
    p.add(lambda s: s.ellipse(8, 8, 3.4, 3.4), iris, sigma=1.2)
    p.solid(lambda s: s.ellipse(8, 8, 1.4, 1.4), '0')
    p.put(7, 6, 'F', outline=False)


def sprig(p, keys='qmoO'):
    p.add(lambda s: s.stroke([(4, 15), (8, 9), (11, 2)], 0.6, 0.5), 'qm', flat=1, sep=False)
    for (x, y, a) in ((5.6, 11, -40), (9, 7.4, -30), (10.6, 3.4, -20), (8, 11.4, 30), (11, 7.6, 40)):
        p.add(lambda s, x=x, y=y, a=a: s.ellipse(x, y, 2.4, 1.2, rot=a), keys, sigma=0.8)


def crescent(p, keys='ElLL', cx=8, cy=8, r=6.6):
    p.add(lambda s: (s.ellipse(cx, cy, r, r), s.ellipse(cx + r * 0.5, cy - r * 0.15, r * 0.85, r * 0.85, fill=0)), keys, sigma=r * 0.4, gloss='8')


def gear(p, keys=STEEL, cx=8, cy=8, r=6):
    pts = []
    for i in range(16):
        a = i * math.pi / 8
        rr = r if i % 2 == 0 else r * 0.75
        pts.append((cx + math.cos(a) * rr, cy + math.sin(a) * rr))
    p.add(lambda s: (s.poly(pts), s.ellipse(cx, cy, r * 0.35, r * 0.35, fill=0)), keys, sigma=r * 0.4, gloss='7')


def temple(p, faith):
    p.add(lambda s: s.poly([(1, 6), (8, 1.4), (15, 6)]), BRIGHT, sigma=1.0)
    for x in (2.6, 6, 9.4, 12.8):
        p.add(lambda s, x=x: s.rect(x, 6.4, x + 1.6, 13.4), BRIGHT, sigma=0.6)
    p.add(lambda s: s.rect(1, 13.4, 15, 15.4), STEEL, sigma=0.6)
    FAITH_GLYPH[faith](p, 8, 9.6, 2.6)


# faith glyphs, small, centred on (cx, cy) with size r
def g_leaves(p, cx, cy, r): p.add(lambda s: s.ellipse(cx, cy, r * 0.7, r * 1.2, rot=30), 'qmoO', sigma=r * 0.4)
def g_order(p, cx, cy, r):
    p.add(lambda s: (s.rect(cx - r * 0.25, cy - r, cx + r * 0.25, cy + r), s.rect(cx - r, cy - r * 0.25, cx + r, cy + r * 0.25)), 'yKL', sigma=r * 0.3)
def g_overlords(p, cx, cy, r):
    p.add(lambda s: s.ellipse(cx, cy - r * 0.3, r * 0.8, r * 0.7), 'ksTS', sigma=r * 0.4)
    p.add(lambda s: [s.stroke([(cx + dx, cy), (cx + dx * 1.5, cy + r)], r * 0.2, r * 0.12) for dx in (-r * 0.6, 0, r * 0.6)], 'ksT', flat=1, sep=False)
def g_kilmorph(p, cx, cy, r):
    p.add(lambda s: s.poly([(cx - r * 0.7, cy + r), (cx - r * 0.5, cy - r * 0.6), (cx, cy - r), (cx + r * 0.5, cy - r * 0.6), (cx + r * 0.7, cy + r)]), 'xyY', sigma=r * 0.4)
def g_veil(p, cx, cy, r):
    p.add(lambda s: s.ellipse(cx, cy, r, r * 0.7), 'cCR', sigma=r * 0.4)
    p.solid(lambda s: s.ellipse(cx, cy, r * 0.35, r * 0.35), '0')
def g_empyrean(p, cx, cy, r):
    p.add(lambda s: s.ellipse(cx, cy, r * 0.8, r * 0.8), 'lLL', sigma=r * 0.4, gloss='8')
    for i in range(8):
        a = i * math.pi / 4
        p.add(lambda s, a=a: s.stroke([(cx + math.cos(a) * r, cy + math.sin(a) * r), (cx + math.cos(a) * r * 1.35, cy + math.sin(a) * r * 1.35)], r * 0.14, r * 0.1, smooth=False), 'lL', flat=1, sep=False, outline=False)
def g_esus(p, cx, cy, r):
    p.add(lambda s: s.ellipse(cx, cy, r * 0.8, r), 'vpP', sigma=r * 0.4)
    p.solid(lambda s: (s.ellipse(cx - r * 0.35, cy - r * 0.2, r * 0.2, r * 0.15), s.ellipse(cx + r * 0.35, cy - r * 0.2, r * 0.2, r * 0.15)), '0')


FAITH_GLYPH = {'fellowship_of_leaves': g_leaves, 'the_order': g_order, 'octopus_overlords': g_overlords, 'runes_of_kilmorph': g_kilmorph,
               'the_ashen_veil': g_veil, 'the_empyrean': g_empyrean, 'council_of_esus': g_esus}


# ---------------------------------------------------------------- yields and status (icons.spr)
@icon
def food(p): sheaf(p)


@icon
def production(p):
    p.add(lambda s: s.poly([(1, 9), (15, 9), (13, 11.4), (11, 11.4), (11.6, 15), (4.4, 15), (5, 11.4), (3, 11.4)]), '2345', sigma=1.4, gloss='6')
    p.add(lambda s: s.stroke([(6, 8.4), (11.6, 2.6)], 0.8, 0.8, smooth=False), 'hri', flat=1, sep=False)
    p.add(lambda s: s.poly([(9.6, 0.6), (14.6, 3.4), (13.6, 5.4), (8.6, 2.6)]), STEEL, sigma=0.8)
    p.solid(lambda s: s.ellipse(4, 6, 0.9, 0.9), 'L')


@icon
def commerce(p): coin(p, 8, 8, 7, hole=True)


@icon
def gold(p):
    for (x, y) in ((5, 13), (11, 13), (5, 10.4), (11, 10.4), (8, 7.8), (8, 5.2)):
        p.add(lambda s, x=x, y=y: s.ellipse(x, y, 3.6, 1.8), GOLD, sigma=1.0, gloss='8')
    coin(p, 10.6, 4.6, 3.6)


@icon
def research(p): book(p)


@icon
def culture(p):
    p.add(lambda s: s.rect(1, 1, 15, 15), 'vpP', sigma=2.0)
    p.add(lambda s: (s.stroke([(4, 13), (8, 3), (12, 13)], 1.0, 1.0, smooth=False), s.stroke([(5.6, 9.4), (10.4, 9.4)], 0.8, 0.8, smooth=False)), GOLD, flat=2, sep=False, outline=False)


@icon
def espionage(p):
    p.add(lambda s: s.poly([(0.6, 6), (4, 3.4), (8, 5), (12, 3.4), (15.4, 6), (13.6, 10.6), (8, 9), (2.4, 10.6)]), 'vpP', sigma=1.4)
    p.solid(lambda s: (s.ellipse(4.8, 6.8, 1.6, 1.1), s.ellipse(11.2, 6.8, 1.6, 1.1)), '0')


@icon
def happy(p): candle(p, True)


@icon
def unhappy(p): candle(p, False)


@icon
def angrypop(p):
    p.add(lambda s: s.rect(3, 12, 13, 15.4), 'cIC', sigma=0.8)
    p.add(lambda s: s.poly([(2.6, 12.4), (2, 6), (3.4, 3), (12.6, 3), (14, 6), (13.4, 12.4)]), 'baa', sigma=1.6)
    for x in (4.6, 7.4, 10.2):
        p.add(lambda s, x=x: s.stroke([(x + 1.4, 3.4), (x + 1.4, 7)], 0.3, 0.3, smooth=False), 'rr', flat=0, sep=False, outline=False)


@icon
def healthy(p): sprig(p)


@icon
def unhealthy(p):
    for (x, y, r) in ((5, 10, 3.4), (11, 11, 2.8), (9, 5, 3), (4, 4, 1.8)):
        p.add(lambda s, x=x, y=y, r=r: s.ellipse(x, y, r, r), 'vpP', sigma=r * 0.6, gloss='W')


@icon
def greatpeople(p):
    p.add(lambda s: (s.stroke([(7, 14.6), (3, 12), (1.4, 7.4), (2.6, 3)], 1.4, 0.8), s.stroke([(9, 14.6), (13, 12), (14.6, 7.4), (13.4, 3)], 1.4, 0.8)), 'qmoO', sigma=0.8)
    star(p, GOLD, 8, 7.4, 4.0)


@icon
def defense(p):
    p.add(lambda s: s.rect(3, 4, 13, 14.4), BRIGHT, sigma=1.6)
    for x in (2.6, 6.4, 10.2):
        p.add(lambda s, x=x: s.rect(x, 1, x + 3.2, 4.6), BRIGHT, sigma=0.6)
    p.solid(lambda s: s.poly([(6.6, 14.4), (6.6, 10), (8, 8.6), (9.4, 10), (9.4, 14.4)]), '0')


@icon
def strength(p): shield(p)


@icon
def moves(p): boot(p)


@icon
def experience(p):
    p.add(lambda s: (s.stroke([(3.4, 1), (8, 7)], 1.2, 1.2, smooth=False), s.stroke([(12.6, 1), (8, 7)], 1.2, 1.2, smooth=False)), 'cIC', flat=1, sep=False)
    p.add(lambda s: s.ellipse(8, 10.4, 4.6, 4.6), GOLD, sigma=2.0, gloss='8')
    star(p, 'iyK', 8, 10.4, 2.6)


@icon
def bullet(p): p.add(lambda s: s.ellipse(8, 8, 3.4, 3.4), GOLD, sigma=1.4)


@icon
def turn(p): crescent(p)


@icon
def golden_age(p):
    p.add(lambda s: s.ellipse(8, 11, 5, 5), 'lLL8', sigma=2.0)
    for i in range(7):
        a = math.pi + i * math.pi / 6
        p.add(lambda s, a=a: s.stroke([(8 + math.cos(a) * 6, 11 + math.sin(a) * 6), (8 + math.cos(a) * 8, 11 + math.sin(a) * 8)], 0.6, 0.4, smooth=False), 'lL', flat=1, sep=False, outline=False)
    p.add(lambda s: s.rect(0.5, 11, 15.5, 15.5), GOLD, sigma=1.2)


@icon
def anarchy(p):
    p.add(lambda s: s.poly([(1, 13), (1, 4), (3.4, 7), (5.6, 2), (7.4, 6.4), (7.2, 13)]), GOLD, sigma=1.4)
    p.add(lambda s: s.poly([(9, 14.4), (9.4, 8), (11.4, 9.6), (13.4, 5.4), (15, 10), (14.6, 14.4)]), 'iyK', sigma=1.2)


@icon
def research_needed(p):
    book(p)
    star(p, 'lLL', 12.6, 3.2, 3)


@icon
def idle_city(p):
    p.add(lambda s: s.poly([(1, 9), (13, 9), (11, 11.4), (9, 11.4), (9.6, 15), (3.4, 15), (4, 11.4), (2, 11.4)]), '2345', sigma=1.4)
    p.add(lambda s: (s.rect(13, 1, 15, 7), s.rect(13, 8.4, 15, 10.4)), 'lLL', flat=1, sep=False)


@icon
def ai_idle(p):
    crescent(p, '4567', 6, 7, 5)
    gear(p, STEEL, 11.4, 11, 3.6)


@icon
def stasis(p):
    p.add(lambda s: (s.rect(2, 0.6, 14, 2.4), s.rect(2, 13.6, 14, 15.4)), WOOD, sigma=0.6)
    p.add(lambda s: s.poly([(3.4, 2.4), (12.6, 2.4), (8.8, 8), (12.6, 13.6), (3.4, 13.6), (7.2, 8)]), 'gfF', sigma=1.4)
    p.add(lambda s: s.poly([(5, 12.6), (11, 12.6), (8, 9.4)]), 'yKL', sigma=0.6, sep=False)


@icon
def mana(p): p.add(lambda s: s.poly([(5, 15), (3, 7), (8, 0.6), (13, 7), (11, 15)]), 'vpPW', sigma=2.0, gloss='W')


@icon
def turn_wait(p): crescent(p, '4567')


def armageddon(p, corona):
    if corona:
        p.add(lambda s: s.ellipse(8, 8, 7.4, 7.4), corona, sigma=2.0, light=(0, 0, 1))
    p.add(lambda s: s.ellipse(8, 8, 5.4, 5.4), '012', sigma=2.0)


@icon
def armageddon_low(p): armageddon(p, None)
@icon
def armageddon_mid(p): armageddon(p, 'cI')
@icon
def armageddon_high(p): armageddon(p, 'cIC')
@icon
def armageddon_doom(p): armageddon(p, 'ICRl')


def seal(p, family=SLOT):
    """A promotion seal: the family rim (the # slot) round a shaded navy face."""
    p.add(lambda s: s.ellipse(8, 8, 8, 8), family + family, flat=0, outline=False, sep=False)
    p.add(lambda s: s.ellipse(8, 8, 6.9, 6.9), '1123', sigma=3.0, outline=False, sep=False, light=(-0.4, -0.6, 0.7))


@icon
def _seal(p):
    seal(p)
    star(p, BRIGHT, 8, 8, 4.6)


@icon
def _religion(p): temple(p, 'the_empyrean')


@icon
def _building(p):
    p.add(lambda s: s.rect(2, 6, 14, 15), 'jQY7', sigma=1.6)
    p.add(lambda s: s.poly([(1, 6.6), (8, 1), (15, 6.6)]), 'cICR', sigma=1.0)
    p.solid(lambda s: s.rect(6.6, 10.4, 9.4, 15), 'h')


@icon
def _specialist(p): bust(p)


@icon
def _combat(p): sword(p)


def bust(p, robe='nNg', halo=False):
    p.add(lambda s: s.poly([(2, 15.4), (3, 11), (8, 9.4), (13, 11), (14, 15.4)]), robe, sigma=1.6)
    p.add(lambda s: s.ellipse(8, 6, 3.4, 3.8), 'rbaa', sigma=1.4)
    if halo:
        for i in range(7):
            a = math.pi + i * math.pi / 6
            p.solid(lambda s, a=a: s.ellipse(8 + math.cos(a) * 6, 6.4 + math.sin(a) * 6, 0.6, 0.6), '8')


# ---------------------------------------------------------------- realm.spr
@icon
def alignment_good(p): g_empyrean(p, 8, 8, 5)
@icon
def alignment_neutral(p):
    p.add(lambda s: (s.rect(7.4, 2, 8.6, 14), s.rect(2, 3, 14, 4.2), s.rect(4.6, 14, 11.4, 15.4)), BRIGHT, flat=2, sep=False)
    for x in (3, 13):
        p.add(lambda s, x=x: s.poly([(x - 2.6, 9), (x + 2.6, 9), (x, 11.6)]), BRIGHT, sigma=0.8)
@icon
def alignment_evil(p): crescent(p, 'cICR')
@icon
def civic_government(p): crown(p)
@icon
def civic_cultural_values(p):
    p.add(lambda s: s.rect(2, 1, 3.2, 15.4), WOOD, flat=1, sep=False)
    p.add(lambda s: s.poly([(3.2, 1.4), (14, 1.4), (14, 11), (8.6, 13.6), (3.2, 11)]), 'vpP', sigma=1.6)
    p.add(lambda s: s.poly([(8.6, 3.4), (11, 6.4), (8.6, 9.4), (6.2, 6.4)]), GOLD, sigma=0.8)
@icon
def civic_labor(p):
    p.add(lambda s: (s.stroke([(2, 15), (13, 3)], 0.8, 0.8, smooth=False), s.stroke([(14, 15), (3, 3)], 0.8, 0.8, smooth=False)), WOOD, flat=1, sep=False)
    p.add(lambda s: s.poly([(9.6, 0.6), (15.4, 2.6), (14, 4.6)]), STEEL, sigma=0.6)
    p.add(lambda s: s.poly([(0.6, 2), (4.4, 0.6), (5.6, 4), (2, 5.4)]), STEEL, sigma=0.6)
@icon
def civic_economy(p):
    p.add(lambda s: s.ellipse(8, 10.4, 6, 5), 'riuj', sigma=2.0)
    p.add(lambda s: s.poly([(5, 5.6), (11, 5.6), (12.4, 2), (3.6, 2)]), 'riu', sigma=0.8)
    p.add(lambda s: s.rect(4.6, 5, 11.4, 6.4), GOLD, flat=1, sep=False)
@icon
def civic_membership(p):
    p.add(lambda s: s.rect(2, 1.4, 14, 12), 'G78', sigma=1.6)
    for y in (4, 6.4, 8.8):
        p.add(lambda s, y=y: s.stroke([(4, y), (12, y)], 0.3, 0.3, smooth=False), '55', flat=0, sep=False, outline=False)
    p.add(lambda s: s.ellipse(11, 12.6, 2.6, 2.6), 'cCR', sigma=1.0)


def _religion_icon(f):
    return lambda p: (p.add(lambda s: s.ellipse(8, 8, 7.4, 7.4), '1123', sigma=3.0), FAITH_GLYPH[f](p, 8, 8, 4.6))


for _f in FAITH_GLYPH:
    ICONS['religion_' + _f] = _religion_icon(_f)


# ---------------------------------------------------------------- buildings.spr
for _f in FAITH_GLYPH:
    ICONS['bclass_temple_' + _f] = (lambda f=_f: (lambda p: temple(p, f)))()


@icon
def bclass_military(p):
    shield(p, 'cICR', 'yK')
@icon
def bclass_defense(p): defense(p)
@icon
def bclass_economy(p):
    coin(p, 6, 10, 4.6)
    coin(p, 10.4, 6, 4.6, hole=True)
@icon
def bclass_provision(p):
    p.add(lambda s: s.rect(3, 6, 13, 15), 'riuj', sigma=1.6)
    p.add(lambda s: s.poly([(1.6, 6.6), (8, 1), (14.4, 6.6)]), 'iyKY', sigma=1.0)
    p.solid(lambda s: s.rect(6.6, 10, 9.4, 15), 'h')
@icon
def bclass_health(p):
    p.add(lambda s: s.poly([(8, 0.6), (13.4, 9), (12, 13.6), (8, 15.2), (4, 13.6), (2.6, 9)]), 'dnNg', sigma=2.0, gloss='F')
    p.add(lambda s: (s.rect(7, 6.4, 9, 13), s.rect(4.6, 8.6, 11.4, 10.6)), '78', flat=1, sep=False, outline=False)
@icon
def bclass_harbour(p):
    p.add(lambda s: (s.rect(7.2, 2.6, 8.8, 14), s.rect(3.4, 4.4, 12.6, 5.8)), STEEL, sigma=0.6)
    p.add(lambda s: (s.ellipse(8, 2, 1.8, 1.8), s.ellipse(8, 2, 0.8, 0.8, fill=0)), STEEL, sigma=0.6, sep=False)
    p.add(lambda s: s.stroke([(1.6, 9.6), (3.4, 13.6), (8, 15), (12.6, 13.6), (14.4, 9.6)], 0.9, 0.9), STEEL, flat=2, sep=False)
@icon
def bclass_learning(p):
    p.add(lambda s: s.rect(2, 3, 14, 13), 'G78', sigma=1.6)
    p.add(lambda s: (s.ellipse(2, 3, 1.6, 1.6), s.ellipse(14, 13, 1.6, 1.6)), 'jQY', sigma=0.6)
    for y in (6, 8, 10):
        p.add(lambda s, y=y: s.stroke([(4, y), (12, y)], 0.3, 0.3, smooth=False), '55', flat=0, sep=False, outline=False)
@icon
def bclass_culture(p):
    p.add(lambda s: (s.stroke([(3, 14), (2, 6), (4, 1.6)], 1.0, 0.7), s.stroke([(13, 14), (14, 6), (12, 1.6)], 1.0, 0.7), s.stroke([(3, 14), (13, 14)], 1.0, 1.0, smooth=False)), GOLD, sigma=0.8)
    for x in (5.4, 7.2, 9, 10.8):
        p.add(lambda s, x=x: s.stroke([(x, 4), (x, 13)], 0.25, 0.25, smooth=False), '88', flat=0, sep=False, outline=False)
@icon
def bclass_arcane(p): star(p, 'vpPW')
@icon
def bclass_government(p):
    p.add(lambda s: s.rect(2, 7, 14, 15), BRIGHT, sigma=1.6)
    p.add(lambda s: s.poly([(1, 7.6), (8, 2.4), (15, 7.6)]), 'dnJN'.replace('J', 'N'), sigma=1.0)
    crown(p) if False else p.add(lambda s: s.poly([(5, 2.6), (5, 0.4), (6.6, 1.6), (8, 0.2), (9.4, 1.6), (11, 0.4), (11, 2.6)]), GOLD, sigma=0.6, sep=False)
    p.solid(lambda s: s.rect(6.6, 10.4, 9.4, 15), '0')
@icon
def bclass_forge(p):
    production(p)
@icon
def bclass_wonder(p):
    p.add(lambda s: s.poly([(2, 15), (5, 4), (8, 0.6), (11, 4), (14, 15)]), GOLD, sigma=2.0, gloss='8')
    p.add(lambda s: s.rect(0.6, 13.4, 15.4, 15.4), STEEL, sigma=0.6)
@icon
def bclass_shrine(p):
    p.add(lambda s: s.rect(4, 6, 12, 14), BRIGHT, sigma=1.4)
    p.add(lambda s: s.poly([(2.6, 6.6), (8, 1.4), (13.4, 6.6)]), 'cICR', sigma=0.8)
    p.add(lambda s: s.ellipse(8, 10, 1.6, 1.6), 'lLL', sigma=0.6)


# ---------------------------------------------------------------- kinds.spr
@icon
def combat_melee(p): sword(p, 8, 15, 8, 1)
@icon
def combat_archer(p):
    p.add(lambda s: s.stroke([(10, 1), (4, 4), (2.6, 8), (4, 12), (10, 15)], 1.0, 1.0), WOOD, flat=2, sep=False)
    p.add(lambda s: s.stroke([(10, 1), (10, 15)], 0.3, 0.3, smooth=False), '88', flat=0, sep=False, outline=False)
    p.add(lambda s: s.stroke([(3, 8), (15, 8)], 0.5, 0.5, smooth=False), BRIGHT, flat=2, sep=False)
@icon
def combat_mounted(p):
    p.add(lambda s: s.poly([(4, 15), (5, 8), (3, 6), (6, 1.4), (9, 1), (14, 6), (13, 8.4), (9.4, 7), (10, 15)]), 'riyY', sigma=1.6)
    p.add(lambda s: s.stroke([(6, 1.4), (4, 6), (4.6, 12)], 0.8, 0.6), 'h0', flat=0, sep=False)
@icon
def combat_recon(p): star(p, BRIGHT, 8, 8, 7.4)
@icon
def combat_siege(p):
    p.add(lambda s: s.rect(1, 11, 15, 13.4), WOOD, sigma=0.6)
    p.add(lambda s: (s.ellipse(4, 14, 1.8, 1.8), s.ellipse(12, 14, 1.8, 1.8)), WOOD, sigma=0.6)
    p.add(lambda s: s.stroke([(5, 11), (12, 2)], 0.8, 0.8, smooth=False), WOOD, flat=2, sep=False)
    p.add(lambda s: s.ellipse(12.6, 2, 2, 1.6), STEEL, sigma=0.6)
@icon
def combat_adept(p):
    p.add(lambda s: s.poly([(1, 14), (5, 11), (9, 0.6), (11, 11), (15, 14)]), 'vpP', sigma=2.0)
    star(p, GOLD, 8, 8, 2.2)
@icon
def combat_disciple(p):
    p.add(lambda s: s.rect(4, 4, 12, 14), 'iyK', sigma=1.2)
    p.add(lambda s: s.rect(5.4, 5.6, 10.6, 12.4), 'lLL', sigma=1.2, sep=False, outline=False)
    p.add(lambda s: (s.ellipse(8, 2.6, 2.6, 2.0), s.ellipse(8, 2.6, 1.2, 0.8, fill=0)), 'iyK', sigma=0.6)
@icon
def combat_animal(p):
    p.add(lambda s: s.ellipse(8, 10.6, 4, 3.6), 'hriu', sigma=1.4)
    for (x, y) in ((3, 6), (6.4, 3.4), (9.6, 3.4), (13, 6)):
        p.add(lambda s, x=x, y=y: s.ellipse(x, y, 1.7, 2.0), 'hriu', sigma=0.8)
@icon
def combat_naval(p):
    p.add(lambda s: s.poly([(1, 10), (15, 10), (12.6, 14.4), (3.4, 14.4)]), WOOD, sigma=1.0)
    p.add(lambda s: s.rect(7.4, 1, 8.6, 10), 'hr', flat=1, sep=False)
    p.add(lambda s: s.poly([(9, 1.4), (14, 8.6), (9, 8.6)]), 'G78', sigma=1.0)
    p.add(lambda s: s.poly([(7, 2.4), (2.6, 8.6), (7, 8.6)]), 'G78', sigma=1.0)
@icon
def combat_beast(p):
    p.add(lambda s: s.ellipse(8, 9.4, 5, 5.4), 'hriu', sigma=2.0)
    p.add(lambda s: (s.stroke([(4, 6), (1.6, 3), (2.6, 0.6)], 0.9, 0.5), s.stroke([(12, 6), (14.4, 3), (13.4, 0.6)], 0.9, 0.5)), '678', sigma=0.6, sep=False)
    p.solid(lambda s: (s.ellipse(6, 9, 0.7, 0.7), s.ellipse(10, 9, 0.7, 0.7)), 'L')


@icon
def specialist_citizen(p): bust(p, 'jQY')
@icon
def specialist_priest(p): bust(p, 'G78', halo=True)
@icon
def specialist_artist(p): bclass_culture(p)
@icon
def specialist_scientist(p): book(p, 'ksT')
@icon
def specialist_merchant(p): civic_economy(p)
@icon
def specialist_engineer(p):
    p.add(lambda s: (s.rect(1.6, 2, 3.6, 14), s.rect(1.6, 12, 14, 14)), GOLD, flat=2, sep=False)
    p.add(lambda s: s.stroke([(5, 12), (12.6, 3)], 0.8, 0.8, smooth=False), WOOD, flat=1, sep=False)
    p.add(lambda s: s.poly([(10.6, 0.6), (15, 3.4), (14, 5.6), (9.6, 2.6)]), STEEL, sigma=0.8)


# ---------------------------------------------------------------- promotions.spr (seal + glyph)
def promo(glyph):
    return lambda p: (seal(p), glyph(p))


def small(fn, k=0.74):
    """Draw a 16-space glyph shrunk about the centre (inside a seal)."""
    def f(p):
        sc = p.scale
        q = Px(p.w, p.h, scale=sc * k)
        q.idx = p.idx; q.okey = p.okey; q.nooutline = p.nooutline
        off = 8 * (1 - k)
        orig_mask = q.mask
        def mask(draw, ss=4):
            from maplib import S, Image
            from PIL import ImageDraw
            s = S(q.w, q.h, 1)
            s.k = sc * k
            s.sh.SS = ss
            s.sh.im = Image.new('L', (q.w * ss, q.h * ss), 0)
            s.sh.d = ImageDraw.Draw(s.sh.im)
            ox = off * sc * ss
            class Shift:
                def __getattr__(self2, n):
                    return getattr(s, n)
            # shift by translating every coordinate: wrap the Shape's draw calls
            orig = s.sh.d
            class D:
                def polygon(self2, pts, fill=255): orig.polygon([(x + ox, y + ox) for x, y in pts], fill=fill)
                def ellipse(self2, box, fill=255): orig.ellipse([box[0] + ox, box[1] + ox, box[2] + ox, box[3] + ox], fill=fill)
                def rectangle(self2, box, fill=255): orig.rectangle([box[0] + ox, box[1] + ox, box[2] + ox, box[3] + ox], fill=fill)
            s.sh.d = D()
            draw(s)
            return s.mask()
        q.mask = mask
        fn(q)
        p.idx, p.okey, p.nooutline = q.idx, q.okey, q.nooutline
    return f


PROMOS = {
    'promo_offense': small(lambda p: sword(p)),
    'promo_defense': small(lambda p: shield(p)),
    'promo_ranged': small(lambda p: (p.add(lambda s: s.stroke([(2, 14), (13, 3)], 0.6, 0.6, smooth=False), WOOD, flat=2, sep=False),
                                     p.add(lambda s: s.poly([(14.6, 1.4), (10, 2.6), (13.4, 6)]), BRIGHT, sigma=0.6),
                                     p.add(lambda s: s.poly([(1, 15), (2, 11), (5, 14)]), 'cC', sigma=0.4))),
    'promo_mobility': small(boot),
    'promo_terrain': small(lambda p: (p.add(lambda s: s.ellipse(8, 15, 8, 5), 'qmo', sigma=2.0),
                                      p.add(lambda s: s.poly([(8, 1), (12.6, 11), (3.4, 11)]), 'ktTm', sigma=1.4))),
    'promo_medic': small(sprig),
    'promo_command': small(lambda p: (p.add(lambda s: s.rect(3, 1, 4.4, 15), WOOD, flat=1, sep=False),
                                      p.add(lambda s: s.poly([(4.4, 1.4), (14, 3), (11, 6), (14, 9), (4.4, 9)]), 'cICR', sigma=1.0))),
    'promo_sight': small(eye),
    'promo_stealth': small(lambda p: (p.add(lambda s: s.poly([(2, 15), (3, 6), (8, 1), (13, 6), (14, 15)]), '1234', sigma=2.0),
                                      p.solid(lambda s: s.ellipse(8, 8, 3, 3.4), '0'))),
    'promo_arcane': small(lambda p: star(p, 'vpPW')),
    'promo_ward': small(lambda p: p.add(lambda s: (s.ellipse(8, 8, 7, 7), s.ellipse(8, 8, 4.6, 4.6, fill=0)), 'gfF8', sigma=1.2, gloss='8')),
    'promo_slaying': small(lambda p: (p.add(lambda s: s.ellipse(8, 7, 6, 6), BRIGHT, sigma=2.0),
                                      p.add(lambda s: s.rect(5, 11, 11, 15), BRIGHT, sigma=0.8),
                                      p.solid(lambda s: (s.ellipse(5.6, 7, 1.6, 1.8), s.ellipse(10.4, 7, 1.6, 1.8)), '0'))),
    'promo_naval': small(lambda p: bclass_harbour(p)),
    'promo_weapons': small(lambda p: (sword(p, 3, 14, 13.4, 2), sword(p, 13, 14, 2.6, 2))),
    'promo_race': small(lambda p: p.add(lambda s: s.poly([(4, 15), (4, 9), (3, 6), (5, 2), (9, 1), (12, 3.4), (13, 7), (11, 8), (11.4, 10.4), (9, 11), (9, 15)]), 'rbaa', sigma=1.6)),
    'promo_equipment': small(lambda p: (p.add(lambda s: s.rect(1.6, 6, 14.4, 14), WOOD, sigma=1.4),
                                        p.add(lambda s: s.ellipse(8, 6, 6.4, 3.4), 'riuj', sigma=1.0),
                                        p.add(lambda s: s.rect(7, 7, 9, 10), GOLD, flat=2, sep=False))),
    'promo_affliction': small(lambda p: p.add(lambda s: s.poly([(8, 0.6), (13, 9), (11.6, 13.6), (8, 15.2), (4.4, 13.6), (3, 9)]), 'wzAO', sigma=2.0, gloss='O')),
    'promo_heroic': small(crown),
    'promo_star': small(lambda p: star(p, BRIGHT)),
}
for _n, _g in PROMOS.items():
    ICONS[_n] = promo(_g)


def promo_sphere(old_rows):
    """A new seal around the sphere rune as the first set drew it (the rune is a symbol)."""
    def f(p):
        seal(p)
        for y, row in enumerate(old_rows):
            for x, c in enumerate(row):
                if c not in '.1#':
                    p.put(x, y, c, outline=False)
    return f


# ---------------------------------------------------------------- marks.spr (8x8 state marks, cursors)
@named('ui/mark_fortify')
def m_fortify(p): shield(p)
@named('ui/mark_sleep')
def m_sleep(p): crescent(p, '4567')
@named('ui/mark_promote')
def m_promote(p): star(p, GOLD)
@named('ui/mark_sentry')
def m_sentry(p): eye(p)
@named('ui/mark_heal')
def m_heal(p): sprig(p)
@named('ui/mark_auto')
def m_auto(p): gear(p)
@named('ui/path_attack')
def m_path_attack(p): sword(p, 3, 14, 13.4, 2, blade='cCR', hilt='yK'); sword(p, 13, 14, 2.6, 2, blade='cCR', hilt='yK')
@named('ui/cursor_attack')
def m_cursor_attack(p): m_path_attack(p)
@named('ui/cursor_cast')
def m_cursor_cast(p): star(p, 'vpPW')
@named('ui/cursor_busy')
def m_cursor_busy(p): candle(p, True)


def budget_rows(rows, maxc):
    import numpy as np
    from maplib import KI, reduce_colours
    idx = np.array([[KI[c] if c in KI else (-1 if c == '.' else -9) for c in r] for r in rows])
    keep = idx == -9
    red = reduce_colours(np.where(keep, -1, idx), maxc)
    return [''.join(rows[y][x] if keep[y, x] else ('.' if red[y, x] < 0 else KEYS[red[y, x]]) for x in range(len(rows[0]))) for y in range(len(rows))]


def halve(rows):
    """16x16 sprite rows -> 8x8 by 2x2 blocks: transparent if most of the block is; else the most
    common colour of the block, preferring the outline on the silhouette's edge so it stays closed."""
    from collections import Counter
    H, W = len(rows), len(rows[0])
    out = []
    for by in range(0, H, 2):
        line = ''
        for bx in range(0, W, 2):
            px = [rows[by + j][bx + i] for j in (0, 1) for i in (0, 1)]
            opaque = [c for c in px if c != '.']
            if len(opaque) < 2:
                line += '.'
                continue
            cnt = Counter(opaque)
            inner = [c for c in opaque if c != '0']
            if len(opaque) < 4 and '0' in cnt:
                line += '0'
            elif inner and len(inner) >= 2:
                line += Counter(inner).most_common(1)[0][0]
            else:
                line += cnt.most_common(1)[0][0]
        out.append(line)
    return out


# 8x8 icons drawn by hand where the 2x2 reduction loses the subject
HAND8 = {
    'icon8/research': ['........',
                       '.00..00.',
                       '08800880',
                       '085G0580',
                       '08850880',
                       '0nNNNNn0',
                       '.000000.',
                       '........'],
    # an ear of wheat: kernels rising in chevrons from the stalk
    'icon8/food': ['...LK...',
                   '.L.Ky.K.',
                   '..LKyK..',
                   '.L.Ky.K.',
                   '..LKyK..',
                   '...Ky...',
                   '...Ky...',
                   '...yi...'],
    # a mallet: stone head, wooden haft
    'icon8/production': ['.000000.',
                         '05666650',
                         '04444440',
                         '.00ri00.',
                         '..0ri0..',
                         '..0ri0..',
                         '..0ri0..',
                         '..0000..'],
    # a stack of coins (commerce is the single holed coin)
    'icon8/gold': ['.000000.',
                   '0LLKKKy0',
                   '0yyyyyx0',
                   '0LKKKKy0',
                   '0yyyyyx0',
                   '0LKKKKy0',
                   '0yyyyyx0',
                   '.000000.'],
    # a domino mask
    'icon8/espionage': ['........',
                        '........',
                        '.000000.',
                        '0PPPPPP0',
                        '0P0PP0P0',
                        '0pp00pp0',
                        '.00..00.',
                        '........'],
    # the lit candle on its dish; unhappy is the same candle snuffed, a wisp of smoke over it
    'icon8/happy': ['...00...',
                    '..0LL0..',
                    '..0Ll0..',
                    '..0870..',
                    '..0870..',
                    '..0870..',
                    '07766660',
                    '.000000.'],
    'icon8/unhappy': ['....5...',
                      '...5....',
                      '....0...',
                      '..0760..',
                      '..0760..',
                      '..0760..',
                      '07766660',
                      '.000000.'],
    # a clenched fist over a crimson cuff
    'icon8/angrypop': ['........',
                       '.000000.',
                       '0aabaab0',
                       '0aaaaab0',
                       '0bbbbab0',
                       '.0aaab0.',
                       '.0CCCC0.',
                       '..0000..'],
    # a green leaf on its stem
    'icon8/healthy': ['....0000',
                      '...0OOq0',
                      '..0OOqm0',
                      '.0OOqmm0',
                      '.0Oqmm0.',
                      '.0qmm0..',
                      '0q000...',
                      'q0......'],
    # sickly purple bubbles
    'icon8/unhealthy': ['.....00.',
                        '....0Pp0',
                        '....0pv0',
                        '.000.00.',
                        '08Pp000.',
                        '0Ppv0Pp0',
                        '0pvv0pv0',
                        '.000.00.'],
    # the amber crescent (your turn)
    'icon8/turn': ['..000...',
                   '.0LL0...',
                   '0LL0....',
                   '0Ll0....',
                   '0Ll0....',
                   '0Lll00..',
                   '.0llll0.',
                   '..0000..'],
    # a crown broken in two, the right half fallen a row
    'icon8/anarchy': ['........',
                      '.0.0....',
                      '0L0K0.0.',
                      '0LKK00K0',
                      '0yKy0KK0',
                      '0yyy0Ky0',
                      '.0000yy0',
                      '....000.'],
    # good: a golden sun with rays
    'icon8/alignment_good': ['...LL...',
                             '.L0000L.',
                             '.0LLKK0.',
                             'L0LKKK0L',
                             'L0KKKl0L',
                             '.0KKll0.',
                             '.L0000L.',
                             '...LL...'],
    # neutral: stone scales
    'icon8/alignment_neutral': ['...87...',
                                '87777776',
                                '7..76..6',
                                '7..76..6',
                                '77.76.66',
                                '...76...',
                                '..7766..',
                                '.677665.'],
    # civic category: labor (pick and spade crossed)
    'icon8/civic_labor': ['566...66',
                          '6r....66',
                          '..r..r..',
                          '...rr...',
                          '...rr...',
                          '..r..r..',
                          '.r....r.',
                          'r......r'],
}


# ================================================================== run
FILES = ['ui/icons.spr', 'ui/realm.spr', 'ui/buildings.spr', 'ui/kinds.spr', 'ui/promotions.spr', 'ui/marks.spr']
BUDGET = {16: 8, 8: 6}

if __name__ == '__main__':
    shots = []
    for f in FILES:
        path = os.path.join(ART, f)
        items = {it['name']: it for kind, it in parse(path) if kind == 'sprite'}
        new = {}
        for name, sp in items.items():
            fn = None
            if name in EXTRA:
                fn = EXTRA[name]
            elif name.startswith(('icon16/', 'icon8/')):
                base = name.split('/', 1)[1]
                if base in ICONS:
                    fn = ICONS[base]
                elif base.startswith('promo_sphere_'):
                    fn = promo_sphere(sp['rows'])
            if fn is None:
                continue
            seal_icon = base.startswith('promo') or base == '_seal' if name.startswith('icon') else False
            if name in HAND8:
                rows = HAND8[name]
            elif sp['w'] == 8 and name.startswith('icon8/'):
                # the 8x8 icon is the finished 16x16 one reduced by 2x2 blocks (majority, outline kept)
                q = Px(16, 16, 0, 0)
                fn(q)
                rows = budget_rows(halve(q.rows(budget=8, outline=not seal_icon)), 6)
            else:
                p = Px(sp['w'], sp['h'], sp['ax'], sp['ay'], scale=sp['w'] / 16.0)
                fn(p)
                rows = p.rows(budget=BUDGET.get(sp['w'], 8), outline=not seal_icon)
            if seal_icon:
                rows = [r.replace(SLOT, '#') for r in rows]
            note = None
            new[name] = (rows, {'note': None})
            shots.append((name, rows))
        rewrite(path, new)
        print(f, len(new), 'icons redrawn')
    if PREV:
        preview([s for s in shots if s[0].startswith('icon16/') or s[0].startswith('ui/')], os.path.join(PREV, 'icons16.png'), scale=4, bg=(20, 34, 62), cols=14)
        preview([s for s in shots if s[0].startswith('icon8/')], os.path.join(PREV, 'icons8.png'), scale=6, bg=(20, 34, 62), cols=14)
