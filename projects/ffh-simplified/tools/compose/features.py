"""Terrain feature + city marker sprite definitions.

Each builder returns (Canvas, palette). These are 32x32 map icons drawn
ON TOP of a colored terrain tile, so artwork stays within the central
~24x24 (rows/cols 4..27) and the outer border is left transparent so the
tile color and grid lines show through.
"""

from sprite_lib import Canvas


def ancient_ruins():
    c = Canvas()
    p = {
        'a': '#c8b896',  # stone
        'b': '#8a7a5c',  # stone shadow
        'c': '#5c4f3a',  # rubble
        'd': '#6a9c40',  # moss
    }
    # rubble base
    c.rect(7, 23, 18, 3, 'c')
    # tall broken column (jagged top)
    c.rect(9, 9, 3, 15, 'a')
    c.vline(11, 9, 23, 'b')
    c.px(9, 8, 'a')
    c.px(10, 7, 'a')
    # medium column
    c.rect(14, 13, 3, 11, 'a')
    c.vline(16, 13, 23, 'b')
    # short stump column
    c.rect(20, 18, 3, 6, 'a')
    c.vline(22, 18, 23, 'b')
    # loose rubble chunks
    c.rect(24, 21, 2, 2, 'a')
    c.px(12, 24, 'a')
    # moss tufts
    c.px(9, 23, 'd')
    c.px(21, 23, 'd')
    c.px(14, 23, 'd')
    c.outline()
    return c, p


def mana_spring():
    c = Canvas()
    p = {
        'a': '#3050c0',  # pool deep
        'b': '#60a0ff',  # pool mid
        'c': '#c0e8ff',  # spurt / glow
        'd': '#ffffff',  # sparkle
    }
    # wide flat pool at the base, clearly separate from the spurt above it
    c.disc(16, 22, 4, 'a')
    c.disc(16, 21, 3, 'b')
    # small rising droplets, with a gap above the pool so the two read apart
    c.disc(16, 15, 2, 'c')
    c.disc(16, 11, 1, 'c')
    c.px(16, 8, 'd')
    # sparkles scattered around
    c.px(10, 13, 'd')
    c.px(22, 12, 'd')
    c.px(21, 20, 'd')
    c.px(11, 20, 'd')
    c.outline()
    return c, p


def iron_vein():
    c = Canvas()
    p = {
        'a': '#707880',  # rock
        'b': '#4a5058',  # rock shadow
        'c': '#a05018',  # rust vein
        'd': '#d07838',  # rust vein light
    }
    c.triangle(16, 9, 26, 10, 'a')
    # shadow half
    c.triangle(19, 11, 26, 7, 'b')
    # rust veins
    c.line(9, 24, 14, 15, 'c')
    c.line(14, 15, 13, 10, 'c')
    c.line(18, 23, 22, 13, 'd')
    c.line(22, 13, 21, 9, 'd')
    c.px(14, 15, 'd')
    c.px(22, 13, 'c')
    c.outline()
    return c, p


def gold_mine():
    c = Canvas()
    p = {
        'a': '#ab8f66',  # rock (lighter, warmer)
        'b': '#241a10',  # cave dark (near-black for contrast)
        'c': '#ffd700',  # gold
        'd': '#c9a227',  # gold shadow
    }
    c.triangle(16, 9, 26, 10, 'a')
    # dark cave mouth, high contrast against the rock
    c.disc(16, 22, 3, 'b')
    c.rect(13, 22, 6, 3, 'b')
    # gold nuggets around entrance and on rock face
    c.rect(9, 18, 2, 2, 'c')
    c.rect(21, 16, 2, 2, 'c')
    c.rect(19, 22, 2, 2, 'd')
    c.rect(11, 23, 2, 2, 'd')
    c.px(16, 12, 'c')
    c.px(13, 14, 'd')
    c.outline()
    return c, p


def sacred_grove():
    c = Canvas()
    p = {
        'a': '#6b4423',  # trunk
        'b': '#3f9142',  # leaves
        'c': '#a8f0a0',  # leaf highlight / blossom
        'd': '#ffffff',  # glow
    }
    for tx, ty in ((10, 20), (22, 20), (16, 11)):
        c.vline(tx, ty + 3, ty + 6, 'a')
        c.disc(tx, ty, 3, 'b')
        c.px(tx - 1, ty - 1, 'c')
        c.px(tx + 1, ty - 1, 'c')
    # sacred glow at center
    c.disc(16, 18, 2, 'd')
    c.outline()
    return c, p


def watchtower():
    c = Canvas()
    p = {
        'a': '#989898',  # stone
        'b': '#606060',  # stone shadow
        'c': '#282828',  # window
        'd': '#787878',  # rubble
    }
    c.rect(13, 10, 6, 14, 'a')
    c.vline(18, 10, 23, 'b')
    # broken crenellations (uneven - one merlon missing to read as "ruined")
    c.rect(13, 8, 3, 2, 'a')
    c.rect(17, 8, 2, 2, 'a')
    # tall window slit + a second smaller one
    c.vline(15, 13, 18, 'c')
    c.vline(17, 20, 22, 'c')
    # rubble at base
    c.rect(9, 24, 15, 2, 'd')
    c.rect(8, 23, 2, 2, 'a')
    c.rect(23, 23, 2, 2, 'a')
    c.px(11, 22, 'a')
    c.outline()
    return c, p


