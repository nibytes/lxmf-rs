# Full Component Comparison: Python LXMF vs Rust lxmf-rs

This document provides a comprehensive comparison of all components between Python LXMF and Rust lxmf-rs implementations.

## Component Mapping

### Python → Rust Mapping

| Python Module | Rust Module | Status | Notes |
|---------------|-------------|--------|-------|
| `LXMRouter.py` | `rns/router.rs` | ✅ Compared | See DETAILED_COMPARISON.md |
| `LXMPeer.py` | `rns/peer.rs` | ⏳ TODO | Need comparison |
| `LXMessage.py` | `message.rs` | ⏳ TODO | Need comparison |
| `LXStamper.py` | `message.rs` (stamp functions) | ⏳ TODO | Need comparison |
| `LXMF.py` | `constants.rs` | ⏳ TODO | Need comparison |
| `Handlers.py` | `rns/transport.rs`, `rns/pn.rs` | ⏳ TODO | Need comparison |
| `Utilities/lxmd.py` | N/A (daemon) | ⏳ TODO | May not be needed in library |

## Component-by-Component Comparison

### 1. LXMRouter vs LxmfRouter
**Status**: ✅ Fully compared in DETAILED_COMPARISON.md

**Summary**:
- Fields: ~70% implemented
- Methods: ~25% fully implemented
- Constants: ~57% implemented

---

### 2. LXMPeer vs Peer
**Status**: ⏳ Partially compared - Need detailed analysis

**Python File**: `LXMF/LXMPeer.py` (~642 lines)
**Rust File**: `lxmf-rs/src/rns/peer.rs` (~577 lines)

#### Fields Comparison

**Python LXMPeer Fields**:
- `alive` (bool) → Rust: `is_alive()` derived from state ✅
- `last_heard` (float) → Rust: `last_heard_ms` (u64) ✅
- `sync_strategy` (int) → ❌ **MISSING** - Not in Rust Peer struct
- `peering_key` (list or None) → Rust: `peering_key: Option<Vec<u8>>` ⚠️ (Python stores [key, value], Rust only key)
- `peering_cost` (int or None) → ❌ **MISSING** - Not in Rust Peer struct
- `metadata` (dict or None) → ❌ **MISSING** - Not in Rust Peer struct
- `next_sync_attempt` (float) → Rust: `next_sync_ms` (u64) ✅
- `last_sync_attempt` (float) → Rust: `last_attempt_ms` (u64) ✅
- `sync_backoff` (float) → Rust: `backoff_ms` (u64) ✅
- `peering_timebase` (float) → ❌ **MISSING** - Not in Rust Peer struct
- `link_establishment_rate` (float) → ❌ **MISSING** - Not in Rust Peer struct
- `sync_transfer_rate` (float) → Rust: `sync_transfer_rate` (f64) ✅
- `propagation_transfer_limit` (float or None) → ❌ **MISSING** - Not in Rust Peer struct
- `propagation_sync_limit` (int or None) → ❌ **MISSING** - Not in Rust Peer struct
- `propagation_stamp_cost` (int or None) → ❌ **MISSING** - Not in Rust Peer struct
- `propagation_stamp_cost_flexibility` (int or None) → ❌ **MISSING** - Not in Rust Peer struct
- `currently_transferring_messages` (list or None) → ❌ **MISSING** - Not in Rust Peer struct
- `current_sync_transfer_started` (float or None) → ❌ **MISSING** - Not in Rust Peer struct
- `handled_messages_queue` (deque) → Rust: `handled_messages_queue` (VecDeque) ✅
- `unhandled_messages_queue` (deque) → Rust: `unhandled_messages_queue` (VecDeque) ✅
- `offered` (int) → Rust: `messages_offered` (u64) ✅
- `outgoing` (int) → Rust: `messages_outgoing` (u64) ✅
- `incoming` (int) → Rust: `messages_incoming` (u64) ✅
- `rx_bytes` (int) → Rust: `rx_bytes` (u64) ✅
- `tx_bytes` (int) → Rust: `tx_bytes` (u64) ✅
- `_hm_count`, `_um_count` (internal) → ❌ **MISSING** - Not needed in Rust (computed on demand)
- `_hm_counts_synced`, `_um_counts_synced` (internal) → ❌ **MISSING** - Not needed in Rust
- `_peering_key_lock` (threading.Lock) → ❌ **MISSING** - Not needed in Rust (single-threaded or Arc<Mutex>)
- `link` (RNS.Link or None) → ❌ **MISSING** - Not stored directly (handled via events)
- `state` (int) → Rust: `state: PeerState` (enum) ✅
- `last_offer` (list) → Rust: `PeerSyncSession.last_offer` ✅
- `router` (LXMRouter) → ❌ **MISSING** - Not stored (passed as parameter)
- `destination_hash` (bytes) → Rust: `id` ([u8; 16]) ✅
- `identity` (RNS.Identity or None) → ❌ **MISSING** - Not stored directly
- `destination` (RNS.Destination or None) → ❌ **MISSING** - Not stored directly

