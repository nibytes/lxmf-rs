#![cfg(feature = "rns")]

use ed25519_dalek::SigningKey;
use lxmf_rs::{
    validate_peering_key, validate_pn_stamp, LxmfRouter, PeerSyncRequest, PeerSyncResponse,
    PropagationEntry, RnsOutbound, RnsRouter, RuntimeConfig, Value, LXMessage,
};
use rand_core::RngCore;
use reticulum::identity::PrivateIdentity;
use rmpv::Value as RmpValue;

fn make_router(name: &str) -> LxmfRouter {
    let source = PrivateIdentity::new_from_name(name);
    let destination = PrivateIdentity::new_from_name(&format!("{name}-dest"))
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::new(outbound);
    LxmfRouter::with_config(router, RuntimeConfig::default())
}

fn encode_entry(entry: &PropagationEntry) -> Vec<u8> {
    let value = RmpValue::Array(vec![
        RmpValue::Binary(entry.transient_id.to_vec()),
        RmpValue::Binary(entry.lxm_data.clone()),
        RmpValue::F64(entry.timestamp),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value).expect("encode");
    buf
}

fn generate_valid_peering_key(peering_id: &[u8], target_cost: u32) -> Vec<u8> {
    let mut stamp = [0u8; 32];
    loop {
        rand_core::OsRng.fill_bytes(&mut stamp);
        if validate_peering_key(peering_id, &stamp, target_cost) {
            return stamp.to_vec();
        }
    }
}

fn local_identity_hash(name: &str) -> [u8; 16] {
    let identity = PrivateIdentity::new_from_name(name);
    let slice = identity.as_identity().address_hash.as_slice();
    let mut out = [0u8; 16];
    if slice.len() == out.len() {
        out.copy_from_slice(slice);
    }
    out
}

#[test]
fn peer_sync_offer_rejects_invalid_peering_key() {
    let mut receiver = make_router("peer-recv-invalid");
    let peer_sender = [8u8; 16];
    let now_ms = 1_000;

    receiver.set_peering_validation(local_identity_hash("peer-recv-invalid"), 2);

    let mut peering_id = Vec::with_capacity(32);
    peering_id.extend_from_slice(&local_identity_hash("peer-recv-invalid"));
    peering_id.extend_from_slice(&peer_sender);

    let mut bad_key = vec![0u8; 32];
    if validate_peering_key(&peering_id, &bad_key, 2) {
        bad_key[0] ^= 0xff;
    }
    assert!(!validate_peering_key(&peering_id, &bad_key, 2));

    let offer = PeerSyncRequest::Offer {
        peering_key: bad_key,
        available: vec![[1u8; 32]],
    };
    let response = receiver.handle_peer_sync_offer(peer_sender, &offer, now_ms);
    assert_eq!(response, PeerSyncResponse::Error(0xf3));
}

#[test]
fn peer_sync_offer_accepts_valid_peering_key() {
    let mut receiver = make_router("peer-recv-valid");
    let peer_sender = [8u8; 16];
    let now_ms = 1_000;

    receiver.set_peering_validation(local_identity_hash("peer-recv-valid"), 2);

    let mut peering_id = Vec::with_capacity(32);
    peering_id.extend_from_slice(&local_identity_hash("peer-recv-valid"));
    peering_id.extend_from_slice(&peer_sender);
    let key = generate_valid_peering_key(&peering_id, 2);

    let offer = PeerSyncRequest::Offer {
        peering_key: key,
        available: vec![[2u8; 32]],
    };
    let response = receiver.handle_peer_sync_offer(peer_sender, &offer, now_ms);
    assert_ne!(response, PeerSyncResponse::Error(0xf3));
}

#[test]
fn peer_sync_payload_validates_pn_stamps() {
    let mut receiver = make_router("peer-recv-stamps");
    let peer_sender = [7u8; 16];
    let now_ms = 5_000;
    receiver.set_propagation_validation(2, 0);

    let mut msg = LXMessage::with_strings(
        [0x11u8; 16],
        [0x22u8; 16],
        1_700_000_000.0,
        "hi",
        "hello",
        Value::Map(vec![]),
    );
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[16..].to_vec();
    let (expected_id, stamp, _value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");
    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let valid_entry = PropagationEntry {
        transient_id: [0x99u8; 32],
        lxm_data: lxm_data.clone(),
        timestamp: 1.0,
    };
    let valid_payload = encode_entry(&valid_entry);

    let mut bad_data = lxm_data.clone();
    if let Some(last) = bad_data.last_mut() {
        *last ^= 0x01;
    }
    let invalid_entry = PropagationEntry {
        transient_id: [0x88u8; 32],
        lxm_data: bad_data,
        timestamp: 1.0,
    };
    let invalid_payload = encode_entry(&invalid_entry);

    let response = PeerSyncResponse::Payload(vec![valid_payload, invalid_payload]);
    let ack = receiver
        .apply_peer_sync_payload(peer_sender, &response, now_ms)
        .expect("ack");

    assert!(receiver.propagation_store().has(&expected_id));
    assert!(!receiver.propagation_store().has(&invalid_entry.transient_id));

    match ack {
        PeerSyncRequest::MessageAck { delivered } => {
            assert_eq!(delivered, vec![expected_id]);
        }
        _ => panic!("expected ack"),
    }

    let stored = receiver
        .propagation_store()
        .get(&expected_id)
        .expect("stored entry");
    assert_eq!(stored.lxm_data, lxm_data);

    let validated = validate_pn_stamp(&lxm_data, 2).expect("validated");
    assert_eq!(validated.0, expected_id);
}

#[test]
fn peer_sync_invalid_stamps_throttle_peer() {
    let mut receiver = make_router("peer-recv-throttle");
    let peer_sender = [6u8; 16];
    let now_ms = 9_000;
    receiver.set_propagation_validation(2, 0);

    let invalid_entry = PropagationEntry {
        transient_id: [0x77u8; 32],
        lxm_data: vec![0x44u8; 64],
        timestamp: 1.0,
    };
    let response = PeerSyncResponse::Payload(vec![encode_entry(&invalid_entry)]);
    let ack = receiver.apply_peer_sync_payload(peer_sender, &response, now_ms);
    assert!(ack.is_none());

    let peer = receiver.peers().get(&peer_sender).expect("peer");
    assert!(peer.next_sync_ms >= now_ms + lxmf_rs::PROPAGATION_INVALID_STAMP_THROTTLE_MS);
    assert_eq!(peer.state, lxmf_rs::PeerState::Backoff);
}
