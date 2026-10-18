// What stands for what (docs/R3D.md "Placement"): the hooks that map the game's types to asset
// recipes. dress.js asks the catalog; the recipe modules (vegetation.js, architecture.js, units.js,
// props.js) fill in the entries they model, and anything not filled in falls back to basic.js.
//
// Entries (all optional; each returns null when it has nothing to say):
//   forest(feat, ctx)      -> [{asset, variant?, param?, scale, weight?}]  species mix of a wooded plot
//   flora(ctx)             -> [{asset, scale, count, param?, variant?}]  loose plants on open
//                              ground (bushes, reeds); a forest mix's `under` entries take the same
//   rocks(ctx)             -> [{asset, scale, count, param?, variant?, prefer?}]  boulders/
//                              outcrops on hills, peaks, tundra (prefer "ridge": on high ground)
//   settlement(type, ctx)  -> part or [parts]  a settlement-like improvement (cottage, hamlet,
//                              village, town, fort, tower, castle) in the plot owner's family
//                              (ctx.family); asked before `improvement`
//   improvement(type, ctx) -> part or [parts]  any other improvement's structure (farm, mine,
//                              lair, mana node, unique feature); part: {asset, param?, variant?,
//                              scale?, sx?, sy?, sz? (stretch), dx?, dz?, yaw?, float?,
//                              emit?: [emitter] (offsets in the part's frame: turned and scaled
//                              with it)}
//   bonus(type, ctx)       -> {asset, param?, scale?, water?}   a resource's marker
//   bridge(ctx)            -> {asset, variant?} (or an asset name)  a road's bridge over a river
//                              edge of the plot, modelled along z with its deck's ends at y = 0
//                              (ctx: the plot ctx, crossing (the plot across), alongX (the road
//                              runs east-west))
//   feature(feat, ctx)     -> [{asset, param?, scale?, dx?, dz?}] one-off features (oasis, volcano)
//   family(civType)        -> string   a civilization's architectural family
//   city(ctx)              -> [{asset, variant?, param?, x, z, yaw, scale, lift?}] (+ optional
//                              `.emit` array property: chimney smoke, braziers) a settlement's
//                              buildings around its plot centre (ctx: size, capital, walls,
//                              family, civ, seed, height(dx, dz), coast, river)
//   unit(type, ctx)        -> {asset, param?, variant?, scale?, height?, float?, fly?}  a unit's
//                              figure (ctx: info, civ, family, domain "DOMAIN_SEA"/"DOMAIN_LAND"/
//                              "DOMAIN_AIR", combat "UNITCOMBAT_MELEE"..., unit, name)
//   emitters(kind, ctx)    -> [emitter]  particles of a "plot" (plot ctx) or a "city" (city ctx)
//
// An emitter: {dx?, dy?, dz? (offsets from the plot centre / part, x east, z south, y up), color
// "#hex", size (world units, ~0.01-0.04), rise (cycles per second), count, spread (radius),
// smoke?}. Particles glow (premultiplied over the frame, after the paint passes); `smoke: true`
// makes soft puffs instead that swell, rise higher and lean with the wind (chimneys, volcanoes:
// a grey `color`, size ~0.03-0.06, rise ~0.1).
//
// Plot ctx (forest, flora, rocks, feature, settlement, improvement, bonus, emitters "plot"): r, x,
// y, i (plot index), kind (world.js KIND), feat (FEAT), ground (GROUND), hills, peak, held
// (something stands at the centre), city, unit, owner (player id or -1), civ, family, bonus and
// improvement (type names or null), world, coast ([[dx, dz], ...] plot-local directions of the
// water around the plot: a harbour faces them; empty inland), river (a river runs along the plot),
// cityNear (a city stands on a neighbouring plot), cityBehind ({n, e, w}: a city on the plot north,
// north-east, north-west: this plot stands between it and the camera).
//
// ctx always carries `r` (a seeded generator for the plot or object) and `name` (the type name).

export const CATALOG = {};

/** Merges a module's entries; later modules override earlier ones per entry. */
export function extendCatalog(entries) {
  Object.assign(CATALOG, entries);
}
