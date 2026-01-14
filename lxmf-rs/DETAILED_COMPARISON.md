# Detailed Comparison: Python LXMRouter vs Rust LxmfRouter

## Purpose
This document tracks detailed field-by-field and method-by-method comparison between Python `LXMRouter` and Rust `LxmfRouter` implementations to identify missing features, discrepancies, and areas for improvement.

## Field Comparison

### Python LXMRouter Fields (from __init__)

#### Message Queues
- `pending_inbound` (list) → Rust: `runtime.inbound` (VecDeque<Received>)
- `pending_outbound` (list) → Rust: `runtime.outbound` (VecDeque<QueuedMessage>)
- `failed_outbound` (list) → Rust: `runtime.failed` (VecDeque<FailedDelivery>)

#### Links
- `direct_links` (dict) → Rust: `link_tracker.direct_links` (HashMap) ✅
- `backchannel_links` (dict) → ⚠️ **UNUSED** - Initialized but never used in Python code
- `delivery_destinations` (dict) → ⚠️ **PARTIAL** - Used only for announce() method, only one destination supported
- `outbound_propagation_link` → Rust: `link_tracker.outbound_propagation_link` (Option<PropagationLinkInfo>) ✅
- `active_propagation_links` (list) → Rust: `link_tracker.active_propagation_links` (Vec<PropagationLinkInfo>) ✅
- `validated_peer_links` (dict) → Rust: `link_tracker.validated_peer_links` (HashSet) ✅ (exists but may not be fully utilized)

#### Access Control Lists
- `prioritised_list` (list) → Rust: `propagation_prioritised` (HashSet)
- `ignored_list` (list) → Rust: `ignored_destinations` (HashSet)
- `allowed_list` (list) → Rust: `allowed_identities` (HashSet)
- `control_allowed_list` (list) → Rust: `control_allowed` (HashSet)

#### Configuration Flags
- `auth_required` (bool) → Rust: `auth_required` (bool) ✅
- `retain_synced_on_node` (bool) → ❌ **MISSING** - Not implemented in Rust
- `from_static_only` (bool) → Rust: `from_static_only` (bool) ✅
- `enforce_ratchets` (bool) → ❌ **MISSING** - Not implemented in Rust
- `_enforce_stamps` (bool) → Rust: `enforce_stamps` (bool) ✅
- `prioritise_rotating_unreachable_peers` (bool) → Rust: `prioritise_rotating_unreachable_peers` (bool) ✅

#### Sync Strategy
- `default_sync_strategy` → ❌ **MISSING** - Not implemented in Rust (LXMPeer.STRATEGY_PERSISTENT)

#### Processing State
- `processing_inbound` (bool) → ❌ **MISSING** - Not implemented in Rust
- `processing_count` (int) → Rust: `runtime.processing_count` (u64) ✅
- `name` (str) → ❌ **MISSING** - Not implemented in Rust

#### Propagation Node State
- `propagation_node` (bool) → Rust: `propagation_node_enabled` (bool) ✅
- `propagation_node_start_time` (float) → Rust: `propagation_started_ms` (u64) ✅
- `outbound_propagation_node` (hash) → ❌ **MISSING** - Not directly stored (only link info)

#### Storage Paths
- `storagepath` (str) → Rust: `persistence_root` (Option<PathBuf>) ✅
- `ratchetpath` (str) → ❌ **MISSING** - Not implemented in Rust

#### Transfer Limits
- `message_storage_limit` → Rust: `propagation_store_limit_bytes` (Option<u64>) ✅
- `information_storage_limit` → ❌ **MISSING** - Not implemented in Rust
- `propagation_per_transfer_limit` → Rust: `propagation_transfer_limit_kb` (f64) ✅
- `propagation_per_sync_limit` → Rust: `propagation_sync_limit_kb` (f64) ✅
- `delivery_per_transfer_limit` → Rust: `delivery_transfer_limit_kb` (f64) ✅

#### Stamp Costs
- `propagation_stamp_cost` → Rust: `propagation_validation.stamp_cost` (u32) ✅
- `propagation_stamp_cost_flexibility` → Rust: `propagation_validation.stamp_cost_flex` (u32) ✅
- `peering_cost` → Rust: `peering_validation.target_cost` (u32) ✅
- `max_peering_cost` → Rust: `max_peering_cost` (u32) ✅
- `outbound_stamp_costs` (dict) → Rust: `outbound_stamp_costs` (HashMap) ✅
- `inbound_stamp_costs` → Rust: `inbound_stamp_costs` (HashMap) ✅

