//! The bounded undo and redo history.

use msquant::support::history::History;

#[test]
fn history_is_bounded_and_ordered() {
    let mut h: History<i32> = History::new(10);
    for i in 0..12 {
        h.record(i);
    }
    assert_eq!(h.lens(), (10, 0));
    assert_eq!(h.undo(100), Some(11));
    assert_eq!(h.undo(11), Some(10));
    assert_eq!(h.redo(10), Some(11));
    assert_eq!(h.redo(11), Some(100));
    assert_eq!(h.redo(100), None);
}

#[test]
fn history_new_action_clears_redo() {
    let mut h: History<i32> = History::new(10);
    h.record(1);
    h.record(2);
    assert_eq!(h.undo(3), Some(2));
    assert!(h.can_redo());
    h.record(4);
    assert!(!h.can_redo());
    assert_eq!(h.undo(5), Some(4));
    assert_eq!(h.undo(4), Some(1));
    assert_eq!(h.undo(1), None);
}
