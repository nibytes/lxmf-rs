#![cfg(feature = "rns")]

use lxmf_rs::{LxmfRouter, RnsOutbound, RnsRouter, PN_META_NAME};
use reticulum::identity::PrivateIdentity;
use rmpv::Value;

fn make_router() -> LxmfRouter {
    let source = PrivateIdentity::new_from_name("test-src");
    let destination = PrivateIdentity::new_from_name("test-dst").as_identity().clone();
    let outbound = RnsOutbound::new(source, destination);
    LxmfRouter::new(RnsRouter::new(outbound))
}

#[test]
fn test_get_propagation_node_announce_metadata_without_name() {
    let router = make_router();
    let metadata = router.get_propagation_node_announce_metadata();
    assert!(metadata.is_empty());
}

#[test]
fn test_get_propagation_node_announce_metadata_with_name() {
    let mut router = make_router();
    router.set_name(Some("Test Node".to_string()));
    let metadata = router.get_propagation_node_announce_metadata();
    
    // Should contain PN_META_NAME with encoded name
    assert!(metadata.contains_key(&PN_META_NAME));
    if let Some(name_bytes) = metadata.get(&PN_META_NAME) {
        let name_str = String::from_utf8(name_bytes.clone()).unwrap();
        assert_eq!(name_str, "Test Node");
    } else {
        panic!("PN_META_NAME should contain binary data");
    }
}

#[test]
fn test_get_propagation_node_app_data_structure() {
    let mut router = make_router();
    router.set_name(Some("Test Node".to_string()));
    router.enable_propagation_node(true, 1_700_000_000_000);
    
    let app_data = router.get_propagation_node_app_data();
    
    // Verify it's valid msgpack and can be decoded
    let decoded: Value = rmpv::decode::read_value(&mut std::io::Cursor::new(&app_data)).unwrap();
    
    // Should be an array with 7 elements
    if let Value::Array(arr) = decoded {
        assert_eq!(arr.len(), 7);
        
        // [0] Legacy support (False)
        assert_eq!(arr[0], Value::Boolean(false));
        
        // [1] Timebase (u64)
        assert!(matches!(arr[1], Value::Integer(_)));
        
        // [2] Node state (bool)
        assert!(matches!(arr[2], Value::Boolean(_)));
        
        // [3] Propagation transfer limit (u64)
        assert!(matches!(arr[3], Value::Integer(_)));
        
        // [4] Propagation sync limit (u64)
        assert!(matches!(arr[4], Value::Integer(_)));
        
        // [5] Stamp cost array [cost, flexibility, peering_cost]
        if let Value::Array(stamp_costs) = &arr[5] {
            assert_eq!(stamp_costs.len(), 3);
            assert!(matches!(stamp_costs[0], Value::Integer(_)));
            assert!(matches!(stamp_costs[1], Value::Integer(_)));
            assert!(matches!(stamp_costs[2], Value::Integer(_)));
        } else {
            panic!("Stamp cost should be an array");
        }
        
        // [6] Metadata (map)
        assert!(matches!(arr[6], Value::Map(_)));
    } else {
        panic!("App data should be an array");
    }
}

#[test]
fn test_get_propagation_node_app_data_node_state() {
    let mut router = make_router();
    
    // Test with propagation disabled
    router.enable_propagation_node(false, 1_700_000_000_000);
    let app_data_disabled = router.get_propagation_node_app_data();
    let decoded_disabled: Value = rmpv::decode::read_value(&mut std::io::Cursor::new(&app_data_disabled)).unwrap();
    if let Value::Array(arr) = decoded_disabled {
        assert_eq!(arr[2], Value::Boolean(false));
    }
    
    // Test with propagation enabled
    router.enable_propagation_node(true, 1_700_000_000_000);
    let app_data_enabled = router.get_propagation_node_app_data();
    let decoded_enabled: Value = rmpv::decode::read_value(&mut std::io::Cursor::new(&app_data_enabled)).unwrap();
    if let Value::Array(arr) = decoded_enabled {
        assert_eq!(arr[2], Value::Boolean(true));
    }
}

#[test]
fn test_get_propagation_node_app_data_from_static_only() {
    let mut router = make_router();
    router.enable_propagation_node(true, 1_700_000_000_000);
    router.set_from_static_only(true);
    
    let app_data = router.get_propagation_node_app_data();
    let decoded: Value = rmpv::decode::read_value(&mut std::io::Cursor::new(&app_data)).unwrap();
    
    // When from_static_only is true, node_state should be false
    if let Value::Array(arr) = decoded {
        assert_eq!(arr[2], Value::Boolean(false));
    }
}

#[test]
fn test_get_propagation_node_app_data_validates_with_pn_announce_data_is_valid() {
    let mut router = make_router();
    router.set_name(Some("Test Node".to_string()));
    router.enable_propagation_node(true, 1_700_000_000_000);
    
    let app_data = router.get_propagation_node_app_data();
    
    // Should validate with pn_announce_data_is_valid
    assert!(lxmf_rs::pn_announce_data_is_valid(&app_data));
}
