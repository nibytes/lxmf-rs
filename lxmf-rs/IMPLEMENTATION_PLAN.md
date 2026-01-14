# Implementation Plan - Missing Features

Based on comprehensive comparison between Python LXMF and Rust lxmf-rs implementations.

**See**:
- `DETAILED_COMPARISON.md` - Detailed LXMRouter comparison
- `FULL_COMPARISON.md` - Complete component-by-component comparison

## Current Status

**Phase 1: Foundation (P0) - ✅ COMPLETED**
- All 8 stages implemented and tested
- ~150+ tests passing
- Core router and peer functionality at parity with Python

**Next**: Phase 2: Core Features (P1)

## Priority Levels
- **P0 (Critical)**: Blocks core functionality, required for parity
- **P1 (High)**: Important features, affects user experience
- **P2 (Medium)**: Nice to have, improves functionality
- **P3 (Low)**: Optional features, edge cases

## Stage 8: Critical Missing Fields

### 8.1: Router Identity & Naming (P0)
**Status**: ✅ Completed

**Fields to add**:
- `name: Option<String>` - Router name/identifier
- `autopeer: bool` - Autopeer flag (default: true)

**Methods to implement**:
- `set_name(name: Option<String>)`
- `name() -> Option<&str>`
- `set_autopeer(enable: bool)`
- `autopeer() -> bool`

**Tests**: 3-4 tests for name and autopeer functionality

**Estimated effort**: 1-2 hours

---

### 8.2: Propagation Node Management Fields (P0)
**Status**: ✅ Completed

**Fields to add**:
- `outbound_propagation_node: Option<[u8; DESTINATION_LENGTH]>` - Direct storage of outbound PN hash
- `propagation_transfer_max_messages: Option<u32>` - Max messages for transfer
- `propagation_transfer_last_duplicates: Option<u32>` - Last transfer duplicates count

**Methods to implement**:
- `set_outbound_propagation_node(hash: Option<[u8; DESTINATION_LENGTH]>)`
- `get_outbound_propagation_node() -> Option<[u8; DESTINATION_LENGTH]>`
- `set_propagation_transfer_max_messages(max: Option<u32>)`
- `propagation_transfer_max_messages() -> Option<u32>`
- `set_propagation_transfer_last_duplicates(count: Option<u32>)`
- `propagation_transfer_last_duplicates() -> Option<u32>`

**Tests**: 5-6 tests for PN management

**Estimated effort**: 2-3 hours

---

### 8.3: Processing State & Sync Strategy (P1)
**Status**: Not started

**Fields to add**:
- `processing_inbound: bool` - Processing state flag
- `default_sync_strategy: u8` - Default peer sync strategy (LXMPeer.STRATEGY_PERSISTENT)
- `retain_synced_on_node: bool` - Flag to retain synced messages on node

**Methods to implement**:
- `set_processing_inbound(processing: bool)`
- `processing_inbound() -> bool`
- `set_default_sync_strategy(strategy: u8)`
- `default_sync_strategy() -> u8`
- `set_retain_node_lxms(retain: bool)`
- `retain_synced_on_node() -> bool`

**Tests**: 4-5 tests for state management

**Estimated effort**: 1-2 hours

---

### 8.4: Ratchet & Destination Support (P2)
**Status**: Not started

**Fields to add**:
- `enforce_ratchets: bool` - Ratchet enforcement flag
- `ratchetpath: Option<PathBuf>` - Ratchet storage path
- `delivery_destinations: HashMap<[u8; DESTINATION_LENGTH], DeliveryDestination>` - Delivery destination tracking
- `propagation_destination: Option<RnsDestination>` - RNS Destination for propagation
- `control_destination: Option<RnsDestination>` - RNS Destination for control

**Methods to implement**:
- `set_enforce_ratchets(enforce: bool)`
- `enforce_ratchets() -> bool`
- `set_ratchetpath(path: Option<PathBuf>)`
- `ratchetpath() -> Option<&Path>`
- `register_delivery_identity(...)` - Complex, requires RNS integration
- `announce(destination_hash, attached_interface)` - Requires RNS integration

