#![cfg(feature = "rns")]

use lxmf_rs::{Peer, DEFAULT_SYNC_STRATEGY, STRATEGY_LAZY, STRATEGY_PERSISTENT};
use std::collections::HashMap;

#[test]
fn test_peer_sync_strategy() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be DEFAULT_SYNC_STRATEGY (STRATEGY_PERSISTENT)
    assert_eq!(peer.sync_strategy(), DEFAULT_SYNC_STRATEGY);
    
    // Set to STRATEGY_LAZY
    peer.set_sync_strategy(STRATEGY_LAZY);
    assert_eq!(peer.sync_strategy(), STRATEGY_LAZY);
    
    // Set to STRATEGY_PERSISTENT
    peer.set_sync_strategy(STRATEGY_PERSISTENT);
    assert_eq!(peer.sync_strategy(), STRATEGY_PERSISTENT);
}

#[test]
fn test_peer_peering_cost() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.peering_cost(), None);
    
    // Set peering cost
    peer.set_peering_cost(Some(18));
    assert_eq!(peer.peering_cost(), Some(18));
    
    // Clear peering cost
    peer.set_peering_cost(None);
    assert_eq!(peer.peering_cost(), None);
}

#[test]
fn test_peer_metadata() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.metadata(), None);
    
    // Set metadata
    let mut metadata = HashMap::new();
    metadata.insert("name".to_string(), b"test-peer".to_vec());
    peer.set_metadata(Some(metadata.clone()));
    assert_eq!(peer.metadata(), Some(&metadata));
    
    // Clear metadata
    peer.set_metadata(None);
    assert_eq!(peer.metadata(), None);
}

#[test]
fn test_peer_peering_timebase() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be 0
    assert_eq!(peer.peering_timebase(), 0);
    
    // Set peering timebase
    peer.set_peering_timebase(1234567890);
    assert_eq!(peer.peering_timebase(), 1234567890);
}

#[test]
fn test_peer_link_establishment_rate() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be 0.0
    assert_eq!(peer.link_establishment_rate(), 0.0);
    
    // Set link establishment rate
    peer.set_link_establishment_rate(100.5);
    assert_eq!(peer.link_establishment_rate(), 100.5);
}

#[test]
fn test_peer_propagation_transfer_limit() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.propagation_transfer_limit(), None);
    
    // Set propagation transfer limit
    peer.set_propagation_transfer_limit(Some(256.0));
    assert_eq!(peer.propagation_transfer_limit(), Some(256.0));
    
    // Clear propagation transfer limit
    peer.set_propagation_transfer_limit(None);
    assert_eq!(peer.propagation_transfer_limit(), None);
}

#[test]
fn test_peer_propagation_sync_limit() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.propagation_sync_limit(), None);
    
    // Set propagation sync limit
    peer.set_propagation_sync_limit(Some(10240));
    assert_eq!(peer.propagation_sync_limit(), Some(10240));
    
    // Clear propagation sync limit
    peer.set_propagation_sync_limit(None);
    assert_eq!(peer.propagation_sync_limit(), None);
}

#[test]
fn test_peer_propagation_stamp_cost() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.propagation_stamp_cost(), None);
    
    // Set propagation stamp cost
    peer.set_propagation_stamp_cost(Some(16));
    assert_eq!(peer.propagation_stamp_cost(), Some(16));
    
    // Clear propagation stamp cost
    peer.set_propagation_stamp_cost(None);
    assert_eq!(peer.propagation_stamp_cost(), None);
}

#[test]
fn test_peer_propagation_stamp_cost_flexibility() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.propagation_stamp_cost_flexibility(), None);
    
    // Set propagation stamp cost flexibility
    peer.set_propagation_stamp_cost_flexibility(Some(3));
    assert_eq!(peer.propagation_stamp_cost_flexibility(), Some(3));
    
    // Clear propagation stamp cost flexibility
    peer.set_propagation_stamp_cost_flexibility(None);
    assert_eq!(peer.propagation_stamp_cost_flexibility(), None);
}

#[test]
fn test_peer_currently_transferring_messages() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.currently_transferring_messages(), None);
    
    // Set currently transferring messages
    let messages = vec![[0x11u8; 32], [0x22u8; 32]];
    peer.set_currently_transferring_messages(Some(messages.clone()));
    assert_eq!(peer.currently_transferring_messages(), Some(&messages));
    
    // Clear currently transferring messages
    peer.set_currently_transferring_messages(None);
    assert_eq!(peer.currently_transferring_messages(), None);
}

#[test]
fn test_peer_current_sync_transfer_started() {
    let mut peer = Peer::new([0x42u8; 16], 1000);
    
    // Default should be None
    assert_eq!(peer.current_sync_transfer_started(), None);
    
    // Set current sync transfer started
    peer.set_current_sync_transfer_started(Some(1234567890));
    assert_eq!(peer.current_sync_transfer_started(), Some(1234567890));
    
    // Clear current sync transfer started
    peer.set_current_sync_transfer_started(None);
    assert_eq!(peer.current_sync_transfer_started(), None);
}
