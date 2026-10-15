//! English game text (TXT_KEY_* -> string), extracted by tools/vendor_xml.py: one line per
//! `<TEXT>` entry, the tag and the raw content of its `<English>` element.
//!
//! Civ4's text format (C++ `CvGameText::read`, CvInfos.cpp): a language element holds either the
//! text itself (`<English>Global Civic: Liberty</English>`) or, in a language with grammatical
//! forms, `<Text>`, `<Gender>` and `<Plural>` children whose values list the forms separated by
//! ':' (`<German><Text>Samhain:Samhains:Samhain:Samhain</Text><Gender>Male:Male:Male:Male</Gender>
//! <Plural>0:0:0:0</Plural></German>`). Only such a `<Text>` value is a list of forms; plain text
//! keeps every ':' (prose such as "Single Currency: +1 Trade Route ..."). Descriptions are read as
//! form 0 (`CvInfoBase::getDescription(uiForm = 0)` -> `getObjectText(key, uiForm)`). FfH2's
//! English entries are all plain text.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Text {
    map: HashMap<String, Entry>,
}

#[derive(Debug, Clone)]
struct Entry {
    /// The plain text, or the `<Text>` value of an entry with forms.
    text: String,
    /// `text` lists ':'-separated forms (it came from a `<Text>` child).
    forms: bool,
}

impl Entry {
    /// C++ `CvGameText::read` of one language element's content.
    fn parse(raw: &str) -> Entry {
        // GetChildXmlValByName(wszTextVal, "Text"), else GetXmlVal: the whole value.
        if let Some(start) = raw.find("<Text>") {
            let rest = &raw[start + "<Text>".len()..];
            if let Some(end) = rest.find("</Text>") {
                return Entry { text: rest[..end].to_string(), forms: true };
            }
        }
        Entry { text: raw.to_string(), forms: false }
    }
}

impl Text {
    pub fn parse_tsv(src: &str) -> Text {
        let mut map = HashMap::new();
        for line in src.lines() {
            if let Some((k, v)) = line.split_once('\t') {
                map.insert(k.to_string(), Entry::parse(&v.replace("\\n", "\n")));
            }
        }
        Text { map }
    }

    /// Text for a key (an entry with forms: its `<Text>` value, every form); falls back to the
    /// key itself.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.map.get(key).map(|e| e.text.as_str()).unwrap_or(key)
    }

    /// Form `i` of a key's text (`getObjectText(key, i)`): plain text is its only form (0), an
    /// entry with forms lists them separated by ':'. `None` past the last form; a missing key is
    /// its own only form.
    pub fn form<'a>(&'a self, key: &'a str, i: usize) -> Option<&'a str> {
        match self.map.get(key) {
            Some(e) if e.forms => e.text.split(':').nth(i),
            _ => (i == 0).then(|| self.get(key)),
        }
    }

    /// Display name (`getDescription()`, form 0): the text with Civ4 markup like `[COLOR_..]` /
    /// `%s1` left intact; for an entry with grammatical forms, the first one. Plain text is whole,
    /// colons included ("Global Civic: Liberty").
    pub fn name<'a>(&'a self, key: &'a str) -> &'a str {
        self.form(key, 0).unwrap_or_else(|| self.get(key))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_keeps_its_colons() {
        let t = Text::parse_tsv(
            "TXT_KEY_VOTE_LIBERTY\tGlobal Civic: Liberty\n\
             TXT_KEY_VOTE_SINGLE_CURRENCY\tSingle Currency: +1 Trade Route ([ICON_TRADE]) in all cities\n\
             TXT_KEY_TWO_LINES\tFirst: one\\nSecond: two\n",
        );
        assert_eq!(t.name("TXT_KEY_VOTE_LIBERTY"), "Global Civic: Liberty");
        assert_eq!(t.get("TXT_KEY_VOTE_LIBERTY"), "Global Civic: Liberty");
        assert_eq!(t.name("TXT_KEY_VOTE_SINGLE_CURRENCY"), "Single Currency: +1 Trade Route ([ICON_TRADE]) in all cities");
        assert_eq!(t.name("TXT_KEY_TWO_LINES"), "First: one\nSecond: two");
        assert_eq!(t.form("TXT_KEY_VOTE_LIBERTY", 0), Some("Global Civic: Liberty"));
        assert_eq!(t.form("TXT_KEY_VOTE_LIBERTY", 1), None);
        // A missing key is its own text.
        assert_eq!(t.name("TXT_KEY_MISSING"), "TXT_KEY_MISSING");
        assert_eq!(t.form("TXT_KEY_MISSING", 1), None);
    }

    #[test]
    fn text_child_lists_forms() {
        // The shape of FfH2's German entries (CIV4GameText_FFH2.xml), as an English entry would be.
        let t = Text::parse_tsv(
            "TXT_KEY_FORMS\t<Text>Samhain:Samhains:Samhain:Samhain</Text><Gender>Male:Male:Male:Male</Gender><Plural>0:0:0:0</Plural>\n\
             TXT_KEY_ONE_FORM\t<Text>Bradelines Brunnen</Text><Gender>Male</Gender><Plural>0</Plural>\n",
        );
        assert_eq!(t.name("TXT_KEY_FORMS"), "Samhain");
        assert_eq!(t.form("TXT_KEY_FORMS", 1), Some("Samhains"));
        assert_eq!(t.form("TXT_KEY_FORMS", 4), None);
        assert_eq!(t.get("TXT_KEY_FORMS"), "Samhain:Samhains:Samhain:Samhain");
        assert_eq!(t.name("TXT_KEY_ONE_FORM"), "Bradelines Brunnen");
    }

    #[test]
    fn vendored_names_are_whole_and_forms_read_form_zero() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/text/en.tsv")).unwrap();
        let t = Text::parse_tsv(&src);
        // Prose keeps its ':' (it is not a forms separator); FfH2's own entries are plain text.
        assert_eq!(t.name("TXT_KEY_VOTE_LIBERTY"), "Global Civic: Liberty");
        assert!(t.name("TXT_KEY_VOTE_SINGLE_CURRENCY").starts_with("Single Currency: +1 Trade Route"));
        // The stock BtS/Civ4 text FfH2 inherits (tools/vendor_xml.py) does use Civ4 <Text>
        // grammatical forms for a few entries -- the vanilla civ adjectives
        // (`<Text>American:Americans</Text>`) -- and a description reads form 0, as CvGameText does.
        assert_eq!(t.name("TXT_KEY_CIV_AMERICA_ADJECTIVE"), "American");
        assert_eq!(t.form("TXT_KEY_CIV_AMERICA_ADJECTIVE", 1), Some("Americans"));
    }
}
