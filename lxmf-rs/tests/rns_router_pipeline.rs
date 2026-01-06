#![cfg(feature = "rns")]

use ed25519_dalek::SigningKey;
use lxmf_rs::{
    DeliveryMode, FIELD_TICKET, LxmfRouter, LXMessage, PeerState, PeerSyncRequest,
    PEER_SYNC_TIMEOUT_MS, RnsOutbound, RnsRouter, Value, DESTINATION_LENGTH,
};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("phase2-src");
    let destination = PrivateIdentity::new_from_name("phase2-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

fn make_message(dest: [u8; 16], src: [u8; 16]) -> LXMessage {
    LXMessage::with_strings(dest, src, 1_700_000_000.0, "hi", "phase2", Value::Map(vec![]))
}

#[test]
fn deferred_stamp_queue_processes() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    let dest = [9u8; 16];
    let src = [1u8; 16];
    router.set_outbound_stamp_cost(dest, Some(8), now_ms);

    let mut msg = make_message(dest, src);
    msg.defer_stamp = true;
    let result = router.handle_outbound(msg, DeliveryMode::Direct, now_ms);
    assert!(result.is_none());
    assert_eq!(router.deferred_len(), 1);
    assert_eq!(router.outbound_len(), 0);

    let processed = router.process_deferred_stamps(now_ms + 1_000, 10);
    assert_eq!(processed, 1);
    assert_eq!(router.deferred_len(), 0);
    assert_eq!(router.outbound_len(), 1);
}

#[test]
fn inbound_ticket_is_remembered_for_outbound() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    let expires_at_s = (now_ms as f64 / 1000.0) + 3600.0;
    let ticket = vec![3u8; 16];

    let fields = Value::Map(vec![
        (
            Value::from(FIELD_TICKET as i64),
            Value::Array(vec![Value::from(expires_at_s as i64), Value::Binary(ticket.clone())]),
        ),
    ]);

    let msg = LXMessage::with_strings([8u8; 16], [7u8; 16], 1_700_000_000.0, "hi", "ticket", fields);
    let accepted = router.process_inbound_message(msg, now_ms, true, false);
    assert!(accepted.is_some());

    let outbound_ticket = router.get_outbound_ticket(&[7u8; 16], now_ms).expect("ticket");
    assert_eq!(outbound_ticket, ticket);
}

#[test]
fn inbound_stamp_validates_with_ticket() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    let dest = [5u8; 16];
    let src = [6u8; 16];

    router.set_inbound_stamp_cost(dest, Some(8));
    let (_, ticket) = router
        .generate_ticket(src, now_ms)
        .expect("ticket");

    let mut msg = make_message(dest, src);
    msg.outbound_ticket = Some(ticket);
    msg.defer_stamp = false;
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let _ = msg.encode_signed(&signing_key).expect("signed");

    let accepted = router.process_inbound_message(msg, now_ms, false, false);
    assert!(accepted.is_some());
}

#[test]
fn inbound_marks_local_delivery() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    let mut msg = make_message([4u8; 16], [5u8; 16]);
    let signing_key = SigningKey::from_bytes(&[5u8; 32]);
    let _ = msg.encode_signed(&signing_key).expect("signed");

    let accepted = router.process_inbound_message(msg, now_ms, false, false);
    assert!(accepted.is_some());
    assert_eq!(router.local_deliveries_len(), 1);
}

#[test]
fn propagated_marks_processed_and_store_keys_use_unstamped_id() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.set_propagation_validation(2, 0);

    let mut msg = make_message([0x11u8; 16], [0x22u8; 16]);
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let (expected_id, stamp, _value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");

    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let stored = router
        .process_propagated(1_700_000_000.0, lxm_data.clone(), now_ms, None, true)
        .expect("stored");
    assert_eq!(stored, expected_id);
    assert_eq!(router.locally_processed_len(), 1);
    let entry = router
        .propagation_store()
        .get(&expected_id)
        .expect("entry");
    assert_eq!(entry.lxm_data, lxm_data);
    assert_eq!(router.node_stats().client_propagation_messages_received, 1);
    assert_eq!(router.node_stats().unpeered_propagation_incoming, 0);
    assert_eq!(router.node_stats().unpeered_propagation_rx_bytes, 0);
}

#[test]
fn propagated_unpeered_counters_increment_with_source_hash() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.set_propagation_validation(2, 0);

    let mut msg = make_message([0x33u8; 16], [0x44u8; 16]);
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let (_expected_id, stamp, _value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");

    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let source_hash = [0x99u8; 16];
    let _ = router.process_propagated(
        1_700_000_000.0,
        lxm_data.clone(),
        now_ms,
        Some(source_hash),
        false,
    );

    assert_eq!(router.node_stats().client_propagation_messages_received, 0);
    assert_eq!(router.node_stats().unpeered_propagation_incoming, 1);
    assert_eq!(
        router.node_stats().unpeered_propagation_rx_bytes as usize,
        lxm_data.len()
    );
}

