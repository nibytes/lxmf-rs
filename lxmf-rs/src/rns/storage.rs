use crate::constants::{DESTINATION_LENGTH, HASH_LENGTH, MESSAGE_EXPIRY, SIGNATURE_LENGTH};
use crate::message::{validate_pn_stamp, LXMessage};
use base64::Engine;
use rmpv::Value;
use std::collections::HashMap;
use std::io::Cursor;

use super::peer::{Peer, PeerState};
use super::pn::{entry_from_value, entry_to_value, PnDirEntryWire};
use super::{
    full_hash, value_to_bytes, value_to_f64, value_to_fixed_bytes, value_to_opt_bool, value_to_opt_bytes,
    value_to_opt_string, value_to_opt_u32, value_to_opt_u64, value_to_opt_u8, DeliveryMode, RnsError,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TicketStoreSnapshot {
    pub outbound: HashMap<[u8; DESTINATION_LENGTH], (f64, Vec<u8>)>,
    pub inbound: HashMap<[u8; DESTINATION_LENGTH], HashMap<Vec<u8>, f64>>,
    pub last_deliveries: HashMap<[u8; DESTINATION_LENGTH], f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutboundStampCosts {
    pub entries: HashMap<[u8; DESTINATION_LENGTH], (f64, u32)>,
}

#[derive(Debug, Clone)]
pub struct PropagationEntry {
    pub transient_id: [u8; 32],
    pub lxm_data: Vec<u8>,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeStats {
    pub client_propagation_messages_received: u64,
    pub client_propagation_messages_served: u64,
    pub unpeered_propagation_incoming: u64,
    pub unpeered_propagation_rx_bytes: u64,
}

#[derive(Debug, Default)]
pub struct PropagationStore {
    entries: HashMap<[u8; 32], PropagationEntry>,
    stamp_values: HashMap<[u8; 32], u32>,
    filenames: HashMap<[u8; 32], String>,
}

impl PropagationStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            stamp_values: HashMap::new(),
            filenames: HashMap::new(),
        }
    }

    pub fn insert(&mut self, entry: PropagationEntry) {
        let stamp_value = validate_pn_stamp(&entry.lxm_data, 0)
            .map(|(_, _, value, _)| value)
            .unwrap_or(0);
        let filename = propagation_entry_filename(&entry.transient_id, entry.timestamp, stamp_value);
        self.stamp_values.insert(entry.transient_id, stamp_value);
        self.filenames.insert(entry.transient_id, filename);
        self.entries.insert(entry.transient_id, entry);
    }

    pub fn insert_with_stamp_value(&mut self, entry: PropagationEntry, stamp_value: u32) {
        let filename = propagation_entry_filename(&entry.transient_id, entry.timestamp, stamp_value);
        self.stamp_values.insert(entry.transient_id, stamp_value);
        self.filenames.insert(entry.transient_id, filename);
        self.entries.insert(entry.transient_id, entry);
    }

    pub fn insert_with_stamp_value_and_filename(
        &mut self,
        entry: PropagationEntry,
        stamp_value: u32,
        filename: String,
    ) {
        self.stamp_values.insert(entry.transient_id, stamp_value);
        self.filenames.insert(entry.transient_id, filename);
        self.entries.insert(entry.transient_id, entry);
    }

    pub fn has(&self, transient_id: &[u8; 32]) -> bool {
        self.entries.contains_key(transient_id)
    }

    pub fn get(&self, transient_id: &[u8; 32]) -> Option<&PropagationEntry> {
        self.entries.get(transient_id)
    }

    pub fn remove(&mut self, transient_id: &[u8; 32]) -> Option<PropagationEntry> {
        self.stamp_values.remove(transient_id);
        self.filenames.remove(transient_id);
        self.entries.remove(transient_id)
    }

    pub fn list_ids(&self) -> Vec<[u8; 32]> {
        self.entries.keys().copied().collect()
    }

    pub fn entries(&self) -> &HashMap<[u8; 32], PropagationEntry> {
        &self.entries
    }

    pub fn stamp_value(&self, transient_id: &[u8; 32]) -> Option<u32> {
        self.stamp_values.get(transient_id).copied()
    }

    pub fn filename(&self, transient_id: &[u8; 32]) -> Option<&str> {
        self.filenames.get(transient_id).map(|name| name.as_str())
    }

    pub fn total_size_bytes(&self) -> usize {
        self.entries.values().map(|entry| entry.lxm_data.len()).sum()
    }

    pub fn remove_older_than(&mut self, now_ms: u64, max_age_ms: u64) -> usize {
        let cutoff = now_ms.saturating_sub(max_age_ms);
        let mut remove = Vec::new();
        for (id, entry) in &self.entries {
            let ts_ms = (entry.timestamp.max(0.0) * 1000.0) as u64;
            if ts_ms <= cutoff {
                remove.push(*id);
            }
        }
        for id in &remove {
            self.stamp_values.remove(id);
            self.filenames.remove(id);
            self.entries.remove(id);
        }
        remove.len()
    }
}

#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub id: u64,
    pub message: LXMessage,
    pub mode: DeliveryMode,
    pub attempts: u32,
    pub next_attempt_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct FailedDelivery {
    pub id: u64,
    pub error: String,
    pub message: LXMessage,
    pub attempts: u32,
}

pub trait MessageStore: Send + Sync {
    fn save_outbound(&mut self, item: &QueuedMessage);
    fn remove_outbound(&mut self, id: u64);
    fn load_outbound(&mut self) -> Vec<QueuedMessage>;
    fn save_inbound(&mut self, message: &LXMessage);
    fn save_failed(&mut self, item: &FailedDelivery);
}

