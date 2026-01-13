#![cfg(feature = "rns")]

use lxmf_rs::{
    DeliveryMode, LxmfRuntime, RuntimeConfig, RnsOutbound, RnsRouter, Value,
};
use reticulum::identity::PrivateIdentity;

fn make_outbound() -> RnsOutbound {
    let source = PrivateIdentity::new_from_name("daemon-test-source");
    let destination = PrivateIdentity::new_from_name("daemon-test-dest").as_identity().clone();
    RnsOutbound::new(source, destination)
}

fn make_message() -> lxmf_rs::LXMessage {
    lxmf_rs::LXMessage::with_strings(
        [0u8; 16],
        [0u8; 16],
        1_700_000_000.0,
        "hi",
        "daemon-test",
        Value::Map(vec![]),
    )
}

// Constants from Python LXMRouter
const JOB_OUTBOUND_INTERVAL: u64 = 1;
const JOB_STAMPS_INTERVAL: u64 = 1;
const JOB_LINKS_INTERVAL: u64 = 1;
const JOB_TRANSIENT_INTERVAL: u64 = 60;
const JOB_STORE_INTERVAL: u64 = 120;
const JOB_PEERSYNC_INTERVAL: u64 = 6;
const JOB_PEERINGEST_INTERVAL: u64 = 6; // same as PEERSYNC
const JOB_ROTATE_INTERVAL: u64 = 56 * JOB_PEERINGEST_INTERVAL; // 336
const PROCESSING_INTERVAL_MS: u64 = 4; // 4ms between ticks

#[test]
fn test_processing_count_increments() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);
    let mut runtime = LxmfRuntime::new(router);

    // First tick
    let _tick1 = runtime.tick(0);
    assert_eq!(runtime.processing_count(), 1);

    // Second tick
    let _tick2 = runtime.tick(PROCESSING_INTERVAL_MS);
    assert_eq!(runtime.processing_count(), 2);

    // Third tick
    let _tick3 = runtime.tick(PROCESSING_INTERVAL_MS * 2);
    assert_eq!(runtime.processing_count(), 3);
}

#[test]
fn test_job_outbound_runs_every_tick() {
    let outbound = make_outbound();
    let mut router = RnsRouter::new(outbound);
    router.enqueue(make_message(), DeliveryMode::Direct);

    let config = RuntimeConfig {
        outbound_interval_ms: 0, // Will be overridden via job interval
        inflight_interval_ms: 1000,
        request_interval_ms: 1000,
        pn_interval_ms: 1000,
        store_interval_ms: 1000,
    };

    let mut runtime = LxmfRuntime::with_config(router, config);

    // Tick 0: processing_count = 1, 1 % 1 == 0 -> should execute
    let tick1 = runtime.tick(0);
    assert!(tick1.outbound_ran);

    // Tick 1: processing_count = 2, 2 % 1 == 0 -> should execute
    let tick2 = runtime.tick(PROCESSING_INTERVAL_MS);
    assert!(tick2.outbound_ran);

    // Tick 2: processing_count = 3, 3 % 1 == 0 -> should execute
    let tick3 = runtime.tick(PROCESSING_INTERVAL_MS * 2);
    assert!(tick3.outbound_ran);
}

#[test]
fn test_job_stamps_runs_every_tick() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 0: processing_count = 1, 1 % 1 == 0 -> should execute
    let tick1 = runtime.tick(0);
    assert!(tick1.stamps_ran);

    // Tick 1: processing_count = 2, 2 % 1 == 0 -> should execute
    let tick2 = runtime.tick(PROCESSING_INTERVAL_MS);
    assert!(tick2.stamps_ran);
}

#[test]
fn test_job_links_runs_every_tick() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 0: processing_count = 1, 1 % 1 == 0 -> should execute
    let tick1 = runtime.tick(0);
    assert!(tick1.links_ran);

    // Tick 1: processing_count = 2, 2 % 1 == 0 -> should execute
    let tick2 = runtime.tick(PROCESSING_INTERVAL_MS);
    assert!(tick2.links_ran);
}

#[test]
fn test_job_transient_runs_every_60_ticks() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 0: processing_count = 1, 1 % 60 != 0 -> should not execute
    let tick1 = runtime.tick(0);
    assert!(!tick1.transient_ran);

    // Ticks 1-59: processing_count = 2..60, not divisible by 60
    for i in 1..59 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 59: processing_count = 60, 60 % 60 == 0 -> should execute
    let tick60 = runtime.tick(PROCESSING_INTERVAL_MS * 59);
    assert!(tick60.transient_ran);

    // Tick 60: processing_count = 61, 61 % 60 != 0 -> should not execute
    let tick61 = runtime.tick(PROCESSING_INTERVAL_MS * 60);
    assert!(!tick61.transient_ran);

    // Ticks 61-119: processing_count = 62..120
    for i in 61..119 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 119: processing_count = 120, 120 % 60 == 0 -> should execute
    let tick120 = runtime.tick(PROCESSING_INTERVAL_MS * 119);
    assert!(tick120.transient_ran);
}

#[test]
fn test_job_store_runs_every_120_ticks() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 0: processing_count = 1, 1 % 120 != 0 -> should not execute
    let tick1 = runtime.tick(0);
    assert!(!tick1.store_ran);

    // Ticks 1-119: processing_count = 2..120
    for i in 1..119 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 119: processing_count = 120, 120 % 120 == 0 -> should execute
    let tick120 = runtime.tick(PROCESSING_INTERVAL_MS * 119);
    assert!(tick120.store_ran);

    // Tick 120: processing_count = 121, 121 % 120 != 0 -> should not execute
    let tick121 = runtime.tick(PROCESSING_INTERVAL_MS * 120);
    assert!(!tick121.store_ran);

    // Ticks 121-239: processing_count = 122..240
    for i in 121..239 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 239: processing_count = 240, 240 % 120 == 0 -> should execute
    let tick240 = runtime.tick(PROCESSING_INTERVAL_MS * 239);
    assert!(tick240.store_ran);
}

