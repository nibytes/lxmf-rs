# lxmd-rs

Minimal LXMF daemon skeleton built on top of `lxmf-rs`. This is intentionally
small: it loads a config, initializes Reticulum + `LxmfRouter` with a
`FileMessageStore`, and runs a simple tick loop.

## Commands

- Default (no subcommand): run daemon loop.
- `status`: initialize config and router (no network) and print queue/peer/PN counts.
- `status --remote <dest>`: send a minimal control stats request over Reticulum and print a brief summary.
- `peers --remote <dest>`: same request, focused on peer counts.

Remote queries expect the control destination hash (hex) of a propagation node.

## Config

Example `lxmd.toml`:

```toml
storage_path = "/var/lib/lxmd"
log_level = 3
identity_path = "/var/lib/lxmd/identity"
inbound_dir = "/var/lib/lxmd/messages"
bind_addr = "127.0.0.1:4242"
forward_addr = "127.0.0.1:4242"
tick_interval_ms = 1000
node_name = "lxmd-rs"
```

`identity_path` defaults to `${storage_path}/identity` and `inbound_dir`
defaults to `${storage_path}/messages` if not set.
