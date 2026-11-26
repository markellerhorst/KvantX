//! The `.kvantx` project file (schema 1, described in `docs/PROJECT_FORMAT.md`).
//!
//! A project is one container file: `[8 bytes magic][u64 length of the manifest][manifest][for every
//! embedded sample: u64 length + the original bytes]`. The manifest is UTF-8 JSON with an explicit
//! schema version. It holds the targets, the analysis settings, the input files (path, SHA-256 and the
//! import route), the integration state of every target in every file (including the reason of a manual
//! change) and an anchor to the audit trail. The raw data are only copied into the container when the
//! user chooses so; otherwise they are re-imported from the recorded paths and verified by SHA-256.
//!
//! Everything read from a project is untrusted: sizes are limited, lengths use checked arithmetic and
//! the manifest is validated field by field. A project written by a newer schema is refused, not guessed.

use crate::domain::Analyte;
use crate::domain::types::{ImportInfo, ImportRoute, Kind};
use crate::support::json::{self, Value};
use crate::support::sha256::Sha256;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Value of the `format` field of every manifest.
pub const FORMAT_NAME: &str = "kvantx-project";

/// Schema version this program writes and reads.
pub const SCHEMA_VERSION: u64 = 1;

/// Largest manifest that is read (64 MiB); real projects need a few kilobytes.
const MAX_TEXT_BYTES: u64 = 64 << 20;

/// Largest number of files, of targets and of cells per file a project may list. A project is not
/// trusted: every file starts a background job and every target allocates memory.
const MAX_ENTRIES: usize = 2000;

/// Largest number of product ions of one target.
const MAX_IONS: usize = 64;

/// Largest number of settings in a manifest.
const MAX_SETTINGS: usize = 256;

/// Longest name of a target, in characters.
const MAX_NAME: usize = 256;

/// Longest file path, in characters.
const MAX_PATH: usize = 4096;

/// Longest free text (import detail, manual reason), in characters.
const MAX_DETAIL: usize = 8192;

/// Largest retention-time shift of a file, in minutes.
const MAX_SHIFT: f64 = 120.0;

/// First bytes of every project: `KVANTX`, a zero byte and the version of the container layout.
const MAGIC: &[u8; 8] = b"KVANTX\x00\x01";

/// Stored state of one target in one file.
#[derive(Clone, Debug, PartialEq)]
pub struct CellRec {
    /// Index of the target in the target list.
    pub analyte: usize,
    /// Whether a peak was found or defined.
    pub found: bool,
    /// Whether the boundaries were set by hand.
    pub manual: bool,
    /// Start of the integration window in minutes.
    pub start: f64,
    /// End of the integration window in minutes.
    pub end: f64,
    /// Whether the file is used as a calibration point for this target.
    pub include: bool,
    /// Standard concentration of this target in this file (0 = not a standard).
    pub conc: f64,
    /// Reason given for the last manual change of the boundaries (empty if none).
    pub reason: String,
}

/// Stored state of one file.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleRec {
    /// Path of the input file as it was imported.
    pub path: String,
    /// SHA-256 of the input file (64 lowercase hexadecimal digits).
    pub raw_sha256: String,
    /// Role of the file: one of the labels of [`Kind`].
    pub kind: String,
    /// Retention-time shift in minutes.
    pub rt_shift: f64,
    /// Whether the file was selected.
    pub show: bool,
    /// Whether the original file is stored inside the project.
    pub embedded: bool,
    /// How the file was imported.
    pub import: ImportInfo,
    /// Integration state of the targets.
    pub cells: Vec<CellRec>,
}

/// The state of the audit trail when the project was saved.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AuditAnchor {
    /// Number of entries in the trail.
    pub entries: u64,
    /// Hash of the last entry (`GENESIS` for an empty trail).
    pub head: String,
}