**Tests**: 6-8 tests (depends on RNS integration)

**Estimated effort**: 4-6 hours (depends on RNS API availability)

---

### 8.5: Information Storage Limit (P2)
**Status**: Not started

**Fields to add**:
- `information_storage_limit_bytes: Option<u64>` - Information storage limit

**Methods to implement**:
- `set_information_storage_limit(kb/mb/gb)`
- `information_storage_limit_bytes() -> Option<u64>`
- `information_storage_size() -> usize`

**Tests**: 3-4 tests

**Estimated effort**: 1 hour

---

## Stage 9: Critical Missing Methods

### 9.1: Propagation Node Announcements (P0)
**Status**: ✅ Completed

**Methods to implement**:
- `get_propagation_node_announce_metadata() -> HashMap<String, Vec<u8>>` - Uses `name` field
- `get_propagation_node_app_data() -> Vec<u8>` - Creates announce data
- `announce_propagation_node()` - Announces PN with delay

**Dependencies**: Stage 8.1 (name field)

**Tests**: 5-6 tests for announcement data generation

**Estimated effort**: 2-3 hours

---

### 9.2: Propagation Node Request Methods (P0)
**Status**: ✅ Completed

**Methods to implement**:
- `request_messages_from_propagation_node(identity, max_messages)` - Request messages from PN
- `cancel_propagation_node_requests()` - Cancel PN requests
- `get_outbound_propagation_cost() -> Option<u32>` - Get PN stamp cost (requests path if needed)
- `request_messages_path_job()` - Path request job
- `__request_messages_path_job()` - Internal path request

**Dependencies**: Stage 8.2 (outbound_propagation_node, propagation_transfer_max_messages)

**Tests**: 8-10 tests for PN request flow

**Estimated effort**: 4-5 hours

---

### 9.3: Access Control Methods (P1)
**Status**: Not started

**Methods to implement**:
- `disallow(identity_hash)` - Remove from allowed list
- `disallow_control(identity_hash)` - Remove from control allowed

**Tests**: 2-3 tests

**Estimated effort**: 30 minutes

---

### 9.4: Message Store Helpers (P1)
**Status**: Not started

**Methods to implement**:
- `has_message(transient_id) -> bool` - Check if message exists
- `get_size(transient_id) -> Option<usize>` - Get message size
- `get_weight(transient_id) -> Option<f64>` - Get message weight
- `get_announce_app_data(destination_hash) -> Vec<u8>` - Get announce app data

**Tests**: 4-5 tests

**Estimated effort**: 1-2 hours

---

### 9.5: Outbound Message Management (P1)
**Status**: Not started

**Methods to implement**:
- `cancel_outbound(message_id, cancel_state)` - Cancel outbound message
- `get_outbound_progress(lxm_hash) -> Option<f64>` - Get outbound progress
- `get_outbound_lxm_stamp_cost(lxm_hash) -> Option<u32>` - Get stamp cost
- `get_outbound_lxm_propagation_stamp_cost(lxm_hash) -> Option<u32>` - Get propagation stamp cost

**Tests**: 6-8 tests

**Estimated effort**: 2-3 hours

---

### 9.6: Delivery Callbacks & Resource Handling (P2)
**Status**: Not started

**Methods to implement**:
- `register_delivery_callback(callback)` - Register delivery callback
- `delivery_packet(data, packet)` - Handle delivery packet
- `delivery_link_established(link)` - Link established callback
- `delivery_link_closed(link)` - Link closed callback
- `delivery_resource_advertised(resource)` - Resource advertised callback
- `delivery_resource_concluded(resource)` - Resource concluded callback
- `delivery_remote_identified(link, identity)` - Remote identified callback
- `resource_transfer_began(resource)` - Resource transfer began callback

**Dependencies**: RNS integration, callback mechanism

**Tests**: 8-10 tests (depends on RNS integration)

