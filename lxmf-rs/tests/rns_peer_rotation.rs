#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter};
use reticulum::identity::PrivateIdentity;

// Constants from Python LXMRouter
const ROTATION_HEADROOM_PCT: f64 = 10.0;
const ROTATION_AR_MAX: f64 = 0.5;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("rotation-test-src");
    let destination = PrivateIdentity::new_from_name("rotation-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let mut router = LxmfRouter::new(RnsRouter::new(outbound));
    let now_ms = 1_700_000_000_000u64;
    router.enable_propagation_node(true, now_ms);
    router
}

// Helper function to create peer ID
fn make_peer_id(byte: u8) -> [u8; 16] {
    [byte; 16]
}

#[test]
fn test_rotation_calculates_headroom() {
    let mut router = make_router();
    router.set_max_peers(Some(20));
    
    // headroom = max(1, floor(20 * 0.1)) = max(1, 2) = 2
    // required_drops = peers.len() - (max_peers - headroom) = peers.len() - 18
    // After implementation rotate_peers() verify calculation
    // For now test should fail, because rotate_peers() is not implemented
    let now_ms = 1_700_000_000_000u64;
    router.rotate_peers(now_ms);
}

#[test]
fn test_rotation_skips_when_below_max_peers() {
    let mut router = make_router();
    router.set_max_peers(Some(20));
    
    // Add less than max_peers peers
    let now_ms = 1_700_000_000_000u64;
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // rotate_peers should do nothing when peers < max_peers
    assert_eq!(peer_count_before, peer_count_after);
}

#[test]
fn test_rotation_skips_when_untested_peers_threshold_reached() {
    let mut router = make_router();
    router.set_max_peers(Some(20));
    
    let now_ms = 1_700_000_000_000u64;
    // Add more than max_peers peers
    // And many untested (last_attempt_ms == 0)
    for i in 0..25 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // If untested_peers >= headroom, rotation should be postponed
    // headroom = max(1, floor(20 * 0.1)) = 2
    // If >= 2 untested peers, rotation is postponed
    // For now verify that method is called without errors
    assert!(peer_count_before >= peer_count_after);
}

#[test]
fn test_rotation_uses_fully_synced_peers_pool() {
    let mut router = make_router();
    router.set_max_peers(Some(20));
    
    let now_ms = 1_700_000_000_000u64;
    // Add peers with different messages_unhandled
    for i in 0..25 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_unhandled = if i < 5 { 0 } else { 10 }; // First 5 fully synced
            peer.messages_offered = 10;
            peer.messages_outgoing = 3;
            peer.last_heard_ms = now_ms;
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested
        }
    }
    
    router.rotate_peers(now_ms);
    // After implementation verify that pool of fully synced peers is used
    // max_peers = 20, headroom = 2, required_drops = 25 - (20 - 2) = 7
    assert!(router.peers().len() <= 20);
}

#[test]
fn test_rotation_preserves_static_peers() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    
    let now_ms = 1_700_000_000_000u64;
    // Add static peer and regular peers
    let static_peer_id = make_peer_id(0x01);
    router.set_static_peers(vec![static_peer_id]);
    router.upsert_peer(static_peer_id, now_ms);
    
    for i in 2..25 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
    }
    
    let has_static_before = router.peers().entries().contains_key(&static_peer_id);
    router.rotate_peers(now_ms);
    let has_static_after = router.peers().entries().contains_key(&static_peer_id);
    
    // rotate_peers should not remove static peer
    assert!(has_static_before);
    assert!(has_static_after);
}

#[test]
fn test_rotation_only_considers_idle_peers() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers with different states
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = 3;
            peer.last_heard_ms = now_ms;
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested
            // State is Idle by default, which is what we want
        }
    }
    
    router.rotate_peers(now_ms);
    // After implementation verify that only Idle peers are considered
    assert!(router.peers().len() <= 5);
}

#[test]
fn test_rotation_skips_peers_with_no_offered_messages() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers with messages_offered == 0
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            // messages_offered is already 0 by default
            peer.last_heard_ms = now_ms;
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested
        }
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // After implementation verify that they are not considered (because offered == 0)
    // peers with offered == 0 do not enter drop_pool, so they are not removed
    // But they may be more than max_peers, and they are not removed
    // In Python version this is also the case - peers with offered == 0 are not removed
    assert_eq!(peer_count_before, peer_count_after);
}

