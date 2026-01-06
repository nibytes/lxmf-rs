use crate::constants::DESTINATION_LENGTH;
use crate::pn::PnDirectory;
use rmpv::Value;
use std::io::Cursor;

use super::{value_to_bytes, value_to_fixed_bytes, RnsError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PnDirOp {
    List = 1,
    Get = 2,
    Ack = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PnDirRequest {
    List,
    Get([u8; DESTINATION_LENGTH]),
    Ack(Vec<[u8; DESTINATION_LENGTH]>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PnDirResponse {
    List(Vec<PnDirEntryWire>),
    Get(Option<PnDirEntryWire>),
    Ack(Vec<[u8; DESTINATION_LENGTH]>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PnDirEntryWire {
    pub destination_hash: [u8; DESTINATION_LENGTH],
    pub app_data: Vec<u8>,
    pub last_seen_ms: u64,
    pub acked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PnDirPacket {
    Request(PnDirRequest),
    Response(PnDirResponse),
}

pub struct PnDirService {
    directory: PnDirectory,
}

impl PnDirService {
    pub fn new(directory: PnDirectory) -> Self {
        Self { directory }
    }

    pub fn directory(&self) -> &PnDirectory {
        &self.directory
    }

    pub fn directory_mut(&mut self) -> &mut PnDirectory {
        &mut self.directory
    }

    pub fn handle_request(&mut self, req: &PnDirRequest, _now_ms: u64) -> PnDirResponse {
        match req {
            PnDirRequest::List => {
                let entries = self
                    .directory
                    .list()
                    .into_iter()
                    .map(|entry| PnDirEntryWire {
                        destination_hash: entry.destination_hash,
                        app_data: entry.app_data,
                        last_seen_ms: entry.last_seen_ms,
                        acked: entry.acked,
                    })
                    .collect();
                PnDirResponse::List(entries)
            }
            PnDirRequest::Get(hash) => {
                let entry = self.directory.get(hash).map(|entry| PnDirEntryWire {
                    destination_hash: entry.destination_hash,
                    app_data: entry.app_data.clone(),
                    last_seen_ms: entry.last_seen_ms,
                    acked: entry.acked,
                });
                PnDirResponse::Get(entry)
            }
            PnDirRequest::Ack(hashes) => {
                let mut acked = Vec::new();
                for hash in hashes {
                    if self.directory.acknowledge(hash) {
                        acked.push(*hash);
                    }
                }
                if acked.is_empty() {
                    PnDirResponse::Ack(Vec::new())
                } else {
                    PnDirResponse::Ack(acked)
                }
            }
        }
    }

    pub fn apply_response(&mut self, response: &PnDirResponse, now_ms: u64) {
        match response {
            PnDirResponse::List(entries) => {
                for entry in entries {
                    let _ = self.directory.upsert_announce(
                        entry.destination_hash,
                        &entry.app_data,
                        entry.last_seen_ms.max(now_ms),
                    );
                    if entry.acked {
                        let _ = self.directory.acknowledge(&entry.destination_hash);
                    }
                }
            }
            PnDirResponse::Get(entry) => {
                if let Some(entry) = entry {
                    let _ = self.directory.upsert_announce(
                        entry.destination_hash,
                        &entry.app_data,
                        entry.last_seen_ms.max(now_ms),
                    );
                    if entry.acked {
                        let _ = self.directory.acknowledge(&entry.destination_hash);
                    }
                }
            }
            PnDirResponse::Ack(hashes) => {
                for hash in hashes {
                    let _ = self.directory.acknowledge(hash);
                }
            }
        }
    }
}

pub fn encode_pn_dir_request(req: &PnDirRequest) -> Result<Vec<u8>, RnsError> {
    let value = match req {
        PnDirRequest::List => Value::Array(vec![Value::from(PnDirOp::List as i64)]),
        PnDirRequest::Get(hash) => Value::Array(vec![
            Value::from(PnDirOp::Get as i64),
            Value::Binary(hash.to_vec()),
        ]),
        PnDirRequest::Ack(hashes) => Value::Array(vec![
            Value::from(PnDirOp::Ack as i64),
            Value::Array(
                hashes
                    .iter()
                    .map(|hash| Value::Binary(hash.to_vec()))
                    .collect(),
            ),
        ]),
    };

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub fn decode_pn_dir_request(bytes: &[u8]) -> Result<PnDirRequest, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("pn dir request must be array".to_string())),
    };
    if arr.is_empty() {
        return Err(RnsError::Msgpack("pn dir request missing op".to_string()));
    }
    let op = arr[0]
        .as_i64()
        .ok_or_else(|| RnsError::Msgpack("pn dir op invalid".to_string()))?;

    match op {
        x if x == PnDirOp::List as i64 => Ok(PnDirRequest::List),
        x if x == PnDirOp::Get as i64 => {
            let hash = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing hash".to_string()))?;
            let hash = value_to_fixed_bytes::<DESTINATION_LENGTH>(hash)
                .ok_or_else(|| RnsError::Msgpack("invalid hash".to_string()))?;
            Ok(PnDirRequest::Get(hash))
        }
        x if x == PnDirOp::Ack as i64 => {
            let list = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing ack list".to_string()))?;
            let list = match list {
                Value::Array(values) => values,
                _ => return Err(RnsError::Msgpack("ack list must be array".to_string())),
            };
            let mut hashes = Vec::with_capacity(list.len());
            for val in list {
                let hash = value_to_fixed_bytes::<DESTINATION_LENGTH>(val)
                    .ok_or_else(|| RnsError::Msgpack("invalid hash".to_string()))?;
                hashes.push(hash);
            }
            Ok(PnDirRequest::Ack(hashes))
        }
        _ => Err(RnsError::Msgpack("unknown pn dir op".to_string())),
    }
}

pub fn encode_pn_dir_response(resp: &PnDirResponse) -> Result<Vec<u8>, RnsError> {
    let value = match resp {
        PnDirResponse::List(entries) => Value::Array(vec![
            Value::from(PnDirOp::List as i64),
            Value::Array(entries.iter().map(entry_to_value).collect()),
        ]),
        PnDirResponse::Get(entry) => Value::Array(vec![
            Value::from(PnDirOp::Get as i64),
            match entry {
                Some(entry) => entry_to_value(entry),
                None => Value::Nil,
            },
        ]),
        PnDirResponse::Ack(hashes) => Value::Array(vec![
            Value::from(PnDirOp::Ack as i64),
            Value::Array(
                hashes
                    .iter()
                    .map(|hash| Value::Binary(hash.to_vec()))
                    .collect(),
            ),
        ]),
    };

    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &value)?;
    Ok(buf)
}

pub fn decode_pn_dir_response(bytes: &[u8]) -> Result<PnDirResponse, RnsError> {
    let value = rmpv::decode::read_value(&mut Cursor::new(bytes))?;
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("pn dir response must be array".to_string())),
    };
    if arr.is_empty() {
        return Err(RnsError::Msgpack("pn dir response missing op".to_string()));
    }
    let op = arr[0]
        .as_i64()
        .ok_or_else(|| RnsError::Msgpack("pn dir op invalid".to_string()))?;

    match op {
        x if x == PnDirOp::List as i64 => {
            let list = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing list".to_string()))?;
            let list = match list {
                Value::Array(values) => values,
                _ => return Err(RnsError::Msgpack("list must be array".to_string())),
            };
            let mut entries = Vec::with_capacity(list.len());
            for val in list {
                entries.push(entry_from_value(val)?);
            }
            Ok(PnDirResponse::List(entries))
        }
        x if x == PnDirOp::Get as i64 => {
            let entry_val = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing entry".to_string()))?;
            if matches!(entry_val, Value::Nil) {
                Ok(PnDirResponse::Get(None))
            } else {
                Ok(PnDirResponse::Get(Some(entry_from_value(entry_val)?)))
            }
        }
        x if x == PnDirOp::Ack as i64 => {
            let list = arr.get(1).ok_or_else(|| RnsError::Msgpack("missing ack list".to_string()))?;
            let list = match list {
                Value::Array(values) => values,
                _ => return Err(RnsError::Msgpack("ack list must be array".to_string())),
            };
            let mut hashes = Vec::with_capacity(list.len());
            for val in list {
                let hash = value_to_fixed_bytes::<DESTINATION_LENGTH>(val)
                    .ok_or_else(|| RnsError::Msgpack("invalid hash".to_string()))?;
                hashes.push(hash);
            }
            Ok(PnDirResponse::Ack(hashes))
        }
        _ => Err(RnsError::Msgpack("unknown pn dir op".to_string())),
    }
}

pub fn decode_pn_dir_packet(packet: &reticulum::packet::Packet) -> Result<PnDirPacket, RnsError> {
    match packet.context {
        reticulum::packet::PacketContext::Request => Ok(PnDirPacket::Request(decode_pn_dir_request(
            packet.data.as_slice(),
        )?)),
        reticulum::packet::PacketContext::Response => Ok(PnDirPacket::Response(decode_pn_dir_response(
            packet.data.as_slice(),
        )?)),
        _ => Err(RnsError::Msgpack("unsupported pn dir packet context".to_string())),
    }
}

pub(crate) fn entry_to_value(entry: &PnDirEntryWire) -> Value {
    Value::Array(vec![
        Value::Binary(entry.destination_hash.to_vec()),
        Value::Binary(entry.app_data.clone()),
        Value::from(entry.last_seen_ms as i64),
        Value::Boolean(entry.acked),
    ])
}

pub(crate) fn entry_from_value(value: &Value) -> Result<PnDirEntryWire, RnsError> {
    let arr = match value {
        Value::Array(arr) => arr,
        _ => return Err(RnsError::Msgpack("entry must be array".to_string())),
    };
    if arr.len() < 4 {
        return Err(RnsError::Msgpack("entry missing fields".to_string()));
    }
    let destination_hash = value_to_fixed_bytes::<DESTINATION_LENGTH>(&arr[0])
        .ok_or_else(|| RnsError::Msgpack("invalid entry hash".to_string()))?;
    let app_data = value_to_bytes(&arr[1]).ok_or_else(|| RnsError::Msgpack("invalid app data".to_string()))?;
    let last_seen_raw = arr[2]
        .as_i64()
        .ok_or_else(|| RnsError::Msgpack("invalid last_seen".to_string()))?;
    if last_seen_raw < 0 {
        return Err(RnsError::Msgpack("negative last_seen".to_string()));
    }
    let last_seen_ms = last_seen_raw as u64;
    let acked = match &arr[3] {
        Value::Boolean(v) => *v,
        _ => return Err(RnsError::Msgpack("invalid ack flag".to_string())),
    };
    Ok(PnDirEntryWire {
        destination_hash,
        app_data,
        last_seen_ms,
        acked,
    })
}