pub struct InMemoryStore {
    pub outbound: Vec<QueuedMessage>,
    pub inbound: Vec<LXMessage>,
    pub failed: Vec<FailedDelivery>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            outbound: Vec::new(),
            inbound: Vec::new(),
            failed: Vec::new(),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageStore for InMemoryStore {
    fn save_outbound(&mut self, item: &QueuedMessage) {
        self.outbound.push(item.clone());
    }

    fn remove_outbound(&mut self, id: u64) {
        self.outbound.retain(|m| m.id != id);
    }

    fn load_outbound(&mut self) -> Vec<QueuedMessage> {
        self.outbound.clone()
    }

    fn save_inbound(&mut self, message: &LXMessage) {
        self.inbound.push(message.clone());
    }

    fn save_failed(&mut self, item: &FailedDelivery) {
        self.failed.push(item.clone());
    }
}

pub struct FileMessageStore {
    root: std::path::PathBuf,
}

impl FileMessageStore {
    pub fn new<P: Into<std::path::PathBuf>>(root: P) -> Self {
        let store = Self { root: root.into() };
        let _ = std::fs::create_dir_all(store.outbound_dir());
        let _ = std::fs::create_dir_all(store.inbound_dir());
        let _ = std::fs::create_dir_all(store.failed_dir());
        let _ = std::fs::create_dir_all(store.propagation_dir());
        store
    }

    fn outbound_dir(&self) -> std::path::PathBuf {
        self.root.join("outbound")
    }

    fn inbound_dir(&self) -> std::path::PathBuf {
        self.root.join("inbound")
    }

    fn failed_dir(&self) -> std::path::PathBuf {
        self.root.join("failed")
    }

    fn propagation_dir(&self) -> std::path::PathBuf {
        self.root.join("propagation")
    }

    fn pn_dir_path(&self) -> std::path::PathBuf {
        self.root.join("pn_directory.mpk")
    }

    fn available_tickets_path(&self) -> std::path::PathBuf {
        self.root.join("available_tickets")
    }

    fn outbound_stamp_costs_path(&self) -> std::path::PathBuf {
        self.root.join("outbound_stamp_costs")
    }

    fn node_stats_path(&self) -> std::path::PathBuf {
        self.root.join("node_stats")
    }

    fn local_deliveries_path(&self) -> std::path::PathBuf {
        self.root.join("local_deliveries")
    }

    fn locally_processed_path(&self) -> std::path::PathBuf {
        self.root.join("locally_processed")
    }

    fn outbound_path(&self, id: u64) -> std::path::PathBuf {
        self.outbound_dir().join(format!("{id}.mpk"))
    }

    fn inbound_path(&self, message: &LXMessage) -> std::path::PathBuf {
        let name_bytes = match message.message_id {
            Some(id) => id.to_vec(),
            None => {
                let mut data = Vec::new();
                data.extend_from_slice(&message.destination_hash);
                data.extend_from_slice(&message.source_hash);
                data.extend_from_slice(&message.title);
                data.extend_from_slice(&message.content);
                full_hash(&data).to_vec()
            }
        };
        let mut name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(name_bytes);
        name.push_str(".mpk");
        self.inbound_dir().join(name)
    }

    fn failed_path(&self, id: u64) -> std::path::PathBuf {
        self.failed_dir().join(format!("{id}.mpk"))
    }

    fn propagation_entry_path_from_filename(&self, filename: &str) -> std::path::PathBuf {
        self.propagation_dir().join(filename)
    }

    fn encode_outbound(item: &QueuedMessage) -> Result<Vec<u8>, RnsError> {
        let entries = vec![
            (Value::from("id"), Value::from(item.id as i64)),
            (Value::from("mode"), Value::from(delivery_mode_to_i64(item.mode))),
            (Value::from("attempts"), Value::from(item.attempts as i64)),
            (Value::from("next_attempt_ms"), Value::from(item.next_attempt_at_ms as i64)),
            (Value::from("message"), encode_message_value(&item.message)),
        ];
        let mut buf = Vec::new();
        rmpv::encode::write_value(&mut buf, &Value::Map(entries))?;
        Ok(buf)
    }

    fn decode_outbound(bytes: &[u8]) -> Result<QueuedMessage, RnsError> {
        let mut cursor = Cursor::new(bytes);
        let value = rmpv::decode::read_value(&mut cursor)?;
        let map = match value {
            Value::Map(entries) => entries,
            _ => return Err(RnsError::Msgpack("expected map".to_string())),
        };

        let mut id: Option<u64> = None;
        let mut mode: Option<DeliveryMode> = None;
        let mut attempts: Option<u32> = None;
        let mut next_attempt_ms: Option<u64> = None;
        let mut message_value: Option<Value> = None;

        for (key, val) in map {
            let key = match key.as_str() {
                Some(key) => key,
                None => continue,
            };
            match key {
                "id" => {
                    if let Some(v) = val.as_i64() {
                        id = Some(v as u64);
                    }
                }
                "mode" => {
                    if let Some(v) = val.as_i64() {
                        mode = Some(delivery_mode_from_i64(v)?);
                    }
                }
                "attempts" => {
                    if let Some(v) = val.as_i64() {
                        attempts = Some(v as u32);
                    }
                }
                "next_attempt_ms" => {
                    if let Some(v) = val.as_i64() {
                        next_attempt_ms = Some(v as u64);
                    }
                }
                "message" => message_value = Some(val),
                _ => {}
            }
        }

        let id = id.ok_or_else(|| RnsError::Msgpack("missing id".to_string()))?;
        let mode = mode.ok_or_else(|| RnsError::Msgpack("missing mode".to_string()))?;
        let attempts = attempts.ok_or_else(|| RnsError::Msgpack("missing attempts".to_string()))?;
        let next_attempt_ms =
            next_attempt_ms.ok_or_else(|| RnsError::Msgpack("missing next_attempt_ms".to_string()))?;
        let message_value =
            message_value.ok_or_else(|| RnsError::Msgpack("missing message".to_string()))?;
        let message = decode_message_value(&message_value)?;

        Ok(QueuedMessage {
            id,
            message,
            mode,
            attempts,
            next_attempt_at_ms: next_attempt_ms,
        })
    }
}

impl MessageStore for FileMessageStore {
    fn save_outbound(&mut self, item: &QueuedMessage) {
        let path = self.outbound_path(item.id);
        if let Ok(data) = Self::encode_outbound(item) {
            let _ = std::fs::write(path, data);
        }
    }

