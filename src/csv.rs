//! CSV output helpers.

/// Quotes a CSV text field and defuses spreadsheet formulas.
///
/// File and target names come from files that are not trusted. A spreadsheet program treats a
/// cell that starts with `=`, `+`, `-` or `@` as a formula (CSV injection), so such a field is
/// prefixed with an apostrophe. Fields with a comma, a quote or a line break are quoted.
/// Use this for text only; numbers must be written unchanged.
#[must_use]
pub fn text_field(s: &str) -> String {
    let defused = match s.chars().next() {
        Some('=' | '+' | '-' | '@' | '\t' | '\r') => format!("'{}", s),
        _ => s.to_string(),
    };
    if defused.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", defused.replace('"', "\"\""))
    } else {
        defused
    }
}

/// Two-column CSV text with a header line.
#[must_use]
pub fn two_columns(h1: &str, h2: &str, pts: impl Iterator<Item = (f64, f64)>) -> String {
    let mut s = format!("{},{}\n", h1, h2);
    for (x, y) in pts {
        s += &format!("{},{}\n", x, y);
    }
    s
}
