//! The strict JSON reader and writer.

use kvantx::support::json::{self, Value};

fn ok(text: &str) -> Value {
    json::parse(text).unwrap_or_else(|e| panic!("{text:?} should parse: {e}"))
}

#[test]
fn valid_documents_parse() {
    assert_eq!(ok("null"), Value::Null);
    assert_eq!(ok(" true "), Value::Bool(true));
    assert_eq!(ok("-0.5e2"), Value::Number(-50.0));
    assert_eq!(
        ok("[1, 2.5, \"a\"]"),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.5),
            Value::String("a".into())
        ])
    );
    let v = ok("{\"a\": {\"b\": [true, null]}}");
    assert_eq!(
        v.get("a")
            .unwrap()
            .get("b")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(ok("\"\\u00e4\\n\\\"\"").as_str(), Some("ä\n\""));
    assert_eq!(
        ok("\"\\ud83d\\ude00\"").as_str(),
        Some("😀"),
        "surrogate pair"
    );
}

#[test]
fn invalid_documents_are_rejected() {
    for bad in [
        "",
        "{",
        "[1,]",
        "{\"a\":1,}",
        "01",
        "1.",
        ".5",
        "+1",
        "1e",
        "NaN",
        "Infinity",
        "-Infinity",
        "'a'",
        "{a:1}",
        "[1 2]",
        "{\"a\" 1}",
        "\"abc",
        "\"\\x\"",
        "\"\\u12\"",
        "tru",
        "nul",
        "[] []",
        "// comment\n1",
        "\"tab\there\"",
        "\"\\ud800\"",
        "\"\\udc00\"",
        "\"\\ud800\\u0041\"",
        "1e999",
        "-1e999",
    ] {
        assert!(json::parse(bad).is_err(), "{bad:?} must be rejected");
    }
}

#[test]
fn duplicate_keys_are_rejected() {
    assert!(json::parse("{\"a\":1,\"a\":2}").is_err());
}

#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    let deep = "[".repeat(100_000) + &"]".repeat(100_000);
    assert!(json::parse(&deep).is_err());
    let allowed = "[".repeat(json::MAX_DEPTH) + &"]".repeat(json::MAX_DEPTH);
    assert!(json::parse(&allowed).is_ok());
}

#[test]
#[allow(clippy::excessive_precision)]
fn numbers_round_trip_exactly() {
    for n in [
        0.0,
        1.0,
        -1.0,
        0.1,
        1.0 / 3.0,
        1e-7,
        1.5e-300,
        123456789.123456789,
        1e15,
        1e16,
        9.007199254740993e15,
        f64::MIN_POSITIVE,
        1.7976931348623157e308,
    ] {
        let text = Value::Number(n).to_pretty_string().unwrap();
        assert_eq!(ok(&text), Value::Number(n), "{n} -> {text}");
    }
}

#[test]
fn the_writer_refuses_non_finite_numbers() {
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(Value::Number(n).to_pretty_string().is_err());
    }
    let nested = json::object(vec![("a", Value::Array(vec![Value::Number(f64::NAN)]))]);
    assert!(nested.to_pretty_string().is_err());
}

#[test]
fn strings_with_special_characters_round_trip() {
    for s in [
        "",
        "plain",
        "quote\" backslash\\ slash/",
        "line\nbreak\ttab\r",
        "\u{1}\u{1f}",
        "ä ö ü µ × ¯ 😀",
        "C:\\data\\a b.raw",
    ] {
        let v = Value::String(s.into());
        assert_eq!(ok(&v.to_pretty_string().unwrap()), v, "{s:?}");
    }
}

#[test]
fn integers_are_recognised_only_when_exact() {
    assert_eq!(ok("5").as_u64(), Some(5));
    assert_eq!(ok("5.5").as_u64(), None);
    assert_eq!(ok("-1").as_u64(), None);
    assert_eq!(ok("1e300").as_u64(), None);
    assert_eq!(ok("9007199254740992").as_u64(), Some(9_007_199_254_740_992));
    assert_eq!(
        ok("9007199254740994").as_u64(),
        None,
        "above 2^53 the value is not exact"
    );
}

/// Deterministic pseudo-random documents survive writing and reading.
#[test]
fn random_documents_round_trip() {
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }
    fn build(rng: &mut Rng, depth: usize) -> Value {
        match rng.next() % if depth > 3 { 4 } else { 6 } {
            0 => Value::Null,
            1 => Value::Bool(rng.next() % 2 == 0),
            2 => Value::Number(((rng.next() % 2_000_001) as f64 - 1_000_000.0) / 997.0),
            3 => Value::String(format!("s{}\"\\\n{}", rng.next() % 1000, rng.next() % 7)),
            4 => Value::Array((0..rng.next() % 4).map(|_| build(rng, depth + 1)).collect()),
            _ => Value::Object(
                (0..rng.next() % 4)
                    .map(|i| (format!("k{i}"), build(rng, depth + 1)))
                    .collect(),
            ),
        }
    }
    let mut rng = Rng(0x9e3779b97f4a7c15);
    for _ in 0..300 {
        let v = build(&mut rng, 0);
        let text = v.to_pretty_string().unwrap();
        assert_eq!(ok(&text), v);
    }
}
