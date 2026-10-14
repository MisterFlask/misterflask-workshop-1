//! Global defines (C++ `GC.getDefineINT` / `getDefineFLOAT` / `getDefineSTRING`).
//!
//! Files are read in C++ order -- GlobalDefines.xml, GlobalDefinesAlt.xml,
//! PythonCallbackDefines.xml -- later files overriding earlier ones. Text values
//! that name a type resolve to that type's index (C++ `SetPostGlobalsGlobalDefines`).
//! Prefer the typed [`crate::Defines`] struct (generated) in game code.

use std::collections::HashMap;

use crate::registry::Registry;
use crate::xml::XmlDoc;

#[derive(Debug, Clone)]
pub enum DefineVal {
    Int(i32),
    Float(f32),
    /// Text value and its resolved type index (-1 if it is not a type name).
    Text(String, i32),
}

#[derive(Debug, Clone, Default)]
pub struct DefineMap {
    pub map: HashMap<String, DefineVal>,
}

impl DefineMap {
    pub fn read_file(&mut self, doc: &XmlDoc, reg: &Registry) {
        for d in doc.root().children() {
            let name = match d.child_text("DefineName") {
                Some(n) => n.to_string(),
                None => continue,
            };
            let mut val = None;
            for c in d.children() {
                let t = c.text();
                match c.name() {
                    "iDefineIntVal" => val = Some(DefineVal::Int(t.parse().unwrap_or(0))),
                    "fDefineFloatVal" => val = Some(DefineVal::Float(t.parse().unwrap_or(0.0))),
                    "DefineTextVal" => {
                        val = Some(DefineVal::Text(t.to_string(), reg.info_type_for_string(t)))
                    }
                    "bDefineBoolVal" => val = Some(DefineVal::Int((t == "1") as i32)),
                    _ => {}
                }
            }
            if let Some(v) = val {
                self.map.insert(name, v);
            }
        }
    }

    /// C++ `getDefineINT`: 0 when missing.
    pub fn int(&self, name: &str) -> i32 {
        match self.map.get(name) {
            Some(DefineVal::Int(i)) => *i,
            Some(DefineVal::Float(f)) => *f as i32,
            Some(DefineVal::Text(_, i)) => *i,
            None => 0,
        }
    }

    /// C++ `getDefineFLOAT`: 0.0 when missing.
    pub fn float(&self, name: &str) -> f32 {
        match self.map.get(name) {
            Some(DefineVal::Float(f)) => *f,
            Some(DefineVal::Int(i)) => *i as f32,
            _ => 0.0,
        }
    }

    /// C++ `getDefineSTRING`: "" when missing.
    pub fn string(&self, name: &str) -> &str {
        match self.map.get(name) {
            Some(DefineVal::Text(s, _)) => s,
            _ => "",
        }
    }
}
