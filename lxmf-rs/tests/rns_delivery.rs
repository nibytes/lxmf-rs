#![cfg(feature = "rns")]

use lxmf_rs::{
    decode_packet, DeliveryMode, DeliveryOutput, EncryptHook, InMemoryStore, NullEncryptor,
    QueuedDelivery, RetryPolicy, RnsCryptoPolicy, RnsOutbound, RnsRouter, Received, RnsError,
};
use lxmf_rs::{LXMessage, Value, DESTINATION_LENGTH};
use reticulum::identity::PrivateIdentity;
use rmpv::Value as RmpValue;
use std::sync::{Arc, Mutex};
use std::io::Cursor;

fn make_outbound() -> RnsOutbound {
    let source = PrivateIdentity::new_from_name("source");
    let destination = PrivateIdentity::new_from_name("dest").as_identity().clone();
    RnsOutbound::with_encryptor(source, destination, Box::new(NullEncryptor))
}

fn make_outbound_failing() -> RnsOutbound {
    struct FailingEncryptor;

    impl EncryptHook for FailingEncryptor {
        fn encrypt(
            &self,
            _destination: &reticulum::destination::SingleOutputDestination,
            _plaintext: &[u8],
        ) -> Result<Vec<u8>, RnsError> {
            Err(RnsError::InvalidDestinationHash)
        }
    }

    let source = PrivateIdentity::new_from_name("source-enc");
    let destination = PrivateIdentity::new_from_name("dest-enc").as_identity().clone();
    RnsOutbound::with_encryptor(source, destination, Box::new(FailingEncryptor))
}

fn make_router() -> RnsRouter {
    let outbound = make_outbound();
    RnsRouter::new(outbound)
}

fn make_outbound_real() -> RnsOutbound {
    let source = PrivateIdentity::new_from_name("source-real");
    let destination = PrivateIdentity::new_from_name("dest-real").as_identity().clone();
    RnsOutbound::new(source, destination)
}

fn make_message() -> LXMessage {
    LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "hello",
        Value::Map(vec![]),
    )
}

#[test]
fn opportunistic_packet_payload_matches_spec() {
    let outbound = make_outbound();
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Opportunistic).unwrap();
    let packet = match result {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    let packed = msg.encode_with_signature().unwrap();
    assert_eq!(packet.data.as_slice(), &packed[DESTINATION_LENGTH..]);
}

#[test]
fn direct_packet_payload_matches_spec() {
    let outbound = make_outbound();
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Direct).unwrap();
    let packet = match result {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    let packed = msg.encode_with_signature().unwrap();
    assert_eq!(packet.data.as_slice(), &packed);
}

#[test]
fn propagated_payload_structure_is_msgpack() {
    let outbound = make_outbound();
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Propagated).unwrap();
    let packet = match result {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    let decoded = rmpv::decode::read_value(&mut Cursor::new(packet.data.as_slice())).unwrap();
    let arr = match decoded {
        RmpValue::Array(arr) => arr,
        _ => panic!("expected array"),
    };
    assert_eq!(arr.len(), 2);
}

#[test]
fn paper_uri_has_expected_prefix() {
    let outbound = make_outbound();
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Paper).unwrap();
    let uri = match result {
        DeliveryOutput::PaperUri(uri) => uri,
        _ => panic!("expected uri"),
    };

    assert!(uri.starts_with("lxm://"));
}

#[test]
fn decode_opportunistic_round_trip() {
    let outbound = make_outbound();
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Opportunistic).unwrap();
    let packet = match result {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    let received = decode_packet(&packet, DeliveryMode::Opportunistic).unwrap();
    let decoded = match received {
        Received::Message(m) => m,
        _ => panic!("expected message"),
    };

    assert_eq!(decoded.title, b"hi");
    assert_eq!(decoded.content, b"hello");
}

#[test]
fn router_queue_fifo_and_state() {
    let mut router = make_router();
    let msg1 = make_message();
    let msg2 = make_message();

    let id1 = router.enqueue(msg1, DeliveryMode::Direct);
    let id2 = router.enqueue(msg2, DeliveryMode::Direct);

    let QueuedDelivery { id, message, .. } = router.next_delivery(0).unwrap().unwrap();
    assert_eq!(id, id1);
    assert_eq!(message.state, lxmf_rs::STATE_SENT);

    let QueuedDelivery { id, .. } = router.next_delivery(0).unwrap().unwrap();
    assert_eq!(id, id2);
    assert_eq!(router.pending_len(), 0);
}

