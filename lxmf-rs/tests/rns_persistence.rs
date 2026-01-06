#![cfg(feature = "rns")]

use ed25519_dalek::SigningKey;
use lxmf_rs::{
    FileMessageStore, LxmfRouter, LXMessage, NodeStats, OutboundStampCosts, PnDirectory,
    PropagationEntry, RuntimeConfig, RnsOutbound, Value, DESTINATION_LENGTH, PN_META_NAME,
};
use std::fs;
use reticulum::identity::PrivateIdentity;
use sha2::{Digest, Sha256};

fn build_announce_data(name: &str, stamp_cost: i64) -> Vec<u8> {
    let metadata = Value::Map(vec![(
        Value::Integer((PN_META_NAME as i64).into()),
        Value::Binary(name.as_bytes().to_vec()),
    )]);

    let announce = Value::Array(vec![
        Value::Boolean(false),
        Value::Integer(1_700_000_000i64.into()),
        Value::Boolean(true),
        Value::Integer(64i64.into()),
        Value::Integer(128i64.into()),
        Value::Array(vec![
            Value::Integer(stamp_cost.into()),
            Value::Integer(2i64.into()),
            Value::Integer(3i64.into()),
        ]),
        metadata,
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &announce).expect("msgpack");
    buf
}

#[test]
fn propagation_store_round_trip() {
    let root = std::env::temp_dir().join(format!("lxmf-prop-store-{}", std::process::id()));
    let store = FileMessageStore::new(&root);

    let mut msg = LXMessage::with_strings(
        [0x10u8; 16],
        [0x11u8; 16],
        1_700_000_000.0,
        "hi",
        "prop",
        Value::Map(vec![]),
    );
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let (transient_id, stamp, stamp_value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");
    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let entry = PropagationEntry {
        transient_id,
        lxm_data: lxm_data.clone(),
        timestamp: 1_700_000_000.0,
    };

    store.save_propagation_entry(&entry);

    let prop_dir = root.join("propagation");
    let filenames: Vec<String> = fs::read_dir(&prop_dir)
        .expect("dir")
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(|s| s.to_string()))
        .collect();
    assert_eq!(filenames.len(), 1);
    let parts: Vec<&str> = filenames[0].split('_').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], hex::encode(transient_id));
    let ts: f64 = parts[1].parse().expect("timestamp");
    assert!((ts - entry.timestamp).abs() < 0.0001);
    let sv: u32 = parts[2].parse().expect("stamp value");
    assert_eq!(sv, stamp_value);

    let loaded = store.load_propagation_store();
    assert!(loaded.has(&entry.transient_id));
    let loaded_entry = loaded.get(&entry.transient_id).expect("entry");
    assert_eq!(loaded_entry.lxm_data, entry.lxm_data);
}

