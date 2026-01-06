#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, InMemoryStore, LxmfRuntime, LXMessage, RnsOutbound, RnsRouter, Value};
use reticulum::identity::PrivateIdentity;

fn make_message() -> LXMessage {
    LXMessage::with_strings([0u8; 16], [0u8; 16], 1_700_000_000.0, "hi", "smoke", Value::Map(vec![]))
}

#[test]
fn runtime_tick_smoke() {
    let source = PrivateIdentity::new_from_name("smoke-src");
    let destination = PrivateIdentity::new_from_name("smoke-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::with_store(outbound, Box::new(InMemoryStore::new()));
    let mut runtime = LxmfRuntime::new(router);

    runtime.router_mut().enqueue(make_message(), DeliveryMode::Direct);
    let tick = runtime.tick(1_000);

    assert!(tick.deliveries.len() + tick.delivery_errors.len() >= 1);
}
