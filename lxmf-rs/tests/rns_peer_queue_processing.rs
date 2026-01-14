#[cfg(test)]
#[cfg(feature = "rns")]
mod tests {
    use lxmf_rs::{Peer, PropagationEntry, PropagationStore, DESTINATION_LENGTH};

    fn make_peer_id(seed: u8) -> [u8; DESTINATION_LENGTH] {
        let mut id = [0u8; DESTINATION_LENGTH];
        id[0] = seed;
        id
    }

    fn make_transient_id(seed: u8) -> [u8; 32] {
        let mut id = [0u8; 32];
        id[0] = seed;
        id
    }

    fn make_propagation_entry(transient_id: [u8; 32], timestamp: f64) -> PropagationEntry {
        PropagationEntry {
            transient_id,
            lxm_data: vec![1, 2, 3, 4],
            timestamp,
            destination_hash: None,
            filepath: None,
            msg_size: None,
        }
    }

    #[test]
    fn test_process_queues_handled_messages() {
        // Test that handled_messages_queue is processed and peer is added to handled_peers
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let mut peer = Peer::new(peer_id, now_ms);
        let transient_id = make_transient_id(1);
        
        // Add message to handled queue
        peer.queue_handled_message(transient_id);
        
        // Create propagation store with entry
        let mut store = PropagationStore::new();
        let entry = make_propagation_entry(transient_id, 1.0);
        store.insert(entry);
        
        // Process queues
        peer.process_queues(&mut store);
        
        // Verify queue is empty
        assert!(peer.handled_messages_queue().is_empty());
        
        // Verify peer is in handled_peers for this transient_id
        assert!(store.is_peer_handled(&transient_id, &peer_id));
    }

    #[test]
    fn test_process_queues_unhandled_messages() {
        // Test that unhandled_messages_queue is processed and peer is added to unhandled_peers
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let mut peer = Peer::new(peer_id, now_ms);
        let transient_id = make_transient_id(1);
        
        // Add message to unhandled queue
        peer.queue_unhandled_message(transient_id);
        
        // Create propagation store with entry
        let mut store = PropagationStore::new();
        let entry = make_propagation_entry(transient_id, 1.0);
        store.insert(entry);
        
        // Process queues
        peer.process_queues(&mut store);
        
        // Verify queue is empty
        assert!(peer.unhandled_messages_queue().is_empty());
        
        // Verify peer is in unhandled_peers for this transient_id
        assert!(store.is_peer_unhandled(&transient_id, &peer_id));
    }

    #[test]
    fn test_process_queues_handled_removes_from_unhandled() {
        // Test that when adding to handled, peer is removed from unhandled
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let mut peer = Peer::new(peer_id, now_ms);
        let transient_id = make_transient_id(1);
        
        // First add to unhandled
        peer.queue_unhandled_message(transient_id);
        
        // Create propagation store with entry
        let mut store = PropagationStore::new();
        let entry = make_propagation_entry(transient_id, 1.0);
        store.insert(entry);
        
        // Process unhandled queue first
        peer.process_queues(&mut store);
        assert!(store.is_peer_unhandled(&transient_id, &peer_id));
        assert!(!store.is_peer_handled(&transient_id, &peer_id));
        
        // Now add to handled queue
        peer.queue_handled_message(transient_id);
        peer.process_queues(&mut store);
        
        // Verify peer is in handled and NOT in unhandled
        assert!(store.is_peer_handled(&transient_id, &peer_id));
        assert!(!store.is_peer_unhandled(&transient_id, &peer_id));
    }

    #[test]
    fn test_process_queues_ignores_missing_entry() {
        // Test that process_queues ignores transient_ids that don't exist in store
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let mut peer = Peer::new(peer_id, now_ms);
        let transient_id = make_transient_id(1);
        
        // Add message to queue
        peer.queue_handled_message(transient_id);
        
        // Create empty propagation store (no entry for transient_id)
        let mut store = PropagationStore::new();
        
        // Process queues - should not panic
        peer.process_queues(&mut store);
        
        // Verify queue is empty (processed, but no effect since entry doesn't exist)
        assert!(peer.handled_messages_queue().is_empty());
        
        // Verify peer is NOT in handled_peers (entry doesn't exist)
        assert!(!store.is_peer_handled(&transient_id, &peer_id));
    }

    #[test]
    fn test_process_queues_multiple_messages() {
        // Test processing multiple messages in both queues
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let mut peer = Peer::new(peer_id, now_ms);
        let transient_id1 = make_transient_id(1);
        let transient_id2 = make_transient_id(2);
        let transient_id3 = make_transient_id(3);
        
        // Add multiple messages
        peer.queue_handled_message(transient_id1);
        peer.queue_unhandled_message(transient_id2);
        peer.queue_unhandled_message(transient_id3);
        
        // Create propagation store with entries
        let mut store = PropagationStore::new();
        store.insert(make_propagation_entry(transient_id1, 1.0));
        store.insert(make_propagation_entry(transient_id2, 1.0));
        store.insert(make_propagation_entry(transient_id3, 1.0));
        
        // Process queues
        peer.process_queues(&mut store);
        
        // Verify all queues are empty
        assert!(peer.handled_messages_queue().is_empty());
        assert!(peer.unhandled_messages_queue().is_empty());
        
        // Verify all peers are correctly marked
        assert!(store.is_peer_handled(&transient_id1, &peer_id));
        assert!(store.is_peer_unhandled(&transient_id2, &peer_id));
        assert!(store.is_peer_unhandled(&transient_id3, &peer_id));
    }

    #[test]
    fn test_handled_messages_property() {
        // Test that handled_messages() returns correct transient_ids
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let peer = Peer::new(peer_id, now_ms);
        let transient_id1 = make_transient_id(1);
        let transient_id2 = make_transient_id(2);
        
        // Create propagation store with entries
        let mut store = PropagationStore::new();
        store.insert(make_propagation_entry(transient_id1, 1.0));
        store.insert(make_propagation_entry(transient_id2, 1.0));
        
        // Add peer to handled for transient_id1
        store.add_handled_peer(&transient_id1, peer_id);
        
        // Get handled messages for this peer
        let handled = peer.handled_messages(&store);
        
        // Verify only transient_id1 is in handled
        assert_eq!(handled.len(), 1);
        assert!(handled.contains(&transient_id1));
        assert!(!handled.contains(&transient_id2));
    }

    #[test]
    fn test_unhandled_messages_property() {
        // Test that unhandled_messages() returns correct transient_ids
        let now_ms = 1000;
        let peer_id = make_peer_id(1);
        let peer = Peer::new(peer_id, now_ms);
        let transient_id1 = make_transient_id(1);
        let transient_id2 = make_transient_id(2);
        
        // Create propagation store with entries
        let mut store = PropagationStore::new();
        store.insert(make_propagation_entry(transient_id1, 1.0));
        store.insert(make_propagation_entry(transient_id2, 1.0));
        
        // Add peer to unhandled for transient_id1
        store.add_unhandled_peer(&transient_id1, peer_id);
        
        // Get unhandled messages for this peer
        let unhandled = peer.unhandled_messages(&store);
        
        // Verify only transient_id1 is in unhandled
        assert_eq!(unhandled.len(), 1);
        assert!(unhandled.contains(&transient_id1));
        assert!(!unhandled.contains(&transient_id2));
    }
}