def fertile_plains():
    c = Canvas()
    p = {
        'a': '#e8c840',  # wheat
        'b': '#c9a227',  # wheat shadow
        'c': '#5a8a2a',  # stalk
    }
    row1 = [8, 11, 14, 17, 20, 23]
    row2 = [9, 13, 16, 19, 22, 24]
    for x in row1:
        c.vline(x, 16, 23, 'c')
        c.triangle(x, 11, 16, 2, 'a')
    for x in row2:
        c.vline(x, 19, 25, 'c')
        c.triangle(x, 15, 19, 2, 'b')
    c.outline()
    return c, p


def haunted_barrow():
    c = Canvas()
    p = {
        'a': '#4a3c2e',  # mound earth
        'b': '#3a5a2a',  # grass tuft
        'c': '#9098a0',  # headstone
        'd': '#a860d0',  # ghost wisp
    }
    # low, wide earthen mound (kept short so it reads as a hill, not a body)
    c.rect(7, 24, 18, 3, 'a')
    c.rect(9, 22, 14, 2, 'a')
    c.rect(12, 20, 8, 2, 'a')
    c.px(7, 23, 'b')
    c.px(24, 23, 'b')
    c.px(9, 21, 'b')
    c.px(20, 21, 'b')
    # crooked stone grave marker jutting from the crest
    c.rect(14, 12, 3, 8, 'c')
    c.px(16, 11, 'c')
    c.px(17, 10, 'c')
    # ghostly wisp floating off to the side, kept clear of the headstone
    # so the two don't read as a single hooded figure
    c.disc(22, 10, 2, 'd')
    c.disc(24, 7, 1, 'd')
    c.disc(20, 7, 1, 'd')
    c.outline()
    return c, p


def dragon_bones():
    c = Canvas()
    p = {
        'a': '#e8e0c8',  # bone
        'b': '#b8a880',  # bone shadow
        'c': '#ff8844',  # ember glow
    }
    # big horned skull, unmistakably a skull rather than a figure
    c.disc(16, 17, 6, 'a')
    c.rect(11, 17, 11, 4, 'a')  # jaw
    c.disc(19, 19, 3, 'b')  # cheek shadow
    # curling horns, drawn on the left half then mirrored
    c.line(12, 12, 9, 8, 'a')
    c.line(9, 8, 7, 5, 'b')
    c.mirror_left_to_right()
    # glowing eye sockets
    c.disc(13, 15, 2, 'x')
    c.disc(19, 15, 2, 'x')
    c.px(13, 15, 'c')
    c.px(19, 15, 'c')
    # jagged teeth along the jaw
    for x in range(12, 21, 2):
        c.px(x, 21, 'x')
    # small rib bones jutting from the ground at each side
    c.line(9, 26, 6, 22, 'b')
    c.line(23, 26, 26, 22, 'b')
    c.outline()
    return c, p


def crystal_cave():
    c = Canvas()
    p = {
        'a': '#8a8f98',  # rock (lightened for contrast)
        'b': '#40e0e0',  # crystal
        'c': '#c0fff8',  # crystal glow
        'd': '#208080',  # crystal shadow
    }
    c.rect(9, 23, 14, 3, 'a')
    c.hline(9, 22, 22, 'd')
    # crystal shards of varying height
    c.triangle(12, 12, 23, 2, 'b')
    c.triangle(16, 7, 23, 3, 'b')
    c.triangle(20, 14, 23, 2, 'b')
    c.vline(16, 7, 14, 'c')
    c.vline(12, 12, 18, 'd')
    c.px(16, 6, 'c')
    c.outline()
    return c, p


def mineral_deposit():
    c = Canvas()
    p = {
        'a': '#787060',  # rock
        'b': '#544c40',  # rock shadow
        'c': '#d8d8e0',  # ore
        'd': '#a0a0b0',  # ore shadow
    }
    # squat rounded boulder pile (distinct from the sharp mountain triangles
    # used by iron_vein/gold_mine/adamantine_vein)
    c.disc(12, 20, 5, 'a')
    c.disc(19, 20, 5, 'a')
    c.disc(15, 16, 4, 'b')
    c.rect(8, 23, 17, 3, 'a')
    c.rect(10, 20, 3, 2, 'c')
    c.rect(19, 15, 3, 2, 'c')
    c.rect(14, 22, 3, 2, 'd')
    c.px(21, 21, 'c')
    c.outline()
    return c, p


def adamantine_vein():
    c = Canvas()
    p = {
        'a': '#242432',  # dark adamantine rock
        'b': '#4090ff',  # glow vein
        'c': '#a8d8ff',  # halo
        'd': '#ffffff',  # spark
    }
    c.triangle(16, 9, 26, 10, 'a')
    c.line(10, 23, 15, 13, 'b')
    c.line(15, 13, 14, 9, 'b')
    c.line(20, 24, 18, 16, 'b')
    # halo + spark at the vein nodes for a "rare/glowing" read
    c.disc(15, 13, 1, 'c')
    c.disc(14, 9, 1, 'd')
    c.disc(18, 16, 1, 'c')
    c.disc(20, 24, 1, 'd')
    c.px(15, 13, 'd')
    c.px(14, 9, 'd')
    c.outline()
    return c, p


