#!/usr/bin/env python3
"""Vendor the gameplay XML this port loads into data/xml/.

Sources (see tools/fetch-reference.sh):
  REF/ffh2  -> FFH2 0.41 mod (Assets/XML)          -- wins when a file exists in both
  REF/bts   -> stock BTS files FFH2 inherits

GlobalDefines.xml is reconstructed: the BTS copy in the reference repo carries a
third-party mod's edits. Every key whose BTS value differs from the (identical)
vanilla/Warlords value was verified to be one of those edits, so it is reset to
the vanilla/Warlords value. See docs/DATA.md.

Text: English strings are extracted to data/text/en.tsv (TAG<TAB>text).

Usage: tools/vendor_xml.py [REF_DIR]
"""
import os, re, sys, shutil

REF = sys.argv[1] if len(sys.argv) > 1 else "/home/user/ref"
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "data"))
FFH = os.path.join(REF, "ffh2", "Assets", "XML")
BTS = os.path.join(REF, "bts", "sid-meiers-civilization-iv-beyond-the-sword")
BTS_EXP = os.path.join(BTS, "Beyond-the-Sword", "Assets", "XML")
BTS_ROOT = os.path.join(BTS, "Assets", "XML")
VANILLA = os.path.join(REF, "bts", "sid-meiers-civilization-iv", "Assets", "XML")
WARLORDS_DEFINES = os.path.join(BTS, "Warlords", "Assets", "XML", "GlobalDefines.xml")

# (output relative path, source path)
FFH_FILES = [
    "GlobalDefinesAlt.xml", "GlobalTypes.xml", "PythonCallbackDefines.xml",
    "Basicinfos/CIV4AlignmentInfos.xml", "Basicinfos/CIV4DamageInfos.xml",
    "Basicinfos/CIV4InvisibleInfos.xml", "Basicinfos/CIV4MemoryInfos.xml",
    "Basicinfos/CIV4UnitAIInfos.xml", "Basicinfos/CIV4UnitCombatInfos.xml",
    "Buildings/CIV4BuildingClassInfos.xml", "Buildings/CIV4BuildingInfos.xml",
    "Buildings/CIV4SpecialBuildingInfos.xml",
    "Civilizations/CIV4CivilizationInfos.xml", "Civilizations/CIV4LeaderHeadInfos.xml",
    "Civilizations/CIV4TraitInfos.xml",
    "Events/CIV4EventInfos.xml", "Events/CIV4EventTriggerInfos.xml",
    "Gameinfo/CIV4CivicInfos.xml", "Gameinfo/CIV4CivicOptionInfos.xml",
    "Gameinfo/CIV4CommerceInfo.xml", "Gameinfo/CIV4CorporationInfo.xml",
    "Gameinfo/CIV4CultureLevelInfo.xml", "Gameinfo/CIV4EmphasizeInfo.xml",
    "Gameinfo/CIV4EraInfos.xml", "Gameinfo/CIV4EspionageMissionInfo.xml",
    "Gameinfo/CIV4GameOptionInfos.xml", "Gameinfo/CIV4GameSpeedInfo.xml",
    "Gameinfo/CIV4GoodyInfo.xml", "Gameinfo/CIV4HandicapInfo.xml",
    "Gameinfo/CIV4ProcessInfo.xml", "Gameinfo/CIV4ProjectInfo.xml",
    "Gameinfo/CIV4ReligionInfo.xml", "Gameinfo/CIV4SpecialistInfos.xml",
    "Gameinfo/CIV4VictoryInfo.xml", "Gameinfo/CIV4VoteInfo.xml",
    "Gameinfo/CIV4VoteSourceInfos.xml", "Gameinfo/CIV4WorldInfo.xml",
    "Misc/CIV4RouteInfos.xml",
    "Technologies/CIV4TechInfos.xml",
    "Terrain/CIV4BonusClassInfos.xml", "Terrain/CIV4BonusInfos.xml",
    "Terrain/CIV4FeatureInfos.xml", "Terrain/CIV4ImprovementInfos.xml",
    "Terrain/CIV4TerrainInfos.xml",
    "Units/CIV4AutomateInfos.xml", "Units/CIV4BuildInfos.xml",
    "Units/CIV4CommandInfos.xml", "Units/CIV4MissionInfos.xml",
    "Units/CIV4PromotionInfos.xml", "Units/CIV4SpecialUnitInfos.xml",
    "Units/CIV4SpellInfos.xml", "Units/CIV4UnitClassInfos.xml",
    "Units/CIV4UnitInfos.xml",
]
BTS_FILES = [
    ("Basicinfos/CIV4AttitudeInfos.xml", os.path.join(BTS_EXP, "BasicInfos/CIV4AttitudeInfos.xml")),
    ("Basicinfos/CIV4DenialInfos.xml", os.path.join(BTS_EXP, "BasicInfos/CIV4DenialInfos.xml")),
    ("Basicinfos/CIV4DomainInfos.xml", os.path.join(BTS_EXP, "BasicInfos/CIV4DomainInfos.xml")),
    ("Basicinfos/CIV4CalendarInfos.xml", os.path.join(BTS_EXP, "BasicInfos/CIV4CalendarInfos.xml")),
    ("Basicinfos/CIV4SeasonInfos.xml", os.path.join(BTS_EXP, "BasicInfos/CIV4SeasonInfos.xml")),
    ("Basicinfos/CIV4MonthInfos.xml", os.path.join(BTS_EXP, "BasicInfos/CIV4MonthInfos.xml")),
    ("Terrain/CIV4YieldInfos.xml", os.path.join(BTS_ROOT, "Terrain/CIV4YieldInfos.xml")),
    ("Gameinfo/CIV4UpkeepInfo.xml", os.path.join(BTS_ROOT, "GameInfo/CIV4UpKeepInfo.xml")),
    ("Gameinfo/CIV4ClimateInfo.xml", os.path.join(BTS_ROOT, "GameInfo/CIV4ClimateInfo.xml")),
    ("Gameinfo/CIV4SeaLevelInfo.xml", os.path.join(BTS_ROOT, "GameInfo/CIV4SeaLevelInfo.xml")),
    # stock hurry values (30 hammers/pop, 3 gold/hammer); both BTS copies carry mod edits
    ("Gameinfo/CIV4HurryInfo.xml", os.path.join(VANILLA, "GameInfo/CIV4HurryInfo.xml")),
]

