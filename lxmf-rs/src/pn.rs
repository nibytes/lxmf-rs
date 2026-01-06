use rmpv::Value;
use std::collections::HashMap;
use std::io::Cursor;

use crate::constants::{DESTINATION_LENGTH, PN_META_NAME};

#[derive(Debug, thiserror::Error)]
pub enum PnDirectoryError {
    #[error("invalid propagation node announce data")]
    InvalidAnnounce,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PnDirectoryEntry {
    pub destination_hash: [u8; DESTINATION_LENGTH],
    pub app_data: Vec<u8>,
    pub name: Option<String>,
    pub stamp_cost: Option<u64>,
    pub last_seen_ms: u64,
    pub acked: bool,
}

#[derive(Default, Debug)]
pub struct PnDirectory {
    entries: HashMap<[u8; DESTINATION_LENGTH], PnDirectoryEntry>,
}

impl PnDirectory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn upsert_announce(
        &mut self,
        destination_hash: [u8; DESTINATION_LENGTH],
        app_data: &[u8],
        now_ms: u64,
    ) -> Result<(), PnDirectoryError> {
        if !pn_announce_data_is_valid(app_data) {
            return Err(PnDirectoryError::InvalidAnnounce);
        }

        let existing_acked = self
            .entries
            .get(&destination_hash)
            .map(|entry| entry.acked)
            .unwrap_or(false);

        let entry = PnDirectoryEntry {
            destination_hash,
            app_data: app_data.to_vec(),
            name: pn_name_from_app_data(app_data),
            stamp_cost: pn_stamp_cost_from_app_data(app_data),
            last_seen_ms: now_ms,
            acked: existing_acked,
        };

        self.entries.insert(destination_hash, entry);
        Ok(())
    }

    pub fn list(&self) -> Vec<PnDirectoryEntry> {
        let mut out: Vec<PnDirectoryEntry> = self.entries.values().cloned().collect();
        out.sort_by_key(|entry| entry.last_seen_ms);
        out
    }

    pub fn get(&self, destination_hash: &[u8; DESTINATION_LENGTH]) -> Option<&PnDirectoryEntry> {
        self.entries.get(destination_hash)
    }

    pub fn stamp_cost(&self, destination_hash: &[u8; DESTINATION_LENGTH]) -> Option<u64> {
        self.entries.get(destination_hash).and_then(|entry| entry.stamp_cost)
    }

    pub fn acknowledge(&mut self, destination_hash: &[u8; DESTINATION_LENGTH]) -> bool {
        if let Some(entry) = self.entries.get_mut(destination_hash) {
            entry.acked = true;
            true
        } else {
            false
        }
    }

    pub fn delete(&mut self, destination_hash: &[u8; DESTINATION_LENGTH]) -> Option<PnDirectoryEntry> {
        self.entries.remove(destination_hash)
    }

    pub fn list_unacked(&self) -> Vec<PnDirectoryEntry> {
        let mut out: Vec<PnDirectoryEntry> = self
            .entries
            .values()
            .filter(|entry| !entry.acked)
            .cloned()
            .collect();
        out.sort_by_key(|entry| entry.last_seen_ms);
        out
    }
}

pub fn display_name_from_app_data(app_data: &[u8]) -> Option<String> {
    if app_data.is_empty() {
        return None;
    }

    if is_msgpack_list(app_data) {
        let value = rmpv::decode::read_value(&mut Cursor::new(app_data)).ok()?;
        let data = match value {
            Value::Array(arr) => arr,
            _ => return None,
        };
        let dn = data.get(0)?;
        match dn {
            Value::Binary(b) => String::from_utf8(b.clone()).ok(),
            Value::String(s) => s.as_str().map(|v| v.to_string()),
            Value::Nil => None,
            _ => None,
        }
    } else {
        String::from_utf8(app_data.to_vec()).ok()
    }
}

pub fn stamp_cost_from_app_data(app_data: &[u8]) -> Option<u64> {
    if app_data.is_empty() {
        return None;
    }
    if is_msgpack_list(app_data) {
        let value = rmpv::decode::read_value(&mut Cursor::new(app_data)).ok()?;
        let data = match value {
            Value::Array(arr) => arr,
            _ => return None,
        };
        let cost = data.get(1)?;
        match cost {
            Value::Integer(i) => i.as_u64(),
            _ => None,
        }
    } else {
        None
    }
}

pub fn pn_name_from_app_data(app_data: &[u8]) -> Option<String> {
    if !pn_announce_data_is_valid(app_data) {
        return None;
    }
    let value = rmpv::decode::read_value(&mut Cursor::new(app_data)).ok()?;
    let data = match value {
        Value::Array(arr) => arr,
        _ => return None,
    };
    let metadata = match &data[6] {
        Value::Map(map) => map,
        _ => return None,
    };
    for (key, val) in metadata {
        if matches!(key, Value::Integer(i) if i.as_u64() == Some(u64::from(PN_META_NAME))) {
            return match val {
                Value::Binary(b) => String::from_utf8(b.clone()).ok(),
                Value::String(s) => s.as_str().map(|v| v.to_string()),
                _ => None,
            };
        }
    }
    None
}

pub fn pn_announce_data_is_valid(app_data: &[u8]) -> bool {
    let value = match rmpv::decode::read_value(&mut Cursor::new(app_data)) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let data = match value {
        Value::Array(arr) => arr,
        _ => return false,
    };

    if data.len() < 7 {
        return false;
    }

    if !matches!(data[1], Value::Integer(_)) {
        return false;
    }
    if !matches!(data[2], Value::Boolean(_)) {
        return false;
    }
    if !matches!(data[3], Value::Integer(_)) {
        return false;
    }
    if !matches!(data[4], Value::Integer(_)) {
        return false;
    }

    let stamp_costs = match &data[5] {
        Value::Array(arr) => arr,
        _ => return false,
    };
    if stamp_costs.len() < 3 {
        return false;
    }
    if !matches!(stamp_costs[0], Value::Integer(_))
        || !matches!(stamp_costs[1], Value::Integer(_))
        || !matches!(stamp_costs[2], Value::Integer(_))
    {
        return false;
    }

    matches!(data[6], Value::Map(_))
}

pub fn pn_stamp_cost_from_app_data(app_data: &[u8]) -> Option<u64> {
    let value = rmpv::decode::read_value(&mut Cursor::new(app_data)).ok()?;
    let data = match value {
        Value::Array(arr) => arr,
        _ => return None,
    };
    if data.len() < 7 {
        return None;
    }

    let stamp_costs = match &data[5] {
        Value::Array(arr) => arr,
        _ => return None,
    };
    let first = stamp_costs.get(0)?;
    match first {
        Value::Integer(i) => i.as_u64(),
        _ => None,
    }
}

fn is_msgpack_list(app_data: &[u8]) -> bool {
    matches!(app_data.first(), Some(0x90..=0x9f) | Some(0xdc))
}
