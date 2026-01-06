# lxmf-rs

Self-contained Rust LXMF message codec (encode/decode, message-id hash, Ed25519 signature).
Optional Reticulum integration is available behind the `rns` feature, and the goal is full practical parity for delivery.

## Round-trip example

```rust
use ed25519_dalek::SigningKey;
use lxmf_rs::{LXMessage, Value};

let dest = [0x11u8; 16];
let src = [0x22u8; 16];
let fields = Value::Map(vec![]);
let mut msg = LXMessage::with_strings(dest, src, 1_700_000_000.0, "hi", "hello", fields);

let signing_key = SigningKey::from_bytes(&[7u8; 32]);
let bytes = msg.encode_signed(&signing_key).unwrap();

let decoded = LXMessage::decode(&bytes).unwrap();
assert!(decoded.verify(&signing_key.verifying_key()).unwrap());
```

Notes:
- Payload is msgpack array: `[timestamp, title_bytes, content_bytes, fields]`.
- `message_id` = SHA-256(dest_hash || src_hash || packed_payload_without_stamp).
- Signature = Ed25519 over `dest_hash || src_hash || packed_payload_without_stamp || message_id`.

Status:
- Golden-tested for core, ticket-stamp, propagation, paper/URI helpers, plus PN/transient-id utilities (see `PORTING_STATUS.md`).
- PN directory/list/ack helpers are available for propagation node metadata.
- QR helper available (text render).
- `rns` module provides a thin LXMF-over-Reticulum delivery layer; building it only requires `protoc`
  when the `kaonic` feature is enabled.
- Includes basic outbound queue/scheduler helpers (no real network loop).
- Includes in-memory + file-backed storage hooks, callbacks, and a minimal inbound pipeline.
- Includes an experimental Node-based runtime (`RnsNodeRouter`) for real Reticulum links.
- Includes PN directory list/get/ack message helpers (msgpack over request/response packets).
- Includes inflight delivery tracking with receipt/timeout transitions.
- Includes request/response primitives (correlation IDs + timeouts).
- Includes a minimal runtime scheduler for periodic ticks (library-only).
- Includes peer sync offer→get→ack flow helpers with propagation store updates.
- Includes minimal in-memory peer management (states + backoff + rotation).
- Includes a small LxmfRouter facade that composes runtime + peers.
- Facade exposes PN directory, propagation store, and peer-sync helpers.
- Facade can be constructed with FileMessageStore for persistent queues.
- Propagation store supports time-based cleanup helpers.
- File-backed persistence for propagation entries and PN directory metadata.
- Inbound stamp validation helper for ticket- or PoW-based stamps.
- Container file helper (`unpack_from_file`) for msgpack wrappers.
- Golden peer-sync Offer vector covering Python umsgpack bytes.
- In-memory delivered-message cache to filter duplicates by message_id.
- PeerTable persistence via FileMessageStore (peer metadata restore).

Environment notes:
- `reticulum` build (and runtime tests) require `protoc` only when `kaonic` is enabled.

To run integration tests:
```bash
cargo test --features rns
```

End-to-end test (ignored by default):
```bash
RNS_E2E=1 cargo test --features rns -- --ignored
```

## Scope vs Python
`lxmf-rs` aims to reach practical parity with Python LXMF/LXMRouter.
The codec and helpers are largely complete, while the Reticulum runtime layer is still partial and evolving.
Some daemon-level responsibilities (path selection, persistence policies, and full link lifecycle) remain unported.