DEFINE_RE = re.compile(r'(<DefineName>(\w+)</DefineName>\s*<(\w+)>)([^<]*)(</\3>)')


def load_defines(path):
    s = open(path, encoding="latin-1").read()
    return {m.group(2): m.group(4) for m in DEFINE_RE.finditer(s)}


def reconstruct_global_defines(dst):
    bts_path = os.path.join(BTS_EXP, "GlobalDefines.xml")
    text = open(bts_path, encoding="latin-1").read()
    war = load_defines(WARLORDS_DEFINES)
    keep = {"CIV4_VERSION", "SAVE_VERSION"}  # genuine BTS changes
    changed = []

    def fix(m):
        key, val = m.group(2), m.group(4)
        if key in war and key not in keep and war[key] != val:
            changed.append((key, val, war[key]))
            return m.group(1) + war[key] + m.group(5)
        return m.group(0)

    text = DEFINE_RE.sub(fix, text)
    with open(dst, "w", encoding="latin-1") as f:
        f.write(text)
    return changed


_TEXT_RE = re.compile(r"<TEXT>\s*<Tag>([^<]+)</Tag>\s*<English>(.*?)</English>", re.S)


def read_english(src, into):
    """Merge the TXT_KEY -> English strings of one CIV4GameText file into `into` (later files and
    layers override earlier ones, as Civ4 loads them). The <English> value is kept raw: a
    `<Text>...</Text>` grammatical-forms child is parsed by ffh-data's Text (CvGameText::read)."""
    s = open(src, encoding="latin-1").read()
    for m in _TEXT_RE.finditer(s):
        tag, txt = m.group(1).strip(), m.group(2)
        into[tag] = txt.replace("\r", "").replace("\n", "\\n").replace("\t", " ")


def text_files(text_dir):
    """The gameplay CIV4GameText files of a Text dir, sorted; the civilopedia files are left out
    (the client has no civilopedia screen, and they are ~0.5 MB of unused prose)."""
    import glob
    return [f for f in sorted(glob.glob(os.path.join(text_dir, "CIV4GameText*.xml")))
            if "Civilopedia" not in os.path.basename(f)]


def extract_english(dst):
    """Vendor the English text the mod loads into en.tsv (TAG<TAB>text, sorted). FfH2 inherits the
    stock BtS text for the keys it does not override (e.g. TXT_KEY_UNIT_WARRIOR), so merge the stock
    layers in load order and let FfH2's own file override them, matching Civ4's base -> Warlords ->
    BtS -> mod precedence."""
    layers = [
        os.path.join(BTS_ROOT, "Text"),                               # base (vanilla) text
        os.path.join(BTS, "Warlords", "Assets", "XML", "Text"),       # Warlords
        os.path.join(BTS_EXP, "Text"),                                # Beyond the Sword
    ]
    strings = {}
    for text_dir in layers:
        for f in text_files(text_dir):
            read_english(f, strings)
    stock = len(strings)
    read_english(os.path.join(FFH, "Text", "CIV4GameText_FFH2.xml"), strings)  # FfH2 overrides
    with open(dst, "w", encoding="utf-8") as f:
        for tag in sorted(strings):
            f.write(f"{tag}\t{strings[tag]}\n")
    return len(strings), stock


def main():
    xml_out = os.path.join(OUT, "xml")
    if os.path.isdir(xml_out):
        shutil.rmtree(xml_out)
    for rel in FFH_FILES:
        dst = os.path.join(xml_out, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(os.path.join(FFH, rel), dst)
    for rel, src in BTS_FILES:
        dst = os.path.join(xml_out, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(src, dst)
    changed = reconstruct_global_defines(os.path.join(xml_out, "GlobalDefines.xml"))
    print(f"GlobalDefines.xml: reset {len(changed)} mod-edited values to stock:")
    for k, old, new in changed:
        print(f"  {k}: {old} -> {new}")
    os.makedirs(os.path.join(OUT, "text"), exist_ok=True)
    n, stock = extract_english(os.path.join(OUT, "text", "en.tsv"))
    print(f"text: {n} English strings ({stock} stock BtS/Civ4 keys FfH2 inherits + its own, FfH2 wins)")


if __name__ == "__main__":
    main()
