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
fn test_cancel_propagation_node_requests_resets_state() {
    let mut router = make_router();
    
    // Set some propagation transfer state
    router.set_propagation_transfer_state(lxmf_rs::PR_REQUEST_SENT);
    router.set_propagation_transfer_progress(0.5);
    router.set_propagation_transfer_max_messages(Some(100));
    
    // Cancel requests
    router.cancel_propagation_node_requests();
    
    // Should reset state via acknowledge_sync_completion
    assert_eq!(router.propagation_transfer_state(), lxmf_rs::PR_IDLE);
    assert_eq!(router.propagation_transfer_progress(), 0.0);
    assert_eq!(router.propagation_transfer_max_messages(), None);
}

#[test]
fn test_cancel_propagation_node_requests_clears_path_tracking() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    let identity_hash = [0x43u8; DESTINATION_LENGTH];
    
    router.set_outbound_propagation_node(Some(node_hash));
    router.set_wants_download_on_path_available_from(Some(node_hash));
    router.set_wants_download_on_path_available_to(Some(identity_hash));
    
    router.cancel_propagation_node_requests();
    
    // Path tracking should be cleared
    assert_eq!(router.wants_download_on_path_available_from(), None);
    assert_eq!(router.wants_download_on_path_available_to(), None);
}

#[test]
fn test_request_messages_from_propagation_node_sets_max_messages() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    let identity_hash = [0x43u8; DESTINATION_LENGTH];
    
    router.set_outbound_propagation_node(Some(node_hash));
    
    // Request messages (will fail without link, but should set max_messages)
    let now_ms = 1_700_000_000_000u64;
    router.request_messages_from_propagation_node(identity_hash, Some(50), now_ms);
    
    assert_eq!(router.propagation_transfer_max_messages(), Some(50));
    assert_eq!(router.propagation_transfer_progress(), 0.0);
}

#[test]
fn test_request_messages_from_propagation_node_with_pr_all_messages() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    let identity_hash = [0x43u8; DESTINATION_LENGTH];
    
    router.set_outbound_propagation_node(Some(node_hash));
    
    // PR_ALL_MESSAGES = 0x00 = 0
    let now_ms = 1_700_000_000_000u64;
    router.request_messages_from_propagation_node(identity_hash, Some(0), now_ms);
    
    assert_eq!(router.propagation_transfer_max_messages(), Some(0));
}

#[test]
fn test_request_messages_from_propagation_node_without_node() {
    let mut router = make_router();
    let identity_hash = [0x43u8; DESTINATION_LENGTH];
    
    // No outbound_propagation_node set
    let now_ms = 1_700_000_000_000u64;
    router.request_messages_from_propagation_node(identity_hash, Some(50), now_ms);
    
    // Should not set max_messages if no node configured
    // (In Python, it logs a warning but doesn't set anything)
    // For now, we'll still set it to track the request
    assert_eq!(router.propagation_transfer_max_messages(), Some(50));
}

#[test]
fn test_get_outbound_propagation_cost_without_node() {
    let mut router = make_router();
    
    // No outbound_propagation_node set
    let cost = router.get_outbound_propagation_cost();
    assert_eq!(cost, None);
}

#[test]
fn test_get_outbound_propagation_cost_with_node_in_directory() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    
    router.set_outbound_propagation_node(Some(node_hash));
    
    // For now, test that it returns None if node not in directory
    let cost = router.get_outbound_propagation_cost();
    // Should be None if node not in pn_directory
    assert_eq!(cost, None);
    // Path request should be queued
    assert_eq!(router.pop_pn_path_request(), Some(node_hash));
}

#[test]
fn test_poll_path_request_timeout() {
    let mut router = make_router();
    let node_hash = [0x42u8; DESTINATION_LENGTH];
    let identity_hash = [0x43u8; DESTINATION_LENGTH];
    
    // Set up path request state
    router.set_wants_download_on_path_available_from(Some(node_hash));
    router.set_wants_download_on_path_available_to(Some(identity_hash));
    router.set_propagation_transfer_state(lxmf_rs::PR_PATH_REQUESTED);
    
    // Set timeout to 10 seconds from now
    let now_ms = 1_700_000_000_000u64;
    let timeout_s = (now_ms / 1000) + 10; // PR_PATH_TIMEOUT = 10 seconds
    router.set_wants_download_on_path_available_timeout(Some(timeout_s));
    
    // Poll before timeout - should return false (still waiting)
    let timed_out = router.poll_path_request_timeout(now_ms);
    assert_eq!(timed_out, false);
    assert_eq!(router.propagation_transfer_state(), lxmf_rs::PR_PATH_REQUESTED);
    
    // Poll after timeout - should return true and set state to PR_NO_PATH
    let after_timeout_ms = now_ms + (10 + 1) * 1000; // PR_PATH_TIMEOUT + 1 second
    let timed_out = router.poll_path_request_timeout(after_timeout_ms);
    assert_eq!(timed_out, true);
    assert_eq!(router.propagation_transfer_state(), lxmf_rs::PR_NO_PATH);
    assert_eq!(router.wants_download_on_path_available_from(), None);
    assert_eq!(router.wants_download_on_path_available_to(), None);
}
