//! Undo and redo of the application model.

mod common;

use common::{app, targets};

#[test]
fn undo_history_is_limited_to_ten_steps() {
    let mut a = app();
    targets(&mut a);
    for _ in 0..12 {
        a.add_target();
    }
    assert_eq!(a.analytes.len(), 13);
    for _ in 0..20 {
        a.undo();
    }
    assert_eq!(
        a.analytes.len(),
        3,
        "only 10 of the 12 additions can be undone"
    );
    for _ in 0..20 {
        a.redo();
    }
    assert_eq!(a.analytes.len(), 13);
}
