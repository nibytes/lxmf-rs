use crate::constants::DESTINATION_LENGTH;
use rmpv::Value;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;

use super::{value_to_bytes, value_to_f64, value_to_fixed_bytes, RnsError};

#[derive(Debug, Clone, PartialEq)]
pub enum PeerSyncRequest {
    Offer {
        peering_key: Vec<u8>,
        available: Vec<[u8; 32]>,
    },
    MessageGetList,
    MessageGet {
        wants: Vec<[u8; 32]>,
        haves: Vec<[u8; 32]>,
        transfer_limit_kb: Option<f64>,
    },
    MessageAck {
        delivered: Vec<[u8; 32]>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeerSyncResponse {
    Error(u8),
    Bool(bool),
    IdList(Vec<[u8; 32]>),
    Payload(Vec<Vec<u8>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerSyncState {
    Idle,
    Offering,
    AwaitingPayload,
    Completed,
    Failed,
}

pub const PEER_ERROR_INVALID_KEY: u8 = 0xf3;
pub const DEFAULT_PEERING_COST: u32 = 18;
pub const DEFAULT_PROPAGATION_STAMP_COST: u32 = 16;
pub const DEFAULT_PROPAGATION_STAMP_FLEX: u32 = 3;
pub const PROPAGATION_INVALID_STAMP_THROTTLE_MS: u64 = 180_000;

// Peer request paths (matching Python LXMPeer)
pub const OFFER_REQUEST_PATH: &str = "/offer"; // Matching Python OFFER_REQUEST_PATH = "/offer"
pub const MESSAGE_GET_PATH: &str = "/get"; // Matching Python MESSAGE_GET_PATH = "/get"

// Peer link states (matching Python LXMPeer)
pub const LINK_ESTABLISHING: u8 = 0x01; // Matching Python LINK_ESTABLISHING = 0x01
pub const LINK_READY: u8 = 0x02; // Matching Python LINK_READY = 0x02
pub const RESOURCE_TRANSFERRING: u8 = 0x05; // Matching Python RESOURCE_TRANSFERRING = 0x05

// Peer error codes (matching Python LXMPeer)
pub const ERROR_NO_IDENTITY: u8 = 0xf0; // Matching Python ERROR_NO_IDENTITY = 0xf0
pub const ERROR_NO_ACCESS: u8 = 0xf1; // Matching Python ERROR_NO_ACCESS = 0xf1
pub const ERROR_INVALID_DATA: u8 = 0xf4; // Matching Python ERROR_INVALID_DATA = 0xf4
pub const ERROR_INVALID_STAMP: u8 = 0xf5; // Matching Python ERROR_INVALID_STAMP = 0xf5
pub const ERROR_THROTTLED: u8 = 0xf6; // Matching Python ERROR_THROTTLED = 0xf6
pub const ERROR_NOT_FOUND: u8 = 0xfd; // Matching Python ERROR_NOT_FOUND = 0xfd
pub const ERROR_TIMEOUT: u8 = 0xfe; // Matching Python ERROR_TIMEOUT = 0xfe

// Peer sync strategies (matching Python LXMPeer)
pub const STRATEGY_LAZY: u8 = 0x01; // Matching Python STRATEGY_LAZY = 0x01
pub const STRATEGY_PERSISTENT: u8 = 0x02; // Matching Python STRATEGY_PERSISTENT = 0x02
pub const DEFAULT_SYNC_STRATEGY: u8 = STRATEGY_PERSISTENT; // Matching Python DEFAULT_SYNC_STRATEGY = STRATEGY_PERSISTENT

// Peer path request grace period (matching Python LXMPeer)
pub const PATH_REQUEST_GRACE: f64 = 7.5; // Matching Python PATH_REQUEST_GRACE = 7.5 (seconds)

const PEER_BACKOFF_STEP_MS: u64 = 12 * 60 * 1000;
const PEER_BACKOFF_MAX_MS: u64 = 14 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct PeerSyncSession {
    pub state: PeerSyncState,
    pub available: Vec<[u8; 32]>,
    pub last_offer: Vec<[u8; 32]>,
}

impl PeerSyncSession {
    pub fn new() -> Self {
        Self {
            state: PeerSyncState::Idle,
            available: Vec::new(),
            last_offer: Vec::new(),
        }
    }

    pub fn offer(
        &mut self,
        peering_key: Vec<u8>,
        available: Vec<[u8; 32]>,
    ) -> PeerSyncRequest {
        self.state = PeerSyncState::Offering;
        self.available = available.clone();
        self.last_offer = available.clone();
        PeerSyncRequest::Offer {
            peering_key,
            available,
        }
    }

    pub fn on_offer_response(&mut self, response: &PeerSyncResponse) -> PeerSyncState {
        self.state = match response {
            PeerSyncResponse::Bool(true) | PeerSyncResponse::IdList(_) => PeerSyncState::AwaitingPayload,
            PeerSyncResponse::Bool(false) | PeerSyncResponse::Payload(_) => PeerSyncState::Completed,
            PeerSyncResponse::Error(_) => PeerSyncState::Failed,
        };
        self.state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Idle,
    OfferSent,
    AwaitingPayload,
    Backoff,
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: [u8; DESTINATION_LENGTH],
    pub state: PeerState,
    pub last_sync_ms: u64,
    pub last_attempt_ms: u64,
    pub next_sync_ms: u64,
    pub backoff_ms: u64,
    pub failures: u32,
    pub last_heard_ms: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub messages_offered: u64,
    pub messages_outgoing: u64,
    pub messages_incoming: u64,
    pub messages_unhandled: u64,
    pub peering_key: Option<Vec<u8>>,
    pub peering_key_value: Option<u32>, // Matching Python peering_key[1] - stamp value (cached for efficiency)
    pub sync_transfer_rate: f64, // Matching Python LXMPeer.sync_transfer_rate (transfer rate in bits per second)
    // Message queues (matching Python LXMPeer)
    pub(crate) handled_messages_queue: VecDeque<[u8; 32]>,   // Matching Python handled_messages_queue
    pub(crate) unhandled_messages_queue: VecDeque<[u8; 32]>, // Matching Python unhandled_messages_queue
    // Additional fields (matching Python LXMPeer)
    pub sync_strategy: u8, // Matching Python LXMPeer.sync_strategy (STRATEGY_LAZY or STRATEGY_PERSISTENT)
    pub peering_cost: Option<u32>, // Matching Python LXMPeer.peering_cost
    pub metadata: Option<HashMap<String, Vec<u8>>>, // Matching Python LXMPeer.metadata
    pub peering_timebase: u64, // Matching Python LXMPeer.peering_timebase
    pub link_establishment_rate: f64, // Matching Python LXMPeer.link_establishment_rate
    pub propagation_transfer_limit: Option<f64>, // Matching Python LXMPeer.propagation_transfer_limit (in KB)
    pub propagation_sync_limit: Option<u64>, // Matching Python LXMPeer.propagation_sync_limit (in KB)
    pub propagation_stamp_cost: Option<u32>, // Matching Python LXMPeer.propagation_stamp_cost
    pub propagation_stamp_cost_flexibility: Option<u32>, // Matching Python LXMPeer.propagation_stamp_cost_flexibility
    pub currently_transferring_messages: Option<Vec<[u8; 32]>>, // Matching Python LXMPeer.currently_transferring_messages
    pub current_sync_transfer_started: Option<u64>, // Matching Python LXMPeer.current_sync_transfer_started
}

impl Peer {
    pub fn new(id: [u8; DESTINATION_LENGTH], now_ms: u64) -> Self {
        Self {
            id,
            state: PeerState::Idle,
            last_sync_ms: 0,
            last_attempt_ms: 0,
            next_sync_ms: now_ms,
            backoff_ms: 0,
            failures: 0,
            last_heard_ms: 0,
            rx_bytes: 0,
            tx_bytes: 0,
            messages_offered: 0,
            messages_outgoing: 0,
            messages_incoming: 0,
            messages_unhandled: 0,
            peering_key: None,
            peering_key_value: None, // Matching Python: peering_key[1] if peering_key is [bytes, value]
            sync_transfer_rate: 0.0, // Matching Python: default sync_transfer_rate = 0
            handled_messages_queue: VecDeque::new(),
            unhandled_messages_queue: VecDeque::new(),
            // Additional fields with defaults (matching Python LXMPeer.__init__)
            sync_strategy: DEFAULT_SYNC_STRATEGY, // Matching Python: sync_strategy=DEFAULT_SYNC_STRATEGY
            peering_cost: None, // Matching Python: peering_cost = None
            metadata: None, // Matching Python: metadata = None
            peering_timebase: 0, // Matching Python: peering_timebase = 0
            link_establishment_rate: 0.0, // Matching Python: link_establishment_rate = 0
            propagation_transfer_limit: None, // Matching Python: propagation_transfer_limit = None
            propagation_sync_limit: None, // Matching Python: propagation_sync_limit = None
            propagation_stamp_cost: None, // Matching Python: propagation_stamp_cost = None
            propagation_stamp_cost_flexibility: None, // Matching Python: propagation_stamp_cost_flexibility = None
            currently_transferring_messages: None, // Matching Python: currently_transferring_messages = None
            current_sync_transfer_started: None, // Matching Python: current_sync_transfer_started = None
        }
    }

    /// Get sync_transfer_rate (matching Python LXMPeer.sync_transfer_rate)
    pub fn sync_transfer_rate(&self) -> f64 {
        self.sync_transfer_rate
    }

    /// Set sync_transfer_rate (matching Python LXMPeer.sync_transfer_rate)
    pub fn set_sync_transfer_rate(&mut self, rate: f64) {
        self.sync_transfer_rate = rate;
    }

    /// Check if peer is alive (matching Python LXMPeer.alive)
    /// In Python: alive is a boolean field
    /// In Rust: we derive it from state (not in Backoff = alive)
    pub fn is_alive(&self) -> bool {
        !matches!(self.state, PeerState::Backoff)
    }

    /// Set alive status (by updating state)
    pub fn set_alive(&mut self, alive: bool) {
        if !alive && matches!(self.state, PeerState::Idle) {
            // Mark as backoff if setting to not alive
            self.state = PeerState::Backoff;
        } else if alive && matches!(self.state, PeerState::Backoff) {
            // Mark as idle if setting to alive
            self.state = PeerState::Idle;
        }
    }

    /// Get last_heard_ms (matching Python LXMPeer.last_heard)
    pub fn last_heard_ms(&self) -> u64 {
        self.last_heard_ms
    }

    /// Set last_heard_ms (matching Python LXMPeer.last_heard)
    pub fn set_last_heard_ms(&mut self, ms: u64) {
        self.last_heard_ms = ms;
    }

    /// Get next_sync_attempt (matching Python LXMPeer.next_sync_attempt)
    /// In Python: next_sync_attempt is a separate field
    /// In Rust: we use next_sync_ms for the same purpose
    pub fn next_sync_attempt_ms(&self) -> u64 {
        self.next_sync_ms
    }

    /// Set next_sync_attempt (matching Python LXMPeer.next_sync_attempt)
    pub fn set_next_sync_attempt_ms(&mut self, ms: u64) {
        self.next_sync_ms = ms;
    }

    /// Get sync_strategy (matching Python LXMPeer.sync_strategy)
    pub fn sync_strategy(&self) -> u8 {
        self.sync_strategy
    }

    /// Set sync_strategy (matching Python LXMPeer.sync_strategy)
    pub fn set_sync_strategy(&mut self, strategy: u8) {
        self.sync_strategy = strategy;
    }

    /// Get peering_cost (matching Python LXMPeer.peering_cost)
    pub fn peering_cost(&self) -> Option<u32> {
        self.peering_cost
    }

    /// Set peering_cost (matching Python LXMPeer.peering_cost)
    pub fn set_peering_cost(&mut self, cost: Option<u32>) {
        self.peering_cost = cost;
    }

    /// Get metadata (matching Python LXMPeer.metadata)
    pub fn metadata(&self) -> Option<&HashMap<String, Vec<u8>>> {
        self.metadata.as_ref()
    }

    /// Set metadata (matching Python LXMPeer.metadata)
    pub fn set_metadata(&mut self, metadata: Option<HashMap<String, Vec<u8>>>) {
        self.metadata = metadata;
    }

    /// Get peering_timebase (matching Python LXMPeer.peering_timebase)
    pub fn peering_timebase(&self) -> u64 {
        self.peering_timebase
    }

    /// Set peering_timebase (matching Python LXMPeer.peering_timebase)
    pub fn set_peering_timebase(&mut self, timebase: u64) {
        self.peering_timebase = timebase;
    }

    /// Get link_establishment_rate (matching Python LXMPeer.link_establishment_rate)
    pub fn link_establishment_rate(&self) -> f64 {
        self.link_establishment_rate
    }

    /// Set link_establishment_rate (matching Python LXMPeer.link_establishment_rate)
    pub fn set_link_establishment_rate(&mut self, rate: f64) {
        self.link_establishment_rate = rate;
    }

    /// Get propagation_transfer_limit (matching Python LXMPeer.propagation_transfer_limit)
    pub fn propagation_transfer_limit(&self) -> Option<f64> {
        self.propagation_transfer_limit
    }

    /// Set propagation_transfer_limit (matching Python LXMPeer.propagation_transfer_limit)
    pub fn set_propagation_transfer_limit(&mut self, limit: Option<f64>) {
        self.propagation_transfer_limit = limit;
    }

    /// Get propagation_sync_limit (matching Python LXMPeer.propagation_sync_limit)
    pub fn propagation_sync_limit(&self) -> Option<u64> {
        self.propagation_sync_limit
    }

    /// Set propagation_sync_limit (matching Python LXMPeer.propagation_sync_limit)
    pub fn set_propagation_sync_limit(&mut self, limit: Option<u64>) {
        self.propagation_sync_limit = limit;
    }

    /// Get propagation_stamp_cost (matching Python LXMPeer.propagation_stamp_cost)
    pub fn propagation_stamp_cost(&self) -> Option<u32> {
        self.propagation_stamp_cost
    }

    /// Set propagation_stamp_cost (matching Python LXMPeer.propagation_stamp_cost)
    pub fn set_propagation_stamp_cost(&mut self, cost: Option<u32>) {
        self.propagation_stamp_cost = cost;
    }

    /// Get propagation_stamp_cost_flexibility (matching Python LXMPeer.propagation_stamp_cost_flexibility)
    pub fn propagation_stamp_cost_flexibility(&self) -> Option<u32> {
        self.propagation_stamp_cost_flexibility
    }

    /// Set propagation_stamp_cost_flexibility (matching Python LXMPeer.propagation_stamp_cost_flexibility)
    pub fn set_propagation_stamp_cost_flexibility(&mut self, flexibility: Option<u32>) {
        self.propagation_stamp_cost_flexibility = flexibility;
    }

    /// Get currently_transferring_messages (matching Python LXMPeer.currently_transferring_messages)
    pub fn currently_transferring_messages(&self) -> Option<&Vec<[u8; 32]>> {
        self.currently_transferring_messages.as_ref()
    }

    /// Set currently_transferring_messages (matching Python LXMPeer.currently_transferring_messages)
    pub fn set_currently_transferring_messages(&mut self, messages: Option<Vec<[u8; 32]>>) {
        self.currently_transferring_messages = messages;
    }

    /// Get current_sync_transfer_started (matching Python LXMPeer.current_sync_transfer_started)
    pub fn current_sync_transfer_started(&self) -> Option<u64> {
        self.current_sync_transfer_started
    }

    /// Set current_sync_transfer_started (matching Python LXMPeer.current_sync_transfer_started)
    pub fn set_current_sync_transfer_started(&mut self, started: Option<u64>) {
        self.current_sync_transfer_started = started;
    }

    /// Get unhandled_messages count (matching Python len(peer.unhandled_messages))
    pub fn unhandled_messages_count(&self) -> usize {
        self.unhandled_messages_queue.len()
    }

    /// Queue unhandled message (matching Python LXMPeer.queue_unhandled_message)
    pub fn queue_unhandled_message(&mut self, transient_id: [u8; 32]) {
        self.unhandled_messages_queue.push_back(transient_id);
    }

    /// Queue handled message (matching Python LXMPeer.queue_handled_message)
    pub fn queue_handled_message(&mut self, transient_id: [u8; 32]) {
        self.handled_messages_queue.push_back(transient_id);
    }

    /// Check if peer has queued items (matching Python LXMPeer.queued_items)
    pub fn queued_items(&self) -> bool {
        !self.handled_messages_queue.is_empty() || !self.unhandled_messages_queue.is_empty()
    }

    /// Get unhandled messages queue (for testing)
    pub fn unhandled_messages_queue(&self) -> &VecDeque<[u8; 32]> {
        &self.unhandled_messages_queue
    }

    /// Get handled messages queue (for testing)
    pub fn handled_messages_queue(&self) -> &VecDeque<[u8; 32]> {
        &self.handled_messages_queue
    }

    /// Process queues (matching Python LXMPeer.process_queues)
    /// Moves transient_ids from queues to PropagationStore handled/unhandled_peers
    /// 
    /// Python logic:
    /// - For handled_messages_queue: if not in handled_messages, add to handled; if in unhandled_messages, remove from unhandled
    /// - For unhandled_messages_queue: if not in handled_messages AND not in unhandled_messages, add to unhandled
    pub fn process_queues(&mut self, store: &mut super::storage::PropagationStore) {
        // Get current state (matching Python: handled_messages = self.handled_messages)
        let handled_messages: std::collections::HashSet<[u8; 32]> = 
            store.handled_messages_for_peer(self.id).into_iter().collect();
        let unhandled_messages: std::collections::HashSet<[u8; 32]> = 
            store.unhandled_messages_for_peer(self.id).into_iter().collect();
        
        // Process handled_messages_queue (matching Python lines 558-561)
        while let Some(transient_id) = self.handled_messages_queue.pop_front() {
            // Python: if not transient_id in handled_messages: self.add_handled_message(transient_id)
            if !handled_messages.contains(&transient_id) {
                store.add_handled_peer(&transient_id, self.id);
            }
            // Python: if transient_id in unhandled_messages: self.remove_unhandled_message(transient_id)
            if unhandled_messages.contains(&transient_id) {
                store.remove_unhandled_peer(&transient_id, &self.id);
            }
        }
        
        // Process unhandled_messages_queue (matching Python lines 563-566)
        while let Some(transient_id) = self.unhandled_messages_queue.pop_front() {
            // Python: if not transient_id in handled_messages and not transient_id in unhandled_messages: self.add_unhandled_message(transient_id)
            if !handled_messages.contains(&transient_id) && !unhandled_messages.contains(&transient_id) {
                store.add_unhandled_peer(&transient_id, self.id);
            }
        }
    }

    /// Get handled messages for this peer (matching Python peer.handled_messages)
    pub fn handled_messages(&self, store: &super::storage::PropagationStore) -> Vec<[u8; 32]> {
        store.handled_messages_for_peer(self.id)
    }

    /// Get unhandled messages for this peer (matching Python peer.unhandled_messages)
    pub fn unhandled_messages(&self, store: &super::storage::PropagationStore) -> Vec<[u8; 32]> {
        store.unhandled_messages_for_peer(self.id)
    }

    /// Check if peering key is ready (matching Python LXMPeer.peering_key_ready)
    /// Returns true if peering_key exists and its value >= peering_cost
    pub fn peering_key_ready(&self) -> bool {
        // In Python: if not self.peering_cost: return False
        let Some(cost) = self.peering_cost else {
            return false;
        };
        
        // In Python: if type(self.peering_key) == list and len(self.peering_key) == 2:
        //   value = self.peering_key[1]
        //   if value >= self.peering_cost: return True
        if let Some(value) = self.peering_key_value {
            if value >= cost {
                return true;
            }
            // In Python: else: log warning and set peering_key = None
            // In Rust, we don't clear it here - caller should handle regeneration
        }
        
        false
    }

    /// Get peering key value (matching Python LXMPeer.peering_key_value)
    /// In Python: if type(self.peering_key) == list and len(self.peering_key) == 2: return self.peering_key[1]
    pub fn peering_key_value(&self) -> Option<u32> {
        self.peering_key_value
    }

    /// Calculate acceptance rate (matching Python LXMPeer.acceptance_rate)
    /// Returns outgoing / offered if offered > 0, else 0.0
    pub fn acceptance_rate(&self) -> f64 {
        if self.messages_offered > 0 {
            self.messages_outgoing as f64 / self.messages_offered as f64
        } else {
            0.0
        }
    }

    /// Serialize peer to bytes (matching Python LXMPeer.to_bytes)
    pub fn to_bytes(&self) -> Result<Vec<u8>, RnsError> {
        use rmpv::Value;
        
        let mut dict: Vec<(Value, Value)> = Vec::new();
        
        // Required fields (matching Python to_bytes)
        dict.push((Value::String("destination_hash".into()), Value::Binary(self.id.to_vec())));
        dict.push((Value::String("peering_timebase".into()), Value::from(self.peering_timebase as i64)));
        dict.push((Value::String("alive".into()), Value::Boolean(self.is_alive())));
        dict.push((Value::String("last_heard".into()), Value::from(self.last_heard_ms as i64)));
        dict.push((Value::String("sync_strategy".into()), Value::from(self.sync_strategy as i64)));
        
        // Optional fields
        if let Some(ref key) = self.peering_key {
            dict.push((Value::String("peering_key".into()), Value::Binary(key.clone())));
        }
        if let Some(value) = self.peering_key_value {
            // Store value separately (in Python it's peering_key[1])
            dict.push((Value::String("peering_key_value".into()), Value::from(value as i64)));
        }
        if let Some(cost) = self.peering_cost {
            dict.push((Value::String("peering_cost".into()), Value::from(cost as i64)));
        }
        if let Some(ref metadata) = self.metadata {
            let mut meta_dict: Vec<(Value, Value)> = Vec::new();
            for (k, v) in metadata {
                meta_dict.push((Value::String(k.clone().into()), Value::Binary(v.clone())));
            }
            dict.push((Value::String("metadata".into()), Value::Map(meta_dict)));
        }
        
        dict.push((Value::String("link_establishment_rate".into()), Value::F64(self.link_establishment_rate)));
        dict.push((Value::String("sync_transfer_rate".into()), Value::F64(self.sync_transfer_rate)));
        
        if let Some(limit) = self.propagation_transfer_limit {
            dict.push((Value::String("propagation_transfer_limit".into()), Value::F64(limit)));
        }
        if let Some(limit) = self.propagation_sync_limit {
            dict.push((Value::String("propagation_sync_limit".into()), Value::from(limit as i64)));
        }
        if let Some(cost) = self.propagation_stamp_cost {
            dict.push((Value::String("propagation_stamp_cost".into()), Value::from(cost as i64)));
        }
        if let Some(flex) = self.propagation_stamp_cost_flexibility {
            dict.push((Value::String("propagation_stamp_cost_flexibility".into()), Value::from(flex as i64)));
        }
        
        // Message counts (matching Python: offered, outgoing, incoming, rx_bytes, tx_bytes)
        dict.push((Value::String("offered".into()), Value::from(self.messages_offered as i64)));
        dict.push((Value::String("outgoing".into()), Value::from(self.messages_outgoing as i64)));
        dict.push((Value::String("incoming".into()), Value::from(self.messages_incoming as i64)));
        dict.push((Value::String("rx_bytes".into()), Value::from(self.rx_bytes as i64)));
        dict.push((Value::String("tx_bytes".into()), Value::from(self.tx_bytes as i64)));
        dict.push((Value::String("last_sync_attempt".into()), Value::from(self.last_attempt_ms as i64)));
        
        // Handled and unhandled message IDs (matching Python: handled_ids, unhandled_ids)
        // Note: In Python, these come from propagation_store, but we serialize empty lists
        // as the store is separate. The caller should populate these from the store.
        dict.push((Value::String("handled_ids".into()), Value::Array(Vec::new())));
        dict.push((Value::String("unhandled_ids".into()), Value::Array(Vec::new())));
        
        let value = Value::Map(dict);
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &value)?;
        Ok(buf)
    }

    /// Deserialize peer from bytes (matching Python LXMPeer.from_bytes)
    pub fn from_bytes(bytes: &[u8], now_ms: u64) -> Result<Self, RnsError> {
        use rmpv::Value;
        use std::io::Cursor;
        
        let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
        let dict = match value {
            Value::Map(map) => map,
            _ => return Err(RnsError::Msgpack("peer data must be a map".to_string())),
        };
        
        // Helper to get value from map
        let get_value = |key: &str| -> Option<&Value> {
            let key_val = Value::String(key.into());
            dict.iter().find(|(k, _)| k == &key_val).map(|(_, v)| v)
        };
        
        // Required fields
        let id = super::value_to_fixed_bytes::<DESTINATION_LENGTH>(
            get_value("destination_hash")
                .ok_or_else(|| RnsError::Msgpack("missing destination_hash".to_string()))?
        ).ok_or_else(|| RnsError::Msgpack("invalid destination_hash".to_string()))?;
        
        let peering_timebase = get_value("peering_timebase")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as u64;
        
        let alive = get_value("alive")
            .and_then(|v| match v {
                Value::Boolean(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(false);
        
        let last_heard = get_value("last_heard")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as u64;
        
        let mut peer = Peer::new(id, now_ms);
        peer.set_peering_timebase(peering_timebase);
        peer.set_last_heard_ms(last_heard);
        peer.set_alive(alive);
        
        // Optional fields (matching Python from_bytes logic)
        if let Some(v) = get_value("sync_strategy") {
            if let Some(strategy) = v.as_i64() {
                peer.set_sync_strategy(strategy as u8);
            } else {
                peer.set_sync_strategy(DEFAULT_SYNC_STRATEGY);
            }
        }
        
        if let Some(v) = get_value("link_establishment_rate") {
            peer.set_link_establishment_rate(super::value_to_f64(v).unwrap_or(0.0));
        }
        
        if let Some(v) = get_value("sync_transfer_rate") {
            peer.set_sync_transfer_rate(super::value_to_f64(v).unwrap_or(0.0));
        }
        
        if let Some(v) = get_value("propagation_transfer_limit") {
            peer.set_propagation_transfer_limit(super::value_to_f64(v));
        }
        
        if let Some(v) = get_value("propagation_sync_limit") {
            if let Some(limit) = v.as_i64() {
                peer.set_propagation_sync_limit(Some(limit as u64));
            } else {
                peer.set_propagation_sync_limit(peer.propagation_transfer_limit().map(|f| f as u64));
            }
        } else {
            peer.set_propagation_sync_limit(peer.propagation_transfer_limit().map(|f| f as u64));
        }
        
        if let Some(v) = get_value("propagation_stamp_cost") {
            if let Some(cost) = v.as_i64() {
                peer.set_propagation_stamp_cost(Some(cost as u32));
            }
        }
        
        if let Some(v) = get_value("propagation_stamp_cost_flexibility") {
            if let Some(flex) = v.as_i64() {
                peer.set_propagation_stamp_cost_flexibility(Some(flex as u32));
            }
        }
        
        if let Some(v) = get_value("peering_cost") {
            if let Some(cost) = v.as_i64() {
                peer.set_peering_cost(Some(cost as u32));
            }
        }
        
        if let Some(v) = get_value("peering_key") {
            peer.peering_key = super::value_to_bytes(v);
        }
        
        if let Some(v) = get_value("peering_key_value") {
            if let Some(value) = v.as_i64() {
                peer.peering_key_value = Some(value as u32);
            }
        }
        
        if let Some(v) = get_value("metadata") {
            if let Value::Map(meta_map) = v {
                let mut metadata = HashMap::new();
                for (k, v) in meta_map {
                    if let (Some(key_str), Some(value_bytes)) = (
                        k.as_str().map(|s| s.to_string()),
                        super::value_to_bytes(v)
                    ) {
                        metadata.insert(key_str, value_bytes);
                    }
                }
                peer.set_metadata(Some(metadata));
            }
        }
        
        // Message counts
        if let Some(v) = get_value("offered") {
            if let Some(count) = v.as_i64() {
                peer.messages_offered = count as u64;
            }
        }
        if let Some(v) = get_value("outgoing") {
            if let Some(count) = v.as_i64() {
                peer.messages_outgoing = count as u64;
            }
        }
        if let Some(v) = get_value("incoming") {
            if let Some(count) = v.as_i64() {
                peer.messages_incoming = count as u64;
            }
        }
        if let Some(v) = get_value("rx_bytes") {
            if let Some(bytes) = v.as_i64() {
                peer.rx_bytes = bytes as u64;
            }
        }
        if let Some(v) = get_value("tx_bytes") {
            if let Some(bytes) = v.as_i64() {
                peer.tx_bytes = bytes as u64;
            }
        }
        if let Some(v) = get_value("last_sync_attempt") {
            if let Some(ms) = v.as_i64() {
                peer.last_attempt_ms = ms as u64;
            }
        }
        
        // Note: handled_ids and unhandled_ids are not restored here
        // as they should be managed by PropagationStore
        
        Ok(peer)
    }

    pub fn should_sync(&self, now_ms: u64) -> bool {
        matches!(self.state, PeerState::Idle | PeerState::Backoff) && now_ms >= self.next_sync_ms
    }

    pub fn on_offer_sent(&mut self, now_ms: u64) -> PeerState {
        self.state = PeerState::OfferSent;
        self.last_attempt_ms = now_ms;
        self.state
    }

    pub fn on_offer_response(&mut self, response: &PeerSyncResponse, now_ms: u64) -> PeerState {
        self.state = match response {
            PeerSyncResponse::Bool(true) | PeerSyncResponse::IdList(_) => PeerState::AwaitingPayload,
            PeerSyncResponse::Bool(false) | PeerSyncResponse::Payload(_) => {
                self.on_sync_complete(now_ms);
                PeerState::Idle
            }
            PeerSyncResponse::Error(_) => {
                self.on_failure(now_ms);
                PeerState::Backoff
            }
        };
        self.state
    }

    pub fn mark_heard(&mut self, now_ms: u64) {
        self.last_heard_ms = now_ms;
    }

    pub fn on_payload_complete(&mut self, now_ms: u64) -> PeerState {
        self.on_sync_complete(now_ms);
        self.state
    }

    pub fn on_timeout(&mut self, now_ms: u64) -> PeerState {
        self.on_failure(now_ms)
    }

    fn on_sync_complete(&mut self, now_ms: u64) {
        self.last_sync_ms = now_ms;
        self.last_attempt_ms = now_ms;
        self.failures = 0;
        self.backoff_ms = 0;
        self.next_sync_ms = now_ms;
        self.state = PeerState::Idle;
    }

    pub(crate) fn on_failure(&mut self, now_ms: u64) -> PeerState {
        self.failures = self.failures.saturating_add(1);
        let next = if self.backoff_ms == 0 {
            PEER_BACKOFF_STEP_MS
        } else {
            self.backoff_ms.saturating_add(PEER_BACKOFF_STEP_MS)
        };
        self.backoff_ms = next.min(PEER_BACKOFF_MAX_MS);
        self.next_sync_ms = now_ms.saturating_add(self.backoff_ms);
        self.state = PeerState::Backoff;
        self.state
    }

    pub(crate) fn throttle(&mut self, now_ms: u64, throttle_ms: u64) {
        self.failures = self.failures.saturating_add(1);
        self.backoff_ms = throttle_ms;
        self.next_sync_ms = now_ms.saturating_add(throttle_ms);
        self.state = PeerState::Backoff;
    }
}

#[derive(Debug, Default)]
pub struct PeerTable {
    peers: HashMap<[u8; DESTINATION_LENGTH], Peer>,
}

impl PeerTable {
    pub fn new() -> Self {
        Self { peers: HashMap::new() }
    }

    pub fn upsert(&mut self, id: [u8; DESTINATION_LENGTH], now_ms: u64) -> &mut Peer {
        self.peers.entry(id).or_insert_with(|| Peer::new(id, now_ms))
    }

    pub fn get(&self, id: &[u8; DESTINATION_LENGTH]) -> Option<&Peer> {
        self.peers.get(id)
    }

    pub fn get_mut(&mut self, id: &[u8; DESTINATION_LENGTH]) -> Option<&mut Peer> {
        self.peers.get_mut(id)
    }

    pub fn next_ready(&self, now_ms: u64) -> Option<[u8; DESTINATION_LENGTH]> {
        self.peers
            .values()
            .filter(|peer| peer.should_sync(now_ms))
            .min_by_key(|peer| peer.next_sync_ms)
            .map(|peer| peer.id)
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn entries(&self) -> &HashMap<[u8; DESTINATION_LENGTH], Peer> {
        &self.peers
    }

    pub fn entries_mut(&mut self) -> &mut HashMap<[u8; DESTINATION_LENGTH], Peer> {
        &mut self.peers
    }

    pub fn set_entries(&mut self, peers: HashMap<[u8; DESTINATION_LENGTH], Peer>) {
        self.peers = peers;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PeeringValidation {
    pub local_identity_hash: [u8; DESTINATION_LENGTH],
    pub target_cost: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct PropagationValidation {
    pub stamp_cost: u32,
    pub stamp_cost_flex: u32,
}

pub fn encode_peer_sync_request(req: &PeerSyncRequest) -> Result<Vec<u8>, RnsError> {
    let value = match req {
        PeerSyncRequest::Offer { peering_key, available } => Value::Array(vec![
            Value::Binary(peering_key.clone()),
            Value::Array(
                available
                    .iter()
                    .map(|id| Value::Binary(id.to_vec()))
                    .collect(),
            ),
        ]),
        PeerSyncRequest::MessageGetList => Value::Array(vec![Value::Nil, Value::Nil]),
        PeerSyncRequest::MessageGet {
            wants,
            haves,
            transfer_limit_kb,
        } => {
            let mut arr = vec![
                Value::Array(wants.iter().map(|id| Value::Binary(id.to_vec())).collect()),
                Value::Array(haves.iter().map(|id| Value::Binary(id.to_vec())).collect()),
            ];
            if let Some(limit) = transfer_limit_kb {
                arr.push(Value::F64(*limit));
            }
            Value::Array(arr)
        }
        PeerSyncRequest::MessageAck { delivered } => Value::Array(vec![
            Value::Nil,
            Value::Array(
                delivered
                    .iter()
                    .map(|id| Value::Binary(id.to_vec()))
                    .collect(),
            ),
        ]),
    };
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub fn decode_peer_sync_request(bytes: &[u8]) -> Result<PeerSyncRequest, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("peer sync req must be array".to_string())),
    };
    if arr.is_empty() {
        return Err(RnsError::Msgpack("peer sync req missing op".to_string()));
    }
    if matches!(arr.get(0), Some(Value::Binary(_))) {
        let key = value_to_bytes(arr.get(0).ok_or_else(|| RnsError::Msgpack("missing key".to_string()))?)
            .ok_or_else(|| RnsError::Msgpack("invalid key".to_string()))?;
        let list = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing list".to_string()))?;
        let list = match list {
            Value::Array(vals) => vals,
            _ => return Err(RnsError::Msgpack("list must be array".to_string())),
        };
        let mut ids = Vec::with_capacity(list.len());
        for val in list {
            let id = value_to_fixed_bytes::<32>(val)
                .ok_or_else(|| RnsError::Msgpack("invalid id".to_string()))?;
            ids.push(id);
        }
        return Ok(PeerSyncRequest::Offer {
            peering_key: key,
            available: ids,
        });
    }

    let wants_val = arr.get(0).ok_or_else(|| RnsError::Msgpack("missing wants".to_string()))?;
    let haves_val = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing haves".to_string()))?;
    let wants = match wants_val {
        Value::Nil => None,
        Value::Array(vals) => {
            let mut ids = Vec::with_capacity(vals.len());
            for val in vals {
                let id = value_to_fixed_bytes::<32>(val)
                    .ok_or_else(|| RnsError::Msgpack("invalid id".to_string()))?;
                ids.push(id);
            }
            Some(ids)
        }
        _ => return Err(RnsError::Msgpack("invalid wants".to_string())),
    };
    let haves = match haves_val {
        Value::Nil => None,
        Value::Array(vals) => {
            let mut ids = Vec::with_capacity(vals.len());
            for val in vals {
                let id = value_to_fixed_bytes::<32>(val)
                    .ok_or_else(|| RnsError::Msgpack("invalid id".to_string()))?;
                ids.push(id);
            }
            Some(ids)
        }
        _ => return Err(RnsError::Msgpack("invalid haves".to_string())),
    };

    let transfer_limit_kb = match arr.get(2) {
        Some(val) => Some(value_to_f64(val).ok_or_else(|| RnsError::Msgpack("invalid transfer limit".to_string()))?),
        None => None,
    };

    match (wants, haves) {
        (None, None) => Ok(PeerSyncRequest::MessageGetList),
        (None, Some(delivered)) => Ok(PeerSyncRequest::MessageAck { delivered }),
        (Some(wants), Some(haves)) => Ok(PeerSyncRequest::MessageGet {
            wants,
            haves,
            transfer_limit_kb,
        }),
        (Some(wants), None) => Ok(PeerSyncRequest::MessageGet {
            wants,
            haves: Vec::new(),
            transfer_limit_kb,
        }),
    }
}

pub fn encode_peer_sync_response(resp: &PeerSyncResponse) -> Result<Vec<u8>, RnsError> {
    let value = match resp {
        PeerSyncResponse::Error(code) => Value::from(*code as i64),
        PeerSyncResponse::Bool(flag) => Value::Boolean(*flag),
        PeerSyncResponse::IdList(list) => Value::Array(
            list.iter()
                .map(|id| Value::Binary(id.to_vec()))
                .collect(),
        ),
        PeerSyncResponse::Payload(items) => Value::Array(
            items
                .iter()
                .map(|data| Value::Binary(data.clone()))
                .collect(),
        ),
    };
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub fn decode_peer_sync_response(bytes: &[u8]) -> Result<PeerSyncResponse, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    match value {
        Value::Boolean(flag) => Ok(PeerSyncResponse::Bool(flag)),
        Value::Integer(int) => {
            let code = int
                .as_i64()
                .ok_or_else(|| RnsError::Msgpack("invalid error code".to_string()))?;
            if !(0..=u8::MAX as i64).contains(&code) {
                return Err(RnsError::Msgpack("error code out of range".to_string()));
            }
            Ok(PeerSyncResponse::Error(code as u8))
        }
        Value::Array(list) => {
            let mut binaries = Vec::with_capacity(list.len());
            let mut all_fixed = true;
            for val in &list {
                let data = value_to_bytes(val)
                    .ok_or_else(|| RnsError::Msgpack("payload must be binary".to_string()))?;
                if data.len() != 32 {
                    all_fixed = false;
                }
                binaries.push(data);
            }
            if all_fixed {
                let mut ids = Vec::with_capacity(binaries.len());
                for data in binaries {
                    let mut id = [0u8; 32];
                    id.copy_from_slice(&data);
                    ids.push(id);
                }
                Ok(PeerSyncResponse::IdList(ids))
            } else {
                Ok(PeerSyncResponse::Payload(binaries))
            }
        }
        _ => Err(RnsError::Msgpack("invalid peer sync response".to_string())),
    }
}
