#![cfg(feature = "rns")]

use lxmf_rs::{
    DeliveryMode, LxmfRuntime, RuntimeConfig, RnsOutbound, RnsRouter, Value, STATE_FAILED,
};
use reticulum::identity::PrivateIdentity;

fn make_outbound() -> RnsOutbound {
    let source = PrivateIdentity::new_from_name("rt-source");
    let destination = PrivateIdentity::new_from_name("rt-dest").as_identity().clone();
    RnsOutbound::new(source, destination)
}

fn make_message() -> lxmf_rs::LXMessage {
    lxmf_rs::LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "runtime",
        Value::Map(vec![]),
    )
}

#[test]
fn runtime_schedules_outbound_ticks() {
    let outbound = make_outbound();
    let mut router = RnsRouter::new(outbound);
    router.enqueue(make_message(), DeliveryMode::Direct);

    let config = RuntimeConfig {
        outbound_interval_ms: 0,
        inflight_interval_ms: 1000,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };

    let mut runtime = LxmfRuntime::with_config(router, config);

    let tick = runtime.tick(0);
    assert_eq!(tick.deliveries.len(), 1);

    let tick = runtime.tick(50);
    assert!(tick.deliveries.is_empty());
}

#[test]
fn runtime_drives_inflight_timeouts() {
    let outbound = make_outbound();
    let mut router = RnsRouter::new(outbound);
    router.enqueue(make_message(), DeliveryMode::Direct);
    let delivery = router.next_delivery_with_retry(0).unwrap().unwrap();
    router.mark_inflight(&delivery, 100, 10);

    let config = RuntimeConfig {
        outbound_interval_ms: 1000,
        inflight_interval_ms: 0,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };

    let mut runtime = LxmfRuntime::with_config(router, config);
    let tick = runtime.tick(200);
    assert_eq!(tick.inflight_failures.len(), 1);
    assert_eq!(tick.inflight_failures[0].message.state, STATE_FAILED);
}

#[test]
fn runtime_tracks_request_timeouts() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);
    let config = RuntimeConfig {
        outbound_interval_ms: 1000,
        inflight_interval_ms: 1000,
        request_interval_ms: 0,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };
    let mut runtime = LxmfRuntime::with_config(router, config);
    let req = runtime
        .requests_mut()
        .register_request(
            [9u8; 16],
            Some("/get".to_string()),
            Value::Binary(b"data".to_vec()),
            100,
            10,
        )
        .expect("register");

    let tick = runtime.tick(200);
    assert_eq!(tick.request_timeouts, vec![req.request_id]);
}
