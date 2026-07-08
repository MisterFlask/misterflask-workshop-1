"""Soldier sprite definitions (all soldier types except the four
hand-authored originals: fighter, archer, cleric, mage).

Each builder returns (Canvas, palette). Palette chars are per-sprite;
'.' and 'x' come from BASE_PALETTE automatically.
"""

from sprite_lib import Canvas, humanoid


def knight():
    c = Canvas()
    p = {'s': '#c0a080', 'm': '#c8c8d8', 'd': '#8890a8', 'b': '#4040ff',
         'w': '#e8e8f0'}
    humanoid(c, skin='s', torso='m', legs='d')
    # great helm over the head
    c.rect(12, 3, 7, 4, 'm')
    c.hline(13, 17, 2, 'm')
    c.hline(13, 17, 5, 'd')          # visor slit shading
    # shield on left arm
    c.rect(8, 10, 4, 6, 'd')
    c.vline(8, 10, 15, 'w')
    c.px(9, 16, 'd')
    c.px(10, 16, 'd')
    # sword raised on right
    c.vline(22, 4, 12, 'w')
    c.hline(20, 24, 12, 'd')
    c.px(22, 13, 's')
    c.outline()
    return c, p


def cavalry():
    c = Canvas()
    p = {'h': '#8b5a2b', 'd': '#5a3a1a', 's': '#c0a080', 'm': '#a8b0c0',
         'w': '#e8e8f0'}
    # horse torso
    c.rect(4, 17, 17, 6, 'h')
    # horse neck & head (facing right)
    c.rect(19, 11, 4, 7, 'h')
    c.disc(24, 10, 3, 'h')
    c.px(27, 10, 'h')
    # mane
    c.vline(19, 9, 16, 'd')
    # tail
    c.line(4, 18, 1, 23, 'd')
    c.line(5, 18, 2, 23, 'd')
    # legs
    c.rect(6, 23, 2, 6, 'h')
    c.rect(10, 23, 2, 6, 'h')
    c.rect(14, 23, 2, 6, 'h')
    c.rect(17, 23, 2, 6, 'h')
    c.rect(6, 29, 2, 1, 'd')
    c.rect(10, 29, 2, 1, 'd')
    c.rect(14, 29, 2, 1, 'd')
    c.rect(17, 29, 2, 1, 'd')
    # rider centered on the horse's back, no separate legs
    c.disc(10, 8, 2, 's')
    c.rect(8, 11, 5, 6, 'm')
    c.rect(8, 6, 5, 3, 'm')  # helmet
    # lance raised up-right over the horse's neck
    c.line(13, 5, 27, 13, 'w')
    c.outline()
    return c, p


def catapult():
    c = Canvas()
    p = {'w': '#8b5a2b', 'd': '#5a3a1a', 'k': '#282828', 'g': '#909090'}
    # base plank
    c.rect(4, 23, 24, 3, 'w')
    # solid wheels (no hollow center, so it doesn't read as eyeglasses)
    c.disc(9, 27, 3, 'k')
    c.disc(23, 27, 3, 'k')
    # A-frame support posts rising from the base to the pivot
    c.line(7, 23, 16, 9, 'd')
    c.line(25, 23, 16, 9, 'd')
    c.hline(11, 21, 15, 'd')  # crossbar brace
    # throwing arm pivoted at the apex, swung up-left, loaded with a stone
    c.line(16, 11, 6, 3, 'd')
    c.line(17, 12, 7, 4, 'd')
    c.rect(3, 1, 5, 4, 'w')  # sling basket
    c.disc(5, 2, 2, 'g')     # stone payload
    c.outline()
    return c, p


def champion():
    c = Canvas()
    p = {'s': '#c0a080', 'm': '#d8d8e0', 'd': '#9098a8', 'r': '#a01818',
         'g': '#ffd700', 'w': '#f0f0f0'}
    # crimson cape behind, drawn first so torso overwrites the center
    c.rect(9, 10, 12, 11, 'r')
    humanoid(c, skin='s', torso='m', legs='d')
    # plumed great helm
    c.rect(12, 3, 7, 4, 'm')
    c.hline(13, 17, 2, 'm')
    c.hline(13, 17, 5, 'd')
    c.vline(15, 0, 3, 'r')  # plume
    c.px(14, 0, 'r')
    c.px(16, 0, 'r')
    # gold belt trim
    c.hline(13, 17, 15, 'g')
    # raised broadsword on right
    c.vline(23, 3, 13, 'w')
    c.hline(21, 25, 12, 'd')
    c.px(23, 14, 's')
    c.outline()
    return c, p


