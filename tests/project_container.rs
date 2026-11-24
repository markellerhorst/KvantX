//! The project container with embedded raw files.

mod common;

use common::{sample_rec, tmp_dir};
use kvantx::formats::project::{self, BlobSource, ProjectFile};
use kvantx::support::sha256;

#[test]
fn project_container_embeds_raw_files_and_extracts_them_bit_exact() {
    let dir = tmp_dir("container");
    let big: Vec<u8> = (0..2_500_000u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    let small = b"tiny raw file".to_vec();
    let (a, b) = (dir.join("a.raw"), dir.join("b.raw"));
    std::fs::write(&a, &big).unwrap();
    std::fs::write(&b, &small).unwrap();
    let pf = ProjectFile {
        app_version: "t".into(),
        user: "u".into(),
        params: vec![],
        analytes: vec![],
        samples: vec![
            sample_rec(&a.to_string_lossy()),
            sample_rec("C:\\gone\\missing.raw"),
            sample_rec(&b.to_string_lossy()),
        ],
    };
    let out = dir.join("p.kvantx");
    let tmp = dir.join("tmp");
    let offs = project::write_container(
        &out,
        &pf,
        &[
            Some(BlobSource::File(a.clone())),
            None,
            Some(BlobSource::File(b.clone())),
        ],
        &tmp,
    )
    .unwrap();
    assert!(offs[1].is_none());

    let (back, offs2) = project::read_container(&out).unwrap();
    assert_eq!(offs, offs2);
    assert_eq!(
        back.samples.iter().map(|s| s.embedded).collect::<Vec<_>>(),
        vec![true, false, true]
    );

    let dest = dir.join("x.raw");
    let (o, l) = offs2[0].unwrap();
    let h = {
        let _ = std::fs::remove_file(&dest);
        project::extract_blob(&out, o, l, &dest).unwrap()
    };
    assert_eq!(std::fs::read(&dest).unwrap(), big);
    assert_eq!(h, sha256::sha256_hex(&big));
    let (o2, l2) = offs2[2].unwrap();
    {
        let _ = std::fs::remove_file(&dest);
        project::extract_blob(&out, o2, l2, &dest).unwrap()
    };
    assert_eq!(std::fs::read(&dest).unwrap(), small);

    // the original files vanish; saving again over the SAME project must keep the data (read from the old copy, written to a temp file)
    std::fs::remove_file(&a).unwrap();
    std::fs::remove_file(&b).unwrap();
    let blobs = vec![
        Some(BlobSource::Range {
            path: out.clone(),
            offset: o,
            len: l,
        }),
        None,
        Some(BlobSource::Range {
            path: out.clone(),
            offset: o2,
            len: l2,
        }),
    ];
    let offs3 = project::write_container(&out, &back, &blobs, &tmp).unwrap();
    let (_, offs4) = project::read_container(&out).unwrap();
    assert_eq!(offs3, offs4);
    let (o, l) = offs4[0].unwrap();
    {
        let _ = std::fs::remove_file(&dest);
        project::extract_blob(&out, o, l, &dest).unwrap()
    };
    assert_eq!(std::fs::read(&dest).unwrap(), big);
    assert_eq!(
        std::fs::read_dir(&tmp).map(|d| d.count()).unwrap_or(0),
        0,
        "the temporary file is moved away, nothing is left behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_text_only_projects_still_open() {
    let dir = tmp_dir("legacy");
    let pf = ProjectFile {
        samples: vec![sample_rec("x.raw")],
        ..Default::default()
    };
    let old = pf.to_text().replace("\ttrue\tfalse\n", "\ttrue\n"); // strip the 'embedded' column an old version did not write
    let p = dir.join("old.kvantx");
    std::fs::write(&p, old).unwrap();
    let (back, offs) = project::read_container(&p).unwrap();
    assert_eq!(back.samples.len(), 1);
    assert_eq!(offs, vec![None]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn truncated_container_is_rejected() {
    let dir = tmp_dir("trunc");
    let raw = dir.join("a.raw");
    std::fs::write(&raw, vec![7u8; 100_000]).unwrap();
    let pf = ProjectFile {
        samples: vec![sample_rec("a.raw")],
        ..Default::default()
    };
    let out = dir.join("p.kvantx");
    project::write_container(&out, &pf, &[Some(BlobSource::File(raw))], &dir.join("tmp")).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    std::fs::write(&out, &bytes[..bytes.len() - 5000]).unwrap();
    assert!(project::read_container(&out).is_err());
    std::fs::write(&out, b"LCMSQNT2\xff\xff\xff\xff\xff\xff\xff\x7f").unwrap();
    assert!(project::read_container(&out).is_err(), "absurd text length");
    assert!(
        project::write_container(&dir.join("q.kvantx"), &pf, &[], &dir.join("tmp")).is_err(),
        "blob list must match the samples"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_blob_reader_returns_the_embedded_bytes() {
    let dir = tmp_dir("blob");
    let raw = dir.join("a.mzML");
    std::fs::write(&raw, b"<mzML>demo</mzML>").unwrap();
    let pf = ProjectFile {
        samples: vec![sample_rec("a.mzML")],
        ..Default::default()
    };
    let out = dir.join("p.kvantx");
    let offs =
        project::write_container(&out, &pf, &[Some(BlobSource::File(raw))], &dir.join("tmp"))
            .unwrap();
    let (o, l) = offs[0].unwrap();
    assert_eq!(
        project::read_blob(&out, o, l).unwrap(),
        b"<mzML>demo</mzML>"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