    fn remove_outbound(&mut self, id: u64) {
        let path = self.outbound_path(id);
        let _ = std::fs::remove_file(path);
    }

    fn load_outbound(&mut self) -> Vec<QueuedMessage> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(self.outbound_dir()) {
            Ok(entries) => entries,
            Err(_) => return out,
        };

        for entry in entries.flatten() {
            if let Ok(data) = std::fs::read(entry.path()) {
                if let Ok(item) = Self::decode_outbound(&data) {
                    out.push(item);
                }
            }
        }

        out
    }

    fn save_inbound(&mut self, message: &LXMessage) {
        let path = self.inbound_path(message);
        let value = encode_message_value(message);
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &value).is_ok() {
            let _ = std::fs::write(path, buf);
        }
    }

    fn save_failed(&mut self, item: &FailedDelivery) {
        let path = self.failed_path(item.id);
        let entries = vec![
            (Value::from("id"), Value::from(item.id as i64)),
            (Value::from("attempts"), Value::from(item.attempts as i64)),
            (Value::from("error"), Value::from(item.error.clone())),
            (Value::from("message"), encode_message_value(&item.message)),
        ];
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &Value::Map(entries)).is_ok() {
            let _ = std::fs::write(path, buf);
        }
    }
}

impl FileMessageStore {
    pub fn save_propagation_entry(&self, entry: &PropagationEntry) {
        let stamp_value = validate_pn_stamp(&entry.lxm_data, 0)
            .map(|(_, _, value, _)| value)
            .unwrap_or(0);
        let filename =
            propagation_entry_filename(&entry.transient_id, entry.timestamp, stamp_value);
        let path = self.propagation_entry_path_from_filename(&filename);
        let _ = std::fs::create_dir_all(self.propagation_dir());
        let _ = std::fs::write(path, &entry.lxm_data);
    }

    pub fn remove_propagation_entry(&self, transient_id: &[u8; 32]) {
        let prefix = format!("{}_", hex::encode(transient_id));
        let entries = match std::fs::read_dir(self.propagation_dir()) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if let Some(name) = name.to_str() {
                if name.starts_with(&prefix) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    pub fn remove_propagation_entry_named(&self, filename: &str) {
        let path = self.propagation_entry_path_from_filename(filename);
        let _ = std::fs::remove_file(path);
    }

    pub fn load_propagation_store(&self) -> PropagationStore {
        let mut store = PropagationStore::new();
        let entries = match std::fs::read_dir(self.propagation_dir()) {
            Ok(entries) => entries,
            Err(_) => return store,
        };
        for entry in entries.flatten() {
            let filename = entry.file_name();
            let Some(filename) = filename.to_str() else {
                let _ = std::fs::remove_file(entry.path());
                continue;
            };

            let Some((transient_id, timestamp, stamp_value)) =
                parse_propagation_entry_filename(filename)
            else {
                let _ = std::fs::remove_file(entry.path());
                continue;
            };

            let data = match std::fs::read(entry.path()) {
                Ok(data) => data,
                Err(_) => {
                    let _ = std::fs::remove_file(entry.path());
                    continue;
                }
            };

            let computed_stamp_value = validate_pn_stamp(&data, 0)
                .map(|(_, _, value, _)| value)
                .unwrap_or(0);
            if computed_stamp_value != stamp_value {
                let _ = std::fs::remove_file(entry.path());
                continue;
            }

            store.insert_with_stamp_value_and_filename(
                PropagationEntry {
                    transient_id,
                    lxm_data: data,
                    timestamp,
                },
                stamp_value,
                filename.to_string(),
            );
        }
        store
    }

    fn peers_dir(&self) -> std::path::PathBuf {
        self.root.join("peers")
    }

    fn peer_path(&self, id: &[u8; DESTINATION_LENGTH]) -> std::path::PathBuf {
        let name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id);
        self.peers_dir().join(name)
    }

    pub fn save_peer(&self, peer: &Peer) {
        let _ = std::fs::create_dir_all(self.peers_dir());
        let value = Value::Map(vec![
            (Value::from("id"), Value::Binary(peer.id.to_vec())),
            (Value::from("state"), Value::from(peer.state as i64)),
            (Value::from("last_sync_ms"), Value::from(peer.last_sync_ms as i64)),
            (Value::from("last_attempt_ms"), Value::from(peer.last_attempt_ms as i64)),
            (Value::from("next_sync_ms"), Value::from(peer.next_sync_ms as i64)),
            (Value::from("backoff_ms"), Value::from(peer.backoff_ms as i64)),
            (Value::from("failures"), Value::from(peer.failures as i64)),
            (Value::from("last_heard_ms"), Value::from(peer.last_heard_ms as i64)),
            (Value::from("rx_bytes"), Value::from(peer.rx_bytes as i64)),
            (Value::from("tx_bytes"), Value::from(peer.tx_bytes as i64)),
            (Value::from("messages_offered"), Value::from(peer.messages_offered as i64)),
            (Value::from("messages_outgoing"), Value::from(peer.messages_outgoing as i64)),
            (Value::from("messages_incoming"), Value::from(peer.messages_incoming as i64)),
            (Value::from("messages_unhandled"), Value::from(peer.messages_unhandled as i64)),
            (
                Value::from("peering_key"),
                peer.peering_key
                    .as_ref()
                    .map(|key| Value::Binary(key.clone()))
                    .unwrap_or(Value::Nil),
            ),
        ]);
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &value).is_ok() {
            let _ = std::fs::write(self.peer_path(&peer.id), buf);
        }
    }

    pub fn remove_peer(&self, id: &[u8; DESTINATION_LENGTH]) {
        let _ = std::fs::remove_file(self.peer_path(id));
    }

