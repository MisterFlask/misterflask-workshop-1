# Art direction reference

`mood-board.png` is the user's mood board for the human-play client's look ("Monastic Pixel Noir",
with a little colour): dark pixel art; strong silhouettes on quiet terrain; the map as a
manuscript. The palette is muted but not monochrome: soot and ash neutrals, bone white, deep
midnight navy water, weathered teal, moss green land, dusky purple for the arcane, warm firelight
in windows and torches, with muted crimson (blood, corruption) and pale green (ritual, the
unnatural) reserved for the extraordinary.

The board is a style direction and vibe only, not a UX spec: its words, specific symbols, icons
and layout are placeholders from the mock-up tool, not part of the direction.

`mood-board-v1-monochrome.png` is the first, near-monochrome version, superseded by the one above
(kept for reference: same mood, less colour).

All game art is original (no Civ4/FFH2 assets); see `docs/ART.md` for the style guide.

`style-reference.png` is a prototype study rendered while writing `docs/ART.md`: the 40-colour
palette, the terrain and feature key, unit silhouettes on three grounds with the ten owner
tinctures, and a sample map at 2x. It validates the guide's rules. It is not final art.

`toolkit-scene.png` and `toolkit-units.png` are screenshots of the art toolkit's preview pages
(`crates/ffh-play/web/art/preview/`, see `docs/ART.md` section 15): the 20x14 sample scene at 2x
rendered from the seed sprites, and the generic unit silhouettes on three grounds. They show the
toolkit's state when it was built; regenerate them with `node tools/screenshot.mjs`.