#### Deferred Processing
- `pending_deferred_stamps` (dict) → Rust: `deferred_outbound` (VecDeque<DeferredOutbound>) ✅
- `throttled_peers` (dict) → Rust: `throttled_peers` (HashMap) ✅

#### Propagation Transfer State
- `wants_download_on_path_available_from` → Rust: `wants_download_on_path_available_from` (Option<[u8; 16]>) ✅
- `wants_download_on_path_available_to` → Rust: `wants_download_on_path_available_to` (Option<[u8; 16]>) ✅
- `propagation_transfer_state` → Rust: `propagation_transfer_state` (u8) ✅
- `propagation_transfer_progress` → Rust: `propagation_transfer_progress` (f64) ✅
- `propagation_transfer_last_result` → Rust: `propagation_transfer_last_result` (Option<u32>) ✅
- `propagation_transfer_last_duplicates` → ❌ **MISSING** - Not implemented in Rust
- `propagation_transfer_max_messages` → ❌ **MISSING** - Not implemented in Rust

#### Local State Tracking
- `locally_delivered_transient_ids` (dict) → Rust: `local_deliveries` (HashMap) ✅
- `locally_processed_transient_ids` (dict) → Rust: `locally_processed` (HashMap) ✅

#### Tickets
- `available_tickets` (dict with "outbound", "inbound", "last_deliveries") → Rust: `ticket_store` (TicketStore) ✅

#### Threading Locks
- `outbound_processing_lock` → ❌ **MISSING** - Rust uses single-threaded or Arc<Mutex>
- `cost_file_lock` → ❌ **MISSING** - Not needed in Rust design
- `ticket_file_lock` → ❌ **MISSING** - Not needed in Rust design
- `stamp_gen_lock` → ❌ **MISSING** - Rust uses separate thread for stamp generation
- `exit_handler_running` → ❌ **MISSING** - Not implemented in Rust

#### Identity and Destinations
- `identity` (RNS.Identity) → Rust: `runtime.router().local_identity()` ✅
- `propagation_destination` (RNS.Destination) → ❌ **MISSING** - Not stored directly
- `control_destination` (RNS.Destination) → ❌ **MISSING** - Not stored directly

#### Statistics
- `client_propagation_messages_received` → Rust: `node_stats.client_propagation_messages_received` (u64) ✅
- `client_propagation_messages_served` → Rust: `node_stats.client_propagation_messages_served` (u64) ✅
- `unpeered_propagation_incoming` → Rust: `node_stats.unpeered_propagation_incoming` (u64) ✅
- `unpeered_propagation_rx_bytes` → Rust: `node_stats.unpeered_propagation_rx_bytes` (u64) ✅

#### Peer Management
- `autopeer` (bool) → ❌ **MISSING** - Not implemented in Rust
- `autopeer_maxdepth` (int) → Rust: `autopeer_maxdepth` (Option<u32>) ✅
- `max_peers` (int) → Rust: `max_peers` (Option<u32>) ✅
- `static_peers` (list) → Rust: `static_peers` (HashSet) ✅
- `peers` (dict) → Rust: `peers` (PeerTable) ✅
- `propagation_entries` (dict) → Rust: `propagation_store` (PropagationStore) ✅
- `peer_distribution_queue` (deque) → Rust: `peer_distribution_queue` (VecDeque) ✅

## Missing Fields Summary

### Critical Missing Fields (High Priority)
1. **`retain_synced_on_node`** - Flag to retain synced messages on node (used in propagation logic)
2. **`enforce_ratchets`** - Ratchet enforcement flag (security feature)
3. **`default_sync_strategy`** - Default peer sync strategy (LXMPeer.STRATEGY_PERSISTENT)
4. **`processing_inbound`** - Processing state flag (prevents concurrent processing)
5. **`name`** - Router name/identifier (used in propagation node announcements)
6. **`outbound_propagation_node`** - Direct storage of outbound propagation node hash (used for path requests)
7. **`propagation_transfer_last_duplicates`** - Last transfer duplicates count (tracking)
8. **`propagation_transfer_max_messages`** - Max messages for transfer (used in request_messages_from_propagation_node)
9. **`propagation_destination`** - RNS Destination object for propagation (used for announcements)
10. **`control_destination`** - RNS Destination object for control (used for control requests)
11. **`autopeer`** - Autopeer flag (controls automatic peering behavior)

