# ART.md: style guide for the ffh-play client ("Cheerful Readability")

This is the binding style guide for all art in the browser client (`crates/ffh-play`): map, units,
cities, icons and UI chrome. Sprites are authored **as text** (section 13) and turned into images
by the client at load time. Every asset is either drawn for this game or cut from the project
owner's own reference sheets (section 10.2); nothing is taken from Civ4 or FfH2. The isometric
renderer is outside this guide: its art and its rules are the M7 package's (docs/ISO.md).

The direction is **cheerful readability**, in the spirit of the 16-bit tactics games (Shining
Force, the handheld Fire Emblems): a sunny, saturated world, crisp outlined figures, bold owner
colours and gold-framed navy panels. It replaced the earlier "Monastic Pixel Noir" (a night-time
manuscript) in the October 2026 revamp. The references are the owner's map mock-up
(`docs/art/style-reference-map.png`) and the unit atlases (`docs/art/unit-atlas-1.png` to
`-3.png`, "part 1 of 4": melee, recon, archers, mounted, siege, naval, animals). They set the look;
their hex grid and hex bases are not copied (square tiles stay, section 12).

---

## 1. Direction

**The map is a sunny tactics board.** Ground in plan view, figures, trees, mountains and towns
upright; everything lit by a bright sun from the upper left; every actor outlined so it reads
against any ground.

Five principles, in priority order:

1. **Readability first.** Every unit, city and landmark reads at 1x on any terrain: a dark outline
   round a bright, simply shaded interior. Who owns what reads at a glance: the team ring under
   each unit, solid border lines, coloured plaques.
2. **A bright, friendly world.** Daylight. Water is a friendly saturated blue, grass sunny
   yellow-green, desert warm sand, snow white. Colour is everywhere, but it is clean, never neon
   and never muddy.
3. **Meaning still has colour.** This is still Fall from Heaven: hell terrain, demons and
   Armageddon keep their colours (firelight, crimson, the ritual green, the arcane purple), drawn
   in the same cheerful key as everything else.
4. **Clean pixel art.** Crisp 1 px outlines, two to four tones per material, ordered dithering
   only, no anti-aliasing, integer scaling.
5. **Calm ground, lively actors.** Terrain texture is fine and even, so a screen of land reads as
   land; contrast and saturation go to units, cities, borders and landmarks.

---

## 2. Palette

### 2.1 The 56 colours

The palette is closed: every pixel of every asset, and every UI colour, is one of these 56
colours (UI chrome may use CSS alpha only for the modal backdrop). Each colour has a one-character
**key** used in sprite text (section 13). The first 40 keys are the original roles, recoloured
(digits `0`-`8` a value ladder; in each family the lower-case key is the darker partner of the
upper-case key); the 16 after them were added for the unit atlases (skin, leather, olive cloth,
warm greys, maroon, royal blue and gold), chosen by a constrained k-means of the downsampled atlas
units around the fixed 40. L* is CIE lightness (0 to 100).

**Neutrals: a cool steel and navy ladder, warm cream and white.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 0 | `0` | void | `#140d0c` | 4 | outlines of every upright sprite, deepest shadow |
| 1 | `1` | soot | `#14223e` | 13 | panel ground (the interface navy), unrevealed land, cool shadow |
| 2 | `2` | coal | `#26334f` | 21 | raised panels, dark cloth and metal |
| 3 | `3` | iron | `#424858` | 31 | steel in shadow, rock shade |
| 4 | `4` | slate | `#5f6676` | 43 | steel, rock, tundra marks |
| 5 | `5` | ash | `#848b98` | 58 | lit steel and stone |
| 6 | `6` | stone | `#adb3bd` | 73 | stone highlights, bright steel |
| 7 | `7` | bone | `#e9dbb3` | 88 | cream: body text, walls, bone, roads |
| 8 | `8` | vellum | `#fffaec` | 98 | warm white: highlights, titles, the selection rim |

**Water: friendly saturated blues.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 9 | `d` | deep | `#0b4290` | 29 | OCEAN ground |
| 10 | `n` | navy | `#1765b2` | 42 | COAST ground, river ink |
| 11 | `N` | tide | `#3f8cd2` | 57 | wave crests, ripples |

**Frost: ice and snow.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 12 | `g` | glint | `#80b7e3` | 72 | glints, ice shadow, sky cloth |
| 13 | `f` | frost | `#c6ddec` | 87 | snow shadow, ice |
| 14 | `F` | rime | `#f2f8fc` | 97 | SNOW ground, foam |

**Pine: blue-green conifers, kept apart from the grass.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 15 | `k` | pine-dk | `#163c2f` | 22 | pine shadow |
| 16 | `t` | pine | `#215e3e` | 35 | pine body |
| 17 | `T` | needle | `#3c8b4f` | 52 | pine highlight, the Vert field |

**Grass: sunny yellow-green.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 18 | `q` | moss-dk | `#4a6f22` | 43 | grass shade, marsh ground |
| 19 | `m` | moss | `#6fa534` | 62 | GRASSLAND ground |
| 20 | `o` | olive | `#91c03d` | 72 | lit grass, tufts |
| 21 | `O` | lichen | `#bddb5d` | 83 | tuft tips, sunlit rims of woods |

**Earth: wood, wheat, gold and sand.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 22 | `u` | umber | `#5b3a1f` | 28 | dark wood and soil, broken lands |
| 23 | `Q` | dun | `#c2a95f` | 70 | PLAINS ground (golden wheat) |
| 24 | `x` | rust | `#b9582a` | 49 | orange-brown, the Tenne shade |
| 25 | `y` | ochre | `#dba63b` | 71 | ochre gold, dune undersides |
| 26 | `Y` | sand | `#ecd18c` | 85 | DESERT ground |

**Purple: the arcane.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 27 | `v` | bruise | `#2f1b4b` | 15 | arcane shadow, shallows |
| 28 | `p` | dusk | `#5c3393` | 31 | mage robes, perdition |
| 29 | `P` | lilac | `#a276da` | 58 | mana, spell light, the Purpure field |

**Firelight.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 30 | `e` | ember | `#8d2f10` | 33 | embers |
| 31 | `E` | flame | `#ef6b1b` | 61 | flame, the Tenne field |
| 32 | `l` | amber | `#ffb63b` | 79 | lit windows, lanterns |
| 33 | `L` | glow | `#ffe78c` | 92 | light cores, pale gold highlights |

**Crimson: blood, war and red roofs.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 34 | `c` | blood | `#561417` | 18 | blood, the maroon title ribbon |
| 35 | `C` | crimson | `#c8272b` | 44 | crimson: roofs, demons, the Gules field |
| 36 | `R` | scarlet | `#f15b47` | 59 | scarlet: roof rims, demon eyes |
| 53 | `I` | maroon | `#8f1e1f` | 32 | maroon: red cloth shade, the Gules shade |

**Pale green: ritual and the unnatural.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 37 | `s` | barrow | `#2a7a63` | 46 | ritual haze |
| 38 | `S` | ritual | `#70e0b1` | 82 | ritual light, undead eyes |
| 39 | `W` | wisp | `#d9fbe9` | 96 | ritual core |

**Skin.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 40 | `a` | skin-lt | `#e8bd90` | 79 | light skin |
| 41 | `b` | skin | `#c58b58` | 63 | skin |

**Wood and leather.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 42 | `h` | bark | `#3a2314` | 16 | dark wood, doors, hair |
| 43 | `r` | auburn | `#7a3b1c` | 33 | auburn hair, leather shade |
| 44 | `i` | leather | `#96612f` | 46 | leather |
| 45 | `j` | khaki | `#a9874f` | 58 | khaki leather, bronze rule, the Or shade |

**Olive cloth.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 46 | `w` | olive-dk | `#2c4117` | 25 | olive cloth shade |
| 47 | `z` | fern-dk | `#5c6c2c` | 43 | olive cloth |
| 48 | `A` | fern | `#7f9134` | 57 | fern cloth, lichen |

**Warm greys.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 49 | `B` | pewter | `#4f4a4c` | 32 | pewter: warm dark grey |
| 50 | `D` | flint | `#77716f` | 48 | flint: warm grey, the Argent shade |
| 51 | `G` | ash-lt | `#a8a09a` | 66 | warm light grey, tundra ground |
| 52 | `H` | silver | `#cdcacc` | 82 | silver: the Argent field |

**Royal blue.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 54 | `J` | royal | `#3356b6` | 39 | royal blue: the Azure field |

**Gold.**

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 55 | `K` | gold | `#f2c74b` | 82 | gold: frames, rules, gilt, the Or field |

The same palette in machine-readable form (`index key name hex family`); `tools/validate.mjs`
checks that it equals `web/art/palette.txt`:

```palette
 0 0 void     #140d0c neutral
 1 1 soot     #14223e neutral
 2 2 coal     #26334f neutral
 3 3 iron     #424858 neutral
 4 4 slate    #5f6676 neutral
 5 5 ash      #848b98 neutral
 6 6 stone    #adb3bd neutral
 7 7 bone     #e9dbb3 neutral
 8 8 vellum   #fffaec neutral
 9 d deep     #0b4290 navy
10 n navy     #1765b2 navy
11 N tide     #3f8cd2 navy
12 g glint    #80b7e3 frost
13 f frost    #c6ddec frost
14 F rime     #f2f8fc frost
15 k pine-dk  #163c2f teal
16 t pine     #215e3e teal
17 T needle   #3c8b4f teal
18 q moss-dk  #4a6f22 moss
19 m moss     #6fa534 moss
20 o olive    #91c03d moss
21 O lichen   #bddb5d moss
22 u umber    #5b3a1f earth
23 Q dun      #c2a95f earth
24 x rust     #b9582a earth
25 y ochre    #dba63b earth
26 Y sand     #ecd18c earth
27 v bruise   #2f1b4b purple
28 p dusk     #5c3393 purple
29 P lilac    #a276da purple
30 e ember    #8d2f10 fire
31 E flame    #ef6b1b fire
32 l amber    #ffb63b fire
33 L glow     #ffe78c fire
34 c blood    #561417 crimson
35 C crimson  #c8272b crimson
36 R scarlet  #f15b47 crimson
37 s barrow   #2a7a63 ritual
38 S ritual   #70e0b1 ritual
39 W wisp     #d9fbe9 ritual
40 a skin-lt  #e8bd90 skin
41 b skin     #c58b58 skin
42 h bark     #3a2314 wood
43 r auburn   #7a3b1c wood
44 i leather  #96612f wood
45 j khaki    #a9874f wood
46 w olive-dk #2c4117 olive
47 z fern-dk  #5c6c2c olive
48 A fern     #7f9134 olive
49 B pewter   #4f4a4c grey
50 D flint    #77716f grey
51 G ash-lt   #a8a09a grey
52 H silver   #cdcacc grey
53 I maroon   #8f1e1f crimson
54 J royal    #3356b6 royal
55 K gold     #f2c74b gold
```

### 2.2 Where each family may be used

