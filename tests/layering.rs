//! The layers of the crate depend only on the layers below them.
//!
//! support, domain < analysis, formats < app < gui. The lower layers know nothing about the user
//! interface; the application layer does not draw.

mod common;

use common::source_files;

/// Layers a layer may use (besides itself).
const ALLOWED: [(&str, &[&str]); 6] = [
    ("support", &[]),
    ("domain", &["support"]),
    ("analysis", &["domain", "support"]),
    ("formats", &["domain", "support"]),
    ("app", &["domain", "support", "analysis", "formats"]),
    ("gui", &["domain", "support", "analysis", "formats", "app"]),
];

const LAYERS: [&str; 6] = ["support", "domain", "analysis", "formats", "app", "gui"];

/// The layer a file belongs to, or `None` for `lib.rs` and `main.rs`.
fn layer_of(path: &str) -> Option<&'static str> {
    let normalized = path.replace('\\', "/");
    LAYERS
        .into_iter()
        .find(|l| normalized.contains(&format!("/src/{}/", l)))
}

#[test]
fn no_layer_uses_a_layer_above_it() {
    for (path, text) in source_files() {
        let Some(layer) = layer_of(&path) else {
            continue;
        };
        let allowed = ALLOWED.iter().find(|(l, _)| *l == layer).unwrap().1;
        for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            for other in LAYERS {
                if other != layer && !allowed.contains(&other) {
                    assert!(
                        !line.contains(&format!("crate::{}::", other)),
                        "{} ({}) uses the {} layer: {}",
                        path,
                        layer,
                        other,
                        line.trim()
                    );
                }
            }
        }
    }
}

#[test]
fn the_calculation_and_file_layers_do_not_know_the_user_interface() {
    for (path, text) in source_files() {
        let Some(layer) = layer_of(&path) else {
            continue;
        };
        if ["support", "domain", "analysis", "formats"].contains(&layer) {
            for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
                assert!(
                    !line.contains("egui") && !line.contains("eframe"),
                    "{}: {}",
                    path,
                    line.trim()
                );
            }
        }
    }
}