def world_tree():
    c = Canvas()
    p = {
        'a': '#5a3a20',  # trunk
        'b': '#3a2410',  # trunk shadow
        'c': '#2f8f4f',  # canopy
        'd': '#8ef0a8',  # canopy glow
        'e': '#ffe080',  # gold mote
    }
    c.rect(14, 18, 4, 8, 'a')
    c.vline(17, 18, 25, 'b')
    c.line(14, 25, 10, 26, 'a')
    c.line(17, 25, 21, 26, 'a')
    c.disc(16, 13, 7, 'c')
    c.disc(13, 10, 3, 'd')
    # gold motes, drawn 2px so they stay visible against the canopy
    c.disc(9, 11, 1, 'e')
    c.disc(23, 10, 1, 'e')
    c.disc(21, 18, 1, 'e')
    c.disc(11, 19, 1, 'e')
    c.px(16, 6, 'e')
    c.outline()
    return c, p


def hellgate():
    c = Canvas()
    p = {
        'a': '#1a0a14',  # obsidian frame
        'b': '#ff4500',  # flame
        'c': '#ffcc00',  # flame inner
        'd': '#3a0050',  # void
        'e': '#ff9060',  # glow
    }
    # jagged frame pillars
    c.rect(8, 7, 3, 19, 'a')
    c.rect(21, 7, 3, 19, 'a')
    c.px(8, 6, 'a')
    c.px(23, 6, 'a')
    # rift void
    c.disc(16, 17, 5, 'd')
    c.ring(16, 17, 5, 'b')
    c.ring(16, 17, 3, 'c')
    # flame licks at top
    c.triangle(11, 6, 9, 1, 'b')
    c.triangle(16, 5, 8, 2, 'c')
    c.triangle(21, 6, 9, 1, 'b')
    # glow halo
    c.px(6, 10, 'e')
    c.px(25, 10, 'e')
    c.px(6, 20, 'e')
    c.px(25, 20, 'e')
    c.outline()
    return c, p


def titans_grave():
    c = Canvas()
    p = {
        'a': '#5a4a3a',  # dirt mound
        'b': '#c8c8d8',  # blade
        'c': '#8890a0',  # blade shadow
        'd': '#a87830',  # hilt
        'e': '#c080ff',  # glow
    }
    # mound
    c.rect(8, 22, 16, 4, 'a')
    c.rect(10, 20, 12, 2, 'a')
    # giant sword blade
    c.rect(14, 6, 4, 14, 'b')
    c.vline(16, 6, 20, 'c')
    c.triangle(16, 5, 6, 2, 'b')
    # cross-guard and hilt
    c.rect(11, 19, 10, 2, 'd')
    c.rect(15, 21, 2, 3, 'd')
    c.disc(16, 24, 1, 'd')
    # purple glow accents
    c.px(14, 8, 'e')
    c.px(19, 10, 'e')
    c.px(11, 19, 'e')
    c.px(21, 19, 'e')
    c.outline()
    return c, p


def city():
    c = Canvas()
    p = {
        'a': '#e8e8e8',  # wall
        'b': '#b8b8c0',  # wall shadow
        'c': '#382e26',  # gate opening
        'd': '#b8503a',  # roof
        'e': '#d8d8d8',  # tower stone
    }
    # perimeter wall
    c.rect(6, 15, 20, 11, 'a')
    c.vline(25, 15, 25, 'b')
    c.hline(6, 25, 25, 'b')
    # crenellations
    for x in (6, 10, 14, 18, 22):
        c.rect(x, 12, 2, 3, 'a')
    # gate
    c.rect(13, 18, 6, 8, 'c')
    c.disc(16, 18, 3, 'c')
    # rooftops peeking over wall
    c.triangle(10, 10, 14, 3, 'd')
    c.triangle(19, 9, 14, 3, 'd')
    # side tower
    c.rect(20, 8, 5, 8, 'e')
    c.vline(24, 8, 15, 'b')
    c.triangle(22, 5, 8, 3, 'd')
    c.px(22, 11, 'b')
    c.outline()
    return c, p


SPRITES = {
    'ancient_ruins': ancient_ruins,
    'mana_spring': mana_spring,
    'iron_vein': iron_vein,
    'gold_mine': gold_mine,
    'sacred_grove': sacred_grove,
    'watchtower': watchtower,
    'fertile_plains': fertile_plains,
    'haunted_barrow': haunted_barrow,
    'dragon_bones': dragon_bones,
    'crystal_cave': crystal_cave,
    'mineral_deposit': mineral_deposit,
    'adamantine_vein': adamantine_vein,
    'world_tree': world_tree,
    'hellgate': hellgate,
    'titans_grave': titans_grave,
    'city': city,
}