    pub fn load_peers(&self) -> super::peer::PeerTable {
        let mut table = super::peer::PeerTable::new();
        let entries = match std::fs::read_dir(self.peers_dir()) {
            Ok(entries) => entries,
            Err(_) => return table,
        };

        let mut map = HashMap::new();
        for entry in entries.flatten() {
            if let Ok(data) = std::fs::read(entry.path()) {
                if let Ok(value) = rmpv::decode::read_value(&mut Cursor::new(data)) {
                    if let Ok(peer) = peer_from_value(&value) {
                        map.insert(peer.id, peer);
                    }
                }
            }
        }
        table.set_entries(map);
        table
    }

    pub fn save_pn_directory(&self, directory: &crate::pn::PnDirectory) {
        let mut entries = Vec::new();
        for entry in directory.list() {
            entries.push(entry_to_value(&PnDirEntryWire {
                destination_hash: entry.destination_hash,
                app_data: entry.app_data,
                last_seen_ms: entry.last_seen_ms,
                acked: entry.acked,
            }));
        }
        let value = Value::Array(entries);
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &value).is_ok() {
            let _ = std::fs::write(self.pn_dir_path(), buf);
        }
    }

    pub fn load_pn_directory(&self) -> crate::pn::PnDirectory {
        let mut dir = crate::pn::PnDirectory::new();
        let data = match std::fs::read(self.pn_dir_path()) {
            Ok(data) => data,
            Err(_) => return dir,
        };
        let value = match rmpv::decode::read_value(&mut Cursor::new(data)) {
            Ok(value) => value,
            Err(_) => return dir,
        };
        let list = match value {
            Value::Array(list) => list,
            _ => return dir,
        };
        for item in list {
            if let Ok(entry) = entry_from_value(&item) {
                let _ = dir.upsert_announce(
                    entry.destination_hash,
                    &entry.app_data,
                    entry.last_seen_ms,
                );
                if entry.acked {
                    let _ = dir.acknowledge(&entry.destination_hash);
                }
            }
        }
        dir
    }

    pub fn save_local_deliveries(&self, entries: &HashMap<[u8; 32], f64>) {
        let value = transient_cache_to_value(entries);
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &value).is_ok() {
            let _ = std::fs::write(self.local_deliveries_path(), buf);
        }
    }

    pub fn load_local_deliveries(&self) -> HashMap<[u8; 32], f64> {
        load_transient_cache(self.local_deliveries_path())
    }

    pub fn save_locally_processed(&self, entries: &HashMap<[u8; 32], f64>) {
        let value = transient_cache_to_value(entries);
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &value).is_ok() {
            let _ = std::fs::write(self.locally_processed_path(), buf);
        }
    }

    pub fn load_locally_processed(&self) -> HashMap<[u8; 32], f64> {
        load_transient_cache(self.locally_processed_path())
    }

    pub fn save_available_tickets(&self, snapshot: &TicketStoreSnapshot) {
        let mut root = Vec::new();
        root.push((Value::from("outbound"), tickets_outbound_to_value(&snapshot.outbound)));
        root.push((Value::from("inbound"), tickets_inbound_to_value(&snapshot.inbound)));
        root.push((
            Value::from("last_deliveries"),
            tickets_last_deliveries_to_value(&snapshot.last_deliveries),
        ));
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &Value::Map(root)).is_ok() {
            let _ = std::fs::write(self.available_tickets_path(), buf);
        }
    }

    pub fn load_available_tickets(&self) -> TicketStoreSnapshot {
        let data = match std::fs::read(self.available_tickets_path()) {
            Ok(data) => data,
            Err(_) => return TicketStoreSnapshot::default(),
        };
        let value = match rmpv::decode::read_value(&mut Cursor::new(data)) {
            Ok(value) => value,
            Err(_) => return TicketStoreSnapshot::default(),
        };
        let map = match value {
            Value::Map(map) => map,
            _ => return TicketStoreSnapshot::default(),
        };

        let mut snapshot = TicketStoreSnapshot::default();
        for (key, val) in map {
            let key = match key.as_str() {
                Some(key) => key,
                None => continue,
            };
            match key {
                "outbound" => snapshot.outbound = tickets_outbound_from_value(&val),
                "inbound" => snapshot.inbound = tickets_inbound_from_value(&val),
                "last_deliveries" => snapshot.last_deliveries = tickets_last_deliveries_from_value(&val),
                _ => {}
            }
        }
        snapshot
    }

    pub fn save_outbound_stamp_costs(&self, costs: &OutboundStampCosts) {
        let value = stamp_costs_to_value(&costs.entries);
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &value).is_ok() {
            let _ = std::fs::write(self.outbound_stamp_costs_path(), buf);
        }
    }

    pub fn load_outbound_stamp_costs(&self) -> OutboundStampCosts {
        let data = match std::fs::read(self.outbound_stamp_costs_path()) {
            Ok(data) => data,
            Err(_) => return OutboundStampCosts::default(),
        };
        let value = match rmpv::decode::read_value(&mut Cursor::new(data)) {
            Ok(value) => value,
            Err(_) => return OutboundStampCosts::default(),
        };
        let entries = stamp_costs_from_value(&value);
        OutboundStampCosts { entries }
    }

    pub fn save_node_stats(&self, stats: &NodeStats) {
        let mut entries = Vec::new();
        entries.push((
            Value::from("client_propagation_messages_received"),
            Value::from(stats.client_propagation_messages_received as i64),
        ));
        entries.push((
            Value::from("client_propagation_messages_served"),
            Value::from(stats.client_propagation_messages_served as i64),
        ));
        entries.push((
            Value::from("unpeered_propagation_incoming"),
            Value::from(stats.unpeered_propagation_incoming as i64),
        ));
        entries.push((
            Value::from("unpeered_propagation_rx_bytes"),
            Value::from(stats.unpeered_propagation_rx_bytes as i64),
        ));
        let mut buf = Vec::new();
        if rmpv::encode::write_value(&mut buf, &Value::Map(entries)).is_ok() {
            let _ = std::fs::write(self.node_stats_path(), buf);
        }
    }

    pub fn load_node_stats(&self) -> NodeStats {
        let data = match std::fs::read(self.node_stats_path()) {
            Ok(data) => data,
            Err(_) => return NodeStats::default(),
        };
        let value = match rmpv::decode::read_value(&mut Cursor::new(data)) {
            Ok(value) => value,
            Err(_) => return NodeStats::default(),
        };
        let map = match value {
            Value::Map(map) => map,
            _ => return NodeStats::default(),
        };

        let mut stats = NodeStats::default();
        for (key, val) in map {
            let key = match key.as_str() {
                Some(key) => key,
                None => continue,
            };
            match key {
                "client_propagation_messages_received" => {
                    if let Some(v) = value_to_opt_u64(&val) {
                        stats.client_propagation_messages_received = v;
                    }
                }
                "client_propagation_messages_served" => {
                    if let Some(v) = value_to_opt_u64(&val) {
                        stats.client_propagation_messages_served = v;
                    }
                }
                "unpeered_propagation_incoming" => {
                    if let Some(v) = value_to_opt_u64(&val) {
                        stats.unpeered_propagation_incoming = v;
                    }
                }
                "unpeered_propagation_rx_bytes" => {
                    if let Some(v) = value_to_opt_u64(&val) {
                        stats.unpeered_propagation_rx_bytes = v;
                    }
                }
                _ => {}
            }
        }
        stats
    }
}