def paladin():
    c = Canvas()
    p = {'s': '#c0a080', 'w': '#f2f2f2', 'g': '#ffd700', 'b': '#3050a0'}
    humanoid(c, skin='s', torso='w', legs='w')
    # halo bar above head
    c.hline(12, 18, 2, 'g')
    # gold trim on chest
    c.hline(13, 17, 11, 'g')
    c.hline(13, 17, 15, 'g')
    # kite shield with cross, left arm
    c.rect(6, 10, 4, 7, 'w')
    c.vline(8, 11, 15, 'b')
    c.hline(6, 9, 13, 'b')
    # mace on right
    c.vline(23, 6, 14, 'g')
    c.disc(23, 5, 2, 'g')
    c.outline()
    return c, p


def fire_elemental():
    c = Canvas()
    p = {'r': '#e02d00', 'o': '#ff8c00', 'y': '#ffd700', 'k': '#8b0000'}
    # outer flame body
    c.triangle(15, 3, 26, 10, 'r')
    # side licks
    c.triangle(7, 12, 20, 4, 'r')
    c.triangle(23, 12, 20, 4, 'r')
    # inner flame
    c.triangle(15, 8, 25, 6, 'o')
    # glowing core
    c.disc(15, 17, 4, 'y')
    # dark base embers
    c.hline(9, 21, 26, 'k')
    # eyes
    c.px(13, 15, 'k')
    c.px(17, 15, 'k')
    c.outline()
    return c, p


def ice_elemental():
    c = Canvas()
    p = {'c': '#3ac6ff', 'w': '#eaffff', 'b': '#0a4a80'}
    # diamond crystalline body: upper half + lower half
    c.triangle(15, 3, 16, 9, 'c')
    c.triangle(15, 27, 16, 9, 'c')
    # angular shard highlights
    c.line(15, 6, 9, 16, 'w')
    c.line(15, 6, 21, 16, 'w')
    c.line(15, 24, 10, 17, 'b')
    c.line(15, 24, 20, 17, 'b')
    # icy core
    c.disc(15, 16, 3, 'w')
    # eyes
    c.px(13, 14, 'b')
    c.px(17, 14, 'b')
    c.outline()
    return c, p


def sage():
    c = Canvas()
    p = {'s': '#c0a080', 't': '#2f6f6f', 'w': '#f5f5f5', 'b': '#5a3a1a'}
    humanoid(c, skin='s', torso='t', legs='t')
    # simple pointed hood/hat
    c.triangle(15, 1, 5, 4, 't')
    # long white beard
    c.rect(13, 8, 4, 3, 'w')
    # gnarled staff with glowing orb
    c.vline(24, 6, 20, 'b')
    c.disc(24, 5, 2, 'w')
    # book held in other hand
    c.rect(7, 14, 3, 3, 'w')
    c.outline()
    return c, p


def archmage():
    c = Canvas()
    p = {'s': '#c0a080', 'v': '#8a2be2', 'g': '#ffd700', 'c': '#00ffff'}
    humanoid(c, skin='s', torso='v', legs='v')
    # ornate wide hat with gold band
    c.triangle(15, 0, 6, 6, 'v')
    c.hline(10, 20, 6, 'g')
    # gold trim on robe
    c.hline(12, 18, 13, 'g')
    c.hline(12, 18, 16, 'g')
    # crystal-topped staff
    c.vline(23, 4, 18, 'g')
    c.disc(23, 3, 2, 'c')
    c.outline()
    return c, p


