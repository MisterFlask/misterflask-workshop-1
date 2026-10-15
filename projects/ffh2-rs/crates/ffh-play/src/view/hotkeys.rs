//! The action hotkeys and order priorities (design §7 P6 "hotkeys"): what the Civ4 interface
//! binds to each action (`<HotKey>`, `<bAltDown>`, `<bShiftDown>`, `<bCtrlDown>`,
//! `<iHotKeyPriority>` and the `...Alt` second binding, CvInfos.cpp `CvHotkeyInfo::read`) and the
//! order of the action buttons (`<iOrderPriority>`, by which `CvXMLLoadUtility::
//! SetGlobalActionInfo` sorts the action list the main interface draws).
//!
//! ffh-data does not load these interface fields, so they are read here from the same vendored
//! XML files (`ffh_data::default_data_dir()`): missions, commands and builds (FfH2's spells and
//! promotions have no hotkeys). The interface modes' XML (`CIV4InterfaceModeInfos.xml`) is not
//! vendored; its bindings are listed in [`INTERFACE_MODES`] as the reference file has them. The
//! exe's dispatch of a key press is not in the DLL source: the client takes, among the
//! selection's enabled actions bound to the key, the one with the highest hotkey priority, ties
//! in the action list's order.

use std::path::Path;

use crate::json::Json;

/// One binding: the key (`KB_F`, `KB_SPACE`, ...), the modifiers and the hotkey priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub key: String,
    pub alt: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub priority: i32,
}

impl Binding {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "key" => self.key.as_str(), "alt" => self.alt, "shift" => self.shift, "ctrl" => self.ctrl, "priority" => self.priority }
    }
}

/// An action info's interface fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionKeys {
    pub bindings: Vec<Binding>,
    /// `iOrderPriority` (0 when absent, as `GetChildXmlValByName` leaves the default).
    pub order: i32,
}

impl ActionKeys {
    pub fn to_json(&self) -> Json {
        crate::jobj! { "keys" => Json::Arr(self.bindings.iter().map(Binding::to_json).collect()), "order" => self.order }
    }
}

/// The text of the first `<tag>..</tag>` in `block` (an empty element `<tag/>` is `""`).
fn element<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    if block.contains(&format!("<{tag}/>")) && !block.contains(&format!("<{tag}>")) {
        return Some("");
    }
    let open = format!("<{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find("</")? + start;
    Some(block[start..end].trim())
}

fn int(block: &str, tag: &str) -> i32 {
    element(block, tag).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn flag(block: &str, tag: &str) -> bool {
    int(block, tag) != 0
}

/// The interface fields of every `<{info}>` block of an XML text, by `<Type>`.
pub fn parse(xml: &str, info: &str) -> Vec<(String, ActionKeys)> {
    let (open, close) = (format!("<{info}>"), format!("</{info}>"));
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let Some(j) = after.find(&close) else { break };
        let block = &after[..j];
        rest = &after[j + close.len()..];
        let Some(ty) = element(block, "Type").or_else(|| element(block, "Automate")) else { continue };
        let mut keys = ActionKeys { bindings: Vec::new(), order: int(block, "iOrderPriority") };
        for alt in ["", "Alt"] {
            let key = element(block, &format!("HotKey{alt}")).unwrap_or("");
            if key.is_empty() {
                continue;
            }
            keys.bindings.push(Binding {
                key: key.to_string(),
                alt: flag(block, &format!("bAltDown{alt}")),
                shift: flag(block, &format!("bShiftDown{alt}")),
                ctrl: flag(block, &format!("bCtrlDown{alt}")),
                priority: int(block, &format!("iHotKeyPriority{alt}")),
            });
        }
        out.push((ty.to_string(), keys));
    }
    out
}

/// The XML files with action hotkeys: (kind, file under `xml/`, info element).
pub const FILES: &[(&str, &str, &str)] = &[
    ("MissionType", "Units/CIV4MissionInfos.xml", "MissionInfo"),
    ("CommandType", "Units/CIV4CommandInfos.xml", "CommandInfo"),
    ("BuildType", "Units/CIV4BuildInfos.xml", "BuildInfo"),
    // Automate infos have no `<Type>`: they are named by their `<Automate>`.
    ("AutomateType", "Units/CIV4AutomateInfos.xml", "AutomateInfo"),
];

