"""Building sprite definitions. Each builder returns (Canvas, palette)."""

from sprite_lib import Canvas


# -- shared helpers ---------------------------------------------------------

def _plinth(c, col, x=3, y=25, w=26, h=3):
    c.rect(x, y, w, h, col)


def _wall(c, x, y, w, h, col):
    c.rect(x, y, w, h, col)


def _gable(c, cx, apex_y, base_y, half_w, col):
    c.triangle(cx, apex_y, base_y, half_w, col)


def _door(c, cx, base_y, w, h, col):
    c.rect(cx - w // 2, base_y - h, w, h, col)


def _window(c, x, y, col, w=2, h=2):
    c.rect(x, y, w, h, col)


def _crenellate(c, x0, x1, y, col, tooth=2, gap=2):
    x = x0
    while x <= x1:
        c.rect(x, y - 2, tooth, 2, col)
        x += tooth + gap


def _tower(c, cx, base_y, width, height, wall, cap_col, cap='cone', cap_h=6):
    x0 = cx - width // 2
    y0 = base_y - height
    c.rect(x0, y0, width, height, wall)
    if cap == 'cone':
        c.triangle(cx, y0 - cap_h, y0, width // 2 + 1, cap_col)
    elif cap == 'flat':
        c.rect(x0 - 1, y0 - 2, width + 2, 2, cap_col)
    return x0, y0


def _spike(c, cx, apex_y, base_y, half_w, col):
    c.triangle(cx, apex_y, base_y, half_w, col)


def _glow(c, cx, cy, r, col):
    c.ring(cx, cy, r, col)


# -- Tier 1 ------------------------------------------------------------------

def barracks():
    c = Canvas()
    p = {'w': '#8b5e3c', 'r': '#8c2020', 'd': '#4a2f1a', 'g': '#5a7a3a',
         'f': '#c94040', 'k': '#3a3a3a'}
    _plinth(c, 'g')
    _wall(c, 7, 14, 18, 12, 'w')
    _gable(c, 16, 6, 14, 11, 'r')
    _door(c, 16, 26, 4, 7, 'd')
    _window(c, 10, 17, 'd')
    _window(c, 20, 17, 'd')
    c.vline(9, 3, 14, 'k')
    c.rect(9, 3, 4, 3, 'f')
    c.outline()
    return c, p


def market():
    c = Canvas()
    p = {'w': '#6b4a2f', 'a': '#c9973a', 'b': '#e8c060', 'd': '#3a2818',
         'g': '#5a7a3a'}
    _plinth(c, 'g')
    # counter
    c.rect(6, 20, 20, 6, 'w')
    c.rect(6, 20, 20, 2, 'd')
    # posts
    c.vline(7, 8, 20, 'w')
    c.vline(24, 8, 20, 'w')
    # striped awning
    for i, x in enumerate(range(5, 27, 2)):
        c.rect(x, 8, 2, 4, 'a' if i % 2 == 0 else 'b')
    c.triangle(16, 4, 8, 12, 'a')
    c.rect(5, 8, 22, 2, 'd')
    # baskets on counter
    c.rect(9, 17, 3, 3, 'a')
    c.rect(19, 17, 3, 3, 'b')
    c.outline()
    return c, p


def temple():
    c = Canvas()
    p = {'w': '#e8e4d8', 'r': '#3a5aa0', 'd': '#2a4070', 'g': '#8898a8',
         's': '#e0c840'}
    _plinth(c, 'g')
    _wall(c, 8, 15, 16, 11, 'w')
    _gable(c, 16, 7, 15, 10, 'r')
    c.rect(15, 3, 2, 4, 'w')
    c.disc(16, 3, 2, 's')
    _door(c, 16, 26, 5, 8, 'd')
    c.rect(10, 12, 2, 2, 's')
    c.rect(20, 12, 2, 2, 's')
    c.vline(9, 15, 25, 'w')
    c.vline(23, 15, 25, 'w')
    c.outline()
    return c, p


def granary():
    c = Canvas()
    p = {'w': '#c9a24a', 'r': '#8a5a2a', 'd': '#6b4423', 'g': '#5a7a3a',
         's': '#e0c040'}
    _plinth(c, 'g')
    c.rect(9, 12, 14, 14, 'w')
    for y in range(13, 25, 2):
        c.hline(9, 22, y, 'd')
    c.triangle(16, 4, 12, 11, 'r')
    _door(c, 16, 26, 4, 6, 'd')
    # wheat sheaf symbol
    c.line(16, 8, 13, 5, 's')
    c.line(16, 8, 16, 4, 's')
    c.line(16, 8, 19, 5, 's')
    c.outline()
    return c, p


def walls():
    c = Canvas()
    p = {'w': '#9a9a92', 'd': '#767068', 'g': '#5a7a3a'}
    _plinth(c, 'g')
    c.rect(3, 15, 26, 11, 'w')
    for y in range(16, 25, 4):
        c.hline(3, 28, y, 'd')
    for x in range(3, 29, 6):
        c.vline(x, 15, 25, 'd')
    _crenellate(c, 3, 27, 15, 'w')
    c.outline()
    return c, p


# -- Tier 2 --------------------------------------------------------------

def archery_range():
    c = Canvas()
    p = {'w': '#8b5e3c', 'r': '#6b4423', 'g': '#5a7a3a', 't': '#c9c0a0',
         'a': '#c94040', 'k': '#3a3a3a'}
    _plinth(c, 'g')
    # small shed to the left
    _wall(c, 4, 18, 8, 8, 'w')
    _gable(c, 8, 13, 18, 5, 'r')
    # target on the right
    c.disc(22, 15, 7, 't')
    c.disc(22, 15, 5, 'a')
    c.disc(22, 15, 3, 't')
    c.disc(22, 15, 1, 'a')
    c.vline(22, 22, 25, 'k')
    # arrows stuck in target
    c.line(15, 18, 20, 15, 'k')
    c.line(15, 10, 19, 13, 'k')
    c.outline()
    return c, p


def war_academy():
    c = Canvas()
    p = {'w': '#8a8a90', 'r': '#2a3a6a', 'd': '#5a5a60', 'g': '#5a7a3a',
         'f': '#d0b030', 'k': '#3a3a3a'}
    _plinth(c, 'g')
    _tower(c, 8, 25, 7, 14, 'w', 'r', cap='cone', cap_h=5)
    _tower(c, 24, 25, 7, 14, 'w', 'r', cap='cone', cap_h=5)
    _wall(c, 11, 16, 10, 9, 'w')
    _gable(c, 16, 9, 16, 6, 'r')
    _door(c, 16, 25, 4, 6, 'd')
    c.vline(8, 6, 11, 'k')
    c.rect(8, 6, 4, 3, 'f')
    c.vline(24, 6, 11, 'k')
    c.rect(24, 6, 4, 3, 'f')
    c.outline()
    return c, p


def mage_tower():
    c = Canvas()
    p = {'w': '#7060a0', 'r': '#4a3a80', 'g': '#5a7a3a', 'l': '#60e0ff',
         'd': '#3a2a60'}
    _tower(c, 16, 25, 12, 20, 'w', 'r', cap='cone', cap_h=8)
    _plinth(c, 'g')
    for y in (10, 15, 20):
        c.rect(13, y, 6, 3, 'd')
        c.rect(14, y + 1, 4, 1, 'l')
    c.disc(16, 5, 2, 'l')
    c.ring(16, 5, 3, 'l')
    c.outline()
    return c, p


def stables():
    c = Canvas()
    p = {'w': '#8b5e3c', 'r': '#5a3a1a', 'g': '#5a7a3a', 'd': '#3a2818',
         'h': '#c9a24a', 'm': '#e0b840'}
    _plinth(c, 'g')
    _wall(c, 6, 17, 20, 9, 'w')
    _gable(c, 16, 10, 17, 13, 'r')
    _door(c, 16, 26, 8, 7, 'd')
    c.rect(9, 21, 2, 2, 'w')
    c.rect(21, 21, 2, 2, 'w')
    # horseshoe over the door (ring with the bottom arc cleared open)
    c.ring(16, 17, 3, 'h')
    c.rect(14, 19, 4, 2, 'w')
    # straw hay bale, bound with dark ties
    c.rect(3, 20, 5, 5, 'm')
    c.hline(3, 7, 22, 'd')
    c.hline(3, 7, 24, 'd')
    c.outline()
    return c, p


def trade_hall():
    c = Canvas()
    p = {'w': '#a9773f', 'r': '#7a3a2a', 'g': '#5a7a3a', 'd': '#4a2f1a',
         'b': '#c9973a', 's': '#e0c840'}
    _plinth(c, 'g')
    _wall(c, 6, 13, 20, 13, 'w')
    _gable(c, 16, 6, 13, 12, 'r')
    _door(c, 16, 26, 6, 9, 'd')
    _window(c, 8, 16, 'd', 3, 3)
    _window(c, 21, 16, 'd', 3, 3)
    # hanging sign, bracketed off the eave
    c.hline(11, 21, 14, 'd')
    c.rect(12, 14, 8, 3, 's')
    # barrels outside
    c.rect(3, 20, 3, 5, 'b')
    c.rect(26, 20, 3, 5, 'b')
    c.outline()
    return c, p


def fortifications():
    c = Canvas()
    p = {'w': '#8a8a82', 'd': '#666058', 'g': '#5a7a3a'}
    _plinth(c, 'g')
    c.rect(3, 16, 26, 10, 'w')
    for y in range(17, 25, 4):
        c.hline(3, 28, y, 'd')
    for x in range(3, 29, 6):
        c.vline(x, 16, 25, 'd')
    _crenellate(c, 3, 27, 16, 'w')
    # corner tower
    _tower(c, 24, 26, 9, 18, 'w', 'd', cap='flat')
    _crenellate(c, 20, 28, 8, 'w')
    c.outline()
    return c, p


def siege_workshop():
    c = Canvas()
    p = {'w': '#8b5e3c', 'r': '#6b4423', 'g': '#5a7a3a', 'd': '#3a2818',
         'k': '#3a3a3a', 'b': '#a9773f'}
    _plinth(c, 'g')
    _wall(c, 5, 18, 13, 8, 'w')
    _gable(c, 11, 12, 18, 8, 'r')
    _door(c, 11, 26, 4, 6, 'd')
    # catapult arm + wheel
    c.disc(24, 23, 4, 'b')
    c.disc(24, 23, 2, 'k')
    c.line(21, 22, 27, 10, 'k')
    c.rect(26, 9, 3, 2, 'd')
    c.vline(21, 19, 25, 'k')
    c.vline(27, 19, 25, 'k')
    c.outline()
    return c, p


# -- Tier 3 ----------------------------------------------------------------

def elite_barracks():
    c = Canvas()
    p = {'w': '#7a5030', 'r': '#6a1818', 'd': '#3a2414', 'g': '#5a7a3a',
         'f': '#8a2040', 's': '#d0b030', 'k': '#3a3a3a'}
    _plinth(c, 'g')
    _wall(c, 6, 13, 20, 13, 'w')
    _gable(c, 16, 5, 13, 12, 'r')
    _door(c, 16, 26, 5, 9, 'd')
    _window(c, 9, 16, 'd')
    _window(c, 21, 16, 'd')
    c.rect(15, 2, 2, 3, 's')
    c.vline(9, 3, 13, 'k')
    c.rect(8, 3, 6, 6, 'f')
    c.rect(9, 8, 4, 2, 's')
    c.outline()
    return c, p


def arcane_sanctum():
    c = Canvas()
    p = {'w': '#6a4aa0', 'r': '#4a2a80', 'g': '#5a7a3a', 'l': '#a060ff',
         'd': '#301a60', 's': '#c090ff'}
    _plinth(c, 'g')
    # two low flanking turrets, well clear of the central tower
    _tower(c, 6, 27, 6, 9, 'w', 'r', cap='cone', cap_h=4)
    _tower(c, 26, 27, 6, 9, 'w', 'r', cap='cone', cap_h=4)
    c.disc(6, 15, 1, 'l')
    c.disc(26, 15, 1, 'l')
    # tall central spire
    _tower(c, 16, 26, 10, 18, 'w', 'r', cap='cone', cap_h=7)
    for y in (12, 16, 20):
        c.rect(14, y, 4, 3, 'd')
        c.rect(15, y + 1, 2, 1, 'l')
    c.disc(16, 3, 3, 'l')
    c.ring(16, 3, 4, 's')
    c.outline()
    return c, p


def cathedral():
    c = Canvas()
    p = {'w': '#eeeae0', 'r': '#2a4888', 'd': '#1a2f60', 'g': '#8898a8',
         's': '#e8d040'}
    _plinth(c, 'g')
    _wall(c, 7, 14, 18, 12, 'w')
    _gable(c, 16, 5, 14, 11, 'r')
    _tower(c, 8, 26, 5, 16, 'w', 'r', cap='cone', cap_h=5)
    _tower(c, 24, 26, 5, 16, 'w', 'r', cap='cone', cap_h=5)
    c.rect(15, 1, 2, 4, 'w')
    c.disc(16, 1, 2, 's')
    # rose window
    c.ring(16, 15, 3, 's')
    c.disc(16, 15, 1, 'd')
    _door(c, 16, 26, 5, 7, 'd')
    c.outline()
    return c, p


def treasury():
    c = Canvas()
    p = {'w': '#8a8478', 'd': '#4a4438', 'g': '#5a7a3a', 's': '#e8c840',
         'k': '#3a3a3a'}
    _plinth(c, 'g')
    _wall(c, 7, 13, 18, 13, 'w')
    c.rect(6, 12, 20, 2, 'd')
    for x in range(9, 24, 4):
        c.vline(x, 13, 25, 'd')
    # vault door
    c.disc(16, 20, 5, 's')
    c.disc(16, 20, 5, 'd')
    c.ring(16, 20, 3, 's')
    c.disc(16, 20, 1, 'k')
    # coin pile beside
    c.disc(9, 24, 2, 's')
    c.disc(23, 24, 2, 's')
    c.outline()
    return c, p


def grand_walls():
    c = Canvas()
    p = {'w': '#9a9a92', 'd': '#767068', 'g': '#5a7a3a', 's': '#e0c040'}
    _plinth(c, 'g')
    c.rect(3, 12, 26, 14, 'w')
    for y in range(13, 25, 4):
        c.hline(3, 28, y, 'd')
    for x in range(3, 29, 6):
        c.vline(x, 12, 25, 'd')
    _crenellate(c, 3, 27, 12, 'w')
    c.hline(3, 28, 12, 's')
    _tower(c, 6, 26, 7, 19, 'w', 's', cap='flat')
    _tower(c, 26, 26, 7, 19, 'w', 's', cap='flat')
    _crenellate(c, 3, 9, 7, 'w')
    _crenellate(c, 23, 29, 7, 'w')
    c.outline()
    return c, p


def masons_guild():
    c = Canvas()
    p = {'w': '#a89878', 'd': '#6a5a3a', 'g': '#5a7a3a', 'k': '#7a5a2a',
         'b': '#c9c0a0', 't': '#e8c840', 'h': '#8a7a60'}
    _plinth(c, 'g')
    _wall(c, 8, 15, 16, 10, 'w')
    _gable(c, 16, 8, 15, 10, 'd')
    _door(c, 16, 25, 5, 7, 'd')
    # timber scaffold: side poles, plank platform, short corner braces
    c.vline(5, 9, 25, 'k')
    c.vline(26, 9, 25, 'k')
    c.hline(5, 26, 12, 'k')
    c.line(5, 20, 9, 12, 'k')
    c.line(22, 20, 26, 12, 'k')
    c.hline(4, 27, 20, 'h')
    # cut stone block pile out front
    c.rect(10, 22, 5, 4, 'b')
    c.rect(17, 21, 5, 5, 'b')
    for x in (11, 18, 23):
        c.vline(x, 21, 26, 'd')
    # trowel leaning on the blocks
    c.line(24, 12, 27, 20, 't')
    c.triangle(24, 10, 13, 3, 't')
    c.outline()
    return c, p


# -- Tier 4 / special --------------------------------------------------------

def legendary_forge():
    c = Canvas()
    p = {'w': '#4a4038', 'd': '#2a241e', 'g': '#5a5048', 'f': '#ff8a30',
         's': '#e8c840', 'k': '#232120', 'h': '#8a6a3a'}
    _plinth(c, 'g')
    _wall(c, 9, 13, 16, 13, 'w')
    _gable(c, 17, 6, 13, 9, 'd')
    c.hline(9, 24, 13, 's')
    # chimney with roaring flame
    c.rect(22, 3, 4, 11, 'd')
    c.disc(24, 3, 3, 'f')
    c.disc(24, 2, 1, 's')
    _door(c, 15, 26, 5, 7, 'k')
    c.rect(18, 16, 3, 3, 'f')
    # anvil out front, glowing hot
    c.rect(4, 22, 7, 3, 'k')
    c.rect(3, 20, 4, 2, 'k')
    c.rect(6, 25, 1, 2, 'k')
    c.rect(9, 25, 1, 2, 'k')
    c.disc(7, 21, 1, 'f')
    # hammer leaning on the anvil
    c.line(5, 10, 9, 20, 'h')
    c.rect(4, 8, 4, 3, 'k')
    c.outline()
    return c, p


def signal_towers():
    c = Canvas()
    p = {'w': '#8a8478', 'd': '#5a5448', 'g': '#5a7a3a', 'f': '#ff9a30',
         's': '#ffd060', 'k': '#3a3a3a'}
    _plinth(c, 'g', x=10, w=12)
    _tower(c, 16, 26, 8, 20, 'w', 'd', cap='flat')
    for y in (10, 15, 20):
        c.hline(13, 18, y, 'd')
    # brazier beacon on top
    c.rect(14, 4, 4, 2, 'k')
    c.disc(16, 2, 3, 'f')
    c.disc(16, 2, 1, 's')
    c.vline(20, 3, 8, 'k')
    c.rect(20, 3, 4, 3, 's')
    c.outline()
    return c, p


def ritual_site():
    c = Canvas()
    p = {'w': '#5a5058', 'g': '#3a3040', 'l': '#8a40e0', 'd': '#2a2030',
         's': '#40c060'}
    _plinth(c, 'g')
    # standing stones (henge)
    for x in (6, 12, 20, 26):
        c.rect(x, 12, 3, 13, 'w')
    c.hline(6, 28, 12, 'w')
    # altar
    c.rect(13, 20, 6, 5, 'd')
    c.disc(16, 18, 3, 'l')
    c.ring(16, 18, 5, 's')
    c.outline()
    return c, p


# -- Exploitation buildings --------------------------------------------------

def mine():
    c = Canvas()
    p = {'r': '#8a7a5a', 'd': '#5a4a30', 'k': '#1a1a1a', 'w': '#6b4423',
         'o': '#c9973a'}
    c.triangle(16, 6, 24, 15, 'r')
    c.rect(2, 20, 28, 6, 'r')
    for y in range(21, 26, 2):
        c.hline(2, 29, y, 'd')
    # cave mouth
    c.rect(12, 15, 8, 11, 'k')
    c.triangle(16, 12, 15, 4, 'k')
    c.vline(11, 13, 26, 'w')
    c.vline(20, 13, 26, 'w')
    c.hline(11, 20, 13, 'w')
    # minecart
    c.rect(22, 22, 5, 3, 'o')
    c.disc(23, 26, 1, 'd')
    c.disc(26, 26, 1, 'd')
    c.outline()
    return c, p


def deep_mine():
    c = Canvas()
    p = {'r': '#5a5258', 'd': '#38343a', 'k': '#101014', 'w': '#4a3a2a',
         'l': '#60d0e0'}
    c.triangle(16, 4, 24, 16, 'r')
    c.rect(2, 20, 28, 6, 'r')
    for y in range(21, 26, 2):
        c.hline(2, 29, y, 'd')
    c.rect(11, 14, 10, 12, 'k')
    c.triangle(16, 10, 14, 5, 'k')
    c.vline(10, 12, 26, 'w')
    c.vline(21, 12, 26, 'w')
    c.hline(10, 21, 12, 'w')
    # crystals jutting from the mound
    _spike(c, 6, 8, 18, 2, 'l')
    _spike(c, 25, 6, 17, 2, 'l')
    _spike(c, 16, 15, 22, 1, 'l')
    c.outline()
    return c, p


def mana_well():
    c = Canvas()
    p = {'w': '#8a8ea0', 'd': '#5a5e70', 'g': '#5a7a3a', 'l': '#40b0ff',
         's': '#a0e0ff'}
    _plinth(c, 'g')
    c.rect(7, 19, 18, 7, 'w')
    c.rect(7, 18, 18, 2, 'd')
    c.disc(16, 20, 4, 'l')
    c.ring(16, 20, 5, 's')
    c.disc(16, 8, 2, 's')
    c.vline(16, 10, 17, 'l')
    c.ring(16, 20, 8, 'l')
    c.outline()
    return c, p


def archive():
    c = Canvas()
    p = {'w': '#c9c0a0', 'd': '#8a7a50', 'g': '#8898a8', 's': '#4a6aa0',
         'k': '#5a4a30'}
    _plinth(c, 'g')
    _wall(c, 7, 15, 18, 11, 'w')
    c.rect(6, 13, 20, 2, 'd')
    for x in (9, 13, 19, 23):
        c.vline(x, 15, 25, 'd')
    _door(c, 16, 26, 5, 8, 's')
    # book/scroll symbol above door
    c.rect(13, 9, 6, 3, 's')
    c.hline(13, 19, 10, 'k')
    c.outline()
    return c, p


def crystal_sanctum():
    c = Canvas()
    p = {'w': '#7a8aa0', 'd': '#4a5a70', 'g': '#5a7a3a', 'l': '#70e0ff',
         's': '#c0f0ff'}
    _plinth(c, 'g')
    c.rect(11, 20, 10, 6, 'd')
    _spike(c, 16, 3, 22, 5, 'l')
    _spike(c, 9, 12, 22, 2, 'l')
    _spike(c, 23, 10, 22, 2, 'l')
    c.line(16, 3, 16, 3, 's')
    c.ring(16, 14, 4, 's')
    c.outline()
    return c, p


def necropolis():
    c = Canvas()
    p = {'w': '#5a5a52', 'd': '#302f2a', 'g': '#4a4038', 'l': '#40c070',
         'k': '#1a1a1a'}
    _plinth(c, 'g')
    _wall(c, 9, 16, 14, 10, 'w')
    _gable(c, 16, 10, 16, 8, 'd')
    c.rect(11, 5, 3, 8, 'w')
    c.rect(18, 5, 3, 8, 'w')
    _door(c, 16, 26, 5, 7, 'k')
    c.disc(16, 26, 2, 'l')
    c.ring(16, 26, 4, 'l')
    # small skull motif
    c.disc(16, 12, 2, 'w')
    c.px(15, 12, 'k')
    c.px(17, 12, 'k')
    c.outline()
    return c, p


def dragon_shrine():
    c = Canvas()
    p = {'w': '#8a7050', 'd': '#5a4030', 'g': '#4a4038', 'f': '#ff7a20',
         'b': '#e0d8c0', 'k': '#1a1210'}
    _plinth(c, 'g')
    # stone altar block
    c.rect(9, 21, 14, 5, 'd')
    # bone-colored dragon skull resting on the altar
    c.disc(16, 13, 6, 'b')
    c.rect(11, 15, 10, 4, 'b')
    # curved horns
    _spike(c, 9, 4, 10, 2, 'b')
    _spike(c, 23, 4, 10, 2, 'b')
    # eye sockets glowing with shrine-fire
    c.disc(13, 12, 1, 'f')
    c.disc(19, 12, 1, 'f')
    # jaw / teeth
    for x in range(12, 21, 2):
        c.vline(x, 18, 19, 'k')
    # brazier flame in front of the altar
    c.rect(14, 20, 4, 2, 'k')
    c.disc(16, 19, 2, 'f')
    c.outline()
    return c, p


def master_forge():
    c = Canvas()
    p = {'w': '#5a4a42', 'd': '#332a24', 'g': '#4a4038', 'f': '#ff9020',
         's': '#60c0ff', 'k': '#1a1a1a'}
    _plinth(c, 'g')
    _wall(c, 5, 14, 22, 12, 'w')
    _gable(c, 16, 7, 14, 12, 'd')
    c.rect(21, 3, 5, 11, 'd')
    c.disc(23, 3, 2, 'f')
    _door(c, 12, 26, 6, 8, 'k')
    c.rect(10, 21, 2, 2, 'f')
    c.rect(13, 21, 2, 2, 'f')
    # anvil with adamantine glow
    c.rect(19, 21, 6, 3, 'k')
    c.rect(20, 19, 4, 2, 'k')
    c.disc(22, 18, 1, 's')
    c.outline()
    return c, p


def grove_of_ages():
    c = Canvas()
    p = {'k': '#5a3a20', 'd': '#3a2410', 'g': '#4a6a2a', 'l': '#60d060',
         'b': '#2a5020'}
    _plinth(c, 'g')
    c.rect(13, 14, 6, 12, 'k')
    c.rect(9, 20, 4, 6, 'd')
    c.rect(19, 20, 4, 6, 'd')
    c.disc(16, 10, 9, 'b')
    c.disc(9, 13, 5, 'b')
    c.disc(23, 13, 5, 'b')
    c.disc(12, 8, 2, 'l')
    c.disc(20, 9, 2, 'l')
    c.disc(16, 6, 2, 'l')
    c.disc(22, 15, 1, 'l')
    c.disc(10, 16, 1, 'l')
    c.outline()
    return c, p


def binding_circle():
    c = Canvas()
    p = {'k': '#2a1420', 'd': '#4a1a30', 'l': '#c02040', 's': '#8020c0',
         'g': '#241018', 'h': '#1a1418'}
    _plinth(c, 'g')
    # twin stone pillars forming a gate, spiked tops, chained lintel
    c.rect(7, 8, 4, 18, 'd')
    c.rect(21, 8, 4, 18, 'd')
    _spike(c, 9, 3, 8, 2, 'k')
    _spike(c, 23, 3, 8, 2, 'k')
    c.hline(9, 23, 8, 'k')
    for x in (12, 16, 20):
        c.vline(x, 8, 11, 'k')
    # swirling portal glow between the pillars
    c.disc(16, 18, 6, 'h')
    c.ring(16, 18, 6, 's')
    c.ring(16, 18, 4, 'l')
    c.disc(16, 18, 2, 'l')
    c.outline()
    return c, p


def titan_forge():
    c = Canvas()
    p = {'w': '#4a423c', 'd': '#28221e', 'g': '#4a4038', 'f': '#ffa030',
         's': '#ffd070', 'k': '#141212'}
    _plinth(c, 'g', x=2, w=28)
    _wall(c, 4, 12, 24, 14, 'w')
    _gable(c, 16, 5, 12, 13, 'd')
    c.rect(7, 2, 5, 11, 'd')
    c.rect(20, 2, 5, 11, 'd')
    c.disc(9, 2, 3, 'f')
    c.disc(22, 2, 3, 'f')
    _door(c, 16, 26, 8, 9, 'k')
    c.rect(12, 22, 3, 3, 'f')
    c.rect(17, 22, 3, 3, 'f')
    c.rect(6, 22, 4, 3, 'k')
    c.rect(4, 20, 2, 2, 's')
    c.outline()
    return c, p


SPRITES = {
    'barracks': barracks,
    'market': market,
    'temple': temple,
    'granary': granary,
    'walls': walls,
    'archery_range': archery_range,
    'war_academy': war_academy,
    'mage_tower': mage_tower,
    'stables': stables,
    'trade_hall': trade_hall,
    'fortifications': fortifications,
    'siege_workshop': siege_workshop,
    'elite_barracks': elite_barracks,
    'arcane_sanctum': arcane_sanctum,
    'cathedral': cathedral,
    'treasury': treasury,
    'grand_walls': grand_walls,
    'masons_guild': masons_guild,
    'legendary_forge': legendary_forge,
    'signal_towers': signal_towers,
    'ritual_site': ritual_site,
    'mine': mine,
    'mana_well': mana_well,
    'archive': archive,
    'deep_mine': deep_mine,
    'crystal_sanctum': crystal_sanctum,
    'necropolis': necropolis,
    'dragon_shrine': dragon_shrine,
    'master_forge': master_forge,
    'grove_of_ages': grove_of_ages,
    'binding_circle': binding_circle,
    'titan_forge': titan_forge,
}
