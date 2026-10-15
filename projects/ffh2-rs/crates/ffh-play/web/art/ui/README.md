# ui set

The interface art of the ffh-play client in the "Monastic Pixel Noir" manner (docs/ART.md 7 and 11):
soot panels with double rules and gilt nails, bone glyphs with one accent each, the Vigil and Tally
bitmap fonts, and the marks drawn over the map. Sources are sprite text (`*.spr`); `manifest.txt`
lists them and maps game types to sprites. `ui.js` draws the pieces; `preview.html` shows them.

```sh
node tools/serve.mjs            # from web/art; then open:
#   /ui/preview.html            mock screen: top bar, map with path and odds, city panel, unit panel, tooltip, minimap
#   /ui/preview.html?modal      the same with an event modal (illuminated initial, option buttons)
#   /ui/preview.html?kit        every piece of the kit; ?kit=frames,buttons,icons,... for some sections
#   ?u=1|2|3 UI scale, ?zoom=1..4 map zoom
#   /preview/sheet.html?set=ui  the toolkit's contact sheet of this set
```

`ref/screen.png` and `ref/kit.png` are small reference shots of the preview (u = 2).

## Files

| file | sprites |
|---|---|
| `chrome.spr` | nine-slice frames (`ui/frame_panel`, `_page`, `_modal`, `_title`, `_raised`, `_card_selected`, `_tooltip`, `_inset`, `_plate`, `_initial`), buttons (`ui/button`, `_hover`, `_pressed`, `_disabled`, `_primary`, `_primary_hover`, `_danger`, `_danger_hover`), the focus ring, the bar track and fills (`ui/bar_<kind>`), the slider handle, the rule ornament |
| `marks.spr` | unit state marks (`ui/mark_fortify sleep promote sentry heal auto`), tile cursors (`ui/tile_hover select attack route target`), path dots and roundels (`ui/path_*`), combat-odds chips (`ui/odds`, `ui/odds_bad`), pointer cursors (`ui/cursor`, `_move`, `_attack`, `_cast`, `_invalid`, `_busy`; anchor = hotspot) |
| `icons.spr` | yields and commerces, mood, health, unit stats, experience, the empire marks of the top bar (turn, Armageddon eclipse in four steps, golden age, anarchy, research needed, idle city, AI-idle, stasis, mana), category fallbacks |
| `realm.spr` | the 7 religions, 3 alignments, 5 civic categories (16 and 8 px), the 18 mana sphere runes (8 px) |
| `promotions.spr` | 37 promotion seals: 19 kinds of line plus the 18 spheres |
| `kinds.spr` | the 10 unit combat classes (8 px) and the 6 specialists (16 px) |
| `fonts.spr` | Vigil (serifed capitals 7 px, small capitals 5 px, lining digits, punctuation) and Tally (3x5 digits, `+ - / x % . ?`) |
| `heraldry.spr` | the stack pennant, the city banner and the 22 civilization sigils |

## Names

Icons the game references by type resolve through the mapping (`art.iconFor(kind, TYPE)`):
religions `icon16/religion_<x>`, promotions (seal + rim colour by family, see `manifest.txt`), unit
combats `icon8/combat_<x>`, specialists `icon16/specialist_<x>` (great ones get a vellum ring),
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

## ui.js

| function | what |
|---|---|
| `installCSS(art, {u, dpr})`, `cssText` | a stylesheet: `--c-<colour>` properties, `.ui-frame-<x>` border-images for every frame, `.ui-button` (`:hover`, `:active`, `:disabled`, `.primary`, `.danger`, `:focus-visible` ring), `.ui-cursor-<x>`, `.ui-px` |
| `nineCSS`, `drawNine`, `nineCanvas`, `nineEl`, `nineOf` | nine-slice frames: `: tags nine=L,T,R,B` gives the corners; images are pre-scaled to `u * dpr` so nothing is resampled |
| `iconEl`, `textEl`, `promotionEl` | DOM canvases sized in CSS px for an icon, Vigil/Tally text, a promotion seal with its level pips |
| `iconName` | interface icon names (table above) |
| `drawButton`, `drawBar`, `drawSlider`, `drawRule`, `drawInitial` | canvas controls: buttons by kind and state, progress bars (`food production research culture gp xp hp hp_low doom`, with a checkered "next turn" part), the commerce slider, a rule with its ornament, a modal's illuminated initial |
| `drawTileMark`, `drawRange`, `drawPath`, `drawOdds`, `drawTileYields` | map overlays: tile cursors, the ARC range preview, the path preview (turn roundels, dots beyond this turn's moves in stone, ochre for route-to, attack brackets with the odds chip, unreachable ring; dots march with `art.time`), combat odds, the yield layer |
| `drawPromotion`, `drawPips` | a seal through the mapping, plus its level (1-5) as pips inside the lower rim |

## Choices beyond docs/ART.md

- Vigil capitals are up to 6 px wide so stems can carry 1 px serifs at both ends; small capitals
  are plain (4-5 px) to stay legible at 1x. Digits are lining 5x7.
- Promotion seals: the rim gives the family (combat lines stone, arcane lines lilac, races bone,
  equipment and weapon promotions ochre, afflictions crimson, heroic and divine amber, spheres their
  colour) and the glyph the kind of line (sword, shield, arrow, boot, fir, sprig, standard, eye,
  hood, star, ward ring, skull, anchor, crossed blades, head, casket, drop, crown, rune).
- Icons carry their void outline; it is invisible on soot panels and gives contrast on the map.
- The modal has three rules and a gilt lozenge at each corner; the tooltip has a void outline so
  it reads over the map.