fn encode_message_value(message: &LXMessage) -> Value {
    let signature = message.signature.map(|sig| sig.to_vec());
    let message_id = message.message_id.map(|id| id.to_vec());

    Value::Map(vec![
        (Value::from("dest"), Value::Binary(message.destination_hash.to_vec())),
        (Value::from("src"), Value::Binary(message.source_hash.to_vec())),
        (Value::from("timestamp"), Value::F64(message.timestamp)),
        (Value::from("title"), Value::Binary(message.title.clone())),
        (Value::from("content"), Value::Binary(message.content.clone())),
        (Value::from("fields"), message.fields.clone()),
        (Value::from("stamp"), option_bytes_to_value(&message.stamp)),
        (Value::from("stamp_cost"), option_u32_to_value(message.stamp_cost)),
        (Value::from("stamp_value"), option_u32_to_value(message.stamp_value)),
        (Value::from("stamp_valid"), Value::Boolean(message.stamp_valid)),
        (Value::from("stamp_checked"), Value::Boolean(message.stamp_checked)),
        (Value::from("outbound_ticket"), option_bytes_to_value(&message.outbound_ticket)),
        (Value::from("include_ticket"), Value::Boolean(message.include_ticket)),
        (Value::from("defer_stamp"), Value::Boolean(message.defer_stamp)),
        (Value::from("state"), Value::from(message.state as i64)),
        (Value::from("method"), option_u8_to_value(message.method)),
        (Value::from("representation"), Value::from(message.representation as i64)),
        (Value::from("transport_encrypted"), option_bool_to_value(message.transport_encrypted)),
        (
            Value::from("transport_encryption"),
            option_string_to_value(&message.transport_encryption),
        ),
        (Value::from("signature"), option_vec_to_value(signature)),
        (Value::from("message_id"), option_vec_to_value(message_id)),
    ])
}

fn decode_message_value(value: &Value) -> Result<LXMessage, RnsError> {
    let map = match value {
        Value::Map(entries) => entries.clone(),
        _ => return Err(RnsError::Msgpack("expected message map".to_string())),
    };

    let mut dest: Option<[u8; DESTINATION_LENGTH]> = None;
    let mut src: Option<[u8; DESTINATION_LENGTH]> = None;
    let mut timestamp: Option<f64> = None;
    let mut title: Option<Vec<u8>> = None;
    let mut content: Option<Vec<u8>> = None;
    let mut fields: Option<Value> = None;
    let mut stamp: Option<Vec<u8>> = None;
    let mut stamp_cost: Option<u32> = None;
    let mut stamp_value: Option<u32> = None;
    let mut stamp_valid: Option<bool> = None;
    let mut stamp_checked: Option<bool> = None;
    let mut outbound_ticket: Option<Vec<u8>> = None;
    let mut include_ticket: Option<bool> = None;
    let mut defer_stamp: Option<bool> = None;
    let mut state: Option<u8> = None;
    let mut method: Option<u8> = None;
    let mut representation: Option<u8> = None;
    let mut transport_encrypted: Option<bool> = None;
    let mut transport_encryption: Option<String> = None;
    let mut signature: Option<[u8; SIGNATURE_LENGTH]> = None;
    let mut message_id: Option<[u8; HASH_LENGTH]> = None;

    for (key, val) in map {
        let key = match key.as_str() {
            Some(key) => key,
            None => continue,
        };
        match key {
            "dest" => dest = value_to_fixed_bytes::<DESTINATION_LENGTH>(&val),
            "src" => src = value_to_fixed_bytes::<DESTINATION_LENGTH>(&val),
            "timestamp" => timestamp = value_to_f64(&val),
            "title" => title = value_to_bytes(&val),
            "content" => content = value_to_bytes(&val),
            "fields" => fields = Some(val),
            "stamp" => stamp = value_to_opt_bytes(&val),
            "stamp_cost" => stamp_cost = value_to_opt_u32(&val),
            "stamp_value" => stamp_value = value_to_opt_u32(&val),
            "stamp_valid" => stamp_valid = value_to_opt_bool(&val),
            "stamp_checked" => stamp_checked = value_to_opt_bool(&val),
            "outbound_ticket" => outbound_ticket = value_to_opt_bytes(&val),
            "include_ticket" => include_ticket = value_to_opt_bool(&val),
            "defer_stamp" => defer_stamp = value_to_opt_bool(&val),
            "state" => state = value_to_opt_u8(&val),
            "method" => method = value_to_opt_u8(&val),
            "representation" => representation = value_to_opt_u8(&val),
            "transport_encrypted" => transport_encrypted = value_to_opt_bool(&val),
            "transport_encryption" => transport_encryption = value_to_opt_string(&val),
            "signature" => signature = value_to_fixed_bytes::<SIGNATURE_LENGTH>(&val),
            "message_id" => message_id = value_to_fixed_bytes::<HASH_LENGTH>(&val),
            _ => {}
        }
    }

    let dest = dest.ok_or_else(|| RnsError::Msgpack("missing dest".to_string()))?;
    let src = src.ok_or_else(|| RnsError::Msgpack("missing src".to_string()))?;
    let mut message = LXMessage::new(dest, src);
    message.timestamp = timestamp.unwrap_or(0.0);
    message.title = title.unwrap_or_default();
    message.content = content.unwrap_or_default();
    message.fields = fields.unwrap_or_else(|| Value::Map(vec![]));
    message.stamp = stamp;
    message.stamp_cost = stamp_cost;
    message.stamp_value = stamp_value;
    message.stamp_valid = stamp_valid.unwrap_or(false);
    message.stamp_checked = stamp_checked.unwrap_or(false);
    message.outbound_ticket = outbound_ticket;
    message.include_ticket = include_ticket.unwrap_or(false);
    message.defer_stamp = defer_stamp.unwrap_or(false);
    if let Some(state) = state {
        message.state = state;
    }
    message.method = method;
    if let Some(repr) = representation {
        message.representation = repr;
    }
    message.transport_encrypted = transport_encrypted;
    message.transport_encryption = transport_encryption;
    message.signature = signature;
    message.message_id = message_id;

    Ok(message)
}

