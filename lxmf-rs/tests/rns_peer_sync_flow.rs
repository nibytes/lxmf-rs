#![cfg(feature = "rns")]

use lxmf_rs::{
    validate_peering_key, LxmfRouter, PeerState, PeerSyncResponse, PropagationEntry, Received,
    RnsOutbound, RnsRouter, RuntimeConfig,
};
use rand_core::RngCore;
use reticulum::identity::PrivateIdentity;

fn make_router(name: &str) -> LxmfRouter {
    let source = PrivateIdentity::new_from_name(name);
    let destination = PrivateIdentity::new_from_name(&format!("{name}-dest"))
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::new(outbound);
    LxmfRouter::with_config(router, RuntimeConfig::default())
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

fn generate_valid_peering_key(peering_id: &[u8], target_cost: u32) -> Vec<u8> {
    let mut stamp = [0u8; 32];
    loop {
        rand_core::OsRng.fill_bytes(&mut stamp);
        if validate_peering_key(peering_id, &stamp, target_cost) {
            return stamp.to_vec();
        }
    }
}


#[test]
fn peer_sync_offer_get_ack_flow() {
    let mut sender = make_router("sender");
    let mut receiver = make_router("receiver");

    let entry_a = PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: b"one".to_vec(),
        timestamp: 1.0,
    };
    let entry_b = PropagationEntry {
        transient_id: [2u8; 32],
        lxm_data: b"two".to_vec(),
        timestamp: 2.0,
    };
    sender.propagation_store_mut().insert(entry_a.clone());
    sender.propagation_store_mut().insert(entry_b.clone());

    let peer_sender = [9u8; 16];
    let peer_receiver = [8u8; 16];
    let now_ms = 1_000;

    let local_hash = local_identity_hash("receiver");
    receiver.set_peering_validation(local_hash, 2);
    receiver.clear_propagation_validation();
    let mut peering_id = Vec::with_capacity(32);
    peering_id.extend_from_slice(&local_hash);
    peering_id.extend_from_slice(&peer_sender);
    let key = generate_valid_peering_key(&peering_id, 2);

    let offer = sender.peer_sync_offer(peer_receiver, key, now_ms);
    let response = receiver.handle_peer_sync_offer(peer_sender, &offer, now_ms);

    let get_req = receiver
        .peer_sync_message_get(peer_sender, &response, None, now_ms)
        .expect("message get");
    let payload = sender.handle_peer_sync_message_get(peer_receiver, &get_req, now_ms);
    assert_eq!(sender.node_stats().client_propagation_messages_served, 2);

    let ack_req = receiver
        .apply_peer_sync_payload(peer_sender, &payload, now_ms)
        .expect("ack request");

    assert!(receiver.propagation_store().has(&entry_a.transient_id));
    assert!(receiver.propagation_store().has(&entry_b.transient_id));

    let ack_resp = sender.handle_peer_sync_ack(peer_receiver, &ack_req, now_ms);
    assert_eq!(ack_resp, PeerSyncResponse::Bool(true));
    assert!(!sender.propagation_store().has(&entry_a.transient_id));

    let sender_state = sender.peers().get(&peer_receiver).expect("peer").state;
    assert_eq!(sender_state, PeerState::Idle);
}

#[test]
fn peer_sync_empty_payload_resets_state() {
    let mut receiver = make_router("receiver-empty");
    let peer_id = [7u8; 16];
    let now_ms = 10_000;

    receiver.upsert_peer(peer_id, now_ms).on_offer_sent(now_ms);
    let response = PeerSyncResponse::Payload(Vec::new());
    let ack = receiver.apply_peer_sync_payload(peer_id, &response, now_ms);
    assert!(ack.is_none());

    let state = receiver.peers().get(&peer_id).expect("peer").state;
    assert_eq!(state, PeerState::Idle);
}

