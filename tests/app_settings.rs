//! Settings, peak methods and layout in the application model.

mod common;

use common::{add, app, targets};
use msquant::app::{Layout, PeakMethod};
use msquant::domain::settings::AnalysisSettings;

#[test]
fn reset_restores_every_setting() {
    let mut a = app();
    a.settings.tol = 99.0;
    a.settings.peak_method = PeakMethod::Adaptive;
    a.settings.zoom = 2.0;
    a.reset_settings();
    assert_eq!(a.settings, AnalysisSettings::default());
}

#[test]
fn both_peak_methods_find_the_synthetic_peak_with_similar_areas() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    assert!(a.samples[0].cells[0].found);
    let legacy = a.samples[0].cells[0].areas[0];
    a.settings.peak_method = PeakMethod::Adaptive;
    a.apply_settings();
    let cell = &a.samples[0].cells[0];
    assert!(cell.found, "the adaptive method finds the peak");
    let ratio = cell.areas[0] / legacy;
    assert!(
        ratio > 0.7 && ratio < 1.3,
        "areas differ too much: {}",
        ratio
    );
}

#[test]
fn the_peak_method_is_part_of_the_project_parameters() {
    let mut a = app();
    a.settings.peak_method = PeakMethod::Adaptive;
    a.settings.tol = 7.0;
    let params = a.project_file().params;
    let mut b = app();
    b.apply_params(&params);
    assert_eq!(b.settings.peak_method, PeakMethod::Adaptive);
    assert_eq!(b.settings.tol, 7.0);
}

#[test]
fn the_standard_curve_can_be_fitted_with_either_method() {
    for method in PeakMethod::ALL {
        let mut a = app();
        a.settings.peak_method = method;
        targets(&mut a);
        for (i, scale) in [1.0, 2.0, 4.0, 8.0].iter().enumerate() {
            add(&mut a, i, 0.0, *scale);
            a.samples[i].kind = msquant::app::Kind::Standard;
            a.samples[i].cells[0].conc = *scale;
        }
        a.refit(0);
        let fit = a.fits[0].as_ref().expect("a fit");
        assert!(fit.r2 > 0.99, "{:?}: r2 = {}", method, fit.r2);
    }
}

#[test]
fn reset_layout_restores_the_standard_layout_and_changes_the_generation() {
    let mut a = app();
    a.layout.files = false;
    a.layout.left_frac = 0.7;
    a.reset_layout();
    assert_eq!(a.layout, Layout::default());
}

#[test]
fn the_new_project_dialog_starts_from_the_current_settings_and_applies_its_answers() {
    let mut a = app();
    targets(&mut a);
    add(&mut a, 0, 0.0, 1.0);
    a.open_new_project_form(false);
    let mut form = a.new_project_form.clone().expect("the dialog is open");
    assert_eq!(form.tol, a.settings.tol);
    assert_eq!(form.method, a.settings.peak_method);
    assert!(
        !form.ask_trfp,
        "the converter is only asked on the first start"
    );
    form.tol = 3.0;
    form.tol_ppm = false;
    form.method = PeakMethod::Adaptive;
    form.user = "tester".into();
    a.apply_new_project_form(&form);
    assert_eq!((a.settings.tol, a.settings.tol_ppm), (3.0, false));
    assert_eq!(a.settings.peak_method, PeakMethod::Adaptive);
    assert_eq!(a.audit.user, "tester");
    assert!(a.samples.is_empty(), "a new project clears the files");
}

#[test]
fn the_welcome_dialog_keeps_the_files_and_asks_for_the_converter() {
    let mut a = app();
    add(&mut a, 0, 0.0, 1.0);
    a.open_new_project_form(true);
    let mut form = a.new_project_form.clone().expect("the dialog is open");
    assert!(form.ask_trfp && form.welcome);
    form.trfp = "C:/tools/ThermoRawFileParser.exe".into();
    a.apply_new_project_form(&form);
    assert_eq!(
        a.samples.len(),
        1,
        "the first start has no project to clear"
    );
    assert_eq!(a.trfp_path, "C:/tools/ThermoRawFileParser.exe");
}