### Medium Priority Missing Fields
12. **`delivery_destinations`** - Delivery destination tracking (used only for announce(), single destination supported)
13. **`ratchetpath`** - Ratchet storage path (for future ratchet support)
14. **`information_storage_limit`** - Information storage limit (separate from message storage)

### Low Priority / Unused Fields
15. **`backchannel_links`** - Initialized but never used in Python code

### Less Critical Missing Fields
- Threading locks (not needed in Rust design)
- Exit handler flag (not needed in Rust design)

## Method Comparison

### Python LXMRouter Methods (99 methods found)

#### Initialization & Configuration
- `__init__()` → Rust: `new()`, `with_config()`, `with_store()` ✅
- `announce()` → ❌ **MISSING** - Announce delivery destination
- `get_propagation_node_announce_metadata()` → ❌ **MISSING** - Get PN metadata (uses `name` field)
- `get_propagation_node_app_data()` → ❌ **MISSING** - Get PN app data for announcements
- `announce_propagation_node()` → ❌ **MISSING** - Announce propagation node
- `register_delivery_identity()` → ❌ **MISSING** - Register delivery identity (uses `delivery_destinations`, `ratchetpath`)
- `register_delivery_callback()` → ❌ **MISSING** - Register delivery callback

#### Propagation Node Management
- `set_active_propagation_node()` → ❌ **MISSING** - Set active PN (calls set_outbound_propagation_node)
- `set_outbound_propagation_node()` → ❌ **MISSING** - Set outbound PN (uses `outbound_propagation_node` field)
- `get_outbound_propagation_node()` → ❌ **MISSING** - Get outbound PN hash
- `get_outbound_propagation_cost()` → ❌ **MISSING** - Get PN stamp cost (requests path if needed)
- `set_inbound_propagation_node()` → ⚠️ **NOT_IMPLEMENTED** - Not implemented in Python either
- `get_inbound_propagation_node()` → ⚠️ **NOT_IMPLEMENTED** - Returns get_outbound_propagation_node()
- `set_retain_node_lxms()` → ❌ **MISSING** - Set retain_synced_on_node flag
- `request_messages_from_propagation_node()` → ❌ **MISSING** - Request messages from PN (uses `propagation_transfer_max_messages`)
- `cancel_propagation_node_requests()` → ❌ **MISSING** - Cancel PN requests
- `enable_propagation()` → Rust: `enable_propagation_node()` ✅
- `disable_propagation()` → Rust: `enable_propagation_node(false)` ✅

#### Access Control
- `set_authentication()` → Rust: `set_authentication()` ✅
- `requires_authentication()` → Rust: `auth_required()` ✅
- `allow()` → Rust: `allow_identity()` ✅
- `disallow()` → ❌ **MISSING** - Remove from allowed list
- `allow_control()` → Rust: `allow_control()` ✅
- `disallow_control()` → ❌ **MISSING** - Remove from control allowed
- `prioritise()` → Rust: `prioritise_destination()` ✅
- `unprioritise()` → Rust: `unprioritise_destination()` ✅
- `ignore_destination()` → Rust: `ignore_destination()` ✅
- `unignore_destination()` → Rust: `unignore_destination()` ✅

#### Stamp Costs & Tickets
- `set_inbound_stamp_cost()` → Rust: `set_inbound_stamp_cost()` ✅
- `get_outbound_stamp_cost()` → Rust: `get_outbound_stamp_cost()` ✅
- `update_stamp_cost()` → ❌ **MISSING** - Update stamp cost (used internally)
- `generate_ticket()` → Rust: `generate_ticket()` ✅
- `remember_ticket()` → Rust: `remember_ticket()` ✅
- `get_outbound_ticket()` → Rust: `get_outbound_ticket()` ✅
- `get_outbound_ticket_expiry()` → ❌ **MISSING** - Get ticket expiry
- `get_inbound_tickets()` → ⚠️ **PARTIAL** - Exists in TicketStore but not exposed as method

#### Storage & Limits
- `set_message_storage_limit()` → Rust: `set_message_storage_limit()` ✅
- `message_storage_limit()` → Rust: `message_storage_limit_bytes()` ✅
- `message_storage_size()` → Rust: `propagation_store_size_bytes()` ✅
- `set_information_storage_limit()` → ❌ **MISSING** - Set information storage limit
- `information_storage_limit()` → ❌ **MISSING** - Get information storage limit
- `information_storage_size()` → ❌ **MISSING** - Get information storage size

