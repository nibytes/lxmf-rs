use thiserror::Error;

#[derive(Debug, Error)]
pub enum LXMFError {
    #[error("invalid LXMF message length")]
    InvalidLength,
    #[error("invalid payload structure")]
    InvalidPayload,
    #[error("invalid bytes field")]
    InvalidBytes,
    #[error("invalid timestamp")]
    InvalidTimestamp,
    #[error("missing signature")]
    MissingSignature,
    #[error("qr error: {0}")]
    Qr(#[from] qrcode::types::QrError),
    #[error("msgpack decode error: {0}")]
    MsgpackDecode(#[from] rmpv::decode::Error),
    #[error("msgpack encode error: {0}")]
    MsgpackEncode(#[from] rmpv::encode::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
