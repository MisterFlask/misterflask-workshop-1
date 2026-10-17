"""Install the composed units: python3 integrate_composed.py WORK ART_DIR

- WORK/out/units_composed.spr -> units/composed.spr, with `: light` metadata for the figures that
  carry a flame or a glow (placed on their brightest pixels);
- the manifest: composed.spr sourced, the hand-drawn legacy files dropped, the exact and category
  rules rewritten for the new sprites (per-religion faiths, per-material golems, per-dragon colours:
  bespoke sprites, so the in-game remaps are gone);
- the legacy unit files (generic, infantry, mounted, robed, siege_naval, animals, natures, heroes)
  are deleted."""
import os, re, sys

WORK, ART = sys.argv[1], sys.argv[2]
U = f'{ART}/units'
LEGACY = ['generic', 'infantry', 'mounted', 'robed', 'siege_naval', 'animals', 'natures', 'heroes']

# figure -> (table, radius): glows kept from the hand-drawn set, now on the composed figures
LIGHTS = {
    'unit/fire_elemental': ('FIRE', 6), 'unit/aurealis': ('FIRE', 6), 'unit/pyre_zombie': ('FIRE', 5), 'unit/balor': ('FIRE', 5),
    'unit/hyborem': ('FIRE', 6), 'unit/wrath': ('FIRE', 7), 'unit/seraph': ('FIRE', 5), 'unit/disciple_of_acheron': ('FIRE', 4),
    'unit/fireball': ('FIRE', 6), 'unit/meteor': ('FIRE', 5), 'unit/arcane_barge': ('ARC', 6), 'unit/lightning_elemental': ('ARC', 5),
}
BRIGHT = 'L8lFW'


def add_light(rows, table, radius):
    best = None
    for k in BRIGHT:
        pts = [(x, y) for y, r in enumerate(rows) for x, c in enumerate(r) if c == k]
        if pts:
            best = pts
            break
    if not best:
        return None
    cx = round(sum(p[0] for p in best) / len(best))
    cy = round(sum(p[1] for p in best) / len(best))
    # snap onto an actual bright pixel nearest the centroid
    x, y = min(best, key=lambda p: (p[0] - cx) ** 2 + (p[1] - cy) ** 2)
    return f': light {x},{y} {table} {radius}'


text = open(f'{WORK}/out/units_composed.spr').read().split('\n')
out, i = [], 0
while i < len(text):
    line = text[i]
    m = re.match(r'^=\s*(\S+)\s+(\d+)x(\d+)', line)
    if not m:
        out.append(line)
        i += 1
        continue
    name, h = m.group(1), int(m.group(3))
    out.append(line)
    i += 1
    meta = []
    while i < len(text) and text[i].startswith(':'):
        meta.append(text[i])
        i += 1
    rows = text[i:i + h]
    i += h
    if name in LIGHTS:
        l = add_light(rows, *LIGHTS[name])
        if l:
            meta.append(l)
    out += meta + rows
open(f'{U}/composed.spr', 'w').write('\n'.join(out))

man = open(f'{U}/manifest.txt').read()
head, rest = man.split('; ================================================================ exact rules', 1)
head = re.sub(r'; Sources: generic\.spr.*?heroes\.spr\.\n',
              '; Sources: atlas.spr (figures cut from the unit atlases, tools/atlas/quantize.py) and composed.spr\n'
              '; (figures composed from atlas parts and painted shapes, tools/atlas/compose_units.py).\n', head, flags=re.S)
head = re.sub(r';   naval     one silhouette.*?main truck\.\n', '', head, flags=re.S)
head = re.sub(r'source atlas\.spr\n(source \w+\.spr\n)+', 'source atlas.spr\nsource composed.spr\n', head)

