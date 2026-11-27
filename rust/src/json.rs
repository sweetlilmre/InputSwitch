//! A minimal JSON reader/writer, enough for the settings file. Keeps object key order.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Object member by name. Matched ignoring case, to be forgiving of hand edits (both apps
    /// write the same PascalCase names).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Json::Str(s)) => Some(s),
            _ => None,
        }
    }
    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(Json::Bool(b)) => Some(*b),
            _ => None,
        }
    }
    pub fn arr(&self, key: &str) -> &[Json] {
        match self.get(key) {
            Some(Json::Arr(a)) => a,
            _ => &[],
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------------------------------ reading

pub fn parse(text: &str) -> Result<Json, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut p = Parser { b: text.as_bytes(), i: 0, depth: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.err("unexpected text after the end"));
    }
    Ok(v)
}

/// How deeply arrays and objects may nest (System.Text.Json's default), so a damaged file can't
/// overflow the stack.
const MAX_DEPTH: usize = 64;

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> String {
        format!("{what} at byte {}", self.i)
    }

    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("expected '{}'", c as char)))
        }
    }

    fn literal(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.b[self.i..].starts_with(word.as_bytes()) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        if matches!(self.b.get(self.i), Some(b'{' | b'[')) {
            if self.depth == MAX_DEPTH {
                return Err(self.err("nested too deeply"));
            }
            self.depth += 1;
            let v = self.container();
            self.depth -= 1;
            return v;
        }
        self.scalar()
    }

    /// An object or an array.
    fn container(&mut self) -> Result<Json, String> {
        match self.b.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut members = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Obj(members));
                }
                loop {
                    self.ws();
                    let key = self.string()?;
                    self.eat(b':')?;
                    members.push((key, self.value()?));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(members));
                        }
                        _ => return Err(self.err("expected ',' or '}'")),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(items));
                        }
                        _ => return Err(self.err("expected ',' or ']'")),
                    }
                }
            }
            _ => unreachable!("container() is only called at '{{' or '['"),
        }
    }

    fn scalar(&mut self) -> Result<Json, String> {
        match self.b.get(self.i) {
            None => Err(self.err("unexpected end")),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(_) => {
                let start = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[start..self.i])
                    .ok()
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(Json::Num)
                    .ok_or_else(|| self.err("invalid number"))
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let s = self.b.get(self.i..self.i + 4).ok_or_else(|| self.err("bad escape"))?;
        let v = u32::from_str_radix(std::str::from_utf8(s).map_err(|_| self.err("bad escape"))?, 16)
            .map_err(|_| self.err("bad escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        if self.b.get(self.i) != Some(&b'"') {
            return Err(self.err("expected a string"));
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or_else(|| self.err("unterminated string"))?;
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or_else(|| self.err("bad escape"))?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut cp = self.hex4()?;
                            // A high surrogate pairs with a following low one; anything else is
                            // left alone (and the lone surrogate becomes U+FFFD).
                            if (0xD800..0xDC00).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
                                let after_high = self.i;
                                self.i += 2;
                                let lo = self.hex4()?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                } else {
                                    self.i = after_high;
                                }
                            }
                            char::from_u32(cp).unwrap_or('\u{fffd}')
                        }
                        _ => return Err(self.err("bad escape")),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("invalid UTF-8"))
    }
}

// ------------------------------------------------------------------------------------------ writing

/// Indented JSON (two spaces), like System.Text.Json's WriteIndented.
pub fn write(v: &Json) -> String {
    let mut out = String::new();
    write_value(v, 0, &mut out);
    out
}

fn indent(n: usize, out: &mut String) {
    out.push('\n');
    for _ in 0..n {
        out.push_str("  ");
    }
}

fn write_value(v: &Json, depth: usize, out: &mut String) {
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                out.push_str(&(*n as i64).to_string());
            } else {
                out.push_str(&n.to_string());
            }
        }
        Json::Str(s) => write_string(s, out),
        Json::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(depth + 1, out);
                write_value(item, depth + 1, out);
            }
            indent(depth, out);
            out.push(']');
        }
        Json::Obj(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(depth + 1, out);
                write_string(k, out);
                out.push_str(": ");
                write_value(val, depth + 1, out);
            }
            indent(depth, out);
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let text = r#"{"a": [1, 2.5, -3], "b": {"c": "x\"y\\z\n", "d": true, "e": null}, "f": []}"#;
        let v = parse(text).unwrap();
        assert_eq!(parse(&write(&v)).unwrap(), v);
        assert_eq!(v.get("b").unwrap().str("c"), Some("x\"y\\z\n"));
        assert_eq!(v.arr("a").len(), 3);
    }

    #[test]
    fn unicode_escapes_and_bom() {
        let v = parse("\u{feff}{\"s\": \"\\u00e9 \\ud83d\\ude00 \u{21c4}\"}").unwrap();
        assert_eq!(v.str("s"), Some("é 😀 ⇄"));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("{").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("{\"a\": 1} x").is_err());
    }

    #[test]
    fn a_lone_high_surrogate_keeps_what_follows() {
        assert_eq!(parse(r#""\ud800A""#).unwrap(), Json::Str("\u{fffd}A".into()));
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        assert!(parse(&"[".repeat(10_000)).is_err());
        assert!(parse(&format!("{}{}", "[".repeat(64), "]".repeat(64))).is_ok());
    }
}
