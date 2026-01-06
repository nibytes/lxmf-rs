# Porting status

This file tracks parity between the Python `LXMF.LXMessage` implementation and `lxmf-rs`.

## Implemented in lxmf-rs
- Core LXMessage packing/unpacking:
  - Payload layout: `[timestamp, title_bytes, content_bytes, fields]` with optional 5th stamp element.
  - Packed bytes: `dest_hash || src_hash || signature || msgpack(payload)`.
- `message_id` (SHA-256 of dest+src+payload_without_stamp).
- Ed25519 signing and verification over `dest+src+payload_without_stamp+message_id`.
- Stamp/ticket logic:
  - Ticket-based stamp derivation (`truncated_hash(ticket + message_id)`).
  - PoW stamp generation/validation (single-thread, low difficulty test usage).
  - Workblock/stamp validation parity with Python (msgpack uint salts, threshold-based validity).
- Peering/propagation stamp validation helpers (`validate_peering_key`, `validate_pn_stamp`).
- Inbound stamp validation helper (ticket or PoW) with tests and golden vectors.
- Propagation/paper/URI helpers:
  - `propagation_packed` and `paper_packed` builders for pre-encrypted payload bytes.
  - URI encoding (`lxm://` + urlsafe base64 without padding).
- Simple container support:
  - `packed_container` and `unpack_from_container` with LXMF bytes + optional metadata.
  - `unpack_from_file` helper to load msgpack containers from disk.
- PN helpers:
  - `pn_announce_data_is_valid`, `pn_stamp_cost_from_app_data`, `pn_name_from_app_data`.
  - `display_name_from_app_data`, `stamp_cost_from_app_data`.
  - PN directory/list/ack helpers for storing announce metadata.
- Transient ID + propagation stamp utilities:
  - `transient_id_from_encrypted` and `generate_propagation_stamp`.
- RNS integration (feature `rns`):
  - Minimal LXMF-over-Reticulum module with delivery modes and packet/resource selection.
  - Basic encryption hook (default uses reticulum Identity encryption).
  - Paper URI helper for delivery.
  - Basic decode path for opportunistic/direct/propagated packets.
  - Basic outbound queue and scheduling helpers (FIFO, one-at-a-time).
  - In-memory and file-backed storage hooks with restore on startup.
  - Delivery callbacks.
  - Minimal inbound pipeline using `decode_packet`.
  - Retry/backoff with failure retention and state transitions.
  - PN directory list/get/ack helpers over request/response packets.
  - Inflight delivery tracking with receipt/timeout transitions.
  - Request/response primitives with correlation IDs and timeouts (library-level).
  - Minimal runtime scheduler ticking queues/requests/timeouts (library-level).
- Peer sync offer→message_get→ack flow with propagation store updates and peer state transitions.
- Peer sync scheduling helper: tick enqueues next peer sync offer based on backoff and readiness.
- Runtime tick now surfaces peer sync requests for transport integration.
- Peer sync timeouts transition peers into backoff when no response is received within a fixed window.
- Peer sync requests/responses can be serialized via request/response bytes and routed with a minimal loopback dispatcher.
- Peer sync dispatchers accept/emit `RnsRequest`/`RnsResponse` for request/response plumbing integration.
- Request timeouts clear pending peer sync entries to avoid unbounded growth.
- Request/response packet payloads decode into peer-sync handlers via transport poll_inbound.
- Request IDs follow Reticulum packet-hash semantics (destination + context + payload).
- LxmfRouter exposes inbound `Received` routing and queues peer-sync responses/outbound requests.
- Transport supports sending request/response packets directly to destination hashes (peer-sync glue).
- Control-plane handlers for propagation nodes:
  - `/pn/get/stats`, `/pn/peer/sync`, `/pn/peer/unpeer` with allow-list gating.
  - Stats map includes propagation store + node stats fields required by lxmd status output.
- Propagation node auth/ignore gates:
  - Auth-required + allowed identity list (applied to propagation resource ingress).
  - Ignored destination list blocks inbound LXMessage delivery.
  - Peer stats include per-peer tx/rx counters and basic offered/outgoing/incoming counts.
- Resource-path propagation helper with peering-key gate, size limits, node_stats alignment, and payload decode.
- Resource payload ingress can be queued into the router and processed through the standard inbound loop.
- Transport poll_inbound falls back to resource payload decode when message/propagated decode fails.
- Peer sync peering-key validation and PN-stamp validation in payload apply (feature `rns`).
- Propagated decode transient-id alignment (stamp-aware hashing for PN payloads).
- Invalid propagation stamps apply throttle/backoff on peers (Python-style penalty).
- Peering/propagation validation defaults auto-configure from local identity + standard costs.
- Golden peer-sync Offer vector matches Python umsgpack encoding.
- Delivered-message cache (in-memory) for duplicate filtering by message_id.
- Peer table persistence (file-backed) for peer state restore.
- Minimal peer management: in-memory peer table with state transitions, backoff, and rotation.
- Propagation store time-based cleanup helper.
- LxmfRouter facade that composes runtime + peers.
- Facade exposes PN directory, propagation store, and peer-sync helper methods.
- Facade supports FileMessageStore-backed persistence for outbound/inbound/failed queues.
  - File-backed persistence for propagation entries and PN directory metadata.
- RNS module split into router/peer/storage/transport/pn submodules with public re-exports.
- RNS smoke test and MessageStore contract tests for enqueue/remove/fail flows.
- Propagated decode transient_id now hashes full LXMF payload (no length heuristic).
- Transient ID parity for propagation:
  - `decode_packet` returns `transient_id = full_hash(lxm_data)`.
  - PN stamp validation derives `transient_id` from unstamped bytes, while storing stamped bytes.
  - Peer-sync payload apply recomputes `transient_id` via PN validation and acks/removes by that ID.
