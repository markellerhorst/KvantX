//! Number formatting: scientific notation, retention times and m/z values.

use msquant::support::fmt;

#[test]
fn scientific_notation_display() {
    assert_eq!(fmt::sci(100.0), "10²");
    assert_eq!(fmt::sci(1000.0), "10³");
    assert_eq!(fmt::sci(1.0), "10⁰");
    assert_eq!(fmt::sci(1234.5), "1.2345×10³");
    assert_eq!(fmt::sci(0.05), "5×10¯²");
    assert_eq!(fmt::sci(-250.0), "-2.5×10²");
    assert_eq!(fmt::sci(0.0), "0");
}

#[test]
fn scientific_notation_round_trips_through_the_parser() {
    for v in [100.0, 1000.0, 1234.5, 0.05, -250.0, 1e-7, 123456.0, 3.0] {
        let back =
            fmt::parse_sci(&fmt::sci(v)).unwrap_or_else(|| panic!("cannot parse {}", fmt::sci(v)));
        assert!(
            (back - v).abs() <= v.abs() * 1e-5,
            "{} -> {} -> {}",
            v,
            fmt::sci(v),
            back
        );
    }
    assert_eq!(fmt::parse_sci("1.5e3"), Some(1500.0));
    assert_eq!(fmt::parse_sci("2,5"), Some(2.5));
    assert!((fmt::parse_sci("5x10^-2").unwrap() - 0.05).abs() < 1e-12);
    assert_eq!(fmt::parse_sci("garbage"), None);
}

#[test]
fn retention_times_use_two_decimals_and_mz_keeps_its_digits() {
    assert_eq!(fmt::rt2(7.2), "7.20");
    assert_eq!(fmt::rt2(0.0), "0.00");
    assert_eq!(fmt::rt2(12.3456), "12.35");
    assert_eq!(fmt::sci_prec(286.1438, 9), "2.861438×10²");
    assert_eq!(fmt::sci_prec(150.0, 9), "1.5×10²");
    assert_eq!(fmt::mz4(286.14384), "286.1438");
    assert_eq!(fmt::mz4(150.0), "150.0000");
    assert_eq!(
        fmt::mz4(1234.56789),
        "1234.5679",
        "plain rounding, never scientific notation"
    );
    assert_eq!(fmt::sci3(1234567.0), "1.23×10⁶", "compact axis ticks");
    assert_eq!(fmt::pct0(40.0), "40%");
    assert_eq!(fmt::pct0(100.0), "100%");
}
