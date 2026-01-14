#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter, DESTINATION_LENGTH};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("test-src");
    let destination = PrivateIdentity::new_from_name("test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

#[test]
fn test_outbound_propagation_node_defaults_to_none() {
    let router = make_router();
    assert_eq!(router.get_outbound_propagation_node(), None);
}

#[test]
fn test_set_outbound_propagation_node() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    router.set_outbound_propagation_node(Some(node_hash));
    assert_eq!(router.get_outbound_propagation_node(), Some(node_hash));
}

#[test]
fn test_set_outbound_propagation_node_to_none() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    router.set_outbound_propagation_node(Some(node_hash));
    assert_eq!(router.get_outbound_propagation_node(), Some(node_hash));
    router.set_outbound_propagation_node(None);
    assert_eq!(router.get_outbound_propagation_node(), None);
}

#[test]
fn test_propagation_transfer_max_messages_defaults_to_none() {
    let router = make_router();
    assert_eq!(router.propagation_transfer_max_messages(), None);
}

#[test]
fn test_set_propagation_transfer_max_messages() {
    let mut router = make_router();
    router.set_propagation_transfer_max_messages(Some(100));
    assert_eq!(router.propagation_transfer_max_messages(), Some(100));
    router.set_propagation_transfer_max_messages(None);
    assert_eq!(router.propagation_transfer_max_messages(), None);
}

#[test]
fn test_propagation_transfer_last_duplicates_defaults_to_none() {
    let router = make_router();
    assert_eq!(router.propagation_transfer_last_duplicates(), None);
}

#[test]
fn test_set_propagation_transfer_last_duplicates() {
    let mut router = make_router();
    router.set_propagation_transfer_last_duplicates(Some(5));
    assert_eq!(router.propagation_transfer_last_duplicates(), Some(5));
    router.set_propagation_transfer_last_duplicates(None);
    assert_eq!(router.propagation_transfer_last_duplicates(), None);
}

#[test]
fn test_set_outbound_propagation_node_updates_link() {
    let mut router = make_router();
    let node_hash1 = [0x42u8; DESTINATION_LENGTH];
    let node_hash2 = [0x43u8; DESTINATION_LENGTH];
    
    // Set first node
    router.set_outbound_propagation_node(Some(node_hash1));
    assert_eq!(router.get_outbound_propagation_node(), Some(node_hash1));
    
    // Change to second node (should teardown first link if exists)
    router.set_outbound_propagation_node(Some(node_hash2));
    assert_eq!(router.get_outbound_propagation_node(), Some(node_hash2));
}
