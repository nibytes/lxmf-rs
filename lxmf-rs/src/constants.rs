pub const DESTINATION_LENGTH: usize = 16;
pub const SIGNATURE_LENGTH: usize = 64;
pub const HASH_LENGTH: usize = 32;
pub const STAMP_SIZE: usize = 32;
pub const TICKET_LENGTH: usize = 16;
pub const TIMESTAMP_SIZE: usize = 8;
pub const STRUCT_OVERHEAD: usize = 8;
pub const LXMF_OVERHEAD: usize =
    2 * DESTINATION_LENGTH + SIGNATURE_LENGTH + TIMESTAMP_SIZE + STRUCT_OVERHEAD;

// LXMessage state/method/representation flags (from LXMessage.py)
pub const STATE_GENERATING: u8 = 0x00;
pub const STATE_OUTBOUND: u8 = 0x01;
pub const STATE_SENDING: u8 = 0x02;
pub const STATE_SENT: u8 = 0x04;
pub const STATE_DELIVERED: u8 = 0x08;
pub const STATE_REJECTED: u8 = 0xfd;
pub const STATE_CANCELLED: u8 = 0xfe;
pub const STATE_FAILED: u8 = 0xff;

pub const REPR_UNKNOWN: u8 = 0x00;
pub const REPR_PACKET: u8 = 0x01;
pub const REPR_RESOURCE: u8 = 0x02;

pub const METHOD_OPPORTUNISTIC: u8 = 0x01;
pub const METHOD_DIRECT: u8 = 0x02;
pub const METHOD_PROPAGATED: u8 = 0x03;
pub const METHOD_PAPER: u8 = 0x05;

pub const PN_META_VERSION: u8 = 0x00;
pub const PN_META_NAME: u8 = 0x01;
pub const PN_META_SYNC_STRATUM: u8 = 0x02;
pub const PN_META_SYNC_THROTTLE: u8 = 0x03;
pub const PN_META_AUTH_BAND: u8 = 0x04;
pub const PN_META_UTIL_PRESSURE: u8 = 0x05;
pub const PN_META_CUSTOM: u8 = 0xff;

pub const WORKBLOCK_EXPAND_ROUNDS: usize = 3000;
pub const WORKBLOCK_EXPAND_ROUNDS_PN: usize = 1000;
pub const WORKBLOCK_EXPAND_ROUNDS_PEERING: usize = 25;

pub const TICKET_EXPIRY: u64 = 21 * 24 * 60 * 60;
pub const TICKET_GRACE: u64 = 5 * 24 * 60 * 60;
pub const TICKET_RENEW: u64 = 14 * 24 * 60 * 60;
pub const TICKET_INTERVAL: u64 = 1 * 24 * 60 * 60;
pub const COST_TICKET: u32 = 0x100;
pub const MESSAGE_EXPIRY: u64 = 30 * 24 * 60 * 60;

// Core field identifiers from LXMF.py (subset; expand as needed).
pub const FIELD_EMBEDDED_LXMS: u8 = 0x01;
pub const FIELD_TELEMETRY: u8 = 0x02;
pub const FIELD_TELEMETRY_STREAM: u8 = 0x03;
pub const FIELD_ICON_APPEARANCE: u8 = 0x04;
pub const FIELD_FILE_ATTACHMENTS: u8 = 0x05;
pub const FIELD_IMAGE: u8 = 0x06;
pub const FIELD_AUDIO: u8 = 0x07;
pub const FIELD_THREAD: u8 = 0x08;
pub const FIELD_COMMANDS: u8 = 0x09;
pub const FIELD_RESULTS: u8 = 0x0a;
pub const FIELD_GROUP: u8 = 0x0b;
pub const FIELD_TICKET: u8 = 0x0c;
pub const FIELD_EVENT: u8 = 0x0d;
pub const FIELD_RNR_REFS: u8 = 0x0e;
pub const FIELD_RENDERER: u8 = 0x0f;

pub const FIELD_CUSTOM_TYPE: u8 = 0xfb;
pub const FIELD_CUSTOM_DATA: u8 = 0xfc;
pub const FIELD_CUSTOM_META: u8 = 0xfd;
pub const FIELD_NON_SPECIFIC: u8 = 0xfe;
pub const FIELD_DEBUG: u8 = 0xff;
