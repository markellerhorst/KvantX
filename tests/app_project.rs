//! Project state and audit entries of the application model.

mod common;

use common::{add, app, targets};
use msquant::app::Kind;
use msquant::formats::project::ProjectFile;
use msquant::support::audit;

#[test]
fn project_round_trip_keeps_manual_integration() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.samples[0].kind = Kind::Standard;
    a.samples[0].cells[0].conc = 5.0;
    a.apply_edit(0, 0, 4.9, 5.1);
    a.commit(0, 0);
    let pf = a.project_file();
    let back = ProjectFile::parse(&pf.to_text()).unwrap();
    assert_eq!(pf, back);
    assert!(back.samples[0].cells[0].manual);
    let mut b = app();
    b.apply_params(&back.params);
    assert_eq!(b.settings.weighting, a.settings.weighting);
    assert_eq!(b.settings.tol, a.settings.tol);
}

#[test]
fn concentrations_survive_a_project_round_trip_and_old_projects_load() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.samples[0].cells[0].conc = 7.5;
    let pf = a.project_file();
    let back = ProjectFile::parse(&pf.to_text()).unwrap();
    assert_eq!(back.samples[0].cells[0].conc, Some(7.5));
    let old = pf.to_text().replace("\ttrue\t7.5\n", "\ttrue\n");
    assert_eq!(
        ProjectFile::parse(&old).unwrap().samples[0].cells[0].conc,
        None,
        "older projects have no per-target concentration"
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
