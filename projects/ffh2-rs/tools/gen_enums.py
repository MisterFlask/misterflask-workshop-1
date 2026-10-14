#!/usr/bin/env python3
"""Generate crates/ffh-data/src/enums.rs from the DLL's CvEnums.h.

Every C++ `XTypes` enum used by game rules becomes a Rust newtype `XType(u16)`
implementing `Idx`. Enum members become associated consts with the C++ names
(`UnitAIType::UNITAI_ATTACK`), so ported code reads like the original.
`NO_X = -1` is represented by `Option<XType>` and is not emitted.
`NUM_X` becomes `XType::NUM` when the enum has a fixed member list.

Usage: tools/gen_enums.py [REF_DIR]
"""
import os, re, sys

REF = sys.argv[1] if len(sys.argv) > 1 else "/home/user/ref"
HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(REF, "ffh2", "CvGameCoreDLL", "CvEnums.h")
OUT = os.path.normpath(os.path.join(HERE, "..", "crates", "ffh-data", "src", "enums.rs"))

# C++ enum name -> Rust type name. Order is irrelevant.
WANT = {
    "GameStateTypes": "GameStateType",
    "DirectionTypes": "DirectionType",
    "CardinalDirectionTypes": "CardinalDirectionType",
    "ClimateTypes": "ClimateType",
    "SeaLevelTypes": "SeaLevelType",
    "WorldSizeTypes": "WorldSizeType",
    "TerrainTypes": "TerrainType",
    "PlotTypes": "PlotType",
    "YieldTypes": "YieldType",
    "CommerceTypes": "CommerceType",
    "FlavorTypes": "FlavorType",
    "EmphasizeTypes": "EmphasizeType",
    "GameOptionTypes": "GameOptionType",
    "PlayerOptionTypes": "PlayerOptionType",
    "VictoryTypes": "VictoryType",
    "FeatureTypes": "FeatureType",
    "BonusTypes": "BonusType",
    "BonusClassTypes": "BonusClassType",
    "ImprovementTypes": "ImprovementType",
    "RouteTypes": "RouteType",
    "GoodyTypes": "GoodyType",
    "BuildTypes": "BuildType",
    "HandicapTypes": "HandicapType",
    "GameSpeedTypes": "GameSpeedType",
    "EraTypes": "EraType",
    "CivilizationTypes": "CivilizationType",
    "LeaderHeadTypes": "LeaderHeadType",
    "ArtStyleTypes": "ArtStyleType",
    "CitySizeTypes": "CitySizeType",
    "TraitTypes": "TraitType",
    "OrderTypes": "OrderType",
    "TaskTypes": "TaskType",
    "BuildingClassTypes": "BuildingClassType",
    "BuildingTypes": "BuildingType",
    "SpecialBuildingTypes": "SpecialBuildingType",
    "ProjectTypes": "ProjectType",
    "ProcessTypes": "ProcessType",
    "VoteTypes": "VoteType",
    "DenialTypes": "DenialType",
    "DomainTypes": "DomainType",
    "UnitClassTypes": "UnitClassType",
    "UnitTypes": "UnitType",
    "SpecialUnitTypes": "SpecialUnitType",
    "UnitCombatTypes": "UnitCombatType",
    "UnitAITypes": "UnitAIType",
    "InvisibleTypes": "InvisibleType",
    "VoteSourceTypes": "VoteSourceType",
    "ProbabilityTypes": "ProbabilityType",
    "ActivityTypes": "ActivityType",
    "AutomateTypes": "AutomateType",
    "MissionTypes": "MissionType",
    "MissionAITypes": "MissionAIType",
    "CommandTypes": "CommandType",
    "PromotionTypes": "PromotionType",
    "AlignmentTypes": "AlignmentType",
    "DamageTypes": "DamageType",
    "SpellTypes": "SpellType",
    "TechTypes": "TechType",
    "SpecialistTypes": "SpecialistType",
    "ReligionTypes": "ReligionType",
    "CorporationTypes": "CorporationType",
    "HurryTypes": "HurryType",
    "UpkeepTypes": "UpkeepType",
    "CultureLevelTypes": "CultureLevelType",
    "CivicOptionTypes": "CivicOptionType",
    "CivicTypes": "CivicType",
    "WarPlanTypes": "WarPlanType",
    "AreaAITypes": "AreaAIType",
    "TradeableItems": "TradeableItem",
    "DiploCommentTypes": "DiploCommentType",
    "ContactTypes": "ContactType",
    "MemoryTypes": "MemoryType",
    "AttitudeTypes": "AttitudeType",
    "DiplomacyPowerTypes": "DiplomacyPowerType",
    "FeatTypes": "FeatType",
    "EventTypes": "EventType",
    "EventTriggerTypes": "EventTriggerType",
    "EspionageMissionTypes": "EspionageMissionType",
    "UnitArtStyleTypes": "UnitArtStyleType",
    "CalendarTypes": "CalendarType",
    "SeasonTypes": "SeasonType",
    "MonthTypes": "MonthType",
    "AdvisorTypes": "AdvisorType",
    "ControlTypes": "ControlType",
    "InterfaceMessageTypes": "InterfaceMessageType",
}


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    s = re.sub(r"//[^\n]*", "", s)
    return s


