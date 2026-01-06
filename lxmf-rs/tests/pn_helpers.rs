use lxmf_rs::{
    display_name_from_app_data, pn_announce_data_is_valid, pn_name_from_app_data,
    pn_stamp_cost_from_app_data, stamp_cost_from_app_data,
};
use rmpv::Value;

#[test]
fn pn_helpers_validate_and_extract_stamp_cost() {
    let app_data = hex::decode("97c402706e7bc30a14930506078101c4046e616d65").unwrap();

    assert!(pn_announce_data_is_valid(&app_data));
    assert_eq!(pn_stamp_cost_from_app_data(&app_data), Some(5));
}

#[test]
fn display_name_and_stamp_cost_from_app_data() {
    let packed = {
        let value = Value::Array(vec![
            Value::Binary(b"Alice".to_vec()),
            Value::Integer(7.into()),
        ]);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &value).unwrap();
        buf
    };

    assert_eq!(display_name_from_app_data(&packed), Some("Alice".to_string()));
    assert_eq!(stamp_cost_from_app_data(&packed), Some(7));

    let legacy = b"Bob".to_vec();
    assert_eq!(display_name_from_app_data(&legacy), Some("Bob".to_string()));
    assert_eq!(stamp_cost_from_app_data(&legacy), None);
}

#[test]
fn pn_name_from_metadata() {
    let packed = {
        let value = Value::Array(vec![
            Value::Binary(b"pn".to_vec()),
            Value::Integer(1.into()),
            Value::Boolean(true),
            Value::Integer(10.into()),
            Value::Integer(20.into()),
            Value::Array(vec![
                Value::Integer(5.into()),
                Value::Integer(6.into()),
                Value::Integer(7.into()),
            ]),
            Value::Map(vec![(Value::Integer(1.into()), Value::Binary(b"Node".to_vec()))]),
        ]);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &value).unwrap();
        buf
    };

    assert_eq!(pn_name_from_app_data(&packed), Some("Node".to_string()));
}

#[test]
fn pn_helpers_reject_invalid_data() {
    let invalid = hex::decode("93c402706e7b").unwrap();
    assert!(!pn_announce_data_is_valid(&invalid));
    assert_eq!(pn_stamp_cost_from_app_data(&invalid), None);
}
