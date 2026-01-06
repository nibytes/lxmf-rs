#![cfg(feature = "rns")]

use ed25519_dalek::SigningKey;
use lxmf_rs::{decode_packet, DeliveryMode, DeliveryOutput, LXMessage, Received, RnsOutbound, RnsRouter};
use lxmf_rs::{STAMP_SIZE, Value, DESTINATION_LENGTH};
use reticulum::identity::PrivateIdentity;
use reticulum::packet::{
    DestinationType, Header, Packet, PacketContext, PacketDataBuffer, PacketType,
};
use sha2::{Digest, Sha256};

fn make_message() -> LXMessage {
    LXMessage::with_strings(
        [0u8; 16],
        [1u8; 16],
        1_700_000_000.0,
        "hi",
        "transport",
        Value::Map(vec![]),
    )
}

fn packet_from_payload(payload: &[u8], destination_hash: [u8; DESTINATION_LENGTH]) -> Packet {
    let mut data = PacketDataBuffer::new();
    data.write(payload).expect("payload write");
    Packet {
        header: Header {
            destination_type: DestinationType::Single,
            packet_type: PacketType::Data,
            ..Default::default()
        },
        ifac: None,
        destination: reticulum::hash::AddressHash::new(destination_hash),
        transport: None,
        context: PacketContext::None,
        data,
    }
}

fn expected_transient_id(lxm_data: &[u8]) -> [u8; 32] {
    Sha256::new().chain_update(lxm_data).finalize().into()
}

#[test]
fn transport_contract_decode_direct_round_trip() {
    let source = PrivateIdentity::new_from_name("direct-src");
    let destination = PrivateIdentity::new_from_name("direct-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Direct).unwrap();
    let packet = match result {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    let received = decode_packet(&packet, DeliveryMode::Direct).unwrap();
    let decoded = match received {
        Received::Message(m) => m,
        _ => panic!("expected message"),
    };

    assert_eq!(decoded.title, b"hi");
    assert_eq!(decoded.content, b"transport");
}

#[test]
fn transport_contract_decode_propagated_unstamped_hashes_full_payload() {
    let mut msg = make_message();
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("packed");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();
    let payload = msg
        .propagation_packed_from_encrypted(&encrypted_payload, 1_700_000_000.0, None)
        .expect("prop payload");

    let packet = packet_from_payload(&payload, msg.destination_hash);
    let received = decode_packet(&packet, DeliveryMode::Propagated).unwrap();
    let (timestamp, lxm_data, transient_id) = match received {
        Received::Propagated {
            timestamp,
            lxm_data,
            transient_id,
            source_hash: None,
            count_client_receive: true,
        } => (timestamp, lxm_data, transient_id),
        _ => panic!("expected propagated"),
    };

    assert_eq!(timestamp, 1_700_000_000.0);
    let mut expected_lxm = Vec::new();
    expected_lxm.extend_from_slice(&msg.destination_hash);
    expected_lxm.extend_from_slice(&encrypted_payload);
    assert_eq!(lxm_data, expected_lxm);

    let expected_hash = expected_transient_id(&expected_lxm);
    assert_eq!(transient_id, expected_hash);
}

#[test]
fn transport_contract_decode_propagated_stamped_hashes_with_stamp() {
    let mut msg = make_message();
    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let packed = msg.encode_signed(&signing_key).expect("packed");
    let encrypted_payload = packed[DESTINATION_LENGTH..].to_vec();

    msg.content = vec![b'a'; 200];
    let stamp = [5u8; STAMP_SIZE];
    let payload = msg
        .propagation_packed_from_encrypted(&encrypted_payload, 1_700_000_000.0, Some(&stamp))
        .expect("prop payload");

    let packet = packet_from_payload(&payload, msg.destination_hash);
    let received = decode_packet(&packet, DeliveryMode::Propagated).unwrap();
    let lxm_data = match received {
        Received::Propagated { lxm_data, .. } => lxm_data,
        _ => panic!("expected propagated"),
    };

    let mut expected_lxm = Vec::new();
    expected_lxm.extend_from_slice(&msg.destination_hash);
    expected_lxm.extend_from_slice(&encrypted_payload);
    expected_lxm.extend_from_slice(&stamp);
    assert_eq!(lxm_data, expected_lxm);

    let expected_hash = expected_transient_id(&expected_lxm);
    let received = decode_packet(&packet, DeliveryMode::Propagated).unwrap();
    let transient_id = match received {
        Received::Propagated { transient_id, .. } => transient_id,
        _ => panic!("expected propagated"),
    };
    assert_eq!(transient_id, expected_hash);
}

#[test]
fn transport_contract_direct_dedupes_by_message_id() {
    let source = PrivateIdentity::new_from_name("dedupe-src");
    let destination = PrivateIdentity::new_from_name("dedupe-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let mut msg = make_message();

    let result = outbound.deliver(&mut msg, DeliveryMode::Direct).unwrap();
    let packet = match result {
        DeliveryOutput::Packet(packet) => packet,
        _ => panic!("expected packet"),
    };

    let mut router = RnsRouter::new(outbound);
    router.receive_packet(&packet, DeliveryMode::Direct).unwrap();
    router.receive_packet(&packet, DeliveryMode::Direct).unwrap();

    let first = router.pop_inbound();
    let second = router.pop_inbound();
    assert!(first.is_some());
    assert!(second.is_none());
}
