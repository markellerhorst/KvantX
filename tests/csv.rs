//! CSV output, including the protection against spreadsheet formulas.

use kvantx::formats::csv;

#[test]
fn plain_text_is_not_changed() {
    assert_eq!(csv::text_field("Morphine"), "Morphine");
    assert_eq!(csv::text_field("sample 1.raw"), "sample 1.raw");
}

#[test]
fn fields_with_separators_are_quoted() {
    assert_eq!(csv::text_field("a,b"), "\"a,b\"");
    assert_eq!(csv::text_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    assert_eq!(csv::text_field("two\nlines"), "\"two\nlines\"");
}

#[test]
fn spreadsheet_formulas_are_defused() {
    for evil in [
        "=1+1",
        "+1+1",
        "-2+3",
        "@SUM(A1)",
        "=cmd|' /C calc'!A0",
        "\t=1+1",
    ] {
        let field = csv::text_field(evil);
        assert!(
            field.starts_with('\'') || field.starts_with("\"'"),
            "{:?} -> {:?}",
            evil,
            field
        );
    }
}

#[test]
fn two_columns_writes_a_header_and_unchanged_numbers() {
    let text = csv::two_columns(
        "RT (min)",
        "Intensity",
        [(1.5, -2.0), (2.0, 1e-7)].into_iter(),
    );
    assert_eq!(text, "RT (min),Intensity\n1.5,-2\n2,0.0000001\n");
}
