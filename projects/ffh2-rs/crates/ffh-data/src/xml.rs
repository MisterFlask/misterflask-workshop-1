//! Minimal, dependency-free XML DOM sufficient for Civ4 game XML.
//!
//! Supports: declarations, comments, elements, attributes (parsed and kept),
//! self-closing tags, character data with the five standard entities and
//! numeric character references, CDATA. No DTDs, no namespaces processing.
//! Files may be UTF-8 or ISO-8859-1 (Civ4 uses both); bytes that are not
//! valid UTF-8 are decoded as Latin-1.

use std::fmt;

#[derive(Debug)]
pub struct XmlError {
    pub msg: String,
    pub offset: usize,
}

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "xml error at byte {}: {}", self.offset, self.msg)
    }
}

impl std::error::Error for XmlError {}

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone)]
struct XNode {
    name: String,
    /// Concatenated direct character data (not trimmed).
    text: String,
    attrs: Vec<(String, String)>,
    first_child: u32,
    last_child: u32,
    next_sibling: u32,
    parent: u32,
}

/// A parsed XML document. Node 0 is the root element.
#[derive(Debug, Clone)]
pub struct XmlDoc {
    nodes: Vec<XNode>,
}

/// Borrowed handle to an element.
#[derive(Clone, Copy)]
pub struct Elem<'a> {
    doc: &'a XmlDoc,
    idx: u32,
}

impl<'a> fmt::Debug for Elem<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.name())
    }
}

pub fn decode_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

impl XmlDoc {
    pub fn parse_bytes(bytes: &[u8]) -> Result<XmlDoc, XmlError> {
        Self::parse(&decode_bytes(bytes))
    }

    pub fn parse(src: &str) -> Result<XmlDoc, XmlError> {
        let mut p = Parser { s: src.as_bytes(), src, pos: 0, nodes: Vec::new() };
        p.parse_document()?;
        if p.nodes.is_empty() {
            return Err(XmlError { msg: "no root element".into(), offset: 0 });
        }
        Ok(XmlDoc { nodes: p.nodes })
    }

    pub fn root(&self) -> Elem<'_> {
        Elem { doc: self, idx: 0 }
    }
}

impl<'a> Elem<'a> {
    pub fn name(&self) -> &'a str {
        &self.doc.nodes[self.idx as usize].name
    }

    /// Trimmed direct character content.
    pub fn text(&self) -> &'a str {
        self.doc.nodes[self.idx as usize].text.trim()
    }

    pub fn attr(&self, name: &str) -> Option<&'a str> {
        self.doc.nodes[self.idx as usize]
            .attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn parent(&self) -> Option<Elem<'a>> {
        let p = self.doc.nodes[self.idx as usize].parent;
        (p != NONE).then_some(Elem { doc: self.doc, idx: p })
    }

    pub fn children(&self) -> Children<'a> {
        Children { doc: self.doc, next: self.doc.nodes[self.idx as usize].first_child }
    }

    pub fn first_child(&self) -> Option<Elem<'a>> {
        self.children().next()
    }

    pub fn num_children(&self) -> usize {
        self.children().count()
    }

    pub fn has_children(&self) -> bool {
        self.doc.nodes[self.idx as usize].first_child != NONE
    }

    /// First direct child element with the given tag name.
    pub fn child(&self, name: &str) -> Option<Elem<'a>> {
        self.children().find(|c| c.name() == name)
    }

    /// All direct child elements with the given tag name.
    pub fn children_named<'b>(&self, name: &'b str) -> impl Iterator<Item = Elem<'a>> + 'b
    where
        'a: 'b,
    {
        self.children().filter(move |c| c.name() == name)
    }

    pub fn next_sibling(&self) -> Option<Elem<'a>> {
        let n = self.doc.nodes[self.idx as usize].next_sibling;
        (n != NONE).then_some(Elem { doc: self.doc, idx: n })
    }

    /// Text of the first child element with this tag, if present.
    pub fn child_text(&self, name: &str) -> Option<&'a str> {
        self.child(name).map(|c| c.text())
    }

    /// Depth-first search for the first descendant (or self) with the given name.
    pub fn find(&self, name: &str) -> Option<Elem<'a>> {
        if self.name() == name {
            return Some(*self);
        }
        for c in self.children() {
            if let Some(f) = c.find(name) {
                return Some(f);
            }
        }
        None
    }
}

