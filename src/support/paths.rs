//! Locations of the files that KvantX writes.
//!
//! Everything the program needs for itself (settings, audit trails of unsaved sessions and temporary files) lives in
//! the folder [`DEFAULT_DIR_NAME`] in the root folder of the installation. Files the user creates
//! (projects, results, figures) go to a standard folder that can be chosen in the preferences; it
//! defaults to the same folder. Nothing else is written to disk during normal use.

use std::path::{Path, PathBuf};

/// Name of the folder next to the program that holds settings, audit trail and temporary files.
pub const DEFAULT_DIR_NAME: &str = "KvantX-default";

/// Derives the installation root from the path of the executable.
///
/// A program started through `cargo run` sits in `target/debug` or `target/release`. In that case
/// the project root (the folder that contains `target`) is returned, so that the data folder does
/// not end up hidden inside the build output.
#[must_use]
pub fn root_from_exe(exe: &Path) -> PathBuf {
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let folder_name = |p: &Path| p.file_name().and_then(|n| n.to_str()).map(str::to_owned);
    // `cargo test` runs executables from `target/<profile>/deps`.
    let build_dir = if folder_name(dir).as_deref() == Some("deps") {
        dir.parent().unwrap_or(dir)
    } else {
        dir
    };
    let in_build_output = matches!(
        folder_name(build_dir).as_deref(),
        Some("debug") | Some("release")
    ) && build_dir.parent().and_then(folder_name).as_deref()
        == Some("target");
    if in_build_output {
        if let Some(project_root) = build_dir.parent().and_then(Path::parent) {
            return project_root.to_path_buf();
        }
    }
    dir.to_path_buf()
}

/// Root folder of the installation (see [`root_from_exe`]); the working directory if the path of
/// the executable cannot be determined.
#[must_use]
pub fn install_root() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => root_from_exe(&exe),
        Err(_) => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Path of the default data folder, without creating it.
#[must_use]
pub fn default_dir_path() -> PathBuf {
    install_root().join(DEFAULT_DIR_NAME)
}

/// Path of the default data folder, created if it does not exist yet.
pub fn ensure_default_dir() -> PathBuf {
    let dir = default_dir_path();
    // A failure shows up later, when the first file is written, with a precise error message.
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// File with the user settings.
#[must_use]
pub fn settings_file() -> PathBuf {
    default_dir_path().join("settings.txt")
}

/// Folder for the audit trails of sessions that were not saved to a project (path only).
#[must_use]
pub fn audit_dir_path() -> PathBuf {
    default_dir_path().join("audit")
}

/// New file for the audit trail of this session; the folder is created if necessary.
#[must_use]
pub fn session_audit_file() -> PathBuf {
    let dir = audit_dir_path();
    let _ = std::fs::create_dir_all(&dir);
    let stamp = crate::support::audit::iso_now().replace(':', "-");
    dir.join(format!("session_{}_{}.log", stamp, std::process::id()))
}

/// Audit trail of a project: next to the project file, with the extension `.audit.log`.
#[must_use]
pub fn project_audit_file(project: &Path) -> PathBuf {
    project.with_extension("audit.log")
}

/// File with the layout of the last session.
#[must_use]
pub fn layout_file() -> PathBuf {
    default_dir_path().join("layout.txt")
}

/// Folder for temporary files, created if it does not exist yet.
pub fn temp_dir() -> PathBuf {
    let dir = default_dir_path().join("tmp");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Deletes everything in the temporary folder (leftovers of an interrupted run).
pub fn clean_temp() {
    let _ = std::fs::remove_dir_all(default_dir_path().join("tmp"));
}
