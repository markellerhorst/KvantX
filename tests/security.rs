//! Security: hostile input must lead to errors, not to panics, crashes or unbounded memory use.

mod common;

use common::tmp_dir;
use msquant::audit::{self, Audit};
use msquant::fmt;
use msquant::inflate;
use msquant::mzml;
use msquant::project::{self, BlobSource, ProjectFile};
use msquant::quant;
use msquant::rawconv;
use msquant::svg;
use msquant::testkit;
use std::path::Path;

/// Packs `(value, number of bits)` fields into bytes, least significant bit first (as deflate does).
fn pack_bits(fields: &[(u32, u32)]) -> Vec<u8> {
    let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0u32);
    for &(value, bits) in fields {
        acc |= (value as u64) << n;
        n += bits;
        while n >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            n -= 8;
        }
    }
    if n > 0 {
        out.push(acc as u8);
    }
    out
}

#[test]
fn decompression_bombs_are_stopped_by_the_size_limit() {
    let data = vec![7u8; 100_000];
    let z = testkit::zlib_stored(&data);
    assert_eq!(inflate::zlib_decompress_limited(&z, 100_000).unwrap(), data);
    assert!(inflate::zlib_decompress_limited(&z, 99_999).is_err());
    let one_a = [0x78, 0x9c, 0x4b, 0x04, 0x00, 0x00, 0x62, 0x00, 0x62];
    assert!(inflate::zlib_decompress_limited(&one_a, 0).is_err());
}

#[test]
fn an_over_subscribed_huffman_table_is_an_error_not_a_panic() {
    // dynamic block whose 19 code-length codes all have length 1: 19 codes do not fit in 1 bit
    let mut fields = vec![(1, 1), (2, 2), (0, 5), (0, 5), (15, 4)];
    fields.extend(std::iter::repeat((1, 3)).take(19));
    let mut stream = vec![0x78, 0x9c];
    stream.extend(pack_bits(&fields));
    stream.extend([0u8; 16]);
    let err = inflate::zlib_decompress(&stream).unwrap_err();
    assert!(err.contains("over-subscribed"), "{}", err);
}

#[test]
fn hostile_mzml_never_panics() {
    let invalid_utf8: &[u8] = b"\xff\xfe<mzML>\xc3\x28</mzML>";
    let cases: Vec<&[u8]> = vec![
        b"<mzML><spectrum",
        b"<mzML></spectrum></binary></mzML>",
        invalid_utf8,
        b"<spectrum id=\"scan=1\"><cvParam accession=\"MS:1000511\" value=\"2\"/><binaryDataArray><cvParam accession=\"MS:1000523\"/><cvParam accession=\"MS:1000514\"/><binary>!!!not base64!!!</binary></binaryDataArray></spectrum>",
        b"<spectrum id=\"scan=1\"><cvParam accession=\"MS:1000511\" value=\"2\"/><binaryDataArray><cvParam accession=\"MS:1000574\"/><cvParam accession=\"MS:1000514\"/><binary>AAAAAAAAAAAA</binary></binaryDataArray></spectrum>",
        b"<spectrum id=\"scan=99999999999999999999\"><cvParam accession=\"MS:1000016\" value=\"NaN\"/></spectrum>",
    ];
    for case in cases {
        assert!(
            mzml::parse_bytes("hostile", case).is_err(),
            "{}",
            String::from_utf8_lossy(case)
        );
    }
}

#[test]
fn alignment_refuses_absurd_or_non_finite_retention_times() {
    let normal: Vec<[f64; 2]> = (0..100)
        .map(|i| [i as f64 * 0.1, 1.0 + (i % 7) as f64])
        .collect();
    let huge: Vec<[f64; 2]> = (0..100).map(|i| [1e18 + i as f64, 1.0]).collect();
    let not_a_number = vec![[f64::NAN, 1.0]; 20];
    assert_eq!(quant::estimate_shift(&normal, &huge, 1.0), 0.0);
    assert_eq!(quant::estimate_shift(&normal, &not_a_number, 1.0), 0.0);
}

