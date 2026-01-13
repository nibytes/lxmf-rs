#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter, LINK_MAX_INACTIVITY};
use reticulum::destination::link::{LinkEventData, LinkEvent, LinkPayload};
use reticulum::hash::AddressHash;
use reticulum::identity::PrivateIdentity;
use reticulum::packet::PacketContext;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("link-event-test-src");
    let destination = PrivateIdentity::new_from_name("link-event-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

fn make_link_id(seed: u8) -> AddressHash {
    let mut bytes = [0u8; 16];
    bytes[0] = seed;
    AddressHash::new(bytes)
}

fn make_address_hash(seed: u8) -> AddressHash {
    let mut bytes = [0u8; 16];
    bytes[0] = seed;
    AddressHash::new(bytes)
}

#[test]
fn test_process_link_event_data_updates_activity() {
    // Test that LinkEvent::Data updates last_activity_ms (matching Python link.no_data_for() reset)
    let mut router = make_router();
    let link_id = make_link_id(1);
    let address_hash = make_address_hash(1);
    let now_ms = 1_700_000_000_000u64;
    
    // Add link first
    router.add_direct_link(address_hash.as_slice().try_into().unwrap(), now_ms);
    
    // Simulate some time passing
    let later_ms = now_ms + 1000;
    
    // Process Data event - this should update activity
    let event = LinkEventData::new(
        link_id,
        address_hash,
        LinkEvent::Data {
            payload: LinkPayload::new(),
            context: PacketContext::None,
        },
    );
    router.process_link_events(&[event], later_ms);
    
    // Now check that link is still active after LINK_MAX_INACTIVITY from the update
    let check_time = later_ms + (LINK_MAX_INACTIVITY - 1) * 1000;
    router.clean_links(check_time);
    assert!(router.has_direct_link(address_hash.as_slice().try_into().unwrap()), 
            "Link should remain active after Data event updated activity");
}

#[test]
fn test_process_link_event_activated_creates_link() {
    // Test that LinkEvent::Activated creates link entry
    let mut router = make_router();
    let link_id = make_link_id(2);
    let address_hash = make_address_hash(2);
    let now_ms = 1_700_000_000_000u64;
    
    // Process Activated event - this should create link
    let event = LinkEventData::new(link_id, address_hash, LinkEvent::Activated);
    router.process_link_events(&[event], now_ms);
    
    // Link should now be tracked
    assert!(router.has_direct_link(address_hash.as_slice().try_into().unwrap()),
            "Link should be tracked after Activated event");
}

#[test]
fn test_process_link_event_closed_sets_flag() {
    // Test that LinkEvent::Closed sets is_closed flag (matching Python link.status == CLOSED)
    let mut router = make_router();
    let link_id = make_link_id(3);
    let address_hash = make_address_hash(3);
    let now_ms = 1_700_000_000_000u64;
    
    // Set outbound propagation link
    router.set_outbound_propagation_link(link_id.as_slice().try_into().unwrap(), now_ms);
    
    // Process Closed event
    let event = LinkEventData::new(link_id, address_hash, LinkEvent::Closed);
    router.process_link_events(&[event], now_ms);
    
    // clean_links() should detect closed link and remove it
    router.clean_links(now_ms);
    assert!(router.outbound_propagation_link().is_none(),
            "Closed link should be removed by clean_links()");
}

#[test]
fn test_process_link_events_matches_python_behavior() {
    // Integration test: verify that processing link events matches Python behavior
    // Python: link.no_data_for() resets on data, link.status == CLOSED on close
    let mut router = make_router();
    let link_id = make_link_id(4);
    let address_hash = make_address_hash(4);
    let initial_ms = 1_700_000_000_000u64;
    
    // 1. Link activated
    let activated_event = LinkEventData::new(link_id, address_hash, LinkEvent::Activated);
    router.process_link_events(&[activated_event], initial_ms);
    assert!(router.has_direct_link(address_hash.as_slice().try_into().unwrap()));
    
    // 2. Data received (should reset inactivity timer)
    let data_time = initial_ms + (LINK_MAX_INACTIVITY - 1) * 1000; // Just before timeout
    let data_event = LinkEventData::new(
        link_id,
        address_hash,
        LinkEvent::Data {
            payload: LinkPayload::new(),
            context: PacketContext::None,
        },
    );
    router.process_link_events(&[data_event], data_time);
    
    // 3. Check that link is still active (timer was reset)
    let check_time = data_time + (LINK_MAX_INACTIVITY - 1) * 1000;
    router.clean_links(check_time);
    assert!(router.has_direct_link(address_hash.as_slice().try_into().unwrap()),
            "Link should remain active after Data event reset timer");
    
    // 4. Link closed
    let closed_event = LinkEventData::new(link_id, address_hash, LinkEvent::Closed);
    router.process_link_events(&[closed_event], check_time);
    
    // Note: For direct links, Closed event doesn't immediately remove them,
    // but clean_links() will handle them based on inactivity
    // This matches Python behavior where link.status == CLOSED is checked in clean_links()
}