#### Message Handling
- `has_message()` → ❌ **MISSING** - Check if message exists in propagation store
- `handle_outbound()` → Rust: `handle_outbound()` ✅
- `cancel_outbound()` → ❌ **MISSING** - Cancel outbound message
- `get_outbound_progress()` → ❌ **MISSING** - Get outbound message progress
- `get_outbound_lxm_stamp_cost()` → ❌ **MISSING** - Get stamp cost for outbound message
- `get_outbound_lxm_propagation_stamp_cost()` → ❌ **MISSING** - Get propagation stamp cost
- `lxmf_delivery()` → ⚠️ **PARTIAL** - Exists as `process_inbound_message()` with different signature
- `delivery_packet()` → ❌ **MISSING** - Handle delivery packet
- `delivery_link_established()` → ❌ **MISSING** - Delivery link established callback
- `delivery_link_closed()` → ❌ **MISSING** - Delivery link closed callback
- `delivery_resource_advertised()` → ❌ **MISSING** - Resource advertised callback
- `delivery_resource_concluded()` → ❌ **MISSING** - Resource concluded callback
- `delivery_remote_identified()` → ❌ **MISSING** - Remote identified callback
- `resource_transfer_began()` → ❌ **MISSING** - Resource transfer began callback

#### Propagation Store Helpers
- `get_size()` → ❌ **MISSING** - Get message size from propagation store
- `get_weight()` → ❌ **MISSING** - Get message weight from propagation store
- `get_stamp_value()` → Rust: `propagation_store.stamp_value()` ✅
- `get_announce_app_data()` → ❌ **MISSING** - Get announce app data for delivery destination

#### Peer Management
- `peer()` → ⚠️ **PARTIAL** - Exists as `upsert_peer()` with different signature
- `unpeer()` → Rust: `unpeer()` ✅
- `rotate_peers()` → Rust: `rotate_peers()` ✅
- `sync_peers()` → Rust: `sync_peers()` ✅

#### Daemon Loop & Jobs
- `jobs()` → Rust: `runtime.tick()` ✅
- `jobloop()` → ❌ **MISSING** - Background job loop (daemon thread)
- `flush_queues()` → Rust: `flush_queues()` ✅
- `clean_links()` → Rust: `clean_links()` ✅
- `clean_transient_id_caches()` → Rust: `clean_transient_id_caches()` ✅
- `clean_throttled_peers()` → Rust: `clean_throttled_peers()` ✅
- `clean_message_store()` → Rust: `clean_propagation()` ✅

#### Persistence & Cleanup
- `save_locally_delivered_transient_ids()` → ⚠️ **PARTIAL** - Exists in FileMessageStore
- `save_locally_processed_transient_ids()` → ⚠️ **PARTIAL** - Exists in FileMessageStore
- `save_node_stats()` → ❌ **MISSING** - Save node statistics
- `clean_outbound_stamp_costs()` → Rust: `clean_outbound_stamp_costs()` ✅
- `save_outbound_stamp_costs()` → ⚠️ **PARTIAL** - Exists in FileMessageStore
- `clean_available_tickets()` → Rust: `clean_tickets()` ✅
- `save_available_tickets()` → ⚠️ **PARTIAL** - Exists in TicketStore
- `reload_available_tickets()` → ❌ **MISSING** - Reload tickets from storage

#### Control Requests
- `stats_get_request()` → ❌ **MISSING** - Handle stats get request
- `peer_sync_request()` → ❌ **MISSING** - Handle peer sync request
- `peer_unpeer_request()` → ❌ **MISSING** - Handle peer unpeer request
- `message_get_request()` → ❌ **MISSING** - Handle message get request
- `message_list_response()` → ❌ **MISSING** - Handle message list response
- `message_get_response()` → ❌ **MISSING** - Handle message get response
- `message_get_progress()` → ❌ **MISSING** - Handle message get progress
- `message_get_failed()` → ❌ **MISSING** - Handle message get failed

#### Propagation Transfer
- `acknowledge_sync_completion()` → Rust: `acknowledge_sync_completion()` ✅
- `request_messages_path_job()` → ❌ **MISSING** - Path request job for message requests
- `__request_messages_path_job()` → ❌ **MISSING** - Internal path request job

#### Utility Methods
- `compile_stats()` → Rust: `compile_stats()` ✅
- `delivery_link_available()` → ❌ **MISSING** - Check if delivery link available
- `identity_allowed()` → ❌ **MISSING** - Check if identity allowed
- `enforce_stamps()` → Rust: `set_enforce_stamps(true)` ✅
- `ignore_stamps()` → Rust: `set_enforce_stamps(false)` ✅
- `__str__()` → ❌ **MISSING** - String representation

