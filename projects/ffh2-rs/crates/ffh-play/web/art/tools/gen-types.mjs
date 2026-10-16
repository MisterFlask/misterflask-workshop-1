#!/usr/bin/env node
// gen-types.mjs: extract the game types the art must cover from data/xml into types.json.
//
//   node tools/gen-types.mjs            write ../types.json
//   node tools/gen-types.mjs --check    exit 1 if types.json is stale
//
// types.json lists, per kind, every XML type with the few facts the sprite mapping keys on
// (unit combat class, domain, tier, race, ...). It is generated, committed, and loaded by art.js
// so the browser can resolve fallbacks without the XML. Only facts come from the XML; any
// curated grouping (which promotions are afflictions, ...) lives in the set manifests.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const ART_DIR = resolve(HERE, '..');
export const XML_DIR = resolve(HERE, '../../../../../data/xml');

// ---- a tiny XML tree parser (elements, text; comments and declarations skipped) ----
export function parseXml(src) {
  const root = { tag: '#root', children: [], text: '' };
  const stack = [root];
  const re = /<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<!\[CDATA\[([\s\S]*?)\]\]>|<(\/?)([A-Za-z_][\w:.-]*)([^>]*?)(\/?)>|([^<]+)/g;
  let m;
  while ((m = re.exec(src))) {
    const top = stack[stack.length - 1];
    if (m[1] !== undefined) top.text += m[1];
    else if (m[3]) {
      if (m[2] === '/') {
        if (top.tag !== m[3]) throw new Error(`xml: </${m[3]}> closes <${top.tag}>`);
        stack.pop();
      } else {
        const el = { tag: m[3], children: [], text: '' };
        top.children.push(el);
        if (m[5] !== '/') stack.push(el);
      }
    } else if (m[6] !== undefined) top.text += m[6];
  }
  return root;
}
const kid = (el, tag) => el.children.find((c) => c.tag === tag);
const kids = (el, tag) => el.children.filter((c) => c.tag === tag);
const val = (el, tag, def = '') => { const k = kid(el, tag); return k ? k.text.trim() : def; };
const num = (el, tag) => { const v = parseInt(val(el, tag, '0'), 10); return Number.isFinite(v) ? v : 0; };
const flag = (el, tag) => (val(el, tag, '0') === '1' ? 1 : 0);
const short = (s, prefix) => (s && s !== 'NONE' ? s.replace(prefix, '') : 'NONE');

function infos(xmlDir, file, listTag, infoTag) {
  const doc = parseXml(readFileSync(join(xmlDir, file), 'utf8'));
  const top = doc.children.find((c) => c.tag !== '#text');
  const list = kid(top, listTag);
  if (!list) throw new Error(`${file}: no <${listTag}>`);
  return kids(list, infoTag);
}

// Race and nature promotions that change a unit's look (docs/ART.md section 10.4).
const SPHERES = ['AIR', 'BODY', 'CHAOS', 'DEATH', 'EARTH', 'ENCHANTMENT', 'ENTROPY', 'FIRE', 'ICE', 'LAW', 'LIFE',
  'METAMAGIC', 'MIND', 'NATURE', 'SHADOW', 'SPIRIT', 'SUN', 'WATER'];