#[test]
fn router_resource_when_payload_large() {
    let mut router = make_router();
    let mut msg = make_message();
    msg.content = vec![b'a'; 4096];

    router.enqueue(msg, DeliveryMode::Direct);
    let delivery = router.next_delivery(0).unwrap().unwrap().delivery;

    match delivery {
        DeliveryOutput::Resource { data, .. } => assert!(data.len() > reticulum::packet::PACKET_MDU),
        _ => panic!("expected resource"),
    }
}

#[test]
fn router_retry_and_failure_retention() {
    let outbound = make_outbound_failing();
    let retry = RetryPolicy {
        max_attempts: 2,
        base_delay_ms: 10,
        max_delay_ms: 100,
    };
    let mut router = RnsRouter::with_retry(outbound, retry);

    let msg = make_message();
    router.enqueue(msg, DeliveryMode::Propagated);

    // First attempt fails and is rescheduled.
    let _ = router
        .next_delivery_with_retry(0)
        .expect("expected retry attempt")
        .unwrap_err();
    assert_eq!(router.failed_len(), 0);

    // Second attempt at later time fails and is moved to failed.
    let _ = router
        .next_delivery_with_retry(20)
        .expect("expected retry attempt")
        .unwrap_err();
    assert_eq!(router.failed_len(), 1);

    let failed = router.pop_failed().unwrap();
    assert_eq!(failed.message.state, lxmf_rs::STATE_FAILED);
}

#[test]
fn router_callbacks_invoked() {
    let outbound = make_outbound();
    let mut router = RnsRouter::with_store(outbound, Box::new(InMemoryStore::default()));
    let delivered = Arc::new(Mutex::new(0));
    let failed = Arc::new(Mutex::new(0));

    let delivered_clone = delivered.clone();
    router.set_delivery_callback(Box::new(move |_| {
        *delivered_clone.lock().unwrap() += 1;
    }));

    let failed_clone = failed.clone();
    router.set_failed_callback(Box::new(move |_| {
        *failed_clone.lock().unwrap() += 1;
    }));

    let msg = make_message();
    router.enqueue(msg, DeliveryMode::Direct);
    let _ = router.next_delivery_with_retry(0).unwrap();

    assert_eq!(*delivered.lock().unwrap(), 1);
    assert_eq!(*failed.lock().unwrap(), 0);
}

#[test]
fn inbound_pipeline_decodes_message() {
    let outbound = make_outbound();
    let mut router = RnsRouter::new(make_outbound());
    let mut msg = make_message();
    let packet = match outbound.deliver(&mut msg, DeliveryMode::Direct).unwrap() {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    router.receive_packet(&packet, DeliveryMode::Direct).unwrap();
    let received = router.pop_inbound().unwrap();
    match received {
        Received::Message(m) => {
            assert_eq!(m.title, b"hi");
        }
        _ => panic!("expected message"),
    }
}

#[test]
fn crypto_policy_enforce_ratchets_rejects_null_encryptor_for_propagated() {
    let mut outbound = make_outbound();
    outbound.set_crypto_policy(RnsCryptoPolicy::EnforceRatchets);
    let mut msg = make_message();

    let err = outbound.deliver(&mut msg, DeliveryMode::Propagated).unwrap_err();
    match err {
        RnsError::CryptoPolicyViolation { policy, mode, .. } => {
            assert_eq!(policy, RnsCryptoPolicy::EnforceRatchets);
            assert_eq!(mode, DeliveryMode::Propagated);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn crypto_policy_enforce_ratchets_rejects_direct_mode() {
    let mut outbound = make_outbound_real();
    outbound.set_crypto_policy(RnsCryptoPolicy::EnforceRatchets);
    let mut msg = make_message();

    let err = outbound.deliver(&mut msg, DeliveryMode::Direct).unwrap_err();
    match err {
        RnsError::CryptoPolicyViolation { policy, mode, .. } => {
            assert_eq!(policy, RnsCryptoPolicy::EnforceRatchets);
            assert_eq!(mode, DeliveryMode::Direct);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_next_delivery_with_retry_treats_crypto_policy_violation_as_fatal() {
    let mut router = RnsRouter::new(make_outbound());
    router.set_crypto_policy(RnsCryptoPolicy::EnforceRatchets);
    router.enqueue(make_message(), DeliveryMode::Propagated);

    let _ = router
        .next_delivery_with_retry(0)
        .expect("expected attempt")
        .unwrap_err();

    assert_eq!(router.failed_len(), 1);
}
