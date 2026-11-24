//! Selecting files and setting their type in the application model.

mod common;

use common::{add, app, targets};
use kvantx::app::{App, Kind};

#[test]
fn explorer_style_selection() {
    let mut a = app();
    for i in 0..4 {
        add(&mut a, i, 0.0, 1.0);
    }
    a.apply_click(1, false, false);
    assert_eq!(a.shown(), vec![1]);
    a.apply_click(3, true, false);
    assert_eq!(a.shown(), vec![1, 3]);
    a.apply_click(0, false, true); // range from anchor(3) to 0
    assert_eq!(a.shown(), vec![0, 1, 2, 3]);
    a.apply_click(2, true, false); // toggle off
    assert_eq!(a.shown(), vec![0, 1, 3]);
    assert!(a.cur().is_some());
}

#[test]
fn no_selection_means_no_current_file() {
    let mut a = app();
    for i in 0..2 {
        add(&mut a, i, 0.0, 1.0);
    }
    for s in a.samples.iter_mut() {
        s.show = false;
    }
    a.fix_selection();
    assert!(a.cur().is_none());
    assert!(a.shown().is_empty());
}

#[test]
fn remove_file_keeps_disk_untouched_and_is_undoable() {
    let mut a = app();
    for i in 0..3 {
        add(&mut a, i, 0.0, 1.0);
    }
    a.remove_sample(1);
    assert_eq!(a.samples.len(), 2);
    assert_eq!(a.samples[1].data.name, "run2.raw");
    a.undo();
    assert_eq!(a.samples.len(), 3);
    assert_eq!(a.samples[1].data.name, "run1.raw");
    a.redo();
    assert_eq!(a.samples.len(), 2);
}

#[test]
fn changing_the_type_of_a_selected_file_changes_all_selected_files() {
    let mut a = app();
    for i in 0..5 {
        add(&mut a, i, 0.0, 1.0);
    }
    a.apply_click(1, false, false);
    a.apply_click(3, false, true); // files 1..=3 selected
    assert_eq!(a.shown(), vec![1, 2, 3]);
    a.set_type(2, Kind::Standard);
    let kinds: Vec<bool> = a.samples.iter().map(|s| s.kind == Kind::Standard).collect();
    assert_eq!(
        kinds,
        vec![false, true, true, true, false],
        "only the selected files change"
    );
    // a file outside the selection changes alone
    a.set_type(4, Kind::Blank);
    assert!(a.samples[4].kind == Kind::Blank && a.samples[1].kind == Kind::Standard);
    // one single selected file changes alone
    a.apply_click(0, false, false);
    a.set_type(0, Kind::QC);
    assert!(a.samples[0].kind == Kind::QC && a.samples[2].kind == Kind::Standard);
    // one undo step reverts the whole group
    a.set_type(0, Kind::Unknown);
    a.apply_click(1, false, false);
    a.apply_click(3, false, true);
    a.set_type(1, Kind::Unknown);
    assert!(a.samples[1..=3].iter().all(|s| s.kind == Kind::Unknown));
    a.undo();
    assert!(
        a.samples[1..=3].iter().all(|s| s.kind == Kind::Standard),
        "undo restores the whole group at once"
    );
}

#[test]
fn only_standards_are_listed_in_the_setup_matrix() {
    let mut a = app();
    for i in 0..3 {
        add(&mut a, i, 0.0, 1.0);
    }
    let listed = |a: &App| {
        (0..a.samples.len())
            .filter(|&s| a.samples[s].kind == Kind::Standard)
            .count()
    };
    assert_eq!(listed(&a), 0);
    a.apply_click(1, false, false);
    a.set_type(1, Kind::Standard);
    assert_eq!(listed(&a), 1);
}

#[test]
fn selected_files_are_sorted_alphabetically_in_the_slots_they_occupy() {
    let mut a = app();
    targets(&mut a);
    for n in [3, 1, 4, 2] {
        add(&mut a, n, 0.0, 1.0);
    }
    for (i, s) in a.samples.iter_mut().enumerate() {
        s.show = i != 2;
    }
    a.sel_s = 0;
    a.sort_selected();
    let names: Vec<String> = a.samples.iter().map(|s| s.data.name.clone()).collect();
    assert_eq!(
        names,
        ["run1.raw", "run2.raw", "run4.raw", "run3.raw"],
        "the unselected file stays in its slot"
    );
    assert_eq!(a.sel_s, 3, "the current file moves with its data");
    a.undo();
    let names: Vec<String> = a.samples.iter().map(|s| s.data.name.clone()).collect();
    assert_eq!(
        names,
        ["run3.raw", "run1.raw", "run4.raw", "run2.raw"],
        "sorting can be undone"
    );
}

#[test]
fn sorting_needs_at_least_two_selected_files_and_leaves_no_undo_step_otherwise() {
    let mut a = app();
    targets(&mut a);
    for n in [2, 1] {
        add(&mut a, n, 0.0, 1.0);
    }
    for (i, s) in a.samples.iter_mut().enumerate() {
        s.show = i == 0;
    }
    let before = a.history_lens();
    a.sort_selected();
    assert_eq!(a.samples[0].data.name, "run2.raw");
    assert_eq!(a.history_lens(), before);
}
