//! Editing, loading and saving of the target table.

use crate::app::state::App;
use crate::domain::Analyte;
use crate::formats::csv;
use std::path::Path;

/// Parses one row of the target editor; returns `None` if a field is invalid.
pub(crate) fn parse_row(r: &[String; 4]) -> Option<Analyte> {
    let name = r[0].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let na = |t: &str| {
        t.is_empty() || t == "-" || t.eq_ignore_ascii_case("n/a") || t.eq_ignore_ascii_case("na")
    };
    let t = r[1].trim();
    let rt = if na(t) {
        None
    } else {
        Some(t.parse::<f64>().ok()?)
    };
    let t = r[2].trim();
    let precursor = if na(t) {
        None
    } else {
        Some(t.parse::<f64>().ok()?)
    };
    let mut ions = vec![];
    for tok in r[3]
        .split(|c: char| c == ';' || c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        ions.push(tok.parse::<f64>().ok()?);
    }
    if ions.is_empty() {
        return None;
    }
    Some(Analyte {
        name,
        rt,
        ions,
        precursor,
    })
}

impl App {
    // ---------------- target table editing ----------------
    /// Rebuilds the text rows of the target editor from the targets.
    pub fn sync_rows(&mut self) {
        self.rows = self
            .analytes
            .iter()
            .map(|a| {
                [
                    a.name.clone(),
                    a.rt.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
                    a.precursor
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "n/a".into()),
                    a.ions
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join("; "),
                ]
            })
            .collect();
        self.rows_dirty = false;
    }

    /// Applies an edited row of the target editor to its target and re-integrates it.
    pub fn apply_row(&mut self, i: usize) {
        if i >= self.rows.len() || i >= self.analytes.len() {
            return;
        }
        if let Some(an) = parse_row(&self.rows[i]) {
            if an != self.analytes[i] {
                self.push_undo("edit target");
                self.audit.log(
                    "TARGET_EDIT",
                    &format!(
                        "#{} {} -> {} rt={:?} ions={:?}",
                        i + 1,
                        self.analytes[i].name,
                        an.name,
                        an.rt,
                        an.ions
                    ),
                );
                self.analytes[i] = an;
                self.recompute_analyte(i);
            }
        }
    }

    /// Appends a placeholder target.
    pub fn add_target(&mut self) {
        self.push_undo("add target");
        let n = self.analytes.len();
        self.analytes.push(Analyte {
            name: format!("Target {}", n + 1),
            rt: None,
            ions: vec![100.0],
            precursor: None,
        });
        self.fits.push(None);
        self.recompute_analyte(n);
        self.rows_dirty = true;
        self.audit.log("TARGET_ADDED", &format!("#{}", n + 1));
    }

    /// Deletes a target and its results from all files.
    pub fn delete_target(&mut self, i: usize) {
        if i >= self.analytes.len() {
            return;
        }
        self.push_undo("delete target");
        let name = self.analytes.remove(i).name;
        for sm in self.samples.iter_mut() {
            if i < sm.xics.len() {
                sm.xics.remove(i);
            }
            if i < sm.cells.len() {
                sm.cells.remove(i);
            }
        }
        if i < self.fits.len() {
            self.fits.remove(i);
        }
        self.sel_a = self.sel_a.min(self.analytes.len().saturating_sub(1));
        self.rows_dirty = true;
        self.audit.log("TARGET_DELETED", &name);
    }

    /// The target table as CSV text.
    pub fn targets_csv(&self) -> String {
        let mut o = String::from("name,rt_min,ions\n");
        for a in &self.analytes {
            let mut ions = a
                .ions
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(";");
            if let Some(p) = a.precursor {
                ions += &format!(";p:{}", p);
            }
            o += &format!(
                "{},{},{}\n",
                csv::text_field(&a.name),
                a.rt.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
                ions
            );
        }
        o
    }

    /// Writes the target table to `path` and remembers the path.
    pub(crate) fn write_targets(&mut self, path: &Path) {
        let t = self.targets_csv();
        match std::fs::write(path, &t) {
            Ok(_) => {
                self.audit.log(
                    "TARGETS_SAVED",
                    &format!(
                        "{} sha256={}",
                        path.display(),
                        crate::support::sha256::sha256_hex(t.as_bytes())
                    ),
                );
                self.table_path = Some(path.to_path_buf());
                self.status = format!("Saved target table {}", path.display());
            }
            Err(e) => self.status = format!("Save failed: {}", e),
        }
    }

    /// Saves the target table to a path chosen in a file dialog.
    pub(crate) fn save_targets_as(&mut self) {
        let mut d = self
            .dialog()
            .set_title("Save target table")
            .add_filter("CSV", &["csv"])
            .set_file_name("targets.csv");
        if let Some(p) = &self.table_path {
            if let Some(dir) = p.parent() {
                d = d.set_directory(dir);
            }
            if let Some(n) = p.file_name() {
                d = d.set_file_name(n.to_string_lossy().to_string());
            }
        }
        if let Some(path) = d.save_file() {
            self.write_targets(&path);
        }
    }

    /// Loads a target table chosen in a file dialog.
    pub(crate) fn open_targets_dialog(&mut self) {
        if let Some(p) = self
            .dialog()
            .set_title("Open target table")
            .add_filter("Target table", &["csv", "tsv", "txt"])
            .pick_file()
        {
            self.load_table(&p);
        }
    }
}
