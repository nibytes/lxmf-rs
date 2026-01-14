#![cfg(feature = "rns")]

use lxmf_rs::{Peer, DEFAULT_SYNC_STRATEGY, STRATEGY_LAZY};
use std::collections::HashMap;

#[test]
fn test_peer_to_bytes_and_from_bytes() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Set various fields
    peer.set_peering_timebase(1234567890);
    peer.set_last_heard_ms(2000);
    peer.set_alive(true);
    peer.set_sync_strategy(STRATEGY_LAZY);
    peer.set_link_establishment_rate(100.5);
    peer.set_sync_transfer_rate(500.0);
    peer.set_propagation_transfer_limit(Some(256.0));
    peer.set_propagation_sync_limit(Some(10240));
    peer.set_propagation_stamp_cost(Some(16));
    peer.set_propagation_stamp_cost_flexibility(Some(3));
    peer.set_peering_cost(Some(18));
    
    // Set metadata
    let mut metadata = HashMap::new();
    metadata.insert("name".to_string(), b"test-peer".to_vec());
    peer.set_metadata(Some(metadata.clone()));
    
    // Set peering_key (in Python it's [key_bytes, value], but in Rust we store just bytes)
    peer.peering_key = Some(b"test-key".to_vec());
    
    // Serialize
    let bytes = peer.to_bytes().unwrap();
    assert!(!bytes.is_empty());
    
    // Deserialize
    let restored = Peer::from_bytes(&bytes, 1000).unwrap();
    
    // Verify fields
    assert_eq!(restored.id, peer.id);
    assert_eq!(restored.peering_timebase(), peer.peering_timebase());
    assert_eq!(restored.last_heard_ms(), peer.last_heard_ms());
    assert_eq!(restored.is_alive(), peer.is_alive());
    assert_eq!(restored.sync_strategy(), peer.sync_strategy());
    assert_eq!(restored.link_establishment_rate(), peer.link_establishment_rate());
    assert_eq!(restored.sync_transfer_rate(), peer.sync_transfer_rate());
    assert_eq!(restored.propagation_transfer_limit(), peer.propagation_transfer_limit());
    assert_eq!(restored.propagation_sync_limit(), peer.propagation_sync_limit());
    assert_eq!(restored.propagation_stamp_cost(), peer.propagation_stamp_cost());
    assert_eq!(restored.propagation_stamp_cost_flexibility(), peer.propagation_stamp_cost_flexibility());
    assert_eq!(restored.peering_cost(), peer.peering_cost());
    assert_eq!(restored.metadata(), peer.metadata());
    assert_eq!(restored.peering_key, peer.peering_key);
}

#[test]
fn test_peer_to_bytes_minimal() {
    let peer = Peer::new([0x42u8; 16], 1000);
    
    // Serialize minimal peer
    let bytes = peer.to_bytes().unwrap();
    assert!(!bytes.is_empty());
    
    // Deserialize
    let restored = Peer::from_bytes(&bytes, 1000).unwrap();
    
    // Verify defaults
    assert_eq!(restored.id, peer.id);
    assert_eq!(restored.peering_timebase(), 0);
    assert_eq!(restored.last_heard_ms(), 0);
    assert_eq!(restored.sync_strategy(), DEFAULT_SYNC_STRATEGY);
    assert_eq!(restored.link_establishment_rate(), 0.0);
    assert_eq!(restored.sync_transfer_rate(), 0.0);
    assert_eq!(restored.propagation_transfer_limit(), None);
    assert_eq!(restored.propagation_sync_limit(), None);
    assert_eq!(restored.propagation_stamp_cost(), None);
    assert_eq!(restored.propagation_stamp_cost_flexibility(), None);
    assert_eq!(restored.peering_cost(), None);
    assert_eq!(restored.metadata(), None);
}

#[test]
fn test_peer_peering_key_ready_without_cost() {
    let peer = Peer::new([0x42u8; 16], 1000);
    
    // Without peering_cost, should return false
    assert_eq!(peer.peering_key_ready(), false);
}

#[test]
fn test_peer_peering_key_ready_without_key() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    peer.set_peering_cost(Some(18));
    
    // Without peering_key, should return false
    assert_eq!(peer.peering_key_ready(), false);
}

#[test]
fn test_peer_peering_key_ready_with_valid_key() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    peer.set_peering_cost(Some(18));
    
    // Set peering_key and value (matching Python: peering_key = [key_bytes, value])
    peer.peering_key = Some(b"test-key".to_vec());
    peer.peering_key_value = Some(20); // value >= cost (18)
    
    assert_eq!(peer.peering_key_ready(), true);
    
    // Value less than cost should return false
    peer.peering_key_value = Some(15); // value < cost (18)
    assert_eq!(peer.peering_key_ready(), false);
}

#[test]
fn test_peer_peering_key_value_without_key() {
    let peer = Peer::new([0x42u8; 16], 1000);
    
    // Without peering_key, should return None
    assert_eq!(peer.peering_key_value(), None);
}

#[test]
fn test_peer_peering_key_value_with_key() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Set peering_key and value (matching Python: peering_key = [key_bytes, value])
    peer.peering_key = Some(b"test-key".to_vec());
    peer.peering_key_value = Some(20);
    
    assert_eq!(peer.peering_key_value(), Some(20));
}

#[test]
fn test_peer_acceptance_rate() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default: no messages offered, acceptance rate should be 0 or undefined
    // In Python: acceptance_rate = outgoing / offered if offered > 0 else 0
    peer.messages_offered = 0;
    peer.messages_outgoing = 0;
    assert_eq!(peer.acceptance_rate(), 0.0);
    
    // With messages offered and accepted
    peer.messages_offered = 100;
    peer.messages_outgoing = 50;
    assert_eq!(peer.acceptance_rate(), 0.5);
    
    // All messages accepted
    peer.messages_offered = 100;
    peer.messages_outgoing = 100;
    assert_eq!(peer.acceptance_rate(), 1.0);
    
    // No messages accepted
    peer.messages_offered = 100;
    peer.messages_outgoing = 0;
    assert_eq!(peer.acceptance_rate(), 0.0);
}
