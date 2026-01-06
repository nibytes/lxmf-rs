use lxmf_rs::{PnDirectory, PnDirectoryError};
use lxmf_rs::{Value, PN_META_NAME};

fn build_announce_data(name: &str, stamp_cost: i64) -> Vec<u8> {
    let metadata = Value::Map(vec![(
        Value::Integer((PN_META_NAME as i64).into()),
        Value::Binary(name.as_bytes().to_vec()),
    )]);

    let announce = Value::Array(vec![
        Value::Boolean(false),
        Value::Integer(1_700_000_000i64.into()),
        Value::Boolean(true),
        Value::Integer(64i64.into()),
        Value::Integer(128i64.into()),
        Value::Array(vec![
            Value::Integer(stamp_cost.into()),
            Value::Integer(2i64.into()),
            Value::Integer(3i64.into()),
        ]),
        metadata,
    ]);

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &announce).expect("msgpack");
    buf
}

#[test]
fn pn_directory_tracks_entries_and_ack() {
    let app_data = build_announce_data("pn-one", 10);
    let mut dir = PnDirectory::new();
    let hash = [1u8; 16];

    dir.upsert_announce(hash, &app_data, 123).expect("announce");
    let entries = dir.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name.as_deref(), Some("pn-one"));
    assert_eq!(entries[0].stamp_cost, Some(10));
    assert!(!entries[0].acked);

    assert!(dir.acknowledge(&hash));
    let entry = dir.get(&hash).expect("entry");
    assert!(entry.acked);
    assert_eq!(dir.stamp_cost(&hash), Some(10));

    let removed = dir.delete(&hash).expect("removed");
    assert_eq!(removed.destination_hash, hash);
    assert!(dir.list().is_empty());
}

#[test]
fn pn_directory_rejects_invalid_announce() {
    let mut dir = PnDirectory::new();
    let hash = [2u8; 16];

    let err = dir.upsert_announce(hash, b"bad", 0).unwrap_err();
    assert!(matches!(err, PnDirectoryError::InvalidAnnounce));
}