**Estimated effort**: 4-6 hours

---

### 9.7: Control Request Handlers (P1)
**Status**: Not started

**Methods to implement**:
- `stats_get_request(path, data, request_id, remote_identity, requested_at)` - Handle stats request
- `peer_sync_request(...)` - Handle peer sync request
- `peer_unpeer_request(...)` - Handle peer unpeer request
- `message_get_request(...)` - Handle message get request
- `message_list_response(...)` - Handle message list response
- `message_get_response(...)` - Handle message get response
- `message_get_progress(...)` - Handle message get progress
- `message_get_failed(...)` - Handle message get failed

**Tests**: 10-12 tests

**Estimated effort**: 5-6 hours

---

### 9.8: Utility Methods (P2)
**Status**: Not started

**Methods to implement**:
- `delivery_link_available(destination_hash) -> bool` - Check if delivery link available
- `identity_allowed(identity) -> bool` - Check if identity allowed
- `get_outbound_ticket_expiry(destination_hash) -> Option<f64>` - Get ticket expiry
- `reload_available_tickets()` - Reload tickets from storage
- `save_node_stats()` - Save node statistics
- `__str__()` / `Display` trait - String representation

**Tests**: 6-8 tests

**Estimated effort**: 2-3 hours

---

### 9.9: Signal Handlers & Daemon Loop (P2)
**Status**: Not started

**Methods to implement**:
- `jobloop()` - Background job loop (daemon thread)
- `exit_handler()` - Exit handler (cleanup on exit)
- `sigint_handler()` - SIGINT handler
- `sigterm_handler()` - SIGTERM handler

**Note**: Rust uses different patterns (Drop trait, signal handling crates)

**Tests**: 4-5 tests

**Estimated effort**: 2-3 hours

---

## Stage 10: Missing Constants

### 10.1: Router Constants (P1)
**Status**: ✅ Completed

**Constants to add**:
- `MAX_DELIVERY_ATTEMPTS = 5`
- `DELIVERY_RETRY_WAIT = 10`
- `PATH_REQUEST_WAIT = 7`
- `MAX_PATHLESS_TRIES = 1`
- `NODE_ANNOUNCE_DELAY = 20`
- `PR_PATH_TIMEOUT = 10`
- `PN_STAMP_THROTTLE = 180`
- `PR_ALL_MESSAGES = 0x00`
- `STATS_GET_PATH = "/pn/get/stats"`
- `SYNC_REQUEST_PATH = "/pn/peer/sync"`
- `UNPEER_REQUEST_PATH = "/pn/peer/unpeer"`
- `DUPLICATE_SIGNAL = "lxmf_duplicate"`

**Tests**: Verify constants match Python values

**Estimated effort**: 30 minutes

---

### 10.2: Peer Constants (P1)
**Status**: ✅ Completed

**Constants to add**:
- `OFFER_REQUEST_PATH = "/offer"`
- `MESSAGE_GET_PATH = "/get"`
- `LINK_ESTABLISHING = 0x01`
- `LINK_READY = 0x02`
- `RESOURCE_TRANSFERRING = 0x05`
- `ERROR_NO_IDENTITY = 0xf0`
- `ERROR_NO_ACCESS = 0xf1`
- `ERROR_INVALID_DATA = 0xf4`
- `ERROR_INVALID_STAMP = 0xf5`
- `ERROR_THROTTLED = 0xf6`
- `ERROR_NOT_FOUND = 0xfd`
- `ERROR_TIMEOUT = 0xfe`
- `STRATEGY_LAZY = 0x01`
- `STRATEGY_PERSISTENT = 0x02`
- `DEFAULT_SYNC_STRATEGY = STRATEGY_PERSISTENT`
- `PATH_REQUEST_GRACE = 7.5`

**Tests**: Verify constants match Python values

**Estimated effort**: 30 minutes

---

### 10.3: Message Constants (P2)
**Status**: Not started

