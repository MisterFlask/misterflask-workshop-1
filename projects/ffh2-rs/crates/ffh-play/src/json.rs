//! Minimal JSON for the ffh-play protocol, journal and views (std only: crates.io is unreachable).
//!
//! * [`Json`] keeps object members in insertion order (a `Vec`), so serialized output is a
//!   function of how the value was built: journals and views are byte-identical across runs.
//! * Integers are `i64` ([`Json::Int`]); other numbers are `f64` ([`Json::Float`]). The parser
//!   yields `Int` for every integer literal (no fraction, no exponent) that fits in an `i64`.
//!   So there is no `u64` conversion (a value above `i64::MAX` would come back as a lossy
//!   float): a `u64` that can exceed it, such as a hash, goes on the wire as a hex string.
//!   `usize` (lengths, indices) converts within `0..=i64::MAX`.
//! * The parser is strict RFC 8259: no comments, trailing commas, `NaN`, leading zeros, single
//!   quotes or raw control characters in strings. It also rejects duplicate object keys (so a
//!   command can never mean two things) and nesting deeper than [`MAX_DEPTH`] (input comes from
//!   untrusted clients). A lone UTF-16 surrogate in a `\u` escape decodes to U+FFFD.
//! * The writer escapes `"`, `\`, control characters, U+2028 and U+2029 and writes all other
//!   characters as UTF-8. Non-finite floats are written as `null`.
//! * Typed getters ([`Json::req`], [`Json::opt`], [`Json::get_or`] with [`FromJson`]) turn
//!   protocol fields into Rust values with a [`FieldError`] that names the field.

use std::fmt::{self, Write};

use ffh_core::ids::{AreaId, CityId, DealId, GroupId, PlayerId, TeamId, TriggeredId, UnitId, VoteId};
use ffh_data::Idx;

/// Maximum nesting of arrays and objects accepted by the parser.
pub const MAX_DEPTH: usize = 128;

/// A JSON value. Objects are ordered member lists; the parser guarantees unique keys.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Json {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

// ---------------------------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------------------------

/// Builds a [`Json::Obj`]: `jobj! { "id" => 12, "ok" => true }`. Values go through `Json::from`.
/// Keys must be distinct (the parser would reject the output otherwise).
#[macro_export]
macro_rules! jobj {
    () => { $crate::json::Json::Obj(::std::vec::Vec::new()) };
    ($($k:expr => $v:expr),+ $(,)?) => {
        $crate::json::Json::Obj(::std::vec![$((::std::string::String::from($k), $crate::json::Json::from($v))),+])
    };
}

/// Builds a [`Json::Arr`]: `jarr![1, "two", Json::Null]`. Values go through `Json::from`.
#[macro_export]
macro_rules! jarr {
    () => { $crate::json::Json::Arr(::std::vec::Vec::new()) };
    ($($v:expr),+ $(,)?) => { $crate::json::Json::Arr(::std::vec![$($crate::json::Json::from($v)),+]) };
}

impl Json {
    /// An empty object.
    pub fn obj() -> Json {
        Json::Obj(Vec::new())
    }

    /// An empty array.
    pub fn arr() -> Json {
        Json::Arr(Vec::new())
    }

    /// A type index as an integer (responses use indices; `/api/data` maps them to names).
    pub fn idx<I: Idx>(v: I) -> Json {
        Json::Int(v.idx() as i64)
    }

    /// An optional type index: `null` when none.
    pub fn opt_idx<I: Idx>(v: Option<I>) -> Json {
        v.map_or(Json::Null, Json::idx)
    }

    /// Sets member `key` of an object (replacing an existing member in place, else appending).
    /// Panics if `self` is not an object.
    pub fn set(&mut self, key: &str, v: impl Into<Json>) -> &mut Json {
        let Json::Obj(m) = self else { panic!("Json::set on {}", self.kind()) };
        let v = v.into();
        match m.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = v,
            None => m.push((key.to_string(), v)),
        }
        self
    }

    /// Builder form of [`Json::set`].
    pub fn with(mut self, key: &str, v: impl Into<Json>) -> Json {
        self.set(key, v);
        self
    }

    /// Appends to an array. Panics if `self` is not an array.
    pub fn push(&mut self, v: impl Into<Json>) {
        let Json::Arr(a) = self else { panic!("Json::push on {}", self.kind()) };
        a.push(v.into());
    }
}

impl From<bool> for Json {
    fn from(v: bool) -> Json {
        Json::Bool(v)
    }
}

macro_rules! from_int {
    ($($t:ty),*) => { $(impl From<$t> for Json { fn from(v: $t) -> Json { Json::Int(v as i64) } })* };
}
from_int!(i8, i16, i32, i64, u8, u16, u32);

/// Lengths and indices. Panics above `i64::MAX`, which no length or index reaches (they are at
/// most `isize::MAX`).
impl From<usize> for Json {
    fn from(v: usize) -> Json {
        Json::Int(i64::try_from(v).expect("usize above i64::MAX has no JSON integer form"))
    }
}

