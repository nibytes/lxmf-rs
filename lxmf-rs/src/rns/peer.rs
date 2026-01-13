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
    // Message queues (matching Python LXMPeer)
    pub(crate) handled_messages_queue: VecDeque<[u8; 32]>,   // Matching Python handled_messages_queue
    pub(crate) unhandled_messages_queue: VecDeque<[u8; 32]>, // Matching Python unhandled_messages_queue
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
            handled_messages_queue: VecDeque::new(),
            unhandled_messages_queue: VecDeque::new(),
        }
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
