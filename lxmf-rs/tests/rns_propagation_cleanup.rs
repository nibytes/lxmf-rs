#![cfg(feature = "rns")]

use lxmf_rs::{
    FileMessageStore, LxmfRouter, PropagationEntry, RnsOutbound, RnsRouter, RuntimeConfig,
    MESSAGE_EXPIRY,
};
use reticulum::identity::PrivateIdentity;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_router(config: RuntimeConfig) -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("prop-clean-src");
    let destination = PrivateIdentity::new_from_name("prop-clean-dst")
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::new(outbound);
    LxmfRouter::with_config(router, config)
}

fn lxm_data(dest: [u8; 16], payload_len: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(16 + payload_len);
    data.extend_from_slice(&dest);
    data.extend(std::iter::repeat(0u8).take(payload_len));
    data
}

#[test]
fn propagation_cleanup_expiry_and_weighted_eviction() {
    let config = RuntimeConfig {
        outbound_interval_ms: 1000,
        inflight_interval_ms: 1000,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };
    let mut router = make_router(config);
    router.set_message_storage_limit(Some(1), None, None);

    let now_ms = 1_700_000_000_000u64;
    let now_s = now_ms as f64 / 1000.0;

    let priority_dest = [0xAAu8; 16];
    router.prioritise_destination(priority_dest);

    let expired_entry = PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: lxm_data([0xCCu8; 16], 884),
        timestamp: now_s - MESSAGE_EXPIRY as f64 - 10.0,
    };
    let prioritized_entry = PropagationEntry {
        transient_id: [2u8; 32],
        lxm_data: lxm_data(priority_dest, 884),
        timestamp: now_s,
    };
    let unprioritized_entry = PropagationEntry {
        transient_id: [3u8; 32],
        lxm_data: lxm_data([0xBBu8; 16], 884),
        timestamp: now_s,
    };

    router
        .propagation_store_mut()
        .insert(expired_entry.clone());
    router
        .propagation_store_mut()
        .insert(prioritized_entry.clone());
    router
        .propagation_store_mut()
        .insert(unprioritized_entry.clone());

    let removed = router.clean_propagation_store(now_ms);
    assert!(removed >= 2);
    assert!(!router.propagation_store().has(&expired_entry.transient_id));
    assert!(router.propagation_store().has(&prioritized_entry.transient_id));
    assert!(!router
        .propagation_store()
        .has(&unprioritized_entry.transient_id));
}

#[test]
fn propagation_cleanup_runs_on_tick_interval() {
    let config = RuntimeConfig {
        outbound_interval_ms: 1000,
        inflight_interval_ms: 1000,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1,
    };
    let mut router = make_router(config);

    let now_ms = (MESSAGE_EXPIRY + 10) * 1000;
    let now_s = now_ms as f64 / 1000.0;
    let expired_entry = PropagationEntry {
        transient_id: [9u8; 32],
        lxm_data: lxm_data([0xDDu8; 16], 32),
        timestamp: now_s - MESSAGE_EXPIRY as f64 - 1.0,
    };
    router.propagation_store_mut().insert(expired_entry.clone());

    // JOB_STORE_INTERVAL = 120, so we need to call tick() 120 times
    // to get processing_count = 120, which is divisible by 120
    let mut tick = router.tick(now_ms);
    for i in 1..120 {
        tick = router.tick(now_ms + i);
    }
    // Now processing_count = 120, so store_ran should be true
    assert!(tick.store_ran);
    assert!(!router.propagation_store().has(&expired_entry.transient_id));
}

#[test]
fn propagation_cleanup_runs_on_startup() {
    let root = std::env::temp_dir().join(format!("lxmf-prop-clean-start-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs_f64();

    let expired_entry = PropagationEntry {
        transient_id: [4u8; 32],
        lxm_data: lxm_data([0xEEu8; 16], 32),
        timestamp: now_s - MESSAGE_EXPIRY as f64 - 10.0,
    };
    store.save_propagation_entry(&expired_entry);

    let source = PrivateIdentity::new_from_name("prop-clean-start-src");
    let destination = PrivateIdentity::new_from_name("prop-clean-start-dst")
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let config = RuntimeConfig::default();
    let router = LxmfRouter::with_store_and_root(outbound, Box::new(store), &root, config);

    assert!(!router.propagation_store().has(&expired_entry.transient_id));
    let reloaded = FileMessageStore::new(&root).load_propagation_store();
    assert!(!reloaded.has(&expired_entry.transient_id));
}

#[test]
fn propagation_cleanup_removes_files_with_trailing_zero_timestamp() {
    let root = std::env::temp_dir().join(format!("lxmf-prop-clean-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs_f64();
    let timestamp = now_s.floor() + 0.1;
    let timestamp_str = format!("{:.6}", timestamp);

    let transient_id = [0x42u8; 32];
    let stamp_value = 0u32;
    let filename = format!("{}_{}_{}", hex::encode(transient_id), timestamp_str, stamp_value);
    let prop_dir = root.join("propagation");
    let _ = std::fs::create_dir_all(&prop_dir);
    let path = prop_dir.join(&filename);
    let _ = std::fs::write(&path, vec![0x01, 0x02, 0x03]);

    let source = PrivateIdentity::new_from_name("prop-clean-file-src");
    let destination = PrivateIdentity::new_from_name("prop-clean-file-dst")
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let config = RuntimeConfig::default();
    let mut router = LxmfRouter::with_store_and_root(outbound, Box::new(store), &root, config);

    assert!(router.propagation_store().has(&transient_id));
    assert!(path.exists());

    let now_ms = ((timestamp + MESSAGE_EXPIRY as f64 + 10.0) * 1000.0) as u64;
    router.clean_propagation_store(now_ms);

    assert!(!router.propagation_store().has(&transient_id));
    assert!(!path.exists());
}