**Rust Peer Additional Fields**:
- `last_sync_ms` (u64) → Python: `last_sync_attempt` ✅
- `failures` (u32) → ❌ **MISSING** in Python (tracked via sync_backoff)

#### Methods Comparison

**Python LXMPeer Methods**:
- `from_bytes()` → ❌ **MISSING** - Not implemented in Rust
- `to_bytes()` → ❌ **MISSING** - Not implemented in Rust
- `peering_key_ready()` → ❌ **MISSING** - Not implemented in Rust
- `peering_key_value()` → ❌ **MISSING** - Not implemented in Rust
- `generate_peering_key()` → ⚠️ **PARTIAL** - Exists in router.rs as `peering_key_for_peer()`
- `sync()` → ⚠️ **PARTIAL** - Logic split across router methods
- `request_failed()` → ⚠️ **PARTIAL** - Handled in `on_timeout()`
- `offer_response()` → ⚠️ **PARTIAL** - Handled in `on_offer_response()`
- `resource_concluded()` → ⚠️ **PARTIAL** - Handled in `on_payload_complete()`
- `link_established()` → ❌ **MISSING** - Handled via events
- `link_closed()` → ❌ **MISSING** - Handled via events
- `queued_items()` → Rust: `queued_items()` ✅
- `queue_unhandled_message()` → Rust: `queue_unhandled_message()` ✅
- `queue_handled_message()` → Rust: `queue_handled_message()` ✅
- `process_queues()` → Rust: `process_queues()` ✅
- `handled_messages` (property) → Rust: `handled_messages()` ✅
- `unhandled_messages` (property) → Rust: `unhandled_messages()` ✅
- `handled_message_count` (property) → ⚠️ **PARTIAL** - Can be computed
- `unhandled_message_count` (property) → Rust: `unhandled_messages_count()` ✅
- `acceptance_rate` (property) → ❌ **MISSING** - Not implemented
- `add_handled_message()` → ⚠️ **PARTIAL** - Handled via `process_queues()`
- `remove_handled_message()` → ⚠️ **PARTIAL** - Handled via `process_queues()`
- `add_unhandled_message()` → ⚠️ **PARTIAL** - Handled via `process_queues()`
- `remove_unhandled_message()` → ⚠️ **PARTIAL** - Handled via `process_queues()`
- `_update_counts()` → ❌ **MISSING** - Not needed in Rust

#### Constants Comparison

**Python LXMPeer Constants**:
- `OFFER_REQUEST_PATH = "/offer"` → ❌ **MISSING**
- `MESSAGE_GET_PATH = "/get"` → ❌ **MISSING**
- `IDLE = 0x00` → Rust: `PeerState::Idle` ✅
- `LINK_ESTABLISHING = 0x01` → ❌ **MISSING** (not in PeerState enum)
- `LINK_READY = 0x02` → ❌ **MISSING** (not in PeerState enum)
- `REQUEST_SENT = 0x03` → Rust: `PeerState::OfferSent` ⚠️ (different name)
- `RESPONSE_RECEIVED = 0x04` → Rust: `PeerState::AwaitingPayload` ⚠️ (different name)
- `RESOURCE_TRANSFERRING = 0x05` → ❌ **MISSING** (not in PeerState enum)
- `ERROR_*` constants → ❌ **MISSING** (only PEER_ERROR_INVALID_KEY exists)
- `STRATEGY_LAZY = 0x01` → ❌ **MISSING**
- `STRATEGY_PERSISTENT = 0x02` → ❌ **MISSING**
- `DEFAULT_SYNC_STRATEGY` → ❌ **MISSING**
- `MAX_UNREACHABLE = 14*24*60*60` → Rust: Used in router.rs ✅
- `SYNC_BACKOFF_STEP = 12*60` → Rust: `PEER_BACKOFF_STEP_MS` ✅
- `PATH_REQUEST_GRACE = 7.5` → ❌ **MISSING**