**Constants to add**:
- `SOURCE_UNKNOWN = 0x01`
- `SIGNATURE_INVALID = 0x02`
- `URI_SCHEMA = "lxm"`
- `QR_ERROR_CORRECTION = "ERROR_CORRECT_L"`
- `QR_MAX_STORAGE = 2953`
- `PAPER_MDU` (computed)
- `ENCRYPTED_PACKET_MDU` (computed)
- `LINK_PACKET_MDU` (computed)
- `PLAIN_PACKET_MDU` (computed)
- `ENCRYPTED_PACKET_MAX_CONTENT` (computed)
- `LINK_PACKET_MAX_CONTENT` (computed)
- `PLAIN_PACKET_MAX_CONTENT` (computed)

**Tests**: Verify constants match Python values

**Estimated effort**: 1 hour

---

### 10.4: Audio Mode & Renderer Constants (P3)
**Status**: Not started

**Constants to add**:
- All `AM_CODEC2_*` constants (9 constants)
- All `AM_OPUS_*` constants (10 constants)
- `AM_CUSTOM = 0xFF`
- `RENDERER_PLAIN = 0x00`
- `RENDERER_MICRON = 0x01`
- `RENDERER_MARKDOWN = 0x02`
- `RENDERER_BBCODE = 0x03`

**Note**: These are optional and may not be needed if audio/renderer features aren't used.

**Tests**: Verify constants match Python values

**Estimated effort**: 30 minutes

---

## Additional Stages from Full Comparison

### Stage 11: LXMPeer Missing Features

#### 11.1: Peer Fields (P0)
**Status**: ✅ Completed

**Fields to add to Peer struct**:
- `sync_strategy: u8` - Sync strategy (STRATEGY_LAZY or STRATEGY_PERSISTENT)
- `peering_cost: Option<u32>` - Peering cost requirement
- `metadata: Option<HashMap<String, Vec<u8>>>` - Peer metadata
- `peering_timebase: u64` - Peering timebase
- `link_establishment_rate: f64` - Link establishment rate
- `propagation_transfer_limit: Option<f64>` - Transfer limit in KB
- `propagation_sync_limit: Option<u64>` - Sync limit in KB
- `propagation_stamp_cost: Option<u32>` - Required stamp cost
- `propagation_stamp_cost_flexibility: Option<u32>` - Stamp cost flexibility
- `currently_transferring_messages: Option<Vec<[u8; 32]>>` - Currently transferring
- `current_sync_transfer_started: Option<u64>` - Transfer start time

**Tests**: 5-6 tests for new fields

**Estimated effort**: 2-3 hours

---

#### 11.2: Peer Methods (P0)
**Status**: ✅ Completed (Partial - Core methods implemented)

**Methods implemented**:
- ✅ `from_bytes()` - Deserialize peer from bytes
- ✅ `to_bytes()` - Serialize peer to bytes
- ✅ `peering_key_ready() -> bool` - Check if peering key is ready
- ✅ `peering_key_value() -> Option<u32>` - Get peering key value
- ✅ `acceptance_rate() -> f64` - Calculate acceptance rate

**Methods not yet implemented** (deferred to later stages):
- `sync()` - Full sync logic (currently partially in router)
- `link_established()` - Link established callback
- `link_closed()` - Link closed callback

**Tests**: 8 tests for serialization and core methods (all passing)

**Actual effort**: ~3 hours

---

### Stage 12: LXMessage Missing Features

#### 12.1: Message Convenience Methods (P1)
**Status**: Not started

**Methods to implement**:
- `title_as_string() -> Option<String>` - Get title as string
- `content_as_string() -> Option<String>` - Get content as string
- `set_title_from_string()` - Set title from string
- `set_title_from_bytes()` - Set title from bytes
- `set_content_from_string()` - Set content from string
- `set_content_from_bytes()` - Set content from bytes

**Tests**: 4-5 tests

**Estimated effort**: 1 hour

---

#### 12.2: Message Additional Fields (P2)
**Status**: Not started

