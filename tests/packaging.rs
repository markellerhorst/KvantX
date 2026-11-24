//! Packaging: `cargo build --release` produces KvantX only; ThermoRawFileParser is never bundled.

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
            "include_bytes!(\"../img/kvantx-ico.ico\"",
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
    assert!(build.contains("kvantx-ico.ico"));
}

#[test]
fn the_manifest_excludes_third_party_binaries_and_has_one_program() {
    let manifest = read("Cargo.toml");
    assert!(
        manifest.contains("exclude ="),
        "Cargo.toml has no exclude list"
    );
    for entry in ["tools/", "testData/", "KvantX-default/"] {
        assert!(
            manifest.contains(&format!("\"{}\"", entry)),
            "Cargo.toml: exclude lacks {}",
            entry
        );
    }
    let without_comments: String = manifest
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !without_comments
            .to_lowercase()
            .contains("thermorawfileparser"),
        "the converter is not a dependency"
    );
    assert_eq!(manifest.matches("[[bin]]").count(), 1);
}

#[test]
fn git_does_not_track_the_converter_or_the_local_data_folder() {
    let ignore = read(".gitignore");
    assert!(ignore.lines().any(|l| l.trim() == "tools/*"));
    assert!(ignore.lines().any(|l| l.trim() == "KvantX-default/"));
}

#[test]
fn an_existing_release_build_does_not_contain_the_converter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release");
    for name in ["kvantx.exe", "kvantx"] {
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

#[test]
fn private_instrument_data_is_git_ignored() {
    let ignore = read(".gitignore");
    for pattern in ["/testData/", "*.raw", "*.RAW", "*.mzML", "*.mzml"] {
        assert!(
            ignore.lines().any(|l| l.trim() == pattern),
            ".gitignore lacks {}",
            pattern
        );
    }
}

/// Instrument data must not sit anywhere in the tree except the ignored data folders.
#[test]
fn no_instrument_data_lies_in_the_repository_tree() {
    fn walk(dir: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if ![
                    "target",
                    ".git",
                    "testData",
                    "test-data",
                    "tools",
                    "KvantX-default",
                ]
                .contains(&name.as_str())
                {
                    walk(&path, found);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ["raw", "mzml", "wiff", "wiff2", "baf", "tdf"]
                    .contains(&ext.to_lowercase().as_str())
                {
                    found.push(path.display().to_string());
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(Path::new(env!("CARGO_MANIFEST_DIR")), &mut found);
    assert!(
        found.is_empty(),
        "instrument data outside the ignored folders: {:?}",
        found
    );
}
