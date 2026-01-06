# RNS parity plan

Goal: reach practical parity with Python LXMF/LXMRouter, including daemon/runtime behaviors, while keeping tests deterministic and local where possible.

## Gap summary (remaining vs Python)
- **Router daemon loop:** job scheduling (outbound, stamps, links, transients, storage, peer sync/rotate) and signal/atexit handling.
- **Link lifecycle + requests/resources:** Reticulum request/response lifecycle, receipt correlation, link proofs, resource transfer hooks.
- **Peer management (LXMPeer):** peer model, sync strategies, backoff, offer/get/ack flows, peer persistence.
- **Propagation node management:** enable/disable node, announce handling, peer sync, propagation message store and cleanup.
- **Transport crypto/ratchets:** full ratchet enforcement and transport encryption policy integration.
- **Persistence:** unified storage of outbound/inbound queues, transients, tickets, stamp costs, peers, node stats, delivered IDs.
- **CLI/runtime tooling:** `lxmd`-like configuration, runtime wiring, and operational flags.
- **Interoperability limits:** reticulum-rs currently lacks full request/receipt/resource APIs; parity requires extending reticulum layer or stubbing compatible flows.

## Prioritized slices (ordered)
1) **Reticulum request/receipt/resource primitives**
   - Add request/response tracking (correlation IDs, timeouts), resource send/receive callbacks.
   - Tests: unit tests for request lifecycle; loopback/ignored e2e for resource transfer.

2) **Router daemon skeleton + scheduling**
   - Job loop with intervals matching Python (outbound, stamps, links, transient cleanup, storage, peer sync/rotate).
   - Tests: deterministic scheduler tick tests; state transitions with injected time.

3) **Peer sync + propagation node flows**
   - Port LXMPeer model (states, sync backoff, offer/get/ack).
   - Propagation node enable/disable, announce handling, message store + cleanup.
   - Tests: offer/get/ack wire tests; peer backoff/state transitions.

4) **Persistence layer parity**
   - Store: outbound/inbound, transients, tickets, stamp costs, peers, stats, delivered IDs.
   - Tests: restore + consistency tests; cleanup/retention tests.

5) **Ratchet/crypto policy integration**
   - Enforce ratchets, link encryption policy, transport metadata, and identity handling.
   - Tests: policy enforcement and error paths.

6) **CLI/runtime tooling parity**
   - `lxmd`-like config loader, daemon startup/shutdown behavior.
   - Tests: smoke tests with example configs.

## Test strategy
- Prefer unit tests for encoding/state transitions and deterministic scheduler steps.
- Use feature-gated integration tests for link/resource with loopback transport.
- Keep Python golden vectors for wire compatibility where possible.
