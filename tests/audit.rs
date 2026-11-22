//! The hash-chained audit trail.

mod common;

use common::tmp_dir;
use msquant::support::audit::{self, Audit};

#[test]
fn audit_trail_detects_tampering_and_continues_across_sessions() {
    let p = std::env::temp_dir().join(format!("lcmsq_audit_{}.log", std::process::id()));
    let _ = std::fs::remove_file(&p);
    let path = p.to_string_lossy().to_string();
    let mut a = audit::Audit::new(&path);
    a.user = "tester".into();
    a.log("TEST1", "first");
    a.log("TEST2", "second");
    a.log("TEST3", "third | with pipe");
    assert_eq!(audit::verify(&path).unwrap(), 3);
    let mut b = audit::Audit::new(&path);
    b.log("TEST4", "after restart");
    assert_eq!(audit::verify(&path).unwrap(), 4);
    let txt = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, txt.replace("second", "SECOND")).unwrap();
    assert!(audit::verify(&path).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn default_user_is_na() {
    let p = std::env::temp_dir().join(format!("lcmsq_audit_user_{}.log", std::process::id()));
    assert_eq!(audit::Audit::new(&p.to_string_lossy()).user, "N/A");
}

#[test]
fn a_session_trail_becomes_the_project_trail_when_the_project_is_saved() {
    let dir = tmp_dir("audit_move");
    let session = dir.join("session.log");
    let project = dir.join("run.audit.log");
    let mut a = Audit::new(&session.to_string_lossy());
    a.user = "tester".into();
    a.log("IMPORT", "a.raw");
    a.log("INTEGRATION", "peak");
    a.move_to(&project.to_string_lossy(), true).unwrap();
    assert!(
        !session.exists(),
        "the session file is deleted: no entry exists twice"
    );
    assert_eq!(audit::verify(&project.to_string_lossy()).unwrap(), 2);
    a.log("EXPORT", "results.csv");
    assert_eq!(audit::verify(&project.to_string_lossy()).unwrap(), 3);
    assert!(
        std::fs::read_to_string(&project)
            .unwrap()
            .lines()
            .all(|l| l.contains("|tester|")),
        "the user name is kept"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_over_an_existing_project_appends_the_session_entries() {
    let dir = tmp_dir("audit_append");
    let project = dir.join("run.audit.log").to_string_lossy().to_string();
    let mut earlier = Audit::new(&project);
    earlier.log("OLD", "1");
    earlier.log("OLD", "2");
    let mut session = Audit::new(&dir.join("s.log").to_string_lossy());
    session.log("NEW", "3");
    session.move_to(&project, true).unwrap();
    assert_eq!(
        audit::verify(&project).unwrap(),
        3,
        "re-chained, still a valid trail"
    );
    assert_eq!(session.bodies().len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_project_continues_its_own_trail_only() {
    let dir = tmp_dir("audit_open");
    let project = dir.join("p.audit.log").to_string_lossy().to_string();
    let mut once = Audit::new(&project);
    once.log("FIRST", "x");
    let mut a = Audit::new(&dir.join("session.log").to_string_lossy());
    a.user = "me".into();
    a.log("BEFORE", "not part of the project");
    a.open_trail(&project);
    assert_eq!(a.user, "me");
    assert_eq!(
        a.bodies().len(),
        1,
        "the entries of the session are not copied into the project"
    );
    a.log("SECOND", "y");
    assert_eq!(audit::verify(&project).unwrap(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
