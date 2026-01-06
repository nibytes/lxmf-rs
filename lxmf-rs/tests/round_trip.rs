use ed25519_dalek::SigningKey;
use lxmf_rs::{LXMessage, Value};
use sha2::{Digest, Sha256};

fn expected_message_id(destination: &[u8; 16], source: &[u8; 16], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(destination);
    hasher.update(source);
    hasher.update(payload);
    hasher.finalize().into()
}

#[test]
fn round_trip_sign_verify() {
    let dest = [0x11u8; 16];
    let src = [0x22u8; 16];
    let fields = Value::Map(vec![(Value::String("k".into()), Value::String("v".into()))]);

    let mut msg = LXMessage::with_strings(dest, src, 1_700_000_000.0, "hi", "hello", fields);
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);

    let bytes = msg.encode_signed(&signing_key).expect("encode");
    let decoded = LXMessage::decode(&bytes).expect("decode");

    assert!(decoded.verify(&signing_key.verifying_key()).expect("verify"));
    assert_eq!(decoded.title, b"hi");
    assert_eq!(decoded.content, b"hello");
}

#[test]
fn message_id_matches_payload() {
    let dest = [0x42u8; 16];
    let src = [0x24u8; 16];
    let fields = Value::Map(vec![]);
    let mut msg = LXMessage::with_strings(dest, src, 42.0, "t", "c", fields);
    let signing_key = SigningKey::from_bytes(&[3u8; 32]);

    let bytes = msg.encode_signed(&signing_key).expect("encode");
    let decoded = LXMessage::decode(&bytes).expect("decode");

    let payload = {
        use rmpv::Value;
        let payload_values = vec![
            Value::F64(decoded.timestamp),
            Value::Binary(decoded.title.clone()),
            Value::Binary(decoded.content.clone()),
            decoded.fields.clone(),
        ];
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &Value::Array(payload_values)).expect("pack");
        buf
    };

    let expected = expected_message_id(&decoded.destination_hash, &decoded.source_hash, &payload);
    assert_eq!(decoded.message_id.expect("message_id"), expected);
}