#### Summary
- **Fields**: ~40% implemented (core fields present, many peer-specific fields missing)
- **Methods**: ~30% implemented (queue methods present, sync logic partially implemented)
- **Constants**: ~20% implemented (state constants partially mapped, error constants missing)

---

### 3. LXMessage vs LXMessage
**Status**: ⏳ Partially compared - Need detailed analysis

**Python File**: `LXMF/LXMessage.py` (~825 lines)
**Rust File**: `lxmf-rs/src/message.rs` (~767 lines)

#### Fields Comparison

**Python LXMessage Fields**:
- `destination_hash` (bytes) → Rust: `destination_hash` ✅
- `source_hash` (bytes) → Rust: `source_hash` ✅
- `timestamp` (float) → Rust: `timestamp` (f64) ✅
- `title` (bytes) → Rust: `title` (Vec<u8>) ✅
- `content` (bytes) → Rust: `content` (Vec<u8>) ✅
- `fields` (dict) → Rust: `fields` (Value) ✅
- `stamp` (bytes or None) → Rust: `stamp: Option<Vec<u8>>` ✅
- `stamp_cost` (int or None) → Rust: `stamp_cost: Option<u32>` ✅
- `stamp_value` (int or None) → Rust: `stamp_value: Option<u32>` ✅
- `stamp_valid` (bool) → Rust: `stamp_valid` (bool) ✅
- `stamp_checked` (bool) → Rust: `stamp_checked` (bool) ✅
- `outbound_ticket` (bytes or None) → Rust: `outbound_ticket: Option<Vec<u8>>` ✅
- `include_ticket` (bool) → Rust: `include_ticket` (bool) ✅
- `defer_stamp` (bool) → Rust: `defer_stamp` (bool) ✅
- `state` (int) → Rust: `state` (u8) ✅
- `method` (int or None) → Rust: `method: Option<u8>` ✅
- `representation` (int) → Rust: `representation` (u8) ✅
- `transport_encrypted` (bool or None) → Rust: `transport_encrypted: Option<bool>` ✅
- `transport_encryption` (str or None) → Rust: `transport_encryption: Option<String>` ✅
- `signature` (bytes or None) → Rust: `signature: Option<[u8; 64]>` ✅
- `message_id` (bytes or None) → Rust: `message_id: Option<[u8; 32]>` ✅
- `__destination` (RNS.Destination) → ❌ **MISSING** - Not stored (only hash)
- `__source` (RNS.Destination) → ❌ **MISSING** - Not stored (only hash)
- `payload` (bytes or None) → ❌ **MISSING** - Computed on demand
- `hash` (bytes or None) → ❌ **MISSING** - Computed from message_id
- `transient_id` (bytes or None) → ❌ **MISSING** - Computed on demand
- `packed` (bytes or None) → ❌ **MISSING** - Computed on demand
- `progress` (float) → ❌ **MISSING** - Not in Rust
- `rssi`, `snr`, `q` (phy stats) → ❌ **MISSING** - Not in Rust
- `propagation_stamp` (bytes or None) → ❌ **MISSING** - Not in Rust
- `propagation_stamp_value` (int or None) → ❌ **MISSING** - Not in Rust
- `propagation_stamp_valid` (bool) → ❌ **MISSING** - Not in Rust
- `propagation_target_cost` (int or None) → ❌ **MISSING** - Not in Rust
- `defer_propagation_stamp` (bool) → ❌ **MISSING** - Not in Rust
- `propagation_packed` (bytes or None) → ❌ **MISSING** - Computed via method
- `paper_packed` (bytes or None) → ❌ **MISSING** - Computed via method
- `incoming` (bool) → ❌ **MISSING** - Not in Rust
- `signature_validated` (bool) → ❌ **MISSING** - Not in Rust
- `unverified_reason` (int or None) → ❌ **MISSING** - Not in Rust
- `ratchet_id` (bytes or None) → ❌ **MISSING** - Not in Rust
- `desired_method` (int or None) → ❌ **MISSING** - Not in Rust
- `delivery_attempts` (int) → ❌ **MISSING** - Not in Rust
- `packet_representation` (int or None) → ❌ **MISSING** - Not in Rust
- `resource_representation` (int or None) → ❌ **MISSING** - Not in Rust
- `__delivery_destination` (RNS.Destination) → ❌ **MISSING** - Not in Rust
- `__delivery_callback` (callable) → ❌ **MISSING** - Not in Rust
- `__pn_encrypted_data` (bytes or None) → ❌ **MISSING** - Not in Rust
- `failed_callback` (callable) → ❌ **MISSING** - Not in Rust
- `deferred_stamp_generating` (bool) → ❌ **MISSING** - Not in Rust

