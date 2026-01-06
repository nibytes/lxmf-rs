#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, DeliveredCache, LXMessage, RnsOutbound, RnsRouter, Value};
use reticulum::identity::PrivateIdentity;

#[test]
fn delivered_cache_eviction() {
    let mut cache = DeliveredCache::new(2);
    let id_a = [1u8; 32];
    let id_b = [2u8; 32];
    let id_c = [3u8; 32];
    assert!(cache.mark_delivered(id_a));
    assert!(cache.mark_delivered(id_b));
    assert_eq!(cache.len(), 2);
    assert!(cache.seen_before(&id_a));
    assert!(cache.mark_delivered(id_c));
    assert_eq!(cache.len(), 2);
    assert!(!cache.seen_before(&id_a));
    assert!(cache.seen_before(&id_c));
}

#[test]
fn receive_packet_filters_duplicate_message_id() {
    let source = PrivateIdentity::new_from_name("dup-source");
    let dest = PrivateIdentity::new_from_name("dup-dest").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let mut router = RnsRouter::new(RnsOutbound::new(
        PrivateIdentity::new_from_name("dup-source-router"),
        PrivateIdentity::new_from_name("dup-dest-router").as_identity().clone(),
    ));

    let mut msg = LXMessage::with_strings(
        [0x11u8; 16],
        [0x22u8; 16],
        1_700_000_000.0,
        "hi",
        "dup",
        Value::Map(vec![]),
    );

    let packet = match outbound.deliver(&mut msg, DeliveryMode::Direct).expect("deliver") {
        lxmf_rs::DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    router.receive_packet(&packet, DeliveryMode::Direct).expect("recv");
    router.receive_packet(&packet, DeliveryMode::Direct).expect("recv dup");
    assert_eq!(router.pop_inbound().is_some(), true);
    assert_eq!(router.pop_inbound().is_some(), false);
}
