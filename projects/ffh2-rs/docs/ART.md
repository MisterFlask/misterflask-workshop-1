# ART.md: style guide for the ffh-play client ("Monastic Pixel Noir")

This is the binding style guide for all art in the browser client (`crates/ffh-play`): map, units,
cities, icons and UI chrome. Every asset is **original** (no Civ4 or FfH2 art, nothing downloaded).
Sprites are authored **as text** (section 13) and turned into images by the client at load time or
by a std-only build step.

The direction comes from the user's mood board (`docs/art/mood-board.png`, v2). The board sets the
**mood**, not the UX. Its words, specific symbols, icons, panel layout and resource counters are
placeholders from the mock-up tool and are not copied. `docs/art/style-reference.png` is a
prototype study rendered while writing this guide (palette, terrain key, unit silhouettes, owner
pennants, a sample map). It validates the rules below. It is not final art.

---

## 1. Direction

**The map is a night-time manuscript.** It reads like an old chorographic map, lit by a candle:
ground drawn flat, in plan view; mountains, forests, towers and figures drawn upright, in
elevation; coasts, rivers, roads and borders inked. It should look like a page, not a game board.

Five principles, in priority order:

1. **Silhouette first.** Every unit, city and landmark must read as a dark shape with a lit edge at
   1x. If a sprite needs colour to be recognised, the sprite is wrong.
2. **Quiet ground.** Terrain is low-chroma colour fields with sparse marks. Emptiness and shadow
   carry the depth. No busy tile textures, no visible tile grid.
3. **Colour is earned.** The world is soot, ash, bone, midnight navy, weathered teal and moss. More
   saturated hues are reserved and mean something:
   - **firelight** (amber) is life: windows, torches, lanterns, campfires;
   - **muted crimson** is blood and corruption: hell terrain, demons, war, Armageddon;
   - **pale green** is ritual and the unnatural: sacred sites, undead eyes, world spells;
   - **dusky purple** is the arcane: mana, spells, mages.
4. **Light is local.** The moon lights everything faintly from the upper left. Warm light comes
   only from sources you can see, and it falls off in dithered pools.
5. **Restraint scales.** A typical screen has a dozen small lights, not a hundred. The more of
   something there is on a map (grass, forest, units), the quieter its drawing must be.

---

## 2. Palette

### 2.1 The 40 colours

The palette is closed: every pixel of every asset, and every UI colour, is one of these 40 colours.
UI chrome may use CSS alpha only for overlays (section 11.2). Each colour has a one-character
**key**, used in sprite text (section 13). Keys follow two patterns:

- digits `0`-`8` are the neutral value ladder, from 0 (darkest) to 8 (lightest);
- inside each colour family, the lower-case key is the darker partner of the upper-case key
  (`n`/`N`, `f`/`F`, `t`/`T`, `o`/`O`, `y`/`Y`, `p`/`P`, `e`/`E`, `l`/`L`, `c`/`C`, `s`/`S`).

L* is CIE lightness (0 to 100).

**Neutrals: soot, ash and bone.** Darks are cool, lights are warm.

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 0 | `0` | void | `#05080b` | 2 | outlines, unrevealed tiles, deepest shadow |
| 1 | `1` | soot | `#0c1115` | 5 | panel ground, ink shadows, fog hatching |
| 2 | `2` | coal | `#161c21` | 10 | shadow faces, dark cloth and metal |
| 3 | `3` | iron | `#252b2f` | 17 | armour, rock in shadow, dark stone |
| 4 | `4` | slate | `#373a3b` | 24 | tundra ground, rock, lit dark stone |
| 5 | `5` | ash | `#56524c` | 35 | moon rim on sprites, lit stone, borders of panels |
| 6 | `6` | stone | `#7d7366` | 49 | stone highlights, secondary text, road shadows |
| 7 | `7` | bone | `#b3a28c` | 68 | ink lines (coast, roads), body text, bone and cloth highlights |
| 8 | `8` | vellum | `#ddcfb4` | 84 | brightest neutral: selection, titles, specular dots |

**Midnight navy and frost.** Water, night, cold.

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 9 | `d` | deep | `#071219` | 5 | ocean ground |
| 10 | `n` | navy | `#0d1d2b` | 10 | coast ground, river ink |
| 11 | `N` | tide | `#193249` | 20 | ripples, foam line |
| 12 | `g` | glint | `#3d5868` | 36 | depth contour dots, river glints, ice and snow shadow |
| 13 | `f` | frost | `#717f86` | 52 | snow ground, ice |
| 14 | `F` | rime | `#aab5b6` | 73 | snow crests, ice edges, cold light |

**Weathered teal.** Conifers, marsh shadow, verdigris, faith and depths.

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 15 | `k` | teal-dk | `#0f2224` | 12 | conifer shadow side, ancient forest mass |
| 16 | `t` | teal | `#1c383d` | 22 | conifer body, marsh pools, priestly cloth |
| 17 | `T` | patina | `#3d6364` | 39 | verdigris, canopy highlight, Overlords accent |

**Moss and olive.** Living land.

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 18 | `q` | moss-dk | `#1f251c` | 14 | grass mottling, marsh ground, jungle shade |
| 19 | `m` | moss | `#363d31` | 25 | grassland ground |
| 20 | `o` | olive | `#4d5537` | 35 | grass tufts, reeds, jungle canopy |
| 21 | `O` | lichen | `#6c724a` | 47 | tuft tips, tundra lichen, the Vert tincture |

**Earth.** Plains, desert, wood and leather. Rust is the corrupted earth tone.

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 22 | `u` | umber | `#2e2620` | 16 | wood, leather, soil, broken lands ground |
| 23 | `Q` | dun | `#4f483e` | 31 | plains ground |
| 24 | `x` | rust | `#5f3a2a` | 28 | burning sands ground, rust, the Tenné tincture |
| 25 | `y` | ochre | `#76663f` | 44 | stubble, gilt, the Or tincture |
| 26 | `Y` | sand | `#8c8068` | 54 | desert ground |

**Dusky purple.** The arcane.

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 27 | `v` | bruise | `#1b1622` | 8 | shallows ground, arcane shadow |
| 28 | `p` | dusk | `#382d45` | 21 | fields of perdition ground, mage robes |
| 29 | `P` | lilac | `#6e5b80` | 42 | mana, spell light, the Purpure tincture |

**Firelight** (reserved).

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 30 | `e` | ember | `#5a2412` | 22 | outer glow ring, embers, burnt glints |
| 31 | `E` | flame | `#ad501d` | 45 | flame body, inner glow ring |
| 32 | `l` | amber | `#e2913c` | 67 | lit window, lantern |
| 33 | `L` | glow | `#f7d47e` | 86 | light core (1 to 2 px only) |

**Muted crimson** (reserved).

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 34 | `c` | blood | `#3a1216` | 12 | hell fissures, corruption haze |
| 35 | `C` | crimson | `#77252b` | 28 | demons, the Gules tincture, war, damage |
| 36 | `R` | scarlet | `#b33a3a` | 43 | corruption light core, demon eyes |

**Pale green** (reserved).

| # | key | name | hex | L* | use |
|---|---|---|---|---|---|
| 37 | `s` | barrow | `#22463a` | 27 | ritual haze |
| 38 | `S` | ritual | `#6aab88` | 65 | ritual circles, undead eyes |
| 39 | `W` | wisp | `#c4efc4` | 91 | ritual light core (1 to 2 px only) |

The same palette in machine-readable form (`index key name hex family`):

```palette
 0 0 void    #05080b neutral
 1 1 soot    #0c1115 neutral
 2 2 coal    #161c21 neutral
 3 3 iron    #252b2f neutral
 4 4 slate   #373a3b neutral
 5 5 ash     #56524c neutral
 6 6 stone   #7d7366 neutral
 7 7 bone    #b3a28c neutral
 8 8 vellum  #ddcfb4 neutral
 9 d deep    #071219 navy
10 n navy    #0d1d2b navy
11 N tide    #193249 navy
12 g glint   #3d5868 frost
13 f frost   #717f86 frost
14 F rime    #aab5b6 frost
15 k teal-dk #0f2224 teal
16 t teal    #1c383d teal
17 T patina  #3d6364 teal
18 q moss-dk #1f251c moss
19 m moss    #363d31 moss
20 o olive   #4d5537 moss
21 O lichen  #6c724a moss
22 u umber   #2e2620 earth
23 Q dun     #4f483e earth
24 x rust    #5f3a2a earth
25 y ochre   #76663f earth
26 Y sand    #8c8068 earth
27 v bruise  #1b1622 purple
28 p dusk    #382d45 purple
29 P lilac   #6e5b80 purple
30 e ember   #5a2412 fire
31 E flame   #ad501d fire
32 l amber   #e2913c fire
33 L glow    #f7d47e fire
34 c blood   #3a1216 crimson
35 C crimson #77252b crimson
36 R scarlet #b33a3a crimson
37 s barrow  #22463a ritual
38 S ritual  #6aab88 ritual
39 W wisp    #c4efc4 ritual
```