export function buildTypes(xmlDir = XML_DIR) {
  const K = {};
  K.terrain = infos(xmlDir, 'Terrain/CIV4TerrainInfos.xml', 'TerrainInfos', 'TerrainInfo').map((e) => ({
    type: val(e, 'Type'), water: flag(e, 'bWater'), impassable: flag(e, 'bImpassable'), graphical: flag(e, 'bGraphicalOnly'),
    up: val(e, 'TerrainUp', 'NONE'), down: val(e, 'TerrainDown', 'NONE'),
  }));
  // a hell terrain is one some other terrain turns into
  const hell = new Set(K.terrain.map((t) => t.up).filter((u) => u !== 'NONE'));
  for (const t of K.terrain) {
    t.hell = hell.has(t.type) ? 1 : 0;
    t.twin = t.hell ? K.terrain.find((o) => o.up === t.type).type : 'NONE';
  }
  K.feature = infos(xmlDir, 'Terrain/CIV4FeatureInfos.xml', 'FeatureInfos', 'FeatureInfo').map((e) => ({
    type: val(e, 'Type'), impassable: flag(e, 'bImpassable'), graphical: flag(e, 'bGraphicalOnly'),
  }));
  K.bonus = infos(xmlDir, 'Terrain/CIV4BonusInfos.xml', 'BonusInfos', 'BonusInfo').map((e) => {
    const y = (kid(e, 'YieldChanges')?.children || []).map((c) => parseInt(c.text, 10) || 0);
    const cls = short(val(e, 'BonusClassType'), 'BONUSCLASS_');
    const health = num(e, 'iHealth'), happy = num(e, 'iHappiness');
    // heuristic resource class (docs/ART.md 6.1: food, strategic, luxury, mana); manifests may override per type
    let cat = 'luxury';
    if (cls === 'MANA' || cls === 'RAWMANA') cat = 'mana';
    else if (cls === 'RUSH' || cls === 'WONDER' || cls === 'MODERN') cat = 'strategic';
    else if (cls === 'GRAIN' || cls === 'LIVESTOCK' || (health > 0 && happy === 0)) cat = 'food';
    return { type: val(e, 'Type'), cls, cat, health, happy, food: y[0] || 0 };
  });
  K.improvement = infos(xmlDir, 'Terrain/CIV4ImprovementInfos.xml', 'ImprovementInfos', 'ImprovementInfo').map((e) => {
    const type = val(e, 'Type');
    return {
      type, permanent: flag(e, 'bPermanent'), unique: flag(e, 'bUnique'), goody: flag(e, 'bGoody'),
      graphical: flag(e, 'bGraphicalOnly'), water: flag(e, 'bWater'), actsAsCity: flag(e, 'bActsAsCity'),
      spawn: val(e, 'SpawnUnitType', 'NONE'), civ: val(e, 'PrereqCivilization', 'NONE'),
      mana: type.startsWith('IMPROVEMENT_MANA_') ? 1 : 0,
    };
  });
  K.route = infos(xmlDir, 'Misc/CIV4RouteInfos.xml', 'RouteInfos', 'RouteInfo').map((e) => ({ type: val(e, 'Type') }));

  const promoEls = infos(xmlDir, 'Units/CIV4PromotionInfos.xml', 'PromotionInfos', 'PromotionInfo');
  const raceSet = new Set(promoEls.filter((e) => flag(e, 'bRace')).map((e) => val(e, 'Type')));
  K.promotion = promoEls.map((e) => {
    const type = val(e, 'Type');
    const name = type.replace('PROMOTION_', '');
    const m = /^(.*?)(\d)$/.exec(name);
    const line = m ? m[1] : name;
    const bonus = val(e, 'BonusPrereq', 'NONE');
    let sphere = 'NONE';
    if (bonus.startsWith('BONUS_MANA_')) sphere = bonus.replace('BONUS_MANA_', '');
    else if (m && SPHERES.includes(line)) sphere = line;
    return {
      type, race: flag(e, 'bRace'), equipment: flag(e, 'bEquipment'), sphere, line, level: m ? +m[2] : 0,
      dispellable: flag(e, 'bDispellable'), expires: num(e, 'iExpireChance') > 0 ? 1 : 0, mutation: flag(e, 'bMutation'),
    };
  });

  const unitClassEls = infos(xmlDir, 'Units/CIV4UnitClassInfos.xml', 'UnitClassInfos', 'UnitClassInfo');
  const classDefault = new Map(unitClassEls.map((e) => [val(e, 'Type'), val(e, 'DefaultUnit', 'NONE')]));
  K.unit = infos(xmlDir, 'Units/CIV4UnitInfos.xml', 'UnitInfos', 'UnitInfo').map((e) => {
    const type = val(e, 'Type');
    const cls = val(e, 'Class');
    const free = kids(kid(e, 'FreePromotions') || { children: [] }, 'FreePromotion')
      .filter((f) => val(f, 'bFreePromotion') === '1').map((f) => val(f, 'PromotionType'));
    const races = free.filter((p) => raceSet.has(p)).map((p) => p.replace('PROMOTION_', ''));
    const def = classDefault.get(cls) || 'NONE';
    return {
      type, cls, combat: short(val(e, 'Combat'), 'UNITCOMBAT_'), domain: short(val(e, 'Domain'), 'DOMAIN_'),
      tier: num(e, 'iTier'), special: short(val(e, 'Special'), 'SPECIALUNIT_'), race: races.length ? races : ['NONE'],
      hero: free.includes('PROMOTION_HERO') ? 1 : 0, equipment: type.startsWith('EQUIPMENT_') ? 1 : 0,
      invisible: val(e, 'Invisible', 'NONE') !== 'NONE' ? 1 : 0, strength: num(e, 'iCombat'),
      base: def !== 'NONE' && def !== type ? def : 'NONE',
    };
  });

  K.civ = infos(xmlDir, 'Civilizations/CIV4CivilizationInfos.xml', 'CivilizationInfos', 'CivilizationInfo').map((e) => ({
    type: val(e, 'Type'), playable: flag(e, 'bPlayable'),
  }));
  K.religion = infos(xmlDir, 'Gameinfo/CIV4ReligionInfo.xml', 'ReligionInfos', 'ReligionInfo').map((e) => ({ type: val(e, 'Type') }));

  const bclassEls = infos(xmlDir, 'Buildings/CIV4BuildingClassInfos.xml', 'BuildingClassInfos', 'BuildingClassInfo');
  const bclass = new Map(bclassEls.map((e) => [val(e, 'Type'), { global: num(e, 'iMaxGlobalInstances'), player: num(e, 'iMaxPlayerInstances') }]));
  K.building = infos(xmlDir, 'Buildings/CIV4BuildingInfos.xml', 'BuildingInfos', 'BuildingInfo').map((e) => {
    const cls = val(e, 'BuildingClass');
    const bc = bclass.get(cls) || { global: -1, player: -1 };
    const religion = val(e, 'PrereqReligion', 'NONE');
    const relType = val(e, 'ReligionType', 'NONE');
    return {
      type: val(e, 'Type'), cls,
      wonder: bc.global > 0 ? 'WORLD' : bc.player > 0 ? 'NATIONAL' : 'NONE',
      religion: religion !== 'NONE' ? religion : relType,
      defense: num(e, 'iDefense') > 0 ? 1 : 0,
      graphical: flag(e, 'bGraphicalOnly'),
    };
  });
  K.unitcombat = infos(xmlDir, 'Basicinfos/CIV4UnitCombatInfos.xml', 'UnitCombatInfos', 'UnitCombatInfo').map((e) => ({ type: val(e, 'Type') }));
  K.specialist = infos(xmlDir, 'Gameinfo/CIV4SpecialistInfos.xml', 'SpecialistInfos', 'SpecialistInfo').map((e) => {
    const type = val(e, 'Type');
    return { type, great: type.startsWith('SPECIALIST_GREAT_') ? 1 : 0 };
  });
  return K;
}

