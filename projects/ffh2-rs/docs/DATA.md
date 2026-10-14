# Game data provenance

## Sources

| What | Source | Pin |
|---|---|---|
| FFH2 0.41 XML, Python, DLL source | github.com/jlippitt/FFH2-Better-AI | `39c753c` "Mod in initial state" (unmodified FFH2 0.41, pre-AI-mod) |
| Stock BTS XML that FFH2 inherits | github.com/rigiechang/Broken-BTS | `7bb2280` (BTS), `9a5358c` (vanilla) |

FFH2 ships only the XML files it changes; everything else is inherited from
Beyond the Sword 3.19. The inherited files vendored here:

- `Basicinfos/CIV4{Attitude,Denial,Domain,Calendar,Season,Month}Infos.xml`
- `Terrain/CIV4YieldInfos.xml`, `Gameinfo/CIV4{Upkeep,Climate,SeaLevel}Info.xml`
  (byte-identical across the vanilla/Warlords/BTS copies -> stock)
- `Gameinfo/CIV4HurryInfo.xml` from the vanilla copy (30 production per pop,
  3 gold per production). Both BTS copies in the reference repo carry mod edits
  (90/1 and 120/3).
- `GlobalDefines.xml`, reconstructed (below).

## GlobalDefines.xml reconstruction

The reference repo's BTS `GlobalDefines.xml` was edited by that repo's mod. For
all 30 keys where its value differs from the Warlords file, the vanilla, Warlords
and BTS-root copies all agree with each other, so these are the mod's edits and
are reset to the vanilla/Warlords value by `tools/vendor_xml.py` (printed when it
runs; e.g. `INITIAL_CITY_POPULATION 3 -> 1`, `UNIT_PRODUCTION_PERCENT 50 -> 100`).
`CIV4_VERSION`/`SAVE_VERSION` keep their BTS values. Defines that exist only in
BTS (94 keys) are taken as-is; they cannot be cross-checked against another copy.

Residual risk: a BTS-only define the mod also edited would go unnoticed. FFH2's
`GlobalDefinesAlt.xml` overrides 25 BTS keys on top of this.

## Quirks handled by the loader

- `CIV4CommandInfos.xml` contains a container nested inside the container;
  entries are flattened in place (matches the C++ enum order).
- `CIV4AutomateInfos.xml` entries have no `<Type>`; names come from the
  `AutomateTypes` list in `GlobalTypes.xml` (as in C++).
- Text-valued defines listed in C++ `SetPostGlobalsGlobalDefines` (`BARBARIAN_CIVILIZATION`,
  `SLAVE_UNIT`, ...) resolve to type indices; other text defines read as -1 as ints (C++
  `getDefineINT` returns the string's address there, matching no type; e.g. FLOOD_PLAINS_FEATURE).