#### Methods Comparison

**Python LXMessage Methods**:
- `set_title_from_string()` → Rust: `with_strings()` includes title ✅
- `set_title_from_bytes()` → ⚠️ **PARTIAL** - Can set title directly
- `title_as_string()` → ❌ **MISSING** - Need to decode manually
- `set_content_from_string()` → Rust: `with_strings()` includes content ✅
- `set_content_from_bytes()` → ⚠️ **PARTIAL** - Can set content directly
- `content_as_string()` → ❌ **MISSING** - Need to decode manually
- `set_fields()` → ⚠️ **PARTIAL** - Can set fields directly
- `get_fields()` → ⚠️ **PARTIAL** - Fields are public
- `set_destination()` → ❌ **MISSING** - Only hash stored
- `get_destination()` → ❌ **MISSING** - Only hash stored
- `pack()` → Rust: `encode_signed()`, `encode_with_signature()` ✅
- `unpack()` → Rust: `decode()` ✅
- `sign()` → Rust: `encode_signed()` ✅
- `verify()` → Rust: `verify()` ✅
- `validate_stamp()` → Rust: `validate_stamp()` ✅
- `propagation_packed()` → Rust: `propagation_packed_from_encrypted()` ✅
- `paper_packed()` → Rust: `paper_packed_from_encrypted()` ✅
- `as_uri()` → Rust: `as_uri_from_paper()` ✅
- `as_qr()` → Rust: `as_qr_from_paper()` ✅
- `packed_container()` → Rust: `packed_container()` ✅
- `unpack_from_container()` → Rust: `unpack_from_container()` ✅
- `unpack_from_file()` → Rust: `unpack_from_file()` ✅
- `transient_id()` → Rust: `transient_id_from_encrypted()` ✅
- `generate_propagation_stamp()` → Rust: `generate_propagation_stamp()` ✅
- Many convenience getters/setters → ⚠️ **PARTIAL** - Some missing

#### Constants Comparison

**Python LXMessage Constants**:
- `GENERATING = 0x00` → Rust: `STATE_GENERATING` ✅
- `OUTBOUND = 0x01` → Rust: `STATE_OUTBOUND` ✅
- `SENDING = 0x02` → Rust: `STATE_SENDING` ✅
- `SENT = 0x04` → Rust: `STATE_SENT` ✅
- `DELIVERED = 0x08` → Rust: `STATE_DELIVERED` ✅
- `REJECTED = 0xFD` → Rust: `STATE_REJECTED` ✅
- `CANCELLED = 0xFE` → Rust: `STATE_CANCELLED` ✅
- `FAILED = 0xFF` → Rust: `STATE_FAILED` ✅
- `UNKNOWN = 0x00` → Rust: `REPR_UNKNOWN` ✅
- `PACKET = 0x01` → Rust: `REPR_PACKET` ✅
- `RESOURCE = 0x02` → Rust: `REPR_RESOURCE` ✅
- `OPPORTUNISTIC = 0x01` → Rust: `METHOD_OPPORTUNISTIC` ✅
- `DIRECT = 0x02` → Rust: `METHOD_DIRECT` ✅
- `PROPAGATED = 0x03` → Rust: `METHOD_PROPAGATED` ✅
- `PAPER = 0x05` → Rust: `METHOD_PAPER` ✅
- `SOURCE_UNKNOWN = 0x01` → ❌ **MISSING**
- `SIGNATURE_INVALID = 0x02` → ❌ **MISSING**
- `DESTINATION_LENGTH`, `SIGNATURE_LENGTH`, etc. → Rust: Same constants ✅
- `TICKET_EXPIRY`, `TICKET_GRACE`, etc. → Rust: Same constants ✅
- `LXMF_OVERHEAD` → Rust: Same constant ✅
- `ENCRYPTED_PACKET_MDU`, `LINK_PACKET_MDU`, etc. → ❌ **MISSING** (may not be needed)
- `URI_SCHEMA = "lxm"` → ❌ **MISSING**
- `QR_ERROR_CORRECTION = "ERROR_CORRECT_L"` → ❌ **MISSING**
- `QR_MAX_STORAGE = 2953` → ❌ **MISSING**
- `PAPER_MDU` → ❌ **MISSING**

