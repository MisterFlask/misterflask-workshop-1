//! English game text (TXT_KEY_* -> string), extracted by tools/vendor_xml.py.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Text {
    map: HashMap<String, String>,
}

impl Text {
    pub fn parse_tsv(src: &str) -> Text {
        let mut map = HashMap::new();
        for line in src.lines() {
            if let Some((k, v)) = line.split_once('\t') {
                map.insert(k.to_string(), v.replace("\\n", "\n"));
            }
        }
        Text { map }
    }

    /// Text for a key; falls back to the key itself.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.map.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// Short display name: text with Civ4 markup like `[COLOR_..]` / `%s1` left intact,
    /// truncated at the first ':' list separator FFH2 uses for gendered forms.
    pub fn name<'a>(&'a self, key: &'a str) -> &'a str {
        let t = self.get(key);
        t.split(':').next().unwrap_or(t)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