| family | on the map | in the UI | never |
|---|---|---|---|
| neutrals | outlines, steel, stone, rock, roads, walls (cream) | panels (navy `1`/`2`), text (cream `7`, white `8`) | (none) |
| water and frost | water, rivers, snow, ice, steel-blue cloth, the Celeste field | water and cold icons, the Celeste and Azure tinctures | warm surfaces |
| pine | conifers, ancient forest, the Vert field | the Vert tincture | open ground |
| grass | grass, reeds, jungle, foliage | health, nature | demons, undead |
| earth | plains, desert, wood, gilt (ochre), roads' verges | food, commerce | (none) |
| purple | the arcane: mana, spells, mage robes, fields of perdition, shallows, the Purpure field | mana, spells, arcane promotions | ordinary cloth on non-arcane units |
| firelight | windows, lanterns, torches, forges, flames, burning sands, the Tenne field (`E`) | happiness, warnings, the End Turn crescent | large fills that are not light or the Tenne field |
| crimson | blood and corruption, demons, war, red roofs and cloth, the Gules field | war, danger, losses, deficits, the maroon title ribbons | good-aligned light |
| pale green | ritual and the unnatural: sacred sites, ritual circles, undead eyes, world spells | rituals, world spells, Life mana | vegetation |
| skin, wood, olive, warm greys | figures (faces, leather, cloth, armour) | (portraits, unit figures in panels) | ground textures |
| royal blue, gold | cloth, banners, gilt; the Azure and Or fields | frames, rules and gilt (gold `K`, bronze `j`) | ground textures |

### 2.3 Value and colour budget

Daylight inverts the old budget: the map is light, with darks reserved for outlines and shade.
Targets for a full-screen view of a typical game (visible tiles, each pixel taken as its nearest
palette colour), with the revamp's measurement of the turn-70 test game at 2x in brackets:

- At most **25%** of map pixels at L* 25 or below: outlines, pine shade, deep water [12%].
- At least **30%** above L* 50 [38%]; above L* 75 only highlights, snow, sand and foam [9%].
- The reserved families (fire, crimson, pale green) cover at most **6%** of a screen outside hell
  terrain and cities [0.2%].
- Outlines (`0`) about **4-8%** [5.9%]: a figure is defined by its outline, the ground has none.
- **Per sprite**: a unit uses at most 18 colours (outline and owner slots excepted; the validator
  warns over 20), a terrain texture at most 6, a city at most 12, an icon at most 4.

### 2.4 Remap tables: fog and light pools

The palette is indexed, so darkening and lighting are **index remaps**, never alpha blends. The
client builds one lookup per table and applies it per pixel, with a 4x4 Bayer threshold where a
falloff is needed (section 4.4). The tables are derived from the colours, not drawn by hand
(`tools/atlas/build_palette.py`):

- **`FOG`**: remembered ground, never black: lightness x 0.8 and chroma x 0.55, the nearest colour
  that is no lighter and is not a light (lights go out). Water steps down within its own family
  (coast to ocean, crests to coast), so remembered sea still reads as sea.
- **`FIRE`**, **`BLOOD`**, **`RITE`**, **`ARC`**: pull a colour toward firelight (lighter, toward
  amber), corruption (redder, a little darker), ritual (toward the pale teal-green) and the arcane
  (toward violet). Used by the light pass (4.4) and by mana nodes, landmarks and lairs.

The passes' own tables follow them as further columns, written by hand in `build_palette.py`
(pairs over identity, or a row per key) rather than derived, and read from `art.tables` like the
rest:

- **`VERGE`**, **`MOON`**, **`GRAIN`**, **`SHADE`**: the ground pass (5.1): one step lighter for
  path and river verges, the sunlit step of a clearing, its brightest grain one step further, and
  the cloud-shade step down. Snow, water, hell purples and lights keep their colour.
- **`REMEMBER`**: `FOG` with snow and ice sent to pale greys rather than blue, so remembered snow
  still reads as snow (the fog pass and the vignette, 8.2, 8.3).
