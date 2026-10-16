# ui set

The interface art of the ffh-play client in the "cheerful readability" manner (docs/ART.md 7 and 11):
navy panels with gold rules and gilt nails, maroon title ribbons, painted icons (tools/atlas/compose_icons.py), the Vigil, Vigil Book
and Tally bitmap fonts, the owner marks, and the marks drawn over the map. Sources are sprite text
(`*.spr`); `manifest.txt` lists them and maps game types to sprites. `ui.js` draws the pieces;
`preview.html` shows them.

```sh
node tools/serve.mjs            # from web/art; then open:
#   /ui/preview.html            mock screen: top bar, map with path and odds, city panel, unit panel, tooltip, minimap
#   /ui/preview.html?modal      the same with an event modal (illuminated initial, option buttons)
#   /ui/preview.html?kit        every piece of the kit; ?kit=frames,buttons,fonts,buildings,... for some sections
#   ?u=1|2|3 UI scale, ?zoom=1..4 map zoom, ?doom=N the Armageddon counter,
#   ?ground the terrain set's ground pass, ?citylight the cities set's light pass
#   /preview/sheet.html?set=ui  the toolkit's contact sheet of this set
```

The preview installs the units set's `units/stack.js` (`installStackOverlay`: the stack standard);
`?ground` adds the terrain set's `terrain/ground.js` (`installTerrain`) and `?citylight` the cities
set's `cities/citylight.js` (`installCityLight`, the light pass). `ref/screen.png`
(`?ground&citylight`) and `ref/kit.png` are small reference shots of the preview (u = 2), taken on
the combined sets (art-review with the second rounds of units, terrain and cities; `screen.png`
retaken after the light and ground pass: moonlit clearings, pale paths, three-ring pools).

## Files

| file | sprites |
|---|---|
| `heraldry.spr` | the owner mark: `ui/stack_banner` (the stack standard, a gonfalon on a pole) and `ui/banner` (its pole-less sibling for cities); the 22 civilization sigils and the fallback lozenge |
| `fonts.spr` | Vigil (serifed capitals 7 px, small capitals 5 px, lining digits, punctuation), Vigil Book (`font/vigil/book/`: the body face, lower case with a 5 px x-height) and Tally (3x5 digits, `+ - / x % . ?`) |
| `chrome.spr` | nine-slice frames (`ui/frame_panel`, `_page`, `_modal`, `_title`, `_raised`, `_card_selected`, `_tooltip`, `_inset`, `_plate`, `_initial`), buttons (`ui/button`, `_hover`, `_pressed`, `_disabled`, `_primary`, `_primary_hover`, `_danger`, `_danger_hover`), the focus ring, the bar track and fills (`ui/bar_<kind>`), the slider handle, the rule ornament |
| `marks.spr` | unit state marks (`ui/mark_fortify sleep promote sentry heal auto`), tile cursors (`ui/tile_hover select attack route target`), path dots and roundels (`ui/path_*`), combat-odds chips (`ui/odds`, `ui/odds_bad`), pointer cursors (`ui/cursor`, `_move`, `_attack`, `_cast`, `_invalid`, `_busy`; anchor = hotspot) |
| `icons.spr` | yields and commerces, mood, health, unit stats, experience, the empire marks of the top bar (turn, Armageddon eclipse in four steps, golden age, anarchy, research needed, idle city, AI-idle, stasis, mana), category fallbacks |
| `realm.spr` | the 7 religions, 3 alignments, 5 civic categories (16 and 8 px), the 18 mana sphere runes (8 px) |
| `buildings.spr` | building class icons, 16 and 8 px (`icon16/bclass_<class>`): a temple per faith, shrine, government, military, defense, economy, provision, health, harbour, learning, culture, arcane, forge, wonder |
| `promotions.spr` | 37 promotion seals: 19 kinds of line plus the 18 spheres |
| `kinds.spr` | the 10 unit combat classes (8 px) and the 6 specialists (16 px) |

## One owner mark

A player's mark is the same field, division, sigil and swallow tails everywhere:

- **Stacks** fly `ui/stack_banner`, the standard designed by the units set: a slate pole and
  crossbar with a 5x8 field, the civilization's sigil centred on it, at the tile's top-left (top-right
  in a city). `units/stack.js` draws it with its looks (quiet for foreign and spent stacks, the full
  tincture for your own movable or selected one), the finial pip (moves, hero) and the count chip;
  `loadArt()` installs it (art.js `PASSES`).
- **Cities** fly `ui/banner`, the standard without its pole, hung from a slate rod with ash finials
  at the city tile's top-left (the cities set's study, adopted); art.js draws the sigil at tile
  (2, 3), centred on the field as on the standard.
- The 6x8 stack pennant (`ui/pennant`) is retired. art.js's seed `drawUnit` drew it and now draws
  no stack mark; the standard comes from `units/stack.js`, which `loadArt()` installs (art.js
  `PASSES`; `loadArt({ passes: false })` leaves it out). The standard lives here; the units set's
  `stack.spr` copy was dropped when the sets were merged (stack.js finds the sprite by name).

## Text: Vigil, Vigil Book, Tally, and the serif

- **Vigil** (`font: 'vigil'`): titles, city names, big numbers, button labels (small capitals).
- **Vigil Book** (`font: 'vigil/book'`, `UI.BOOK`): the body face for short UI text: labels, list
  rows, tooltips, option buttons, numbers in panels. Lower case with a 5 px x-height and 7 px
  ascenders, capitals and tabular digits 7 tall, on Vigil's 7-row cell and baseline, so the two set
  side by side on one grid. The cell has no room below the baseline: g j p q y keep their tails
  inside it with the bowl raised.
- **Tally**: map numbers (stack counts, size chips, yields on tiles, path turns, odds).
- The system serif is kept for long prose only: event texts and the pedia.

