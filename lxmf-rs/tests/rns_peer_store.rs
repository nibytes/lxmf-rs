#![cfg(feature = "rns")]

use lxmf_rs::{PeerState, RnsOutbound, RuntimeConfig};
use reticulum::identity::PrivateIdentity;

#[test]
fn peer_store_round_trip() {
    let root = std::env::temp_dir().join("lxmf-peer-store-test");
    let _ = std::fs::remove_dir_all(&root);

    let source = PrivateIdentity::new_from_name("peer-store-src");
    let dest = PrivateIdentity::new_from_name("peer-store-dest").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let mut router = lxmf_rs::LxmfRouter::with_store_and_root(
        outbound,
        Box::new(lxmf_rs::FileMessageStore::new(&root)),
        &root,
        RuntimeConfig::default(),
    );

    let peer_id = [9u8; 16];
    let peer = router.upsert_peer(peer_id, 1234);
    peer.state = PeerState::OfferSent;
    peer.backoff_ms = 500;
    router.save_peers(&root);

    let source = PrivateIdentity::new_from_name("peer-store-src2");
    let dest = PrivateIdentity::new_from_name("peer-store-dest2").as_identity().clone();
    let outbound = RnsOutbound::new(source, dest);
    let router = lxmf_rs::LxmfRouter::with_store_and_root(
        outbound,
        Box::new(lxmf_rs::FileMessageStore::new(&root)),
        &root,
        RuntimeConfig::default(),
    );

    let restored = router.peers().get(&peer_id).expect("peer");
    assert_eq!(restored.state, PeerState::OfferSent);
    assert_eq!(restored.backoff_ms, 500);
}
