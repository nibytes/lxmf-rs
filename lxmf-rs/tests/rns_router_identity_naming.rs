#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter, RuntimeConfig};
use reticulum::identity::PrivateIdentity;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("test-src");
    let destination = PrivateIdentity::new_from_name("test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

#[test]
fn test_router_name_defaults_to_none() {
    let router = make_router();
    assert_eq!(router.name(), None);
}

#[test]
fn test_set_router_name() {
    let mut router = make_router();
    router.set_name(Some("Test Router".to_string()));
    assert_eq!(router.name(), Some("Test Router"));
}

#[test]
fn test_set_router_name_to_none() {
    let mut router = make_router();
    router.set_name(Some("Test Router".to_string()));
    assert_eq!(router.name(), Some("Test Router"));
    router.set_name(None);
    assert_eq!(router.name(), None);
}

#[test]
fn test_router_autopeer_defaults_to_true() {
    let router = make_router();
    assert_eq!(router.autopeer(), true);
}

#[test]
fn test_set_router_autopeer() {
    let mut router = make_router();
    router.set_autopeer(false);
    assert_eq!(router.autopeer(), false);
    router.set_autopeer(true);
    assert_eq!(router.autopeer(), true);
}

#[test]
fn test_router_with_config_name() {
    let source = PrivateIdentity::new_from_name("test-src");
    let destination = PrivateIdentity::new_from_name("test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    let mut lxmf_router = LxmfRouter::with_config(
        RnsRouter::new(outbound),
        RuntimeConfig {
            name: Some("Config Router".to_string()),
            autopeer: Some(false),
            ..Default::default()
        },
    );
    assert_eq!(lxmf_router.name(), Some("Config Router"));
    assert_eq!(lxmf_router.autopeer(), false);
}
