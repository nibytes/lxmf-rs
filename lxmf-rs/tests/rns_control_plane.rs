#![cfg(feature = "rns")]

use lxmf_rs::{
    LxmfRouter, PeerState, RnsOutbound, RnsRequest, RnsRouter, RuntimeConfig, Value,
    CONTROL_ERROR_NO_ACCESS, CONTROL_STATS_PATH, CONTROL_SYNC_PATH, CONTROL_UNPEER_PATH,
};
use reticulum::identity::PrivateIdentity;
use sha2::{Digest, Sha256};

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("control");
    let destination = PrivateIdentity::new_from_name("control-dest")
        .as_identity()
        .clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::new(outbound);
    LxmfRouter::with_config(router, RuntimeConfig::default())
}

fn request(path: &str, data: Value) -> RnsRequest {
    let mut out = [0u8; 16];
    let digest = Sha256::new().chain_update(path.as_bytes()).finalize();
    out.copy_from_slice(&digest[..16]);
    RnsRequest {
        request_id: [1u8; 16],
        requested_at: 1_700_000_000.0,
        path_hash: out,
        data,
    }
}

#[test]
fn control_stats_respects_allowlist() {
    let mut router = make_router();
    let now_ms = 1_000;
    router.enable_propagation_node(true, now_ms);

    let caller = [9u8; 16];
    let req = request(CONTROL_STATS_PATH, Value::Nil);
    router.receive_inbound(lxmf_rs::Received::Request {
        request: req,
        source_hash: Some(caller),
    });
    router.process_inbound_queue(now_ms, true);

    let (_, resp) = router.pop_control_response().expect("control response");
    assert_eq!(resp.data, Value::from(CONTROL_ERROR_NO_ACCESS as i64));

    router.allow_control(caller);
    let req = request(CONTROL_STATS_PATH, Value::Nil);
    router.receive_inbound(lxmf_rs::Received::Request {
        request: req,
        source_hash: Some(caller),
    });
    router.process_inbound_queue(now_ms, true);

    let (_, resp) = router.pop_control_response().expect("control response");
    assert!(matches!(resp.data, Value::Map(_)));
}

#[test]
fn control_sync_and_unpeer_work() {
    let mut router = make_router();
    let now_ms = 2_000;
    router.enable_propagation_node(true, now_ms);

    let controller = [7u8; 16];
    let target = [5u8; 16];
    router.allow_control(controller);
    router.upsert_peer(target, now_ms).state = PeerState::Backoff;

    let req = request(CONTROL_SYNC_PATH, Value::Binary(target.to_vec()));
    router.receive_inbound(lxmf_rs::Received::Request {
        request: req,
        source_hash: Some(controller),
    });
    router.process_inbound_queue(now_ms, true);
    let (_, resp) = router.pop_control_response().expect("sync response");
    assert_eq!(resp.data, Value::Boolean(true));

    let req = request(CONTROL_UNPEER_PATH, Value::Binary(target.to_vec()));
    router.receive_inbound(lxmf_rs::Received::Request {
        request: req,
        source_hash: Some(controller),
    });
    router.process_inbound_queue(now_ms, true);
    let (_, resp) = router.pop_control_response().expect("unpeer response");
    assert_eq!(resp.data, Value::Boolean(true));
    assert!(router.peers().get(&target).is_none());
}

#[test]
fn auth_required_blocks_unallowed_resources() {
    let mut router = make_router();
    let now_ms = 10_000;
    router.set_authentication(true);
    router.set_allowed_identities(vec![[2u8; 16]]);
    router.clear_propagation_validation();

    let payload = vec![0x01u8; 32];
    let rejected = router.process_propagation_resource(None, None, &[payload.clone()], None, now_ms);
    assert!(rejected.is_empty());

    let rejected =
        router.process_propagation_resource(Some([3u8; 16]), None, &[payload.clone()], None, now_ms);
    assert!(rejected.is_empty());

    let accepted =
        router.process_propagation_resource(Some([2u8; 16]), None, &[payload], None, now_ms);
    assert_eq!(accepted.len(), 1);
}

#[test]
fn control_request_without_identity_returns_no_identity() {
    let mut router = make_router();
    let now_ms = 1234;
    router.enable_propagation_node(true, now_ms);
    let req = request(CONTROL_STATS_PATH, Value::Nil);
    router.receive_inbound(lxmf_rs::Received::ControlRequest {
        request: req,
        source_hash: None,
    });
    router.process_inbound_queue(now_ms, true);
    let (_, resp) = router.pop_control_response().expect("control response");
    assert_eq!(resp.data, Value::from(lxmf_rs::CONTROL_ERROR_NO_IDENTITY as i64));
}

#[test]
fn ignored_destinations_drop_inbound_messages() {
    let mut router = make_router();
    let now_ms = 1_000;
    let ignored = [9u8; 16];
    router.ignore_destination(ignored);
    let msg = lxmf_rs::LXMessage::with_strings(
        [1u8; 16],
        ignored,
        1_700_000_000.0,
        "hi",
        "there",
        Value::Map(vec![]),
    );
    let out = router.process_inbound_message(msg, now_ms, false, false);
    assert!(out.is_none());
}

#[test]
fn control_stats_include_peer_metrics() {
    let mut router = make_router();
    let now_ms = 5_000;
    router.enable_propagation_node(true, now_ms);
    let controller = [3u8; 16];
    router.allow_control(controller);

    let entry = lxmf_rs::PropagationEntry {
        transient_id: [1u8; 32],
        lxm_data: vec![0x01, 0x02, 0x03],
        timestamp: 1.0,
    };
    router.propagation_store_mut().insert(entry);
    let peer_id = [4u8; 16];

    let wants = router.propagation_store().list_ids();
    let req = lxmf_rs::PeerSyncRequest::MessageGet {
        wants,
        haves: Vec::new(),
        transfer_limit_kb: None,
    };
    let response = router.handle_peer_sync_message_get(peer_id, &req, now_ms);
    if let lxmf_rs::PeerSyncResponse::Payload(items) = response {
        let _ = router.apply_peer_sync_payload(peer_id, &lxmf_rs::PeerSyncResponse::Payload(items), now_ms);
    }

    let req = request(CONTROL_STATS_PATH, Value::Nil);
    router.receive_inbound(lxmf_rs::Received::Request {
        request: req,
        source_hash: Some(controller),
    });
    router.process_inbound_queue(now_ms, true);
    let (_, resp) = router.pop_control_response().expect("control response");
    if let Value::Map(stats) = resp.data {
        let peers = stats.iter().find_map(|(k, v)| match k {
            Value::String(s) if s.as_str() == Some("peers") => Some(v),
            _ => None,
        });
        assert!(matches!(peers, Some(Value::Map(_))));
    } else {
        panic!("expected map stats");
    }
}