EXACT = '''; ================================================================ exact rules
; Faiths (disciple_*, priest_of_*, high_priest_of_*), golems (*_golem), dragons (abashi, acheron,
; drifa, eurabatres), elementals and the demon and angel variants are bespoke sprites: no remaps.
map unit UNIT_LIGHTBRINGER -> unit/disciple_empyrean
map unit UNIT_PARAMANDER -> unit/priest_of_the_empyrean
map unit UNIT_EINHERJAR -> unit/einherjar ghost
; ---- elementals and spells made of their element (docs/ART.md 10.4)
map unit UNIT_AZER -> unit/fire_elemental
map unit UNIT_AIR_ELEMENTAL|UNIT_DJINN|UNIT_MISTFORM -> unit/air_elemental ghost
map unit UNIT_LIGHTNING_ELEMENTAL -> unit/lightning_elemental ghost
map unit UNIT_YVAIN -> unit/treant
map unit UNIT_SAND_LION -> unit/lion remap=ij,ui,yY,hu
map unit UNIT_SEVERED_SOUL -> unit/severed_soul ghost
; ---- ghosts, illusions and puppets are translucent (docs/ART.md 4.5)
map unit UNIT_SPECTRE -> unit/spectre ghost
map unit UNIT_WRAITH -> unit/wraith ghost
map unit UNIT_SAILORS_DIRGE -> unit/sailors_dirge ghost
map unit UNIT_GHOST -> unit/assassin ghost
map unit UNIT_SHADE -> unit/spectre remap=53,64,f5,HD,42 ghost
map unit UNIT_DIVIDED_SOUL -> unit/hunter ghost
map unit UNIT_PUPPET|UNIT_PUPPET_LOKI -> unit/puppet ghost
map unit UNIT_MIMIC -> unit/champion ghost
map unit UNIT_EIDOLON -> unit/spectre remap=4p,5P,6P,fP ghost
; ---- unique units of another nature than their base unit
map unit UNIT_MOROI|UNIT_BRUJAH|UNIT_VAMPIRE_LORD -> unit/vampire
map unit UNIT_OGRE_WARCHIEF|UNIT_STONESKIN_OGRE -> unit/ogre
map unit UNIT_POLAR_BEAR -> unit/bear remap=uG,hD,iH,j8,rD
map unit UNIT_BLOODPET -> unit/warrior remap=hc,uc,rI,jr,iI
map unit UNIT_LIZARDMAN|UNIT_LIZARDMAN_RANGER|UNIT_LIZARDMAN_ASSASSIN -> unit/scout remap=ao,bm,Bw,Dz,GA
map unit UNIT_LIZARDMAN_BEASTMASTER -> unit/beastmaster remap=ao,bm,Bw,Dz,GA
map unit UNIT_GOBLIN_SCORPION_CLAN -> unit/goblin
map unit UNIT_WOLF_RIDER_SCORPION_CLAN -> unit/wolf_rider
map unit UNIT_STEPHANOS|UNIT_BUBOES|UNIT_YERSINIA|UNIT_ARS -> unit/death_knight
map unit UNIT_CENTAUR_CHARGER -> unit/centaur
map unit UNIT_CHARIOT_SCORPION_CLAN -> unit/chariot
map unit UNIT_ARCHER_SCORPION_CLAN -> unit/archer remap=uz,jA,iz
map unit UNIT_FROSTLING_ARCHER -> unit/archer remap=aF,bf,ug,ig,jf,rN
map unit UNIT_FROSTLING -> unit/scout remap=aF,bf,ug,hN,Bg,Df,GF
map unit UNIT_FROSTLING_WOLF_RIDER -> unit/wolf_rider remap=aF,bf,Dg,Bg,3N,4g
map unit UNIT_DWARVEN_DRUID|UNIT_LIZARDMAN_DRUID -> unit/druid
map unit UNIT_HORNGUARD -> unit/boar_rider
map unit UNIT_BRIGIT|UNIT_BRIGIT_HELD -> unit/valkyrie
map unit UNIT_DUIN|UNIT_GREATER_WEREWOLF|UNIT_RAVENOUS_WEREWOLF -> unit/werewolf
map unit UNIT_MUIRIN -> unit/drown
map unit UNIT_BOARDING_PARTY -> unit/guybrush
map unit UNIT_BEAST_OF_AGARES -> unit/pit_beast
map unit UNIT_IRA -> unit/chaos_marauder
map unit UNIT_SAVEROUS -> unit/chaos_marauder remap=It,ck,CT,RS
map unit UNIT_MINOTAUR -> unit/berserker
map unit UNIT_ROSIER|UNIT_ROSIER_OATHTAKER -> unit/knight remap=BI,DC,GR
map unit UNIT_HEMAH|UNIT_SON_OF_THE_INFERNO -> unit/archmage
map unit UNIT_GAELAN|UNIT_MOBIUS_WITCH -> unit/mage
map unit UNIT_GIBBON -> unit/mage ghost

'''
CATEGORY = '''; ================================================================ category rules
; ---- natures that override the class silhouette (10.4)
map unit equipment=1 -> unit/equipment_treasure
map unit special=SPELL -> unit/fireball prio=20
map unit race=UNDEAD -> unit/skeleton prio=15
map unit race=DEMON -> unit/imp prio=15
map unit race=ANGEL -> unit/angel prio=5
map unit race=GOLEM -> unit/iron_golem prio=5
map unit race=ELEMENTAL -> unit/earth_elemental prio=5
map unit race=DRAGON -> unit/acheron prio=5
map unit race=ILLUSION|PUPPET -> unit/puppet ghost prio=5
map unit race=DWARF combat=MELEE -> unit/dwarven_soldier_runes
map unit race=DWARF combat=ARCHER -> unit/dwarven_slinger
map unit race=ORC combat=MELEE -> unit/ogre
map unit race=ORC combat=RECON -> unit/goblin
map unit race=CENTAUR -> unit/centaur
map unit domain=AIR -> unit/hawk
map unit domain=SEA combat=ANIMAL|BEAST -> unit/sea_serpent

; ---- one silhouette per combat class (10.2), refined by tier (10.3): tier is gear, not size
map unit combat=MELEE -> unit/axeman
map unit combat=MELEE tier=1 -> unit/warrior
map unit combat=MELEE tier=3 -> unit/champion
map unit combat=MELEE tier=4 -> unit/immortal
map unit combat=ARCHER -> unit/archer
map unit combat=ARCHER tier=3 -> unit/longbowman
map unit combat=ARCHER tier=4 -> unit/crossbowman
map unit combat=MOUNTED -> unit/horseman
map unit combat=MOUNTED tier=3 -> unit/horse_archer
map unit combat=MOUNTED tier=4 -> unit/knight
map unit combat=RECON -> unit/hunter
map unit combat=RECON tier=1 -> unit/scout
map unit combat=RECON tier=3 -> unit/ranger
map unit combat=RECON tier=4 -> unit/shadow
map unit combat=SIEGE -> unit/catapult
map unit combat=ADEPT -> unit/adept
map unit combat=ADEPT tier=3 -> unit/mage
map unit combat=ADEPT tier=4 -> unit/archmage
map unit combat=DISCIPLE -> unit/disciple
map unit combat=DISCIPLE tier=3 -> unit/priest_of_the_order
map unit combat=DISCIPLE tier=4 -> unit/high_priest_of_the_order
map unit combat=NAVAL -> unit/galley
map unit combat=ANIMAL -> unit/wolf
map unit combat=BEAST -> unit/bear
map unit domain=SEA -> unit/galley
map unit * -> unit/worker
'''
open(f'{U}/manifest.txt', 'w').write(head + EXACT + CATEGORY)

for f in LEGACY:
    p = f'{U}/{f}.spr'
    if os.path.exists(p):
        os.remove(p)
print('composed.spr written; manifest rewritten; legacy files removed:', ' '.join(LEGACY))