#[test]
fn test_job_peeringest_runs_every_6_ticks() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);
    let mut runtime = LxmfRuntime::new(router);

    // PEERINGEST should run every 6 ticks
    // processing_count starts at 1, so it will first run on tick 5 (processing_count = 6)
    for i in 0..JOB_PEERINGEST_INTERVAL {
        let tick = runtime.tick(PROCESSING_INTERVAL_MS * i);
        // processing_count = i + 1, so we check (i + 1) % 6 == 0
        if (i + 1) % JOB_PEERINGEST_INTERVAL == 0 {
            assert!(tick.peeringest_ran, "PEERINGEST should run on tick {} (processing_count={})", i, i + 1);
        } else {
            assert!(!tick.peeringest_ran, "PEERINGEST should not run on tick {} (processing_count={})", i, i + 1);
        }
    }

    // Verify that it runs again after 6 ticks
    // Tick 5: processing_count = 6 -> already checked above
    // Tick 11: processing_count = 12 -> should run
    for i in JOB_PEERINGEST_INTERVAL..(JOB_PEERINGEST_INTERVAL * 2) {
        let tick = runtime.tick(PROCESSING_INTERVAL_MS * i);
        if (i + 1) % JOB_PEERINGEST_INTERVAL == 0 {
            assert!(tick.peeringest_ran, "PEERINGEST should run on tick {} (processing_count={})", i, i + 1);
        } else {
            assert!(!tick.peeringest_ran, "PEERINGEST should not run on tick {} (processing_count={})", i, i + 1);
        }
    }
}

#[test]
fn test_job_peersync_runs_every_6_ticks() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 0: processing_count = 1, 1 % 6 != 0 -> should not execute
    let tick1 = runtime.tick(0);
    assert!(!tick1.peersync_ran);

    // Ticks 1-5: processing_count = 2..6
    for i in 1..5 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 5: processing_count = 6, 6 % 6 == 0 -> should execute
    let tick6 = runtime.tick(PROCESSING_INTERVAL_MS * 5);
    assert!(tick6.peersync_ran);

    // Tick 6: processing_count = 7, 7 % 6 != 0 -> should not execute
    let tick7 = runtime.tick(PROCESSING_INTERVAL_MS * 6);
    assert!(!tick7.peersync_ran);

    // Ticks 7-11: processing_count = 8..12
    for i in 7..11 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 11: processing_count = 12, 12 % 6 == 0 -> should execute
    let tick12 = runtime.tick(PROCESSING_INTERVAL_MS * 11);
    assert!(tick12.peersync_ran);
}

#[test]
fn test_job_rotate_runs_every_336_ticks() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 0: processing_count = 1, 1 % 336 != 0 -> should not execute
    let tick1 = runtime.tick(0);
    assert!(!tick1.rotate_ran);

    // Ticks 1-335: processing_count = 2..336
    for i in 1..335 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 335: processing_count = 336, 336 % 336 == 0 -> should execute
    let tick336 = runtime.tick(PROCESSING_INTERVAL_MS * 335);
    assert!(tick336.rotate_ran);

    // Tick 336: processing_count = 337, 337 % 336 != 0 -> should not execute
    let tick337 = runtime.tick(PROCESSING_INTERVAL_MS * 336);
    assert!(!tick337.rotate_ran);

    // Ticks 337-671: processing_count = 338..672
    for i in 337..671 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 671: processing_count = 672, 672 % 336 == 0 -> should execute
    let tick672 = runtime.tick(PROCESSING_INTERVAL_MS * 671);
    assert!(tick672.rotate_ran);
}

#[test]
fn test_jobs_skip_when_not_due() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Tick 1: only outbound/stamps/links should execute (interval 1)
    let tick1 = runtime.tick(0);
    assert!(tick1.outbound_ran);
    assert!(tick1.stamps_ran);
    assert!(tick1.links_ran);
    assert!(!tick1.transient_ran); // interval 60
    assert!(!tick1.store_ran); // interval 120
    assert!(!tick1.peersync_ran); // interval 6

    // Tick 2: again only outbound/stamps/links
    let tick2 = runtime.tick(PROCESSING_INTERVAL_MS);
    assert!(tick2.outbound_ran);
    assert!(tick2.stamps_ran);
    assert!(tick2.links_ran);
    assert!(!tick2.transient_ran);
    assert!(!tick2.store_ran);
    assert!(!tick2.peersync_ran);
}

#[test]
fn test_multiple_jobs_run_on_same_tick() {
    let outbound = make_outbound();
    let router = RnsRouter::new(outbound);

    let mut runtime = LxmfRuntime::new(router);

    // Ticks 0-118: processing_count = 1..119
    for i in 0..119 {
        let _tick = runtime.tick(PROCESSING_INTERVAL_MS * i as u64);
    }

    // Tick 119: processing_count = 120
    // 120 % 1 == 0 -> outbound, stamps, links
    // 120 % 60 == 0 -> transient
    // 120 % 120 == 0 -> store
    // 120 % 6 == 0 -> peersync
    let tick120 = runtime.tick(PROCESSING_INTERVAL_MS * 119);
    assert!(tick120.outbound_ran);
    assert!(tick120.stamps_ran);
    assert!(tick120.links_ran);
    assert!(tick120.transient_ran);
    assert!(tick120.store_ran);
    assert!(tick120.peersync_ran);
    assert!(!tick120.rotate_ran); // 120 % 336 != 0
}