/// The interface modes with a button (`bVisible`), their bindings (key, alt, shift, ctrl) and
/// hotkey priority as `Assets/XML/Interface/CIV4InterfaceModeInfos.xml` of FfH2 0.41 has them
/// (no `iOrderPriority` there).
pub const INTERFACE_MODES: &[(&str, &str, bool, bool, bool)] = &[
    ("INTERFACEMODE_GO_TO", "KB_G", false, false, false),
    ("INTERFACEMODE_GO_TO_TYPE", "KB_G", false, false, true),
    ("INTERFACEMODE_GO_TO_ALL", "KB_G", true, false, false),
    ("INTERFACEMODE_ROUTE_TO", "KB_R", true, false, false),
    ("INTERFACEMODE_AIRLIFT", "KB_A", false, true, false),
    ("INTERFACEMODE_NUKE", "KB_N", false, false, false),
    ("INTERFACEMODE_RECON", "KB_R", false, false, false),
    ("INTERFACEMODE_PARADROP", "KB_P", false, false, false),
    ("INTERFACEMODE_AIRBOMB", "KB_B", false, false, false),
    ("INTERFACEMODE_RANGE_ATTACK", "KB_B", false, false, false),
    ("INTERFACEMODE_AIRSTRIKE", "KB_S", false, false, false),
    ("INTERFACEMODE_REBASE", "KB_R", true, false, false),
];

/// The action keys of `kind` (`MissionType`, `CommandType`, `BuildType`) by type name, read from
/// the data directory `dir` (empty when the file cannot be read).
pub fn read(dir: &Path, kind: &str) -> Vec<(String, ActionKeys)> {
    let Some((_, file, info)) = FILES.iter().find(|(k, _, _)| *k == kind) else { return Vec::new() };
    match std::fs::read_to_string(dir.join("xml").join(file)) {
        Ok(xml) => parse(&xml, info),
        Err(e) => {
            eprintln!("ffh-play: no hotkeys for {kind}: {file}: {e}");
            Vec::new()
        }
    }
}

/// The interface modes for `/api/data`: `[{type, keys, order}]`.
pub fn interface_modes_json() -> Json {
    Json::Arr(
        INTERFACE_MODES
            .iter()
            .map(|&(ty, key, alt, shift, ctrl)| {
                let keys = ActionKeys { bindings: vec![Binding { key: key.into(), alt, shift, ctrl, priority: 0 }], order: 0 };
                keys.to_json().with("type", ty)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_are_read_from_the_vendored_xml() {
        let dir = ffh_data::default_data_dir();
        let missions = read(&dir, "MissionType");
        let get = |list: &[(String, ActionKeys)], t: &str| list.iter().find(|(n, _)| n == t).map(|(_, k)| k.clone()).unwrap_or_else(|| panic!("{t}"));
        let skip = get(&missions, "MISSION_SKIP");
        assert_eq!(skip.bindings[0], Binding { key: "KB_SPACE".into(), alt: false, shift: false, ctrl: false, priority: 1 });
        assert_eq!(skip.bindings[1].key, "KB_NUMPAD0");
        assert_eq!(skip.order, 13);
        assert_eq!(get(&missions, "MISSION_FORTIFY").bindings[0].key, "KB_F");
        assert_eq!(get(&missions, "MISSION_SENTRY").bindings[0].key, "KB_S");
        assert!(get(&missions, "MISSION_FOUND").bindings[0].priority > get(&missions, "MISSION_BOMBARD").bindings[0].priority);
        assert!(get(&missions, "MISSION_PILLAGE").bindings[0].shift);
        assert!(get(&missions, "MISSION_MOVE_TO").bindings.is_empty());
        // Every type the XML lists is a mission (and none is missed: one entry per type).
        let d = ffh_data::global();
        assert_eq!(missions.len(), crate::names::kind("MissionType").map(|k| (k.count)(d)).unwrap());
        let commands = read(&dir, "CommandType");
        assert_eq!(get(&commands, "COMMAND_DELETE").bindings[0].key, "KB_DELETE");
        assert!(get(&commands, "COMMAND_UNLOAD_ALL").bindings[0].shift);
        let automates = read(&dir, "AutomateType");
        assert_eq!(get(&automates, "AUTOMATE_BUILD").bindings[0].key, "KB_A");
        assert_eq!(get(&automates, "AUTOMATE_EXPLORE").bindings[0].key, "KB_E");
        assert_eq!(automates.len(), crate::names::kind("AutomateType").map(|k| (k.count)(d)).unwrap());
        let builds = read(&dir, "BuildType");
        assert_eq!(get(&builds, "BUILD_ROAD").bindings[0].key, "KB_R");
        assert!(get(&builds, "BUILD_REMOVE_FOREST").bindings[0].alt);
        assert_eq!(builds.len(), d.builds.len());
        assert!(read(Path::new("/nonexistent"), "MissionType").is_empty());
        assert_eq!(parse("<A><Type>X</Type><HotKey/><iOrderPriority>3</iOrderPriority></A>", "A"), vec![("X".to_string(), ActionKeys { bindings: vec![], order: 3 })]);
    }
}
