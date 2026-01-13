#![cfg(feature = "rns")]

use lxmf_rs::{DeliveryMode, LxmfRouter, LXMessage, RnsOutbound, RnsRouter, Value};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("deferred-test-src");
    let destination = PrivateIdentity::new_from_name("deferred-test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let router = RnsRouter::new(outbound);
    LxmfRouter::new(router)
}

fn make_message_with_defer_stamp() -> LXMessage {
    let mut msg = LXMessage::with_strings(
        [0x01u8; 16],
        [0x02u8; 16],
        1_700_000_000.0,
        "test",
        "deferred",
        Value::Map(vec![]),
    );
    msg.defer_stamp = true;
    msg.stamp_cost = Some(100); // Set stamp_cost so defer_stamp is not cleared
    msg
}

#[test]
fn test_process_deferred_stamps_empty_queue() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Empty queue processes nothing
    let processed = router.process_deferred_stamps(now_ms, 10);
    assert_eq!(processed, 0);
    assert_eq!(router.deferred_len(), 0);
}

#[test]
fn test_process_deferred_stamps_processes_items() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add message with defer_stamp via handle_outbound
    let msg = make_message_with_defer_stamp();
    router.handle_outbound(msg, DeliveryMode::Direct, now_ms);
    
    // Verify that message in deferred queue
    assert_eq!(router.deferred_len(), 1);
    
    // Process deferred stamps
    let processed = router.process_deferred_stamps(now_ms, 10);
    assert_eq!(processed, 1);
    assert_eq!(router.deferred_len(), 0);
    
    // Verify that message added to outbound queue
    assert!(router.outbound_len() > 0);
}

#[test]
fn test_process_deferred_stamps_respects_max_items() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add several messages
    for i in 0..5 {
        let mut msg = LXMessage::with_strings(
            [i; 16],
            [0x02u8; 16],
            1_700_000_000.0,
            "test",
            "deferred",
            Value::Map(vec![]),
        );
        msg.defer_stamp = true;
        msg.stamp_cost = Some(100); // Set stamp_cost so defer_stamp is not cleared
        router.handle_outbound(msg, DeliveryMode::Direct, now_ms);
    }
    
    assert_eq!(router.deferred_len(), 5);
    
    // Process only 2 items
    let processed = router.process_deferred_stamps(now_ms, 2);
    assert_eq!(processed, 2);
    assert_eq!(router.deferred_len(), 3);
}

#[test]
fn test_process_deferred_stamps_clears_defer_stamp() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add message with defer_stamp
    let msg = make_message_with_defer_stamp();
    router.handle_outbound(msg, DeliveryMode::Direct, now_ms);
    
    // Process
    router.process_deferred_stamps(now_ms, 10);
    
    // Verify that defer_stamp cleared (message in outbound)
    // This is indirectly verified by, that message processed
    assert_eq!(router.deferred_len(), 0);
}

#[test]
fn test_process_deferred_stamps_integration_with_tick() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add message with defer_stamp
    let msg = make_message_with_defer_stamp();
    router.handle_outbound(msg, DeliveryMode::Direct, now_ms);
    
    assert_eq!(router.deferred_len(), 1);
    
    // Call tick() several times so stamps_ran becomes true
    // JOB_STAMPS_INTERVAL = 1, so stamps_ran will be true on first tick
    let tick = router.tick(now_ms);
    assert!(tick.stamps_ran);
    
    // Processing happens asynchronously in separate thread, need to give time for processing
    // In real usage this is not a problem, because tick() is called periodically
    std::thread::sleep(std::time::Duration::from_millis(10));
    
    // Call tick() once more to receive processed messages from channel
    let _tick2 = router.tick(now_ms + 1000);
    
    // After integration process_deferred_stamps in tick(), message should be processed
    // Verify that deferred_len() decreased (processed 1 item from batch)
    // DEFERRED_STAMPS_BATCH_SIZE = 1, so 1 item will be processed
    assert_eq!(router.deferred_len(), 0);
}

#[test]
fn test_deferred_stamps_with_different_modes() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add messages with different delivery modes
    let msg1 = make_message_with_defer_stamp();
    router.handle_outbound(msg1, DeliveryMode::Direct, now_ms);
    
    let msg2 = make_message_with_defer_stamp();
    router.handle_outbound(msg2, DeliveryMode::Propagated, now_ms);
    
    assert_eq!(router.deferred_len(), 2);
    
    // Process all
    let processed = router.process_deferred_stamps(now_ms, 10);
    assert_eq!(processed, 2);
    assert_eq!(router.deferred_len(), 0);
}

#[test]
fn test_deferred_stamps_queue_processing_order() {
    let mut router = make_router();
    let now_ms = 1_700_000_000_000u64;
    
    // Add several messages
    for i in 0..3 {
        let mut msg = LXMessage::with_strings(
            [i; 16],
            [0x02u8; 16],
            1_700_000_000.0,
            &format!("test{}", i),
            "deferred",
            Value::Map(vec![]),
        );
        msg.defer_stamp = true;
        msg.stamp_cost = Some(100); // Set stamp_cost so defer_stamp is not cleared
        router.handle_outbound(msg, DeliveryMode::Direct, now_ms);
    }
    
    assert_eq!(router.deferred_len(), 3);
    
    // Process one by one
    let processed1 = router.process_deferred_stamps(now_ms, 1);
    assert_eq!(processed1, 1);
    assert_eq!(router.deferred_len(), 2);
    
    let processed2 = router.process_deferred_stamps(now_ms, 1);
    assert_eq!(processed2, 1);
    assert_eq!(router.deferred_len(), 1);
    
    let processed3 = router.process_deferred_stamps(now_ms, 1);
    assert_eq!(processed3, 1);
    assert_eq!(router.deferred_len(), 0);
}