/// Everything a project stores except the raw data.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProjectFile {
    /// Version of the program that wrote the project.
    pub app_version: String,
    /// Time of saving, ISO 8601 in UTC.
    pub saved_utc: String,
    /// User name at the time of saving.
    pub user: String,
    /// Analysis settings as `key`/`value` pairs.
    pub params: Vec<(String, String)>,
    /// The targets.
    pub analytes: Vec<Analyte>,
    /// The input files.
    pub samples: Vec<SampleRec>,
    /// Anchor to the audit trail.
    pub audit: AuditAnchor,
}

/// A setting as JSON: `true`/`false` and numbers whose text is reproduced exactly become typed values.
fn setting_to_value(text: &str) -> Value {
    match text {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    match text.parse::<f64>() {
        Ok(n) if n.is_finite() && n.to_string() == text => Value::Number(n),
        _ => Value::String(text.to_string()),
    }
}

/// The text form of a setting read from JSON.
fn value_to_setting(value: &Value) -> Option<String> {
    match value {
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// The member `key` of the object `v`, or an error that names the place.
fn field<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("{ctx}: \"{key}\" is missing"))
}

fn text_of(v: &Value, key: &str, ctx: &str, max: usize) -> Result<String, String> {
    let s = field(v, key, ctx)?.as_str().ok_or_else(|| format!("{ctx}: \"{key}\" must be text"))?;
    if s.chars().count() > max {
        return Err(format!("{ctx}: \"{key}\" is longer than {max} characters"));
    }
    Ok(s.to_string())
}

fn number(v: &Value, key: &str, ctx: &str) -> Result<f64, String> {
    field(v, key, ctx)?.as_f64().ok_or_else(|| format!("{ctx}: \"{key}\" must be a number"))
}

fn flag(v: &Value, key: &str, ctx: &str) -> Result<bool, String> {
    field(v, key, ctx)?.as_bool().ok_or_else(|| format!("{ctx}: \"{key}\" must be true or false"))
}

fn optional_number(v: &Value, key: &str, ctx: &str) -> Result<Option<f64>, String> {
    match field(v, key, ctx)? {
        Value::Null => Ok(None),
        Value::Number(n) => Ok(Some(*n)),
        _ => Err(format!("{ctx}: \"{key}\" must be a number or null")),
    }
}

fn list<'a>(v: &'a Value, key: &str, ctx: &str, max: usize) -> Result<&'a [Value], String> {
    let items = field(v, key, ctx)?.as_array().ok_or_else(|| format!("{ctx}: \"{key}\" must be a list"))?;
    if items.len() > max {
        return Err(format!("{ctx}: \"{key}\" has more than {max} entries"));
    }
    Ok(items)
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl ProjectFile {
    /// The manifest as a JSON value.
    fn to_value(&self) -> Value {
        let n = Value::Number;
        let targets = self.analytes.iter().map(|a| {
            json::object(vec![
                ("name", json::string(&a.name)),
                ("rt_min", json::optional_number(a.rt)),
                ("product_ions", Value::Array(a.ions.iter().map(|&m| n(m)).collect())),
                ("precursor_mz", json::optional_number(a.precursor)),
            ])
        });
        let samples = self.samples.iter().map(|s| {
            let cells = s.cells.iter().map(|c| {
                json::object(vec![
                    ("target", n(c.analyte as f64)),
                    ("peak_found", Value::Bool(c.found)),
                    ("manual", Value::Bool(c.manual)),
                    ("start_min", n(c.start)),
                    ("end_min", n(c.end)),
                    ("use_for_calibration", Value::Bool(c.include)),
                    ("standard_conc", n(c.conc)),
                    ("manual_reason", json::string(&c.reason)),
                ])
            });
            json::object(vec![
                ("path", json::string(&s.path)),
                ("raw_sha256", json::string(&s.raw_sha256)),
                ("kind", json::string(&s.kind)),
                ("rt_shift_min", n(s.rt_shift)),
                ("selected", Value::Bool(s.show)),
                ("embedded", Value::Bool(s.embedded)),
                ("import", json::object(vec![("route", json::string(s.import.route.key())), ("detail", json::string(&s.import.detail))])),
                ("cells", Value::Array(cells.collect())),
            ])
        });
        json::object(vec![
            ("format", json::string(FORMAT_NAME)),
            ("schema", n(SCHEMA_VERSION as f64)),
            ("saved_by", json::object(vec![("application", json::string("KvantX")), ("version", json::string(&self.app_version))])),
            ("saved_utc", json::string(&self.saved_utc)),
            ("user", json::string(&self.user)),
            ("settings", Value::Object(self.params.iter().map(|(k, v)| (k.clone(), setting_to_value(v))).collect())),
            ("targets", Value::Array(targets.collect())),
            ("samples", Value::Array(samples.collect())),
            ("audit", json::object(vec![("entries", n(self.audit.entries as f64)), ("head", json::string(&self.audit.head))])),
        ])
    }

    /// The manifest as JSON text. The text is read back and validated before it is returned, so that a
    /// project that could not be opened again is never written.
    ///
    /// # Errors
    ///
    /// Returns a message if a value is not finite or the project would not pass validation.
    pub fn to_json(&self) -> Result<String, String> {
        let text = self.to_value().to_pretty_string()?;
        ProjectFile::from_json(&text).map_err(|e| format!("refusing to write an invalid project: {e}"))?;
        Ok(text)
    }

    /// Reads and validates a manifest.
    ///
    /// # Errors
    ///
    /// Returns a message for malformed JSON, a wrong format name, a schema that this program does not
    /// read, a missing or mistyped field, a value outside its range or a limit that is exceeded.
    pub fn from_json(text: &str) -> Result<ProjectFile, String> {
        let root = json::parse(text)?;
        if root.as_object().is_none() {
            return Err("the project manifest is not a JSON object".into());
        }
        match root.get("format").and_then(Value::as_str) {
            Some(FORMAT_NAME) => {}
            Some(other) => return Err(format!("not a KvantX project (format \"{other}\")")),
            None => return Err("not a KvantX project (\"format\" is missing)".into()),
        }
        let schema = root.get("schema").and_then(Value::as_u64).ok_or("\"schema\" is missing or not an integer")?;
        if schema > SCHEMA_VERSION {
            return Err(format!(
                "this project uses schema {schema}, written by a newer KvantX; this version reads schema {SCHEMA_VERSION}"
            ));
        }
        if schema < 1 {
            return Err("invalid schema version 0".into());
        }
        let saved_by = field(&root, "saved_by", "manifest")?;
        let mut pf = ProjectFile {
            app_version: text_of(saved_by, "version", "saved_by", 64)?,
            saved_utc: text_of(&root, "saved_utc", "manifest", 40)?,
            user: text_of(&root, "user", "manifest", MAX_NAME)?,
            ..Default::default()
        };
        let settings = field(&root, "settings", "manifest")?.as_object().ok_or("\"settings\" must be an object")?;
        if settings.len() > MAX_SETTINGS {
            return Err(format!("more than {MAX_SETTINGS} settings"));
        }
        for (key, value) in settings {
            if key.is_empty() || key.chars().count() > 64 {
                return Err("a setting has an empty or too long name".into());
            }
            let value = value_to_setting(value).ok_or_else(|| format!("setting \"{key}\" must be text, a number or true/false"))?;
            if value.chars().count() > MAX_NAME {
                return Err(format!("setting \"{key}\" is too long"));
            }
            pf.params.push((key.clone(), value));
        }
        for (i, t) in list(&root, "targets", "manifest", MAX_ENTRIES)?.iter().enumerate() {
            let ctx = format!("target {}", i + 1);
            let name = text_of(t, "name", &ctx, MAX_NAME)?;
            if name.trim().is_empty() {
                return Err(format!("{ctx}: the name is empty"));
            }
            let rt = optional_number(t, "rt_min", &ctx)?;
            if rt.is_some_and(|r| r < 0.0) {
                return Err(format!("{ctx}: the retention time is negative"));
            }
            let precursor = optional_number(t, "precursor_mz", &ctx)?;
            if precursor.is_some_and(|m| m <= 0.0) {
                return Err(format!("{ctx}: the precursor m/z must be positive"));
            }
            let ions: Vec<f64> = list(t, "product_ions", &ctx, MAX_IONS)?
                .iter()
                .map(|v| v.as_f64().filter(|m| *m > 0.0).ok_or_else(|| format!("{ctx}: product ions must be positive numbers")))
                .collect::<Result<_, _>>()?;
            if ions.is_empty() {
                return Err(format!("{ctx}: no product ion"));
            }
            pf.analytes.push(Analyte { name, rt, ions, precursor });
        }
        for (i, s) in list(&root, "samples", "manifest", MAX_ENTRIES)?.iter().enumerate() {
            let ctx = format!("file {}", i + 1);
            let path = text_of(s, "path", &ctx, MAX_PATH)?;
            if path.is_empty() {
                return Err(format!("{ctx}: the path is empty"));
            }
            if pf.samples.iter().any(|x| x.path == path) {
                return Err(format!("{ctx}: the path {path} is listed twice"));
            }
            let raw_sha256 = text_of(s, "raw_sha256", &ctx, 64)?;
            if !is_sha256(&raw_sha256) {
                return Err(format!("{ctx}: \"raw_sha256\" must be 64 lowercase hexadecimal digits"));
            }
            let kind = text_of(s, "kind", &ctx, 16)?;
            if !Kind::ALL.iter().any(|k| k.label() == kind) {
                return Err(format!("{ctx}: unknown type \"{kind}\""));
            }
            let rt_shift = number(s, "rt_shift_min", &ctx)?;
            if rt_shift.abs() > MAX_SHIFT {
                return Err(format!("{ctx}: the retention-time shift is outside +-{MAX_SHIFT} min"));
            }
            let import_obj = field(s, "import", &ctx)?;
            let route_key = text_of(import_obj, "route", &ctx, 32)?;
            let route = ImportRoute::from_key(&route_key).ok_or_else(|| format!("{ctx}: unknown import route \"{route_key}\""))?;
            let import = ImportInfo { route, detail: text_of(import_obj, "detail", &ctx, MAX_DETAIL)? };
            let mut cells: Vec<CellRec> = Vec::new();
            for (k, c) in list(s, "cells", &ctx, MAX_ENTRIES)?.iter().enumerate() {
                let cctx = format!("{ctx}, cell {}", k + 1);
                let target = field(c, "target", &cctx)?.as_u64().ok_or_else(|| format!("{cctx}: \"target\" must be an index"))? as usize;
                if target >= pf.analytes.len() {
                    return Err(format!("{cctx}: target {target} does not exist"));
                }
                if cells.iter().any(|x| x.analyte == target) {
                    return Err(format!("{cctx}: target {target} appears twice"));
                }
                let (found, start, end) = (flag(c, "peak_found", &cctx)?, number(c, "start_min", &cctx)?, number(c, "end_min", &cctx)?);
                if found && start > end {
                    return Err(format!("{cctx}: the integration window is reversed"));
                }
                let conc = number(c, "standard_conc", &cctx)?;
                if conc < 0.0 {
                    return Err(format!("{cctx}: the standard concentration is negative"));
                }
                cells.push(CellRec {
                    analyte: target,
                    found,
                    manual: flag(c, "manual", &cctx)?,
                    start,
                    end,
                    include: flag(c, "use_for_calibration", &cctx)?,
                    conc,
                    reason: text_of(c, "manual_reason", &cctx, MAX_DETAIL)?,
                });
            }
            pf.samples.push(SampleRec {
                path,
                raw_sha256,
                kind,
                rt_shift,
                show: flag(s, "selected", &ctx)?,
                embedded: flag(s, "embedded", &ctx)?,
                import,
                cells,
            });
        }
        let audit = field(&root, "audit", "manifest")?;
        let head = text_of(audit, "head", "audit", 64)?;
        if !head.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("audit: \"head\" has invalid characters".into());
        }
        let entries = field(audit, "entries", "audit")?.as_u64().ok_or("audit: \"entries\" must be a non-negative integer")?;
        pf.audit = AuditAnchor { entries, head };
        Ok(pf)
    }
}