#### Summary
- **Fields**: ~50% implemented (core fields present, many Python-specific fields missing)
- **Methods**: ~60% implemented (core pack/unpack present, convenience methods partially missing)
- **Constants**: ~70% implemented (core constants present, some utility constants missing)

---

### 4. LXStamper vs Stamp Functions
**Status**: ⏳ Partially compared - Need detailed analysis

**Python File**: `LXMF/LXStamper.py` (~391 lines)
**Rust File**: `lxmf-rs/src/message.rs` (stamp functions)

#### Functions Comparison

**Python LXStamper Functions**:
- `stamp_workblock()` → ⚠️ **PARTIAL** - Logic in `generate_stamp()` but not separate function
- `stamp_value()` → ⚠️ **PARTIAL** - Logic exists but not as separate function
- `stamp_valid()` → ⚠️ **PARTIAL** - Logic in validation functions
- `validate_peering_key()` → Rust: `validate_peering_key()` ✅
- `validate_pn_stamp()` → Rust: `validate_pn_stamp()` ✅
- `validate_pn_stamps_job_simple()` → ❌ **MISSING** - Not implemented
- `validate_pn_stamps_job_multip()` → ❌ **MISSING** - Not implemented (multiprocessing)
- `validate_pn_stamps()` → ❌ **MISSING** - Not implemented (wrapper)
- `generate_stamp()` → Rust: `generate_stamp()` ✅ (with max_rounds limit)
- `cancel_work()` → ❌ **MISSING** - Not implemented
- `job_simple()` → ⚠️ **PARTIAL** - Logic in `generate_stamp()` but simplified
- `job_linux()` → ❌ **MISSING** - Platform-specific optimization not implemented
- `job_android()` → ❌ **MISSING** - Platform-specific optimization not implemented

#### Constants Comparison

**Python LXStamper Constants**:
- `WORKBLOCK_EXPAND_ROUNDS = 3000` → Rust: `WORKBLOCK_EXPAND_ROUNDS` ✅
- `WORKBLOCK_EXPAND_ROUNDS_PN = 1000` → Rust: `WORKBLOCK_EXPAND_ROUNDS_PN` ✅
- `WORKBLOCK_EXPAND_ROUNDS_PEERING = 25` → Rust: `WORKBLOCK_EXPAND_ROUNDS_PEERING` ✅
- `STAMP_SIZE = 32` → Rust: `STAMP_SIZE` ✅
- `PN_VALIDATION_POOL_MIN_SIZE = 256` → ❌ **MISSING**

#### Summary
- **Functions**: ~40% implemented (core functions present, multiprocessing and platform-specific optimizations missing)
- **Constants**: ~80% implemented (core constants present, pool size constant missing)

---

### 5. LXMF.py vs constants.rs
**Status**: ✅ Mostly compared

**Python File**: `LXMF/LXMF.py` (~198 lines)
**Rust File**: `lxmf-rs/src/constants.rs` (~71 lines)

#### Constants Comparison

