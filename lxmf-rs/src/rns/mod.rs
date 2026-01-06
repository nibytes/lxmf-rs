mod peer;
mod pn;
mod router;
mod storage;
mod transport;

pub use peer::*;
pub use pn::*;
pub use router::*;
pub use storage::*;
pub use transport::*;

use crate::error::LXMFError;
use rmpv::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Opportunistic,
    Direct,
    Propagated,
    Paper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RnsCryptoPolicy {
    #[default]
    AllowPlaintext,
    EnforceRatchets,
}

#[derive(Debug, thiserror::Error)]
pub enum RnsError {
    #[error("reticulum error: {0}")]
    Reticulum(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("invalid destination hash length")]
    InvalidDestinationHash,
    #[error("crypto policy violation ({policy:?}, {mode:?}): {reason}")]
    CryptoPolicyViolation {
        policy: RnsCryptoPolicy,
        mode: DeliveryMode,
        reason: String,
    },
    #[error("lxmf error: {0}")]
    Lxmf(#[from] LXMFError),
    #[error("msgpack decode error: {0}")]
    Msgpack(String),
}

impl From<reticulum::error::RnsError> for RnsError {
    fn from(err: reticulum::error::RnsError) -> Self {
        RnsError::Reticulum(format!("{:?}", err))
    }
}

impl From<rmpv::decode::Error> for RnsError {
    fn from(err: rmpv::decode::Error) -> Self {
        RnsError::Msgpack(err.to_string())
    }
}

impl From<rmpv::encode::Error> for RnsError {
    fn from(err: rmpv::encode::Error) -> Self {
        RnsError::Msgpack(err.to_string())
    }
}

impl From<std::io::Error> for RnsError {
    fn from(err: std::io::Error) -> Self {
        RnsError::Io(err.to_string())
    }
}

pub(crate) fn value_to_bytes(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Binary(bytes) => Some(bytes.clone()),
        _ => None,
    }
}

pub(crate) fn value_to_opt_bytes(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Nil => None,
        _ => value_to_bytes(value),
    }
}

pub(crate) fn value_to_opt_u32(value: &Value) -> Option<u32> {
    match value.as_i64() {
        Some(v) => Some(v as u32),
        None => None,
    }
}

pub(crate) fn value_to_opt_u64(value: &Value) -> Option<u64> {
    match value.as_i64() {
        Some(v) => u64::try_from(v).ok(),
        None => None,
    }
}

pub(crate) fn value_to_opt_u8(value: &Value) -> Option<u8> {
    match value.as_i64() {
        Some(v) => Some(v as u8),
        None => None,
    }
}

pub(crate) fn value_to_opt_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Boolean(v) => Some(*v),
        _ => None,
    }
}

pub(crate) fn value_to_opt_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => s.as_str().map(|v| v.to_string()),
        _ => None,
    }
}

pub(crate) fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::F64(v) => Some(*v),
        Value::F32(v) => Some(*v as f64),
        Value::Integer(v) => v.as_i64().map(|v| v as f64),
        _ => None,
    }
}

pub(crate) fn value_to_fixed_bytes<const N: usize>(value: &Value) -> Option<[u8; N]> {
    let bytes = value_to_bytes(value)?;
    if bytes.len() != N {
        return None;
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Some(out)
}

pub(crate) fn full_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}
