"""Palette v3 ("cheerful readability") -- the base 40, role for role with the old keys.
(index, key, name, hex, family). Extras are appended by build_palette.py."""

BASE = [
    # neutrals: a cool steel/navy ladder (soot/coal double as the UI navy), warm cream and white
    (0, '0', 'void',    '#140d0c', 'neutral'),
    (1, '1', 'soot',    '#14223e', 'neutral'),
    (2, '2', 'coal',    '#26334f', 'neutral'),
    (3, '3', 'iron',    '#424858', 'neutral'),
    (4, '4', 'slate',   '#5f6676', 'neutral'),
    (5, '5', 'ash',     '#848b98', 'neutral'),
    (6, '6', 'stone',   '#adb3bd', 'neutral'),
    (7, '7', 'bone',    '#e9dbb3', 'neutral'),
    (8, '8', 'vellum',  '#fffaec', 'neutral'),
    # water: the reference's saturated friendly blues; frost: ice, snow
    (9,  'd', 'deep',   '#0b4290', 'navy'),
    (10, 'n', 'navy',   '#1765b2', 'navy'),
    (11, 'N', 'tide',   '#3f8cd2', 'navy'),
    (12, 'g', 'glint',  '#80b7e3', 'frost'),
    (13, 'f', 'frost',  '#c6ddec', 'frost'),
    (14, 'F', 'rime',   '#f2f8fc', 'frost'),
    # pine: blue-green conifers, kept apart from the yellow-green grass
    (15, 'k', 'pine-dk', '#163c2f', 'teal'),
    (16, 't', 'pine',   '#215e3e', 'teal'),
    (17, 'T', 'needle', '#3c8b4f', 'teal'),
    # grass: sunny yellow-green
    (18, 'q', 'moss-dk', '#4a6f22', 'moss'),
    (19, 'm', 'moss',   '#6fa534', 'moss'),
    (20, 'o', 'olive',  '#91c03d', 'moss'),
    (21, 'O', 'lichen', '#bddb5d', 'moss'),
    # earth
    (22, 'u', 'umber',  '#5b3a1f', 'earth'),
    (23, 'Q', 'dun',    '#c2a95f', 'earth'),
    (24, 'x', 'rust',   '#b9582a', 'earth'),
    (25, 'y', 'ochre',  '#dba63b', 'earth'),
    (26, 'Y', 'sand',   '#ecd18c', 'earth'),
    # arcane
    (27, 'v', 'bruise', '#2f1b4b', 'purple'),
    (28, 'p', 'dusk',   '#5c3393', 'purple'),
    (29, 'P', 'lilac',  '#a276da', 'purple'),
    # firelight
    (30, 'e', 'ember',  '#8d2f10', 'fire'),
    (31, 'E', 'flame',  '#ef6b1b', 'fire'),
    (32, 'l', 'amber',  '#ffb63b', 'fire'),
    (33, 'L', 'glow',   '#ffe78c', 'fire'),
    # crimson
    (34, 'c', 'blood',  '#561417', 'crimson'),
    (35, 'C', 'crimson', '#c8272b', 'crimson'),
    (36, 'R', 'scarlet', '#f15b47', 'crimson'),
    # ritual
    (37, 's', 'barrow', '#2a7a63', 'ritual'),
    (38, 'S', 'ritual', '#70e0b1', 'ritual'),
    (39, 'W', 'wisp',   '#d9fbe9', 'ritual'),
]

# extras: character ramps the unit atlases need (skin, wood/leather, olive cloth, warm greys,
# maroon, royal blue, bright gold). Chosen from a constrained k-means of the downsampled units.
EXTRAS = [
    ('a', 'skin-lt', '#e8bd90', 'skin'),
    ('b', 'skin',    '#c58b58', 'skin'),
    ('h', 'bark',    '#3a2314', 'wood'),
    ('r', 'auburn',  '#7a3b1c', 'wood'),
    ('i', 'leather', '#96612f', 'wood'),
    ('j', 'khaki',   '#a9874f', 'wood'),
    ('w', 'olive-dk', '#2c4117', 'olive'),
    ('z', 'fern-dk', '#5c6c2c', 'olive'),
    ('A', 'fern',    '#7f9134', 'olive'),
    ('B', 'pewter',  '#4f4a4c', 'grey'),
    ('D', 'flint',   '#77716f', 'grey'),
    ('G', 'ash-lt',  '#a8a09a', 'grey'),
    ('H', 'silver',  '#cdcacc', 'grey'),
    ('I', 'maroon',  '#8f1e1f', 'crimson'),
    ('J', 'royal',   '#3356b6', 'royal'),
    ('K', 'gold',    '#f2c74b', 'gold'),
]
