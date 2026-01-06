#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, MessageStore, NullEncryptor, RnsCryptoPolicy, RnsOutbound, RnsRouter};
use lxmf_rs::{FailedDelivery, LXMessage, QueuedMessage, Value};
use reticulum::identity::PrivateIdentity;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
struct StoreCounts {
    save_outbound: usize,
    remove_outbound: usize,
    save_inbound: usize,
    save_failed: usize,
}

struct CountingStore {
    counts: Arc<Mutex<StoreCounts>>,
}

impl CountingStore {
    fn new(counts: Arc<Mutex<StoreCounts>>) -> Self {
        Self { counts }
    }
}

impl MessageStore for CountingStore {
    fn save_outbound(&mut self, _item: &QueuedMessage) {
        self.counts.lock().unwrap().save_outbound += 1;
    }

    fn remove_outbound(&mut self, _id: u64) {
        self.counts.lock().unwrap().remove_outbound += 1;
    }

    fn load_outbound(&mut self) -> Vec<QueuedMessage> {
        Vec::new()
    }

    fn save_inbound(&mut self, _message: &LXMessage) {
        self.counts.lock().unwrap().save_inbound += 1;
    }

    fn save_failed(&mut self, _item: &FailedDelivery) {
        self.counts.lock().unwrap().save_failed += 1;
    }
}

fn make_message() -> LXMessage {
    LXMessage::with_strings([0u8; 16], [0u8; 16], 1_700_000_000.0, "hi", "store", Value::Map(vec![]))
}

#[test]
fn store_contract_success_path() {
    let counts = Arc::new(Mutex::new(StoreCounts::default()));
    let store = CountingStore::new(counts.clone());
    let source = PrivateIdentity::new_from_name("store-success-src");
    let destination = PrivateIdentity::new_from_name("store-success-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let mut router = RnsRouter::with_store(outbound, Box::new(store));

    router.enqueue(make_message(), DeliveryMode::Direct);
    assert_eq!(counts.lock().unwrap().save_outbound, 1);

    let result = router.next_delivery_with_retry(0).expect("queued");
    assert!(result.is_ok());
    assert_eq!(counts.lock().unwrap().remove_outbound, 1);
}

#[test]
fn store_contract_crypto_policy_failure() {
    let counts = Arc::new(Mutex::new(StoreCounts::default()));
    let store = CountingStore::new(counts.clone());
    let source = PrivateIdentity::new_from_name("store-fail-src");
    let destination = PrivateIdentity::new_from_name("store-fail-dst").as_identity().clone();
    let outbound = RnsOutbound::with_encryptor(source, destination, Box::new(NullEncryptor));
    let mut router = RnsRouter::with_store(outbound, Box::new(store));
    router.set_crypto_policy(RnsCryptoPolicy::EnforceRatchets);

    router.enqueue(make_message(), DeliveryMode::Direct);
    let result = router.next_delivery_with_retry(0).expect("queued");
    assert!(result.is_err());

    let counts = counts.lock().unwrap().clone();
    assert_eq!(counts.remove_outbound, 1);
    assert_eq!(counts.save_failed, 1);
}