**FIELD_* Constants**:
- `FIELD_EMBEDDED_LXMS = 0x01` → Rust: `FIELD_EMBEDDED_LXMS` ✅
- `FIELD_TELEMETRY = 0x02` → Rust: `FIELD_TELEMETRY` ✅
- `FIELD_TELEMETRY_STREAM = 0x03` → Rust: `FIELD_TELEMETRY_STREAM` ✅
- `FIELD_ICON_APPEARANCE = 0x04` → Rust: `FIELD_ICON_APPEARANCE` ✅
- `FIELD_FILE_ATTACHMENTS = 0x05` → Rust: `FIELD_FILE_ATTACHMENTS` ✅
- `FIELD_IMAGE = 0x06` → Rust: `FIELD_IMAGE` ✅
- `FIELD_AUDIO = 0x07` → Rust: `FIELD_AUDIO` ✅
- `FIELD_THREAD = 0x08` → Rust: `FIELD_THREAD` ✅
- `FIELD_COMMANDS = 0x09` → Rust: `FIELD_COMMANDS` ✅
- `FIELD_RESULTS = 0x0A` → Rust: `FIELD_RESULTS` ✅
- `FIELD_GROUP = 0x0B` → Rust: `FIELD_GROUP` ✅
- `FIELD_TICKET = 0x0C` → Rust: `FIELD_TICKET` ✅
- `FIELD_EVENT = 0x0D` → Rust: `FIELD_EVENT` ✅
- `FIELD_RNR_REFS = 0x0E` → Rust: `FIELD_RNR_REFS` ✅
- `FIELD_RENDERER = 0x0F` → Rust: `FIELD_RENDERER` ✅
- `FIELD_CUSTOM_TYPE = 0xFB` → Rust: `FIELD_CUSTOM_TYPE` ✅
- `FIELD_CUSTOM_DATA = 0xFC` → Rust: `FIELD_CUSTOM_DATA` ✅
- `FIELD_CUSTOM_META = 0xFD` → Rust: `FIELD_CUSTOM_META` ✅
- `FIELD_NON_SPECIFIC = 0xFE` → Rust: `FIELD_NON_SPECIFIC` ✅
- `FIELD_DEBUG = 0xFF` → Rust: `FIELD_DEBUG` ✅

**AM_* (Audio Mode) Constants**:
- `AM_CODEC2_450PWB = 0x01` → ❌ **MISSING**
- `AM_CODEC2_450 = 0x02` → ❌ **MISSING**
- `AM_CODEC2_700C = 0x03` → ❌ **MISSING**
- `AM_CODEC2_1200 = 0x04` → ❌ **MISSING**
- `AM_CODEC2_1300 = 0x05` → ❌ **MISSING**
- `AM_CODEC2_1400 = 0x06` → ❌ **MISSING**
- `AM_CODEC2_1600 = 0x07` → ❌ **MISSING**
- `AM_CODEC2_2400 = 0x08` → ❌ **MISSING**
- `AM_CODEC2_3200 = 0x09` → ❌ **MISSING**
- `AM_OPUS_OGG = 0x10` → ❌ **MISSING**
- `AM_OPUS_LBW = 0x11` → ❌ **MISSING**
- `AM_OPUS_MBW = 0x12` → ❌ **MISSING**
- `AM_OPUS_PTT = 0x13` → ❌ **MISSING**
- `AM_OPUS_RT_HDX = 0x14` → ❌ **MISSING**
- `AM_OPUS_RT_FDX = 0x15` → ❌ **MISSING**
- `AM_OPUS_STANDARD = 0x16` → ❌ **MISSING**
- `AM_OPUS_HQ = 0x17` → ❌ **MISSING**
- `AM_OPUS_BROADCAST = 0x18` → ❌ **MISSING**
- `AM_OPUS_LOSSLESS = 0x19` → ❌ **MISSING**
- `AM_CUSTOM = 0xFF` → ❌ **MISSING**

**RENDERER_* Constants**:
- `RENDERER_PLAIN = 0x00` → ❌ **MISSING**
- `RENDERER_MICRON = 0x01` → ❌ **MISSING**
- `RENDERER_MARKDOWN = 0x02` → ❌ **MISSING**
- `RENDERER_BBCODE = 0x03` → ❌ **MISSING**

