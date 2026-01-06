#![cfg(feature = "rns")]

use lxmf_rs::{
    DeliveryMode, FileMessageStore, LxmfRouter, LXMessage, RnsOutbound, RnsRouter, RuntimeConfig,
    Value,
};
use reticulum::identity::PrivateIdentity;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("facade-source");
    let destination = PrivateIdentity::new_from_name("facade-dest").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::new(outbound);
    let config = RuntimeConfig {
        outbound_interval_ms: 0,
        inflight_interval_ms: 1000,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };
    LxmfRouter::with_config(router, config)
}

fn make_message() -> LXMessage {
    LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "facade",
        Value::Map(vec![]),
    )
}

#[test]
fn facade_enqueue_and_tick_drains_queue() {
    let mut router = make_router();
    router.enqueue(make_message(), DeliveryMode::Direct);
    assert_eq!(router.outbound_len(), 1);

    let tick = router.tick(0);
    assert_eq!(tick.deliveries.len(), 1);
    assert_eq!(router.outbound_len(), 0);
}

#[test]
fn facade_peer_rotation_uses_next_ready() {
    let mut router = make_router();
    let peer_a = router.upsert_peer([1u8; 16], 0);
    peer_a.next_sync_ms = 500;
    let peer_b = router.upsert_peer([2u8; 16], 0);
    peer_b.next_sync_ms = 200;

    let next = router.next_peer_ready(250).expect("peer");
    assert_eq!(next, [2u8; 16]);
}

#[test]
fn facade_pn_and_propagation_cleanup() {
    let mut router = make_router();
    let app_data = {
        let metadata = Value::Map(vec![]);
        let announce = Value::Array(vec![
            Value::Boolean(false),
            Value::Integer(1_700_000_000i64.into()),
            Value::Boolean(true),
            Value::Integer(64i64.into()),
            Value::Integer(128i64.into()),
            Value::Array(vec![
                Value::Integer(8i64.into()),
                Value::Integer(2i64.into()),
                Value::Integer(3i64.into()),
            ]),
            metadata,
        ]);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &announce).expect("msgpack");
        buf
    };
    let hash = [9u8; 16];
    router
        .pn_directory_mut()
        .upsert_announce(hash, &app_data, 100)
        .expect("announce");
    assert!(router.pn_directory().get(&hash).is_some());

    router.propagation_store_mut().insert(lxmf_rs::PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: b"old".to_vec(),
        timestamp: 1.0,
    });
    router.propagation_store_mut().insert(lxmf_rs::PropagationEntry {
        transient_id: [2u8; 32],
        lxm_data: b"new".to_vec(),
        timestamp: 4.5,
    });

    let removed = router.cleanup_propagation(5_000, 1_000);
    assert_eq!(removed, 1);
    assert!(router.propagation_store().has(&[2u8; 32]));
}

#[test]
fn facade_peer_sync_offer_uses_store_ids() {
    let mut router = make_router();
    router.propagation_store_mut().insert(lxmf_rs::PropagationEntry {
        transient_id: [3u8; 32],
        lxm_data: b"a".to_vec(),
        timestamp: 2.0,
    });
    let offer = router.peer_sync_offer([7u8; 16], b"key".to_vec(), 0);
    match offer {
        lxmf_rs::PeerSyncRequest::Offer { available, .. } => {
            assert_eq!(available, vec![[3u8; 32]]);
        }
        _ => panic!("expected offer"),
    }
}

#[test]
fn facade_restores_outbound_from_store() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "lxmf-facade-store-{}-{}",
        std::process::id(),
        stamp
    ));
    let store = FileMessageStore::new(&root);

    let source = PrivateIdentity::new_from_name("facade-store-source");
    let destination = PrivateIdentity::new_from_name("facade-store-dest").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);

    let config = RuntimeConfig {
        outbound_interval_ms: 1000,
        inflight_interval_ms: 1000,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };

    let mut router = LxmfRouter::with_store(outbound, Box::new(store), config);
    router.enqueue(make_message(), DeliveryMode::Direct);
    assert_eq!(router.outbound_len(), 1);

    let source = PrivateIdentity::new_from_name("facade-store-source");
    let destination = PrivateIdentity::new_from_name("facade-store-dest").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let store = FileMessageStore::new(&root);
    let router = LxmfRouter::with_store(outbound, Box::new(store), config);
    assert_eq!(router.outbound_len(), 1);
}
