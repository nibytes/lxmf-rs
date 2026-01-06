use ed25519_dalek::SigningKey;
use lxmf_rs::{LXMessage, Value, COST_TICKET, TICKET_LENGTH};

fn make_message() -> LXMessage {
    LXMessage::with_strings(
        [0x11u8; 16],
        [0x22u8; 16],
        1_700_000_000.0,
        "hi",
        "hello",
        Value::Map(vec![]),
    )
}

#[test]
fn validate_stamp_with_ticket() {
    let mut msg = make_message();
    msg.set_outbound_ticket(vec![0x55u8; TICKET_LENGTH]);
    msg.set_defer_stamp(false);
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let bytes = msg.encode_signed(&signing_key).expect("encode");

    let mut decoded = LXMessage::decode(&bytes).expect("decode");
    let valid = decoded
        .validate_stamp(None, Some(&vec![0x55u8; TICKET_LENGTH]))
        .expect("validate");

    assert!(valid);
    assert_eq!(decoded.stamp_value, Some(COST_TICKET));
    assert!(decoded.stamp_valid);
}

#[test]
fn validate_stamp_with_pow() {
    let mut msg = make_message();
    msg.set_stamp_cost(8);
    msg.set_defer_stamp(false);
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let bytes = msg.encode_signed(&signing_key).expect("encode");

    let mut decoded = LXMessage::decode(&bytes).expect("decode");
    let valid = decoded.validate_stamp(Some(8), None).expect("validate");

    assert!(valid);
    assert!(decoded.stamp_value.unwrap_or(0) >= 8);
}