fn option_bytes_to_value(value: &Option<Vec<u8>>) -> Value {
    match value {
        Some(bytes) => Value::Binary(bytes.clone()),
        None => Value::Nil,
    }
}

fn option_vec_to_value(value: Option<Vec<u8>>) -> Value {
    match value {
        Some(bytes) => Value::Binary(bytes),
        None => Value::Nil,
    }
}

fn option_u32_to_value(value: Option<u32>) -> Value {
    match value {
        Some(v) => Value::from(v as i64),
        None => Value::Nil,
    }
}

fn option_u8_to_value(value: Option<u8>) -> Value {
    match value {
        Some(v) => Value::from(v as i64),
        None => Value::Nil,
    }
}

fn option_bool_to_value(value: Option<bool>) -> Value {
    match value {
        Some(v) => Value::Boolean(v),
        None => Value::Nil,
    }
}

fn option_string_to_value(value: &Option<String>) -> Value {
    match value {
        Some(v) => Value::String(v.clone().into()),
        None => Value::Nil,
    }
}

pub(crate) fn propagation_entry_to_bytes(entry: &PropagationEntry) -> Result<Vec<u8>, RnsError> {
    let value = propagation_entry_to_value(entry);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub(crate) fn propagation_entry_from_bytes(bytes: &[u8]) -> Result<PropagationEntry, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    propagation_entry_from_value(&value)
}

fn parse_propagation_entry_filename(filename: &str) -> Option<([u8; 32], f64, u32)> {
    let parts: Vec<&str> = filename.split('_').collect();
    if parts.len() != 3 {
        return None;
    }
    if parts[0].len() != 64 {
        return None;
    }
    let bytes = hex::decode(parts[0]).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut transient_id = [0u8; 32];
    transient_id.copy_from_slice(&bytes);
    let timestamp: f64 = parts[1].parse().ok()?;
    if !timestamp.is_finite() || timestamp <= 0.0 {
        return None;
    }
    let stamp_value: u32 = parts[2].parse().ok()?;
    Some((transient_id, timestamp, stamp_value))
}

fn propagation_entry_filename(
    transient_id: &[u8; 32],
    timestamp: f64,
    stamp_value: u32,
) -> String {
    format!("{}_{}_{}", hex::encode(transient_id), timestamp, stamp_value)
}

fn propagation_entry_to_value(entry: &PropagationEntry) -> Value {
    Value::Array(vec![
        Value::Binary(entry.transient_id.to_vec()),
        Value::Binary(entry.lxm_data.clone()),
        Value::F64(entry.timestamp),
    ])
}

fn propagation_entry_from_value(value: &Value) -> Result<PropagationEntry, RnsError> {
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("propagation entry must be array".to_string())),
    };
    if arr.len() < 3 {
        return Err(RnsError::Msgpack("propagation entry missing fields".to_string()));
    }
    let transient_id = value_to_fixed_bytes::<32>(&arr[0])
        .ok_or_else(|| RnsError::Msgpack("invalid transient id".to_string()))?;
    let lxm_data = value_to_bytes(&arr[1])
        .ok_or_else(|| RnsError::Msgpack("invalid lxm data".to_string()))?;
    let timestamp = match &arr[2] {
        Value::F64(v) => *v,
        Value::F32(v) => *v as f64,
        Value::Integer(i) => i
            .as_i64()
            .map(|v| v as f64)
            .ok_or_else(|| RnsError::Msgpack("invalid timestamp".to_string()))?,
        _ => return Err(RnsError::Msgpack("invalid timestamp".to_string())),
    };
    Ok(PropagationEntry {
        transient_id,
        lxm_data,
        timestamp,
    })
}

fn delivery_mode_to_i64(mode: DeliveryMode) -> i64 {
    match mode {
        DeliveryMode::Opportunistic => 0,
        DeliveryMode::Direct => 1,
        DeliveryMode::Propagated => 2,
        DeliveryMode::Paper => 3,
    }
}