- Cache marks parity:
  - Inbound `process_inbound_message` records `local_deliveries` (message_id).
  - Propagation intake/peer-sync payload apply record `locally_processed` and persist caches.
- Dedup parity using persisted caches:
  - Inbound drops duplicates when `local_deliveries` already contains the message_id (prevents replays after restart).
  - Propagation intake/peer-sync drops duplicates when `locally_processed` contains the transient_id.
- Transport contract tests cover decode_packet for direct/propagated and dedupe behavior.
- Deferred outbound stamp queue with explicit processing hook.
- Ticket store lifecycle (generate/remember/clean) and inbound stamp validation with tickets.
- Outbound/inbound stamp cost maps with helpers.
- Ticket store persistence (available_tickets) and outbound stamp-cost persistence using msgpack maps with float seconds timestamps (int timestamps accepted on load).
- Local delivery/processed caches persisted as msgpack map<bytes,f64 seconds>, int timestamps accepted on load; cleaned at startup + periodic (MESSAGE_EXPIRY*6).
- Propagation store cleanup parity:
  - Expiry at `MESSAGE_EXPIRY` seconds, invalid/zero timestamps purged.
  - Weighted eviction when size limit exceeded: age/size/priority (4-day age weight, 0.1 priority weight).
  - Runs at startup and on ~8-minute cadence via `store_interval_ms`.
- Propagation store persistence parity:
  - On-disk format uses raw stamped bytes with filename `<transient_hex>_<received_ts>_<stamp_value>`.
  - Load validates filename strictness and stamp_value parity; invalid filenames are purged.
  - Filenames are tracked in-memory to ensure deletion does not depend on float formatting.
- Node stats parity:
  - `node_stats` stored as msgpack dict with counters `client_propagation_messages_received`,
    `client_propagation_messages_served`, `unpeered_propagation_incoming`, `unpeered_propagation_rx_bytes`.
  - Loaded at startup when persistence root is set; saved on drop (shutdown-equivalent).
  - Counters increment on propagation intake (client) and when serving propagation payloads.
  - Client propagation increments can be skipped by callers when the ingress path is not a client flow.
  - `unpeered_*` counters increment when propagation ingress supplies a source hash that is not peered.
  - Propagation ingress now carries optional `source_hash` (via `receive_packet_with_source`) for this purpose.
  - `count_client_receive` takes precedence over `source_hash` to mirror packet-vs-resource semantics.
  - Link event extraction treats missing/zero peer identity as `None` to match Python’s `remote_identity` handling.
  - Reticulum link events expose peer identity via a getter (no public field), enabling auto-filled `source_hash`.
  - Requires the local `reticulum-patched` override (kaonic feature gated).
- Basic field constants from `LXMF.py`.
- Golden tests for core message format and ticket-stamp payload against Python bytes.

## Missing vs Python LXMessage / LXMRouter
- Stamps/PoW:
  - Multi-process generation and performance tuning.
  - Stamp cancellation helpers (cancel work) are not exposed yet.
- Tickets:
  - Ticket lifecycle handling beyond stamp derivation.
- Delivery methods and representations:
  - Full delivery pipeline with link/resource handling, receipts, retries across sessions, and path selection.
- Propagation:
  - No RNS encryption/ratchet integration; caller must supply encrypted payload bytes.
  - Propagation stamps not wired to RNS transport beyond peer-sync payload validation.
- RNS integration gaps:
  - Requires `reticulum` crate and `protoc` to build (feature-gated).
  - Node-based runtime is experimental and not exercised without `protoc` + `RNS_E2E=1`.
  - Link lifecycle, path selection, peer management, and persistence are still partial.
  - Control-plane peer metrics are mostly placeholders (rx/tx bytes, speeds, last_heard).
  - "No identity" control errors still unimplemented (requires transport to expose identity-less requests).
- Paper/URI/QR:
  - No RNS encryption integration; caller must supply encrypted payload bytes.
  - QR helper implemented (text render only).
- PN helpers:
  - PN directory list/get/ack uses lightweight request/response packets; no full request lifecycle yet.
- Containers/storage:
  - `unpack_from_file`, container metadata fields beyond the basic wrapper.
- Ticket persistence uses msgpack maps in `available_tickets` and `outbound_stamp_costs` (Python-compatible shape, float timestamps).
- Deferred stamp processing runs inline via queue flushing (no background multiprocess).
- Inbound ticket remember requires caller-supplied `signature_validated` flag.
- Ticket delivery timestamps must be updated by caller (`mark_ticket_delivered`).
- Ticket/stamp-cost cleanup is performed at startup (not periodic), matching Python.
- Propagated decode returns transient_id as full hash of payload; validated PN stamps compute ID from unstamped bytes.

## Persistence coverage
- File-backed: outbound/inbound/failed queues, propagation entries, PN directory metadata.
- In-memory only: peer sync state, request manager state, inflight state.

## Planned next slices (priority order)
1) Reticulum request/receipt/resource primitives (correlation IDs, timeouts, resource callbacks).
2) Router daemon job loop + scheduling (outbound/stamps/links/transients/storage/peer sync/rotate).
3) Peer sync + propagation node flows (LXMPeer model, offer/get/ack, propagation store cleanup).
4) Persistence parity (queues, transients, tickets, stamp costs, peers, stats, delivered IDs).
5) Ratchet/crypto enforcement policies and transport metadata.
6) CLI/runtime tooling (`lxmd`-like) with config + control output parity and lifecycle.

## Test coverage gaps
- Golden vectors exist for core, ticket-stamp, propagation, and paper/URI cases.
- No golden vectors for containers or QR.
- RNS module split covered by smoke/contract tests; full peer/propagation flow still needs e2e coverage.