/// Where the original .raw bytes of a loaded file can be found again (for saving into a project).
#[derive(Clone, Debug, PartialEq)]
pub enum RawSource {
    Disk(PathBuf),
    Embedded { project: PathBuf, offset: u64, len: u64 },
}

/// Data to copy into a project container.
pub enum BlobSource {
    File(PathBuf),
    Range { path: PathBuf, offset: u64, len: u64 },
}

/// Adds `what` to an I/O error message.
fn io<T>(r: std::io::Result<T>, what: &str) -> Result<T, String> { r.map_err(|e| format!("{}: {}", what, e)) }

/// Copies exactly `len` bytes from `src` to `dst` in chunks, optionally feeding them to `hash`.
fn copy_exact(src: &mut std::fs::File, len: u64, dst: &mut impl Write, mut hash: Option<&mut Sha256>) -> Result<(), String> {
    let mut buf = vec![0u8; 1 << 20];
    let mut left = len;
    while left > 0 {
        let n = left.min(buf.len() as u64) as usize;
        io(src.read_exact(&mut buf[..n]), "reading source")?;
        io(dst.write_all(&buf[..n]), "writing")?;
        if let Some(h) = hash.as_mut() { h.update(&buf[..n]); }
        left -= n as u64;
    }
    Ok(())
}

