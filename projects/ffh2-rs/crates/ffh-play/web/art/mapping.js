// mapping.js: game type -> sprite resolution with fallbacks (docs/ART.md, "Toolkit").
// Pure ES module (no DOM): used by art.js in the browser and by tools/validate.mjs in node.
//
// Resolution order for (kind, TYPE):
//   1. an explicit manifest rule naming the type        map unit UNIT_WARRIOR -> unit/warrior
//   2. a sprite whose metadata lists the type           : types UNIT_WARRIOR
//   3. the naming convention                            unit/warrior exists
//   4. units only: the same steps for the default unit of the unit's class (unique units)
//   5. the most specific category rule that matches     map unit combat=MELEE tier=4 -> unit/_melee remap=34,45
// Category rules score 10 per condition plus `prio=N`; ties go to the rule loaded first.
// The target `none` means "deliberately not drawn" and counts as resolved.

/** Naming convention per kind: sprite name for a type when no rule says otherwise. */
export const CONVENTION = {
  terrain: ['TERRAIN_', 'terrain/'],
  feature: ['FEATURE_', 'feature/'],
  bonus: ['BONUS_', 'res/'],
  improvement: ['IMPROVEMENT_', 'improvement/'],
  route: ['ROUTE_', 'route/'],
  unit: ['UNIT_', 'unit/'], // EQUIPMENT_X -> unit/equipment_x
  civ: ['CIVILIZATION_', 'sigil/'],
  religion: ['RELIGION_', 'icon16/religion_'],
  building: ['BUILDING_', 'icon16/building_'],
  promotion: ['PROMOTION_', 'icon16/promotion_'],
  unitcombat: ['UNITCOMBAT_', 'icon8/combat_'],
  specialist: ['SPECIALIST_', 'icon16/specialist_'],
  hill: ['TERRAIN_', 'hill/'],
  peak: ['TERRAIN_', 'peak/'],
};

/** Pseudo-kinds keyed by another kind's types (hills and peaks are drawn per terrain). */
export const KIND_SOURCE = { hill: 'terrain', peak: 'terrain', citystyle: 'civ' };

/** Kinds whose rule targets are not sprite names (citystyle targets a style: city/<style>_<tier>). */
export const NON_SPRITE_KINDS = new Set(['citystyle']);

/**
 * Option keys that describe the type rather than adjust a sprite. A bespoke (exact) match
 * inherits them from the best category rule, so a new res/wheat keeps its food rim.
 */
export const INHERIT = ['rim', 'tincture', 'light', 'pool', 'scatter', 'deep', 'style'];

export function conventionName(kind, type) {
  const c = CONVENTION[kind];
  if (!c) return null;
  const [prefix, ns] = c;
  const rest = type.startsWith(prefix) ? type.slice(prefix.length) : type;
  return ns + rest.toLowerCase();
}

function condMatches(cond, entry, type) {
  if (cond.attr === 'type') return cond.values.includes(type);
  const neg = cond.attr.endsWith('!');
  const attr = neg ? cond.attr.slice(0, -1) : cond.attr;
  if (!entry || !(attr in entry)) return neg;
  const v = entry[attr];
  const hit = Array.isArray(v) ? v.some((x) => cond.values.includes(String(x))) : cond.values.includes(String(v));
  return neg ? !hit : hit;
}

export class Mapper {
  /**
   * @param types   types.json `kinds` object: {kind: [{type, ...facts}]}
   * @param rules   manifest rules in load order: [{kind, conds, sprite, opts, file, line}]
   * @param has     (spriteBaseName) => bool
   * @param metaTypes Map<TYPE, spriteBaseName> from sprite `: types` metadata
   */
  constructor(types, rules, has, metaTypes = new Map()) {
    this.types = types;
    this.has = has;
    this.metaTypes = metaTypes;
    this.byKind = new Map();
    rules.forEach((r, order) => {
      const exact = r.conds.some((c) => c.attr === 'type');
      const score = (exact ? 1000 : 0) + r.conds.length * 10 + (Number(r.opts.prio) || 0);
      const list = this.byKind.get(r.kind) || [];
      list.push({ ...r, exact, score, order });
      this.byKind.set(r.kind, list);
    });
    for (const list of this.byKind.values()) list.sort((a, b) => b.score - a.score || a.order - b.order);
    this.index = new Map();
    for (const [kind, list] of Object.entries(types)) this.index.set(kind, new Map(list.map((e) => [e.type, e])));
    this.cache = new Map();
  }

