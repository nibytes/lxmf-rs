//! LXMF message format in Rust.
//!
//! ```
//! use ed25519_dalek::SigningKey;
//! use lxmf_rs::{LXMessage, Value};
//!
//! let dest = [0x11u8; 16];
//! let src = [0x22u8; 16];
//! let fields = Value::Map(vec![]);
//! let mut msg = LXMessage::with_strings(dest, src, 1_700_000_000.0, "hi", "hello", fields);
//!
//! let signing_key = SigningKey::from_bytes(&[7u8; 32]);
//! let bytes = msg.encode_signed(&signing_key).unwrap();
//!
//! let decoded = LXMessage::decode(&bytes).unwrap();
//! let valid = decoded.verify(&signing_key.verifying_key()).unwrap();
//! assert!(valid);
//! ```

mod constants;
mod error;
mod message;
mod pn;
#[cfg(feature = "rns")]
mod rns;

pub use constants::*;
pub use error::LXMFError;
pub use message::{validate_peering_key, validate_pn_stamp, LXMessage};
pub use pn::{
    display_name_from_app_data, pn_announce_data_is_valid, pn_name_from_app_data,
    pn_stamp_cost_from_app_data, stamp_cost_from_app_data, PnDirectory, PnDirectoryEntry,
    PnDirectoryError,
};
#[cfg(feature = "rns")]
pub use rns::*;
pub use rmpv::Value;