def lich():
    c = Canvas()
    p = {'g': '#a8dca8', 'k': '#181818', 'y': '#e8ff40', 'w': '#c8c8c8'}
    humanoid(c, skin='g', torso='k', legs='k')
    # tattered robe hem flare over legs
    c.triangle(15, 20, 26, 9, 'k')
    # hollow glowing eyes
    c.px(13, 7, 'y')
    c.px(17, 7, 'y')
    # bone staff with skull top
    c.vline(23, 6, 20, 'w')
    c.disc(23, 5, 2, 'w')
    c.px(22, 5, 'k')
    c.px(24, 5, 'k')
    c.outline()
    return c, p


def dragon_knight():
    c = Canvas()
    p = {'s': '#c0a080', 'r': '#a83232', 'o': '#e08020', 'd': '#701818'}
    # wings behind, drawn first so torso overwrites the center
    c.triangle(6, 9, 17, 6, 'o')
    c.triangle(24, 9, 17, 6, 'o')
    humanoid(c, skin='s', torso='r', legs='d')
    # dragon-helm with horns
    c.rect(12, 3, 7, 4, 'r')
    c.hline(13, 17, 5, 'd')
    c.line(12, 3, 9, 0, 'o')
    c.line(18, 3, 21, 0, 'o')
    # tail
    c.line(15, 21, 9, 27, 'd')
    c.px(8, 27, 'd')
    c.outline()
    return c, p


def siege_titan():
    c = Canvas()
    p = {'g': '#707078', 'd': '#404048', 'o': '#ff8000'}
    # blocky head
    c.rect(11, 2, 10, 6, 'g')
    c.px(14, 5, 'o')
    c.px(18, 5, 'o')
    # massive torso
    c.rect(6, 9, 20, 13, 'g')
    c.rect(9, 12, 14, 6, 'd')
    c.disc(15, 15, 3, 'o')
    # legs
    c.rect(6, 23, 8, 7, 'd')
    c.rect(18, 23, 8, 7, 'd')
    # left arm stub
    c.rect(2, 10, 5, 11, 'g')
    # right arm: giant hammer
    c.rect(24, 9, 5, 11, 'g')
    c.rect(27, 4, 5, 7, 'd')
    c.outline()
    return c, p


def treant():
    c = Canvas()
    p = {'b': '#6b4a2b', 'g': '#3f7f3f', 'd': '#4a3218', 'e': '#ffcc00'}
    # leafy canopy
    c.disc(15, 6, 7, 'g')
    # trunk / torso with bark face
    c.rect(10, 11, 10, 11, 'b')
    c.px(13, 15, 'e')
    c.px(17, 15, 'e')
    c.hline(13, 17, 18, 'd')  # bark grain / mouth
    # branch arms
    c.line(10, 13, 3, 8, 'd')
    c.line(20, 13, 27, 8, 'd')
    # gnarled roots / legs
    c.rect(9, 22, 4, 6, 'd')
    c.rect(19, 22, 4, 6, 'd')
    c.line(9, 27, 5, 30, 'd')
    c.line(23, 27, 27, 30, 'd')
    c.outline()
    return c, p


def summoned_demon():
    c = Canvas()
    p = {'r': '#8b1a4a', 'k': '#1a1a1a', 'o': '#ff6a00', 'w': '#c8a8ff'}
    # wings behind, drawn first
    c.triangle(6, 11, 21, 5, 'k')
    c.triangle(24, 11, 21, 5, 'k')
    # head with horns
    c.disc(15, 8, 3, 'r')
    c.line(13, 6, 10, 2, 'k')
    c.line(17, 6, 20, 2, 'k')
    c.px(13, 8, 'o')
    c.px(17, 8, 'o')
    # bulky torso
    c.rect(10, 12, 10, 9, 'r')
    # legs
    c.rect(10, 21, 4, 7, 'r')
    c.rect(16, 21, 4, 7, 'r')
    # clawed arms
    c.line(10, 13, 4, 20, 'r')
    c.line(20, 13, 26, 20, 'r')
    # binding chains on wrists (flavor of the Binding Circle)
    c.px(4, 20, 'w')
    c.px(26, 20, 'w')
    c.px(5, 21, 'w')
    c.px(25, 21, 'w')
    c.outline()
    return c, p