fn delivery_mode_from_i64(value: i64) -> Result<DeliveryMode, RnsError> {
    match value {
        0 => Ok(DeliveryMode::Opportunistic),
        1 => Ok(DeliveryMode::Direct),
        2 => Ok(DeliveryMode::Propagated),
        3 => Ok(DeliveryMode::Paper),
        _ => Err(RnsError::Msgpack("invalid delivery mode".to_string())),
    }
}

fn peer_from_value(value: &Value) -> Result<Peer, RnsError> {
    let entries = match value {
        Value::Map(map) => map,
        _ => return Err(RnsError::Msgpack("peer must be map".to_string())),
    };

    let mut id = None;
    let mut state = None;
    let mut last_sync_ms = None;
    let mut last_attempt_ms = None;
    let mut next_sync_ms = None;
    let mut backoff_ms = None;
    let mut failures = None;
    let mut last_heard_ms = None;
    let mut rx_bytes = None;
    let mut tx_bytes = None;
    let mut messages_offered = None;
    let mut messages_outgoing = None;
    let mut messages_incoming = None;
    let mut messages_unhandled = None;
    let mut peering_key: Option<Vec<u8>> = None;

    for (key, val) in entries {
        let key = match key {
            Value::String(s) => s.as_str().map(|v| v.to_string()),
            _ => None,
        };
        let Some(key) = key else { continue };
        match key.as_str() {
            "id" => {
                if let Some(bytes) = value_to_fixed_bytes::<DESTINATION_LENGTH>(&val) {
                    id = Some(bytes);
                }
            }
            "state" => {
                if let Value::Integer(i) = val {
                    state = i
                        .as_u64()
                        .and_then(|v| u8::try_from(v).ok())
                        .and_then(peer_state_from_u8);
                }
            }
            "last_sync_ms" => last_sync_ms = value_to_opt_u64(&val),
            "last_attempt_ms" => last_attempt_ms = value_to_opt_u64(&val),
            "next_sync_ms" => next_sync_ms = value_to_opt_u64(&val),
            "backoff_ms" => backoff_ms = value_to_opt_u64(&val),
            "failures" => failures = value_to_opt_u64(&val).and_then(|v| u32::try_from(v).ok()),
            "last_heard_ms" => last_heard_ms = value_to_opt_u64(&val),
            "rx_bytes" => rx_bytes = value_to_opt_u64(&val),
            "tx_bytes" => tx_bytes = value_to_opt_u64(&val),
            "messages_offered" => messages_offered = value_to_opt_u64(&val),
            "messages_outgoing" => messages_outgoing = value_to_opt_u64(&val),
            "messages_incoming" => messages_incoming = value_to_opt_u64(&val),
            "messages_unhandled" => messages_unhandled = value_to_opt_u64(&val),
            "peering_key" => {
                peering_key = match val {
                    Value::Binary(bytes) => Some(bytes.clone()),
                    _ => None,
                }
            }
            _ => {}
        }
    }

    let id = id.ok_or_else(|| RnsError::Msgpack("peer missing id".to_string()))?;
    let mut peer = Peer::new(id, 0);
    if let Some(state) = state {
        peer.state = state;
    }
    if let Some(v) = last_sync_ms {
        peer.last_sync_ms = v;
    }
    if let Some(v) = last_attempt_ms {
        peer.last_attempt_ms = v;
    }
    if let Some(v) = next_sync_ms {
        peer.next_sync_ms = v;
    }
    if let Some(v) = backoff_ms {
        peer.backoff_ms = v;
    }
    if let Some(v) = failures {
        peer.failures = v;
    }
    if let Some(v) = last_heard_ms {
        peer.last_heard_ms = v;
    }
    if let Some(v) = rx_bytes {
        peer.rx_bytes = v;
    }
    if let Some(v) = tx_bytes {
        peer.tx_bytes = v;
    }
    if let Some(v) = messages_offered {
        peer.messages_offered = v;
    }
    if let Some(v) = messages_outgoing {
        peer.messages_outgoing = v;
    }
    if let Some(v) = messages_incoming {
        peer.messages_incoming = v;
    }
    if let Some(v) = messages_unhandled {
        peer.messages_unhandled = v;
    }
    if let Some(v) = peering_key {
        peer.peering_key = Some(v);
    }
    Ok(peer)
}

fn peer_state_from_u8(value: u8) -> Option<PeerState> {
    match value {
        x if x == PeerState::Idle as u8 => Some(PeerState::Idle),
        x if x == PeerState::OfferSent as u8 => Some(PeerState::OfferSent),
        x if x == PeerState::AwaitingPayload as u8 => Some(PeerState::AwaitingPayload),
        x if x == PeerState::Backoff as u8 => Some(PeerState::Backoff),
        _ => None,
    }
}

fn value_to_f64_seconds(value: &Value) -> Option<f64> {
    if let Some(v) = value.as_i64() {
        if v >= 0 {
            return Some(v as f64);
        } else {
            return None;
        }
    }
    let v = value_to_f64(value)?;
    if v < 0.0 {
        None
    } else {
        Some(v)
    }
}

fn tickets_outbound_to_value(
    outbound: &HashMap<[u8; DESTINATION_LENGTH], (f64, Vec<u8>)>,
) -> Value {
    let mut entries = Vec::new();
    for (dest, (expires_at, ticket)) in outbound {
        entries.push((
            Value::Binary(dest.to_vec()),
            Value::Array(vec![Value::F64(*expires_at), Value::Binary(ticket.clone())]),
        ));
    }
    Value::Map(entries)
}

fn tickets_outbound_from_value(value: &Value) -> HashMap<[u8; DESTINATION_LENGTH], (f64, Vec<u8>)> {
    let mut out = HashMap::new();
    let entries = match value {
        Value::Map(map) => map,
        _ => return out,
    };
    for (key, val) in entries {
        let dest = match value_to_fixed_bytes::<DESTINATION_LENGTH>(key) {
            Some(dest) => dest,
            None => continue,
        };
        let arr = match val {
            Value::Array(arr) => arr,
            _ => continue,
        };
        if arr.len() < 2 {
            continue;
        }
        let expires_at = match value_to_f64_seconds(&arr[0]) {
            Some(v) => v,
            None => continue,
        };
        let ticket = match &arr[1] {
            Value::Binary(bytes) => bytes.clone(),
            _ => continue,
        };
        out.insert(dest, (expires_at, ticket));
    }
    out
}

