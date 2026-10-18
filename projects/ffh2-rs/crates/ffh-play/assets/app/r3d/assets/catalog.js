// What stands for what (docs/R3D.md "Placement"): the hooks that map the game's types to asset
// recipes. dress.js asks the catalog; the recipe modules (vegetation.js, architecture.js, units.js,
// props.js) fill in the entries they model, and anything not filled in falls back to basic.js.
//
// Entries (all optional; each returns null when it has nothing to say):
//   forest(feat, ctx)      -> [{asset, variant?, param?, scale, weight?}]  species mix of a wooded plot
//   flora(ctx)             -> [{asset, scale, count, param?, variant?, clump?, spread?, size?,
//                              dense?, mix?}]  open ground's plants, scattered in clumps of
//                              `clump` ([min, max]) within `spread`, `count` at most (a density
//                              field anchored to the map thins it by `dense`, 0..1); `size` 0 low
//                              clutter, 1 bushes, 2 trees (`mix`: [{asset, scale, weight}] per
//                              tree); a forest mix's `under` and `edge` entries take {asset, scale,
//                              count?, param?, variant?}
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
//   feature(feat, ctx)     -> [{asset, param?, scale?, dx?, dz?}] one-off features (oasis, volcano)
//   family(civType)        -> string   a civilization's architectural family
//   city(ctx)              -> [{asset, variant?, param?, x, z, yaw, scale, lift?}] (+ optional
//                              `.emit` array property: chimney smoke, braziers; `.reach`: how far
//                              from the plot centre it stands) a settlement's buildings around its
//                              plot centre (ctx: size, capital, walls, family, civ, seed,
//                              height(dx, dz), coast, river); a great city spills over its plot
//   cityReach(ctx)         -> number  how far a city's buildings reach from its plot centre (ctx:
//                              size, capital, walls): ~0.4 for a small town to ~0.9 for a great
//                              walled capital (its plate, its neighbours' woods keep clear of it)
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
// north-east, north-west: this plot stands between it and the camera), woods (how many of the
// eight neighbours are wooded), roadDist(lx, lz) (distance to the road through the plot).
//
// ctx always carries `r` (a seeded generator for the plot or object) and `name` (the type name).

export const CATALOG = {};

/** Merges a module's entries; later modules override earlier ones per entry. */
export function extendCatalog(entries) {
  Object.assign(CATALOG, entries);
}
