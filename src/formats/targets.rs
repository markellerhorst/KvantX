//! The target table: a CSV or TSV file with one compound per row.

use crate::domain::Analyte;

/// CSV/TSV: name, RT (min or n/a), ions (extra columns, or separated by ; | space). First ion = quantifier.
pub fn parse_table(text: &str) -> Result<Vec<Analyte>, String> {
    let mut out: Vec<Analyte> = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = if line.contains('\t') {
            line.split('\t').collect()
        } else {
            line.split(',').collect()
        };
        if f.len() < 3 {
            if out.is_empty() {
                continue;
            }
            return Err(format!("line {}: need name, RT, ions", ln + 1));
        }
        let name = f[0].trim().trim_matches('"').to_string();
        let rt_s = f[1].trim().trim_matches('"');
        let rt = rt_s.parse::<f64>().ok();
        // product ions are plain numbers; an optional MS1 precursor is written "p:286.1438"
        let mut ions: Vec<f64> = Vec::new();
        let mut precursor: Option<f64> = None;
        for t in f[2..]
            .iter()
            .flat_map(|s| s.split(|c: char| c.is_whitespace() || c == ';' || c == '|'))
        {
            let t = t.trim_matches('"');
            if let Some(r) = t.strip_prefix("p:").or_else(|| t.strip_prefix("P:")) {
                precursor = r.parse::<f64>().ok();
            } else if let Ok(v) = t.parse::<f64>() {
                ions.push(v);
            }
        }
        let rt_na = rt_s.is_empty()
            || rt_s == "-"
            || rt_s.eq_ignore_ascii_case("n/a")
            || rt_s.eq_ignore_ascii_case("na");
        if ions.is_empty() || (rt.is_none() && !rt_na) {
            if out.is_empty() {
                continue;
            } // header row
            return Err(format!("line {}: cannot parse RT/ions", ln + 1));
        }
        out.push(Analyte {
            name,
            rt,
            ions,
            precursor,
        });
    }
    if out.is_empty() {
        Err("no analytes found".into())
    } else {
        Ok(out)
    }
}