fn tickets_inbound_to_value(
    inbound: &HashMap<[u8; DESTINATION_LENGTH], HashMap<Vec<u8>, f64>>,
) -> Value {
    let mut entries = Vec::new();
    for (dest, tickets) in inbound {
        let mut inner = Vec::new();
        for (ticket, expires_at) in tickets {
            inner.push((
                Value::Binary(ticket.clone()),
                Value::Array(vec![Value::F64(*expires_at)]),
            ));
        }
        entries.push((Value::Binary(dest.to_vec()), Value::Map(inner)));
    }
    Value::Map(entries)
}

fn tickets_inbound_from_value(
    value: &Value,
) -> HashMap<[u8; DESTINATION_LENGTH], HashMap<Vec<u8>, f64>> {
    let mut out = HashMap::new();
    let entries = match value {
        Value::Map(map) => map,
        _ => return out,
    };
    for (key, val) in entries {
        let dest = match value_to_fixed_bytes::<DESTINATION_LENGTH>(key) {
            Some(dest) => dest,
            None => continue,
        };
        let tickets_map = match val {
            Value::Map(map) => map,
            _ => continue,
        };
        let mut tickets = HashMap::new();
        for (ticket_key, expires_val) in tickets_map {
            let ticket = match ticket_key {
                Value::Binary(bytes) => bytes.clone(),
                _ => continue,
            };
            let expires_at = match expires_val {
                Value::Array(arr) if !arr.is_empty() => value_to_f64_seconds(&arr[0]),
                _ => value_to_f64_seconds(expires_val),
            };
            if let Some(expires_at) = expires_at {
                tickets.insert(ticket, expires_at);
            }
        }
        out.insert(dest, tickets);
    }
    out
}

fn tickets_last_deliveries_to_value(
    last_deliveries: &HashMap<[u8; DESTINATION_LENGTH], f64>,
) -> Value {
    let mut entries = Vec::new();
    for (dest, ts) in last_deliveries {
        entries.push((Value::Binary(dest.to_vec()), Value::F64(*ts)));
    }
    Value::Map(entries)
}

fn tickets_last_deliveries_from_value(value: &Value) -> HashMap<[u8; DESTINATION_LENGTH], f64> {
    let mut out = HashMap::new();
    let entries = match value {
        Value::Map(map) => map,
        _ => return out,
    };
    for (key, val) in entries {
        let dest = match value_to_fixed_bytes::<DESTINATION_LENGTH>(key) {
            Some(dest) => dest,
            None => continue,
        };
        if let Some(ts) = value_to_f64_seconds(val) {
            out.insert(dest, ts);
        }
    }
    out
}

fn stamp_costs_to_value(entries: &HashMap<[u8; DESTINATION_LENGTH], (f64, u32)>) -> Value {
    let mut out = Vec::new();
    for (dest, (updated_at, cost)) in entries {
        out.push((
            Value::Binary(dest.to_vec()),
            Value::Array(vec![Value::F64(*updated_at), Value::from(*cost as i64)]),
        ));
    }
    Value::Map(out)
}

fn stamp_costs_from_value(value: &Value) -> HashMap<[u8; DESTINATION_LENGTH], (f64, u32)> {
    let mut out = HashMap::new();
    let entries = match value {
        Value::Map(map) => map,
        _ => return out,
    };
    for (key, val) in entries {
        let dest = match value_to_fixed_bytes::<DESTINATION_LENGTH>(key) {
            Some(dest) => dest,
            None => continue,
        };
        let arr = match val {
            Value::Array(arr) => arr,
            _ => continue,
        };
        if arr.len() < 2 {
            continue;
        }
        let updated_at = match value_to_f64_seconds(&arr[0]) {
            Some(v) => v,
            None => continue,
        };
        let cost = match value_to_opt_u32(&arr[1]) {
            Some(v) => v,
            None => continue,
        };
        out.insert(dest, (updated_at, cost));
    }
    out
}

fn load_transient_cache(path: std::path::PathBuf) -> HashMap<[u8; 32], f64> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(_) => return HashMap::new(),
    };
    let value = match rmpv::decode::read_value(&mut Cursor::new(data)) {
        Ok(value) => value,
        Err(_) => return HashMap::new(),
    };
    transient_cache_from_value(&value)
}

fn transient_cache_to_value(cache: &HashMap<[u8; 32], f64>) -> Value {
    let mut entries = Vec::new();
    for (key, timestamp) in cache {
        entries.push((Value::Binary(key.to_vec()), Value::F64(*timestamp)));
    }
    Value::Map(entries)
}

fn transient_cache_from_value(value: &Value) -> HashMap<[u8; 32], f64> {
    let mut out = HashMap::new();
    let entries = match value {
        Value::Map(map) => map,
        _ => return out,
    };
    for (key, val) in entries {
        let id = match value_to_fixed_bytes::<32>(key) {
            Some(id) => id,
            None => continue,
        };
        let timestamp = match val {
            Value::F64(v) => *v,
            Value::F32(v) => *v as f64,
            Value::Integer(i) => i.as_i64().map(|v| v as f64).unwrap_or(0.0),
            _ => 0.0,
        };
        if timestamp > 0.0 {
            out.insert(id, timestamp);
        }
    }
    out
}

pub fn clean_transient_cache(cache: &mut HashMap<[u8; 32], f64>, now_s: f64) -> usize {
    let expiry = MESSAGE_EXPIRY as f64 * 6.0;
    let before = cache.len();
    cache.retain(|_, ts| now_s <= *ts + expiry);
    before.saturating_sub(cache.len())
}