**Fields to add** (if needed):
- `progress: f64` - Delivery progress
- `propagation_stamp: Option<Vec<u8>>` - Propagation stamp
- `propagation_stamp_value: Option<u32>` - Propagation stamp value
- `propagation_stamp_valid: bool` - Propagation stamp validity
- `propagation_target_cost: Option<u32>` - Target propagation cost
- `defer_propagation_stamp: bool` - Defer propagation stamp flag

**Note**: Some fields may not be needed if functionality is handled differently.

**Tests**: 3-4 tests

**Estimated effort**: 1-2 hours

---

### Stage 13: LXStamper Missing Features

#### 13.1: Stamp Validation Functions (P1)
**Status**: Not started

**Functions to implement**:
- `validate_pn_stamps_job_simple()` - Simple validation (single-threaded)
- `validate_pn_stamps_job_multip()` - Multiprocessing validation
- `validate_pn_stamps()` - Wrapper function (chooses simple or multip)

**Note**: Multiprocessing may not be needed in Rust (can use rayon or async).

**Tests**: 5-6 tests

**Estimated effort**: 2-3 hours

---

#### 13.2: Stamp Generation Optimizations (P2)
**Status**: Not started

**Functions to implement**:
- `job_linux()` - Linux-specific optimization (may not be needed)
- `job_android()` - Android-specific optimization (may not be needed)
- `cancel_work()` - Cancel stamp generation

**Note**: Platform-specific optimizations may not be needed in Rust.

**Tests**: 3-4 tests

**Estimated effort**: 2-3 hours

---

## Implementation Order

### Phase 1: Foundation (P0) - ✅ COMPLETED (~18 hours)
1. ✅ Stage 8.1: Router Identity & Naming
2. ✅ Stage 8.2: Propagation Node Management Fields
3. ✅ Stage 9.1: Propagation Node Announcements
4. ✅ Stage 9.2: Propagation Node Request Methods
5. ✅ Stage 10.1: Router Constants
6. ✅ Stage 10.2: Peer Constants
7. ✅ Stage 11.1: Peer Fields
8. ✅ Stage 11.2: Peer Methods (partial - core methods: from_bytes, to_bytes, peering_key_ready, peering_key_value, acceptance_rate)

**Completion**: All 8 stages implemented with comprehensive tests. All tests passing.

### Phase 2: Core Features (P1) - ~20-25 hours
9. Stage 8.3: Processing State & Sync Strategy
10. Stage 9.3: Access Control Methods
11. Stage 9.4: Message Store Helpers
12. Stage 9.5: Outbound Message Management
13. Stage 9.7: Control Request Handlers
14. Stage 11.2: Peer Methods (sync, link callbacks)
15. Stage 12.1: Message Convenience Methods
16. Stage 13.1: Stamp Validation Functions

### Phase 3: Advanced Features (P2) - ~15-25 hours
17. Stage 8.4: Ratchet & Destination Support
18. Stage 8.5: Information Storage Limit
19. Stage 9.6: Delivery Callbacks & Resource Handling
20. Stage 9.8: Utility Methods
21. Stage 9.9: Signal Handlers & Daemon Loop
22. Stage 10.3: Message Constants
23. Stage 12.2: Message Additional Fields
24. Stage 13.2: Stamp Generation Optimizations

### Phase 4: Optional Features (P3) - ~1-2 hours
25. Stage 10.4: Audio Mode & Renderer Constants

## Progress Tracking

Each stage should:
1. ✅ Implement fields/methods
2. ✅ Write comprehensive tests (TDD approach)
3. ✅ Verify parity with Python implementation
4. ✅ Update DETAILED_COMPARISON.md
5. ✅ Update GAP_ANALYSIS_STATUS.md
6. ✅ Commit with descriptive message

## Notes

- All implementations should follow TDD approach (tests first, then implementation)
- Each stage should be independently testable
- Maintain backward compatibility where possible
- Document any design decisions that differ from Python implementation
- Track test coverage for each stage
