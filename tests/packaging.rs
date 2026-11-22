//! Packaging: `cargo build --release` produces msQuant only; ThermoRawFileParser is never bundled.

mod common;

use common::source_files;
use std::path::Path;

/// Contents of a file of the project.
fn read(relative: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)).unwrap()
}

#[test]
fn only_the_icon_and_the_readme_are_embedded_in_the_program() {
    let mut found: Vec<String> = Vec::new();
    for (_, text) in source_files() {
        for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            for macro_name in ["include_bytes!(", "include_str!(", "include!("] {
                if let Some(pos) = line.find(macro_name) {
                    found.push(line[pos..].split(')').next().unwrap().to_string());
                }
            }
        }
    }
    found.sort();
    assert_eq!(
        found,
        vec![
            "include_bytes!(\"../img/msQuant-ico.ico\"",
            "include_str!(\"../../README.md\""
        ]
    );
}

#[test]
fn the_build_script_only_embeds_the_icon() {
    let build = read("build.rs");
    for word in ["tools", "ThermoRawFileParser", "copy", "include"] {
        assert!(!build.contains(word), "build.rs mentions {}", word);
    }
    assert!(build.contains("msQuant-ico.ico"));
}

#[test]
fn the_manifest_excludes_third_party_binaries_and_has_one_program() {
    let manifest = read("Cargo.toml");
    assert!(
        manifest.contains("exclude =")
            && manifest.contains("\"tools/\"")
            && manifest.contains("\"test-data/\"")
    );
    assert!(
        !manifest.to_lowercase().contains("thermorawfileparser"),
        "the converter is not a dependency"
    );
    assert_eq!(manifest.matches("[[bin]]").count(), 1);
}

#[test]
fn git_does_not_track_the_converter_or_the_local_data_folder() {
    let ignore = read(".gitignore");
    assert!(ignore.lines().any(|l| l.trim() == "tools/*"));
    assert!(ignore.lines().any(|l| l.trim() == "msQuant-default/"));
}

#[test]
fn an_existing_release_build_does_not_contain_the_converter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release");
    for name in ["msQuant.exe", "msQuant"] {
        let path = root.join(name);
        if !path.is_file() {
            eprintln!(
                "SKIPPED: {} does not exist (run `cargo build --release` first)",
                path.display()
            );
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        // assembly name inside ThermoRawFileParser's own libraries
        let needle = b"ThermoFisher.CommonCore";
        assert!(
            !bytes.windows(needle.len()).any(|w| w == needle),
            "{} contains Thermo libraries",
            path.display()
        );
        assert!(
            bytes.len() < 100 << 20,
            "{} is suspiciously large",
            path.display()
        );
    }
}
