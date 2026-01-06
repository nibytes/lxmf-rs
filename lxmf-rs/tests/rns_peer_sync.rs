#![cfg(feature = "rns")]

use lxmf_rs::{
    decode_peer_sync_request, decode_peer_sync_response, encode_peer_sync_request,
    encode_peer_sync_response, PeerSyncRequest, PeerSyncResponse, PeerSyncSession,
    PeerSyncState, PropagationEntry, PropagationStore,
};

#[test]
fn peer_sync_request_round_trip() {
    let req = PeerSyncRequest::Offer {
        peering_key: b"key".to_vec(),
        available: vec![[1u8; 32], [2u8; 32]],
    };
    let bytes = encode_peer_sync_request(&req).expect("encode");
    let decoded = decode_peer_sync_request(&bytes).expect("decode");
    assert_eq!(decoded, req);

    let get_req = PeerSyncRequest::MessageGet {
        wants: vec![[3u8; 32]],
        haves: vec![[4u8; 32]],
        transfer_limit_kb: Some(12.5),
    };
    let bytes = encode_peer_sync_request(&get_req).expect("encode");
    let decoded = decode_peer_sync_request(&bytes).expect("decode");
    assert_eq!(decoded, get_req);

    let ack_req = PeerSyncRequest::MessageAck {
        delivered: vec![[5u8; 32]],
    };
    let bytes = encode_peer_sync_request(&ack_req).expect("encode");
    let decoded = decode_peer_sync_request(&bytes).expect("decode");
    assert_eq!(decoded, ack_req);
}

#[test]
fn peer_sync_response_round_trip() {
    let resp = PeerSyncResponse::Payload(vec![b"a".to_vec(), b"b".to_vec()]);
    let bytes = encode_peer_sync_response(&resp).expect("encode");
    let decoded = decode_peer_sync_response(&bytes).expect("decode");
    assert_eq!(decoded, resp);

    let resp = PeerSyncResponse::IdList(vec![[7u8; 32], [8u8; 32]]);
    let bytes = encode_peer_sync_response(&resp).expect("encode");
    let decoded = decode_peer_sync_response(&bytes).expect("decode");
    assert_eq!(decoded, resp);

    let resp = PeerSyncResponse::Bool(true);
    let bytes = encode_peer_sync_response(&resp).expect("encode");
    let decoded = decode_peer_sync_response(&bytes).expect("decode");
    assert_eq!(decoded, resp);

    let resp = PeerSyncResponse::Error(0xf1);
    let bytes = encode_peer_sync_response(&resp).expect("encode");
    let decoded = decode_peer_sync_response(&bytes).expect("decode");
    assert_eq!(decoded, resp);
}

#[test]
fn peer_sync_session_state_flow() {
    let mut session = PeerSyncSession::new();
    let req = session.offer(b"k".to_vec(), vec![[1u8; 32]]);
    match req {
        PeerSyncRequest::Offer { .. } => {}
        _ => panic!("expected offer"),
    }
    assert_eq!(session.state, PeerSyncState::Offering);
    let state = session.on_offer_response(&PeerSyncResponse::Bool(true));
    assert_eq!(state, PeerSyncState::AwaitingPayload);
}

#[test]
fn propagation_store_insert_and_remove() {
    let mut store = PropagationStore::new();
    let entry = PropagationEntry {
        transient_id: [9u8; 32],
        lxm_data: b"data".to_vec(),
        timestamp: 1.0,
    };
    store.insert(entry.clone());
    assert!(store.has(&entry.transient_id));
    let removed = store.remove(&entry.transient_id).expect("removed");
    assert_eq!(removed.transient_id, entry.transient_id);
}
