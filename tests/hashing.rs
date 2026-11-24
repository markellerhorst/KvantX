//! SHA-256: known test vectors and the incremental interface.

use kvantx::support::sha256::{self, Sha256};

#[test]
fn sha256_known_vectors() {
    assert_eq!(
        sha256::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256::sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn sha256_streaming_matches_one_shot() {
    let mut h = Sha256::new();
    let chunk = vec![b'a'; 1000];
    for _ in 0..1000 {
        h.update(&chunk);
    }
    assert_eq!(
        h.finish_hex(),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    let data: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 251) as u8).collect();
    let mut h = Sha256::new();
    for c in data.chunks(37) {
        h.update(c);
    }
    assert_eq!(h.finish_hex(), sha256::sha256_hex(&data));
}
