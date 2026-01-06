#![cfg(feature = "rns")]

use lxmf_rs::{encode_peer_sync_request, PeerSyncRequest};

#[test]
fn golden_peer_sync_offer_matches_python_umsgpack() {
    let peering_key: Vec<u8> = (1u8..=32).collect();
    let available = vec![[0x10u8; 32], [0x20u8; 32]];
    let req = PeerSyncRequest::Offer {
        peering_key,
        available,
    };

    let bytes = encode_peer_sync_request(&req).expect("encode");
    let expected = hex::decode(
        "92c4200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2092c4201010101010101010101010101010101010101010101010101010101010101010c4202020202020202020202020202020202020202020202020202020202020202020",
    )
    .expect("hex");

    assert_eq!(bytes, expected);
}