### 2.2 Where each family may be used

| family | allowed on the map | allowed in UI | never |
|---|---|---|---|
| neutrals | everything | everything | (none) |
| navy and frost | water, rivers, snow, ice, cold metal, night sky, Illian frost | water and cold icons, the Azure and Celeste tinctures | warm-lit surfaces |
| teal | conifers, marsh, ancient forest, verdigris, priestly cloth | Overlords and teal icons, the Cendrée tincture | large flat areas outside water and marsh |
| moss | grass, reeds, jungle, foliage | health, nature, the Vert tincture | demons, undead |
| earth | plains, desert, wood, leather, hills, rock, gilt (ochre) | food, commerce, gold, wood | (none) |
| purple | arcane: mana nodes, spell effects, mage cloth, ancient forest shade, fields of perdition, shallows | mana, spells, arcane promotions, the Purpure tincture | ordinary cloth on non-arcane units |
| firelight | light sources only: windows, lanterns, torches, forges, campfires, flames, burning sands glints, volcano | happiness, warnings, the End Turn glyph, the active-turn mark | large fills; anything that is not a light |
| crimson | blood and corruption: hell terrain fissures, demons and Infernal or Sheaim works, Armageddon, the Gules tincture, low HP | war, danger, losses, deficits, Armageddon, delete | decoration; good-aligned things |
| pale green | ritual and the unnatural: sacred landmarks, ritual circles, undead eye pixels, world-spell effects, Life mana | rituals (FfH projects), world spells, Life mana | vegetation; ordinary healing |

### 2.3 Value and colour budget

Measured on the prototype map. Treat these as targets for any full-screen view of a typical game.

- At least **70%** of lit-map pixels have L* at or below 25 (the darkest 17 colours of section
  2.1). The prototype map measured 75%, with 8% above L* 40, 1% above L* 60 and 0.7% in the
  reserved families.
- At most **10%** are above L* 40. At most **2%** are above L* 60 (bone, vellum, rime, glow, ritual,
  wisp, amber).
- The three reserved families together (fire, crimson, pale green) cover at most **2%** of the
  screen. Their light cores (`l L R W`) cover at most **0.3%**.
- A saturated colour never fills more than about 6 px in a row except in glows, rituals or hell
  features. If it does, it is decoration.

### 2.4 Remap tables: fog and light pools

The palette is indexed, so darkening and lighting are **index remaps**, never alpha blends. The
client builds a 40-entry lookup per table and applies it per pixel, using a 4x4 Bayer threshold
where a falloff is needed (section 4.4).

`FOG` darkens about two steps within the family and puts every light out. The four pool tables pull
the ground toward one reserved family: firelight (`FIRE`), corruption (`BLOOD`), ritual (`RITE`) and
the arcane (`ARC`).

```remap
key  FOG FIRE BLOOD RITE ARC
0    0   u    c     s    v
1    0   u    c     s    v
2    1   u    c     s    v
3    2   u    c     s    p
4    2   Q    c     s    p
5    3   6    C     S    p
6    4   7    C     S    P
7    5   8    C     S    P
8    6   8    R     W    8
d    0   n    c     s    v
n    d   N    c     s    v
N    n   g    c     s    p
g    N   f    C     S    p
f    g   7    C     S    P
F    f   8    C     S    P
k    1   u    c     s    v
t    k   Q    c     s    p
T    t   y    C     S    P
q    1   u    c     s    v
m    q   Q    c     s    p
o    m   y    C     S    p
O    o   Y    C     S    P
u    1   Q    c     s    v
Q    u   y    c     s    p
x    u   E    C     s    p
y    Q   Y    C     S    P
Y    5   7    C     S    P
v    1   u    c     s    p
p    v   Q    c     s    P
P    p   6    C     S    8
e    u   E    C     s    p
E    u   l    R     S    P
l    x   L    R     W    P
L    y   L    R     W    8
c    v   x    C     s    v
C    c   E    R     S    p
R    C   l    R     W    P
s    k   Q    c     S    p
S    s   y    C     W    P
W    T   8    R     W    8
```

---

## 3. Grid, projection and scale

### 3.1 Tiles and projection

- **Square tiles of 32x32 art pixels**, north up, matching Civ4's square plot grid. Map widths are
  always multiples of 4 plots, so the 64 px texture period divides every map and wrapping maps
  repeat seamlessly.
- **Ground in plan view; objects in elevation.** Terrain, water, roads, rivers and borders are seen
  from above. Trees, hills, peaks, buildings, improvements and figures stand upright, seen from the
  front, as on a medieval map.
- **Overhang.** An upright sprite stands on its tile and may rise into the tile to the north:
  - by up to **8 px**: peaks, ancient forest, cities of size tier I to III, most improvements;
  - by up to **12 px**: volcano, city tiers IV and V, Yggdrasil and other tall landmarks, huge
    units (dragons, giants, kraken).

  Sprites never extend below their tile, and never more than 4 px past its left or right edge.
- **Draw order.** The terrain cache (ground, water, rivers, routes, flat improvements, then upright
  features, peaks, improvements and cities, row by row from north to south) is drawn first. Then
  borders. Then units, again row by row from north to south, so terrain never hides a unit. Then
  overlays: pennants, HP strips, plaques, selection, path.

### 3.2 On-screen scale

- Art is drawn at 1x and shown only at **integer scales**: one art pixel is always a whole number
  of device pixels. Compute the factor as `zoom × devicePixelRatio`, rounded to an integer. Use
  `imageSmoothingEnabled = false` and `image-rendering: pixelated`. Never fractional, never
  smoothed.
- **Zoom steps** (on-screen pixels per tile at device-pixel ratio 1):

  | step | px/tile | what is drawn |
  |---|---|---|
  | chart | 8 | minimap-style: flat terrain-family colours, owner washes, city dots, stitched borders; no sprites |
  | survey | 32 (1x) | everything; plaques show the size only |
  | **default** | **64 (2x)** | everything |
  | close | 96 (3x) | everything |
  | study | 128 (4x) | everything |

- **UI scale** `u` is an integer (default 2 at device-pixel ratio 1). Every pixel-art UI asset is
  shown at `u`; CSS lengths in UI chrome are multiples of `u`.

### 3.3 Sprite sizes

| asset | canvas (art px) | notes |
|---|---|---|
| terrain texture | 64x64 per terrain, seamless | sampled through the domain warp (section 5.1) |
| feature, peak, hill | 32x32, plus overhang | scattered inside the tile |
| unit | **24x24** box | feet at (11, 21); placed at tile offset (6, 7), so feet land at tile y = 28 |
| huge unit | 32x32 box | feet at (15, 29); placed at tile offset (0, 0) with up to 12 px of overhang |
| city | up to 32 wide x (20 + overhang) | base on tile row 30; tiers in section 9 |
| improvement | up to 24x20 | quiet; no overhang except towers and castles (8) |
| landmark (unique improvement) | 32x32, plus overhang | may glow (section 6.4) |
| resource medallion | 11x11 disc, 7x7 glyph | bottom-right of the tile, centred at (25, 25) |
| owner pennant on a unit | 6x8, with pole | top-right of the tile |
| city banner | 7x12 | top-left of the city tile |
| UI icon | 16x16 (8x8 for inline and map icons) | section 11.5 |
| cursor | 16x16 | section 11.7 |

---

## 4. Drawing rules

### 4.1 Light

- **Moonlight from the upper left (north-west)**, cold and faint. Lit faces are one ramp step
  lighter, shadow faces one or two steps darker. Shadow faces lean cool: toward navy, teal or
  bruise rather than plain grey.
- **Warm local light** only from visible sources (section 4.4).
- **Contact shadow.** Every unit and freestanding object sits on a 50% checker ellipse of void,
  about 14x3 px, under its base. Ground marks never cast shadows.

### 4.2 Outlines and rims

