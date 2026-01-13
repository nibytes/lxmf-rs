#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("flush-test-src");
    let destination = PrivateIdentity::new_from_name("flush-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let mut router = LxmfRouter::new(RnsRouter::new(outbound));
    let now_ms = 1_700_000_000_000u64;
    router.enable_propagation_node(true, now_ms);
    router
}

fn make_peer_id(byte: u8) -> [u8; 16] {
    [byte; 16]
}

fn make_transient_id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

#[test]
fn test_flush_empty_queue() {
    let mut router = make_router();
    let _now_ms = 1_700_000_000_000u64;
    
    // Empty queue nothing does nothing
    router.flush_peer_distribution_queue();
    // After implementation verify that method is called without errors
    assert!(router.peers().len() == 0);
}

#[test]
fn test_flush_distributes_to_all_peers() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add several peers
    let peer1_id = make_peer_id(0x01);
    let peer2_id = make_peer_id(0x02);
    let peer3_id = make_peer_id(0x03);
    router.upsert_peer(peer1_id, now_ms);
    router.upsert_peer(peer2_id, now_ms);
    router.upsert_peer(peer3_id, now_ms);
    
    // Add transient_id to queue
    let transient_id = make_transient_id(0xAA);
    router.enqueue_peer_distribution(transient_id, None);
    
    // Flush should distribute to all peers
    router.flush_peer_distribution_queue();
    
    // Verify that all peers received transient_id in unhandled_messages_queue
    let peer1 = router.peers().get(&peer1_id).unwrap();
    let peer2 = router.peers().get(&peer2_id).unwrap();
    let peer3 = router.peers().get(&peer3_id).unwrap();
    assert!(peer1.unhandled_messages_queue().contains(&transient_id));
    assert!(peer2.unhandled_messages_queue().contains(&transient_id));
    assert!(peer3.unhandled_messages_queue().contains(&transient_id));
}

#[test]
fn test_flush_skips_source_peer() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers
    let source_peer_id = make_peer_id(0x01);
    let other_peer_id = make_peer_id(0x02);
    router.upsert_peer(source_peer_id, now_ms);
    router.upsert_peer(other_peer_id, now_ms);
    
    // Add transient_id with from_peer specified
    let transient_id = make_transient_id(0xBB);
    router.enqueue_peer_distribution(transient_id, Some(source_peer_id));
    
    // Flush should skip source_peer
    router.flush_peer_distribution_queue();
    
    // Verify that source_peer did NOT receive transient_id, and other_peer received
    let source_peer = router.peers().get(&source_peer_id).unwrap();
    let other_peer = router.peers().get(&other_peer_id).unwrap();
    assert!(!source_peer.unhandled_messages_queue().contains(&transient_id));
    assert!(other_peer.unhandled_messages_queue().contains(&transient_id));
}

#[test]
fn test_flush_handles_peer_removal() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add peer
    let peer_id = make_peer_id(0x01);
    router.upsert_peer(peer_id, now_ms);
    
    // Add transient_id to queue
    let transient_id = make_transient_id(0xCC);
    router.enqueue_peer_distribution(transient_id, None);
    
    // Remove peer before flush
    router.unpeer(peer_id);
    
    // Flush should handle this correctly
    router.flush_peer_distribution_queue();
    // After implementation verify that no errors
    assert!(router.peers().len() == 0);
}

#[test]
fn test_enqueue_peer_distribution() {
    let mut router = make_router();
    
    // Add transient_id to queue
    let transient_id = make_transient_id(0xDD);
    router.enqueue_peer_distribution(transient_id, None);
    
    // After implementation verify that transient_id added to queue
    // For now just verify that method is called without errors
    router.flush_peer_distribution_queue();
}

#[test]
fn test_flush_clears_queue() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add peer
    let peer_id = make_peer_id(0x01);
    router.upsert_peer(peer_id, now_ms);
    
    // Add transient_id to queue
    let transient_id = make_transient_id(0xEE);
    router.enqueue_peer_distribution(transient_id, None);
    
    // Flush should clear queue
    router.flush_peer_distribution_queue();
    
    // Second flush should not do anything
    router.flush_peer_distribution_queue();
    // After implementation verify that queue is empty
    assert!(router.peers().len() == 1);
}

#[test]
fn test_flush_with_multiple_entries() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add peers
    let peer1_id = make_peer_id(0x01);
    let peer2_id = make_peer_id(0x02);
    router.upsert_peer(peer1_id, now_ms);
    router.upsert_peer(peer2_id, now_ms);
    
    // Add several transient_id to queue
    let transient_id1 = make_transient_id(0x01);
    let transient_id2 = make_transient_id(0x02);
    let transient_id3 = make_transient_id(0x03);
    router.enqueue_peer_distribution(transient_id1, None);
    router.enqueue_peer_distribution(transient_id2, None);
    router.enqueue_peer_distribution(transient_id3, None);
    
    // Flush should distribute all transient_id to all peers
    router.flush_peer_distribution_queue();
    
    // Verify that all peers received all transient_id
    let peer1 = router.peers().get(&peer1_id).unwrap();
    let peer2 = router.peers().get(&peer2_id).unwrap();
    assert!(peer1.unhandled_messages_queue().contains(&transient_id1));
    assert!(peer1.unhandled_messages_queue().contains(&transient_id2));
    assert!(peer1.unhandled_messages_queue().contains(&transient_id3));
    assert!(peer2.unhandled_messages_queue().contains(&transient_id1));
    assert!(peer2.unhandled_messages_queue().contains(&transient_id2));
    assert!(peer2.unhandled_messages_queue().contains(&transient_id3));
}

#[test]
fn test_flush_calls_queue_unhandled_message() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add peer
    let peer_id = make_peer_id(0x01);
    router.upsert_peer(peer_id, now_ms);
    
    // Add transient_id to queue
    let transient_id = make_transient_id(0xFF);
    router.enqueue_peer_distribution(transient_id, None);
    
    // Flush should call queue_unhandled_message on peer
    router.flush_peer_distribution_queue();
    
    // Verify that peer.queue_unhandled_message was called
    let peer = router.peers().get(&peer_id).unwrap();
    assert!(peer.unhandled_messages_queue().contains(&transient_id));
    assert_eq!(peer.unhandled_messages_queue().len(), 1);
}