**PN_META_* Constants**:
- `PN_META_VERSION = 0x00` → Rust: `PN_META_VERSION` ✅
- `PN_META_NAME = 0x01` → Rust: `PN_META_NAME` ✅
- `PN_META_SYNC_STRATUM = 0x02` → Rust: `PN_META_SYNC_STRATUM` ✅
- `PN_META_SYNC_THROTTLE = 0x03` → Rust: `PN_META_SYNC_THROTTLE` ✅
- `PN_META_AUTH_BAND = 0x04` → Rust: `PN_META_AUTH_BAND` ✅
- `PN_META_UTIL_PRESSURE = 0x05` → Rust: `PN_META_UTIL_PRESSURE` ✅
- `PN_META_CUSTOM = 0xFF` → Rust: `PN_META_CUSTOM` ✅

**Helper Functions**:
- `display_name_from_app_data()` → ⚠️ **PARTIAL** - Exists in pn.rs
- `stamp_cost_from_app_data()` → ⚠️ **PARTIAL** - Exists in pn.rs
- `pn_name_from_app_data()` → ⚠️ **PARTIAL** - Exists in pn.rs
- `pn_stamp_cost_from_app_data()` → ⚠️ **PARTIAL** - Exists in pn.rs
- `pn_announce_data_is_valid()` → ⚠️ **PARTIAL** - Exists in pn.rs

#### Summary
- **FIELD_* Constants**: 100% implemented ✅
- **AM_* Constants**: 0% implemented ❌ (20 constants missing)
- **RENDERER_* Constants**: 0% implemented ❌ (4 constants missing)
- **PN_META_* Constants**: 100% implemented ✅
- **Helper Functions**: ~80% implemented (all exist in pn.rs)

---

### 6. Handlers vs Transport/PN
**Status**: ⏳ Partially compared - Architecture differs

**Python File**: `LXMF/Handlers.py` (~92 lines)
**Rust Files**: `lxmf-rs/src/rns/transport.rs`, `lxmf-rs/src/rns/pn.rs`, `lxmf-rs/src/pn.rs`

#### Python Handlers

**LXMFDeliveryAnnounceHandler**:
- `__init__(lxmrouter)` - Initializes handler
- `received_announce(destination_hash, announced_identity, app_data)` - Handles delivery announces
  - Extracts stamp cost from app_data
  - Updates stamp cost in router
  - Triggers outbound processing for matching messages

**LXMFPropagationAnnounceHandler**:
- `__init__(lxmrouter)` - Initializes handler
- `received_announce(destination_hash, announced_identity, app_data, announce_packet_hash, is_path_response)` - Handles PN announces
  - Validates announce data
  - Extracts PN configuration (timebase, limits, stamp costs, metadata)
  - Calls `router.peer()` or `router.unpeer()` based on conditions
  - Handles autopeer logic

#### Rust Implementation

**Architecture Difference**: Rust uses event-driven architecture via `reticulum-rs`, not direct handler classes.

**Equivalent Functionality**:
- Announce handling → ⚠️ **PARTIAL** - Exists in `pn.rs` and `rns/pn.rs` but architecture differs
- `pn_announce_data_is_valid()` → Rust: `pn_announce_data_is_valid()` ✅
- `stamp_cost_from_app_data()` → Rust: `stamp_cost_from_app_data()` ✅
- `pn_name_from_app_data()` → Rust: `pn_name_from_app_data()` ✅
- `pn_stamp_cost_from_app_data()` → Rust: `pn_stamp_cost_from_app_data()` ✅
- PN directory management → Rust: `PnDirectory`, `PnDirService` ✅
- Announce processing → ⚠️ **PARTIAL** - Logic exists but integrated differently

#### Summary
- **Handler Classes**: 0% (architecture differs, not needed)
- **Handler Logic**: ~70% implemented (core functions exist, integration differs)

---

### 7. Storage & Persistence
**Status**: ⏳ TODO - Need detailed comparison

**Python**: Storage handled in LXMRouter methods
**Rust**: `lxmf-rs/src/rns/storage.rs`

**TODO**: Compare storage structures, serialization, persistence logic

---

### 8. Transport & RNS Integration
**Status**: ⏳ TODO - Need detailed comparison

**Python**: RNS integration in LXMRouter, Handlers
**Rust**: `lxmf-rs/src/rns/transport.rs`

**TODO**: Compare delivery modes, link handling, resource transfer

---

## Comparison Checklist