- **Sprites** (units, cities, trees, peaks, upright improvements and landmarks) have a 1 px
  **void outline** all round. Do not use selective or coloured outlines.
- **Moon rim on units.** Outside the outline, on the top and left edges only, units get a 1 px
  **ash** (`5`) rim. It is what keeps a dark figure readable on dark ground (the prototype study
  shows figures vanish on ocean and forest without it). A selected unit's rim becomes vellum on
  all sides (section 11.7).
- **Ground** (terrain textures, flat marks, water) has no outlines. Linework on the ground is ink:
  shoreline, rivers, roads and borders (section 5).

### 4.3 Shading and colour count

- Two or three tones per material, plus outline. No pillow shading. Hue-shift along the ramp
  rather than blending families.
- Colour limits, outline and owner slots excluded: unit 8 colours; city 10; terrain texture 5;
  feature 6; icon 4 (one of them an accent).
- No anti-aliasing against the background; sprites have 1-bit alpha.

### 4.4 Dithering, glows and pools

- **Only ordered patterns**: a 50% checker, a 25% grid, and 4x4 Bayer thresholds for falloffs. No
  noise dither, no error diffusion.
- Where dithering is used:
  - **Mottling** in terrain textures (25% grid or 50% checker of the next darker colour in the same
    family, in blotches 4 to 12 px across, covering at most 30% of the tile);
  - **glow rings**;
  - **pools** (index remaps with Bayer falloff, section 2.4);
  - the **fog edge** and the **vignette**;
  - **translucency** (section 4.5).
- No dithering inside unit, icon and improvement sprites, except for glow rings and translucency.
  Hills, peaks and canopies may use a 50% checker on their shadow flank.
- **Point light** (lantern, window, torch): a 1 to 2 px core of `L` or `l`, a 1 px ring of `l` or
  `E`, then a `FIRE` pool, with a radius depending on the source:

  | source | `FIRE` pool radius |
  |---|---|
  | window | none (a window is just the lit pixel) |
  | lantern or torch | 5 to 6 px |
  | campfire or forge | 8 px |
  | city | 12 + 3 x size tier (two passes: the outer pass at full radius, the inner at half radius) |

  The pool is elliptical (vertical radius divided by 1.4) and centred on the source's ground point.
- **Corruption** (volcano crater, Hellfire, Infernal cities, Broken Sepulcher...) uses `BLOOD`.
  **Ritual** sites use `RITE`. **Mana** nodes and spell effects use `ARC`.
- **Pools never cross fog.** Lights are drawn only on visible tiles: what you saw last turn is
  remembered dark.

### 4.5 Translucency and motion

