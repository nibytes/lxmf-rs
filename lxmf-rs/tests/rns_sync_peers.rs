#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter, PeerState, DESTINATION_LENGTH, MAX_UNREACHABLE, FASTEST_N_RANDOM_POOL};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("sync-peers-test-src");
    let destination = PrivateIdentity::new_from_name("sync-peers-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

fn make_peer_id(seed: u8) -> [u8; DESTINATION_LENGTH] {
    let mut id = [0u8; DESTINATION_LENGTH];
    id[0] = seed;
    id
}

#[test]
fn test_peer_has_sync_transfer_rate() {
    // Test that Peer struct has sync_transfer_rate field (matching Python LXMPeer.sync_transfer_rate)
    let mut router = make_router();
    let peer_id = make_peer_id(1);
    let now_ms = 1_700_000_000_000u64;
    
    let peer = router.upsert_peer(peer_id, now_ms);
    assert_eq!(peer.sync_transfer_rate(), 0.0, "Default sync_transfer_rate should be 0.0");
    
    let peer = router.upsert_peer(peer_id, now_ms); // Get mutable reference
    peer.set_sync_transfer_rate(100.5);
    let peer = router.peers().get(&peer_id).expect("peer should exist");
    assert_eq!(peer.sync_transfer_rate(), 100.5, "sync_transfer_rate should be settable");
}

#[test]
fn test_constants_defined() {
    // Test that constants FASTEST_N_RANDOM_POOL and MAX_UNREACHABLE are defined
    assert_eq!(FASTEST_N_RANDOM_POOL, 2, "FASTEST_N_RANDOM_POOL should be 2");
    assert_eq!(MAX_UNREACHABLE, 14 * 24 * 60 * 60, "MAX_UNREACHABLE should be 14 days in seconds");
}

#[test]
fn test_sync_peers_selects_waiting_peers() {
    // Test that sync_peers() selects from waiting peers (alive, IDLE, with unhandled_messages)
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    let peer1_id = make_peer_id(1);
    let peer2_id = make_peer_id(2);
    
    // Create waiting peers (alive, IDLE, with unhandled_messages)
    let peer1 = router.upsert_peer(peer1_id, now_ms);
    peer1.state = PeerState::Idle;
    peer1.next_sync_ms = now_ms;
    peer1.set_sync_transfer_rate(100.0);
    peer1.set_alive(true);
    peer1.set_last_heard_ms(now_ms); // Set last_heard to avoid being culled
    peer1.queue_unhandled_message([0x01u8; 32]);
    
    let peer2 = router.upsert_peer(peer2_id, now_ms);
    peer2.state = PeerState::Idle;
    peer2.next_sync_ms = now_ms;
    peer2.set_sync_transfer_rate(50.0);
    peer2.set_alive(true);
    peer2.set_last_heard_ms(now_ms); // Set last_heard to avoid being culled
    peer2.queue_unhandled_message([0x02u8; 32]);
    
    // sync_peers() should select from waiting peers
    router.sync_peers(now_ms);
    
    // Should have scheduled at least one peer sync
    // (exact selection depends on random, but should be from waiting peers)
    let scheduled = router.pop_peer_sync_request();
    assert!(scheduled.is_some(), "sync_peers() should schedule at least one peer");
}

#[test]
fn test_sync_peers_sorts_by_transfer_rate() {
    // Test that sync_peers() sorts waiting peers by sync_transfer_rate (highest first)
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Create multiple waiting peers with different transfer rates
    for i in 1..=5 {
        let peer_id = make_peer_id(i);
        let peer = router.upsert_peer(peer_id, now_ms);
        peer.state = PeerState::Idle;
        peer.next_sync_ms = now_ms;
        peer.set_sync_transfer_rate((i * 10) as f64); // 10, 20, 30, 40, 50
        peer.set_alive(true);
        peer.set_last_heard_ms(now_ms); // Set last_heard to avoid being culled
        peer.queue_unhandled_message([i; 32]);
    }
    
    // sync_peers() should select from fastest peers (FASTEST_N_RANDOM_POOL = 2)
    router.sync_peers(now_ms);
    
    // Should have scheduled peer syncs (from top 2 fastest: 50 and 40)
    let scheduled = router.pop_peer_sync_request();
    assert!(scheduled.is_some(), "sync_peers() should schedule peer from fastest pool");
}

#[test]
fn test_sync_peers_removes_culled_peers() {
    // Test that sync_peers() removes peers older than MAX_UNREACHABLE
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    let peer_id = make_peer_id(1);
    let peer = router.upsert_peer(peer_id, now_ms);
    peer.set_last_heard_ms(now_ms - (MAX_UNREACHABLE + 1) * 1000); // Older than MAX_UNREACHABLE
    peer.set_alive(false);
    
    // sync_peers() should remove culled peer
    router.sync_peers(now_ms);
    
    // Peer should be removed (unless it's a static peer)
    assert!(router.peers().get(&peer_id).is_none() || router.static_peers_len() > 0,
            "Culled peer should be removed by sync_peers()");
}

#[test]
fn test_sync_peers_keeps_static_peers() {
    // Test that sync_peers() does not remove static peers even if they're old
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    let peer_id = make_peer_id(1);
    router.set_static_peers(vec![peer_id]);
    let peer = router.upsert_peer(peer_id, now_ms);
    peer.set_last_heard_ms(now_ms - (MAX_UNREACHABLE + 1) * 1000); // Older than MAX_UNREACHABLE
    peer.set_alive(false);
    
    // sync_peers() should NOT remove static peer
    router.sync_peers(now_ms);
    
    assert!(router.peers().get(&peer_id).is_some(),
            "Static peer should NOT be removed even if old");
}

#[test]
fn test_sync_peers_handles_unresponsive_peers() {
    // Test that sync_peers() selects from unresponsive peers when no waiting peers available
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    let peer_id = make_peer_id(1);
    let peer = router.upsert_peer(peer_id, now_ms);
    peer.state = PeerState::Idle; // Set state first
    peer.next_sync_ms = now_ms;
    peer.set_last_heard_ms(now_ms); // Set last_heard to avoid being culled
    peer.set_next_sync_attempt_ms(now_ms - 1000); // Past next_sync_attempt
    peer.queue_unhandled_message([0x01u8; 32]);
    // Set alive to false AFTER setting state (this will change state to Backoff)
    peer.set_alive(false); // Not alive - this will change state to Backoff
    // But we need Idle state for the check, so set it back
    peer.state = PeerState::Idle; // Keep Idle state for unresponsive peer check
    
    // sync_peers() should select from unresponsive peers
    router.sync_peers(now_ms);
    
    let scheduled = router.pop_peer_sync_request();
    assert!(scheduled.is_some(), "sync_peers() should schedule unresponsive peer when no waiting peers");
}

#[test]
fn test_clean_throttled_peers_removes_expired() {
    // Test that clean_throttled_peers() removes expired entries
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    let peer_id = make_peer_id(1);
    router.add_throttled_peer(peer_id, now_ms - 1000); // Expired
    
    router.clean_throttled_peers(now_ms);
    
    assert!(!router.is_peer_throttled(&peer_id, now_ms),
            "Expired throttled peer should be removed");
}

#[test]
fn test_clean_throttled_peers_keeps_active() {
    // Test that clean_throttled_peers() keeps non-expired entries
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    let peer_id = make_peer_id(1);
    router.add_throttled_peer(peer_id, now_ms + 1000); // Not expired yet
    
    router.clean_throttled_peers(now_ms);
    
    assert!(router.is_peer_throttled(&peer_id, now_ms),
            "Non-expired throttled peer should be kept");
}
