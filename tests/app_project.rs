//! Project state, provenance and audit entries of the application model.

mod common;

use common::{add, app, targets};
use kvantx::app::Kind;
use kvantx::domain::types::ImportRoute;
use kvantx::formats::project::ProjectFile;
use kvantx::support::audit;
use std::path::Path;

#[test]
fn project_round_trip_keeps_manual_integration_and_its_reason() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.samples[0].kind = Kind::Standard;
    a.samples[0].cells[0].conc = 5.0;
    a.reason = "baseline drift under the peak".into();
    a.apply_edit(0, 0, 4.9, 5.1);
    a.commit(0, 0);
    let pf = a.project_file();
    let back = ProjectFile::from_json(&pf.to_json().unwrap()).unwrap();
    assert_eq!(pf, back);
    assert!(back.samples[0].cells[0].manual);
    assert_eq!(
        back.samples[0].cells[0].reason,
        "baseline drift under the peak"
    );
    let mut b = app();
    b.apply_params(&back.params);
    assert_eq!(
        b.settings, a.settings,
        "all analysis settings travel with the project"
    );
}

#[test]
fn concentrations_and_the_import_route_survive_a_project_round_trip() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.samples[0].cells[0].conc = 7.5;
    let back = ProjectFile::from_json(&a.project_file().to_json().unwrap()).unwrap();
    assert_eq!(back.samples[0].cells[0].conc, 7.5);
    assert_eq!(back.samples[0].import.route, ImportRoute::Conversion);
    assert_eq!(back.samples[0].import.detail, "synthetic test run");
    assert_eq!(
        back.samples[0].raw_sha256,
        "0".repeat(64),
        "the hash of the input file is kept"
    );
}

#[test]
fn manual_integration_is_audited_with_old_and_new_values() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.reason = "test".into();
    a.apply_edit(0, 0, 4.8, 5.2);
    a.commit(0, 0);
    assert!(
        a.audit
            .recent
            .iter()
            .any(|l| l.contains("MANUAL_INTEGRATION") && l.contains("reason=test"))
    );
    assert!(audit::verify(&a.audit.path).is_ok());
}

/// An application with a few audit entries, and its project.
fn with_trail() -> (kvantx::app::App, ProjectFile) {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.audit.log("TEST", "one");
    a.audit.log("TEST", "two");
    let pf = a.project_file();
    (a, pf)
}

#[test]
fn the_project_records_the_state_of_the_audit_trail() {
    let (a, pf) = with_trail();
    assert!(pf.audit.entries >= 2);
    assert_eq!(pf.audit.entries, a.audit.bodies().len() as u64);
    assert_eq!(
        audit::hash_at(&a.audit.path, pf.audit.entries as usize),
        Some(pf.audit.head.clone())
    );
}

#[test]
fn an_intact_audit_trail_matches_its_project_even_after_it_grew() {
    let (mut a, pf) = with_trail();
    assert!(kvantx::app::App::check_audit_anchor(&pf, Path::new(&a.audit.path)).is_empty());
    a.audit.log("LATER", "entries added after saving");
    assert!(kvantx::app::App::check_audit_anchor(&pf, Path::new(&a.audit.path)).is_empty());
}

#[test]
fn a_changed_missing_shortened_or_foreign_audit_trail_is_reported() {
    let (a, pf) = with_trail();
    let path = a.audit.path.clone();
    let original = std::fs::read_to_string(&path).unwrap();
    // an entry was edited
    std::fs::write(&path, original.replacen("|TEST|", "|TESX|", 1)).unwrap();
    let notes = kvantx::app::App::check_audit_anchor(&pf, Path::new(&path));
    assert!(
        notes.iter().any(|n| n.contains("audit trail problem")),
        "{notes:?}"
    );
    // entries were removed (the remaining chain is valid)
    let first: String = original.lines().take(1).map(|l| format!("{l}\n")).collect();
    std::fs::write(&path, first).unwrap();
    let notes = kvantx::app::App::check_audit_anchor(&pf, Path::new(&path));
    assert!(
        notes.iter().any(|n| n.contains("fewer entries")),
        "{notes:?}"
    );
    // the trail is gone
    std::fs::remove_file(&path).unwrap();
    assert!(!kvantx::app::App::check_audit_anchor(&pf, Path::new(&path)).is_empty());
    // a valid trail with a different history
    std::fs::write(&path, &original).unwrap();
    let mut other = pf.clone();
    other.audit.head = "f".repeat(64);
    let notes = kvantx::app::App::check_audit_anchor(&other, Path::new(&path));
    assert!(
        notes.iter().any(|n| n.contains("does not match")),
        "{notes:?}"
    );
}
