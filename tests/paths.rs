//! File locations.

use msquant::support::paths;
use std::path::Path;

#[test]
fn installation_root_is_found_from_the_executable_path() {
    assert_eq!(
        paths::root_from_exe(Path::new("/apps/msQuant/msQuant.exe")),
        Path::new("/apps/msQuant")
    );
    assert_eq!(
        paths::root_from_exe(Path::new("/work/msquant/target/debug/msQuant.exe")),
        Path::new("/work/msquant"),
        "cargo run uses the project root"
    );
    assert_eq!(
        paths::root_from_exe(Path::new("/work/msquant/target/release/msQuant")),
        Path::new("/work/msquant")
    );
    assert_eq!(
        paths::root_from_exe(Path::new("/x/debug/msQuant")),
        Path::new("/x/debug"),
        "a folder called debug outside of target is a normal install folder"
    );
}

#[test]
fn cargo_test_executables_resolve_to_the_project_root() {
    assert_eq!(
        paths::root_from_exe(Path::new("/work/msquant/target/debug/deps/audit-1a2b3c")),
        Path::new("/work/msquant")
    );
}

#[test]
fn the_audit_trail_of_a_project_lies_next_to_the_project() {
    assert_eq!(
        paths::project_audit_file(Path::new("/data/run1.quant")),
        Path::new("/data/run1.audit.log")
    );
}
