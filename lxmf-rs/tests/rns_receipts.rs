#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, RnsOutbound, RnsRouter, Value, STATE_DELIVERED, STATE_FAILED};
use reticulum::identity::PrivateIdentity;

fn make_outbound() -> RnsOutbound {
    let source = PrivateIdentity::new_from_name("receipt-source");
    let destination = PrivateIdentity::new_from_name("receipt-dest").as_identity().clone();
    RnsOutbound::new(source, destination)
}

fn make_message() -> lxmf_rs::LXMessage {
    lxmf_rs::LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "receipt",
        Value::Map(vec![]),
    )
}

#[test]
fn inflight_mark_delivered_transitions_state() {
    let outbound = make_outbound();
    let mut router = RnsRouter::new(outbound);
    let msg = make_message();
    router.enqueue(msg, DeliveryMode::Direct);
    let delivery = router.next_delivery_with_retry(0).unwrap().unwrap();
    router.mark_inflight(&delivery, 1000, 5000);

    let delivered = router.mark_delivered(delivery.id).expect("delivered");
    assert_eq!(delivered.state, STATE_DELIVERED);
    assert_eq!(router.inflight_len(), 0);
}

#[test]
fn inflight_timeout_moves_to_failed() {
    let outbound = make_outbound();
    let mut router = RnsRouter::new(outbound);
    let msg = make_message();

    router.enqueue(msg, DeliveryMode::Direct);
    let delivery = router.next_delivery_with_retry(0).unwrap().unwrap();
    router.mark_inflight(&delivery, 1000, 10);

    let failed = router.poll_timeouts(2000);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].message.state, STATE_FAILED);
    assert_eq!(router.inflight_len(), 0);
}