#[test]
fn propagation_store_purges_invalid_filenames() {
    let root = std::env::temp_dir().join(format!("lxmf-prop-invalid-{}", std::process::id()));
    let store = FileMessageStore::new(&root);
    let prop_dir = root.join("propagation");
    let _ = std::fs::create_dir_all(&prop_dir);

    let bad_path = prop_dir.join("badname");
    std::fs::write(&bad_path, b"data").expect("write bad file");

    let mut msg = LXMessage::with_strings(
        [0x21u8; 16],
        [0x22u8; 16],
        1_700_000_000.0,
        "hi",
        "prop",
        Value::Map(vec![]),
    );
    let signing_key = SigningKey::from_bytes(&[8u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let (transient_id, stamp, stamp_value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");
    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let filename = format!(
        "{}_{}_{}",
        hex::encode(transient_id),
        1_700_000_000.0,
        stamp_value + 1
    );
    let bad_stamp_path = prop_dir.join(filename);
    std::fs::write(&bad_stamp_path, &lxm_data).expect("write bad stamp file");

    let loaded = store.load_propagation_store();
    assert!(!loaded.has(&transient_id));
    assert!(std::fs::read_dir(&prop_dir).expect("dir").next().is_none());
}

#[test]
fn propagation_resource_payload_persists_entries() {
    let root = std::env::temp_dir().join(format!("lxmf-prop-resource-{}", std::process::id()));
    let store = FileMessageStore::new(&root);

    let source = PrivateIdentity::new_from_name("prop-resource-src");
    let destination = PrivateIdentity::new_from_name("prop-resource-dst")
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let config = RuntimeConfig::default();
    let mut router = LxmfRouter::with_store_and_root(outbound, Box::new(store), &root, config);
    router.clear_propagation_validation();
    router.clear_peering_validation();

    let now_ms = 1_700_000_000_000u64;
    let msg1 = vec![0x01u8; 33];
    let msg2 = vec![0x02u8; 34];
    let payload = Value::Array(vec![
        Value::F64(now_ms as f64 / 1000.0),
        Value::Array(vec![Value::Binary(msg1.clone()), Value::Binary(msg2.clone())]),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &payload).expect("msgpack");

    let accepted = router
        .handle_propagation_resource_payload(Some([0x11u8; 16]), None, None, &buf, now_ms)
        .expect("accepted");
    assert_eq!(accepted.len(), 2);

    let store = FileMessageStore::new(&root);
    let loaded = store.load_propagation_store();
    let id1: [u8; 32] = Sha256::new().chain_update(&msg1).finalize().into();
    let id2: [u8; 32] = Sha256::new().chain_update(&msg2).finalize().into();
    assert!(loaded.has(&id1));
    assert!(loaded.has(&id2));
}

#[test]
fn propagation_resource_ingress_persists_entries() {
    let root = std::env::temp_dir().join(format!("lxmf-prop-ingress-{}", std::process::id()));
    let store = FileMessageStore::new(&root);

    let source = PrivateIdentity::new_from_name("prop-ingress-src");
    let destination = PrivateIdentity::new_from_name("prop-ingress-dst")
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let config = RuntimeConfig::default();
    let mut router = LxmfRouter::with_store_and_root(outbound, Box::new(store), &root, config);
    router.clear_propagation_validation();

    let now_ms = 1_700_000_000_000u64;
    let msg = vec![0x09u8; 40];
    let payload = Value::Array(vec![
        Value::F64(now_ms as f64 / 1000.0),
        Value::Array(vec![Value::Binary(msg.clone())]),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &payload).expect("msgpack");

    router.receive_resource_payload(buf, None);
    let _ = router.process_inbound_queue(now_ms, false);

    let store = FileMessageStore::new(&root);
    let loaded = store.load_propagation_store();
    let id: [u8; 32] = Sha256::new().chain_update(&msg).finalize().into();
    assert!(loaded.has(&id));
}

#[test]
fn pn_directory_round_trip() {
    let root = std::env::temp_dir().join(format!("lxmf-pn-store-{}", std::process::id()));
    let store = FileMessageStore::new(&root);

    let mut dir = PnDirectory::new();
    let app_data = build_announce_data("pn", 9);
    let hash = [1u8; 16];
    dir.upsert_announce(hash, &app_data, 123).expect("announce");
    dir.acknowledge(&hash);

    store.save_pn_directory(&dir);
    let loaded = store.load_pn_directory();
    let entry = loaded.get(&hash).expect("entry");
    assert!(entry.acked);
    assert_eq!(entry.app_data, app_data);
}

#[test]
fn ticket_store_round_trip() {
    let root = std::env::temp_dir().join(format!("lxmf-ticket-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let source = PrivateIdentity::new_from_name("ticket-store-src");
    let dest = PrivateIdentity::new_from_name("ticket-store-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let mut router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(store),
        &root,
        RuntimeConfig::default(),
    );

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis() as u64;
    let expires_at_s = (now_ms as f64 / 1000.0) + 3600.0;
    let ticket = vec![7u8; 16];
    let destination_hash = [9u8; 16];

    router.remember_ticket(destination_hash, expires_at_s, ticket.clone());
    router.mark_ticket_delivered(destination_hash, now_ms);

    let source = PrivateIdentity::new_from_name("ticket-store-src2");
    let dest = PrivateIdentity::new_from_name("ticket-store-dst2").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let store = FileMessageStore::new(&root);
    let router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(store),
        &root,
        RuntimeConfig::default(),
    );

    let loaded = router
        .get_outbound_ticket(&destination_hash, now_ms)
        .expect("ticket");
    assert_eq!(loaded, ticket);
}

#[test]
fn outbound_stamp_costs_persist_and_cleanup() {
    let root = std::env::temp_dir().join(format!("lxmf-stamp-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let now_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_secs_f64();
    let destination_hash = [4u8; 16];

    let mut costs = OutboundStampCosts::default();
    costs
        .entries
        .insert(destination_hash, (now_s - (46.0 * 24.0 * 60.0 * 60.0), 12));
    store.save_outbound_stamp_costs(&costs);

    let source = PrivateIdentity::new_from_name("stamp-store-src");
    let dest = PrivateIdentity::new_from_name("stamp-store-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(store),
        &root,
        RuntimeConfig::default(),
    );

    assert_eq!(router.get_outbound_stamp_cost(&destination_hash), None);

    let store = FileMessageStore::new(&root);
    let reloaded = store.load_outbound_stamp_costs();
    assert!(!reloaded.entries.contains_key(&destination_hash));
}

#[test]
fn node_stats_round_trip() {
    let root = std::env::temp_dir().join(format!("lxmf-node-stats-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let stats = NodeStats {
        client_propagation_messages_received: 7,
        client_propagation_messages_served: 5,
        unpeered_propagation_incoming: 3,
        unpeered_propagation_rx_bytes: 1024,
    };
    store.save_node_stats(&stats);

    let loaded = store.load_node_stats();
    assert_eq!(loaded, stats);
}

#[test]
fn router_loads_node_stats_from_store() {
    let root = std::env::temp_dir().join(format!("lxmf-node-router-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);
    let stats = NodeStats {
        client_propagation_messages_received: 2,
        client_propagation_messages_served: 4,
        unpeered_propagation_incoming: 6,
        unpeered_propagation_rx_bytes: 2048,
    };
    store.save_node_stats(&stats);

    let source = PrivateIdentity::new_from_name("node-stats-src");
    let dest = PrivateIdentity::new_from_name("node-stats-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(FileMessageStore::new(&root)),
        &root,
        RuntimeConfig::default(),
    );

    assert_eq!(router.node_stats(), &stats);
}

#[test]
fn local_caches_round_trip_and_cleanup() {
    let root = std::env::temp_dir().join(format!("lxmf-local-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let mut local_deliveries = std::collections::HashMap::new();
    local_deliveries.insert([1u8; 32], 1_700_000_000.0);
    let mut locally_processed = std::collections::HashMap::new();
    locally_processed.insert([2u8; 32], 1_700_000_000.0);

    store.save_local_deliveries(&local_deliveries);
    store.save_locally_processed(&locally_processed);

    let manual_path = root.join("local_deliveries");
    let manual_value = Value::Map(vec![(
        Value::Binary([3u8; 32].to_vec()),
        Value::Integer(1_600_000_000i64.into()),
    )]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &manual_value).expect("msgpack");
    std::fs::write(&manual_path, buf).expect("write manual cache");

    let loaded_deliveries = store.load_local_deliveries();
    assert!(loaded_deliveries.contains_key(&[3u8; 32]));
    let loaded_processed = store.load_locally_processed();
    assert!(loaded_processed.contains_key(&[2u8; 32]));

    let source = PrivateIdentity::new_from_name("local-cache-src");
    let dest = PrivateIdentity::new_from_name("local-cache-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(FileMessageStore::new(&root)),
        &root,
        RuntimeConfig::default(),
    );

    assert_eq!(router.local_deliveries_len(), 0);
    assert_eq!(router.locally_processed_len(), 0);
}

#[test]
fn inbound_duplicate_suppressed_by_persisted_local_deliveries() {
    let root = std::env::temp_dir().join(format!("lxmf-local-deliveries-dupe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis() as u64;
    let mut message = LXMessage::with_strings(
        [1u8; 16],
        [2u8; 16],
        1_700_000_000.0,
        "dupe",
        "payload",
        Value::Map(vec![]),
    );
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let _ = message.encode_signed(&signing_key).expect("signed");
    let message_id = message.message_id.expect("message id");

    let mut local_deliveries = std::collections::HashMap::new();
    local_deliveries.insert(message_id, (now_ms as f64) / 1000.0);
    store.save_local_deliveries(&local_deliveries);

    let source = PrivateIdentity::new_from_name("dedupe-src");
    let dest = PrivateIdentity::new_from_name("dedupe-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let mut router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(FileMessageStore::new(&root)),
        &root,
        RuntimeConfig::default(),
    );

    let accepted = router.process_inbound_message(message, now_ms, false, false);
    assert!(accepted.is_none());
}

#[test]
fn propagation_duplicate_suppressed_by_persisted_locally_processed() {
    let root = std::env::temp_dir().join(format!("lxmf-local-processed-dupe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = FileMessageStore::new(&root);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis() as u64;
    let lxm_data = vec![0x55u8; 48];
    let transient_id: [u8; 32] = Sha256::digest(&lxm_data).into();

    let mut locally_processed = std::collections::HashMap::new();
    locally_processed.insert(transient_id, (now_ms as f64) / 1000.0);
    store.save_locally_processed(&locally_processed);

    let source = PrivateIdentity::new_from_name("processed-src");
    let dest = PrivateIdentity::new_from_name("processed-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let mut router = LxmfRouter::with_store_and_root(
        outbound,
        Box::new(FileMessageStore::new(&root)),
        &root,
        RuntimeConfig::default(),
    );
    router.clear_propagation_validation();

    let result = router.process_propagated(1_700_000_000.0, lxm_data, now_ms, None, true);
    assert!(result.is_none());
    assert!(!router.propagation_store().has(&transient_id));
    assert_eq!(router.locally_processed_len(), 1);
}
