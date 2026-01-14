#![cfg(feature = "rns")]

use lxmf_rs::{
    OFFER_REQUEST_PATH, MESSAGE_GET_PATH, LINK_ESTABLISHING, LINK_READY, RESOURCE_TRANSFERRING,
    ERROR_NO_IDENTITY, ERROR_NO_ACCESS, ERROR_INVALID_DATA, ERROR_INVALID_STAMP, ERROR_THROTTLED,
    ERROR_NOT_FOUND, ERROR_TIMEOUT, STRATEGY_LAZY, STRATEGY_PERSISTENT, DEFAULT_SYNC_STRATEGY,
    PATH_REQUEST_GRACE,
};

#[test]
fn test_offer_request_path() {
    // Python: OFFER_REQUEST_PATH = "/offer"
    assert_eq!(OFFER_REQUEST_PATH, "/offer");
}

#[test]
fn test_message_get_path() {
    // Python: MESSAGE_GET_PATH = "/get"
    assert_eq!(MESSAGE_GET_PATH, "/get");
}

#[test]
fn test_link_establishing() {
    // Python: LINK_ESTABLISHING = 0x01
    assert_eq!(LINK_ESTABLISHING, 0x01);
}

#[test]
fn test_link_ready() {
    // Python: LINK_READY = 0x02
    assert_eq!(LINK_READY, 0x02);
}

#[test]
fn test_resource_transferring() {
    // Python: RESOURCE_TRANSFERRING = 0x05
    assert_eq!(RESOURCE_TRANSFERRING, 0x05);
}

#[test]
fn test_error_no_identity() {
    // Python: ERROR_NO_IDENTITY = 0xf0
    assert_eq!(ERROR_NO_IDENTITY, 0xf0);
}

#[test]
fn test_error_no_access() {
    // Python: ERROR_NO_ACCESS = 0xf1
    assert_eq!(ERROR_NO_ACCESS, 0xf1);
}

#[test]
fn test_error_invalid_data() {
    // Python: ERROR_INVALID_DATA = 0xf4
    assert_eq!(ERROR_INVALID_DATA, 0xf4);
}

#[test]
fn test_error_invalid_stamp() {
    // Python: ERROR_INVALID_STAMP = 0xf5
    assert_eq!(ERROR_INVALID_STAMP, 0xf5);
}

#[test]
fn test_error_throttled() {
    // Python: ERROR_THROTTLED = 0xf6
    assert_eq!(ERROR_THROTTLED, 0xf6);
}

#[test]
fn test_error_not_found() {
    // Python: ERROR_NOT_FOUND = 0xfd
    assert_eq!(ERROR_NOT_FOUND, 0xfd);
}

#[test]
fn test_error_timeout() {
    // Python: ERROR_TIMEOUT = 0xfe
    assert_eq!(ERROR_TIMEOUT, 0xfe);
}

#[test]
fn test_strategy_lazy() {
    // Python: STRATEGY_LAZY = 0x01
    assert_eq!(STRATEGY_LAZY, 0x01);
}

#[test]
fn test_strategy_persistent() {
    // Python: STRATEGY_PERSISTENT = 0x02
    assert_eq!(STRATEGY_PERSISTENT, 0x02);
}

#[test]
fn test_default_sync_strategy() {
    // Python: DEFAULT_SYNC_STRATEGY = STRATEGY_PERSISTENT
    assert_eq!(DEFAULT_SYNC_STRATEGY, STRATEGY_PERSISTENT);
}

#[test]
fn test_path_request_grace() {
    // Python: PATH_REQUEST_GRACE = 7.5
    assert!((PATH_REQUEST_GRACE - 7.5).abs() < f64::EPSILON);
}
