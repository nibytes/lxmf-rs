#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter, LINK_MAX_INACTIVITY};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("teardown-test-src");
    let destination = PrivateIdentity::new_from_name("teardown-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

#[test]
fn test_no_data_for_emulation_matches_python() {
    // Test that our emulation of link.no_data_for() matches Python behavior
    // Python: inactive_time = link.no_data_for() (returns seconds)
    // Rust: inactive_time_ms = now_ms - last_activity_ms (returns milliseconds)
    let mut router = make_router();
    let link_id = [0x01u8; 16];
    let initial_time = 1_700_000_000_000u64; // milliseconds
    
    router.add_direct_link(link_id, initial_time);
    
    // Simulate 5 minutes of inactivity (300 seconds)
    let check_time = initial_time + (5 * 60 * 1000); // 5 minutes in milliseconds
    
    // In Python: link.no_data_for() would return 300.0 (seconds)
    // In Rust: we compute (check_time - initial_time) / 1000 = 300 (seconds equivalent)
    // Since LINK_MAX_INACTIVITY = 10*60 = 600 seconds, link should NOT be cleaned
    router.clean_links(check_time);
    assert!(router.has_direct_link(&link_id), "Link should not be cleaned after 5 minutes (below threshold)");
    
    // Now simulate 11 minutes of inactivity (660 seconds > 600 seconds threshold)
    let check_time2 = initial_time + (11 * 60 * 1000); // 11 minutes in milliseconds
    router.clean_links(check_time2);
    assert!(!router.has_direct_link(&link_id), "Link should be cleaned after 11 minutes (above threshold)");
}

#[test]
fn test_propagation_link_no_data_for_emulation() {
    // Test that propagation link inactivity check matches Python behavior
    let mut router = make_router();
    let link_id = [0x02u8; 16];
    let initial_time = 1_700_000_000_000u64;
    
    router.add_propagation_link(link_id, initial_time);
    
    // P_LINK_MAX_INACTIVITY = 3*60 = 180 seconds
    // Test with 2 minutes (120 seconds < 180) - should NOT be cleaned
    let check_time = initial_time + (2 * 60 * 1000);
    router.clean_links(check_time);
    assert!(router.has_propagation_link(&link_id), "Propagation link should not be cleaned after 2 minutes");
    
    // Test with 4 minutes (240 seconds > 180) - should be cleaned
    let check_time2 = initial_time + (4 * 60 * 1000);
    router.clean_links(check_time2);
    assert!(!router.has_propagation_link(&link_id), "Propagation link should be cleaned after 4 minutes");
}

#[test]
fn test_is_closed_flag_set_correctly() {
    // Test that is_closed flag is properly set when link is marked as closed
    let mut router = make_router();
    let link_id = [0x03u8; 16];
    let now_ms = 1_700_000_000_000u64;
    
    router.set_outbound_propagation_link(link_id, now_ms);
    assert!(router.outbound_propagation_link().is_some(), "Outbound propagation link should exist");
    
    // Mark link as closed (matching Python: link.status == RNS.Link.CLOSED)
    router.mark_outbound_propagation_link_closed();
    
    // clean_links() should detect is_closed and remove the link
    router.clean_links(now_ms);
    assert!(router.outbound_propagation_link().is_none(), "Closed link should be removed by clean_links()");
}

#[test]
fn test_link_activity_update_resets_inactivity() {
    // Test that updating link activity resets the inactivity timer
    let mut router = make_router();
    let link_id = [0x04u8; 16];
    let initial_time = 1_700_000_000_000u64;
    
    router.add_direct_link(link_id, initial_time);
    
    // After 9 minutes, update activity
    let update_time = initial_time + (9 * 60 * 1000);
    router.update_link_activity(link_id, update_time);
    
    // Now check after another 9 minutes (total 18 minutes from initial, but only 9 from update)
    let check_time = update_time + (9 * 60 * 1000);
    router.clean_links(check_time);
    
    // Link should still exist because activity was updated
    assert!(router.has_direct_link(&link_id), "Link should remain after activity update");
    
    // But if we wait another 2 minutes (11 minutes total from update), it should be cleaned
    let check_time2 = update_time + (11 * 60 * 1000);
    router.clean_links(check_time2);
    assert!(!router.has_direct_link(&link_id), "Link should be cleaned after 11 minutes from last activity");
}

#[test]
fn test_clean_links_handles_exact_threshold() {
    // Test that clean_links handles exact threshold values correctly
    // Python uses > comparison, so exact threshold should NOT trigger cleanup
    let mut router = make_router();
    let link_id = [0x05u8; 16];
    let initial_time = 1_700_000_000_000u64;
    
    router.add_direct_link(link_id, initial_time);
    
    // Exactly at threshold (LINK_MAX_INACTIVITY = 600 seconds)
    let exact_threshold = initial_time + (LINK_MAX_INACTIVITY * 1000);
    router.clean_links(exact_threshold);
    
    // Should NOT be cleaned (Python uses >, not >=)
    // But wait, let me check the Rust implementation...
    // Rust: if inactive_time_ms > link_max_inactivity_ms
    // So exact threshold should NOT trigger cleanup
    assert!(router.has_direct_link(&link_id), "Link at exact threshold should not be cleaned");
    
    // One millisecond over threshold should trigger cleanup
    let over_threshold = exact_threshold + 1;
    router.clean_links(over_threshold);
    assert!(!router.has_direct_link(&link_id), "Link over threshold should be cleaned");
}
