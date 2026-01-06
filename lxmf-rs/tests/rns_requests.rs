#![cfg(feature = "rns")]

use lxmf_rs::{
    decode_request_bytes_for_destination, decode_response_bytes, encode_request_bytes,
    encode_response_bytes, RnsRequest, RnsRequestManager, RnsResponse,
};
use lxmf_rs::Value;
use reticulum::packet::{DestinationType, Header, PacketContext, PacketType};
use sha2::{Digest, Sha256};

fn truncated_sha256(input: &[u8]) -> [u8; 16] {
    let digest = Sha256::new().chain_update(input).finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn path_hash(path: &str) -> [u8; 16] {
    truncated_sha256(path.as_bytes())
}

fn request_id_for_destination(destination_hash: [u8; 16], payload: &[u8]) -> [u8; 16] {
    let header = Header {
        destination_type: DestinationType::Single,
        packet_type: PacketType::Data,
        ..Default::default()
    };
    let mut hasher = Sha256::new();
    hasher.update(&[header.to_meta() & 0b0000_1111]);
    hasher.update(&destination_hash);
    hasher.update(&[PacketContext::Request as u8]);
    hasher.update(payload);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

#[test]
fn request_round_trip() {
    let destination_hash = [0x22u8; 16];
    let req = RnsRequest {
        request_id: [0u8; 16],
        requested_at: 1_700_000_000.0,
        path_hash: path_hash("/get"),
        data: Value::Binary(b"ping".to_vec()),
    };
    let bytes = encode_request_bytes(&req).expect("encode");
    let decoded = decode_request_bytes_for_destination(destination_hash, &bytes).expect("decode");
    assert_eq!(decoded.requested_at, req.requested_at);
    assert_eq!(decoded.path_hash, req.path_hash);
    assert_eq!(decoded.data, req.data);
    assert_eq!(decoded.request_id, request_id_for_destination(destination_hash, &bytes));
}

#[test]
fn response_round_trip() {
    let resp = RnsResponse {
        request_id: [9u8; 16],
        data: Value::Binary(b"pong".to_vec()),
    };
    let bytes = encode_response_bytes(&resp).expect("encode");
    let decoded = decode_response_bytes(&bytes).expect("decode");
    assert_eq!(decoded, resp);
}

#[test]
fn request_manager_tracks_and_times_out() {
    let mut manager = RnsRequestManager::new();
    let req = manager
        .register_request(
            [7u8; 16],
            Some("/get".to_string()),
            Value::Binary(b"data".to_vec()),
            100,
            50,
        )
        .expect("register");
    assert!(manager.is_pending(req.request_id));

    let timeouts = manager.poll_timeouts(160);
    assert_eq!(timeouts, vec![req.request_id]);
    assert!(!manager.is_pending(req.request_id));
}

#[test]
fn request_manager_records_response() {
    let mut manager = RnsRequestManager::new();
    let req = manager
        .register_request([8u8; 16], None, Value::Binary(b"data".to_vec()), 100, 50)
        .expect("register");
    let resp = RnsResponse {
        request_id: req.request_id,
        data: Value::Binary(b"ok".to_vec()),
    };
    assert!(manager.record_response(resp.clone()));
    let out = manager.take_response(req.request_id).expect("response");
    assert_eq!(out, resp);
}
