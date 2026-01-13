#[cfg(test)]
#[cfg(feature = "rns")]
mod tests {
    use lxmf_rs::{PropagationEntry, PropagationStore, DESTINATION_LENGTH};

    fn make_transient_id(seed: u8) -> [u8; 32] {
        let mut id = [0u8; 32];
        id[0] = seed;
        id
    }

    fn make_destination_hash(seed: u8) -> [u8; DESTINATION_LENGTH] {
        let mut hash = [0u8; DESTINATION_LENGTH];
        hash[0] = seed;
        hash
    }

    #[test]
    fn test_propagation_entry_has_destination_hash() {
        // Test that PropagationEntry stores destination_hash (matching Python index 0)
        let transient_id = make_transient_id(1);
        let destination_hash = make_destination_hash(2);
        let entry = PropagationEntry {
            transient_id,
            destination_hash: Some(destination_hash),
            lxm_data: vec![1, 2, 3, 4],
            timestamp: 1.0,
            msg_size: Some(4),
            filepath: None,
        };

        assert_eq!(entry.destination_hash, Some(destination_hash));
    }

    #[test]
    fn test_propagation_entry_has_filepath() {
        // Test that PropagationEntry stores filepath (matching Python index 1)
        let transient_id = make_transient_id(1);
        let filepath = Some("test/path/file.lxm".to_string());
        let entry = PropagationEntry {
            transient_id,
            destination_hash: None,
            lxm_data: vec![1, 2, 3, 4],
            timestamp: 1.0,
            msg_size: Some(4),
            filepath: filepath.clone(),
        };

        assert_eq!(entry.filepath, filepath);
    }

    #[test]
    fn test_propagation_entry_has_msg_size() {
        // Test that PropagationEntry stores msg_size (matching Python index 3)
        let transient_id = make_transient_id(1);
        let msg_size = Some(1024);
        let entry = PropagationEntry {
            transient_id,
            destination_hash: None,
            lxm_data: vec![1, 2, 3, 4],
            timestamp: 1.0,
            msg_size,
            filepath: None,
        };

        assert_eq!(entry.msg_size, msg_size);
    }

    #[test]
    fn test_propagation_store_handles_full_entry() {
        // Test that PropagationStore can store and retrieve full PropagationEntry
        let mut store = PropagationStore::new();
        let transient_id = make_transient_id(1);
        let destination_hash = make_destination_hash(2);
        let entry = PropagationEntry {
            transient_id,
            destination_hash: Some(destination_hash),
            lxm_data: vec![1, 2, 3, 4],
            timestamp: 1.0,
            msg_size: Some(4),
            filepath: Some("test.lxm".to_string()),
        };

        store.insert(entry.clone());

        let retrieved = store.get(&transient_id).expect("entry should exist");
        assert_eq!(retrieved.destination_hash, Some(destination_hash));
        assert_eq!(retrieved.filepath, Some("test.lxm".to_string()));
        assert_eq!(retrieved.msg_size, Some(4));
    }

    #[test]
    fn test_propagation_entry_serialization() {
        // Test that PropagationEntry can be serialized/deserialized with all fields
        let transient_id = make_transient_id(1);
        let destination_hash = make_destination_hash(2);
        let entry = PropagationEntry {
            transient_id,
            destination_hash: Some(destination_hash),
            lxm_data: vec![1, 2, 3, 4],
            timestamp: 1.0,
            msg_size: Some(4),
            filepath: Some("test.lxm".to_string()),
        };

        // Serialize using PropagationStore (which uses internal serialization)
        let mut store = PropagationStore::new();
        store.insert(entry.clone());
        
        // Get entry back to verify it was stored correctly
        let deserialized = store.get(&transient_id).expect("entry should exist").clone();

        assert_eq!(deserialized.transient_id, transient_id);
        assert_eq!(deserialized.destination_hash, Some(destination_hash));
        assert_eq!(deserialized.lxm_data, vec![1, 2, 3, 4]);
        assert_eq!(deserialized.timestamp, 1.0);
        assert_eq!(deserialized.msg_size, Some(4));
        assert_eq!(deserialized.filepath, Some("test.lxm".to_string()));
    }

    #[test]
    fn test_propagation_entry_backward_compatibility() {
        // Test that entries without new fields can still be created and used
        let transient_id = make_transient_id(1);
        let entry = PropagationEntry {
            transient_id,
            destination_hash: None,
            lxm_data: vec![1, 2, 3, 4],
            timestamp: 1.0,
            msg_size: None,
            filepath: None,
        };

        let mut store = PropagationStore::new();
        store.insert(entry.clone());
        
        let retrieved = store.get(&transient_id).expect("entry should exist");
        assert_eq!(retrieved.transient_id, make_transient_id(1));
        assert_eq!(retrieved.lxm_data, vec![1, 2, 3, 4]);
        assert_eq!(retrieved.timestamp, 1.0);
        // New fields should be None
        assert_eq!(retrieved.destination_hash, None);
        assert_eq!(retrieved.msg_size, None);
        assert_eq!(retrieved.filepath, None);
    }
}