- **`WARM`**, **`HEARTH`**, **`EMBER`**, **`SMOULDER`**, **`GORE`**, **`AURA`**, **`HALLOW`**,
  **`HAZE`**: the light pass's rings (4.4): the outer, inner and core tables of each pool family
  that the derived tables do not give (`RINGS` in `cities/citylight.js` names each family's three).

```remap
key  FOG      FIRE     BLOOD    RITE     ARC      VERGE    MOON     GRAIN    SHADE    REMEMBER WARM     HEARTH   EMBER    SMOULDER GORE     AURA     HALLOW   HAZE
0    0        0        0        0        0        0        0        0        0        0        u        e        e        c        c        v        k        k
1    1        u        c        k        v        2        1        1        1        1        u        e        e        c        c        v        s        k
2    2        u        c        k        v        3        2        2        2        2        u        e        E        c        c        v        s        t
3    3        i        c        s        d        4        4        4        3        3        x        e        E        c        C        p        s        t
4    B        i        c        s        p        5        5        5        3        B        x        x        E        x        C        p        s        t
5    4        b        r        s        P        6        6        6        4        4        y        E        l        x        C        P        S        T
6    5        a        b        S        P        7        7        6        5        5        Y        E        l        C        R        P        S        S
7    G        L        b        S        P        7        7        7        7        G        8        l        L        C        R        8        W        W
8    H        Y        a        W        P        8        8        8        8        H        8        l        L        C        R        8        W        W
d    d        B        c        k        p        d        d        d        d        d        d        d        d        d        d        d        d        d
n    d        D        c        s        J        n        n        n        n        d        n        n        n        n        n        n        n        n
N    n        G        D        s        J        N        N        N        N        n        N        e        E        c        C        p        s        t
g    N        a        G        S        P        f        g        g        g        5        g        E        l        x        C        P        S        T
f    g        7        G        S        P        f        f        f        f        6        Y        E        l        x        C        P        S        S
F    f        a        a        W        P        F        F        F        F        H        7        l        L        C        R        8        W        W
k    k        u        c        t        v        t        t        t        k        k        u        e        E        c        c        v        s        t
t    k        z        u        s        v        T        T        T        k        k        x        x        E        c        C        p        s        T
T    t        A        r        T        4        T        T        T        t        t        y        E        l        x        C        P        S        S
q    w        A        r        T        4        m        m        o        q        w        x        x        E        c        c        p        s        t
m    z        K        i        T        5        o        o        o        q        z        y        E        l        x        C        p        S        T
o    A        K        b        S        5        O        O        O        m        A        Y        E        l        x        C        P        S        T
O    A        K        b        S        6        O        O        O        o        A        Y        l        L        x        C        P        S        S
u    h        i        c        t        v        i        i        i        u        h        x        x        E        c        C        p        s        t
Q    j        y        x        S        P        Y        Y        Y        j        j        y        E        l        x        C        p        S        T
x    r        E        I        z        P        y        y        y        x        r        y        E        l        C        C        P        S        T
y    j        l        x        S        5        Y        Y        Y        j        j        Y        l        L        x        C        P        S        S
Y    G        K        b        S        6        7        Y        Y        Q        G        7        l        L        C        R        P        W        S
v    1        c        c        k        p        p        v        v        v        1        x        x        E        c        c        p        s        t
p    v        c        c        3        p        p        p        p        p        v        x        E        l        x        C        P        S        T
P    4        G        I        5        P        P        P        P        P        4        Y        l        L        C        R        8        W        S
e    u        x        I        t        v        e        e        e        e        u        E        E        l        C        C        p        s        t
E    i        E        C        z        P        E        E        E        E        i        l        l        L        C        R        P        S        S
l    j        l        x        S        6        l        l        l        l        j        L        L        L        R        R        P        W        W
L    a        K        b        S        H        L        L        L        L        a        L        L        L        R        R        8        W        W
c    h        e        c        k        v        c        c        c        c        h        x        x        E        C        C        p        s        t
C    r        E        I        D        p        C        C        C        C        r        E        E        l        C        R        P        S        T
R    r        E        C        G        P        R        R        R        R        r        l        l        L        R        R        P        W        S
s    k        j        u        s        n        s        s        s        s        k        x        x        E        c        C        p        S        T
S    s        L        a        S        g        S        S        S        S        s        y        E        l        x        C        P        W        W
W    H        Y        a        S        g        W        W        W        W        H        8        l        L        C        R        8        W        W
a    G        l        b        S        P        a        a        a        a        G        a        a        a        a        a        a        a        a
b    j        y        x        s        P        b        b        b        b        j        b        b        b        b        b        b        b        b
h    h        r        c        k        v        u        h        h        h        h        h        h        h        h        h        h        h        h
r    u        x        I        t        v        x        r        r        r        u        r        r        r        r        r        r        r        r
i    u        x        I        s        p        j        j        j        u        u        i        i        i        i        i        i        i        i
j    D        y        x        s        P        Q        Q        Y        i        D        j        j        j        j        j        j        j        j
w    k        i        c        t        v        z        z        z        w        k        w        w        w        w        w        w        w        w
z    w        j        r        s        4        A        A        A        w        w        z        z        z        z        z        z        z        z
A    z        y        i        T        4        O        O        O        z        z        A        A        A        A        A        A        A        A
B    B        i        c        s        v        B        B        B        B        B        B        B        B        B        B        B        B        B
D    B        b        I        s        P        G        G        G        D        B        D        D        D        D        D        D        D        D
G    D        a        r        s        P        H        H        H        D        D        G        G        G        G        G        G        G        G
H    G        a        b        S        P        H        H        H        G        G        H        H        H        H        H        H        H        H
I    c        x        I        t        p        I        I        I        I        c        I        I        I        I        I        I        I        I
J    d        D        v        s        p        J        J        J        J        d        J        J        J        J        J        J        J        J
K    Q        l        x        S        6        K        K        K        K        Q        K        K        K        K        K        K        K        K
```

---

## 3. Grid, projection and scale

### 3.1 Tiles and projection

- **Square tiles of 32x32 art pixels**, north up, matching Civ4's square plot grid. Map widths are
  multiples of 4 plots, so the 64 px texture period divides every map and wrapping maps repeat
  seamlessly. (The references use hexes; this game does not.)
- **Ground in plan view; objects in elevation.** Terrain, water, roads, rivers and borders are seen
  from above. Trees, hills, peaks, buildings, improvements and figures stand upright, seen from the
  front.
- **Overhang.** An upright sprite stands on its tile and may rise into the tile to the north: by up
  to 8 px (peaks, ancient forest, cities of tier I to III, most improvements), by up to 12 px
  (volcano, tall landmarks, giants in the 40x40 frame). Sprites never extend more than 4 px past
  their tile's left or right edge.
- **Draw order.** The terrain cache (ground, water, rivers, routes, flat improvements, city pools,
  then upright features, peaks, improvements and cities row by row from north to south), then
  borders and fog, then units row by row (team ring, then figure), then overlays: count chips, health
  bars, plaques, selection, path.

### 3.2 On-screen scale

- Art is drawn at 1x and shown only at **integer scales**: one art pixel is always a whole number
  of device pixels (`zoom x devicePixelRatio`, rounded). `imageSmoothingEnabled = false`,
  `image-rendering: pixelated`.
- **Zoom steps** (on-screen pixels per tile at device-pixel ratio 1):

  | step | px/tile | what is drawn |
  |---|---|---|
  | chart | 8 | minimap-style: flat terrain-family colours, owner colours, city dots, borders; no sprites |
  | survey | 32 (1x) | everything; plaques show the size only |
  | **default** | **64 (2x)** | everything |
  | close | 96 (3x) | everything |
  | study | 128 (4x) | everything |

- **UI scale** `u` is an integer (default 2 at device-pixel ratio 1). Every pixel-art UI asset is
  shown at `u`; CSS lengths in UI chrome are multiples of `u`.

### 3.3 Sprite sizes

| asset | canvas (art px) | notes |
|---|---|---|
| terrain texture | 64x64 per terrain, seamless | sampled through the domain warp (5.1) |
| feature, peak, hill | up to 34x40, plus overhang | scattered inside the tile |
| **unit** | **32x32** | feet at (15, 29): the figure fills the tile, as in the references |
| giant unit | 40x40 | feet at (19, 37): rises 8 px into the tile above, 4 px over each side |
| equipment | 24x24 | feet at (11, 21), placed at tile offset (6, 7): items lying on the tile (10.4) |
| team ring | 24x8 (30x9 under giants) | drawn by the renderer under the feet (7.4) |
| city | up to 34x34 | base on the bottom row; tiers in section 9 |
| improvement | up to 24x20 | small painted objects; towers and castles may rise 8 px |
| landmark | 32x32, plus overhang | may glow (6.2) |
| resource | 13x13, feet at (6, 12) | an object standing in the tile's lower right, feet at (25, 30), on a soft shadow |
| stack count chip | up to 13x9 | top-left corner of the tile (top-right in a city) |
| UI icon | 16x16 (8x8 inline and on the map) | section 11.5 |
| cursor | 16x16 | section 11.7 |

---

## 4. Drawing rules

### 4.1 Light

- **Sunlight from the upper left.** Lit faces are one or two ramp steps lighter, shadow faces one
  or two darker. Shadows lean cool (navy, pine, bruise) rather than grey: the classic 16-bit
  shading.
- **Warm local light** (windows, lanterns, fires) is a small accent by day (4.4).
- **Contact.** Every owned unit stands on its team ring (7.4); unowned figures and animals on a 50%
  checker shadow ellipse (14x3). Ground marks cast no shadow.

### 4.2 Outlines

- **Every upright sprite** (units, cities, trees, peaks, upright improvements, landmarks) has a
  1 px dark outline (`0`). On an atlas unit the outline is enforced: dark edge pixels become `0`,
  and a light edge pixel gets an outline pixel outside it, so a white sail or a silver helm still
  reads on snow.
- There is **no moon rim** (the old ash rim on the top and left edges): the outline does that work
  in daylight. A **selected** unit gets a 1 px white (`8`) rim on all sides.
- **Ground** (terrain textures, flat marks, water) has no outlines; its linework is the
  shoreline, rivers, roads and borders (section 5).

### 4.3 Shading and colour count

- Two to four tones per material, plus outline; hue-shift along the ramp (skin to auburn, gold to
  khaki) rather than blending families; no pillow shading.
- Colour limits (outline and owner slots excluded): unit 18; city 12; terrain texture 6; feature,
  improvement and resource 8; peak and hill 8; icon 8 (16x16) and 6 (8x8). An atlas or composed
  unit is quantized to the palette and then reduced to its 18 most used colours (the rarest merged
  into their nearest kept colour); map sprites and icons are reduced the same way to their budget.
- **Painted at the sprite's own resolution** (map sprites and icons, `tools/atlas/maplib.Px`): each
  part is a crisp shape (at least half a pixel covered), shaded per pixel by the lit-dome normal of
  its blurred mask and posterized to its ramp; where a part lies over an earlier one its lower and
  right edge takes its darkest tone; one void outline closes the whole silhouette, except on thin
  strokes (fronds, twigs, staves) and flat ground marks. Small regular things (pines, palms, dead
  trees, bushes, motes) follow pixel rules instead: a lit edge pixel on the west, base, shade,
  a dark underside.
- No anti-aliasing against the background; sprites have 1-bit alpha.

### 4.4 Dithering, glows and daylight

- **Only ordered patterns**: a 50% checker, a 25% grid, and 4x4 Bayer thresholds for falloffs.
  No noise dither, no error diffusion.
- Where dithering is used: soft variation in terrain textures (small 25% grids or 50% checkers of
  a near tone, never solid blobs); transition bands between terrains (5.1); glow rings; the fog
  edge; translucency (4.5); a border's inner fringe (8.1).
- **Daylight** (`DAYLIGHT` in `cities/citylight.js`): the sun outshines firelight, so
  - a city casts **no firelight pool**, and no wall glow or facade light (the night effects of the
    old guide);
  - corruption (`BLOOD`, Infernal and Sheaim towns) and the arcane (`ARC`, Amurites and
    Svartalfar) keep their pools as faction identity, at 55% of the old radii; ritual (`RITE`) at
    70%;
  - firelight point lights (lanterns, campfires, the tribal bonfire) are halved; `BLOOD`, `RITE`
    and `ARC` point lights (hellfire, sacred sites, mana nodes) keep their size.
  A pool is three nested rings (outer, inner, core), each remapping a pixel once from its own
  ground colour with Bayer falloff; open water keeps its colour under every ring.
- City windows stay amber (`l`, recoloured to the civilization's window light, 7.3): a few warm
  windows on cream walls read as cosy, not as night. The stone around a window is not pre-lit.

### 4.5 Translucency and motion

- 1-bit alpha only. Translucency is a 50% checker of the sprite over what is behind it: illusions
  and puppets, your own invisible units, ghosts (Spectre, Sailor's Dirge), move and placement
  previews.
- **Motion is short and purposeful** (all of it off under `prefers-reduced-motion`):

  | what | how |
  |---|---|
  | unit move | the figure (and its ring, marks and selection) slides tile to tile, 4 tiles at most |
  | combat | the struck tile flashes a dithered crimson |
  | camera | glides to the selection |
  | turn | the End Turn crescent breathes while the AIs move and flares when your turn returns |
  | HUD numbers | count up and down to their new value |
  | fire flicker | 2 frames, 400 ms |
  | path dots | march 1 px every 150 ms |

---

## 5. Terrain

### 5.1 How terrain works

1. **One seamless 64x64 texture per terrain type**: a clear base, soft dithered variation in a
   near tone, and small lit marks (tufts, flowers, stubble, dune crests, wave crests) about one
   per 40 to 80 px². The textures were baked by `tools/atlas/bake_textures.py` and are edited by
   hand from there.
2. **The ground pass** (`terrain/ground.js`) picks the texture per pixel: each pixel takes the
   terrain class with the largest Gaussian weight of the 3x3 tile centres around it, sampled
   through a three-octave domain warp, so a lone tile of one terrain becomes a blob and edges
   wander; where two land textures meet, the winner's share rises from 50% to 100% over 4 px,
   thresholded by the Bayer matrix (an ordered transition band, never a line). The centre of every
   tile always shows its own terrain.
3. **Sunlit clearings and cloud shade**: a low-frequency field lifts the land one step within its
   own family (`MOON`: grass to lit grass, wheat to sand, greys up the ladder) by a Bayer share up
   to 40%, adds a warm grain at its brightest (`GRAIN`, up to 15%), and lowers it one step
   (`SHADE`) by up to 15% where it is low. Water, ink and lights are left alone.
4. **Uprights are scattered per tile** by a hash of the plot coordinates: trees, hills, peaks.
5. The result is cached per zoom level and invalidated per tile.

### 5.2 Base terrains

Each hell terrain is the **corrupted twin** of a normal one (`TerrainUp` in the XML): it keeps its
twin's layout (the same marks in the same places) in its own colours, so a player can tell what
the land used to be.

| terrain (`TERRAIN_*`) | base | variation | marks | reads as |
|---|---|---|---|---|
| GRASS | moss `m` | lit grass `o` checkers, shade `q` grids | lit tufts (`O` tip, `o` blades, `q` shadow); a few yellow `L` and white `8` flowers | a sunny meadow |
| PLAINS | dun `Q` | sand `Y` and khaki `j` grids | wheat stubble dashes (`Y` over `j`); a few green tufts `o`/`A` | golden wheat |
| DESERT | sand `Y` | cream `7`, dun `Q` | dune crests (`8` over `y`), khaki specks | warm sand: the lightest land |
| TUNDRA | ash-lt `G` | fern `A` checkers, flint `D` grids | lichen pairs `A`/`O`, short cracks `5` | a pale lichen steppe |
| SNOW | rime `F` | frost `f` grids | wind streaks `f` with a `g` pixel; sparkles `8` | bright snow |
| MARSH | moss-dk `q` | moss `m`, olive-dk `w` | water slits `N` with a `g` glint, reeds `O` | wet green |
| COAST | navy `n` | tide `N` grids | wave crests `g` (a smile of 3-6 px); foam `F` glints | friendly blue shallows |
| OCEAN | deep `d` | navy `n` grids | sparse crests `N`, glints `g` | deep blue |
| BROKEN_LANDS (hell grass) | umber `u` | auburn `r`, bark `h` | the tufts in maroon `I`, scarlet `R` flowers | cracked brown earth |
| FIELDS_OF_PERDITION (hell plains) | dusk `p` | lilac `P`, bruise `v` | stubble in `P` over `v`, blood tufts | purple ash fields |
| BURNING_SANDS (hell desert) | rust `x` | auburn `r` | dune crests in amber `l` over ember `e`, flame specks | smouldering sand |
| SHALLOWS (hell marsh) | bruise `v` | soot `1`, dusk `p` | slits in maroon `I` with a `C` pixel, dead reeds `D` | a black-purple bog |

`TERRAIN_PEAK` and `TERRAIN_HILL` are graphics-only types; on the map, peaks and hills are **plot
types** drawn over the plot's terrain (5.3).

### 5.3 Hills and peaks

- **Hills** (`PLOT_HILLS`): solid rounded mounds (23x11, 19x9, 27x12), lit from the upper left:
  crest `O`, lit body `o`, the ground tone `m` at the base, shade `q` and a soft contour `w` down
  the right side and under it. Keyed in grass colours and remapped per terrain
  (`terrain/manifest.txt`), so a hill is its terrain raised: golden on plains (`O7,oY,mQ,qj,wi`),
  cream on desert, pale grey on tundra, white on snow, ember on burning sands. Never mirrored.
  Forests stand on top of them.
- **Peaks** (`PLOT_PEAK`): faceted massifs 29 to 32 px wide rising up to 8 px into the tile above,
  two or three peaks each: a lit west face (`4`-`7`, bone where the sun strikes) and a shaded east
  face (`2`-`5`) split at the ridge, a few ledges, a void outline, and caps of rime `F`/vellum `8`
  in the light and frost `f`/glint `g` in shadow, jagged at the snow line: small on `rock_*`, deep
  on `snow_*` (snow and tundra). As in the references, peaks are the grey-and-white crests of the
  map.

### 5.4 Water edges

- **Shoreline**: a land pixel next to water is drawn cream `7`, the land pixel behind it a darker
  step; water within 2 px of land gets a foam checker. This lit rim is the strongest line on the
  map.
- **Depth contour**: where ocean meets coast, a dotted line on the ocean side.
- **Lakes** are coast tiles. Light sources on water cast a short broken reflection.

### 5.5 Features

The feature sprites are painted by `tools/atlas/compose_map.py` (4.3):

- **Forest**: pines by pixel rules, two to four tiers widening a pixel a row, a lit `m`/`O` west edge,
  needle `T` body, pine `t` shade and a dark `k` underside with drooping tips, an umber trunk;
  the void outline closes each tier's slanted sides, but the underside rows stay `k`, not void, so
  a stand of overlapping pines reads as green tiers, not a black lattice;
  seven shapes (one in the mossy ramp) scattered per tile. **New forest**: saplings by the same
  rules. **Ancient forest**: broad canopies of overlapping lit domes in deep greens over a thick
  rooted trunk, rising into the tile above.
- **Jungle**: palms (a ringed trunk, two-row fronds lit above and dark beneath, no outline on the
  leaves), a bright round canopy and a low row of bushes. **Burnt forest**: charred trunks.
- **Flood plains**: lush tufts, silt dashes and water glints, a flat ground mark. **Oasis**: a blue
  pool in a green bank under a leaning palm. **Ice**: floes of rime and frost with glint shade
  beneath. **Scrub**: small dry bushes. **Burning sands**: three fires (two frames) with firelight
  pools. **Obsidian**: black glass shards with a lilac glint. **Volcano**: a faceted cone with lava
  runs from a glowing crater and a smoke plume. **Blizzard**: driven snow streaks. **Fallout**:
  glowing pools among dead trees.
- The north-west rim of every wood mass catches the sun (a 1 px `O` edge where open ground lies
  above or to the left, the ground pass).

### 5.6 Rivers and routes

- **Rivers** run on tile edges: a 2 px line of navy `n` with `g` glints, meandering with the warp,
  over a pale verge (a checker one step lighter on the bank).
- **Roads** are pale paths: dashes of cream `7` along a gently swaying line, over a worn verge one
  step lighter (the ground pass, `VERGE`). Roads enter cities at the gate.
- **Railroads** (rare in FfH): two dotted lines in stone `6`.

### 5.7 Improvements and 5.8 Lairs

Improvements and lairs are small painted objects in the tile's lower half (`compose_map.py`): a
farm is a golden field of ripe ears in green rows (a flat ground mark), a pasture a rail fence
round two sheep, plantations and vineyards rows of shrubs and vines; homes grow from cottage to
town (one hut, then two, three and a little town round a spire, thatch and red tile, lit windows);
mines are an adit in a hillock beside a spoil heap with a gold fleck; forts, towers, castles and
citadels are stone keeps and timber; each mana node is a slate obelisk carved with its sphere's
rune. Lairs read at a glance: cave mouths in rock or sandstone, a barrow door, a goblin palisade
round a campfire, a ribcage, ruins wrapped in vine, a dungeon stair, a portal's lilac swirl. A
goody hut is a small cairn with a flickering candle.

---

## 6. Resources and landmarks

### 6.1 Resources

A revealed resource (`BONUS_*`) is a **small object** standing in the tile's lower right, as in the
references: a 13x13 painted sprite with its feet at (6, 12), drawn with its feet at tile (25, 30)
on a soft checker shadow. The object names the resource: a golden sheaf, a husked cob of corn, a bowl of
rice, a cow, a pig, a sheep, a deer, a toad, a banana bunch, a fish, a clam, a crab, a whale's back
and spout; copper, iron and mithril ingots; a horse and the nightmare; a keg, a vial of reagents, a
veined sheut stone, a marble block, a dye pot, a pelt, cut gems, gold nuggets, a censer, a tusk, a
bolt of silk, sugar cane, grapes, a pearl in its shell, cotton, a tankard of ale, folded fine
clothes, a jewelled ring, razorweed, a sealed jar, the Fruit of Yggdrasil. **Mana** is a standing
stone carved with its sphere's rune (colour the family, shape the sphere, 11.5). The category
objects (`res/_food`, `_strategic`, `_luxury`, `_mana`) are the fallbacks for bonuses added later.
Where painting lost the subject at this size, the object is drawn by hand as a fill that the tool
outlines (`HAND_RES` in `compose_map.py`): the sheaf (three ears fanned over a tie, stalks
splayed; also `res/_food`), the husked cob, cotton bolls on a shrub, a pearl in an open scallop,
the squat frog.

### 6.2 Landmarks (unique improvements)

The 17 unique improvements (Standing Stones, Yggdrasil, Pool of Tears, Ring of Carcer, Broken
Sepulcher, Maelstrom...) are painted once each (`compose_map.py`: Yggdrasil's vast crown of lit
domes over a buttressed trunk, seven pines by the forest's rules, the menhir ring round a
glowing circle, the Maelstrom's spiral), 32 px wide with up to 8-12 px of overhang, and are
the only map objects besides lights, volcanoes and mana nodes allowed a full glow: sacred sites a
`RITE` haze with a ritual heart, the corrupted ones `BLOOD`, the pyre `FIRE` (halved by day).

---

## 7. Owner identity

Up to about 20 players are told apart by a **tincture** (one of 10 bold field colours), a
**division** of the field that separates players sharing a tincture, and a **sigil** (one charge
per civilization). The tinctures were re-chosen for daylight: ten clearly separate hues and values
that hold up on grass, sand, water and snow.

### 7.1 Tinctures

| tincture | field | shade | charge | border (core over edge) | reads as |
|---|---|---|---|---|---|
| Argent | silver `H` | flint `D` | soot `1` | `H` over `D` | silver-white |
| Or | gold `K` | khaki `j` | soot `1` | `K` over `j` | gold |
| Gules | crimson `C` | maroon `I` | white `8` | `C` over `I` | red |
| Azure | royal `J` | deep `d` | white `8` | `J` over `d` | royal blue |
| Vert | needle `T` | pine `t` | white `8` | `T` over `t` | emerald |
| Purpure | lilac `P` | dusk `p` | white `8` | `P` over `p` | purple |
| Tenne | flame `E` | rust `x` | soot `1` | `E` over `x` | orange |
| Celeste | glint `g` | tide `N` | soot `1` | `g` over `N` | sky blue |
| Sable | coal `2` | soot `1` | cream `7` | `2` over `1`, a cream tick on every segment | navy-black |
| Cendree | flint `D` | pewter `B` | white `8` | `D` over `B` | ash grey |

Gules is the sanctioned owner use of crimson; it goes by default to the blood-and-corruption
civilizations. Argent, Cendree and Sable are told apart by value (light, mid, dark).

### 7.2 Divisions and assignment

Divisions, in order: **plain**; **per pale** (the right half of the field is Sable, or Argent if the
field is Sable); **per fess** (the same, on the lower half); **per bend** (diagonally);
**quarterly**. At game start, in player-slot order, each player takes its civilization's preferred
tincture, plain; if that pair is taken, the next division of the same tincture. Ten tinctures times
five divisions give 50 identities; the sigil always follows the civilization. Barbarians fly Sable
with a cream skull and draw no borders; animals have no owner marks at all.

### 7.3 Civilizations

Sigils are drawn in the charge colour, 5x5 (on standards and plaques) or 7x7 (on banners and
diplomacy screens). They are simple heraldic charges designed for this game. Window light is the
colour of a city's lit windows (amber by default); by day there is no ground pool (4.4).

| civilization | tincture | sigil | city style (section 9.2) | window light |
|---|---|---|---|---|
| AMURITES | Purpure | an eye in a triangle | Cloister | lilac `P` |
| BALSERAPHS | Or | a two-horned jester's mask | Carnival | amber |
| BANNOR | Argent | a sword, point down | Cloister | amber |
| CALABIM | Gules | a goblet | Cloister (dark stone) | amber |
| CLAN_OF_EMBERS | Tenné | two crossed tusks | Tribal | amber |
| DOVIELLO | Sable | a wolf's head | Tribal | amber |
| ELOHIM | Argent | an open hand | Cloister | amber |
| GRIGORI | Azure | a broken chain link | Cloister | amber |
| HIPPUS | Tenné | a horse's head | Tribal | amber |
| ILLIANS | Celeste | a six-rayed frost star | Frost | rime `F` |
| INFERNAL | Gules | a horned skull | Infernal | scarlet `R` |
| KHAZAD | Or | an anvil | Dwarven | amber |
| KURIOTATES | Vert | a strung bow | Cloister | amber |
| LANUN | Azure | an anchor | Harbour | amber |
| LJOSALFAR | Vert | a leaf | Elven | amber |
| LUCHUIRP | Cendrée | a cogwheel | Dwarven | amber |
| MALAKIM | Or | a rayed sun disc | Sun-domes | amber |
| MERCURIANS | Argent | a pair of wings | Celestial | vellum `8` |
| SHEAIM | Gules | a ring with an inverted star | Infernal | scarlet `R` |
| SIDAR | Cendrée | a veiled face | Cloister (faded: ash, no lit windows above size tier II) | stone `6` |
| SVARTALFAR | Purpure | a spider | Elven | lilac `P` |
| barbarian | Sable | a skull | Tribal | amber |

### 7.4 Where identity shows

- **The team ring** (new in the revamp, the primary owner mark): every owned, non-animal unit
  stands on an ellipse in its tincture's field colour with a shade lip at the front and a checker
  shadow inside (24x8, 30x9 under giants), drawn through the owner slots with the division applied,
  so two players sharing a tincture show two-tone rings. It replaces the contact shadow and plays
  the part of the references' coloured bases. Units with hidden nationality show no ring.
- **Borders** (8.1): solid lines in the tincture.
- **The stack marks** keep to the corners and the top edge, so the figure and its ring stay clear:
  a count chip in the top-left corner (top-right in a city), a gold pip beside it for a hero, the
  health bar on the top edge, the state mark top-right. Your own spent stack stands on a quiet ring
  (the tincture's shade as the field): a bright ring under your unit says it can still move. The
  night look's 7x12 gonfalon is opt-in (`state.standard`), like the references, which mark a unit by
  its colours and its land alone.
- **City banner and plaque** (9.4): the size chip in the owner's field.
- **Unit cloth**: an atlas unit's main cloth accent (its red, blue, purple or olive) is converted
  to the owner slots `#` (field, the lighter half of the ramp) and `%` (shade); figures whose colour
  is their identity (demons, angels, ghosts, animals, siege engines, ships) keep their own colours.

---

## 8. Political layer, fog and the edge of the world

### 8.1 Borders

Borders are drawn on tile edges where the revealed owner changes, **on the owner's side**, as a
**solid** 3-row line (no dashes): the tincture's shade at the very edge (so two owners meeting, or
a pale border on pale ground, still separate), the bright stitch colour as the core, then a 50%
checker of the stitch fading into the territory. A divided owner alternates its two tinctures every
6 px; Sable keeps a cream tick on every segment. Two owners sharing an edge show two lines back to
back. Borders are not warped. The **political layer** (a toggle) adds a 25% grid of the owner's
shade over owned ground.

### 8.2 Fog of war

Remembered tiles (revealed, not visible) pass through `REMEMBER`: the palette's `FOG` (dimmer and
greyer, never black, lights out) with snow and ice sent to pale greys so remembered snow still
reads as snow. Every pixel with `(x + y) mod 4 = 0` takes the remap twice (light diagonal
hatching). Last-seen improvements, routes and cities are remembered; foreign units are not. The
fog edge follows the terrain's warp in a 4 px Bayer band.

### 8.3 Unrevealed tiles

Unrevealed land is the interface's **navy** (`1`, `GROUND.unseenKey`), not black, with a ragged
3 px edge. There is **no vignette** by day (`GROUND.vignette`; the ground pass sets
`art.vignetteOn`, which the live client's own vignette honours too). The map viewport sits in the
panels' gold frame.

### 8.4 Minimap

One pixel per tile (two on small maps): water `d`/`n`, unowned land its terrain colour, owned land
the owner's field, cities a white dot, fog through `FOG`, unrevealed navy-black; the viewport a
1 px white rectangle; the frame as for panels.

---

## 9. Cities

### 9.1 Size tiers

Cities are the map's focal points: each tier is a clearly bigger mass than the one below. "Top" is
the tile row of the sprite's top (negative: into the tile to the north).

| tier | population | footprint (px) | top | content |
|---|---|---|---|---|
| I | 1-3 | about 26-28 x 17 | 11 | 2 or 3 houses and a bell-cote, outskirt houses; a few lit windows |
| II | 4-7 | about 30 x 23 | 5 | 4 buildings, a spire, outskirts at both ends |
| III | 8-12 | about 30 x 30 | -2 | the main hall and a tower |
| IV | 13-19 | about 33 x 33 | -5 | a great front, the tall tower, a chapter house |
| V | 20+ | 34 x 34 | -6 | a cathedral: twin towers, the crossing spire |

- The city's ground line is tile row 27 (its anchor 3 rows below, on row 30), so the plaque (rows
  29-35) never covers a door.
- **Daylight towns**: walls are lighter than roofs, so a town reads at 1x as warm walls under
  coloured roofs; windows amber `l` (recoloured to the civilization's light, 7.3); no ground pool
  for firelight towns (4.4).
- **Garrisons**: units fill the tile, so a garrison would hide its city. In a city the client
  draws the stack's figure only while it is selected or sliding; the count chip (top-right) marks
  the garrison.

### 9.2 City styles

Sixteen styles are generated from one kit (`cities/citystyles.mjs` with `cities/citykit.mjs`;
`node cities/citystyles.mjs` rewrites `cities.spr`). The kit's materials set walls (lit edge, body,
shade) and roofs (rim, lit half, dark half, tip); the hand-drawn stamps (doors, gates, crags,
frames) have their dark noir neutrals lifted two steps by day (`DAY_LIFT`), except in the styles
meant to be dark or faded.

| style | civilizations | signature |
|---|---|---|
| Cloister | Bannor, Elohim, Grigori, Kuriotates | cream walls (`8 7 G`) under red roofs (`R C I`) with gold tips `K`: the references' red-roofed towns |
| Amurite | Amurites | the cloister with a mage tower crowned by a lilac crystal; lilac windows; `ARC` pool |
| Calabim | Calabim | the cloister in darker stone under dark red roofs (not lifted) |
| Sidar | Sidar | the cloister faded to warm greys; no lit windows above tier II (not lifted) |
| Sun-domes | Malakim | cream walls, gold domes and a minaret |
| Elven | Ljosalfar | great trees (brightened one step, `t T o`) with lit hollows and platforms |
| Svartalfar | Svartalfar | the elven trees in `v p P` with lilac hollows; `ARC` pool |
| Dwarven | Khazad | a hold built out of the mountain: a stepped gatehouse, battlemented towers and ramparts in warm grey stone with a bronze band, ember chimneys, rock shoulders with ledged tops at its flanks. Its outline is flat ledges and upright walls, never a peak's slopes, so it cannot be read as a mountain beside the cool-grey peaks |
| Luchuirp | Luchuirp | the dwarven hold with a gear wheel on a tower and a golem forge |
| Tribal | Clan of Embers, Doviello, Hippus | cream hide tents, a longhouse, a palisade and a bonfire (`FIRE` radius 8, halved) |
| Barbarian | barbarian | grey pelt tents, a skull-topped palisade, a lookout, a bonfire |
| Harbour | Lanun | stilt houses, moored ships with square sails, a pier, a lighthouse from tier IV |
| Carnival | Balseraphs | striped pavilions in the owner colours (`#` `%`), a big top, lantern strings |
| Infernal | Infernal, Sheaim | black thorn spires, crimson cracks, scarlet windows; `BLOOD` pool (not lifted) |
| Celestial | Mercurians | white slender towers and domes with gold tips |
| Frost | Illians | rime and ice: white walls, an ice spire, rime windows |

### 9.3 Modifiers

- **Walls**: a low curtain wall in front, 3 px tall; **castle**: corner towers.
- **Capital**: the tallest spire carries a finial in the owner's field colour.
- **Holy city**: the religion's glyph on the plaque. **Disorder**: windows dark, an ash smoke
  column. **Razed**: CITY_RUINS.

### 9.4 Plaque

Below the city, overlapping the tile's bottom edge by 3 px: a navy plate 7 px tall. At the left a
5x7 **size chip** in the owner's field colour with the population in the Tally font; then the name
in Vigil in cream (white for your capital); at default zoom your cities add the production glyph (a
5 px mallet, `ui/plaque_production`) and the turns, so the digit cannot pass for a unit count. At
survey zoom only the chip is drawn. This is the references' "1 Rome" label.

---

## 10. Units

### 10.1 Frame, anchor, facing and states

- **The 32x32 frame** has its feet at (15, 29): an infantryman stands about 27 px from feet to
  crown, raised weapons reach the top rows, mounted units span the tile. Giants (war elephants,
  demon riders, dragons, balors) use the **40x40** frame, feet at (19, 37). Equipment glyphs are
  24x24 (feet 11, 21): items lying on the tile.
- All units face **right** (east) and are mirrored after a westward move.
- **One figure per stack**: the head unit (the best defender of a foreign stack, the selected unit
  of yours), on its team ring (7.4), with up to 2 offset echoes behind it as dark silhouettes.
- **Overlays** (`units/stack.js`), in the corners and on the top edge: the count chip (top-left,
  top-right in a city), a gold hero pip beside it, a 12x1 health bar in a void frame on the top edge
  (only when hurt or selected: cream above 50%, ochre from 25%, crimson below), state marks
  (fortified, sleeping, sentry, healing, automated, promotion ready) top-right. Moves show on the
  ring (quiet when your stack is spent); units without a ring show them as a pip bottom-right.
- **Selection**: a white rim round the figure; **garrisons** show only when selected or moving
  (9.1).

### 10.2 Where the figures come from: the unit atlases

The bespoke figures are cut from the owner's unit atlases by the pipeline in
`web/art/tools/atlas/` (Python 3 with numpy and Pillow; an authoring aid, not part of the build:
its `.spr` output is the source afterwards):

1. **Segment** (`extract_units.py`): the atlas panel is a dark, low-chroma navy (L* 4-18,
   chroma about 20-24), so the background is the connected navy regions that are large or touch the
   edge, found after a morphological opening (the flood cannot leak through gaps in an outline) and
   with every 200 px straight run cut (section rules); dark-cloaked figures survive because their
   cloth is more saturated, redder or lighter than the panel. Touching hex bases are split by a
   geodesic Voronoi of eroded seeds.
2. **Strip the base**: the grass (or water) top of each hex is found in the lower half; everything
   below the middle of its top surface goes, and grass and rim colours above it.
3. **Downsample** (`downsample.py`, `quantize.py`): one uniform scale per atlas, so relative sizes
   survive (a typical infantryman about 27 px), area-averaged in linear light with alpha-weighted
   colour; coverage of 50% or more is opaque. A figure too big for its frame is scaled down to fit.
4. **Quantize** to the palette (nearest in CIE Lab; L* under 9 is outline), clean isolated pixels,
   drop base residue at the feet (the bottom rows keep only the body's column span, and lose grass
   and rim colours), keep the main connected body, **enforce the outline** (4.2), reduce to 18
   colours (4.3), convert the cloth accent to owner slots (7.4), and place the feet on the anchor.
5. **Map** (`unit_mapping.json`): each crop to a game type, with options (`slots`, `frame`, `ship`
   to strip the water under a hull). Writes `units/atlas.spr`; `integrate_units.py` adds the source
   to the manifest and rewrites the variant remaps.

81 figures come from the atlases: the melee, archer, recon, mounted, siege and naval lines, the
civilians, the core animals, and stand-ins chosen for FfH units from unused atlas figures (adept:
the Dreamstalker; mage: the Pathfinder; archmage: the Avatar; battlemaster: the Greatsword; stygian
guard: the Dreadnought; mercenary: the Longswordsman; dragon slayer: the Slayer; royal guard: the
Cuirassier; crusader: the atlas Paladin; nightwatch: the Stealth Archer; shadowrider: the Risen
Rider; ophanim: the Unicorn Cavalry; death knight: the Dread Knight; chaos marauder: the Demon
Rider). Atlas names with no FfH unit (pikeman, rifleman, ...) are not used. Unique units borrow
their base unit's figure (mapping step 4, 15.2).

### 10.3 Variants by remap

Several units are an atlas figure recoloured by a manifest remap on the new keys (skin `a b`,
leather `i j h r`, greys `B D G H`): the polar bear (the bear in whites), the sand lion, the
bloodpet (the warrior in blood), lizardmen (green skin and olive hoods on the scout, archer and
beastmaster), frostlings (ice-blue skin), the ophanim's brightened steel, the shade and the eidolon
(the spectre darkened, or in violet), Rosier (the knight in crimson) and Saverous (the demon rider
in teal). Ghosts are the same figures through the translucency checker.

### 10.4 Composed figures

The units the atlas sheets do not show (demons, undead, golems, elementals, angels, dragons, sea
monsters, the heroes, the faiths drawn per religion, the great people, the special ships, the
spells and the 24 equipment glyphs) are composed by `tools/atlas/compose_units.py` at the atlases'
own scale and go through the same quantizer (10.2, steps 3-4), so they share the atlas look: the
same outline, ramps and colour budget. 161 sprites, in `units/composed.spr`.

- **From atlas parts** wherever a figure fits: recoloured along a palette ramp by lightness rank
  (`Layer.recolor`: a hue, chroma and lightness selection mapped onto, say, `cICR` for a demon's
  skin), mirrored, scaled (giants and dragons into the 40x40 frame), combined (a pride of lions, a
  pack of wolves), riders erased and the mount's back painted under the gap. The dragons are the
  atlas dragon in four colourings (Abashi black, Acheron red, Drifa frost, Eurabatres gold); the
  balor is the berserker grown, reddened and bat-winged; the kraken is the Eater of Dreams in sea
  red; Auric Ascended is the Dread Knight in frost with feathered wings; the heroes are unused atlas
  figures in their own colours with a mark of their own (Herne's antlers, Loki's gold horns, Valin's
  plume, Auric's crown, Wilboman's frost skin).
- **Painted** where nothing fits: ellipses, polygons and tapered strokes shaded as lit domes under
  the top-left light, posterized to a palette ramp, each with a dark rim so overlapping parts stay
  apart (`composelib.paint`): the skeleton, spiders, scorpion, gorilla, tortoise, golems, treant,
  elementals, sea serpents, gargoyle, manes, wraith, hawk, vines, the workboat and supplies, the
  spells and every equipment glyph. Accessories (feathered and bat wings, halos, horns, flames,
  staffs, beards, plumes, crowns, antlers) are painted onto atlas figures the same way.
- **Variants are separate sprites**, not manifest remaps: one golem per material, a disciple,
  priest and high priest per religion (the trim in the faith's colour, the robe dark for the Veil and
  the Overlords), a dragon per colour, the angel of death and the repentant angel, Aurealis, the ice
  and lightning elementals, the beastman, the tar demon, Meshabber.
- Figures with a flame or a glow carry `: light` on their brightest pixel: the fire elemental,
  Aurealis, the pyre zombie, balor, Hyborem, Wrath, the seraph, the disciple of Acheron, fireball and
  meteor (FIRE), the arcane barge and the lightning elemental (ARC).

`cut_missed.py` cuts figures the automatic extraction missed (they touched their card frame) by
hand-given boxes: the elephant of sheet 3. `integrate_composed.py` writes `units/composed.spr` and
the manifest's rules. Every unit type resolves to an atlas or a composed figure; no pre-revamp
drawing is left in the units set.

### 10.5 Naval and air

Ships (galley, trireme, caravel, privateer, frigate, man-o-war, sailors' dirge) come from the
atlas with the water of their hex stripped below the waterline; the airship hangs under its gas
bag. Ships stand on a team ring on the water. The galleon (khaki sails), queen of the line (40x40,
a long owner pennant), black wind (black sails, dark hull) and arcane barge (violet sails and a
glowing orb) are atlas ships recoloured; the workboat is painted. Sea monsters (kraken, sea serpent,
leviathan) rise out of a ring of foam.

---

## 11. UI

### 11.1 Principles

- **The map dominates.** Chrome hugs the edges in narrow bands; no panel covers the centre except
  modals.
- The look of the references' screens: **navy panels, gold rules, cream text, maroon title
  ribbons**, small gilt ornaments; owner names in their tincture.
- Information first: every number is legible at `u = 1`; ornament never sits on top of data.

### 11.2 Colours as CSS

Every palette colour is a custom property (`--c-void`, `--c-soot`, ... `--c-gold`), generated from
`palette.txt` by `ui/ui.js` (`paletteCSS`) and mirrored in `assets/app/style.css` for the moment
before the art loads; the chrome uses only those.

| role | colour |
|---|---|
| page background, panels | soot (navy) |
| raised panels | coal |
| frames and rules | gold `K` outside, bronze `j` inside |
| title ribbons | blood `c` with a gold rule beneath |
| secondary text | stone |
| body text | cream (bone `7`) |
| emphasis and titles | white (vellum `8`) |
| accents and links | gold |
| disabled | ash |
| modal backdrop | void at 70% alpha (the only alpha) |

Civ4 text markup maps to: highlight white; positive lichen; negative and red scarlet; warning
amber; unit, building and other link colours gold; anything else cream.

### 11.3 Panels and frames

- **Frame** (`ui/frame_panel`, a nine-slice): a gold `K` rule outside and a bronze `j` rule inside,
  1 art px each, on navy, with pale-gold nails at the corners. The map page and modal pages use the
  same rules (the modal adds a third and gilt lozenges at the corners).
- **Title**: Vigil in white on a maroon ribbon (`ui/frame_title`) with a 1 px gold rule beneath.
- **Dividers**: 1 px khaki, inset 4 px from the frame.
- **Modals** are pages: the wider frame, an illuminated initial, serif body text, options as a
  vertical list of buttons.

### 11.4 Buttons and controls

| state | fill | border | text |
|---|---|---|---|
| normal | navy, light along the top and left | ochre `y` | cream |
| hover | navy | gold `K` | white |
| pressed | navy with a dark inset | gold | white |
| disabled | navy | pewter `B` | ash |
| focus | (as current) | plus a dotted white ring, offset 1 | (as current) |
| primary (End Turn, Confirm) | navy | gold `K` with pale-gold nails | white, with an icon at left |
| danger (declare war, delete, raze) | navy | crimson | cream; hover adds an inner blood rule |

Sliders, lists (hover coal, selected coal with a 1 px cream bar), tabs (white underline when
active) and thin dark scrollbars follow the same colours.

### 11.5 Icons

**Grid.** Icons are drawn on a 16x16 grid (panel icons) and an 8x8 grid (inline in text, map
overlays, yields on tiles) by `tools/atlas/compose_icons.py`: each is painted once on the 16 grid
like the map sprites (4.3: lit shapes posterized to palette ramps, up to 8 colours, one void
outline), and its 8x8 version is the finished 16x16 icon reduced by 2x2 blocks (the majority
colour, the outline kept), redrawn by hand where the reduction loses the subject: the open codex,
and the inline yields and states that blur at that size (the wheat ear for food, the mallet for
production, a coin stack for gold, the domino mask, the lit and snuffed candles, the fist, the leaf
and the sickly bubbles, the crescent, the broken crown, the sun and scales of the alignments, the
crossed pick and spade).
Promotion seals keep their family rim (the `#` slot, recoloured by the mapping) around a shaded
navy face with the painted glyph. The mana runes are a notation, like the font, and keep their
shapes.

**Yields and commerce** (also the `[ICON_*]` set used by the game text):

| icon | glyph | accent |
|---|---|---|
| FOOD | a bound sheaf | ochre `y` |
| PRODUCTION | a hammer over an anvil | iron, with an `E` spark |
| COMMERCE / TRADE | a coin with a square hole | ochre `y`, gold `K` |
| GOLD | stacked coins and one standing | ochre `y`, gold `K` |
| RESEARCH | an open codex | vellum pages |
| CULTURE | a gilt letter on a lilac plaque | gold on `p`/`P` |
| ESPIONAGE | a lilac mask (drawn for completeness; espionage is locked off) | dusk `p` |
| HAPPY | a lit candle | amber `l` flame |
| UNHAPPY | a snuffed candle with smoke | ash smoke, `C` wick |
| HEALTHY | a green sprig | olive `o` / lichen `O` |
| UNHEALTHY | boils of miasma | dusk `p`, lilac `P` |
| GREATPEOPLE | a laurel round a gold star | olive, gold |
| BULLET | a 3x3 lozenge | stone |
| STRENGTH | a raised shield | (none) |
| MOVES | a boot | (none) |
| DEFENSE | a tower | (none) |
| ANGRYPOP | a clenched fist | `C` |

**Empire** (top bar):

| icon | glyph |
|---|---|
| turn and End Turn | a crescent moon, amber when it is your turn |
| Armageddon counter | an **eclipse**: a void disc with a corona of blood `c` below 50, crimson `C` from 50, scarlet `R` from 75, and scarlet with a glow `L` core pixel at 100 (the steps FfH itself uses, for example for the Planar Gate) |
| golden age | a vellum sun |
| anarchy | a broken crown |
| research needed | a codex with a star |
| idle city | an empty anvil |
| AI-controlled units idle | a sleeping crescent with a gear |

**Mana spheres**: 18 runes (plus raw MANA as a crystal), each in a lozenge frame. Colour gives the
family; the rune shape identifies the sphere.

| sphere | colour | rune motif |
|---|---|---|
| AIR | rime `F` | three wind strokes |
| BODY | sand `Y` | a hand |
| CHAOS | scarlet `R` | an eight-pointed burst |
| DEATH | bone `7` on void | a skull |
| EARTH | ochre `y` | a mountain glyph |
| ENCHANTMENT | lilac `P` | a star in a circle |
| ENTROPY | dusk `p` | a spiral inward |
| FIRE | amber `l` | a flame |
| ICE | frost `f` | a crystal |
| LAW | vellum `8` | scales |
| LIFE | wisp `W` | an ankh-like knot |
| METAMAGIC | lilac `P` with a vellum centre | two interlocked rings |
| MIND | needle `T` | an eye |
| NATURE | lichen `O` | a leaf |
| SHADOW | slate `4` on void | a crescent hood |
| SPIRIT | glint `g` | a rising wisp |
| SUN | glow `L` | a rayed disc |
| WATER | tide `N` with `g` | waves |

**Religions**: 7 glyphs on 16x16 and 8x8. The colour also fills the `@` accent of their disciples.

| religion | glyph | colour |
|---|---|---|
| FELLOWSHIP_OF_LEAVES | an oak leaf | lichen `O` |
| THE_ORDER | a radiant cross | vellum `8` |
| OCTOPUS_OVERLORDS | a tentacled eye | needle `T` |
| RUNES_OF_KILMORPH | a carved rune stone | ochre `y` |
| THE_ASHEN_VEIL | a veiled skull | crimson `C` |
| THE_EMPYREAN | a sun eye | glow `L` |
| COUNCIL_OF_ESUS | a hooded mask | dusk `p` |

**Other sets**:

| set | design |
|---|---|
| unit combat classes | the class silhouette of section 10.2 reduced to 8x8 |
| promotions | a 16x16 **seal**: a round frame whose rim gives the family, and a glyph inside; levels I to III as 1 to 3 pips under the glyph |
| specialists | the great-person attribute (section 10.2) on a plain disc; great specialists get a vellum ring |
| alignments | good: a vellum sun; neutral: stone scales; evil: a crimson crescent |
| buildings | none in the MVP; buildings are listed with their name and the yield or commerce icons of their effects |

Promotion seal rims by family:

| family | rim |
|---|---|
| combat lines | stone |
| spheres | the sphere colour |
| races and natures | bone |
| equipment | ochre |
| afflictions (poisoned, plagued, diseased, withered, rusted, enraged...) | crimson |
| heroic and divine | amber |

### 11.6 Typography

- **Vigil**, the hand-authored bitmap title font:
  - small-caps serif: cap height 7 px; small capitals (for lower-case input) 5 px; no descenders;
  - proportional advance of 4 to 6 px plus 1 px tracking; word space 3 px;
  - 1 px foot serifs on stems; strokes 1 px;
  - glyphs: A-Z, digits 0-9 (lining, 5x7), and `. , : ; ' - – ( ) / + × ! ? & #`;
  - drawn in vellum or cream, with an optional 1 px void drop shadow (down-right) over the map;
  - used for: panel titles, city names on plaques, modal initials (at 2x), big numbers in the top
    bar, the turn count.
- **Tally**: a 3x5 pixel font of digits `0`-`9` and `+ − / ×`. Used for: stack counts, size chips,
  yield numbers on tiles, path turn numbers, combat odds on the map.
- **Body text** is the system serif, never a web font:
  - stack: `font-family: "Iowan Old Style", "Palatino Linotype", Palatino, "Book Antiqua",
    "URW Palladio L", Georgia, serif`;
  - sizes: 15 px body, 13 px small, line-height 1.4, `font-variant-numeric: lining-nums
    tabular-nums` in tables and the top bar;
  - colour: cream on navy; never pure white, never below stone for anything a player must read.

### 11.7 Cursor, selection and path

- **Pointer cursors**, 16x16 at `u`, all with a void outline: pointer, move, attack (crossed
  blades, crimson), cast (a lilac star), invalid, busy.
- **Tile cursor**: four corner brackets standing 1 px outside the tile on every side (34x34
  marks anchored at 1,1), stone on hover, white on the selected tile; the bracket rides with a
  sliding unit. Tile marks are drawn after the unit sprites but under the city banners, plaques
  and stack chips, so a bracket never writes over a town's name.
- **Selected unit**: the white rim (4.2). **Path preview**: a dotted white line, a navy turn
  roundel with the turn number in Tally at each turn boundary, the part beyond this turn in stone,
  unreachable ends in an ash ring with a bar, attacks in crimson brackets with the odds on a chip.
- **Hover tooltip**: a navy panel with a gold border (`ui/frame_tooltip`).

---

## 12. What the references taught us (and what they did not)

Taken:

- a bright, saturated sea and sunny land with clearly separate biomes;
- crisp dark outlines round brightly shaded figures that fill their tile;
- bold, distinct owner colours on the units and the borders;
- coloured bases under the figures (here: the team ring);
- red-roofed towns with cream walls, labelled "1 Rome" on a dark plate;
- navy panels with gold frames and maroon header ribbons;
- the units themselves (section 10.2).

Not taken:

- the **hex grid** and the **hex bases**: tiles stay square (Civ4's plot grid), the ring replaces
  the base;
- the atlases' rendering noise: figures are quantized to the palette, cleaned and reduced to 18
  colours;
- the Civ4-only unit types (pikemen, riflemen, ...), the atlases' lettering, and their painted
  panel textures.

---

## 13. Sprite sources as text

All sprites live in text files next to the client, in `crates/ffh-play/web/art/` (section 15
describes the toolkit that loads them, with a few extensions to this format). One file holds
many sprites:

```text
; comment lines start with ';'
= unit/swordsman 32x32 15,29
.............00.................
...
```

- The header `= <name> <W>x<H> <anchor x>,<anchor y>` is followed by exactly H rows of exactly W
  characters.
- Characters:
  - a **palette key** (section 2.1);
  - `.` transparent;
  - owner slots `#` field, `%` shade, `*` charge;
  - `@` the per-unit accent (religion or sphere colour; it defaults to lilac for adepts).
- Names are paths: `terrain/grass`, `feature/forest_tree_a`, `unit/<UNIT_TYPE lower-case>`,
  `city/cloister_3`, `icon16/food`, `icon8/food`, `font/vigil/A`...
- Variants are suffixed: `_a`, `_b` for scatter variants; `_f1`, `_f2` for animation frames.
- Outlines are part of the sprite (`0`). The team ring, contact shadow, selection rim, pools,
  standards and plaques are added by the renderer, not drawn into sprites.
- Generated PNG sheets are build products and are not committed. Only small reference contact
  sheets (under 300 KB) may live in `docs/art/`.

---

## 14. Review checklist for new art

1. Uses only the 56 palette colours; owner slots only on cloth and banners; per-sprite colour limits
   (4.3).
2. Reads at 1x on grass, sand, water and snow: a dark outline all round, light edges outlined
   outside.
3. Unit: the 32x32 frame with the feet at (15, 29) (giants 40x40 at 19, 37), facing right; no
   colour-only distinctions between unit types.
4. Reserved colours (fire, crimson, pale green, purple) only for their meaning (2.2), within the
   budget (2.3).
5. Light from the upper left; shadows lean cool; by day no firelight pools (4.4).
6. Dithering ordered and purposeful; no noise; no anti-aliasing; integer scaling only.
7. Terrain is calm: at 2x a screen of land reads as land, with the strong shapes on the actors.
8. Nothing copies Civ4 or FfH2 art; atlas figures come only from the owner's own sheets.

---

## 15. Toolkit

The art toolkit turns the text sprites into canvas drawing for the client. It lives in
`crates/ffh-play/web/art/`, a directory owned by the art track; the client imports from it. Vanilla
ES modules, no dependencies; the node tools use only node's standard library.

```text
web/art/
  art.js           the public module: loadArt(), drawing functions, atlases, remaps
  compose.js       renderMap(): a whole view in the order of section 3.1; fromApiMap()
  mapping.js       game type -> sprite resolution with category fallbacks
  library.js       loads and merges palette, set list, manifests, sources, types.json
  spritefmt.js     parsers for every text format (shared by the browser and node)
  palette.txt      the 56 colours, the remap tables (2.4) and the tinctures (7.1)
  sets.txt         the sprite sets, in load order
  types.json       generated from data/xml: every type with the facts the mapping keys on
  terrain/ units/ cities/ ui/     one directory per set: manifest.txt + *.spr sources
  preview/         contact sheets and the sample scene (open with tools/serve.mjs)
  tools/           validate, check.sh, screenshot, serve, gen-types, bundle, sprtool, bake-terrain;
                   atlas/ (the unit-atlas pipeline and the revamp's texture recipes, section 10.2)
```

### 15.1 Sprite files (`*.spr`)

Section 13's format, extended with metadata lines between the header and the rows:

```text
; comment
= unit/_disciple 24x24 11,21
: note disciple: the hooded acolyte with a lantern
: accent T
: light 15,18 FIRE 5
........................
...                       (exactly 24 rows of exactly 24 characters)
```

- Header `= <name> <W>x<H> <ax>,<ay>`: the anchor is the point placed on the map (feet, base,
  trunk bottom, glyph baseline).
- Characters: a palette key, `.` transparent, `#` `%` `*` owner field, shade and charge, `@` accent.
- Metadata (all optional):

  | key | meaning |
  |---|---|
  | `types A B` | game types this sprite represents exactly (`UNIT_WARRIOR`, `TERRAIN_GRASS`...) |
  | `light x,y TABLE r` | a light pool (FIRE, BLOOD, RITE, ARC) of radius r at sprite pixel x,y, drawn on visible tiles |
  | `place x,y` | tile pixel where the anchor lands (features, improvements); default 16,28 / 16,29 |
  | `flat` | an improvement drawn in the ground pass, under uprights (farms) |
  | `flag x,y,w,h` | the rectangle in which an owner's division (per pale, fess...) is applied |
  | `finial x,y` | where a capital's finial goes on a city sprite |
  | `accent K` | the default colour of `@` |
  | `tags ...`, `note ...` | free text |

- Names are paths; the namespace fixes the size, checked by the validator:

  | namespace | size and anchor |
  |---|---|
  | `terrain/` | 64x64 seamless texture |
  | `hill/`, `peak/`, `feature/` | upright sprites (humps, masses, trees, single features) |
  | `route/` | the dot a road is stamped with |
  | `unit/` | 32x32 feet 15,29 (the standard frame), 40x40 feet 19,37 (giants), 24x24 feet 11,21 (equipment) |
  | `city/<style>_<tier>` | base on the bottom row; tiers 1-5 (section 9.1) |
  | `improvement/` | at most 34x34 |
  | `res/` | 7x7 glyph centred at 3,3; `res/_disc` the 11x11 seal |
  | `icon16/`, `icon8/`, `ui/mark_*` | 16x16, 8x8, 8x8 |
  | `sigil/` | 5x5 (or 7x7) charges in `*` |
  | `font/vigil/`, `font/tally/` | glyphs in `8`, anchor at the baseline; `x_sc` are small capitals |
  | `ui/` | the stack standard, the city banner and other chrome |

- Groups: `name_a`, `name_b`... are variants of `name` (picked per tile by hash); `name_f1`,
  `name_f2`... are animation frames (picked by `art.time`, 400 ms each). A rule targets the group.
- Names starting with `_` (`res/_food`) are generic: category fallbacks, not a game type.
- Outlines are part of the sprite. Team rings, contact shadows, selection rims, pools, standards,
  HP strips and plaques are added by the renderer.

### 15.2 Sets, manifests and the mapping

`sets.txt` lists the set directories. Each set has a `manifest.txt`, so four people can add sprites
in parallel without touching a shared file (only a new set edits `sets.txt`):

```text
set units
source atlas.spr
map unit race=UNDEAD -> unit/skeleton prio=15
map unit combat=MELEE -> unit/axeman
map unit UNIT_POLAR_BEAR -> unit/bear remap=uG,hD,iH,j8,rD
map unit * -> unit/worker
```

A rule is `map <kind> <conditions> -> <sprite|none> [options]`. A condition is a type name
(`UNIT_WARRIOR`, alternatives with `|`), `attr=value` (`|` alternatives, `attr!=value` negates) or
`*`. `none` means deliberately not drawn (LAND_WORKED). Kinds and the attributes they key on
(see `types.json`):

| kind | attributes |
|---|---|
| `terrain` | water, impassable, graphical, hell, twin, up, down |
| `hill`, `peak` | the plot's terrain (same attributes) |
| `feature` | impassable, graphical |
| `bonus` | cls (BONUSCLASS), cat (food, strategic, luxury, mana: a heuristic), health, happy |
| `improvement` | permanent, unique, goody, graphical, water, actsAsCity, spawn, civ, mana |
| `route` | (type only) |
| `unit` | cls, combat, domain, tier, special, race (list), hero, equipment, invisible, base |
| `civ`, `citystyle` | playable (citystyle targets a style name: `-> cloister light=P`) |
| `religion`, `unitcombat` | (type only) |
| `building` | cls, wonder (WORLD, NATIONAL, NONE), religion, defense |
| `promotion` | race, equipment, sphere, line, level, dispellable, expires, mutation |
| `specialist` | great |

Options: `remap=ab,cd` (recolour key a to b...; slots allowed), `accent=K`, `scatter=A-B`
(features: A to B sprites per tile), `rim=K` (resource class), `tincture=Name` (civ), `light=K`
and `pool=TABLE|none` (city style), `deep` (ocean texture), `double` (railroad), `ghost`
(translucent unit), `pool=TABLE:r` (improvement), `prio=N`.

Resolution for `(kind, TYPE)` (mapping.js): (1) a rule naming the type; (2) a sprite whose
`: types` lists it; (3) the naming convention (`unit/<type without UNIT_>`, `terrain/`, `feature/`,
`res/`, `improvement/`, `route/`, `sigil/<civ>`, `icon16/religion_<x>`, `icon16/promotion_<x>`,
`icon16/building_<x>`, `icon8/combat_<x>`, `icon16/specialist_<x>`); (4) for units, the same steps
for the default unit of the unit's class, so unique units borrow their base's sprite; (5) the
matching category rule with the most conditions (`prio` adds; ties go to the rule loaded first).
An exact match inherits the type-describing options (`rim`, `tincture`, `light`, `pool`, `scatter`,
`deep`) of the best category rule, so a new `res/wheat` keeps its food rim. So a bespoke sprite needs
no rule at all: drop `unit/warrior` into a units file and UNIT_WARRIOR (and its unique units) use it.

`types.json` is generated by `node tools/gen-types.mjs` from `data/xml` and committed (the browser
cannot read the XML). Regenerate it when the XML changes; the validator fails while it is stale.

### 15.3 art.js

```js
import { loadArt, TILE } from './art/art.js';
import { renderMap, makePlots, fromApiMap } from './art/compose.js';
const art = await loadArt();                 // or loadArt({ base: url }) or loadArt({ bundle: text });
                                             // installs the set passes (section 16.3)
art.scale = 2;                               // canvas pixels per art pixel, an integer (3.2)
art.setWorld({ width, height, wrapX });      // the warp noise wraps with the map
art.setIndexNames('terrain', names);         // let plots carry engine indices (from /api/data)
```

A `plot` is `{ x, y, terrain, plotType, feature, bonus, improvement, route, river, owner, vis,
city, at(dx, dy) }` in screen orientation (row 0 north): types as names or indices, `plotType`
PEAK, HILLS, LAND, OCEAN (or 0-3), `river` bit 0 = south edge, bit 1 = east edge (the API's
n_of_river and w_of_river), `vis` 0 unrevealed, 1 fogged, 2 visible, `at` the neighbour or null.
`fromApiMap(api, w, h, wrapX)` builds these from `/api/map` (flipping Civ4's y-up rows) and
`makePlots(map)` adds coordinates and `at`.

Drawing functions take the canvas position of the tile's top-left corner:

| function | draws |
|---|---|
| `drawTerrain(ctx, x, y, plot)` | ground through the domain warp, shoreline ink, foam, depth contour; void if unrevealed |
| `drawRelief(ctx, x, y, plot)` | hill humps or peaks, recoloured per terrain |
| `drawFeature(ctx, x, y, plot[, type])` | scattered or single feature sprites, their pools |
| `drawRiver(ctx, x, y, plot[, {s, e}])` | river ink on the south and east edges (meanders up to 8 px out of the tile) |
| `drawRoad(ctx, x, y, plot[, dirs8])` | dotted road to connected neighbours (N, NE, E ... NW) |
| `drawBorder(ctx, x, y, plot \| {n,e,s,w}, ownerStyle)` | solid border lines on the owner's side (8.1) |
| `drawResource(ctx, x, y, bonusType)` | the seal at tile 25,25 |
| `drawImprovement(ctx, x, y, type, plot)` | improvement with its pools (dark windows when not visible) |
| `drawCity(ctx, x, y, city, ownerStyle)` | style and tier sprite, pool, window light, finial, banner, plaque |
| `drawUnit(ctx, x, y, unitType, ownerStyle, state)` | team ring (or contact shadow), echoes, selection rim; the overlay (count chip, hero pip, health bar, mark, lantern reflections) from `units/stack.js`; a spent own stack's quiet ring |
| `drawFog(ctx, x, y, plot)` | the fog pass: REMEMBER remap with hatching, navy for unrevealed, ragged edges |
| `drawLight(ctx, cx, cy, table, r, {inner})`, `drawVignette(ctx, x, y, w, h)` | index-remap pools (daylight-scaled, 4.4) and the vignette (off by day) |
| `drawText(ctx, x, y, text, {font, color, shadow, smallCaps})`, `measureText` | Vigil and Tally |
| `icon(name)`, `iconFor(kind, type)`, `drawIcon` | icons as small cached canvases |
| `drawSprite(ctx, x, y, name, variant)`, `drawTileCursor` | any sprite by name; the tile cursor |

`city` is `{ size, name, capital, own, turns, disorder, visible, style, plaque, layer }`; `state` is
`{ facing, selected, hp, moves, own, count, mark, hero, inCity, translucent, accent, noPennant, layer }`;
`layer: 'sprite' | 'overlay'` splits a unit or city into the map part and the overlays drawn
above every unit. `ownerStyle(civ, {division, tincture, ...})` builds an owner style and
`assignOwners([{id, civ}])` applies the assignment of section 7.2.

Draw order matters because rivers and uprights spill into neighbouring tiles, and fog and pools
read the canvas back: ground for every tile, then rivers, roads, flat improvements, city pools,
uprights row by row from north to south, seals, borders, fog, vignette (none by day), units
(sprites, then the tile marks, then the city and stack overlays), the path preview.
`renderMap(art, ctx, map, { plots, owners, cities, stacks, cursor, x0, y0, tx0, ty0, cols, rows })`
does exactly that, clipped to the view, and reports per-phase times in `timings`. With
`only: 'terrain'` it draws what a terrain cache holds (everything up to the fog), with
`only: 'units'` the units, city overlays and cursor; a client blits its cache, calls
`drawVignette` on the view, then draws the units (`preview/scene.html?cache=1` does this). Create
canvases that fog and pools read back with `getContext('2d', { willReadFrequently: true })`.

Internals worth knowing: sprites are rasterised per variant (remap, owner tincture and division,
accent, FOG, silhouette, rim, translucency) and scale into shared atlas pages; remaps and fog read
the canvas back and map each colour to its palette index, so they work at any integer scale and
through a translate or integer-scale transform. Ground tiles are cached (index buffers keyed by the
3x3 neighbourhood). A 20x14 view at 2x renders in about one second in headless Chromium, most of
it terrain: the client should keep the terrain layer in an offscreen canvas per zoom level and
redraw only changed tiles (3.1).

### 15.4 Previews, screenshots and checks

- `node tools/serve.mjs [port]` serves the directory; open `/preview/`: a contact sheet per set
  (`sheet.html?set=terrain|units|cities|ui`), everything (`?set=all`, with the coverage table),
  every game type through the mapping (`?set=coverage`, fallbacks greyed), and the sample scene
  (`scene.html`, `?scale=1|2|3`): a 20x14 map with coast, a river, forests, hills, peaks, hell
  terrain, roads, three owners' borders, three cities, stacks, fog and the unrevealed edge.
- `node tools/screenshot.mjs <outdir> [page...]` screenshots every page with headless Chromium
  (the playwright package if it resolves, else the chrome binary under `PLAYWRIGHT_BROWSERS_PATH`)
  and fails on console errors. Screenshots are scratch: keep them out of the repo, except the small
  reference sheets in `docs/art/`.
- `tools/check.sh` is the test: every module passes `node --check`; `tools/test-units.mjs`
  (node's test runner) checks the parsers and the mapping's resolution order on synthetic data;
  and `tools/validate.mjs [-v]` checks the real sources: palette.txt equals the blocks of
  section 2; every sprite parses, uses only palette keys and slots and has its namespace's size;
  every rule's target exists and its options parse; types.json matches data/xml; the bundle
  loads the same library; and every type of every kind (terrains, hills, peaks, features,
  bonuses, improvements, routes, all units, civilizations, city styles, religions, buildings,
  promotions, unit combats, specialists) resolves to a sprite or to `none`. Colour counts over
  the limits of 4.3 are warnings. The script exits 0 when node is missing. The client crate can
  run it from `cargo test` with a test like:

  ```rust
  #[test]
  fn art_sources_validate() {
      let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/web/art");
      match std::process::Command::new("sh").arg("tools/check.sh").current_dir(dir).status() {
          Ok(s) => assert!(s.success(), "web/art/tools/check.sh failed"),
          Err(e) => eprintln!("art check skipped: {e}"),
      }
  }
  ```

- `node tools/bundle.mjs > art.bundle.txt` concatenates every source into one file
  (`loadArt({ bundle })`), for a server that embeds its assets.

### 15.5 Adding art

1. Draw in the set you own: add or edit a `.spr` file and list new files in the set's manifest.
   Name a bespoke sprite by the convention (`unit/swordsman`) or claim types with `: types`.
2. Drafts may be sketched without outlines and with short rows: `node tools/sprtool.mjs outline
   draft.spr` pads rows and adds the void outline to sprites whose header ends in `+outline`;
   `sprtool.mjs stats` counts colours, `sprtool.mjs mirror` flips.
3. New terrain textures can start from `tools/atlas/bake_textures.py` (the revamp's recipes, 5.2) or
   `node tools/bake-terrain.mjs <name>` (the original night recipes); the baked text is then the source
   and is edited by hand. New units from an atlas sheet go through `tools/atlas/` (10.2).
4. Run `tools/check.sh`, then look: `node tools/screenshot.mjs /tmp/art` and read the set's sheet,
   `coverage.png` and `scene.png` at 1x and 2x. Review against section 14.


---

## 16. Status

The revamp (October 2026) replaced the night look with this one: the palette (40 colours
recoloured, 16 added), the terrain textures and hills, the ground and light passes (daylight), the
borders, the unit figures (cut from the atlases and composed) on their team rings, the stack marks,
the city kit, every map object (features, peaks, landmarks, improvements, lairs, resources), the
icons and the UI chrome. `tools/check.sh` passes (every module parses, the unit tests pass, every
sprite and rule validates, every game type resolves); the client's `tools/web-check.sh` and the
headless `tools/web-smoke.mjs` pass with it, and so does the Rust suite (the art is embedded at
build time).

### 16.1 Coverage

1013 sprites (978 groups) in 19 source files. Units: 325 types, 298 exact (81 atlas figures and
161 composed ones, some with variant remaps), 20 by their base unit, 7 by a category rule, all of
them figures in the new look. Terrains 14/14, hills and peaks 10 each (4 deliberately none),
features 20/20, bonuses 58/58, improvements 79 exact (2 none), civilizations 23/23, city styles 22
(+1 category), religions 7/7, buildings 90 exact (80 by class icon), promotions 30 exact (231 by
category), unit combats 10/10, specialists 11/11.

### 16.2 What is new, what is kept

- **New in the revamp**: the palette and its remap tables; the 12 terrain textures; the hill
  mounds and their remaps; the 81 atlas units (`units/atlas.spr`) and the 161 composed ones
  (`units/composed.spr`, 10.4); the team ring and the corner stack marks; solid borders; daylight
  fog, navy unrevealed ground, no vignette; the daylight light pass; the city kit's materials (and
  the regenerated `cities.spr`); the features, peaks, landmarks, improvements and lairs
  (`compose_map.py`); the resource objects; the icons (`compose_icons.py`); the gold-on-navy chrome
  (`ui/chrome.spr`, `style.css`); the reference sheets.
- **Kept as designs** (re-tinted by the palette, not redrawn, on purpose): the two fonts; the
  heraldic sigils and the gonfalon and city banner shapes; the mana runes (a notation, stamped on
  the new mana stones, obelisks and seals); the chrome primitives of `ui/marks.spr` (path dots and
  turn roundels, tile brackets, odds chips, the pointer, move and invalid cursors); the road dots.

### 16.3 How the client calls the toolkit

`loadArt()` loads the sprites and the mapping, then installs the sets' **passes** (`PASSES` in
art.js): `terrain/ground.js` (the ground pass: blob terrain classes, a wider warp, Bayer transition
bands, sunlit clearings and cloud shade, organic fog through REMEMBER, pale paths, river verges,
wooded crests, sunlit wood rims), `units/stack.js` (the count chip, hero pip, health bar, marks, quiet rings and lantern
reflections) and `cities/citylight.js` (the light pass: three-ring pools for cities and every other
light, scaled by its DAYLIGHT table, 4.4). Each wraps or replaces some `Art`
methods, so the drawing
functions of 15.3 and `renderMap` produce the final look without further calls.
`loadArt({ passes: false })` gives the seed drawing (the set previews use it to install their own
module with their toggles). The pass modules are imported next to art.js, so serve the whole
`web/art` directory even when the sprites come from a bundle.

```js
import { loadArt, TILE } from './art/art.js';
import { renderMap, makePlots, fromApiMap } from './art/compose.js';
import * as UI from './art/ui/ui.js';

const art = await loadArt();                       // sprites, mapping, set passes
for (const [kind, names] of Object.entries(typeNames)) art.setIndexNames(kind, names); // /api/data
art.scale = zoom;                                  // 1 (survey) to 4; integers only
const owners = art.assignOwners(players);          // [{ id, civ }] -> Map(id -> owner style)
const map = fromApiMap(apiMap, w, h, wrapX);
const plots = makePlots(map);
UI.installCSS(art, { u, dpr });                    // chrome: frames, buttons, cursors, colours

// per zoom: the terrain layer into an offscreen cache (redraw only tiles that change) ...
renderMap(art, cacheCtx, map, { plots, owners, cities, only: 'terrain' });
// ... per frame: blit the cache, the vignette (a no-op by day), then units, city overlays, cursor
ctx.drawImage(cache, x0, y0);
art.drawVignette(ctx, x0, y0, viewW, viewH);
renderMap(art, ctx, map, { plots, owners, cities, stacks, cursor, x0, y0, only: 'units' });
```

- `cities`: `{ x, y, owner, size, name, capital, own, turns, disorder }`; `stacks`: one per tile,
  the head unit, `{ x, y, unit, owner, count, state }` with `state` `{ own, moves, selected, hp,
  mark, hero, facing }` (`moves` only for your own units; foreign stacks stay quiet). A stack on a
  city tile shows its count chip at the top-right (`inCity`, set by `renderMap` from `cities`); the
  client draws a garrison's figure only while it is selected or sliding (10.1).
- Canvases that fog and pools read back need `getContext('2d', { willReadFrequently: true })`.
- Map overlays from `ui/ui.js`: `UI.drawTileMark`, `drawPath`, `drawOdds`, `drawRange`,
  `drawTileYields`; panels and text: `UI.textSpan` (Vigil, Vigil Book), `UI.iconFor(art, kind, TYPE,
  16|8)`, `UI.promotionEl`, `drawButton`, `drawBar` (`ui/README.md` lists them all).
- Minimap colours: `art.resolve(kind, type).opts.mini` on the terrain, hill, peak and feature
  rules (`terrain/manifest.txt`); owned land takes the owner's field colour.
- Working examples: `preview/scene.html` (`?cache=1` the cached path, `?scale=1`, `?passes=0`
  the seed) and `ui/preview.html?ground&citylight` (a mock screen) draw this way.

### 16.4 Open points

None. The last one carried over from the first guide, the pass tables living in their modules, is
closed: they are `palette.txt` remap columns (2.4).
