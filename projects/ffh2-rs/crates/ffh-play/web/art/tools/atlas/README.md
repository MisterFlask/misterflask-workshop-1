# tools/atlas: the unit-atlas pipeline and the revamp's recipes

Authoring aids for the "cheerful readability" look (docs/ART.md 2, 5.2, 10.2). They need Python 3
with numpy and Pillow; they are not part of the build or of `tools/check.sh`. What they write
(`palette.txt` and the ART.md blocks, `terrain/textures.spr`, `units/atlas.spr`) is the source
afterwards and may be edited by hand.

Each script takes a **work directory** `WORK` (any scratch folder) as its first argument:
`WORK/ref/` holds the input sheets, and the scripts write `WORK/crops/`, `WORK/small/` and
`WORK/out/` there.

## Units from an atlas sheet (docs/ART.md 10.2)

Put the owner's full-resolution atlas sheets in `WORK/ref/` as `atlas1.png`, `atlas2.png`, ...
(the small copies in `docs/art/unit-atlas-*.png` are for reading, not for cutting).

```sh
python3 extract_units.py WORK atlas1 atlas2 atlas3   # crops (base stripped) + numbered contact sheets
python3 downsample.py WORK                            # sprite-resolution RGBA, one scale per atlas
# edit unit_mapping.json: sprite name -> crop (atlasN_KK) and options
python3 quantize.py WORK                              # WORK/out/units_atlas.spr + WORK/units_preview.png
python3 integrate_units.py WORK ../..                 # into units/atlas.spr, legacy defs dropped, manifest fixed
sh ../check.sh                                        # validate
```

`extract_units.py` writes `WORK/<atlas>_sheet.png`, a numbered contact sheet of the crops: read it
to fill `unit_mapping.json`. Options per sprite: `slots` (`auto`, `none`, `red`, `blue`, `skyblue`,
`purple`, `olive`: which cloth becomes the owner slots), `frame` (`32`, or `40` for giants), `ship`
(strip the water under the hull). `integrate_units.py` also rewrites the manifest's variant remaps
for the atlas figures' keys; re-running it on an already integrated tree only reports the rules it
has already changed.

## Composed units (docs/ART.md 10.4)

The units the sheets lack are composed from atlas crops and painted shapes at atlas 2's scale, then
quantized like the cut figures:

```sh
python3 cut_missed.py WORK                           # figures the extraction missed (hand-given boxes)
python3 compose_units.py WORK [NAME...]              # WORK/crops/comp/NAME.png, one per recipe
python3 quantize.py WORK composed_mapping.json units_composed   # WORK/out/units_composed.spr + preview
python3 integrate_composed.py WORK ../..             # units/composed.spr (+ lights), manifest rules
sh ../check.sh
```

`composelib.py` holds the tools: `Layer` (recolour along a palette ramp by lightness rank, erase,
patch, scale, mirror, composite), `Shape` (ellipses, polygons, tapered strokes, supersampled) and
`paint` (a shape shaded as a lit dome, posterized to a ramp, dark rim). A recipe in
`compose_units.py` is a function returning a `Canvas` with the figure's feet on its anchor;
`composed_mapping.json` gives each its quantize options (`frame`, `slots`, `size`, `feet`, `ship`).

## The palette (docs/ART.md 2)

```sh
python3 pick_extras.py WORK 24 small   # how the 16 extra colours were found (constrained k-means)
python3 build_palette.py WORK          # WORK/out/palette.txt, art_blocks.md, css_tokens.txt
```

`pal_v3.py` holds the 56 colours. After changing it, copy `out/palette.txt` to `web/art/`, the
`palette` and `remap` blocks of `out/art_blocks.md` into docs/ART.md, and `out/css_tokens.txt` into
`assets/app/style.css` (`:root`), then run `tools/check.sh` (it compares palette.txt with the guide).

## Terrain textures (docs/ART.md 5.2)

```sh
python3 bake_textures.py WORK          # WORK/out/textures.spr + WORK/textures_preview.png
```