- 1-bit alpha only. Translucency is a 50% checker of the sprite over what is behind it. Used for:
  - illusions and puppets;
  - your own invisible units;
  - ghosts (Spectre, Wraith, the ghost ship Sailor's Dirge);
  - move and placement previews.
- **Motion is scarce.** Allowed animations:

  | what | how |
  |---|---|
  | fire flicker | 2 frames, 400 ms: swap the core and ring pixels |
  | water ripples | marks shift 1 px every 800 ms |
  | ritual glow | 3-frame pulse, 600 ms |
  | path dots | march 1 px every 150 ms |
  | unit move | slides tile to tile at about 8 tiles/s; no walk cycles |
  | combat | 2-frame lunge toward the target; the loser's rim flashes crimson for 1 frame |
  | death | Bayer-dissolve to void over 6 frames |

  Nothing else moves. Idle units are still.

---

## 5. Terrain

### 5.1 How ultra-minimal terrain works

1. **One seamless 64x64 texture per terrain type.** Each texture has:
   - a **low-chroma base** colour;
   - **mottling** in the family's darker partner;
   - a few **signature marks**, about one mark per 40 to 80 px².

   Hue comes mostly from the marks. The base stays close to grey.
2. **A domain-warped mask picks the texture per pixel.** A pixel at `(x, y)` shows the terrain of
   the tile under `(x + wx, y + wy)`, where `w` is smooth value noise (a deterministic hash of the
   map coordinates): amplitude 6.5 px at cell 22 px, plus a 1.5 px detail octave at cell 7, so
   `|w| ≤ 8 px`. Edges between terrains, and coastlines, become organic, while the centre 16x16 of
   every tile always shows its own terrain, so the tile you click is the terrain you see. Marks
   flow across edges because the textures are in map space.
3. **Uprights are scattered per tile**, with a deterministic hash of the plot coordinates: trees,
   hills, peaks.
4. The result is cached per zoom level and invalidated per tile.

### 5.2 Base terrains

Each hell terrain is the **corrupted twin** of a normal one (`TerrainUp` in the XML). The twin keeps
its twin's mark *shapes* and swaps the colours to rust, purple and crimson, so a player can tell
what the land used to be.

| terrain (`TERRAIN_*`) | base | mottling | signature marks | reads as |
|---|---|---|---|---|
| GRASS | moss `m` | moss-dk `q` | three-pixel tufts `o O o` (a small inverted V) | green-grey |
| PLAINS | dun `Q` | umber `u` | horizontal stubble dashes, 2 to 4 px, alternating `y 5`, with a `u` pixel under | warm grey-brown |
| DESERT | sand `Y` | stone `6` | dune arcs, 5 to 8 px, crest `7`, underside `y` | pale sand: the lightest land |
| TUNDRA | slate `4` | iron `3` | lichen dots `O`/`o`, short crack lines `2` with a `5` lip | cool grey with specks |
| SNOW | frost `f` | glint `g` | drift lines `g`, 4 to 8 px, with `F` crest pixels | pale blue-grey |
| MARSH | moss-dk `q` | teal-dk `k` | water slits `n` with one `N` glint, reed ticks `o`/`O` | dark wet green |
| BROKEN_LANDS (hell grass) | umber `u` | iron `3` | jagged cracks `1` with `c` lips, ash pebbles `5` | cracked dark earth |
| FIELDS_OF_PERDITION (hell plains) | dusk `p` | bruise `v` | furrow dashes `c` with `P` flecks, bone-dust `6` | purple ash fields |
| BURNING_SANDS (hell desert) | rust `x` | umber `u` | dune arcs in `e` with an `E` crest; rare 1 px `l` glints | smouldering sand |
| SHALLOWS (hell marsh) | bruise `v` | soot `1` | stagnant slits `c` with one `C` pixel, dead reeds `4` | black-purple bog |
| COAST | navy `n` | deep `d` streaks on every third row | ripple dashes `N`, 2 to 5 px | midnight water |
| OCEAN | deep `d` | void `0` streaks on every third row | sparse ripple dashes `n`, rare `N` | near-black water |

`TERRAIN_PEAK` and `TERRAIN_HILL` are graphics-only types in the XML. On the map, peaks and hills
are **plot types** drawn over the plot's real terrain (section 5.3).

### 5.3 Hills and peaks

- **Hills** (`PLOT_HILLS`): two low humps per tile (about 20x8 and 18x7 px), in the terrain's own
  colours:
  - body one step darker than the ground;
  - shadow flank checkered one step darker again;
  - a lit crest line on the left two thirds of each hump (the terrain's light mark colour: `O` on
    grass, `6` on plains, `8` on desert, `F` on snow, `P` on perdition...);
  - no outline at the base, so the humps grow out of the ground.

  Forests and other features stand on top of the humps.
- **Peaks** (`PLOT_PEAK`): one or two pointed masses, 20 to 24 px wide, rising up to 8 px into the
  tile to the north. Colours and marks:
  - lit (west) face `4`/`5`, with a `6` ridge line;
  - shadow face `3`/`2`, with `1` hatch strokes every 3 rows;
  - void outline;
  - a cap of 3 rows (`6`, and `4` in shadow) on most terrains; on snow and tundra the cap is 7 rows
    (`7`, and `f` in shadow).

  They are the tallest, darkest shapes on the map, after cities.

### 5.4 Water edges

- **Shoreline ink.** A land pixel next to water is drawn bone `7` (with a `6` pixel every fifth
  pixel), and the land pixel behind it coal `2`. This lit rim is the strongest line on the map; it
  draws the continent like a pen. Water within 2 px of land gets a 50% `N` foam checker.
- **Depth contour.** Where ocean meets coast, a dotted `g` line runs on the ocean side (one dot in
  four pixels), like a chart's sounding line.
- **Lakes** are coast tiles, drawn the same way.
- Light sources on water cast a vertical reflection: 3 to 4 dashes of `E` then `e`, 2 px apart,
  under the source.

### 5.5 Features

| feature (`FEATURE_*`) | on | drawing |
|---|---|---|
| FOREST | grass, plains, tundra | 9 to 12 conifers, 7 to 12 px tall, scattered and sorted by y, up to 3 px into neighbours. Tiers of `t`, shadow side `k`, lit edge `m`; 70% teal, 30% moss (`q m o`); umber trunk pixel. A dense dark mass, not a plantation. |
| FOREST_ANCIENT | grass, plains, tundra | 4 to 6 huge merged canopies (`k`, lit edges `t` then `T`, underside `v` dusky purple), 3 visible trunks; rises 8 px. Old, dark, faintly arcane. |
| FOREST_NEW | grass, plains, marsh, tundra | 2 or 3 saplings at 55% scale. |
| FOREST_BURNT | grass, plains, marsh, tundra | 3 or 4 bare charred trunks (`2`/`3`, ash `5` tip); no embers, the fire is out. |
| JUNGLE | grass, marsh | 8 or 9 rounded canopy clumps (`m`, shadow `q`, lit `o`), touching, with void edge pixels. Darker and rounder than forest. |
| FLOOD_PLAINS | desert | 5 ruled bands of `o`/`m` dashes with `u` silt under; reads as green ruled lines on sand. |
| OASIS | desert | a pool ellipse (`n`, upper rows `N`, `g` glint) plus one palm (umber trunk, `o` fronds). |
| ICE | coast, ocean | 3 or 4 rime slabs (top row `F`, body `f`, bottom two rows `g`), black water between. |
| SCRUB | desert | 4 dry bushes (`u` stems, `o` top pixel). |
| FLAMES | burning sands | 2 or 3 small fires, 3x5 (`E e l L`), 4 px `FIRE` pool; 2-frame flicker. |
| OBSIDIAN_PLAINS | burning sands | 4 black glass shards (`0`/`2`) with a `P` glint at the tip. |
| VOLCANO | many | a soot cone 24 px wide (lit side `3`, shadow `2`), crater rim `C` with an `R` core, `BLOOD` pool radius 10, ash plume (`4`/`5`), rising 12 px. Impassable. |
| BLIZZARD | snow, tundra | about 14 diagonal `F`/`f` pixel pairs over the tile; with ripple-speed drift if animated. |
| FALLOUT | (not created in FfH play) | fallback: 25% `s` speckle. |
| TORMENTED_SOULS, DOOR_*, WALLS | scenario-only | fallback glyphs: pale `S` wisps; a stone arch (closed: `0` fill; open: `n`); a `4`/`5` block wall. |

### 5.6 Rivers and routes

- **Rivers** run on tile edges (`n_of_river` gives the south edge, `w_of_river` the east edge). Each
  is a **2 px ink line** of navy `n`, with a `g` glint pixel every 6 px and a 1 px soot `1`
  border. The line follows the terrain warp (it is displaced by −0.8 × the warp vector), so it
  meanders with the land. It joins water bodies; where river edges meet at corners, it rounds them.
- **ROUTE_ROAD**: a dotted ink line from the tile anchor (16, 18) to each neighbour that has a road
  (8 directions). One bone `7` dot every 3 px, with a coal `2` shadow pixel below-right. This is the
  board's path motif.
- **ROUTE_RAILROAD** (in the data, rarely seen): two parallel dotted lines 2 px apart, in stone `6`.
- Roads enter cities to the city's gate point.

### 5.7 Improvements: quiet marks

Improvements are seen at the tile's lower half. They use at most 4 colours plus outline and stay
below the value of the ground's light marks, except where a window or fire is lit. Unrevealed
improvements are not drawn; fogged ones are remembered (drawn through `FOG`).

| group | improvement (`IMPROVEMENT_*`) | mark |
|---|---|---|
| fields | FARM | 4 to 6 ruled furrow rows (`u`/`Q` dashes), flat |
| | PASTURE | dotted stone fence loop, 2 animal pixels |
| | PLANTATION | a dot grid of `o` in rows |
| | WINERY | trellis: umber posts with `o` dots |
| | CAMP | a hide lean-to (`5`/`u`) with a 1 px campfire |
| | FISHING_BOATS, WHALING_BOATS | a tiny boat (umber), net dots; a harpoon line for whaling |
| | PENGUINS, MUSHROOMS | 3 to 5 tiny figures or caps (`8`/`6`) |
| works | MINE | an adit (void square, umber frame) with an ash spoil heap |
| | QUARRY | stepped cut blocks (`6`/`5`) |
| | LUMBERMILL | stacked logs (`u`) with a saw wheel |
| | WORKSHOP | a small hall, ash smoke, 1 forge pixel `E` |
| | WINDMILL | mill with 4 bone sail lines |
| homes | COTTAGE, HAMLET, VILLAGE, TOWN | 1, 2, 3, then 5 huts (roof `5`/`3`, wall `3`/`2`), lit windows 0, 1, 2, then 3 (`l`); a TOWN adds a small spire. The growth is visible as more light. |
| defence | FORT | a palisade square of umber dots |
| | TOWER | a skeletal watchtower: open timber frame, one lantern, 8 px overhang |
| | CASTLE, CITADEL | a stone keep, 2 towers for Citadel, owner pennant on top |
| FfH | MANA_* (18) | a slate obelisk with the sphere's rune in its colour (section 11.5), `ARC` pool radius 6 |
| | ENCLAVE | a small walled garden |
| | PIRATE_COVE, PIRATE_HARBOR, PIRATE_PORT | a stilted shack with a sable flag, growing like cottages |
| | NECROTOTEM | a bone totem with a skull, `S` eye pixel |
| | JUNGLE_ALTAR | a stepped altar with a `C` top |
| | GRAVEYARD | 4 headstones (`4`/`5`) |
| | WARNING_POST | a stake with a skull |
| | CAGE | an iron cage (`3` bars) |
| | PORTAL | a stone ring with a `P` centre, `ARC` pool |
| | SMOKE | an ash smoke column |
| | CITY_RUINS | broken walls (`4`/`5`), with no light at all |
| hidden | LAND_WORKED, WATER_WORKED | not drawn |

### 5.8 Lairs and goody huts

Lairs (`bPermanent` and spawning units) are ominous but small: no light, or a single ember.

| lair | drawing |
|---|---|
| BARROW | an earth mound with a void door and one `S` pixel |
| BEAR_DEN, LION_DEN | a rock cave mouth with a bone pixel |
| GOBLIN_FORT | a spiked umber palisade with a campfire |
| RUINS | broken columns with vine `o` |
| DUNGEON | stairs into void, with an `e` glow |
| HELLFIRE | a pit with a `BLOOD` pool and an `R` core |
| SHIP_WRECK | a broken hull on the shore |
| SNAKE_PILLAR | a coiled pillar |
| MONSTER_SKELETON | a huge bone ribcage |

GOODY_HUT is a small cairn with **one lit candle**: a point of life worth walking to.

---

## 6. Resources and landmarks

### 6.1 Resource medallions

A resource (`BONUS_*`, drawn only when revealed by tech for the viewer's team) is a **seal on the
map**: an 11x11 disc of soot `1` with a 1 px rim, at the tile's lower right, and a 7x7 glyph
inside. The rim colour gives the class at a glance; the glyph names the resource.

| class | rim | resources and glyph motifs |
|---|---|---|
| food | olive `o` | CORN (cob), RICE (bent sheaf), WHEAT (sheaf), COW (horned head), PIG (snout), SHEEP (curl), TOAD (frog), BANANA (bunch), DEER (antlers), FISH (fish), CLAM (shell), CRAB (crab), WHALE (fluke) |
| strategic | stone `6` | COPPER (ingot, `x`), IRON (ingot, `4`), MITHRIL (ingot, `F`), HORSE (horse head, `7`), NIGHTMARE (horse head, `R` eye), GUNPOWDER (keg), REAGENTS (vial, `P`), SHEUT_STONE (black stone, `C` vein), MARBLE (block, `8`) |
| luxury | ochre `y` | DYE (drop, `P`), FUR (pelt), GEMS (cut stone), GOLD (nugget, `Y`/`L` pixel), INCENSE (smoke curl), IVORY (tusk), SILK (bolt), SUGAR (cane), WINE (grapes), PEARL (pearl), COTTON (boll), ALE (tankard), FINE_CLOTHES (folded cloth), JEWELS (ring), RAZORWEED (thorn stalk), GULAGARM (sealed jar), FRUIT_OF_YGGDRASIL (fruit, `W` pixel) |
| mana | lilac `P` | MANA (raw: a lilac crystal) and the 18 spheres (section 11.5) |

Medallions are flat: no overhang, no light, except the single core pixel of FRUIT_OF_YGGDRASIL,
GOLD and the mana glyphs.

### 6.2 Landmarks (unique improvements)

Unique improvements are the **extraordinary** made visible. They are the only map objects, apart
from lit windows, flames, volcanoes and mana nodes, allowed a full accent glow. Each is drawn once,
32 px wide, with up to 8 to 12 px of overhang.

| landmark | accent | pool |
|---|---|---|
| STANDING_STONES | 5 slate menhirs around a pale-green ring (the board's ritual circle) | `RITE` |
| POOL_OF_TEARS | a still pool with `S` ripples | `RITE` |
| SEVEN_PINES | 7 tall conifers, each with a `W` mote | `RITE` |
| YGGDRASIL | a giant tree, rising 12 px, with `S` glints in its crown | `RITE` |
| BRADELINES_WELL | a stone well with a `W` core | `RITE` |
| TOMB_OF_SUCELLUS | a low tomb with an `S` door glow | `RITE` |
| AIFON_ISLE | an islet with one pale light | `RITE` |
| MIRROR_OF_HEAVEN | a vellum pool reflecting glow `L` | `FIRE` (cold: inner radius only) |
| PYRE_OF_THE_SERAPHIC | a pyre with a vellum flame core | `FIRE` |
| BROKEN_SEPULCHER | a split sarcophagus, `R` crack | `BLOOD` |
| RING_OF_CARCER | a ring of black stones, `C` centre | `BLOOD` |
| ODIOS_PRISON | an iron cage tower, `R` within | `BLOOD` |
| LETUM_FRIGUS | an ice cairn with `F` light | none (rime pixels only) |
| MAELSTROM | a whirlpool spiral of `N`/`g` on water | none |
| GUARDIAN | a colossal statue (`5`/`6`), `S` eyes | `RITE` (radius 4) |
| DRAGON_BONES | a dragon skeleton in bone `7`, no glow | none |
| REMNANTS_OF_PATRIA | ruined walls in ash, no glow | none |

---

## 7. Owner identity

Up to about 20 players must be told apart in a restricted look. Identity is **heraldic**, built
from three parts:

- a **tincture**: one of 10 muted field colours;
- a **division** of the field, which separates players who share a tincture;
- a **sigil**: one charge per civilization.

### 7.1 Tinctures

| tincture | field | shade | charge colour | border stitch |
|---|---|---|---|---|
| Argent | bone `7` | stone `6` | soot `1` | `7` over `6` |
| Or | ochre `y` | umber `u` | soot `1` | `y` over `u` |
| Gules | crimson `C` | blood `c` | vellum `8` | `C` over `c` |
| Azure | glint `g` | navy `n` | vellum `8` | `g` over `n` |
| Vert | lichen `O` | olive `o` | soot `1` | `O` over `o` |
| Purpure | lilac `P` | dusk `p` | vellum `8` | `P` over `p` |
| Tenné | rust `x` | umber `u` | bone `7` | `x` over `u` |
| Celeste | frost `f` | glint `g` | soot `1` | `f` over `g` |
| Sable | coal `2` | soot `1` | bone `7` | `2` over `1`, with a bone tick on every dash |
| Cendrée | patina `T` | teal `t` | vellum `8` | `T` over `t` |

Gules is the one sanctioned use of crimson as an owner colour. It goes by default to the
blood-and-corruption civilizations.

### 7.2 Divisions and assignment

Divisions, in order:

1. **plain**;
2. **per pale**: the right half of the field is Sable, or Argent if the field is Sable;
3. **per fess**: the same, on the lower half;
4. **per bend**: the same, diagonally;
5. **quarterly**.

On borders, a divided owner's stitch **alternates** dashes of the field colour and the second
colour.

Assignment, at game start, in player-slot order: give each player its civilization's preferred
tincture, plain. If that pair is taken, use the next division of the same tincture. Ten tinctures
times five divisions gives 50 distinct identities. The sigil always follows the civilization, so
two Bannor players are two argent banners with the same sword, one of them divided.

Barbarians are not assigned. They fly **Sable with a bone skull**, draw no borders, and animals
fly no pennant at all.

### 7.3 Civilizations

Sigils are drawn in the charge colour, 5x5 (on pennants and plaques) or 7x7 (on banners and
diplomacy screens). They are simple heraldic charges designed for this game.

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

- **Borders** (section 8.1).
- **City banner**: 7x12, top-left of the city tile, with the sigil. **City plaque**: tincture chip
  (section 9.4).
- **Unit pennant**: a 6x8 swallow-tail on a pole at the tile's top-right, with field and division
  only (no sigil: it is too small). One pennant per stack. A stack containing a hero gets a gilt
  finial (`y` with an `L` pixel) on the pole.
- **Unit cloth**: every unit sprite marks its cloth (tabard, shield face, caparison, sail emblem,
  robe trim) with the owner slots `#` (field), `%` (shade) and `*` (charge), filled at draw time.
- Units with hidden nationality (privateers, pirates, assassins with the promotion) show **no
  pennant**, and their cloth slots render as Sable.

---

## 8. Political layer, fog and the edge of the world

### 8.1 Borders: the stitched ribbon

Borders are drawn on tile edges where the revealed owner changes, **on the owner's side**, as a
3-row ribbon:

- outer row: void `0`, the underlay that keeps the stitch readable on pale ground;
- middle row: the tincture's stitch colour;
- inner row: the tincture's shade.

The pattern is dashed, **4 on and 2 off**, phased by map coordinate, so dashes meet cleanly at
corners. Two owners sharing an edge show two ribbons back to back: a double stitch. Borders are not
warped; they are political, not natural.

The **political layer** (toggle) adds a wash: every owned pixel of ground passes through a 25%
grid of the owner's shade.

### 8.2 Fog of war

Fogged tiles (revealed, not visible) are:

- passed through `FOG` (section 2.4), so terrain keeps its family but drops two steps and every
  light goes out;
- crossed by **ink hatching**: every pixel with `(x + y) mod 4 = 0` passes through `FOG` twice;
- remembered: last-seen improvements, routes and cities (with their windows dark), but no foreign
  units.

The fog edge follows the terrain warp at 0.6 × amplitude, so the line between seen and remembered
is ragged like worn ink.

### 8.3 Unrevealed tiles and the vignette

- Unrevealed tiles are **void**. Their edge is ragged: an independent warp of the same amplitude
  shifts the boundary.
- **Viewport vignette**: beyond 75% of the half-diagonal from the screen centre, pixels pass
  through `FOG` with a Bayer threshold rising to full at the corners. The map fades like a page read
  by candlelight.
- The map viewport sits in a 1 px ash `5` frame with a soot margin, like the edge of a page.

### 8.4 Minimap

- One pixel per tile, or two on small maps.
- Colours:
  - water: `d` for ocean, `n` for coast;
  - unowned land: the terrain base;
  - owned land: the owner's field colour;
  - cities: a vellum `8` dot;
  - fog: through `FOG`;
  - unrevealed: void.
- The viewport is a 1 px vellum rectangle.
- Frame: as for panels (section 11.2).

---

## 9. Cities

### 9.1 Size tiers

| tier | population | footprint (px) | content |
|---|---|---|---|
| I | 1-3 | 16 wide, 12 tall | 2 or 3 buildings, 1 lit window |
| II | 4-7 | 20 wide, 16 tall | 4 or 5 buildings, a chapel spire, 2 or 3 windows |
| III | 8-12 | 24 wide, 20 tall, overhang 4 | a dense cluster, 2 spires, 4 or 5 windows |
| IV | 13-19 | 28 wide, 24 tall, overhang 8 | a great hall or cathedral, 6 or 7 windows |
| V | 20+ | 32 wide, 28 tall, overhang 12 | the whole tile, the tallest spire, 8 windows |

Buildings are dark masses: lit faces `5`/`6`, shadow `3`/`2`, void outline. The facade nearest a
window takes the `FIRE` remap for 1 px around it, so stone looks lit from within. The city's ground
pool follows section 4.4. Windows are 1x1 or 1x2 in the style's light colour (section 7.3), with at
most one `L` core pixel per building.

### 9.2 City styles

Ten restrained styles are built from one kit. Each changes only roofs, the landmark piece and the
window light.

| style | civilizations | signature |
|---|---|---|
| Cloister | Bannor, Elohim, Grigori, Kuriotates, Amurites, Calabim, Sidar | gothic stone, pointed roofs and spires (the board's abbey); Amurites add a mage tower with a lilac top; Calabim use darker stone (`2`/`3`); Sidar are faded |
| Sun-domes | Malakim | flat roofs, low domes, sand stone (`Y`/`6`), a tall minaret |
| Elven | Ljosalfar, Svartalfar | 2 or 3 great trees (`k t T`) with lit hollows and platforms; Svartalfar trees in `v p` with lilac light |
| Dwarven | Khazad, Luchuirp | halls cut into a rock mound: carved door arches, chimneys with ember tops; Luchuirp add a gear wheel and a golem forge glow |
| Tribal | Clan of Embers, Doviello, Hippus, barbarian | a palisade ring, hide tents and longhouses, a central bonfire (`FIRE` radius 8) |
| Harbour | Lanun | stilt houses over water, piers and masts; on a coastal tile it extends 6 px into the water |
| Carnival | Balseraphs | peaked tents with tincture stripes, a lantern string |
| Infernal | Infernal, Sheaim | black jagged spires (`1`/`2`), scarlet windows, `BLOOD` pool instead of `FIRE` |
| Celestial | Mercurians | pale slender towers (`6`/`7`), vellum light, a pool with no colour cast |
| Frost | Illians | rime-capped stone (`F` caps on `4`/`5`), cold rime windows, no warm pool |

### 9.3 Modifiers

- **Walls** (the Walls building): a low curtain wall line in front, 3 px tall.
- **Castle**: adds corner towers.
- **Capital** (Palace): the tallest spire carries a finial in the owner's field colour.
- **Holy city**: the religion's glyph is stamped on the plaque.
- **Disorder**: windows go dark, an ash smoke column rises.
- **Razed or ruined**: CITY_RUINS (section 5.7).

No era-based art: FfH eras are religious and change nothing visual.

### 9.4 Plaque

Below the city, overlapping the tile's bottom edge by 3 px, sits a **plaque**: a soot `1` plate
7 px tall, with an ash `5` 1 px border.

- Left: a 5x7 **size chip** in the owner's field colour, with the population in the 3x5 tally font
  (section 11.6), in the charge colour.
- Middle: the name, in the Vigil bitmap font, in bone `7` (vellum `8` for your capital).
- At default zoom, the production glyph and turns follow at the right, for your cities.

At survey zoom, only the chip is drawn.

---

## 10. Units

### 10.1 Box, anchor, facing and states

- The 24x24 box has its feet at (11, 21). Figures are about **16 px** tall from feet to crown;
  weapons, hats and banners reach up to 21 px. Mounted units are about 20 px wide.
- All units face **right** (east). They are mirrored when their last move had a westward component.
- **One sprite per stack**: the head unit, which is the best defender for foreign stacks and the
  selected unit for your own. Behind it, up to 2 offset echoes (2 px up-left each), drawn as coal
  `2` silhouettes. A stack-count chip in the tally font sits at the top-left of the tile.
- **HP strip**: 12x1 at tile (10, 31), with a dark `2` remainder. The filled part is bone `7`
  above 50% HP, ochre `y` from 25% to 50%, crimson `C` below 25%.
- **Moves pip**: tile (23, 31): vellum `8` if moves remain, iron `3` if spent.
- **State marks** (8x8, top-left of the tile when there is no stack chip):

  | state | mark |
  |---|---|
  | fortified | a small shield |
  | sleeping | a crescent |
  | sentry | an eye |
  | healing | a sprig |
  | automated | a gear |
  | promotion ready | a gilt star (`y` with an `L` pixel), which takes precedence |

### 10.2 Silhouette grammar by unit combat class

Category is read from the **outline**, never from colour. Base materials are `2 3 4 5` for metal
and dark cloth, `u Q y` for leather and wood. Owner slots go on cloth.

| class (`UNITCOMBAT_*`) | silhouette | key shapes |
|---|---|---|
| MELEE | upright, square-shouldered, legs apart | helmet dome; shield on the front (owner field and charge); blade or axe raised behind |
| ARCHER | upright, slimmer, hooded or capped | a bow arc ahead of the body (a 9 px curve in `y` with a `5` string); a quiver on the back |
| MOUNTED | a horizontal mass, about 20x12, on 4 legs | horse body `3` with a `4` back line; caparison in the owner slots; rider torso and helm above; lance or bow |
| RECON | low, leaning forward, cloak flared back | hood; short blade or bow; one foot forward (the stride) |
| SIEGE | a wooden machine (`u Q y`) with a small crew figure | catapult arm, trebuchet beam, cannon barrel (`3`/`4`), roofed ram |
| ADEPT (arcane) | a tall robe flaring to the ground, a pointed hood or hat | a staff with an orb in the `@` accent (default lilac `P`, with an `L` core); robe in `v`/`p` |
| DISCIPLE (divine) | a hooded robe (the board's acolyte), or armour with a tabard | a lantern or censer (amber, `FIRE` radius 5); a stole in the `@` accent = the unit's religion colour (section 11.5) |
| NAVAL | a hull profile on water, about 22 px long | sails `6`/`7` with an owner emblem; lantern at the stern; reflection streak |
| ANIMAL | a natural creature in earth and ash tones | no owner slots, no pennant |
| BEAST | a monster, often in the 32x32 huge box | exaggerated mass: giants, werewolves, manticores, dragons |
| none (civilian) | unarmed, softer outline | worker: pick or hoe; settler: bundle on the back and a walking staff; great people: a robe and their attribute (see below) |

Great people and specialists, by attribute:

| unit | attribute |
|---|---|
| prophet | staff and a ring of vellum dots |
| artist | lyre |
| scientist | open book |
| merchant | purse and scales |
| engineer | square and hammer |
| commander | standard |

### 10.3 Tier: ornament, not size

Tier (`iTier`, 1 to 4) is shown by **gear**. Silhouettes of the same class keep the same height.

| tier | gear |
|---|---|
| 1 | bare head or hood, crude weapon (club, sling), no shield; leather `u`/`Q` |
| 2 | helmet, standard weapon, plain shield; iron `3`/`4` |
| 3 | crest or plume (owner field), cape (owner shade), shield with charge, better weapon |
| 4 | full harness: tall helm or great helm, great weapon or tower shield, barding on mounts, a carried standard or aura; lit metal edges `6` |

Unique units (civilization replacements) keep their class silhouette and change **one** prop. For
example, the Javelin Thrower holds a javelin bundle instead of a bow, and the Boar Rider rides a
boar instead of a horse.

### 10.4 Race and nature

These modifiers come from the unit's free promotions and apply on top of the class silhouette:

| race or nature | modifier |
|---|---|
| ELF, DARK_ELF | 1 px taller and narrower; leaf-shaped helm and blade; cloak in `t` (elves) or `p` (dark elves) |
| DWARF | 3 px shorter, 2 px broader; heavy helm; beard in `5`/`6` |
| ORC | hunched, heavier jaw with a tusk pixel; crude gear in rust and olive |
| LIZARDMAN | forward lean, tail; olive scales |
| UNDEAD | bone `7`/`6` body or rags in `3`; **eye pixel `S`** (pale green: the unnatural) |
| DEMON | horns or spines, coal `2` body, **scarlet `R` eyes**, `c`/`C` cracks; Infernal owner slots |
| ANGEL | wings (a vellum `8` leading edge, `6` feathers), always fully rimmed in vellum; no crimson ever |
| GOLEM | blocky, material by kind: wood `u`, clay `Q`, iron `3`, bone `7`, mithril `F`, nullstone `1`; one core pixel (`E`, or `P` for magical golems) |
| ELEMENTAL | made of its element and self-lit: fire (`E l L`, `FIRE` radius 6), water (`N g F`), earth (`4 5 u`), air (`F` wisps, translucent); summons of the same element share the body |
| ILLUSION, PUPPET | translucent (50% checker, section 4.5) |
| DRAGON | huge box; wingspan; Abashi (`2`), Acheron (`C` underbelly), Drifa (`F`), Eurabatres (`y` gilt) |
| HERO | a unique silhouette, plus the gilt finial on the stack pennant |

### 10.5 Naval and air

| class | units | silhouette |
|---|---|---|
| oared | Galley, Trireme | low hull, oar ticks below the waterline |
| sail | Caravel, Galleon, Frigate, Man-o-War, Queen of the Line | masts from 1 to 3; gun ports as a `0` dot row on Frigate and later |
| raider | Privateer, Pirate | sable sails, no pennant |
| arcane | Arcane Barge | a flat hull with a lilac lantern (`ARC` radius 5) |
| ghost | Sailor's Dirge | ragged translucent sails, an `S` lantern |
| hero | Black Wind | a unique silhouette |
| sea monster | Sea Serpent, Kraken, Leviathan | coils breaking the surface, `g` glints |
| boat | Workboat | a rowboat, 10 px |
| air | Airship | a hull hanging under a gas bag (`5`/`6`), ropes; it casts a contact shadow on the ground 4 px below |
| bird | Hawk | a 7 px bird |

Equipment units (`EQUIPMENT_*`, immobile) are items on the ground: a 12x12 glyph on a tiny void
shadow, gilt (`y`) for treasure and relics, with a single `l` or `S` glint for magical ones.

---

## 11. UI

### 11.1 Principles

- **The map dominates.** Chrome hugs the edges in narrow bands; no panel covers the centre except
  modals.
- The look of a scriptorium at night: soot panels, double rules, bone text, small gilt
  ornaments. **Not copied from the board**: the layout, counters and symbols here are designed for
  this game's needs.
- Information first: every number is legible at `u = 1`; ornament never sits on top of data.

### 11.2 Colours as CSS

Define every palette colour as a custom property (`--c-void`, `--c-soot`, ... `--c-wisp`) and use
only those.

| role | colour |
|---|---|
| page background | soot |
| panel | soot |
| raised panel | coal |
| hairlines | iron |
| frames | ash |
| secondary text | stone |
| body text | bone |
| emphasis and titles | vellum |
| disabled | ash |
| links (Civilopedia references) | sand |
| modal backdrop | void at 70% alpha (the only alpha allowed) |

Civ4 text markup maps to:

| markup | colour |
|---|---|
| `COLOR_HIGHLIGHT_TEXT` | vellum |
| `COLOR_POSITIVE_TEXT` | lichen |
| `COLOR_NEGATIVE_TEXT`, `COLOR_RED` | scarlet |
| `COLOR_WARNING_TEXT` | amber |
| `COLOR_UNIT_TEXT`, `COLOR_BUILDING_TEXT` (and other link colours) | sand |
| `COLOR_SOMNIUM_*` | frost |
| anything else | bone |

### 11.3 Panels and frames

- **Frame**: a double rule, ash `5` outside and iron `3` inside, 1 art px each with 1 art px of
  soot between. At each corner, a 3x3 ornament: a bone `7` pixel cross with an ochre `y` centre (a
  gilt nail).
- **Panel title**: the Vigil bitmap font in vellum on coal `2`, with a 1 px ash rule beneath, and
  an optional 8x8 icon at the left.
- **Dividers**: 1 px iron, inset 4 px from the frame.
- **Modals** (events, votes, decisions) are pages:
  - wider frame: a third rule in ash;
  - an **illuminated initial**: the first letter of the text in Vigil at 2x, ochre `y` with an
    amber `l` highlight pixel, on a dusk `p` square;
  - body in the serif;
  - the options as a vertical list of buttons.

### 11.4 Buttons and controls

| state | fill | border | text |
|---|---|---|---|
| normal | coal | 1 px ash | bone |
| hover | coal | stone | vellum |
| pressed | soot, with a 1 px void inset top-left | stone | vellum |
| disabled | soot | iron | ash |
| focus | (as current state) | plus a 1 px vellum dotted ring, offset 1 | (as current state) |
| primary (End Turn, Confirm) | coal | bone, plus inner gilt nails | vellum, with an icon at left |
| danger (declare war, delete, raze) | coal | crimson | bone; hover adds an inner blood `c` rule |

Other controls:

- **Sliders** (commerce percent): a 1 px stone track with tick dots every 10%, and a 5x7 handle in
  bone with a void outline.
- **Lists**: soot rows; the hover row coal; the selected row coal with a 1 px bone left bar.
- **Tabs**: ash underline; the active tab has a vellum underline and vellum text.
- **Scrollbars**: 2 art px, iron track, ash thumb.

### 11.5 Icons

**Grid.** Icons are drawn on a 16x16 grid (panel icons) and an 8x8 grid (inline in text, map
overlays, yields on tiles). They are 1 px line-and-mass glyphs in bone `7`/`6`, with **one** accent
colour and a void outline when placed over the map.

**Yields and commerce** (also the `[ICON_*]` set used by the game text):

| icon | glyph | accent |
|---|---|---|
| FOOD | a bound sheaf | ochre `y` |
| PRODUCTION | a hammer over an anvil | iron, with an `E` spark |
| COMMERCE / TRADE | a coin on an open palm | ochre `y` |
| GOLD | a stack of coins | ochre `y` / `Y` |
| RESEARCH | an open codex | vellum pages |
| CULTURE | an illuminated letter on a square | ochre `y` with a `P` square |
| ESPIONAGE | an eye (drawn for completeness; espionage is locked off) | stone |
| HAPPY | a lit candle | amber `l` flame |
| UNHAPPY | a snuffed candle with smoke | ash smoke, `C` wick |
| HEALTHY | a green sprig | olive `o` / lichen `O` |
| UNHEALTHY | a bubble of miasma | dusk `p` |
| GREATPEOPLE | a ring of vellum dots | vellum |
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
| MIND | patina `T` | an eye |
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
| OCTOPUS_OVERLORDS | a tentacled eye | patina `T` |
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
  - drawn in vellum or bone, with an optional 1 px void drop shadow (down-right) over the map;
  - used for: panel titles, city names on plaques, modal initials (at 2x), big numbers in the top
    bar, the turn count.
- **Tally**: a 3x5 pixel font of digits `0`-`9` and `+ − / ×`. Used for: stack counts, size chips,
  yield numbers on tiles, path turn numbers, combat odds on the map.
- **Body text** is the system serif, never a web font:
  - stack: `font-family: "Iowan Old Style", "Palatino Linotype", Palatino, "Book Antiqua",
    "URW Palladio L", Georgia, serif`;
  - sizes: 15 px body, 13 px small, line-height 1.4, `font-variant-numeric: lining-nums
    tabular-nums` in tables and the top bar;
  - colour: bone on soot; never pure white, never below stone for anything a player must read.

### 11.7 Cursor, selection and path

- **Pointer cursors**, 16x16 at `u`. All have a void outline.

  | cursor | design |
  |---|---|
  | pointer | a bone quill-nib arrow |
  | move | the arrow with a dotted trail |
  | attack | crossed blades, crimson |
  | cast | a lilac star |
  | invalid | an ash ring with a bar |
  | busy | a guttering candle |

- **Tile cursor**: four corner brackets, 1 px wide, with 5 px arms; stone `6` on hover, vellum `8`
  on the selected tile.
- **Selected unit**: the moon rim (section 4.2) turns vellum on all four sides. No pulsing glow; the
  path dots march instead.
- **Path preview**:
  - a dotted vellum line, one dot every 3 px, from tile anchor to tile anchor;
  - at each turn boundary, a 7x7 soot roundel with a stone rim and the turn number in Tally;
  - the part beyond this turn's moves in stone;
  - unreachable tiles end in an ash ring with a bar;
  - an attack ends in **crimson brackets** and crossed blades, with the odds in Tally on a soot
    chip (vellum when odds are 50% or more, scarlet when under 50%);
  - route-to (build road) paths are drawn in ochre dots.
- **Range and AOE previews** (spells, bombard): tiles outlined with a dotted lilac ring and a 25%
  `ARC` grid inside.
- **Hover tooltip**: a soot panel with a 1 px stone border and 4 art px padding, serif text,
  following the cursor at an offset of +12, +12 px and clamped to the viewport.

---

## 12. What the board taught us (and what it did not)

Taken from the board:

- the near-black teal-tinted ground of UI and night;
- low-chroma grey-green and ash land, with darker forest masses;
- the bone-lit shoreline;
- dotted paths;
- dark gothic silhouettes with amber windows and a warm ground glow;
- the black banner with a bone charge;
- the pale-green ritual circle and the crimson monolith as the only loud things;
- the double-ruled frames;
- the crescent-moon end-turn;
- the eclipse.

Rejected:

- its specific icons (the faith star, war swords, crown and skull counters);
- its panel arrangement;
- its lettering;
- its painterly texture density: our ground is sparser, so ~20 terrain types stay readable at 1x;
- its mock-up resolution: everything here is native pixel art at integer scale.

---

## 13. Sprite sources as text

All sprites live in text files next to the client, in `crates/ffh-play/web/art/` (section 15
describes the toolkit that loads them, with a few extensions to this format). One file holds
many sprites:

```text
; comment lines start with ';'
= unit/swordsman 24x24 11,21
.....................0..
...
```

- The header `= <name> <W>x<H> <anchor x>,<anchor y>` is followed by exactly H rows of exactly W
  characters.
- Characters:
  - a **palette key** (section 2.1);
  - `.` transparent;
  - owner slots `#` field, `%` shade, `*` charge;
  - `@` the per-unit accent (religion or sphere colour; it defaults to lilac for adepts and patina
    for disciples).
- Names are paths: `terrain/grass`, `feature/forest_tree_a`, `unit/<UNIT_TYPE lower-case>`,
  `city/cloister_3`, `icon16/food`, `icon8/food`, `font/vigil/A`...
- Variants are suffixed: `_a`, `_b` for scatter variants; `_f1`, `_f2` for animation frames.
- Outlines are part of the sprite (`0`). The moon rim, contact shadow, pools, owner pennants and
  plaques are added by the renderer, not drawn into sprites.
- Generated PNG sheets are build products and are not committed. Only small reference contact
  sheets (under 300 KB) may live in `docs/art/`.

---

## 14. Review checklist for new art

1. Uses only the 40 palette colours; owner and accent slots only where cloth or charges belong.
2. Readable at 1x as a silhouette alone (fill it with void: can you still tell what it is?).
3. Unit: 24x24 box, feet at (11, 21), faces right, void outline, tier shown by gear, no colour-only
   distinctions.
4. Reserved colours (fire, crimson, pale green, purple) used only for their meaning (section 2.2),
   within the budget (section 2.3).
5. Light from the upper left; any warm light has a visible source and a pool, and none of it shows
   in fog.
6. Dithering is ordered and purposeful; no noise; no anti-aliasing; integer scaling only.
7. Terrain marks are sparse; at 2x a screen of the terrain still reads as quiet ground with a few
   strong shapes.
8. Nothing copies Civ4, FfH2 or the mood board's specific symbols.

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
  palette.txt      the 40 colours, the remap tables (2.4) and the tinctures (7.1)
  sets.txt         the sprite sets, in load order
  types.json       generated from data/xml: every type with the facts the mapping keys on
  terrain/ units/ cities/ ui/     one directory per set: manifest.txt + *.spr sources
  preview/         contact sheets and the sample scene (open with tools/serve.mjs)
  tools/           validate, check.sh, screenshot, serve, gen-types, bundle, sprtool, bake-terrain
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
  | `unit/` | 24x24 feet 11,21, or 32x32 feet 15,29 (huge) |
  | `city/<style>_<tier>` | base on the bottom row; tiers 1-5 (section 9.1) |
  | `improvement/` | at most 34x34 |
  | `res/` | 7x7 glyph centred at 3,3; `res/_disc` the 11x11 seal |
  | `icon16/`, `icon8/`, `ui/mark_*` | 16x16, 8x8, 8x8 |
  | `sigil/` | 5x5 (or 7x7) charges in `*` |
  | `font/vigil/`, `font/tally/` | glyphs in `8`, anchor at the baseline; `x_sc` are small capitals |
  | `ui/` | pennant, banner and other chrome |

- Groups: `name_a`, `name_b`... are variants of `name` (picked per tile by hash); `name_f1`,
  `name_f2`... are animation frames (picked by `art.time`, 400 ms each). A rule targets the group.
- Names starting with `_` (`unit/_melee`) are generic: category fallbacks, not a game type.
- Outlines are part of the sprite. Moon rims, contact shadows, pools, pennants, HP strips and plaques
  are added by the renderer.

### 15.2 Sets, manifests and the mapping

`sets.txt` lists the set directories. Each set has a `manifest.txt`, so four people can add sprites
in parallel without touching a shared file (only a new set edits `sets.txt`):

```text
set units
source generic.spr
map unit race=UNDEAD -> unit/_undead prio=15
map unit combat=MELEE -> unit/_melee
map unit combat=MELEE tier=4 -> unit/_melee remap=34,45,56,u3
map unit * -> unit/_civilian
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
const art = await loadArt();                 // or loadArt({ base: url }) or loadArt({ bundle: text })
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
| `drawBorder(ctx, x, y, plot \| {n,e,s,w}, ownerStyle)` | stitched ribbons on the owner's side |
| `drawResource(ctx, x, y, bonusType)` | the seal at tile 25,25 |
| `drawImprovement(ctx, x, y, type, plot)` | improvement with its pools (dark windows when not visible) |
| `drawCity(ctx, x, y, city, ownerStyle)` | style and tier sprite, pool, window light, finial, banner, plaque |
| `drawUnit(ctx, x, y, unitType, ownerStyle, state)` | contact shadow, echoes, rim, pennant, HP strip, moves pip, stack chip or mark |
| `drawFog(ctx, x, y, plot)` | the fog pass: FOG remap with hatching, void for unrevealed, ragged edges |
| `drawLight(ctx, cx, cy, table, r, {inner})`, `drawVignette(ctx, x, y, w, h)` | index-remap pools and the vignette |
| `drawText(ctx, x, y, text, {font, color, shadow, smallCaps})`, `measureText` | Vigil and Tally |
| `icon(name)`, `iconFor(kind, type)`, `drawIcon` | icons as small cached canvases |
| `drawSprite(ctx, x, y, name, variant)`, `drawTileCursor` | any sprite by name; the tile cursor |

`city` is `{ size, name, capital, own, turns, disorder, visible, style, plaque, layer }`; `state` is
`{ facing, selected, hp, moves, count, mark, hero, translucent, accent, noPennant, layer }`;
`layer: 'sprite' | 'overlay'` splits a unit or city into the map part and the overlays drawn
above every unit. `ownerStyle(civ, {division, tincture, ...})` builds an owner style and
`assignOwners([{id, civ}])` applies the assignment of section 7.2.

Draw order matters because rivers and uprights spill into neighbouring tiles, and fog and pools
read the canvas back: ground for every tile, then rivers, roads, flat improvements, city pools,
uprights row by row from north to south, seals, borders, fog, vignette, units, overlays.
`renderMap(art, ctx, map, { plots, owners, cities, stacks, cursor, x0, y0, tx0, ty0, cols, rows })`
does exactly that, clipped to the view, and reports per-phase times in `timings`.

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
- `tools/check.sh` (or `node tools/validate.mjs [-v]`) is the test: every module passes
  `node --check`; palette.txt equals the blocks of section 2; every sprite parses, uses only palette
  keys and slots and has its namespace's size; every rule's target exists and its options parse;
  types.json matches data/xml; the bundle loads the same library; and every type of every kind
  (terrains, hills, peaks, features, bonuses, improvements, routes, all units, civilizations, city
  styles, religions, buildings, promotions, unit combats, specialists) resolves to a sprite or to
  `none`. Colour counts over the limits of 4.3 are warnings. The script exits 0 when node is
  missing. The client crate can run it from `cargo test` with a test like:

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
3. New terrain textures can start from `node tools/bake-terrain.mjs <name>` (the recipes of 5.2);
   the baked text is then the source and is edited by hand.
4. Run `tools/check.sh`, then look: `node tools/screenshot.mjs /tmp/art` and read the set's sheet,
   `coverage.png` and `scene.png` at 1x and 2x. Review against section 14.

The seed art is deliberately small: 8 terrain textures (hell terrains are recoloured twins), hill
humps, rock and snow peaks, three conifers (also ancient, burnt, new forest and jungle through
remaps), flames, routes; 15 generic unit silhouettes that every unit falls back to by combat class,
race, domain and tier; Cloister tiers 1-3, Infernal 2 and Tribal 2 cities; farm, cottage, hamlet,
mine and category fallbacks for improvements and lairs; resource seals; pennant, banner and six
sigils; the Vigil and Tally fonts; yield and category icons. Not drawn yet: bespoke units, most
sigils and city styles, the 18 sphere runes, landmarks, walls, disorder smoke, holy-city glyphs,
cursors and path previews.
