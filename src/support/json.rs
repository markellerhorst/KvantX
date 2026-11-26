//! A strict JSON reader and writer (RFC 8259) with limits, for project files and manifests.
//!
//! Project files are not trusted, so the reader is strict: no comments, no trailing commas, no
//! `NaN` or `Infinity`, no duplicate object keys, no control characters inside strings, no lone
//! surrogates, and a limit on the nesting depth. Numbers that do not fit a finite `f64` are
//! rejected. The caller limits the size of the text.
//!
//! The writer refuses non-finite numbers instead of writing something else: a value that is not
//! a number must never appear as a plausible one.

/// Deepest nesting of arrays and objects the reader accepts.
pub const MAX_DEPTH: usize = 32;

/// Integers up to this value are exactly representable as `f64` (2^53).
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// A JSON value. Objects keep the order of their keys.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A finite number.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object; keys are unique.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The value of `key` if this is an object that has it.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The text if this is a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The number if this is a number.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The number if this is a non-negative integer that `f64` represents exactly.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n) if n.fract() == 0.0 && *n >= 0.0 && *n <= MAX_EXACT_INTEGER => {
                Some(*n as u64)
            }
            _ => None,
        }
    }

    /// The flag if this is a boolean.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The elements if this is an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The key/value pairs if this is an object.
    #[must_use]
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(pairs) => Some(pairs),
            _ => None,
        }
    }

    /// Serializes with two-space indentation.
    ///
    /// # Errors
    ///
    /// Returns a message if a number is not finite.
    pub fn to_pretty_string(&self) -> Result<String, String> {
        let mut out = String::new();
        self.write(&mut out, 0)?;
        out.push('\n');
        Ok(out)
    }

    fn write(&self, out: &mut String, level: usize) -> Result<(), String> {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&number_text(*n)?),
            Value::String(s) => write_string(out, s),
            Value::Array(items) if items.is_empty() => out.push_str("[]"),
            Value::Array(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    indent(out, level + 1);
                    item.write(out, level + 1)?;
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                indent(out, level);
                out.push(']');
            }
            Value::Object(pairs) if pairs.is_empty() => out.push_str("{}"),
            Value::Object(pairs) => {
                out.push_str("{\n");
                for (i, (key, value)) in pairs.iter().enumerate() {
                    indent(out, level + 1);
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, level + 1)?;
                    out.push_str(if i + 1 < pairs.len() { ",\n" } else { "\n" });
                }
                indent(out, level);
                out.push('}');
            }
        }
        Ok(())
    }
}

/// Builds an object from `(key, value)` pairs.
#[must_use]
pub fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// A string value.
#[must_use]
pub fn string(text: &str) -> Value {
    Value::String(text.to_string())
}

/// A number value or `null` for `None`.
#[must_use]
pub fn optional_number(value: Option<f64>) -> Value {
    value.map_or(Value::Null, Value::Number)
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

/// Text of a number that reads back as exactly the same `f64`.
fn number_text(n: f64) -> Result<String, String> {
    if !n.is_finite() {
        return Err(format!("the number {n} cannot be written to JSON"));
    }
    if n.fract() == 0.0 && n.abs() < 1e15 {
        return Ok(format!("{}", n as i64));
    }
    if n != 0.0 && (n.abs() < 1e-6 || n.abs() >= 1e16) {
        return Ok(format!("{n:e}"));
    }
    Ok(format!("{n}"))
}

fn write_string(out: &mut String, s: &str) {
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

/// Parses `text` as one JSON value.
///
/// # Errors
///
/// Returns a message with the byte offset if the text is not valid JSON, is nested deeper than
/// [`MAX_DEPTH`], contains duplicate keys or a number outside the range of `f64`.
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_space();
    let value = p.value(0)?;
    p.skip_space();
    if p.pos != p.bytes.len() {
        return Err(p.error("unexpected text after the value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        format!("invalid JSON at byte {}: {}", self.pos, what)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect_word(&mut self, word: &str, value: Value) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("unknown literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        match self.peek() {
            None => Err(self.error("unexpected end of the text")),
            Some(b'n') => self.expect_word("null", Value::Null),
            Some(b't') => self.expect_word("true", Value::Bool(true)),
            Some(b'f') => self.expect_word("false", Value::Bool(false)),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("unexpected character")),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_space();
            items.push(self.value(depth + 1)?);
            self.skip_space();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.pos += 1;
        let mut pairs: Vec<(String, Value)> = Vec::new();
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_space();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a key"));
            }
            let key = self.string()?;
            if pairs.iter().any(|(k, _)| *k == key) {
                return Err(self.error(&format!("duplicate key \"{key}\"")));
            }
            self.skip_space();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':'"));
            }
            self.pos += 1;
            self.skip_space();
            let value = self.value(depth + 1)?;
            pairs.push((key, value));
            self.skip_space();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(pairs));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.error("a digit is expected")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("a digit is expected after the decimal point"));
            }
            self.digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("a digit is expected in the exponent"));
            }
            self.digits();
        }
        // the grammar above only lets ASCII through, so the slice is valid UTF-8
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.error("bad number"))?;
        match text.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(Value::Number(n)),
            _ => Err(self.error("number out of range")),
        }
    }

    fn digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.error("short \\u escape"))?;
        let mut value = 0u32;
        for &d in digits {
            let digit = (d as char)
                .to_digit(16)
                .ok_or_else(|| self.error("bad \\u escape"))?;
            value = value * 16 + digit;
        }
        self.pos += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1; // opening quote
        let mut out = String::new();
        loop {
            let start = self.pos;
            // copy the run of ordinary bytes; the text is valid UTF-8 and only ASCII bytes end a run
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.pos])
                    .map_err(|_| self.error("bad UTF-8"))?,
            );
            match self.peek() {
                None => return Err(self.error("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self
                        .peek()
                        .ok_or_else(|| self.error("unterminated escape"))?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                if !self.bytes[self.pos..].starts_with(b"\\u") {
                                    return Err(self.error("lone high surrogate"));
                                }
                                self.pos += 2;
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(self.error("bad surrogate pair"));
                                }
                                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else if (0xDC00..0xE000).contains(&first) {
                                return Err(self.error("lone low surrogate"));
                            } else {
                                first
                            };
                            out.push(
                                char::from_u32(code).ok_or_else(|| self.error("bad code point"))?,
                            );
                        }
                        _ => return Err(self.error("unknown escape")),
                    }
                }
                Some(_) => return Err(self.error("control character in a string")),
            }
        }
    }
}