#[test]
fn a_container_with_wrapping_or_absurd_lengths_is_rejected() {
    let dir = tmp_dir("hostile_container");
    let path = dir.join("evil.quant");
    let mut wrap = b"LCMSQNT2".to_vec();
    wrap.extend((u64::MAX - 5).to_le_bytes()); // 16 + length wraps around in unchecked arithmetic
    std::fs::write(&path, &wrap).unwrap();
    assert!(project::read_container(&path).is_err());
    let mut big = b"LCMSQNT2".to_vec();
    big.extend((200u64 << 20).to_le_bytes());
    big.extend(vec![b'x'; 64]);
    std::fs::write(&path, &big).unwrap();
    assert!(
        project::read_container(&path).is_err(),
        "text part above the limit"
    );
    assert!(
        project::read_blob(&path, 0, u64::MAX).is_err(),
        "an impossible length is an error, not an abort"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_cannot_start_thousands_of_jobs() {
    let mut text = String::from("LCMSQUANT-PROJECT\t1\n");
    for i in 0..2001 {
        text += &format!("S\tfile{}.raw\t\tUnknown\t0\t0\ttrue\n", i);
    }
    assert!(ProjectFile::parse(&text).is_err());
    let mut targets = String::from("LCMSQUANT-PROJECT\t1\n");
    for i in 0..2001 {
        targets += &format!("A\tT{}\tn/a\t100\tn/a\n", i);
    }
    assert!(ProjectFile::parse(&targets).is_err());
}

#[test]
fn extracting_an_embedded_file_never_overwrites_an_existing_file() {
    let dir = tmp_dir("no_overwrite");
    let raw = dir.join("a.raw");
    std::fs::write(&raw, b"payload").unwrap();
    let pf = ProjectFile {
        samples: vec![common::sample_rec("a.raw")],
        ..Default::default()
    };
    let out = dir.join("p.quant");
    let offs =
        project::write_container(&out, &pf, &[Some(BlobSource::File(raw))], &dir.join("tmp"))
            .unwrap();
    let (offset, len) = offs[0].unwrap();
    let victim = dir.join("victim.txt");
    std::fs::write(&victim, b"precious").unwrap();
    assert!(project::extract_blob(&out, offset, len, &victim).is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn audit_fields_cannot_forge_further_entries() {
    let dir = tmp_dir("audit_forge");
    let path = dir.join("trail.log");
    let mut a = Audit::new(&path.to_string_lossy());
    a.user = "ev|il\nuser".into();
    a.log("ACT|ION\r", "line1\nline2|x|y");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert_eq!(
        text.matches('|').count(),
        4,
        "three field separators and one before the hash"
    );
    assert_eq!(a.bodies().len(), 1);
    assert!(audit::verify(&path.to_string_lossy()).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn text_in_figures_cannot_inject_markup() {
    let plot = svg::SvgPlot {
        title: "</text><script>alert(1)</script>\u{7}".into(),
        x_label: "x".into(),
        y_label: "y".into(),
        series: vec![svg::SvgSeries {
            name: "a\"><b>".into(),
            rgb: (0, 0, 0),
            width: 1.0,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            marker: false,
        }],
        vlines: vec![],
        labels: vec![],
        notes: vec!["<i>".into()],
        bounds: [0.0, 1.0, 0.0, 1.0],
        x_fmt: fmt::sci3,
        y_fmt: fmt::sci3,
    };
    let s = svg::render(&plot);
    assert!(!s.contains("<script>") && !s.contains("<b>") && !s.contains("<i>"));
    assert!(
        !s.contains('\u{7}'),
        "control characters are not valid in XML"
    );
}

#[test]
fn only_a_file_called_like_the_converter_is_ever_started() {
    let dir = tmp_dir("converter_names");
    let evil = dir.join("evil.exe");
    let good = dir.join("ThermoRawFileParser.exe");
    std::fs::write(&evil, b"").unwrap();
    std::fs::write(&good, b"").unwrap();
    assert_eq!(rawconv::find_trfp(&evil.to_string_lossy()), None);
    assert_eq!(
        rawconv::find_trfp(&good.to_string_lossy()),
        Some(good.clone())
    );
    assert_eq!(
        rawconv::find_trfp(&dir.join("missing.exe").to_string_lossy()),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// All `.rs` files of the library and the program.
fn source_files() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|e| e == "rs").unwrap_or(false) {
            files.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    files
}

#[test]
fn the_source_contains_no_unsafe_code() {
    for (name, text) in source_files() {
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            assert!(
                !line.replace("unsafe_code", "").contains("unsafe"),
                "{}:{}: {}",
                name,
                n + 1,
                line
            );
        }
    }
    let lib =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs")).unwrap();
    assert!(lib.contains("#![forbid(unsafe_code)]"));
}

#[test]
fn no_program_is_started_through_a_shell() {
    for (name, text) in source_files() {
        for pattern in [
            "Command::new(\"cmd",
            "Command::new(\"sh",
            "Command::new(\"powershell",
            "Command::new(\"bash",
            "\"/C\"",
        ] {
            assert!(!text.contains(pattern), "{} contains {}", name, pattern);
        }
    }
}
