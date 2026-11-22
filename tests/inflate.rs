//! The zlib decoder: fixed and stored blocks and the error cases.

use msquant::support::inflate;
use msquant::support::testkit;

#[test]
fn inflate_fixed_huffman_streams() {
    assert_eq!(
        inflate::zlib_decompress(&[0x78, 0x9c, 0x4b, 0x04, 0x00, 0x00, 0x62, 0x00, 0x62]).unwrap(),
        b"a"
    );
    let hw = [
        0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00,
        0x1a, 0x0b, 0x04, 0x5d,
    ];
    assert_eq!(inflate::zlib_decompress(&hw).unwrap(), b"hello world");
}

#[test]
fn inflate_stored_blocks_including_multi_block() {
    let data: Vec<u8> = (0..200_000u32).map(|i| (i % 253) as u8).collect();
    assert_eq!(
        inflate::zlib_decompress(&testkit::zlib_stored(&data)).unwrap(),
        data
    );
    assert!(inflate::zlib_decompress(&[0x78, 0x9c, 0xff]).is_err());
}
