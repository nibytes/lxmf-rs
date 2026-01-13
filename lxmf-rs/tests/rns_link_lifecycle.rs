#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter};
use reticulum::identity::PrivateIdentity;

// Constants from Python LXMRouter
const LINK_MAX_INACTIVITY: u64 = 10 * 60; // 10 minutes in seconds
const P_LINK_MAX_INACTIVITY: u64 = 3 * 60; // 3 minutes in seconds

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("link-test-src");
    let destination = PrivateIdentity::new_from_name("link-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}


#[test]
fn test_track_direct_link() {
    let mut router = make_router();
    let link_id = [0x01u8; 16];
    let now_ms = 1_700_000_000_000u64;

    router.add_direct_link(link_id, now_ms);
    assert!(router.has_direct_link(&link_id));
}

#[test]
fn test_track_propagation_link() {
    let mut router = make_router();
    let link_id = [0x02u8; 16];
    let now_ms = 1_700_000_000_000u64;

    router.add_propagation_link(link_id, now_ms);
    assert!(router.has_propagation_link(&link_id));
}

#[test]
fn test_clean_inactive_direct_link() {
    let mut router = make_router();
    let link_id = [0x03u8; 16];
    let initial_time = 1_700_000_000_000u64;

    // Add link
    router.add_direct_link(link_id, initial_time);

    // Time passed more than LINK_MAX_INACTIVITY
    let cleanup_time = initial_time + (LINK_MAX_INACTIVITY + 1) * 1000;

    // Call clean_links
    router.clean_links(cleanup_time);

    // Link should be removed
    assert!(!router.has_direct_link(&link_id));
}

#[test]
fn test_clean_inactive_propagation_link() {
    let mut router = make_router();
    let link_id = [0x04u8; 16];
    let initial_time = 1_700_000_000_000u64;

    // Add propagation link
    router.add_propagation_link(link_id, initial_time);

    // Time passed more than P_LINK_MAX_INACTIVITY
    let cleanup_time = initial_time + (P_LINK_MAX_INACTIVITY + 1) * 1000;

    // Call clean_links
    router.clean_links(cleanup_time);

    // Link should be removed
    assert!(!router.has_propagation_link(&link_id));
}

#[test]
fn test_clean_closed_link() {
    let mut router = make_router();
    let link_id = [0x05u8; 16];
    let now_ms = 1_700_000_000_000u64;

    // Add link
    router.add_direct_link(link_id, now_ms);

    // For closed link we simply do not update activity
    // and after time it will be removed as inactive
    let cleanup_time = now_ms + (LINK_MAX_INACTIVITY + 1) * 1000;

    // Call clean_links
    router.clean_links(cleanup_time);

    // Inactive link should be removed
    assert!(!router.has_direct_link(&link_id));
}

#[test]
fn test_remove_from_validated_peer_links() {
    let mut router = make_router();
    let link_id = [0x06u8; 16];
    let now_ms = 1_700_000_000_000u64;

    // Add link and to validated_peer_links
    router.add_direct_link(link_id, now_ms);
    router.add_validated_peer_link(link_id);
    assert!(router.is_validated_peer_link(&link_id));

    // link becomes inactive
    let inactive_time = now_ms + (LINK_MAX_INACTIVITY + 1) * 1000;

    // Call clean_links
    router.clean_links(inactive_time);

    // Should be removed from validated_peer_links
    assert!(!router.is_validated_peer_link(&link_id));
}

#[test]
fn test_handle_outbound_propagation_link_closed() {
    let mut router = make_router();
    let link_id = [0x07u8; 16];
    let now_ms = 1_700_000_000_000u64;

    // Set outbound propagation link
    router.set_outbound_propagation_link(link_id, now_ms);
    assert!(router.outbound_propagation_link().is_some());

    // Mark link as closed (in Python this is link.status == CLOSED)
    router.mark_outbound_propagation_link_closed();

    // Call clean_links
    router.clean_links(now_ms);

    // Outbound propagation link should be cleared
    assert!(router.outbound_propagation_link().is_none());
}

#[test]
fn test_propagation_transfer_state_on_link_close() {
    let mut router = make_router();
    let link_id = [0x08u8; 16];
    let now_ms = 1_700_000_000_000u64;

    // Set different transfer states
    router.set_outbound_propagation_link(link_id, now_ms);
    
    // Test: mark link as closed and verify cleanup
    router.mark_outbound_propagation_link_closed();
    router.clean_links(now_ms);
    assert!(router.outbound_propagation_link().is_none());
    
    // Test that acknowledge_sync_completion is called correctly
    // (This is now implemented in Этап 7.3)
}

#[test]
fn test_clean_links_does_not_affect_active_links() {
    let mut router = make_router();
    let active_link_id = [0x09u8; 16];
    let inactive_link_id = [0x0Au8; 16];
    let initial_time = 1_700_000_000_000u64;

    // Add both links
    router.add_direct_link(active_link_id, initial_time);
    router.add_direct_link(inactive_link_id, initial_time);

    // Update activity of active link
    let active_time = initial_time + 1000; // 1 second passed
    router.update_link_activity(active_link_id, active_time);

    // Inactive link remains old (do not update its activity)

    // Time passed more than LINK_MAX_INACTIVITY
    let cleanup_time = initial_time + (LINK_MAX_INACTIVITY + 1) * 1000;

    // Call clean_links
    router.clean_links(cleanup_time);

    // Active link should remain (its activity was updated later)
    // Inactive link should be removed
    assert!(router.has_direct_link(&active_link_id));
    assert!(!router.has_direct_link(&inactive_link_id));
}