### LXMPeer Comparison
- [ ] Compare class structure and fields
- [ ] Compare all methods
- [ ] Compare constants (STRATEGY_*, STATE_*, etc.)
- [ ] Compare sync logic
- [ ] Compare backoff mechanisms
- [ ] Compare queue processing
- [ ] Compare serialization/deserialization

### LXMessage Comparison
- [ ] Compare class structure and fields
- [ ] Compare packing/unpacking logic
- [ ] Compare signature generation/verification
- [ ] Compare stamp handling
- [ ] Compare ticket handling
- [ ] Compare field handling
- [ ] Compare serialization formats
- [ ] Compare convenience methods

### LXStamper Comparison
- [ ] Compare all stamp generation functions
- [ ] Compare validation functions
- [ ] Compare workblock functions
- [ ] Compare constants (WORKBLOCK_*)
- [ ] Compare job functions (simple/linux/android)
- [ ] Compare cancellation mechanism

### Constants Comparison
- [ ] Compare FIELD_* constants
- [ ] Compare AM_* (audio mode) constants
- [ ] Compare RENDERER_* constants
- [ ] Compare PN_META_* constants
- [ ] Compare other utility constants

### Handlers Comparison
- [ ] Compare LXMFDeliveryAnnounceHandler
- [ ] Compare LXMFPropagationAnnounceHandler
- [ ] Compare RNS integration points
- [ ] Compare announce handling logic

### Storage Comparison
- [ ] Compare PropagationStore structure
- [ ] Compare serialization formats
- [ ] Compare persistence mechanisms
- [ ] Compare cleanup logic

### Transport Comparison
- [ ] Compare delivery modes
- [ ] Compare link lifecycle handling
- [ ] Compare resource transfer
- [ ] Compare request/response handling

## Next Steps

1. Complete LXMPeer comparison
2. Complete LXMessage comparison
3. Complete LXStamper comparison
4. Complete Constants comparison
5. Complete Handlers comparison
6. Complete Storage comparison
7. Complete Transport comparison
8. Create comprehensive implementation plan based on all findings

## Overall Summary

### Component Coverage

| Component | Fields | Methods | Constants | Overall |
|-----------|--------|---------|-----------|---------|
| **LXMRouter** | ~70% | ~25% | ~57% | ~51% |
| **LXMPeer** | ~40% | ~30% | ~20% | ~30% |
| **LXMessage** | ~50% | ~60% | ~70% | ~60% |
| **LXStamper** | N/A | ~40% | ~80% | ~60% |
| **LXMF.py** | N/A | ~80% | ~60% | ~70% |
| **Handlers** | N/A | ~70% | N/A | ~70% |

### Critical Missing Features

#### High Priority (P0)
1. **LXMRouter**: Missing fields (name, autopeer, outbound_propagation_node, etc.)
2. **LXMRouter**: Missing methods (announce, propagation node management, etc.)
3. **LXMPeer**: Missing fields (sync_strategy, peering_cost, propagation limits, etc.)
4. **LXMPeer**: Missing methods (sync(), generate_peering_key(), link callbacks, etc.)
5. **LXMessage**: Missing convenience methods (title_as_string, content_as_string, etc.)

#### Medium Priority (P1)
1. **LXMPeer**: Missing constants (STRATEGY_*, ERROR_*, PATH_REQUEST_GRACE)
2. **LXMessage**: Missing constants (AM_*, RENDERER_*, URI_SCHEMA)
3. **LXStamper**: Missing multiprocessing validation functions
4. **LXStamper**: Missing platform-specific optimizations (job_linux, job_android)

#### Low Priority (P2)
1. **LXMessage**: Missing fields (progress, rssi, snr, q, propagation_stamp, etc.)
2. **LXMPeer**: Missing serialization (from_bytes, to_bytes)
3. **LXStamper**: Missing cancellation mechanism (cancel_work)

## Progress Tracking

- [x] LXMRouter comparison (DETAILED_COMPARISON.md)
- [x] LXMPeer comparison
- [x] LXMessage comparison
- [x] LXStamper comparison
- [x] Constants comparison
- [x] Handlers comparison
- [ ] Storage comparison (TODO - needs detailed analysis)
- [ ] Transport comparison (TODO - needs detailed analysis)