def golem():
    c = Canvas()
    p = {'g': '#8a8a8a', 'd': '#5a5a5a', 'y': '#e0c040'}
    # blocky head
    c.rect(10, 3, 10, 6, 'g')
    c.rect(12, 6, 2, 1, 'd')
    c.rect(17, 6, 2, 1, 'd')
    # massive torso block
    c.rect(6, 10, 20, 12, 'g')
    # glowing rune core
    c.rect(13, 14, 5, 5, 'y')
    # cracks
    c.line(8, 12, 10, 18, 'd')
    c.line(22, 12, 20, 18, 'd')
    # blocky arms
    c.rect(2, 11, 4, 10, 'g')
    c.rect(26, 11, 4, 10, 'g')
    # blocky legs
    c.rect(8, 22, 6, 7, 'g')
    c.rect(18, 22, 6, 7, 'g')
    c.outline()
    return c, p


def titan():
    c = Canvas()
    p = {'t': '#c8945a', 'd': '#8a5a2a', 'w': '#f0e0c0', 'k': '#4a3018'}
    # big head + hair
    c.disc(15, 5, 4, 't')
    c.rect(11, 1, 8, 3, 'k')
    # massive torso spanning most of the canvas width
    c.rect(4, 9, 23, 13, 't')
    # loincloth band
    c.rect(9, 19, 13, 3, 'd')
    # ab/chest highlight shading
    c.hline(9, 22, 13, 'w')
    # huge arms with fists
    c.rect(0, 10, 4, 10, 't')
    c.disc(2, 21, 3, 't')
    c.rect(28, 10, 4, 10, 't')
    c.disc(29, 21, 3, 't')
    # thick legs
    c.rect(6, 22, 8, 8, 't')
    c.rect(18, 22, 8, 8, 't')
    c.outline()
    return c, p


def demon():
    c = Canvas()
    p = {'r': '#5a0a0a', 'k': '#0d0d0d', 'o': '#ffcc00', 'm': '#8a1010'}
    # small leathery wings
    c.triangle(6, 13, 22, 5, 'k')
    c.triangle(24, 13, 22, 5, 'k')
    # head with curling ram horns
    c.disc(15, 7, 4, 'r')
    c.line(12, 6, 7, 3, 'k')
    c.px(6, 5, 'k')
    c.line(18, 6, 23, 3, 'k')
    c.px(24, 5, 'k')
    c.px(13, 7, 'o')
    c.px(17, 7, 'o')
    # broad muscular torso
    c.rect(8, 11, 14, 11, 'r')
    c.hline(11, 18, 13, 'm')
    c.hline(11, 18, 17, 'm')
    # legs
    c.rect(9, 22, 5, 7, 'r')
    c.rect(18, 22, 5, 7, 'r')
    # clawed arms
    c.line(8, 13, 2, 20, 'r')
    c.line(22, 13, 28, 20, 'r')
    c.outline()
    return c, p


def city_garrison():
    c = Canvas()
    p = {'s': '#c0a080', 'u': '#2a4a7a', 'm': '#909090', 'w': '#d8d8d8'}
    humanoid(c, skin='s', torso='u', legs='m')
    # kettle helm
    c.rect(12, 3, 7, 3, 'm')
    c.hline(12, 18, 6, 'm')
    # round shield on left arm, with a single rivet boss (not a cross,
    # to keep it visually distinct from the paladin/cleric holy symbols)
    c.disc(8, 13, 3, 'm')
    c.px(8, 13, 'w')
    # spear on right, held upright
    c.vline(24, 2, 21, 'w')
    c.hline(22, 26, 5, 'm')
    c.outline()
    return c, p


SPRITES = {
    'knight': knight,
    'cavalry': cavalry,
    'catapult': catapult,
    'champion': champion,
    'paladin': paladin,
    'fire_elemental': fire_elemental,
    'ice_elemental': ice_elemental,
    'sage': sage,
    'archmage': archmage,
    'lich': lich,
    'dragon_knight': dragon_knight,
    'siege_titan': siege_titan,
    'treant': treant,
    'summoned_demon': summoned_demon,
    'golem': golem,
    'titan': titan,
    'demon': demon,
    'city_garrison': city_garrison,
}
