# Parity Summary (Python LXMF → Rust)

Status legend: ✅ = implemented + tests, ⚠️ = partial/limited, ❌ = not implemented.

| Feature / Entity (Python) | Rust Status | Notes |
|---|---|---|
| LXMessage core format (pack/unpack) | ✅ | Byte‑compatible msgpack payload + header; golden tests vs Python. |
| message_id + signature (Ed25519) | ✅ | Verified round‑trip tests. |
| Stamps (PoW) generation/validation | ✅ | Single‑threaded, test‑oriented cost; inbound validation helper added. |
| Tickets (stamp derivation) | ✅ | Ticket stamp generation + validation helper + tests. |
| Propagation packed format | ✅ | Golden test for propagation container bytes. |
| Paper packed + URI helper | ✅ | Golden tests; QR helper text render. |
| QR helper (text render) | ✅ | ASCII QR output only. |
| Packed container / unpack from container | ✅ | Includes metadata fields. |
| unpack_from_file (container) | ✅ | Matches Python helper; tested. |
| PN helpers (announce data, name, cost) | ✅ | Validation + extraction tests. |
| PN directory list/get/ack helpers | ✅ | Tests cover list/ack/round‑trip. |
| Propagation store (entries) | ✅ | In‑memory + file‑backed helpers; cleanup by age. |
| Peer sync codecs (offer/get/ack) | ✅ | Offer golden test vs Python umsgpack. |
| Peer sync flow (offer→get→ack) | ✅ | Implemented in rns module; unit tests. |
| Peer management (states/backoff) | ✅ | In‑memory table with rotation/backoff tests. |
| RNS delivery modes (opportunistic/direct/propagated/paper) | ⚠️ | Encoding/selection helpers; no full link/resource lifecycle. |
| RNS request/response primitives | ⚠️ | Library‑level; minimal control request usage. |
| Inflight receipts/timeouts | ⚠️ | Implemented in rns module; no full network receipt integration. |
| Runtime tick loop (library) | ⚠️ | Minimal scheduler, no daemon‑grade job loop. |
| Node/Transport integration | ⚠️ | `RnsNodeRouter` exists; end‑to‑end loopback tests ignored by default. |
| Propagation node control (stats/sync/unpeer) | ⚠️ | Minimal remote stats request in `lxmd-rs`; not full control policy. |
| Persistent store (outbound/inbound/failed) | ✅ | FileMessageStore with restore tests. |
| Persistence for peers/stats/long‑term state | 🟡 | PeerTable persisted via msgpack PeerStore; stats/long‑term state still TODO. |
| LXMRouter full job loop (Python) | ❌ | No full daemon job schedule (stamps/links/rotation). |
| Link/resource lifecycle (Python LXMRouter/LXMPeer) | ❌ | Only minimal send helpers; no full link management. |
| Advanced peer analytics & stats | ❌ | Basic counts only; no detailed metrics. |
| Full lxmd daemon parity | ⚠️ | `lxmd-rs` is a minimal skeleton (config/identity/inbound/basic status). |
