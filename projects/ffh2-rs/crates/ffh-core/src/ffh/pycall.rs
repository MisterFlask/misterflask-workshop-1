//! Parsing of the Python call expressions FFH2 stores in XML
//! (`<PyResult>spellSummon(pCaster,'UNIT_X')</PyResult>`), so each expression is
//! resolved once into a function name + literal arguments instead of `eval`-ed
//! on every call as in the original.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PyArg {
    /// A context variable such as `pCaster`, `pPlot`, `pOpponent`.
    Var(String),
    /// A quoted string literal, usually a type name (`'UNIT_IMP'`).
    Str(String),
    /// An integer literal.
    Int(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyCall {
    pub name: String,
    pub args: Vec<PyArg>,
}

impl PyCall {
    /// Parses `name(arg, arg, ...)` or a bare `name`. Returns None for empty input.
    pub fn parse(src: &str) -> Option<PyCall> {
        let s = src.trim();
        if s.is_empty() {
            return None;
        }
        let (name, rest) = match s.find('(') {
            Some(i) => (&s[..i], Some(&s[i + 1..])),
            None => (s, None),
        };
        let mut args = Vec::new();
        if let Some(rest) = rest {
            let inner = rest.trim_end().strip_suffix(')').unwrap_or(rest);
            for raw in split_args(inner) {
                let a = raw.trim();
                if a.is_empty() {
                    continue;
                }
                let arg = if (a.starts_with('\'') && a.ends_with('\'')) || (a.starts_with('"') && a.ends_with('"')) {
                    PyArg::Str(a[1..a.len() - 1].to_string())
                } else if let Ok(i) = a.parse::<i32>() {
                    PyArg::Int(i)
                } else {
                    PyArg::Var(a.to_string())
                };
                args.push(arg);
            }
        }
        Some(PyCall { name: name.trim().to_string(), args })
    }

    /// The i-th literal string argument.
    pub fn str_arg(&self, i: usize) -> Option<&str> {
        match self.args.get(i) {
            Some(PyArg::Str(s)) => Some(s),
            _ => None,
        }
    }

    /// The i-th literal integer argument.
    pub fn int_arg(&self, i: usize) -> Option<i32> {
        match self.args.get(i) {
            Some(PyArg::Int(v)) => Some(*v),
            _ => None,
        }
    }
}

fn split_args(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), ch) if ch == q => quote = None,
            (Some(_), _) => {}
            (None, '\'') | (None, '"') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, ',') if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shapes() {
        assert_eq!(PyCall::parse(""), None);
        let c = PyCall::parse("spellBless(pCaster)").unwrap();
        assert_eq!(c.name, "spellBless");
        assert_eq!(c.args, vec![PyArg::Var("pCaster".into())]);
        let c = PyCall::parse("reqEquipment(pCaster,'EQUIPMENT_DRAGONS_HORDE')").unwrap();
        assert_eq!(c.str_arg(1), Some("EQUIPMENT_DRAGONS_HORDE"));
        let c = PyCall::parse("spellX(pCaster,20)").unwrap();
        assert_eq!(c.int_arg(1), Some(20));
        let c = PyCall::parse("canTriggerPlotEmpty").unwrap();
        assert!(c.args.is_empty());
    }
}
