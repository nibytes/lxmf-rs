#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, LXMessage, Received, RnsNodeRouter, Value};
use reticulum::identity::PrivateIdentity;
use reticulum::packet::PACKET_MDU;
use tokio::time::{sleep, Duration};

#[tokio::test]
#[ignore]
async fn resource_over_udp_loopback() {
    if std::env::var("RNS_E2E").is_err() {
        return;
    }

    let id_a = PrivateIdentity::new_from_name("res-node-a");
    let id_b = PrivateIdentity::new_from_name("res-node-b");

    let mut node_a = RnsNodeRouter::new_udp(
        "res-node-a",
        id_a.clone(),
        "127.0.0.1:4442",
        "127.0.0.1:4443",
    )
    .await
    .expect("node a");
    let mut node_b = RnsNodeRouter::new_udp(
        "res-node-b",
        id_b.clone(),
        "127.0.0.1:4443",
        "127.0.0.1:4442",
    )
    .await
    .expect("node b");

    for _ in 0..3 {
        node_a.announce().await;
        node_b.announce().await;
        sleep(Duration::from_millis(200)).await;
    }

    let large_body = "x".repeat((PACKET_MDU as usize) * 2);
    let mut msg = LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        &large_body,
        Value::Map(vec![]),
    );

    node_a
        .send_message(id_b.as_identity().clone(), &mut msg, DeliveryMode::Direct)
        .await
        .expect("send");

    let mut received_msg = None;
    for _ in 0..20 {
        for received in node_b.poll_inbound().await {
            if let Received::Message(message) = received {
                received_msg = Some(message);
                break;
            }
        }
        if received_msg.is_some() {
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }

    let received_msg = received_msg.expect("received message");
    assert_eq!(received_msg.content, large_body.as_bytes().to_vec());
}
