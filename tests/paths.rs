//! File locations.

use kvantx::support::paths;
use std::path::Path;

#[test]
fn installation_root_is_found_from_the_executable_path() {
    assert_eq!(
        paths::root_from_exe(Path::new("/apps/KvantX/KvantX.exe")),
        Path::new("/apps/KvantX")
    );
    assert_eq!(
        paths::root_from_exe(Path::new("/work/kvantx/target/debug/KvantX.exe")),
        Path::new("/work/kvantx"),
        "cargo run uses the project root"
    );
    assert_eq!(
        paths::root_from_exe(Path::new("/work/kvantx/target/release/KvantX")),
        Path::new("/work/kvantx")
    );
    assert_eq!(
        paths::root_from_exe(Path::new("/x/debug/KvantX")),
        Path::new("/x/debug"),
        "a folder called debug outside of target is a normal install folder"
    );
}

#[test]
fn cargo_test_executables_resolve_to_the_project_root() {
    assert_eq!(
        paths::root_from_exe(Path::new("/work/kvantx/target/debug/deps/audit-1a2b3c")),
        Path::new("/work/kvantx")
    );
}

#[test]
fn the_audit_trail_of_a_project_lies_next_to_the_project() {
    assert_eq!(
        paths::project_audit_file(Path::new("/data/run1.kvantx")),
        Path::new("/data/run1.audit.log")
    );
}
