//! Append-only, hash-chained audit trails.
//!
//! Every entry is a line `time|user|action|detail|hash`. The hash covers the hash of the previous
//! line and the text of the line, so a later change of any line is detected by [`verify`].
//!
//! There is one trail per project file. Work that has not been saved to a project yet is recorded
//! in the trail of the current session; saving the project moves these entries into the trail of
//! the project (see [`Audit::move_to`]).
//!
//! The hash chain detects accidental damage and simple edits. It is not a signature: somebody
//! who rewrites the whole file, including all hashes, cannot be detected.

use crate::support::sha256;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC time as ISO 8601 text, for example `2026-01-31T12:00:00Z`.
#[must_use]
pub fn iso_now() -> String {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (days, rem) = (s.div_euclid(86400), s.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Makes text safe for one field of an entry: the field separator and control characters
/// (line breaks) would allow a value to forge further entries.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '|' {
                '/'
            } else if c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// One audit trail.
pub struct Audit {
    /// File the entries are appended to.
    pub path: String,
    /// User name written into every entry.
    pub user: String,
    /// Hash of the last entry (`GENESIS` for an empty trail).
    last: String,
    /// All entries of the trail as written to the file, including their hash.
    pub recent: Vec<String>,
}

impl Audit {
    /// Opens the trail in `path`, continuing its hash chain; the file is created with the first
    /// entry.
    pub fn new(path: &str) -> Audit {
        let mut recent = Vec::new();
        let mut last = "GENESIS".to_string();
        if let Ok(s) = std::fs::read_to_string(path) {
            for l in s.lines() {
                if let Some(h) = l.rsplit('|').next() {
                    last = h.to_string();
                }
                recent.push(l.to_string());
            }
        }
        Audit {
            path: path.into(),
            user: "N/A".to_string(),
            last,
            recent,
        }
    }

    /// Appends an entry. The characters `|` and line breaks are replaced in every field.
    pub fn log(&mut self, action: &str, detail: &str) {
        let body = format!(
            "{}|{}|{}|{}",
            iso_now(),
            clean(&self.user),
            clean(action),
            clean(detail)
        );
        self.append_body(&body);
    }

    /// Appends an entry given as `time|user|action|detail` and chains it to the last entry.
    fn append_body(&mut self, body: &str) {
        let hash = sha256::sha256_hex(format!("{}|{}", self.last, body).as_bytes());
        let line = format!("{}|{}", body, hash);
        if let Some(dir) = Path::new(&self.path).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut f| writeln!(f, "{}", line));
        if written.is_err() {
            self.recent.push("!! AUDIT WRITE FAILED !!".into());
        }
        self.last = hash;
        self.recent.push(line);
    }

    /// Number of entries and hash of the last entry (`GENESIS` for an empty trail), for the project manifest.
    #[must_use]
    pub fn head(&self) -> (u64, String) {
        (self.bodies().len() as u64, self.last.clone())
    }

    /// The entries of this trail without their hash, oldest first.
    #[must_use]
    pub fn bodies(&self) -> Vec<String> {
        self.recent
            .iter()
            .filter(|l| l.matches('|').count() >= 4)
            .filter_map(|l| l.rsplit_once('|').map(|(body, _)| body.to_string()))
            .collect()
    }

    /// Continues with the trail in `path` (a new, empty one if the file does not exist); the
    /// user name is kept. The entries of the old trail stay where they are.
    pub fn open_trail(&mut self, path: &str) {
        let user = std::mem::take(&mut self.user);
        *self = Audit::new(path);
        self.user = user;
    }

    /// Carries all entries of this trail over to the trail in `new_path` and continues there.
    ///
    /// If that file does not exist yet, the entries are copied unchanged (the hash chain stays
    /// valid). If it exists, the entries are appended to it and re-chained. With `discard_old`
    /// the old file is deleted afterwards (used to turn a session trail into a project trail).
    ///
    /// # Errors
    ///
    /// Returns a message if the new trail cannot be written; the old trail stays active then.
    pub fn move_to(&mut self, new_path: &str, discard_old: bool) -> Result<(), String> {
        if self.path == new_path {
            return Ok(());
        }
        let old_path = self.path.clone();
        let lines: Vec<String> = self
            .recent
            .iter()
            .filter(|l| l.matches('|').count() >= 4)
            .cloned()
            .collect();
        let mut target = Audit::new(new_path);
        if target.recent.is_empty() {
            if let Some(dir) = Path::new(new_path).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut text = lines.join("\n");
            if !lines.is_empty() {
                text.push('\n');
            }
            std::fs::write(new_path, text).map_err(|e| format!("{}: {}", new_path, e))?;
            target = Audit::new(new_path);
        } else {
            for body in self.bodies() {
                target.append_body(&body);
            }
        }
        target.user = std::mem::take(&mut self.user);
        *self = target;
        if discard_old {
            let _ = std::fs::remove_file(&old_path);
        }
        Ok(())
    }
}

/// Re-computes the hash chain of an audit file; returns the number of entries if it is intact.
///
/// # Errors
///
/// Returns a message with the line number of the first entry that does not match its hash.
pub fn verify(path: &str) -> Result<usize, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    let mut last = "GENESIS".to_string();
    let mut n = 0;
    for (i, l) in s.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let (body, hash) = l
            .rsplit_once('|')
            .ok_or_else(|| format!("line {}: malformed", i + 1))?;
        if sha256::sha256_hex(format!("{}|{}", last, body).as_bytes()) != hash {
            return Err(format!(
                "line {}: hash mismatch - audit trail altered or corrupted",
                i + 1
            ));
        }
        last = hash.to_string();
        n += 1;
    }
    Ok(n)
}

/// Hash of the `entries`-th entry (counted from 1) of the trail in `path`, if it has that many.
///
/// A project records the number of entries and the hash of the last one when it is saved; the trail
/// may have grown since, so the check compares the hash at that position.
#[must_use]
pub fn hash_at(path: &str, entries: usize) -> Option<String> {
    if entries == 0 {
        return Some("GENESIS".to_string());
    }
    let text = std::fs::read_to_string(path).ok()?;
    let line = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .nth(entries - 1)?;
    line.rsplit_once('|').map(|(_, hash)| hash.to_string())
}