impl From<f64> for Json {
    fn from(v: f64) -> Json {
        Json::Float(v)
    }
}

impl From<&str> for Json {
    fn from(v: &str) -> Json {
        Json::Str(v.to_string())
    }
}

impl From<String> for Json {
    fn from(v: String) -> Json {
        Json::Str(v)
    }
}

impl From<&String> for Json {
    fn from(v: &String) -> Json {
        Json::Str(v.clone())
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(v: Option<T>) -> Json {
        v.map_or(Json::Null, Into::into)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(v: Vec<T>) -> Json {
        Json::Arr(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Clone + Into<Json>> From<&[T]> for Json {
    fn from(v: &[T]) -> Json {
        Json::Arr(v.iter().cloned().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------------------------
// Access
// ---------------------------------------------------------------------------------------------

impl Json {
    /// Name of the value's kind, for error messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a boolean",
            Json::Int(_) => "an integer",
            Json::Float(_) => "a number",
            Json::Str(_) => "a string",
            Json::Arr(_) => "an array",
            Json::Obj(_) => "an object",
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Integer value; a float with an integral value in `i64` range also counts (`1e2`).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(i) => Some(*i),
            // -2^63 is exact in f64; the range is [-2^63, 2^63).
            Json::Float(f) if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f < -(i64::MIN as f64) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Int(i) => Some(*i as f64),
            Json::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }

    /// Member `key` of an object (`None` if absent or `self` is not an object).
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.as_obj()?.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Obj(m) => m.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Element `i` of an array.
    pub fn at(&self, i: usize) -> Option<&Json> {
        self.as_arr()?.get(i)
    }

    /// Member lookup for the typed getters: `Err` if `self` is not an object.
    fn member(&self, key: &str) -> Result<Option<&Json>, FieldError> {
        match self {
            Json::Obj(m) => Ok(m.iter().find(|(k, _)| k == key).map(|(_, v)| v)),
            _ => Err(FieldError::new(key, format!("expected an object, got {}", self.kind()))),
        }
    }

    /// Required member (any value, including `null`).
    pub fn req_value(&self, key: &str) -> Result<&Json, FieldError> {
        self.member(key)?.ok_or_else(|| FieldError::missing(key))
    }

    /// Optional member: `None` when absent or `null`.
    pub fn opt_value(&self, key: &str) -> Result<Option<&Json>, FieldError> {
        Ok(self.member(key)?.filter(|v| !v.is_null()))
    }

    /// Required member converted to `T`.
    pub fn req<T: FromJson>(&self, key: &str) -> Result<T, FieldError> {
        let v = self.req_value(key)?;
        T::from_json(v).ok_or_else(|| FieldError::wrong(key, &T::expected(), v))
    }

    /// Optional member converted to `T` (`None` when absent or `null`; a wrong type is an error).
    pub fn opt<T: FromJson>(&self, key: &str) -> Result<Option<T>, FieldError> {
        match self.opt_value(key)? {
            None => Ok(None),
            Some(v) => T::from_json(v).map(Some).ok_or_else(|| FieldError::wrong(key, &T::expected(), v)),
        }
    }

    /// Optional member with a default.
    pub fn get_or<T: FromJson>(&self, key: &str, default: T) -> Result<T, FieldError> {
        Ok(self.opt(key)?.unwrap_or(default))
    }

    /// Required string member, borrowed.
    pub fn req_str(&self, key: &str) -> Result<&str, FieldError> {
        let v = self.req_value(key)?;
        v.as_str().ok_or_else(|| FieldError::wrong(key, "a string", v))
    }

    /// Optional string member, borrowed.
    pub fn opt_str(&self, key: &str) -> Result<Option<&str>, FieldError> {
        match self.opt_value(key)? {
            None => Ok(None),
            Some(v) => v.as_str().map(Some).ok_or_else(|| FieldError::wrong(key, "a string", v)),
        }
    }

    /// Required array member, borrowed.
    pub fn req_arr(&self, key: &str) -> Result<&[Json], FieldError> {
        let v = self.req_value(key)?;
        v.as_arr().ok_or_else(|| FieldError::wrong(key, "an array", v))
    }

    /// Every member's name is in `known` (a typo is an error, not a default silently taken).
    /// Not an object: an error too.
    pub fn only(&self, known: &[&str]) -> Result<(), FieldError> {
        match self {
            Json::Obj(m) => match m.iter().find(|(k, _)| !known.contains(&k.as_str())) {
                Some((k, _)) => Err(FieldError::new(k, format!("unknown field (known: {})", known.join(", ")))),
                None => Ok(()),
            },
            _ => Err(FieldError::new("", format!("expected an object, got {}", self.kind()))),
        }
    }

    /// Required object member, borrowed.
    pub fn req_obj(&self, key: &str) -> Result<&Json, FieldError> {
        let v = self.req_value(key)?;
        if v.as_obj().is_some() {
            Ok(v)
        } else {
            Err(FieldError::wrong(key, "an object", v))
        }
    }
}

/// A protocol field that is missing or has the wrong type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldError {
    pub key: String,
    pub msg: String,
}

impl FieldError {
    pub fn new(key: &str, msg: impl Into<String>) -> FieldError {
        FieldError { key: key.to_string(), msg: msg.into() }
    }

    pub fn missing(key: &str) -> FieldError {
        FieldError::new(key, "missing")
    }

    /// "expected <what>, got <snippet of v>".
    pub fn wrong(key: &str, expected: &str, v: &Json) -> FieldError {
        FieldError::new(key, format!("expected {expected}, got {}", snippet(v, 40)))
    }
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "field `{}`: {}", self.key, self.msg)
    }
}

impl std::error::Error for FieldError {}

/// Compact JSON of `v`, cut to at most `max` bytes (on a char boundary) with `...` appended.
pub fn snippet(v: &Json, max: usize) -> String {
    let mut s = v.to_string();
    if s.len() > max {
        let mut cut = max;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("...");
    }
    s
}

/// Conversion of a JSON value to a Rust value, for the typed getters.
pub trait FromJson: Sized {
    fn from_json(j: &Json) -> Option<Self>;
    /// What a valid value looks like, for error messages ("an integer in 0..=255").
    fn expected() -> String;
}

impl FromJson for Json {
    fn from_json(j: &Json) -> Option<Json> {
        Some(j.clone())
    }
    fn expected() -> String {
        "any value".into()
    }
}

impl FromJson for bool {
    fn from_json(j: &Json) -> Option<bool> {
        j.as_bool()
    }
    fn expected() -> String {
        "a boolean".into()
    }
}

macro_rules! from_json_int {
    ($($t:ty),*) => { $(
        impl FromJson for $t {
            fn from_json(j: &Json) -> Option<$t> {
                j.as_i64().and_then(|i| <$t>::try_from(i).ok())
            }
            fn expected() -> String {
                format!("an integer in {}..={}", <$t>::MIN, <$t>::MAX)
            }
        }
    )* };
}
from_json_int!(i8, i16, i32, i64, u8, u16, u32);

/// `0..=i64::MAX` (JSON integers are `i64`; on 32-bit targets `0..=usize::MAX`).
impl FromJson for usize {
    fn from_json(j: &Json) -> Option<usize> {
        j.as_i64().and_then(|i| usize::try_from(i).ok())
    }
    fn expected() -> String {
        format!("an integer in 0..={}", usize::try_from(i64::MAX).unwrap_or(usize::MAX))
    }
}

impl FromJson for f64 {
    fn from_json(j: &Json) -> Option<f64> {
        j.as_f64()
    }
    fn expected() -> String {
        "a number".into()
    }
}

impl FromJson for String {
    fn from_json(j: &Json) -> Option<String> {
        j.as_str().map(str::to_string)
    }
    fn expected() -> String {
        "a string".into()
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn from_json(j: &Json) -> Option<Vec<T>> {
        j.as_arr()?.iter().map(T::from_json).collect()
    }
    fn expected() -> String {
        format!("an array of ({})", T::expected())
    }
}

/// Entity ids (`ffh_core::ids`) are plain unsigned integers on the wire.
macro_rules! entity_ids {
    ($($t:ident($inner:ty) $what:literal),* $(,)?) => { $(
        impl From<$t> for Json {
            fn from(v: $t) -> Json {
                Json::Int(v.0 as i64)
            }
        }
        impl FromJson for $t {
            fn from_json(j: &Json) -> Option<$t> {
                <$inner>::from_json(j).map($t)
            }
            fn expected() -> String {
                format!("{} ({})", $what, <$inner>::expected())
            }
        }
    )* };
}
entity_ids! {
    PlayerId(u8) "a player id",
    TeamId(u8) "a team id",
    UnitId(u32) "a unit id",
    CityId(u32) "a city id",
    GroupId(u32) "a group id",
    AreaId(u32) "an area id",
    DealId(u32) "a deal id",
    TriggeredId(u32) "a triggered event id",
    VoteId(u32) "a vote id",
}

// ---------------------------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------------------------

impl Json {
    /// Writes compact JSON (no whitespace).
    pub fn write<W: Write>(&self, w: &mut W) -> fmt::Result {
        match self {
            Json::Null => w.write_str("null"),
            Json::Bool(b) => w.write_str(if *b { "true" } else { "false" }),
            Json::Int(i) => write!(w, "{i}"),
            Json::Float(f) => write_f64(w, *f),
            Json::Str(s) => write_escaped(w, s),
            Json::Arr(a) => {
                w.write_char('[')?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        w.write_char(',')?;
                    }
                    v.write(w)?;
                }
                w.write_char(']')
            }
            Json::Obj(m) => {
                w.write_char('{')?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        w.write_char(',')?;
                    }
                    write_escaped(w, k)?;
                    w.write_char(':')?;
                    v.write(w)?;
                }
                w.write_char('}')
            }
        }
    }

    /// Indented JSON (2 spaces) for people: CLI output, `journal print`, bug reports.
    /// Arrays whose elements are all scalars stay on one line.
    pub fn to_pretty(&self) -> String {
        let mut s = String::new();
        self.pretty_into(&mut s, 0);
        s
    }

    fn pretty_into(&self, out: &mut String, indent: usize) {
        fn pad(out: &mut String, n: usize) {
            for _ in 0..n {
                out.push_str("  ");
            }
        }
        match self {
            Json::Arr(a) if a.iter().any(|v| matches!(v, Json::Arr(_) | Json::Obj(_))) => {
                out.push_str("[\n");
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    pad(out, indent + 1);
                    v.pretty_into(out, indent + 1);
                }
                out.push('\n');
                pad(out, indent);
                out.push(']');
            }
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    let _ = v.write(out);
                }
                out.push(']');
            }
            Json::Obj(m) if !m.is_empty() => {
                out.push_str("{\n");
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push_str(",\n");
                    }
                    pad(out, indent + 1);
                    let _ = write_escaped(out, k);
                    out.push_str(": ");
                    v.pretty_into(out, indent + 1);
                }
                out.push('\n');
                pad(out, indent);
                out.push('}');
            }
            _ => {
                let _ = self.write(out);
            }
        }
    }
}