#[test]
fn peer_sync_automation_offer_get_ack_loop() {
    let mut sender = make_router("sender-auto");
    let mut receiver = make_router("receiver-auto");

    let entry_a = PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: b"one".to_vec(),
        timestamp: 1.0,
    };
    let entry_b = PropagationEntry {
        transient_id: [2u8; 32],
        lxm_data: b"two".to_vec(),
        timestamp: 2.0,
    };
    sender.propagation_store_mut().insert(entry_a.clone());
    sender.propagation_store_mut().insert(entry_b.clone());

    let peer_sender = [9u8; 16];
    let peer_receiver = [8u8; 16];
    let now_ms = 1_000;

    let local_hash = local_identity_hash("receiver-auto");
    receiver.set_peering_validation(local_hash, 2);
    receiver.clear_propagation_validation();

    let offer = sender.peer_sync_offer(peer_receiver, vec![0x01u8; 32], now_ms);
    let offer_req = sender
        .build_peer_sync_request(peer_receiver, offer, now_ms)
        .expect("request");

    let offer_resp = receiver
        .handle_peer_sync_request(peer_sender, &offer_req, now_ms)
        .expect("offer response");

    let _ = sender
        .handle_peer_sync_response(&offer_resp, now_ms)
        .expect("offer response handled");

    let (peer_id, msg_get) = receiver.pop_peer_sync_request().expect("msg get queued");
    assert_eq!(peer_id, peer_sender);
    let msg_get_req = receiver
        .build_peer_sync_request(peer_sender, msg_get, now_ms)
        .expect("message get request");

    let payload_resp = sender
        .handle_peer_sync_request(peer_receiver, &msg_get_req, now_ms)
        .expect("payload response");

    let ack_req = receiver
        .handle_peer_sync_response(&payload_resp, now_ms)
        .expect("ack req")
        .expect("ack req");

    let ack_resp = sender
        .handle_peer_sync_request(peer_receiver, &ack_req, now_ms)
        .expect("ack resp");

    let _ = receiver
        .handle_peer_sync_response(&ack_resp, now_ms)
        .expect("ack handled");

    assert!(receiver.propagation_store().has(&entry_a.transient_id));
    assert!(receiver.propagation_store().has(&entry_b.transient_id));
    let state = receiver.peers().get(&peer_sender).expect("peer").state;
    assert_eq!(state, PeerState::Idle);
}

#[test]
fn peer_sync_received_request_response_flow() {
    let mut sender = make_router("sender-received");
    let mut receiver = make_router("receiver-received");

    let entry_a = PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: b"one".to_vec(),
        timestamp: 1.0,
    };
    let entry_b = PropagationEntry {
        transient_id: [2u8; 32],
        lxm_data: b"two".to_vec(),
        timestamp: 2.0,
    };
    sender.propagation_store_mut().insert(entry_a.clone());
    sender.propagation_store_mut().insert(entry_b.clone());

    let peer_sender = [9u8; 16];
    let peer_receiver = [8u8; 16];
    let now_ms = 1_000;

    let local_hash = local_identity_hash("receiver-received");
    receiver.set_peering_validation(local_hash, 2);
    receiver.clear_propagation_validation();
    let mut peering_id = Vec::with_capacity(32);
    peering_id.extend_from_slice(&local_hash);
    peering_id.extend_from_slice(&peer_sender);
    let key = generate_valid_peering_key(&peering_id, 2);

    let offer = sender.peer_sync_offer(peer_receiver, key, now_ms);
    let offer_req = sender
        .build_peer_sync_request(peer_receiver, offer, now_ms)
        .expect("offer request");

    receiver.receive_inbound(Received::Request {
        request: offer_req,
        source_hash: Some(peer_sender),
    });
    receiver.process_inbound_queue(now_ms, true);

    let (peer_id, offer_resp) = receiver.pop_peer_sync_response().expect("offer response");
    assert_eq!(peer_id, peer_sender);
    let (peer_id, msg_get) = receiver.pop_peer_sync_request().expect("msg get queued");
    assert_eq!(peer_id, peer_sender);

    sender.receive_inbound(Received::Response {
        response: offer_resp,
        source_hash: Some(peer_receiver),
    });
    sender.process_inbound_queue(now_ms, true);

    let msg_get_req = receiver
        .build_peer_sync_request(peer_sender, msg_get, now_ms)
        .expect("message get request");
    sender.receive_inbound(Received::Request {
        request: msg_get_req,
        source_hash: Some(peer_receiver),
    });
    sender.process_inbound_queue(now_ms, true);

    let (peer_id, payload_resp) = sender.pop_peer_sync_response().expect("payload response");
    assert_eq!(peer_id, peer_receiver);
    receiver.receive_inbound(Received::Response {
        response: payload_resp,
        source_hash: Some(peer_receiver),
    });
    receiver.process_inbound_queue(now_ms, true);

    let (peer_id, ack_req) = receiver.pop_peer_sync_outbound().expect("ack request");
    assert_eq!(peer_id, peer_sender);
    sender.receive_inbound(Received::Request {
        request: ack_req,
        source_hash: Some(peer_receiver),
    });
    sender.process_inbound_queue(now_ms, true);

    let (peer_id, ack_resp) = sender.pop_peer_sync_response().expect("ack response");
    assert_eq!(peer_id, peer_receiver);
    receiver.receive_inbound(Received::Response {
        response: ack_resp,
        source_hash: Some(peer_receiver),
    });
    receiver.process_inbound_queue(now_ms, true);

    assert!(receiver.propagation_store().has(&entry_a.transient_id));
    assert!(receiver.propagation_store().has(&entry_b.transient_id));
    let state = receiver.peers().get(&peer_sender).expect("peer").state;
    assert_eq!(state, PeerState::Idle);
}