#### Signal Handlers
- `exit_handler()` → ❌ **MISSING** - Exit handler (atexit)
- `sigint_handler()` → ❌ **MISSING** - SIGINT handler
- `sigterm_handler()` → ❌ **MISSING** - SIGTERM handler

## Constants Comparison

### Python Constants
- `MAX_DELIVERY_ATTEMPTS = 5` → ❌ **MISSING**
- `PROCESSING_INTERVAL = 4` → Rust: `JOB_*_INTERVAL` constants ✅
- `DELIVERY_RETRY_WAIT = 10` → ❌ **MISSING**
- `PATH_REQUEST_WAIT = 7` → ❌ **MISSING**
- `MAX_PATHLESS_TRIES = 1` → ❌ **MISSING**
- `LINK_MAX_INACTIVITY = 10*60` → Rust: `LINK_MAX_INACTIVITY` ✅
- `P_LINK_MAX_INACTIVITY = 3*60` → Rust: `P_LINK_MAX_INACTIVITY` ✅
- `MESSAGE_EXPIRY = 30*24*60*60` → Rust: `MESSAGE_EXPIRY` ✅
- `STAMP_COST_EXPIRY = 45*24*60*60` → Rust: `STAMP_COST_EXPIRY` ✅
- `NODE_ANNOUNCE_DELAY = 20` → ❌ **MISSING**
- `MAX_PEERS = 20` → Rust: Default in `max_peers` ✅
- `AUTOPEER = True` → ❌ **MISSING** (field exists but constant not used)
- `AUTOPEER_MAXDEPTH = 4` → Rust: Default in `autopeer_maxdepth` ✅
- `FASTEST_N_RANDOM_POOL = 2` → Rust: `FASTEST_N_RANDOM_POOL` ✅
- `ROTATION_HEADROOM_PCT = 10` → Rust: `ROTATION_HEADROOM_PCT` ✅
- `ROTATION_AR_MAX = 0.5` → Rust: `ROTATION_AR_MAX` ✅
- `PEERING_COST = 18` → Rust: `DEFAULT_PEERING_COST` ✅
- `MAX_PEERING_COST = 26` → Rust: `max_peering_cost` default ✅
- `PROPAGATION_COST_MIN = 13` → Rust: `PROPAGATION_COST_MIN` ✅
- `PROPAGATION_COST_FLEX = 3` → Rust: `PROPAGATION_COST_FLEX` ✅
- `PROPAGATION_COST = 16` → Rust: Default in `propagation_validation` ✅
- `PROPAGATION_LIMIT = 256` → Rust: `propagation_transfer_limit_kb` default ✅
- `SYNC_LIMIT = PROPAGATION_LIMIT*40` → Rust: `propagation_sync_limit_kb` default ✅
- `DELIVERY_LIMIT = 1000` → Rust: `delivery_transfer_limit_kb` default ✅
- `PR_PATH_TIMEOUT = 10` → ❌ **MISSING**
- `PN_STAMP_THROTTLE = 180` → ❌ **MISSING**
- `PR_*` constants → Rust: `PR_*` constants ✅
- `PR_ALL_MESSAGES = 0x00` → ❌ **MISSING**
- `STATS_GET_PATH = "/pn/get/stats"` → ❌ **MISSING**
- `SYNC_REQUEST_PATH = "/pn/peer/sync"` → ❌ **MISSING**
- `UNPEER_REQUEST_PATH = "/pn/peer/unpeer"` → ❌ **MISSING**
- `DUPLICATE_SIGNAL = "lxmf_duplicate"` → ❌ **MISSING**

## Summary Statistics

### Fields
- **Total Python fields**: ~50
- **Implemented in Rust**: ~35 (70%)
- **Missing critical**: 11 (22%)
- **Missing medium priority**: 3 (6%)
- **Unused/Low priority**: 1 (2%)

### Methods
- **Total Python methods**: 99
- **Fully implemented**: ~25 (25%)
- **Partially implemented**: ~10 (10%)
- **Missing**: ~64 (65%)

### Constants
- **Total Python constants**: ~35
- **Implemented in Rust**: ~20 (57%)
- **Missing**: ~15 (43%)

## Next Steps
1. Prioritize missing critical fields and methods
2. Create implementation plan with stages
3. Document behavioral differences for partially implemented methods
4. Track progress systematically