/// Offset and length of the raw file embedded in a container for every sample; `None` for a sample without one.
pub type EmbeddedBlobs = Vec<Option<(u64, u64)>>;

/// Writes the project (streaming, large raw files never need to fit in memory). Returns (data offset, length) of each embedded blob.
/// The data is first written to a file in `tmp_dir` and moved to `path` at the end, so a source inside the same project file stays readable until then.
///
/// # Errors
///
/// Returns a message if the manifest is invalid, a source cannot be read or the file cannot be written.
pub fn write_container(path: &Path, pf: &ProjectFile, blobs: &[Option<BlobSource>], tmp_dir: &Path) -> Result<EmbeddedBlobs, String> {
    if blobs.len() != pf.samples.len() { return Err("internal error: blob list does not match the samples".into()); }
    let mut pf2 = pf.clone();
    for (s, b) in pf2.samples.iter_mut().zip(blobs.iter()) { s.embedded = b.is_some(); }
    let manifest = pf2.to_json()?;
    io(std::fs::create_dir_all(tmp_dir), &tmp_dir.display().to_string())?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "project.kvantx".into());
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = tmp_dir.join(format!("{}.{}.{}.tmp", file_name, std::process::id(), nanos));
    let tmp_file = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp);
    let mut out = BufWriter::new(io(tmp_file, &tmp.display().to_string())?);
    let _cleanup = RemoveOnDrop(tmp.clone());
    io(out.write_all(MAGIC), "writing")?;
    io(out.write_all(&(manifest.len() as u64).to_le_bytes()), "writing")?;
    io(out.write_all(manifest.as_bytes()), "writing")?;
    let mut pos = 16 + manifest.len() as u64;
    let mut offs: EmbeddedBlobs = vec![None; blobs.len()];
    for (i, b) in blobs.iter().enumerate() {
        let Some(src) = b else { continue };
        let (mut f, off, len) = match src {
            BlobSource::File(p) => {
                let f = io(std::fs::File::open(p), &p.display().to_string())?;
                let len = io(f.metadata(), "metadata")?.len();
                (f, 0, len)
            }
            BlobSource::Range { path, offset, len } => (io(std::fs::File::open(path), &path.display().to_string())?, *offset, *len),
        };
        io(f.seek(SeekFrom::Start(off)), "seek")?;
        io(out.write_all(&len.to_le_bytes()), "writing")?;
        copy_exact(&mut f, len, &mut out, None)?;
        offs[i] = Some((pos + 8, len));
        pos += 8 + len;
    }
    io(out.flush(), "writing")?;
    drop(out);
    move_file(&tmp, path)?;
    Ok(offs)
}

