#![cfg(feature = "rns")]

use lxmf_rs::{
    MAX_DELIVERY_ATTEMPTS, DELIVERY_RETRY_WAIT, PATH_REQUEST_WAIT, MAX_PATHLESS_TRIES,
    NODE_ANNOUNCE_DELAY, PR_PATH_TIMEOUT, PN_STAMP_THROTTLE, PR_ALL_MESSAGES,
    STATS_GET_PATH, SYNC_REQUEST_PATH, UNPEER_REQUEST_PATH, DUPLICATE_SIGNAL,
};

#[test]
fn test_max_delivery_attempts() {
    // Python: MAX_DELIVERY_ATTEMPTS = 5
    assert_eq!(MAX_DELIVERY_ATTEMPTS, 5);
}

#[test]
fn test_delivery_retry_wait() {
    // Python: DELIVERY_RETRY_WAIT = 10
    assert_eq!(DELIVERY_RETRY_WAIT, 10);
}

#[test]
fn test_path_request_wait() {
    // Python: PATH_REQUEST_WAIT = 7
    assert_eq!(PATH_REQUEST_WAIT, 7);
}

#[test]
fn test_max_pathless_tries() {
    // Python: MAX_PATHLESS_TRIES = 1
    assert_eq!(MAX_PATHLESS_TRIES, 1);
}

#[test]
fn test_node_announce_delay() {
    // Python: NODE_ANNOUNCE_DELAY = 20
    assert_eq!(NODE_ANNOUNCE_DELAY, 20);
}

#[test]
fn test_pr_path_timeout() {
    // Python: PR_PATH_TIMEOUT = 10
    assert_eq!(PR_PATH_TIMEOUT, 10);
}

#[test]
fn test_pn_stamp_throttle() {
    // Python: PN_STAMP_THROTTLE = 180
    assert_eq!(PN_STAMP_THROTTLE, 180);
}

#[test]
fn test_pr_all_messages() {
    // Python: PR_ALL_MESSAGES = 0x00
    assert_eq!(PR_ALL_MESSAGES, 0x00);
}

#[test]
fn test_stats_get_path() {
    // Python: STATS_GET_PATH = "/pn/get/stats"
    assert_eq!(STATS_GET_PATH, "/pn/get/stats");
}

#[test]
fn test_sync_request_path() {
    // Python: SYNC_REQUEST_PATH = "/pn/peer/sync"
    assert_eq!(SYNC_REQUEST_PATH, "/pn/peer/sync");
}

#[test]
fn test_unpeer_request_path() {
    // Python: UNPEER_REQUEST_PATH = "/pn/peer/unpeer"
    assert_eq!(UNPEER_REQUEST_PATH, "/pn/peer/unpeer");
}

#[test]
fn test_duplicate_signal() {
    // Python: DUPLICATE_SIGNAL = "lxmf_duplicate"
    assert_eq!(DUPLICATE_SIGNAL, "lxmf_duplicate");
}