def main():
    src = strip_comments(open(SRC, encoding="latin-1").read())
    enums = {}
    for m in re.finditer(r"enum\s+(?:DllExport\s+)?(\w+)\s*\{(.*?)\};", src, re.S):
        name, body = m.group(1), m.group(2)
        # keep the _USRDLL branch of #ifdef/#else/#endif blocks
        kept, skipping = [], False
        for line in body.split("\n"):
            t = line.strip()
            if t.startswith("#ifdef") or t.startswith("#ifndef") or t.startswith("#if "):
                continue
            if t.startswith("#else"):
                skipping = True
                continue
            if t.startswith("#endif"):
                skipping = False
                continue
            if not skipping:
                kept.append(line)
        body = "\n".join(kept)
        members = []
        val = -1
        for raw in body.split(","):
            raw = raw.strip()
            if not raw:
                continue
            if "=" in raw:
                n, v = [x.strip() for x in raw.split("=", 1)]
                try:
                    val = int(v, 0)
                except ValueError:
                    # refer to previous member or expression: try resolving simple names
                    prev = dict(members)
                    if v in prev:
                        val = prev[v]
                    else:
                        val = None
            else:
                n = raw
                val = None if val is None else val + 1
            members.append((n, val))
        enums[name] = members

    out = []
    out.append("//! GENERATED by tools/gen_enums.py from CvEnums.h -- do not edit by hand.\n")
    out.append("//! Each C++ `XTypes` enum is a newtype index `XType`; members are associated")
    out.append("//! consts with the original C++ names. `NO_X` is `Option::None`.\n")
    out.append("#![allow(non_upper_case_globals)]\n")
    out.append("use crate::define_idx;\n")
    missing = []
    for cname, rname in sorted(WANT.items(), key=lambda kv: kv[1]):
        if cname not in enums:
            missing.append(cname)
            out.append(f"define_idx!(/// C++ `{cname}` (not in CvEnums.h; data-defined)\n{rname});\n")
            continue
        mem = enums[cname]
        out.append(f"define_idx!(/// C++ `{cname}`\n{rname});")
        consts = []
        num = None
        for n, v in mem:
            if v is None:
                continue
            if n.startswith("NO_") and v == -1:
                continue
            if n.startswith("NUM_") or n.startswith("MAX_"):
                num = (n, v)
                continue
            if v < 0:
                continue
            consts.append((n, v))
        if consts or num:
            out.append(f"impl {rname} {{")
            for n, v in consts:
                out.append(f"    pub const {n}: {rname} = {rname}({v});")
            if num:
                out.append(f"    /// C++ `{num[0]}`")
                out.append(f"    pub const NUM: usize = {num[1]};")
            out.append("    /// C++ member names in declaration order (index = value).")
            names = ", ".join(f'"{n}"' for n, _ in consts)
            out.append(f"    pub const NAMES: &'static [&'static str] = &[{names}];")
            out.append("}")
        out.append("")
    with open(OUT, "w") as f:
        f.write("\n".join(out) + "\n")
    print(f"wrote {OUT}; data-only (not in CvEnums.h): {missing}")


if __name__ == "__main__":
    main()