/// Deletes a file when dropped, so that a failed write leaves no temporary file behind.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Moves a file; falls back to copy and delete when source and target are on different volumes.
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() { return Ok(()); }
    io(std::fs::copy(from, to), "writing the project file")?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

/// Reads and validates the manifest and locates the embedded blobs (without loading them).
///
/// # Errors
///
/// Returns a message if the file is not a KvantX project, uses a container layout or schema this
/// program does not read, is truncated or corrupt, or its manifest is invalid.
pub fn read_container(path: &Path) -> Result<(ProjectFile, EmbeddedBlobs), String> {
    let mut f = io(std::fs::File::open(path), &path.display().to_string())?;
    let file_len = io(f.metadata(), "metadata")?.len();
    let mut head = [0u8; 8];
    f.read_exact(&mut head).map_err(|_| "not a KvantX project file (too short)".to_string())?;
    if &head[..6] != b"KVANTX" {
        return Err("not a KvantX project file".into());
    }
    if head != *MAGIC {
        return Err(format!("unsupported project container layout {}.{} (this KvantX reads 0.1)", head[6], head[7]));
    }
    let mut l = [0u8; 8];
    io(f.read_exact(&mut l), "reading")?;
    let tl = u64::from_le_bytes(l);
    // The lengths come from the file itself: use checked arithmetic and refuse absurd values.
    let text_end = tl.checked_add(16).filter(|&e| e <= file_len && tl <= MAX_TEXT_BYTES)
        .ok_or("project file is truncated or corrupt")?;
    let mut tb = vec![0u8; tl as usize];
    io(f.read_exact(&mut tb), "reading")?;
    let manifest = String::from_utf8(tb).map_err(|_| "the project manifest is not valid UTF-8".to_string())?;
    let pf = ProjectFile::from_json(&manifest)?;
    let mut pos = text_end;
    let mut offs = Vec::with_capacity(pf.samples.len());
    for s in &pf.samples {
        if !s.embedded { offs.push(None); continue; }
        let data_start = pos.checked_add(8).filter(|&e| e <= file_len).ok_or("project file is truncated")?;
        io(f.seek(SeekFrom::Start(pos)), "seek")?;
        let mut lb = [0u8; 8];
        io(f.read_exact(&mut lb), "reading")?;
        let len = u64::from_le_bytes(lb);
        let data_end = data_start.checked_add(len).filter(|&e| e <= file_len)
            .ok_or_else(|| format!("project file is truncated inside the embedded file {}", s.path))?;
        offs.push(Some((data_start, len)));
        pos = data_end;
    }
    if pos != file_len {
        return Err("unexpected data after the last embedded file: the project file is corrupt".into());
    }
    Ok((pf, offs))
}

