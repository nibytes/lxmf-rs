#![cfg(feature = "rns")]

use lxmf_rs::{
    decode_pn_dir_packet, decode_pn_dir_request, decode_pn_dir_response, encode_pn_dir_request,
    encode_pn_dir_response, PnDirPacket, PnDirRequest, PnDirResponse, PnDirectory,
};
use lxmf_rs::{PnDirService, Value, PN_META_NAME};
use reticulum::hash::AddressHash;
use reticulum::packet::{Header, Packet, PacketContext, PacketDataBuffer, PacketType};

fn build_announce_data(name: &str, stamp_cost: i64) -> Vec<u8> {
    let metadata = Value::Map(vec![(
        Value::Integer((PN_META_NAME as i64).into()),
        Value::Binary(name.as_bytes().to_vec()),
    )]);

    let announce = Value::Array(vec![
        Value::Boolean(false),
        Value::Integer(1_700_000_000i64.into()),
        Value::Boolean(true),
        Value::Integer(64i64.into()),
        Value::Integer(128i64.into()),
        Value::Array(vec![
            Value::Integer(stamp_cost.into()),
            Value::Integer(2i64.into()),
            Value::Integer(3i64.into()),
        ]),
        metadata,
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &announce).expect("msgpack");
    buf
}

#[test]
fn pn_dir_request_round_trip() {
    let list = PnDirRequest::List;
    let bytes = encode_pn_dir_request(&list).expect("encode");
    let decoded = decode_pn_dir_request(&bytes).expect("decode");
    assert_eq!(decoded, list);

    let hash = [2u8; 16];
    let get = PnDirRequest::Get(hash);
    let bytes = encode_pn_dir_request(&get).expect("encode");
    let decoded = decode_pn_dir_request(&bytes).expect("decode");
    assert_eq!(decoded, get);
}

#[test]
fn pn_dir_response_round_trip() {
    let entry = lxmf_rs::PnDirEntryWire {
        destination_hash: [3u8; 16],
        app_data: build_announce_data("pn-one", 9),
        last_seen_ms: 123,
        acked: false,
    };
    let resp = PnDirResponse::List(vec![entry.clone()]);
    let bytes = encode_pn_dir_response(&resp).expect("encode");
    let decoded = decode_pn_dir_response(&bytes).expect("decode");
    assert_eq!(decoded, resp);

    let resp = PnDirResponse::Get(Some(entry));
    let bytes = encode_pn_dir_response(&resp).expect("encode");
    let decoded = decode_pn_dir_response(&bytes).expect("decode");
    assert_eq!(decoded, resp);
}

#[test]
fn pn_dir_service_handles_list_and_ack() {
    let mut directory = PnDirectory::new();
    let app_data = build_announce_data("pn-one", 9);
    let hash = [4u8; 16];
    directory
        .upsert_announce(hash, &app_data, 111)
        .expect("announce");

    let mut service = PnDirService::new(directory);
    let response = service.handle_request(&PnDirRequest::List, 200);
    match response {
        PnDirResponse::List(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].destination_hash, hash);
        }
        _ => panic!("expected list"),
    }

    let response = service.handle_request(&PnDirRequest::Ack(vec![hash]), 200);
    match response {
        PnDirResponse::Ack(acked) => assert_eq!(acked, vec![hash]),
        _ => panic!("expected ack"),
    }
}

#[test]
fn pn_dir_packet_decode_request() {
    let request = PnDirRequest::List;
    let payload = encode_pn_dir_request(&request).expect("encode");
    let mut data = PacketDataBuffer::new();
    data.write(&payload).expect("data");

    let packet = Packet {
        header: Header {
            packet_type: PacketType::Data,
            ..Default::default()
        },
        ifac: None,
        destination: AddressHash::new([0u8; 16]),
        transport: None,
        context: PacketContext::Request,
        data,
    };

    let decoded = decode_pn_dir_packet(&packet).expect("decode");
    match decoded {
        PnDirPacket::Request(req) => assert_eq!(req, request),
        _ => panic!("expected request"),
    }
}