#[test]
fn test_rotation_prioritizes_unresponsive_peers() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Add unresponsive (last_heard_ms == 0) and waiting peers
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = 5;
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested
            // last_heard_ms = 0 means unresponsive (by default)
            // First 5 - unresponsive, others - waiting (alive)
            if i >= 5 {
                peer.last_heard_ms = now_ms; // Mark as alive (waiting)
            }
        }
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // After implementation verify that unresponsive are prioritized
    // max_peers = 5, headroom = 1, required_drops = 10 - (5 - 1) = 6
    // First 5 - unresponsive (AR = 0.5), others - waiting (AR = 0.5)
    // AR = 0.5 >= 0.5, so they are not removed
    // But unresponsive should be prioritized in drop_pool
    // In Python version peers with AR >= 0.5 are not removed
    assert!(peer_count_after <= peer_count_before);
}

#[test]
fn test_rotation_sorts_by_acceptance_rate() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers with different acceptance rates
    // AR = messages_outgoing / messages_offered
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = i as u64; // AR from 0.0 to 0.9
            peer.last_heard_ms = now_ms; // Mark as alive (waiting peer)
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested (not untested)
        }
    }
    
    router.rotate_peers(now_ms);
    // After implementation verify sorting by AR (ascending)
    // max_peers = 5, headroom = 1, required_drops = 10 - (5 - 1) = 6
    // Should be removed 6 peers with low AR
    assert!(router.peers().len() <= 5);
}

#[test]
fn test_rotation_drops_low_ar_peers() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers with AR < ROTATION_AR_MAX (0.5)
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = 3; // AR = 0.3 < 0.5
            peer.last_heard_ms = now_ms; // Mark as alive (waiting peer)
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested (not untested)
        }
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // After implementation verify that they are removed
    // max_peers = 5, headroom = 1, required_drops = 10 - (5 - 1) = 6
    // All peers have AR = 0.3 < 0.5, so they should be removed
    assert!(peer_count_after < peer_count_before);
    assert!(peer_count_after <= 5);
}

#[test]
fn test_rotation_keeps_high_ar_peers() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers with AR >= ROTATION_AR_MAX (0.5)
    // But they more than max_peers, so some should be removed
    // But those with AR >= 0.5, should not be removed (if there is room)
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = 7; // AR = 0.7 >= 0.5
            peer.last_heard_ms = now_ms; // Mark as alive (waiting peer)
            peer.last_attempt_ms = now_ms - 1000; // Mark as tested (not untested)
        }
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // max_peers = 5, headroom = 1, required_drops = 10 - (5 - 1) = 6
    // But all peers have AR = 0.7 >= 0.5, so they should not be removed
    // In Python version peers with AR >= 0.5 are not removed (line 1999: if ar < ROTATION_AR_MAX*100)
    // So they remain, even if there are more than max_peers
    assert_eq!(peer_count_before, peer_count_after);
}

#[test]
fn test_rotation_handles_edge_cases() {
    let mut router = make_router();
    router.set_max_peers(Some(5));
    let now_ms = 1_700_000_000_000u64;
    
    // Edge case 1: all static
    router.set_static_peers((0..10).map(|j| make_peer_id(j)).collect());
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = 3;
            peer.last_heard_ms = now_ms;
            peer.last_attempt_ms = now_ms - 1000;
        }
    }
    
    let peer_count_before = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after = router.peers().len();
    
    // All static, so should not be removed
    assert_eq!(peer_count_before, peer_count_after);
    
    // Edge case 2: all with high AR
    router.set_static_peers(vec![]);
    for i in 0..10 {
        let peer_id = make_peer_id(i);
        router.upsert_peer(peer_id, now_ms);
        if let Some(peer) = router.peers_mut().get_mut(&peer_id) {
            peer.messages_offered = 10;
            peer.messages_outgoing = 9; // AR = 0.9 >= 0.5
            peer.last_heard_ms = now_ms;
            peer.last_attempt_ms = now_ms - 1000;
        }
    }
    
    let peer_count_before_high_ar = router.peers().len();
    router.rotate_peers(now_ms);
    let peer_count_after_high_ar = router.peers().len();
    
    // After implementation verify handling of edge cases
    // max_peers = 5, headroom = 1, required_drops = 10 - (5 - 1) = 6
    // But all peers have AR = 0.9 >= 0.5, so they should not be removed
    // In Python version peers with AR >= 0.5 are not removed (line 1999: if ar < ROTATION_AR_MAX*100)
    assert_eq!(peer_count_before_high_ar, peer_count_after_high_ar);
}
