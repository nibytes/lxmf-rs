use lxmf_rs::{LXMessage, Value};
use ed25519_dalek::SigningKey;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn unpack_from_file_round_trip() {
    let mut msg = LXMessage::with_strings(
        [0x11u8; 16],
        [0x22u8; 16],
        1_700_000_000.0,
        "title",
        "body",
        Value::Map(vec![]),
    );
    msg.state = 7;
    msg.method = Some(2);
    msg.transport_encrypted = Some(true);
    msg.transport_encryption = Some("test".to_string());

    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let _ = msg.encode_signed(&signing_key).expect("sign");
    let container = msg.packed_container().expect("container");

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("lxmf-container-{stamp}.msgpack"));
    std::fs::write(&path, container).expect("write");

    let unpacked = LXMessage::unpack_from_file(&path).expect("unpack");
    assert_eq!(unpacked.state, 7);
    assert_eq!(unpacked.method, Some(2));
    assert_eq!(unpacked.transport_encrypted, Some(true));
    assert_eq!(unpacked.transport_encryption.as_deref(), Some("test"));
    assert_eq!(unpacked.title, b"title");
    assert_eq!(unpacked.content, b"body");
}