/// Copies an embedded blob to `dest` (streaming) and returns its SHA-256.
pub fn extract_blob(project: &Path, offset: u64, len: u64, dest: &Path) -> Result<String, String> {
    let mut f = io(std::fs::File::open(project), &project.display().to_string())?;
    io(f.seek(SeekFrom::Start(offset)), "seek")?;
    // `create_new`: never overwrite (or write through a link to) an existing file.
    let dest_file = std::fs::OpenOptions::new().write(true).create_new(true).open(dest);
    let mut out = BufWriter::new(io(dest_file, &dest.display().to_string())?);
    let mut h = Sha256::new();
    copy_exact(&mut f, len, &mut out, Some(&mut h))?;
    io(out.flush(), "writing")?;
    Ok(h.finish_hex())
}

/// Reads an embedded blob completely into memory (used for embedded mzML files, which need no converter).
pub fn read_blob(project: &Path, offset: u64, len: u64) -> Result<Vec<u8>, String> {
    let mut f = io(std::fs::File::open(project), &project.display().to_string())?;
    io(f.seek(SeekFrom::Start(offset)), "seek")?;
    // A length that cannot be allocated is an error, not an abort of the program.
    let mut v: Vec<u8> = Vec::new();
    let size = usize::try_from(len).map_err(|_| "embedded file is too large".to_string())?;
    v.try_reserve_exact(size).map_err(|_| "not enough memory for the embedded file".to_string())?;
    io(f.take(len).read_to_end(&mut v), "reading")?;
    if v.len() as u64 != len { return Err("project file is truncated".into()); }
    Ok(v)
}