pub struct Children<'a> {
    doc: &'a XmlDoc,
    next: u32,
}

impl<'a> Iterator for Children<'a> {
    type Item = Elem<'a>;
    fn next(&mut self) -> Option<Elem<'a>> {
        if self.next == NONE {
            return None;
        }
        let e = Elem { doc: self.doc, idx: self.next };
        self.next = self.doc.nodes[self.next as usize].next_sibling;
        Some(e)
    }
}

struct Parser<'s> {
    s: &'s [u8],
    src: &'s str,
    pos: usize,
    nodes: Vec<XNode>,
}

impl<'s> Parser<'s> {
    fn err<T>(&self, msg: impl Into<String>) -> Result<T, XmlError> {
        Err(XmlError { msg: msg.into(), offset: self.pos })
    }

    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.pos..].starts_with(pat.as_bytes())
    }

    fn skip_until(&mut self, pat: &str) -> Result<(), XmlError> {
        match self.src[self.pos..].find(pat) {
            Some(i) => {
                self.pos += i + pat.len();
                Ok(())
            }
            None => self.err(format!("unterminated construct, expected {pat}")),
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn parse_document(&mut self) -> Result<(), XmlError> {
        loop {
            self.skip_ws();
            if self.pos >= self.s.len() {
                return self.err("no root element");
            }
            if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if self.starts_with("<!") {
                self.skip_until(">")?;
            } else if self.starts_with("<") {
                self.parse_element(NONE)?;
                return Ok(());
            } else {
                return self.err("unexpected content before root element");
            }
        }
    }

    fn parse_name(&mut self) -> Result<String, XmlError> {
        let start = self.pos;
        while self.pos < self.s.len() {
            let c = self.s[self.pos];
            if c.is_ascii_whitespace() || c == b'>' || c == b'/' || c == b'=' {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            return self.err("expected name");
        }
        Ok(self.src[start..self.pos].to_string())
    }

    fn add_node(&mut self, name: String, parent: u32) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(XNode {
            name,
            text: String::new(),
            attrs: Vec::new(),
            first_child: NONE,
            last_child: NONE,
            next_sibling: NONE,
            parent,
        });
        if parent != NONE {
            let p = &mut self.nodes[parent as usize];
            if p.first_child == NONE {
                p.first_child = idx;
                p.last_child = idx;
            } else {
                let last = p.last_child;
                p.last_child = idx;
                self.nodes[last as usize].next_sibling = idx;
            }
        }
        idx
    }

    /// Parses an element starting at '<'. Iterative over children to avoid deep recursion issues.
    fn parse_element(&mut self, parent: u32) -> Result<(), XmlError> {
        // stack of open element indices
        let mut stack: Vec<u32> = Vec::new();
        let mut cur_parent = parent;
        loop {
            // At '<' of a start tag
            debug_assert!(self.s[self.pos] == b'<');
            self.pos += 1;
            let name = self.parse_name()?;
            let idx = self.add_node(name, cur_parent);
            // attributes
            let mut self_closing = false;
            loop {
                self.skip_ws();
                if self.pos >= self.s.len() {
                    return self.err("unexpected eof in tag");
                }
                match self.s[self.pos] {
                    b'/' => {
                        if self.s.get(self.pos + 1) == Some(&b'>') {
                            self.pos += 2;
                            self_closing = true;
                            break;
                        }
                        return self.err("stray '/' in tag");
                    }
                    b'>' => {
                        self.pos += 1;
                        break;
                    }
                    _ => {
                        let an = self.parse_name()?;
                        self.skip_ws();
                        if self.s.get(self.pos) != Some(&b'=') {
                            return self.err("expected '=' after attribute name");
                        }
                        self.pos += 1;
                        self.skip_ws();
                        let q = self.s[self.pos];
                        if q != b'"' && q != b'\'' {
                            return self.err("expected quoted attribute value");
                        }
                        self.pos += 1;
                        let start = self.pos;
                        while self.pos < self.s.len() && self.s[self.pos] != q {
                            self.pos += 1;
                        }
                        let raw = &self.src[start..self.pos];
                        self.pos += 1;
                        let val = unescape(raw);
                        self.nodes[idx as usize].attrs.push((an, val));
                    }
                }
            }
            if !self_closing {
                stack.push(idx);
                cur_parent = idx;
            }
            // content loop: read until next start tag (then continue outer loop) or until stack empties
            loop {
                if stack.is_empty() {
                    return Ok(());
                }
                if self.pos >= self.s.len() {
                    return self.err("unexpected eof in content");
                }
                if self.starts_with("<!--") {
                    self.skip_until("-->")?;
                } else if self.starts_with("<![CDATA[") {
                    self.pos += 9;
                    let start = self.pos;
                    self.skip_until("]]>")?;
                    let txt = &self.src[start..self.pos - 3];
                    let top = *stack.last().unwrap();
                    self.nodes[top as usize].text.push_str(txt);
                } else if self.starts_with("<?") {
                    self.skip_until("?>")?;
                } else if self.starts_with("</") {
                    self.pos += 2;
                    let name = self.parse_name()?;
                    self.skip_ws();
                    if self.s.get(self.pos) != Some(&b'>') {
                        return self.err("expected '>' in end tag");
                    }
                    self.pos += 1;
                    let top = stack.pop().unwrap();
                    if self.nodes[top as usize].name != name {
                        return self.err(format!(
                            "mismatched end tag </{}> for <{}>",
                            name, self.nodes[top as usize].name
                        ));
                    }
                    cur_parent = stack.last().copied().unwrap_or(parent);
                } else if self.s[self.pos] == b'<' {
                    break; // child start tag
                } else {
                    let start = self.pos;
                    while self.pos < self.s.len() && self.s[self.pos] != b'<' {
                        self.pos += 1;
                    }
                    let raw = &self.src[start..self.pos];
                    let top = *stack.last().unwrap();
                    if raw.contains('&') {
                        let t = unescape(raw);
                        self.nodes[top as usize].text.push_str(&t);
                    } else {
                        self.nodes[top as usize].text.push_str(raw);
                    }
                }
            }
        }
    }
}

pub fn unescape(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        match rest.find(';') {
            Some(j) => {
                let ent = &rest[1..j];
                let rep = match ent {
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "amp" => Some('&'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                    _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                    _ => None,
                };
                match rep {
                    Some(c) => {
                        out.push(c);
                        rest = &rest[j + 1..];
                    }
                    None => {
                        out.push('&');
                        rest = &rest[1..];
                    }
                }
            }
            None => {
                out.push_str(rest);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic() {
        let d = XmlDoc::parse(
            r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<!-- c -->
<Root xmlns="x-schema:foo.xml">
  <A>  1 </A>
  <!-- inner -->
  <B><C>x &amp; y</C><C/><C>z</C></B>
  <D attr='q'/>
</Root>"#,
        )
        .unwrap();
        let r = d.root();
        assert_eq!(r.name(), "Root");
        assert_eq!(r.child_text("A"), Some("1"));
        let b = r.child("B").unwrap();
        assert_eq!(b.num_children(), 3);
        let texts: Vec<_> = b.children().map(|c| c.text()).collect();
        assert_eq!(texts, vec!["x & y", "", "z"]);
        assert_eq!(r.child("D").unwrap().attr("attr"), Some("q"));
        assert_eq!(r.attr("xmlns"), Some("x-schema:foo.xml"));
    }
}