#[test]
fn propagated_client_receive_count_is_optional() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.set_propagation_validation(2, 0);

    let mut msg = make_message([0x55u8; 16], [0x66u8; 16]);
    let signing_key = SigningKey::from_bytes(&[11u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let (_expected_id, stamp, _value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");

    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let _ = router.process_propagated(1_700_000_000.0, lxm_data, now_ms, None, false);
    assert_eq!(router.node_stats().client_propagation_messages_received, 0);
}

#[test]
fn propagated_packet_path_counts_client_even_with_source_hash() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.set_propagation_validation(2, 0);

    let mut msg = make_message([0x77u8; 16], [0x88u8; 16]);
    let signing_key = SigningKey::from_bytes(&[13u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("encode");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let (_expected_id, stamp, _value) = msg
        .generate_propagation_stamp(&encrypted_payload, 2)
        .expect("stamp");

    let mut lxm_data = Vec::new();
    lxm_data.extend_from_slice(&msg.destination_hash);
    lxm_data.extend_from_slice(&encrypted_payload);
    lxm_data.extend_from_slice(&stamp);

    let source_hash = [0x10u8; 16];
    let _ = router.process_propagated(
        1_700_000_000.0,
        lxm_data,
        now_ms,
        Some(source_hash),
        true,
    );

    assert_eq!(router.node_stats().client_propagation_messages_received, 1);
    assert_eq!(router.node_stats().unpeered_propagation_incoming, 0);
}

#[test]
fn peer_sync_is_scheduled_on_tick() {
    let mut router = make_router();
    let now_ms = 5_000u64;
    let peer_id = [0xABu8; 16];
    let local_hash = router.runtime().router().local_identity_hash();
    router.set_peering_validation(local_hash, 1);

    let peer = router.upsert_peer(peer_id, 0);
    peer.next_sync_ms = 0;
    peer.state = PeerState::Idle;

    let tick = router.tick(now_ms);
    let (scheduled_id, req) = tick
        .peer_sync_requests
        .first()
        .cloned()
        .expect("peer sync req");
    assert_eq!(scheduled_id, peer_id);
    match req {
        PeerSyncRequest::Offer { peering_key, .. } => {
            assert!(!peering_key.is_empty());
        }
        _ => panic!("expected offer"),
    }
    let state = router.peers().get(&peer_id).expect("peer").state;
    assert_eq!(state, PeerState::OfferSent);
}

#[test]
fn peer_sync_times_out_and_backoffs() {
    let mut router = make_router();
    let now_ms = PEER_SYNC_TIMEOUT_MS + 10;
    let peer_id = [0xCDu8; 16];
    let peer = router.upsert_peer(peer_id, 0);
    peer.state = PeerState::OfferSent;
    peer.last_attempt_ms = 0;
    peer.next_sync_ms = 0;

    let _ = router.tick(now_ms);
    let peer = router.peers().get(&peer_id).expect("peer");
    assert_eq!(peer.state, PeerState::Backoff);
    assert!(peer.next_sync_ms >= now_ms);
}

#[test]
fn peer_sync_request_timeouts_clear_pending() {
    let mut router = make_router();
    let peer_id = [0xEEu8; 16];
    let now_ms = 0u64;
    let offer = router.peer_sync_offer(peer_id, vec![0x01u8; 32], now_ms);
    let _req = router
        .build_peer_sync_request(peer_id, offer, now_ms)
        .expect("request");
    assert_eq!(router.peer_sync_pending_len(), 1);

    let _ = router.tick(PEER_SYNC_TIMEOUT_MS + 1);
    assert_eq!(router.peer_sync_pending_len(), 0);
}

#[test]
fn propagation_resource_rejects_multi_message_without_identity() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.clear_propagation_validation();

    let messages = vec![vec![0x01u8; 32], vec![0x02u8; 32]];
    let accepted = router.process_propagation_resource(None, None, &messages, None, now_ms);
    assert!(accepted.is_empty());
}

#[test]
fn propagation_resource_rejects_invalid_peering_key() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    let local_hash = router.runtime().router().local_identity_hash();
    router.set_peering_validation(local_hash, 1);
    let remote_hash = [0x11u8; 16];
    let messages = vec![vec![0x01u8; 32], vec![0x02u8; 32]];

    let accepted = router.process_propagation_resource(
        Some(remote_hash),
        None,
        &messages,
        None,
        now_ms,
    );
    assert!(accepted.is_empty());
    let peer = router.peers().get(&remote_hash).expect("peer");
    assert_eq!(peer.state, PeerState::Backoff);
}

#[test]
fn propagation_resource_honors_transfer_limit_and_counts_clients() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.clear_propagation_validation();

    let messages = vec![vec![0x03u8; 1200]];
    let accepted = router.process_propagation_resource(None, None, &messages, Some(1.0), now_ms);
    assert!(accepted.is_empty());
    assert_eq!(router.node_stats().client_propagation_messages_received, 0);

    let messages = vec![vec![0x04u8; 32]];
    let accepted = router.process_propagation_resource(None, None, &messages, None, now_ms);
    assert_eq!(accepted.len(), 1);
    assert_eq!(router.node_stats().client_propagation_messages_received, 1);
}

#[test]
fn propagation_resource_ingress_counts_unpeered() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    router.clear_propagation_validation();

    let payload = Value::Array(vec![
        Value::F64(now_ms as f64 / 1000.0),
        Value::Array(vec![Value::Binary(vec![0x07u8; 32])]),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &payload).expect("msgpack");

    router.receive_resource_payload(buf, Some([0x22u8; 16]));
    let _ = router.process_inbound_queue(now_ms, false);

    assert_eq!(router.node_stats().client_propagation_messages_received, 0);
    assert_eq!(router.node_stats().unpeered_propagation_incoming, 1);
}
