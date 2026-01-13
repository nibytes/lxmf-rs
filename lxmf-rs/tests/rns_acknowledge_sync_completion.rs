#[cfg(test)]
#[cfg(feature = "rns")]
mod tests {
    use lxmf_rs::{LxmfRouter, RnsRouter, PR_IDLE, PR_COMPLETE, PR_LINK_FAILED, PR_TRANSFER_FAILED, DESTINATION_LENGTH};

    fn make_router() -> LxmfRouter {
        use lxmf_rs::{RnsOutbound, RnsRouter};
        use reticulum::identity::PrivateIdentity;
        
        let source = PrivateIdentity::new_from_name("test-src");
        let destination = PrivateIdentity::new_from_name("test-dst").as_identity().clone();
        let delivery = RnsOutbound::new(source, destination);
        let rns_router = RnsRouter::new(delivery);
        LxmfRouter::new(rns_router)
    }

    fn make_destination_hash(seed: u8) -> [u8; DESTINATION_LENGTH] {
        let mut hash = [0u8; DESTINATION_LENGTH];
        hash[0] = seed;
        hash
    }

    #[test]
    fn test_acknowledge_sync_completion_resets_last_result() {
        // Test that acknowledge_sync_completion() resets propagation_transfer_last_result to None
        let mut router = make_router();
        router.set_propagation_transfer_last_result(Some(42));
        
        router.acknowledge_sync_completion(false, None);
        
        assert_eq!(router.propagation_transfer_last_result(), None);
    }

    #[test]
    fn test_acknowledge_sync_completion_resets_progress() {
        // Test that acknowledge_sync_completion() resets propagation_transfer_progress to 0.0
        let mut router = make_router();
        router.set_propagation_transfer_progress(0.75);
        
        router.acknowledge_sync_completion(false, None);
        
        assert_eq!(router.propagation_transfer_progress(), 0.0);
    }

    #[test]
    fn test_acknowledge_sync_completion_resets_wants_download() {
        // Test that acknowledge_sync_completion() resets wants_download_on_path_available_from/to to None
        let mut router = make_router();
        let from = make_destination_hash(1);
        let to = make_destination_hash(2);
        router.set_wants_download_on_path_available_from(Some(from));
        router.set_wants_download_on_path_available_to(Some(to));
        
        router.acknowledge_sync_completion(false, None);
        
        assert_eq!(router.wants_download_on_path_available_from(), None);
        assert_eq!(router.wants_download_on_path_available_to(), None);
    }

    #[test]
    fn test_acknowledge_sync_completion_sets_idle_when_no_failure_state() {
        // Test that acknowledge_sync_completion() sets state to PR_IDLE when failure_state is None
        let mut router = make_router();
        router.set_propagation_transfer_state(PR_COMPLETE);
        
        router.acknowledge_sync_completion(false, None);
        
        assert_eq!(router.propagation_transfer_state(), PR_IDLE);
    }

    #[test]
    fn test_acknowledge_sync_completion_sets_failure_state_when_provided() {
        // Test that acknowledge_sync_completion() sets state to failure_state when provided
        let mut router = make_router();
        router.set_propagation_transfer_state(PR_COMPLETE);
        
        router.acknowledge_sync_completion(false, Some(PR_LINK_FAILED));
        
        assert_eq!(router.propagation_transfer_state(), PR_LINK_FAILED);
    }

    #[test]
    fn test_acknowledge_sync_completion_reset_state_true() {
        // Test that acknowledge_sync_completion(reset_state=true) always resets state
        let mut router = make_router();
        router.set_propagation_transfer_state(PR_COMPLETE);
        
        router.acknowledge_sync_completion(true, None);
        
        assert_eq!(router.propagation_transfer_state(), PR_IDLE);
    }

    #[test]
    fn test_acknowledge_sync_completion_reset_state_with_failure() {
        // Test that acknowledge_sync_completion(reset_state=true, failure_state=X) sets state to X
        let mut router = make_router();
        router.set_propagation_transfer_state(PR_COMPLETE);
        
        router.acknowledge_sync_completion(true, Some(PR_TRANSFER_FAILED));
        
        assert_eq!(router.propagation_transfer_state(), PR_TRANSFER_FAILED);
    }

    #[test]
    fn test_acknowledge_sync_completion_when_state_below_complete() {
        // Test that acknowledge_sync_completion() resets state when current state <= PR_COMPLETE
        let mut router = make_router();
        router.set_propagation_transfer_state(PR_COMPLETE);
        
        router.acknowledge_sync_completion(false, None);
        
        assert_eq!(router.propagation_transfer_state(), PR_IDLE);
    }

    #[test]
    fn test_acknowledge_sync_completion_called_from_clean_links_when_complete() {
        // Test that clean_links() calls acknowledge_sync_completion() when link is closed and state is PR_COMPLETE
        let mut router = make_router();
        router.set_propagation_transfer_state(PR_COMPLETE);
        router.set_propagation_transfer_last_result(Some(10));
        router.set_propagation_transfer_progress(1.0);
        let link_id = make_destination_hash(1);
        router.set_outbound_propagation_link(link_id, 0);
        router.mark_outbound_propagation_link_closed();
        
        router.clean_links(1000);
        
        assert_eq!(router.propagation_transfer_state(), PR_IDLE);
        assert_eq!(router.propagation_transfer_last_result(), None);
        assert_eq!(router.propagation_transfer_progress(), 0.0);
    }

    #[test]
    fn test_acknowledge_sync_completion_called_from_clean_links_when_link_failed() {
        // Test that clean_links() calls acknowledge_sync_completion(failure_state=PR_LINK_FAILED) when state < PR_LINK_ESTABLISHED
        let mut router = make_router();
        router.set_propagation_transfer_state(0x01); // PR_PATH_REQUESTED < PR_LINK_ESTABLISHED
        let link_id = make_destination_hash(1);
        router.set_outbound_propagation_link(link_id, 0);
        router.mark_outbound_propagation_link_closed();
        
        router.clean_links(1000);
        
        assert_eq!(router.propagation_transfer_state(), PR_LINK_FAILED);
        assert_eq!(router.propagation_transfer_last_result(), None);
        assert_eq!(router.propagation_transfer_progress(), 0.0);
    }

    #[test]
    fn test_acknowledge_sync_completion_called_from_clean_links_when_transfer_failed() {
        // Test that clean_links() calls acknowledge_sync_completion(failure_state=PR_TRANSFER_FAILED) when PR_LINK_ESTABLISHED <= state < PR_COMPLETE
        let mut router = make_router();
        router.set_propagation_transfer_state(0x05); // PR_RECEIVING, between PR_LINK_ESTABLISHED and PR_COMPLETE
        let link_id = make_destination_hash(1);
        router.set_outbound_propagation_link(link_id, 0);
        router.mark_outbound_propagation_link_closed();
        
        router.clean_links(1000);
        
        assert_eq!(router.propagation_transfer_state(), PR_TRANSFER_FAILED);
        assert_eq!(router.propagation_transfer_last_result(), None);
        assert_eq!(router.propagation_transfer_progress(), 0.0);
    }
}
