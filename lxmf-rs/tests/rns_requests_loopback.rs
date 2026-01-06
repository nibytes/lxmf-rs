#![cfg(feature = "rns")]

use lxmf_rs::{RnsRequest, RnsNodeRouter, Value};
use reticulum::identity::PrivateIdentity;
use sha2::{Digest, Sha256};

#[tokio::test]
#[ignore]
async fn request_send_over_udp_loopback() {
    if std::env::var("RNS_E2E").is_err() {
        return;
    }

    let id_a = PrivateIdentity::new_from_name("req-node-a");
    let id_b = PrivateIdentity::new_from_name("req-node-b");

    let node_a = RnsNodeRouter::new_udp(
        "req-node-a",
        id_a.clone(),
        "127.0.0.1:4342",
        "127.0.0.1:4343",
    )
    .await
    .expect("node a");
    let _node_b = RnsNodeRouter::new_udp(
        "req-node-b",
        id_b.clone(),
        "127.0.0.1:4343",
        "127.0.0.1:4342",
    )
    .await
    .expect("node b");

    let mut req = RnsRequest {
        request_id: [0u8; 16],
        requested_at: 1_700_000_000.0,
        path_hash: {
            let mut hash = [0u8; 16];
            hash.copy_from_slice(&Sha256::digest(b"/ping")[..16]);
            hash
        },
        data: Value::Binary(b"hello".to_vec()),
    };

    node_a
        .send_request(id_b.as_identity().clone(), &mut req)
        .await
        .expect("send request");
}