/** Stable text form: one entry per line, so diffs stay readable. */
export function formatTypes(K) {
  const out = ['{"source": "data/xml via crates/ffh-play/web/art/tools/gen-types.mjs; do not edit", "kinds": {'];
  const kinds = Object.keys(K);
  kinds.forEach((k, ki) => {
    out.push(`"${k}": [`);
    K[k].forEach((e, i) => out.push(JSON.stringify(e) + (i < K[k].length - 1 ? ',' : '')));
    out.push(']' + (ki < kinds.length - 1 ? ',' : ''));
  });
  out.push('}}');
  return out.join('\n') + '\n';
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (!existsSync(XML_DIR)) { console.error(`gen-types: no XML at ${XML_DIR}`); process.exit(2); }
  const text = formatTypes(buildTypes());
  const target = join(ART_DIR, 'types.json');
  if (process.argv.includes('--check')) {
    const cur = existsSync(target) ? readFileSync(target, 'utf8') : '';
    if (cur !== text) { console.error('gen-types: types.json is stale; run node tools/gen-types.mjs'); process.exit(1); }
    console.log('gen-types: types.json is current');
  } else {
    writeFileSync(target, text);
    const K = JSON.parse(text).kinds;
    console.log('gen-types: wrote types.json:', Object.entries(K).map(([k, v]) => `${k} ${v.length}`).join(', '));
  }
}
