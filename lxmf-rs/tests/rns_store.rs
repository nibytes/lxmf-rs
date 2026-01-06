#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, FileMessageStore, MessageStore, QueuedMessage, RnsOutbound, RnsRouter};
use lxmf_rs::{LXMessage, Value};
use reticulum::identity::PrivateIdentity;

fn make_message() -> LXMessage {
    LXMessage::with_strings([0u8; 16], [0u8; 16], 1_700_000_000.0, "hi", "store", Value::Map(vec![]))
}

fn make_outbound() -> RnsOutbound {
    let source = PrivateIdentity::new_from_name("store-source");
    let destination = PrivateIdentity::new_from_name("store-dest").as_identity().clone();
    RnsOutbound::new(source, destination)
}

fn make_message_named(title: &str) -> LXMessage {
    LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        title,
        "store",
        Value::Map(vec![]),
    )
}

#[test]
fn file_store_restores_outbound_queue() {
    let root = std::env::temp_dir().join(format!("lxmf-store-{}", std::process::id()));
    let store = FileMessageStore::new(&root);

    {
        let outbound = make_outbound();
        let mut router = RnsRouter::with_store(outbound, Box::new(store));
        let msg = make_message();
        router.enqueue(msg, DeliveryMode::Direct);
        assert_eq!(router.pending_len(), 1);
    }

    let outbound = make_outbound();
    let store = FileMessageStore::new(&root);
    let router = RnsRouter::with_store(outbound, Box::new(store));
    assert_eq!(router.pending_len(), 1);
}

#[test]
fn restore_preserves_delay_when_immediate_exists() {
    let root = std::env::temp_dir().join(format!("lxmf-store-delay-{}", std::process::id()));
    let mut store = FileMessageStore::new(&root);

    let item_immediate = QueuedMessage {
        id: 1,
        message: make_message_named("immediate"),
        mode: DeliveryMode::Direct,
        attempts: 0,
        next_attempt_at_ms: 0,
    };
    let item_delayed = QueuedMessage {
        id: 2,
        message: make_message_named("delayed"),
        mode: DeliveryMode::Direct,
        attempts: 0,
        next_attempt_at_ms: 500,
    };

    store.save_outbound(&item_immediate);
    store.save_outbound(&item_delayed);

    let outbound = make_outbound();
    let store = FileMessageStore::new(&root);
    let mut router = RnsRouter::with_store(outbound, Box::new(store));

    let first = router.next_delivery_with_retry(0).unwrap().unwrap();
    assert_eq!(first.id, 1);

    let second = router.next_delivery_with_retry(0);
    assert!(second.is_none());

    let third = router.next_delivery_with_retry(500).unwrap().unwrap();
    assert_eq!(third.id, 2);
}
