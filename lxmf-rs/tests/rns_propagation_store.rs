#![cfg(feature = "rns")]

use lxmf_rs::{PropagationEntry, PropagationStore};

#[test]
fn propagation_store_cleanup_empty() {
    let mut store = PropagationStore::new();
    let removed = store.remove_older_than(1_000, 500);
    assert_eq!(removed, 0);
}

#[test]
fn propagation_store_cleanup_all_stale() {
    let mut store = PropagationStore::new();
    store.insert(PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: b"a".to_vec(),
        timestamp: 1.0,
    });
    store.insert(PropagationEntry {
        transient_id: [2u8; 32],
        lxm_data: b"b".to_vec(),
        timestamp: 2.0,
    });

    let removed = store.remove_older_than(5_000, 1_000);
    assert_eq!(removed, 2);
    assert!(store.list_ids().is_empty());
}

#[test]
fn propagation_store_cleanup_partial() {
    let mut store = PropagationStore::new();
    store.insert(PropagationEntry {
        transient_id: [3u8; 32],
        lxm_data: b"old".to_vec(),
        timestamp: 4.0,
    });
    store.insert(PropagationEntry {
        transient_id: [4u8; 32],
        lxm_data: b"new".to_vec(),
        timestamp: 4.5,
    });

    let removed = store.remove_older_than(5_000, 1_000);
    assert_eq!(removed, 1);
    assert!(!store.has(&[3u8; 32]));
    assert!(store.has(&[4u8; 32]));
}
