#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, LXMessage, RnsNodeRouter, Value};
use reticulum::identity::PrivateIdentity;
use tokio::time::{sleep, Duration};

#[tokio::test]
#[ignore]
async fn lxmf_over_udp_loopback() {
    if std::env::var("RNS_E2E").is_err() {
        return;
    }

    let id_a = PrivateIdentity::new_from_name("node-a");
    let id_b = PrivateIdentity::new_from_name("node-b");

    let node_a = RnsNodeRouter::new_udp("node-a", id_a.clone(), "127.0.0.1:4242", "127.0.0.1:4243")
        .await
        .expect("node a");
    let mut node_b = RnsNodeRouter::new_udp("node-b", id_b.clone(), "127.0.0.1:4243", "127.0.0.1:4242")
        .await
        .expect("node b");

    // Exchange announces to establish links.
    for _ in 0..3 {
        node_a.announce().await;
        node_b.announce().await;
        sleep(Duration::from_millis(200)).await;
    }

    let mut msg = LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "hello",
        Value::Map(vec![]),
    );

    node_a
        .send_message(id_b.as_identity().clone(), &mut msg, DeliveryMode::Direct)
        .await
        .expect("send");

    let mut received = Vec::new();
    for _ in 0..10 {
        received.extend(node_b.poll_inbound().await);
        if !received.is_empty() {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(!received.is_empty());
}
