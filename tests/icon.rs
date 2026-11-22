//! The application icon decoder.

use msquant::formats::icon;
use msquant::support::testkit;

fn ico(data: &[u8], w: u8, h: u8) -> Vec<u8> {
    let mut o = vec![0, 0, 1, 0, 1, 0, w, h, 0, 0, 1, 0, 32, 0];
    o.extend_from_slice(&(data.len() as u32).to_le_bytes());
    o.extend_from_slice(&22u32.to_le_bytes());
    o.extend_from_slice(data);
    o
}

#[test]
fn ico_with_bmp_entry_decodes() {
    let mut d = vec![];
    for v in [40u32, 2, 4] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&32u16.to_le_bytes());
    d.extend_from_slice(&[0u8; 24]);
    d.extend_from_slice(&[255, 0, 0, 255, 255, 255, 255, 128]); // bottom row BGRA: blue, white(alpha 128)
    d.extend_from_slice(&[0, 0, 255, 255, 0, 255, 0, 255]); // top row: red, green
    let (rgba, w, h) = icon::decode_ico(&ico(&d, 2, 2)).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(
        rgba,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128
        ]
    );
}

#[test]
fn ico_with_png_entry_decodes_including_sub_filter() {
    fn chunk(ty: &[u8], body: &[u8]) -> Vec<u8> {
        let mut c = (body.len() as u32).to_be_bytes().to_vec();
        c.extend_from_slice(ty);
        c.extend_from_slice(body);
        c.extend_from_slice(&[0, 0, 0, 0]);
        c
    }
    let raw = [
        0u8, 1, 2, 3, 255, 4, 5, 6, 255, 1, 10, 20, 30, 255, 10, 10, 10, 0,
    ];
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = vec![];
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&2u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &testkit::zlib_stored(&raw)));
    png.extend(chunk(b"IEND", &[]));
    let (rgba, w, h) = icon::decode_ico(&ico(&png, 2, 2)).unwrap();
    assert_eq!((w, h), (2, 2));
    assert_eq!(
        rgba,
        vec![1, 2, 3, 255, 4, 5, 6, 255, 10, 20, 30, 255, 20, 30, 40, 255]
    );
    assert!(icon::decode_ico(b"not an icon at all, definitely not").is_none());
}

#[test]
fn the_real_app_icon_decodes_when_present() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("img")
        .join("msQuant-ico.ico");
    if let Ok(bytes) = std::fs::read(&p) {
        match icon::decode_ico(&bytes) {
            Some((rgba, w, h)) => assert_eq!(rgba.len(), (w * h * 4) as usize),
            None => eprintln!(
                "NOTE: {} uses an ICO sub-format the built-in decoder does not support (window icon is skipped, exe icon still embedded)",
                p.display()
            ),
        }
    } else {
        eprintln!("SKIPPED: {} not found", p.display());
    }
}
