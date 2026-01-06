#![cfg(feature = "rns")]

use lxmf_rs::{Peer, PeerState, PeerTable, PeerSyncResponse};

#[test]
fn peer_transitions_offer_to_payload_and_complete() {
    let mut peer = Peer::new([1u8; 16], 0);
    assert_eq!(peer.state, PeerState::Idle);

    peer.on_offer_sent(100);
    assert_eq!(peer.state, PeerState::OfferSent);

    peer.on_offer_response(&PeerSyncResponse::IdList(vec![[9u8; 32]]), 150);
    assert_eq!(peer.state, PeerState::AwaitingPayload);

    peer.on_payload_complete(200);
    assert_eq!(peer.state, PeerState::Idle);
    assert_eq!(peer.last_sync_ms, 200);
}

#[test]
fn peer_backoff_increases_on_timeouts() {
    let mut peer = Peer::new([2u8; 16], 0);
    peer.on_timeout(1000);
    assert_eq!(peer.state, PeerState::Backoff);
    let first_next = peer.next_sync_ms;

    peer.on_timeout(2000);
    assert!(peer.next_sync_ms > first_next);
}

#[test]
fn peer_table_rotation_prefers_earliest_due() {
    let mut table = PeerTable::new();
    let peer_a = table.upsert([3u8; 16], 0);
    peer_a.next_sync_ms = 500;
    let peer_b = table.upsert([4u8; 16], 0);
    peer_b.next_sync_ms = 200;

    let next = table.next_ready(250).expect("peer");
    assert_eq!(next, [4u8; 16]);
}