  entry(kind, type) {
    const src = KIND_SOURCE[kind] || kind;
    return this.index.get(src)?.get(type) || null;
  }

  /** Names of all types of a kind (pseudo-kinds use their source kind). */
  typeNames(kind) {
    const src = KIND_SOURCE[kind] || kind;
    return (this.types[src] || []).map((e) => e.type);
  }

  /** Exact steps only (1-3). */
  exact(kind, type, entry) {
    const rules = this.byKind.get(kind) || [];
    for (const r of rules) {
      if (!r.exact) break; // sorted: exact rules first
      if (r.conds.every((c) => condMatches(c, entry, type))) return { sprite: r.sprite, opts: r.opts, via: 'rule', rule: r };
    }
    if (kind === (KIND_SOURCE[kind] || kind)) {
      const m = this.metaTypes.get(`${kind}:${type}`) || this.metaTypes.get(type);
      if (m) return { sprite: m, opts: {}, via: 'meta', rule: null };
    }
    const conv = NON_SPRITE_KINDS.has(kind) ? null : conventionName(kind, type);
    if (conv && this.has(conv)) return { sprite: conv, opts: {}, via: 'name', rule: null };
    return null;
  }

  /** The best category (non-exact) rule for a type, or null. */
  category(kind, type, entry = this.entry(kind, type)) {
    for (const r of this.byKind.get(kind) || []) {
      if (r.exact) continue;
      if (r.conds.every((c) => condMatches(c, entry, type))) return r;
    }
    return null;
  }

  /** Resolve a type to {sprite, opts, via, rule} or null. */
  resolve(kind, type) {
    const key = `${kind}:${type}`;
    if (this.cache.has(key)) return this.cache.get(key);
    const entry = this.entry(kind, type);
    let res = this.exact(kind, type, entry);
    if (!res && kind === 'unit' && entry && entry.base && entry.base !== 'NONE') {
      const b = this.exact(kind, entry.base, this.entry(kind, entry.base));
      if (b) res = { ...b, via: 'base' };
    }
    const cat = this.category(kind, type, entry);
    if (res && cat) {
      const inherited = {};
      for (const k of INHERIT) if (k in cat.opts && !(k in res.opts)) inherited[k] = cat.opts[k];
      if (Object.keys(inherited).length) res = { ...res, opts: { ...inherited, ...res.opts } };
    }
    if (!res && cat) res = { sprite: cat.sprite, opts: cat.opts, via: 'category', rule: cat };
    this.cache.set(key, res);
    return res;
  }

  /** Coverage report: {kind: {total, exact, base, category, none, missing: [types]}} */
  coverage(kinds) {
    const out = {};
    for (const kind of kinds) {
      const c = { total: 0, rule: 0, meta: 0, name: 0, base: 0, category: 0, none: 0, missing: [] };
      for (const t of this.typeNames(kind)) {
        c.total++;
        const r = this.resolve(kind, t);
        if (!r) c.missing.push(t);
        else if (r.sprite === 'none') c.none++;
        else c[r.via]++;
      }
      out[kind] = c;
    }
    return out;
  }
}

/** Kinds every game type must resolve for (the validator checks all of them). */
export const KINDS = ['terrain', 'hill', 'peak', 'feature', 'bonus', 'improvement', 'route', 'unit', 'civ', 'citystyle',
  'religion', 'building', 'promotion', 'unitcombat', 'specialist'];