In the DOM, `UI.textSpan(art, text, {font})` gives bitmap text whose colour is the CSS `color`
(the text is the span's mask), so it follows `:hover`, `:disabled`, `.sel` and colour classes like
ordinary text; `UI.textBlock` wraps it to a width in art px. Text is folded first (`foldText`:
accents dropped, curly quotes straightened).

## Firelight in the UI

Amber (`e E l L`) is kept for happiness, warnings (research needed, idle city, the warning text
colour), the End Turn glyph and the active-turn mark (ART.md 2.2). So:

- the turn crescent is amber with a flame inner edge and a single glow pixel (it was rimmed in glow);
- heroic and divine promotion seals have a double gilt rim (sand `Y` outside, ochre `y` inside) with
  ochre jewels on the crown, apart from the single ochre rim of equipment;
- the top bar's AI-idle count is bone, not amber;
- the Armageddon eclipse under 50 is nearly all soot: an iron corona from the moon side and a
  thread of blood at its foot; crimson starts at 50 (`_mid`), scarlet at 75, the glow core at 100.

## Names

Icons the game references by type resolve through the mapping (`art.iconFor(kind, TYPE)`, or
`UI.iconFor(art, kind, TYPE, 8)` for the 8 px twin): religions `icon16/religion_<x>`, buildings
(class icons, below), promotions (seal + rim colour by family, see `manifest.txt`), unit combats
`icon8/combat_<x>`, specialists `icon16/specialist_<x>` (great ones get a vellum ring),
civilizations `sigil/<x>`. Interface icons without a game type use `UI.iconName(kind, key, size)`:

| kind | key | sprite |
|---|---|---|
| `yield` | 0-2 or `YIELD_FOOD` | `icon16/food`, `production`, `commerce` |
| `commerce` | 0-3 or `COMMERCE_GOLD` | `icon16/gold`, `research`, `culture`, `espionage` |
| `civicoption` | `CIVICOPTION_LABOR` | `icon16/civic_labor` |
| `alignment` | `ALIGNMENT_EVIL` | `icon16/alignment_evil` |
| `mana` | `BONUS_MANA_FIRE` | `icon8/sphere_fire` |
| `armageddon` | the counter | `icon16/armageddon_low` (0), `_mid` (50), `_high` (75), `_doom` (100) |
| `icon` | `happy`, `turn`, ... | `icon16/happy` |

Both sizes exist for every name used inline (`icon8/...`), except the top-bar-only marks.

**Buildings.** Every one of the 170 buildings resolves to a class icon (`manifest.txt`, "buildings"):
world wonders the spire under the gilt star (38); buildings of a faith its temple (the faith glyph
over a temple front; in 8 px lists the faith glyph itself); walls and other defence buildings the
curtain wall; then by class: government (palaces, courthouses), military (training yards, ranges,
stables, pens), economy (markets, taverns, vaults), provision (granary, smokehouse, grove), health
(aqueduct, baths, herbalist), harbour, learning (library, academy, elder council), culture
(monument, theatre, menagerie), arcane (mage guild, the five towers, gates), forge (forge, smithy,
golem works), shrine (pagan temples, the altars of the Luonnotar). The kit's "buildings" section
counts them.

## ui.js

| function | what |
|---|---|
| `installCSS(art, {u, dpr})`, `cssText` | a stylesheet: `--c-<colour>` properties, `.ui-frame-<x>` border-images for every frame, `.ui-button` (`:hover`, `:active`, `:disabled`, `.primary`, `.danger`, `:focus-visible` ring), `.ui-cursor-<x>`, `.ui-px`, `.ui-ptext` |
| `nineCSS`, `drawNine`, `nineCanvas`, `nineEl`, `nineOf` | nine-slice frames: `: tags nine=L,T,R,B` gives the corners; images are pre-scaled to `u * dpr` so nothing is resampled |
| `textSpan`, `textBlock`, `wrapText`, `foldText`, `BOOK` | bitmap text in the DOM whose colour follows CSS (above) |
| `iconEl`, `textEl`, `promotionEl` | DOM canvases sized in CSS px for an icon, text in a fixed colour, a promotion seal with its level pips |
| `iconName`, `iconFor` | interface icon names (table above); a game type's icon at 16 or 8 px |
| `drawButton`, `drawBar`, `drawSlider`, `drawRule`, `drawInitial` | canvas controls: buttons by kind and state (Vigil small capitals, or `font: BOOK`), progress bars (`food production research culture gp xp hp hp_low doom`, with a checkered "next turn" part), the commerce slider, a rule with its ornament, a modal's illuminated initial |
| `drawTileMark`, `drawRange`, `drawPath`, `drawOdds`, `drawTileYields` | map overlays: tile cursors, the ARC range preview, the path preview (turn roundels, dots beyond this turn's moves in stone, ochre for route-to, attack brackets with the odds chip, unreachable ring; dots march with `art.time`), combat odds, the yield layer |
| `drawPromotion`, `drawPips` | a seal through the mapping, plus its level (1-5) as pips inside the lower rim |

## Choices beyond docs/ART.md

What docs/ART.md should take over from this set (3.3, 7.4, 10.1, 11.5, 11.6):

- 3.3 / 7.4 / 10.1: the stack mark is the 7x12 standard `ui/stack_banner` at the tile's top-left
  (top-right in a city), with the sigil, the moves and hero pip on its finial and the count chip
  under it; the city banner is its pole-less sibling, 7x12 with the rod's finials; the 6x8 pennant
  is gone.
- 11.5: buildings have class icons (above), no longer "none in the MVP"; heroic and divine seals
  have a double gilt rim, not amber; the Armageddon eclipse under 50 has an iron corona, not blood.
- 11.6: Vigil Book is the body face for short UI text; the system serif is for long prose only.
- Vigil capitals are up to 6 px wide so stems can carry 1 px serifs at both ends; small capitals
  are plain (4-5 px) to stay legible at 1x. Digits are lining 5x7.
- Promotion seals: the rim gives the family (combat lines stone, arcane lines lilac, races bone,
  equipment and weapon promotions ochre, afflictions crimson, heroic and divine double gilt, spheres
  their colour) and the glyph the kind of line (sword, shield, arrow, boot, fir, sprig, standard,
  eye, hood, star, ward ring, skull, anchor, crossed blades, head, casket, drop, crown, rune).
- Icons carry their void outline; it is invisible on soot panels and gives contrast on the map.
- The modal has three rules and a gilt lozenge at each corner; the tooltip has a void outline so
  it reads over the map.