impl fmt::Display for Json {
    /// Compact JSON.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write(f)
    }
}

/// Finite floats use Rust's shortest round-trip form (`0.1`, `1.0`, `1e-7`; all valid JSON and
/// parsed back as `Float`), non-finite ones `null`.
fn write_f64<W: Write>(w: &mut W, f: f64) -> fmt::Result {
    if f.is_finite() {
        write!(w, "{f:?}")
    } else {
        w.write_str("null")
    }
}

/// Writes `s` as a quoted JSON string.
pub fn write_escaped<W: Write>(w: &mut W, s: &str) -> fmt::Result {
    w.write_char('"')?;
    let mut run = 0;
    for (i, c) in s.char_indices() {
        let short = match c {
            '"' => "\\\"",
            '\\' => "\\\\",
            '\n' => "\\n",
            '\r' => "\\r",
            '\t' => "\\t",
            '\u{8}' => "\\b",
            '\u{c}' => "\\f",
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => "",
            _ => continue,
        };
        w.write_str(&s[run..i])?;
        if short.is_empty() {
            write!(w, "\\u{:04x}", c as u32)?;
        } else {
            w.write_str(short)?;
        }
        run = i + c.len_utf8();
    }
    w.write_str(&s[run..])?;
    w.write_char('"')
}

/// `s` as a quoted JSON string.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    let _ = write_escaped(&mut out, s);
    out
}

// ---------------------------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------------------------

/// Where and why parsing failed (`pos` is a byte offset into the input).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub pos: usize,
    pub msg: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid JSON at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for ParseError {}

impl Json {
    /// Parses one JSON document (surrounding whitespace allowed, nothing else).
    pub fn parse(s: &str) -> Result<Json, ParseError> {
        let mut p = Parser { s, b: s.as_bytes(), i: 0, depth: 0 };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(p.err("trailing characters after the value"));
        }
        Ok(v)
    }

    /// Parses UTF-8 bytes (an HTTP body).
    pub fn parse_bytes(b: &[u8]) -> Result<Json, ParseError> {
        match std::str::from_utf8(b) {
            Ok(s) => Json::parse(s),
            Err(e) => Err(ParseError { pos: e.valid_up_to(), msg: "invalid UTF-8" }),
        }
    }
}

impl std::str::FromStr for Json {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Json, ParseError> {
        Json::parse(s)
    }
}

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn err(&self, msg: &'static str) -> ParseError {
        ParseError { pos: self.i, msg }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Json, ParseError> {
        match self.peek() {
            None => Err(self.err("unexpected end of input")),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::Str),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("expected a value")),
        }
    }

    fn literal(&mut self, word: &'static str, v: Json) -> Result<Json, ParseError> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.i += 1; // the opening bracket
        self.ws();
        Ok(())
    }

    /// After a member or element: `,` (true: another follows) or the closing bracket (false).
    fn next_or_close(&mut self, close: u8, msg: &'static str) -> Result<bool, ParseError> {
        self.ws();
        match self.peek() {
            Some(b',') => {
                self.i += 1;
                self.ws();
                Ok(true)
            }
            Some(c) if c == close => {
                self.i += 1;
                self.depth -= 1;
                Ok(false)
            }
            None => Err(self.err("unexpected end of input")),
            Some(_) => Err(self.err(msg)),
        }
    }

    fn array(&mut self) -> Result<Json, ParseError> {
        self.enter()?;
        let mut a = Vec::new();
        if self.peek() == Some(b']') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Json::Arr(a));
        }
        loop {
            a.push(self.value()?);
            if !self.next_or_close(b']', "expected ',' or ']'")? {
                return Ok(Json::Arr(a));
            }
        }
    }

    fn object(&mut self) -> Result<Json, ParseError> {
        let start = self.i;
        self.enter()?;
        let mut m: Vec<(String, Json)> = Vec::new();
        if self.peek() == Some(b'}') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Json::Obj(m));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a string key"));
            }
            let key_pos = self.i;
            let k = self.string()?;
            // Small objects: check as we go (exact position). Large ones: sort once at the end.
            if m.len() < 16 && m.iter().any(|(k2, _)| *k2 == k) {
                return Err(ParseError { pos: key_pos, msg: "duplicate key" });
            }
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.i += 1;
            self.ws();
            let v = self.value()?;
            m.push((k, v));
            if !self.next_or_close(b'}', "expected ',' or '}'")? {
                break;
            }
        }
        if m.len() > 16 {
            let mut keys: Vec<&str> = m.iter().map(|(k, _)| k.as_str()).collect();
            keys.sort_unstable();
            if keys.windows(2).any(|w| w[0] == w[1]) {
                return Err(ParseError { pos: start, msg: "duplicate key" });
            }
        }
        Ok(Json::Obj(m))
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let Some(h) = self.b.get(self.i..self.i + 4) else {
            return Err(self.err("truncated \\u escape"));
        };
        let mut v = 0;
        for &c in h {
            let Some(d) = (c as char).to_digit(16) else {
                return Err(self.err("invalid \\u escape"));
            };
            v = v * 16 + d;
        }
        self.i += 4;
        Ok(v)
    }

    /// A string starting at the opening quote.
    fn string(&mut self) -> Result<String, ParseError> {
        self.i += 1;
        let mut out = String::new();
        // Unescaped runs are copied as slices; they start and end next to ASCII bytes
        // ('"', '\\'), which are always char boundaries.
        let mut run = self.i;
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            match c {
                b'"' => {
                    out.push_str(&self.s[run..self.i]);
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    out.push_str(&self.s[run..self.i]);
                    self.i += 1;
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated string"));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let cp = self.hex4()?;
                            out.push(self.code_point(cp)?);
                        }
                        _ => {
                            self.i -= 1;
                            return Err(self.err("invalid escape"));
                        }
                    }
                    run = self.i;
                }
                0x00..=0x1f => return Err(self.err("control character in string")),
                _ => self.i += 1,
            }
        }
    }

    /// The character of a `\uXXXX` escape, joining a surrogate pair with a following `\uXXXX`.
    fn code_point(&mut self, cp: u32) -> Result<char, ParseError> {
        if (0xD800..0xDC00).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
            let save = self.i;
            self.i += 2;
            let lo = self.hex4()?;
            if (0xDC00..0xE000).contains(&lo) {
                let c = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                return Ok(char::from_u32(c).unwrap_or('\u{FFFD}'));
            }
            self.i = save; // not a low surrogate: decode it on its own
        }
        Ok(char::from_u32(cp).unwrap_or('\u{FFFD}'))
    }

    fn digits(&mut self) -> usize {
        let start = self.i;
        while let Some(b'0'..=b'9') = self.peek() {
            self.i += 1;
        }
        self.i - start
    }

    fn number(&mut self) -> Result<Json, ParseError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.i += 1;
                if let Some(b'0'..=b'9') = self.peek() {
                    return Err(self.err("leading zero in number"));
                }
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.err("invalid number")),
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            self.i += 1;
            float = true;
            if self.digits() == 0 {
                return Err(self.err("expected a digit after '.'"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.i += 1;
            float = true;
            if let Some(b'+' | b'-') = self.peek() {
                self.i += 1;
            }
            if self.digits() == 0 {
                return Err(self.err("expected a digit in the exponent"));
            }
        }
        let text = &self.s[start..self.i];
        if !float {
            if let Ok(v) = text.parse::<i64>() {
                return Ok(Json::Int(v));
            }
        }
        match text.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(Json::Float(f)),
            _ => Err(ParseError { pos: start, msg: "number out of range" }),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(v: &Json) {
        let s = v.to_string();
        assert_eq!(&Json::parse(&s).unwrap(), v, "compact round trip of {s}");
        let p = v.to_pretty();
        assert_eq!(&Json::parse(&p).unwrap(), v, "pretty round trip of {p}");
    }

    fn err(s: &str) -> ParseError {
        Json::parse(s).expect_err(s)
    }

    #[test]
    fn round_trip_values() {
        let all_chars: String = (0u32..0x3000).filter_map(char::from_u32).collect();
        let v = jobj! {
            "null" => Json::Null,
            "t" => true,
            "f" => false,
            "ints" => jarr![0, -1, 1, i32::MIN, i32::MAX, i64::MIN, i64::MAX, u32::MAX],
            "floats" => jarr![0.5, -0.25, 1.0, -0.0, 1e-7, 1e300, f64::MAX, f64::MIN_POSITIVE, 0.1 + 0.2],
            "s" => "plain",
            "esc" => "q\" b\\ s/ n\n r\r t\t b\u{8} f\u{c} nul\u{0} us\u{1f} del\u{7f} ls\u{2028} ps\u{2029}",
            "unicode" => "Ærë ünïcødé \u{1F409} \u{FFFF} \u{10FFFF}",
            "all" => all_chars,
            "nested" => jobj! { "a" => jarr![jarr![], jobj! {}, jarr![jobj! { "x" => 1 }]], "empty" => "" },
            "" => "empty key",
        };
        rt(&v);
        for x in [Json::Null, Json::Bool(true), Json::Int(7), Json::Float(2.5), Json::from("x"), Json::arr(), Json::obj()] {
            rt(&x);
        }
    }

    #[test]
    fn writer_escaping() {
        assert_eq!(quote("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(quote("\n\r\t\u{8}\u{c}"), r#""\n\r\t\b\f""#);
        assert_eq!(quote("\u{0}\u{1}\u{1f}"), r#""\u0000\u0001\u001f""#);
        assert_eq!(quote("\u{2028}\u{2029}"), "\"\\u2028\\u2029\"");
        // Not escaped: '/', DEL, non-ASCII.
        assert_eq!(quote("/\u{7f}é\u{1F409}"), "\"/\u{7f}é\u{1F409}\"");
        assert_eq!(Json::from("ok").to_string(), "\"ok\"");
    }

    #[test]
    fn writer_numbers_and_layout() {
        assert_eq!(Json::Float(1.0).to_string(), "1.0");
        assert_eq!(Json::Float(0.1).to_string(), "0.1");
        assert_eq!(Json::Float(1e-7).to_string(), "1e-7");
        assert_eq!(Json::Float(f64::NAN).to_string(), "null");
        assert_eq!(Json::Float(f64::INFINITY).to_string(), "null");
        assert_eq!(Json::Int(-42).to_string(), "-42");
        let v = jobj! { "id" => 12, "ok" => true, "r" => jobj! { "applied" => true, "accepted" => Json::Null }, "log" => jarr![] };
        assert_eq!(v.to_string(), r#"{"id":12,"ok":true,"r":{"applied":true,"accepted":null},"log":[]}"#);
        assert_eq!(
            v.to_pretty(),
            "{\n  \"id\": 12,\n  \"ok\": true,\n  \"r\": {\n    \"applied\": true,\n    \"accepted\": null\n  },\n  \"log\": []\n}"
        );
        assert_eq!(jarr![1, 2, 3].to_pretty(), "[1, 2, 3]");
        assert_eq!(jarr![jarr![1], 2].to_pretty(), "[\n  [1],\n  2\n]");
    }

    #[test]
    fn insertion_order_is_kept() {
        let mut v = Json::obj();
        v.set("z", 1).set("a", 2).set("m", 3);
        v.set("a", 20); // replaced in place
        assert_eq!(v.to_string(), r#"{"z":1,"a":20,"m":3}"#);
        let parsed = Json::parse(r#"{"z":1,"a":20,"m":3}"#).unwrap();
        assert_eq!(parsed, v);
        assert_eq!(parsed.to_string(), v.to_string());
        let mut a = Json::arr();
        a.push(1);
        a.push("x");
        assert_eq!(a, jarr![1, "x"]);
        assert_eq!(Json::obj().with("k", Some(3)).with("n", None::<i32>).to_string(), r#"{"k":3,"n":null}"#);
    }

    #[test]
    fn parse_numbers() {
        assert_eq!(Json::parse("0").unwrap(), Json::Int(0));
        assert_eq!(Json::parse("-0").unwrap(), Json::Int(0));
        assert_eq!(Json::parse("-12").unwrap(), Json::Int(-12));
        assert_eq!(Json::parse("9223372036854775807").unwrap(), Json::Int(i64::MAX));
        assert_eq!(Json::parse("-9223372036854775808").unwrap(), Json::Int(i64::MIN));
        // Overflowing integers become floats.
        assert_eq!(Json::parse("9223372036854775808").unwrap(), Json::Float(9223372036854775808.0));
        assert_eq!(Json::parse("1.5").unwrap(), Json::Float(1.5));
        assert_eq!(Json::parse("1e2").unwrap(), Json::Float(100.0));
        assert_eq!(Json::parse("1E+2").unwrap(), Json::Float(100.0));
        assert_eq!(Json::parse("-2.5e-3").unwrap(), Json::Float(-0.0025));
        assert_eq!(Json::parse("1e2").unwrap().as_i64(), Some(100));
        assert_eq!(Json::parse("1.5").unwrap().as_i64(), None);
        for bad in ["01", "-", "-a", "1.", ".5", "1e", "1e+", "+1", "0x10", "1e400", "-1e400", "NaN", "Infinity"] {
            err(bad);
        }
    }

    #[test]
    fn parse_strings() {
        let p = |s: &str| Json::parse(s).unwrap();
        assert_eq!(p(r#""\"\\\/\b\f\n\r\t""#), Json::from("\"\\/\u{8}\u{c}\n\r\t"));
        assert_eq!(p(r#""\u0041\u00e9\u2028""#), Json::from("A\u{e9}\u{2028}"));
        assert_eq!(p(r#""\ud83d\ude00""#), Json::from("\u{1F600}"));
        assert_eq!(p(r#""\uD83D\uDE00""#), Json::from("\u{1F600}"));
        // Lone surrogates decode to U+FFFD; a following escape still decodes.
        assert_eq!(p(r#""\ud800""#), Json::from("\u{FFFD}"));
        assert_eq!(p(r#""\udc00x""#), Json::from("\u{FFFD}x"));
        assert_eq!(p(r#""\ud800\u0041""#), Json::from("\u{FFFD}A"));
        assert_eq!(p("\"raw é \u{1F409}\""), Json::from("raw é \u{1F409}"));
        err("\"abc");
        err("\"a\nb\"");
        err("\"tab\there\"");
        err(r#""\x""#);
        err(r#""\u12""#);
        err(r#""\u12g4""#);
        err("'single'");
        assert_eq!(Json::parse_bytes(b"\"\xff\"").unwrap_err().msg, "invalid UTF-8");
        assert_eq!(Json::parse_bytes(b"[1]").unwrap(), jarr![1]);
    }

    #[test]
    fn parse_structure_errors() {
        assert_eq!(Json::parse(" \t\r\n[ 1 , {\"a\" : null } ]\n").unwrap(), jarr![1, jobj! { "a" => Json::Null }]);
        for bad in [
            "", " ", "[", "]", "[1,]", "[,1]", "[1 2]", "{", "{\"a\"}", "{\"a\":}", "{\"a\":1,}", "{a:1}", "{1:1}",
            "{\"a\" 1}", "[1]x", "[1] [2]", "tru", "nul", "true false", "/* c */ 1", "\u{feff}1",
        ] {
            err(bad);
        }
        let e = err("[1,]");
        assert_eq!(e.pos, 3);
        assert_eq!(e.to_string(), "invalid JSON at byte 3: expected a value");
        // Duplicate keys, small and large objects.
        assert_eq!(err(r#"{"a":1,"a":2}"#).msg, "duplicate key");
        let mut big: Vec<String> = (0..40).map(|i| format!("\"k{i}\":{i}")).collect();
        let ok = format!("{{{}}}", big.join(","));
        assert_eq!(Json::parse(&ok).unwrap().as_obj().unwrap().len(), 40);
        big.push("\"k3\":0".into());
        assert_eq!(err(&format!("{{{}}}", big.join(","))).msg, "duplicate key");
        // Depth limit.
        let deep = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        assert!(Json::parse(&deep(MAX_DEPTH)).is_ok());
        assert_eq!(err(&deep(MAX_DEPTH + 1)).msg, "nesting too deep");
        assert_eq!(err(&"[".repeat(100_000)).msg, "nesting too deep");
        let deep_obj = format!("{}1{}", "{\"a\":".repeat(MAX_DEPTH + 1), "}".repeat(MAX_DEPTH + 1));
        assert_eq!(err(&deep_obj).msg, "nesting too deep");
    }

    #[test]
    fn typed_getters() {
        let v = Json::parse(
            r#"{"id":12,"c":"move","units":[881,890],"x":-3,"big":4294967296,"f":2.5,"n":null,"append":false,"o":{"k":1}}"#,
        )
        .unwrap();
        assert_eq!(v.req::<i64>("id"), Ok(12));
        assert_eq!(v.req::<u8>("id"), Ok(12));
        assert_eq!(v.req_str("c"), Ok("move"));
        assert_eq!(v.req::<String>("c"), Ok("move".to_string()));
        assert_eq!(v.req::<Vec<u32>>("units"), Ok(vec![881, 890]));
        assert_eq!(v.req::<Vec<UnitId>>("units"), Ok(vec![UnitId(881), UnitId(890)]));
        assert_eq!(v.req::<i32>("x"), Ok(-3));
        assert_eq!(v.req::<bool>("append"), Ok(false));
        assert_eq!(v.get_or("missing", true), Ok(true));
        assert_eq!(v.get_or("n", 7i32), Ok(7));
        assert_eq!(v.opt::<i32>("missing"), Ok(None));
        assert_eq!(v.opt::<i32>("n"), Ok(None));
        assert_eq!(v.opt::<i32>("x"), Ok(Some(-3)));
        assert_eq!(v.opt_str("c"), Ok(Some("move")));
        assert_eq!(v.req_arr("units").map(<[Json]>::len), Ok(2));
        assert_eq!(v.req_obj("o").map(|o| o.get("k").cloned()), Ok(Some(Json::Int(1))));
        assert_eq!(v.req_value("n"), Ok(&Json::Null));
        assert_eq!(v.opt_value("n"), Ok(None));

        // Errors name the field and say what was expected.
        assert_eq!(v.req::<i32>("missing").unwrap_err().to_string(), "field `missing`: missing");
        assert_eq!(
            v.req::<u32>("x").unwrap_err().to_string(),
            "field `x`: expected an integer in 0..=4294967295, got -3"
        );
        assert_eq!(
            v.req::<u32>("big").unwrap_err().to_string(),
            "field `big`: expected an integer in 0..=4294967295, got 4294967296"
        );
        assert_eq!(v.req::<i32>("f").unwrap_err().msg, "expected an integer in -2147483648..=2147483647, got 2.5");
        assert_eq!(v.req::<i32>("n").unwrap_err().msg, "expected an integer in -2147483648..=2147483647, got null");
        assert_eq!(v.req::<bool>("c").unwrap_err().msg, "expected a boolean, got \"move\"");
        assert_eq!(v.opt::<bool>("c").unwrap_err().key, "c");
        assert_eq!(v.req_str("id").unwrap_err().msg, "expected a string, got 12");
        assert_eq!(v.req_arr("c").unwrap_err().msg, "expected an array, got \"move\"");
        assert_eq!(v.req_obj("units").unwrap_err().msg, "expected an object, got [881,890]");
        assert_eq!(
            v.req::<Vec<u8>>("units").unwrap_err().msg,
            "expected an array of (an integer in 0..=255), got [881,890]"
        );
        assert_eq!(
            v.req::<Vec<UnitId>>("c").unwrap_err().msg,
            "expected an array of (a unit id (an integer in 0..=4294967295)), got \"move\""
        );
        assert_eq!(jarr![1].req::<i32>("k").unwrap_err().msg, "expected an object, got an array");
        let long = Json::from("x".repeat(100));
        assert_eq!(snippet(&long, 10), "\"xxxxxxxxx...");
        assert_eq!(snippet(&Json::from("ééééé"), 4), "\"é...");
    }

    #[test]
    fn accessors_and_conversions() {
        assert_eq!(Json::from(5usize), Json::Int(5));
        assert_eq!(Json::from(i64::MAX as usize), Json::Int(i64::MAX));
        assert_eq!(Json::from(vec![1u8, 2]), jarr![1, 2]);
        assert_eq!(Json::from(&[UnitId(4), UnitId(5)][..]), jarr![4, 5]);
        assert_eq!(Json::from(PlayerId(2)), Json::Int(2));
        assert_eq!(Json::idx(ffh_data::YieldType::YIELD_COMMERCE), Json::Int(2));
        assert_eq!(Json::opt_idx(None::<ffh_data::YieldType>), Json::Null);
        assert_eq!(Json::Float(1e20).as_i64(), None);
        assert_eq!(Json::Float(-(i64::MIN as f64)).as_i64(), None);
        assert_eq!(Json::Float(i64::MIN as f64).as_i64(), Some(i64::MIN));
        assert_eq!(Json::Float(-3.0).as_i64(), Some(-3));
        assert_eq!(Json::Int(3).as_f64(), Some(3.0));
        assert_eq!(Json::from("s").as_i64(), None);
        let v = jobj! { "a" => jarr![10, 20] };
        assert_eq!(v.get("a").and_then(|a| a.at(1)), Some(&Json::Int(20)));
        assert_eq!(v.get("b"), None);
        assert_eq!(jarr![1].get("a"), None);
        assert_eq!("[true]".parse::<Json>(), Ok(jarr![true]));
        let mut m = v.clone();
        *m.get_mut("a").unwrap() = Json::Null;
        assert_eq!(m.to_string(), r#"{"a":null}"#);
    }

    /// `usize` round-trips over its whole JSON range, and the error message states that range
    /// (integers above `i64::MAX` parse as lossy floats, so they are refused).
    #[test]
    fn usize_range() {
        for n in [0usize, 1, 312, u32::MAX as usize, i64::MAX as usize] {
            let text = jobj! { "n" => n }.to_string();
            assert_eq!(Json::parse(&text).unwrap().req::<usize>("n"), Ok(n), "{text}");
        }
        let expected = "expected an integer in 0..=9223372036854775807";
        for (lit, shown) in [
            ("9223372036854775808", "9.223372036854776e18"),
            ("18446744073709551615", "1.8446744073709552e19"),
            ("-1", "-1"),
            ("1.5", "1.5"),
        ] {
            let v = Json::parse(&format!(r#"{{"n":{lit}}}"#)).unwrap();
            assert_eq!(v.req::<usize>("n").unwrap_err().msg, format!("{expected}, got {shown}"), "{lit}");
        }
        assert_eq!(Json::parse("9223372036854775807").unwrap(), Json::Int(i64::MAX));
    }

    #[test]
    #[should_panic(expected = "usize above i64::MAX")]
    fn usize_above_i64_max_panics() {
        let _ = Json::from(usize::MAX);
    }
}
