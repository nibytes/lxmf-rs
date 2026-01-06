mod config;

use clap::{ArgAction, Parser};
use config::DaemonConfig;
use log::LevelFilter;
use lxmf_rs::{
    decode_response_bytes, DeliveryOutput, FileMessageStore, LxmfRouter, QueuedDelivery,
    RnsNodeRouter, RnsOutbound, RnsRequest, RnsRequestManager, RuntimeConfig, Value,
};
use rand_core::OsRng;
use reticulum::hash::AddressHash;
use reticulum::identity::PrivateIdentity;
use reticulum::request::RequestStatus;
use reticulum::error::RnsError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:4242";
const DEFAULT_FORWARD_ADDR: &str = "127.0.0.1:4242";
const DEFAULT_TICK_INTERVAL_MS: u64 = 1000;
const DEFAULT_INBOUND_DIR: &str = "messages";
const CONTROL_STATS_PATH: &str = "/pn/get/stats";
const CONTROL_SYNC_PATH: &str = "/pn/peer/sync";
const CONTROL_UNPEER_PATH: &str = "/pn/peer/unpeer";
const DEFAULT_CONFIG_FILE: &str = "config";
const DEFAULT_CONFIG_DIR_NAME: &str = ".lxmd";
const DEFAULT_CONFIG_DIR_ALT: &str = ".config/lxmd";
const DEFAULT_CONFIG_SYSTEM: &str = "/etc/lxmd";
const MIN_TRANSFER_LIMIT_KB: f64 = 0.38;
const MIN_MESSAGE_STORE_MB: f64 = 0.005;

struct ConfigPaths {
    config_dir: PathBuf,
    #[allow(dead_code)]
    config_path: PathBuf,
    storage_dir: PathBuf,
    identity_path: PathBuf,
    inbound_dir: PathBuf,
}

fn load_or_create_identity(path: &Path) -> Result<PrivateIdentity, std::io::Error> {
    if path.exists() {
        let data = std::fs::read_to_string(path)?;
        let trimmed = data.trim();
        let identity = PrivateIdentity::new_from_hex_string(trimmed).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("identity decode: {err:?}"))
        })?;
        return Ok(identity);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let identity = PrivateIdentity::new_from_rand(OsRng);
    std::fs::write(path, identity.to_hex_string())?;
    Ok(identity)
}

fn default_config_dir() -> PathBuf {
    let system = PathBuf::from(DEFAULT_CONFIG_SYSTEM);
    if system.join(DEFAULT_CONFIG_FILE).exists() {
        return system;
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "".to_string());
    if home.is_empty() {
        return PathBuf::from(DEFAULT_CONFIG_DIR_NAME);
    }

    let alt = PathBuf::from(&home).join(DEFAULT_CONFIG_DIR_ALT);
    if alt.join(DEFAULT_CONFIG_FILE).exists() {
        return alt;
    }

    PathBuf::from(&home).join(DEFAULT_CONFIG_DIR_NAME)
}

fn effective_log_level(config: &DaemonConfig, args: &Args) -> u8 {
    let base = config
        .logging
        .as_ref()
        .and_then(|cfg| cfg.loglevel)
        .unwrap_or(3) as i32;
    (base + args.verbose as i32 - args.quiet as i32).max(0) as u8
}

fn init_logging(level: u8) {
    let filter = match level {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let mut builder = env_logger::Builder::new();
    builder.filter_level(filter);
    let _ = builder.try_init();
}

fn resolve_config_location(config_arg: Option<PathBuf>) -> Result<(PathBuf, PathBuf), std::io::Error> {
    if let Some(path) = config_arg {
        if path.exists() {
            if path.is_dir() {
                let config_path = path.join(DEFAULT_CONFIG_FILE);
                return Ok((path, config_path));
            }
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            return Ok((dir, path));
        }

        if path
            .file_name()
            .map(|name| name == DEFAULT_CONFIG_FILE)
            .unwrap_or(false)
        {
            let dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            return Ok((dir, path));
        }

        let config_path = path.join(DEFAULT_CONFIG_FILE);
        return Ok((path, config_path));
    }

    let dir = default_config_dir();
    let path = dir.join(DEFAULT_CONFIG_FILE);
    Ok((dir, path))
}

fn ensure_default_config(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, include_str!("../lxmd.example.toml"))?;
    Ok(())
}

fn resolve_paths(config_dir: &Path, config_path: &Path, config: &DaemonConfig) -> ConfigPaths {
    let storage_dir = config
        .storage_path
        .clone()
        .unwrap_or_else(|| config_dir.join("storage"));
    let identity_path = config
        .identity_path
        .clone()
        .unwrap_or_else(|| config_dir.join("identity"));
    let inbound_dir = config
        .inbound_dir
        .clone()
        .unwrap_or_else(|| storage_dir.join(DEFAULT_INBOUND_DIR));
    ConfigPaths {
        config_dir: config_dir.to_path_buf(),
        config_path: config_path.to_path_buf(),
        storage_dir,
        identity_path,
        inbound_dir,
    }
}

fn write_inbound_message(dir: &Path, msg: &mut lxmf_rs::LXMessage) -> Option<PathBuf> {
    std::fs::create_dir_all(dir).ok()?;
    let mut name = None;

    if let Some(id) = msg.message_id {
        let mut hex = String::with_capacity(id.len() * 2);
        for byte in &id {
            hex.push_str(&format!("{:02x}", byte));
        }
        name = Some(hex);
    }

    let filename = name.unwrap_or_else(|| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("lxm-{}", now)
    });

    let path = dir.join(format!("{}.lxm", filename));
    let bytes = msg.encode_with_signature().ok()?;
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

#[derive(Parser, Debug)]
#[command(name = "lxmd-rs", about = "Minimal LXMF daemon skeleton")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    rnsconfig: Option<PathBuf>,

    #[arg(short = 'p', long, action = ArgAction::SetTrue)]
    propagation_node: bool,

    #[arg(short = 'i', long, value_name = "PATH")]
    on_inbound: Option<String>,

    #[arg(short = 'v', long, action = ArgAction::Count)]
    verbose: u8,

    #[arg(short = 'q', long, action = ArgAction::Count)]
    quiet: u8,

    #[arg(short = 's', long, action = ArgAction::SetTrue)]
    service: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    status: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    peers: bool,

    #[arg(long)]
    sync: Option<String>,

    #[arg(short = 'b', long = "break")]
    unpeer: Option<String>,

    #[arg(long)]
    timeout: Option<f64>,

    #[arg(short = 'r', long)]
    remote: Option<String>,

    #[arg(long)]
    identity: Option<PathBuf>,

    #[arg(long, action = ArgAction::SetTrue)]
    exampleconfig: bool,
}

fn read_config(path: &Path) -> Result<DaemonConfig, Box<dyn std::error::Error>> {
    ensure_default_config(path)?;
    let config_text = std::fs::read_to_string(path)?;
    let config: DaemonConfig = toml::from_str(&config_text)?;
    Ok(config)
}

fn apply_router_config(router: &mut LxmfRouter, config: &DaemonConfig, now_ms: u64) {
    let lxmf_cfg = config.lxmf.as_ref();
    let propagation_cfg = config.propagation.as_ref();

    let delivery_limit = lxmf_cfg
        .and_then(|cfg| cfg.delivery_transfer_max_accepted_size)
        .unwrap_or(1000.0)
        .max(MIN_TRANSFER_LIMIT_KB);
    let propagation_limit = propagation_cfg
        .and_then(|cfg| cfg.propagation_message_max_accepted_size)
        .or_else(|| propagation_cfg.and_then(|cfg| cfg.propagation_transfer_max_accepted_size))
        .unwrap_or(256.0)
        .max(MIN_TRANSFER_LIMIT_KB);
    let sync_limit = propagation_cfg
        .and_then(|cfg| cfg.propagation_sync_max_accepted_size)
        .unwrap_or(256.0 * 40.0)
        .max(MIN_TRANSFER_LIMIT_KB);
    router.set_transfer_limits_kb(delivery_limit, propagation_limit, sync_limit);

    let storage_limit_mb = propagation_cfg
        .and_then(|cfg| cfg.message_storage_limit)
        .unwrap_or(500.0)
        .max(MIN_MESSAGE_STORE_MB);
    router.set_message_storage_limit(Some((storage_limit_mb * 1000.0) as u64), None, None);

    if let Some(propagation_cfg) = propagation_cfg {
        let target_cost = propagation_cfg.propagation_stamp_cost_target.unwrap_or(16);
        let flex = propagation_cfg.propagation_stamp_cost_flexibility.unwrap_or(3);
        router.set_propagation_validation(target_cost, flex);

        let peering_cost = propagation_cfg.peering_cost.unwrap_or(18);
        let local_hash = router.runtime().router().local_identity_hash();
        router.set_peering_validation(local_hash, peering_cost);
        router.set_max_peering_cost(propagation_cfg.remote_peering_cost_max.unwrap_or(26));

        router.set_autopeer_maxdepth(propagation_cfg.autopeer_maxdepth);
        router.set_max_peers(propagation_cfg.max_peers);
        router.set_from_static_only(propagation_cfg.from_static_only.unwrap_or(false));

        router.set_authentication(propagation_cfg.auth_required.unwrap_or(false));

        let static_peers = parse_hash_list(propagation_cfg.static_peers.as_ref());
        if !static_peers.is_empty() {
            router.set_static_peers(static_peers);
        }

        let mut allowed = parse_hash_list(propagation_cfg.control_allowed.as_ref());
        let local_hash = router.runtime().router().local_identity_hash();
        if !allowed.contains(&local_hash) {
            allowed.push(local_hash);
        }
        router.set_control_allowed(allowed);

        if let Some(prioritised) = propagation_cfg.prioritise_destinations.as_ref() {
            for hex in prioritised {
                if let Ok(addr) = AddressHash::new_from_hex_string(hex) {
                    let mut bytes = [0u8; 16];
                    bytes.copy_from_slice(addr.as_slice());
                    router.prioritise_destination(bytes);
                }
            }
        }
    }

    router.enable_propagation_node(
        propagation_cfg
            .and_then(|cfg| cfg.enable_node)
            .unwrap_or(false),
        now_ms,
    );
}

fn build_router(identity: PrivateIdentity, config: &DaemonConfig, paths: &ConfigPaths) -> LxmfRouter {
    let store = FileMessageStore::new(paths.storage_dir.clone());
    let outbound = RnsOutbound::new(identity.clone(), identity.as_identity().clone());
    let mut router = LxmfRouter::with_store(outbound, Box::new(store), RuntimeConfig::default());
    apply_router_config(&mut router, config, SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64);
    router
}

#[cfg(test)]
async fn dispatch_deliveries_with<F, Fut, E>(
    deliveries: Vec<QueuedDelivery>,
    mut send: F,
) -> usize
where
    F: FnMut(QueuedDelivery) -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
{
    let mut sent = 0usize;
    for delivery in deliveries {
        if send(delivery).await.is_ok() {
            sent = sent.saturating_add(1);
        }
    }
    sent
}

async fn dispatch_deliveries(
    node_router: &mut lxmf_rs::RnsNodeRouter,
    deliveries: Vec<QueuedDelivery>,
) -> usize {
    let mut sent = 0usize;
    for delivery in deliveries {
        let result = match delivery.delivery {
            DeliveryOutput::Packet(packet) => {
                node_router.transport.send_packet(packet).await;
                Ok(())
            }
            DeliveryOutput::Resource {
                destination,
                data,
                context: _,
            } => node_router
                .send_resource_payload(destination, &data)
                .await
                .map_err(map_lxmf_error),
            DeliveryOutput::PaperUri(uri) => {
                println!("paper delivery: {uri}");
                Ok(())
            }
        };
        if result.is_ok() {
            sent = sent.saturating_add(1);
        }
    }
    sent
}

fn format_status(router: &LxmfRouter, now_ms: u64, show_status: bool, show_peers: bool) -> String {
    let stats = router.compile_stats(now_ms);
    format_stats_output(&stats, show_status, show_peers)
}

fn map_reticulum_error(err: RnsError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("reticulum error: {err:?}"))
}

fn map_lxmf_error(err: lxmf_rs::RnsError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, format!("lxmf error: {err:?}"))
}

fn parse_destination_bytes(hex: &str) -> Result<[u8; 16], std::io::Error> {
    let address = AddressHash::new_from_hex_string(hex).map_err(map_reticulum_error)?;
    let mut out = [0u8; 16];
    let bytes = address.as_slice();
    out.copy_from_slice(bytes);
    Ok(out)
}

fn parse_hash_list(list: Option<&Vec<String>>) -> Vec<[u8; 16]> {
    let mut out = Vec::new();
    let Some(values) = list else {
        return out;
    };
    for item in values {
        if let Ok(address) = AddressHash::new_from_hex_string(item) {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(address.as_slice());
            out.push(bytes);
        }
    }
    out
}

fn read_hash_file(path: &Path) -> Vec<[u8; 16]> {
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Ok(address) = AddressHash::new_from_hex_string(trimmed) {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(address.as_slice());
            out.push(bytes);
        }
    }
    out
}

fn extract_map_value<'a>(map: &'a Vec<(Value, Value)>, key: &str) -> Option<&'a Value> {
    map.iter().find_map(|(k, v)| match k {
        Value::String(s) => s.as_str().filter(|name| *name == key).map(|_| v),
        Value::Binary(b) => std::str::from_utf8(b)
            .ok()
            .filter(|name| *name == key)
            .map(|_| v),
        _ => None,
    })
}

fn pretty_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn prettysize(bytes: f64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes;
    let mut idx = 0usize;
    while size >= 1000.0 && idx + 1 < units.len() {
        size /= 1000.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{} {}", size as u64, units[idx])
    } else {
        format!("{:.2} {}", size, units[idx])
    }
}

fn prettytime(seconds: f64) -> String {
    let mut secs = seconds.max(0.0) as u64;
    let days = secs / 86_400;
    secs %= 86_400;
    let hours = secs / 3_600;
    secs %= 3_600;
    let mins = secs / 60;
    secs %= 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::F64(v) => Some(*v),
        Value::F32(v) => Some(*v as f64),
        Value::Integer(v) => v.as_i64().map(|v| v as f64),
        _ => None,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Integer(v) => v.as_i64(),
        _ => None,
    }
}

fn format_stats_output(stats: &Value, show_status: bool, show_peers: bool) -> String {
    let Value::Map(map) = stats else {
        return format!("stats: {stats:?}\n");
    };

    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let dest_hash = extract_map_value(map, "destination_hash")
        .and_then(|v| match v {
            Value::Binary(bytes) => Some(pretty_hex(bytes)),
            _ => None,
        })
        .unwrap_or_else(|| "unknown".to_string());
    let uptime = extract_map_value(map, "uptime")
        .and_then(value_as_f64)
        .unwrap_or(0.0);

    let mut output = String::new();
    output.push_str(&format!(
        "\nLXMF Propagation Node running on {}, uptime is {}\n",
        dest_hash,
        prettytime(uptime)
    ));

    if show_status {
        if let Some(Value::Map(store)) = extract_map_value(map, "messagestore") {
            let count = extract_map_value(store, "count")
                .and_then(value_as_i64)
                .unwrap_or(0);
            let bytes = extract_map_value(store, "bytes")
                .and_then(value_as_f64)
                .unwrap_or(0.0);
            let limit = extract_map_value(store, "limit")
                .and_then(value_as_f64)
                .unwrap_or(0.0);
            let util = if limit > 0.0 { (bytes / limit) * 100.0 } else { 0.0 };
            output.push_str(&format!(
                "Messagestore contains {} messages, {} ({:.2}% utilised of {})\n",
                count,
                prettysize(bytes),
                util,
                prettysize(limit)
            ));
        }

        let stamp_cost = extract_map_value(map, "target_stamp_cost")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let stamp_flex = extract_map_value(map, "stamp_cost_flexibility")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let peering_cost = extract_map_value(map, "peering_cost")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let max_peering = extract_map_value(map, "max_peering_cost")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let from_static = extract_map_value(map, "from_static_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let who_str = if from_static {
            "static peers only"
        } else {
            "all nodes"
        };
        let propagation_limit = extract_map_value(map, "propagation_limit")
            .and_then(value_as_f64)
            .unwrap_or(0.0);
        let sync_limit = extract_map_value(map, "sync_limit")
            .and_then(value_as_f64)
            .unwrap_or(0.0);

        output.push_str(&format!(
            "Required propagation stamp cost is {}, flexibility is {}\n",
            stamp_cost, stamp_flex
        ));
        output.push_str(&format!(
            "Peering cost is {}, max remote peering cost is {}\n",
            peering_cost, max_peering
        ));
        output.push_str(&format!("Accepting propagated messages from {}\n", who_str));
        output.push_str(&format!(
            "{} message limit, {} sync limit\n\n",
            prettysize(propagation_limit * 1000.0),
            prettysize(sync_limit * 1000.0)
        ));

        let total_peers = extract_map_value(map, "total_peers")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let max_peers = extract_map_value(map, "max_peers")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let static_peers = extract_map_value(map, "static_peers")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let discovered = extract_map_value(map, "discovered_peers")
            .and_then(value_as_i64)
            .unwrap_or(0);
        output.push_str(&format!(
            "Peers   : {} total (peer limit is {})\n",
            total_peers, max_peers
        ));
        output.push_str(&format!("          {} discovered, {} static\n\n", discovered, static_peers));

        let unpeered_incoming = extract_map_value(map, "unpeered_propagation_incoming")
            .and_then(value_as_i64)
            .unwrap_or(0);
        let unpeered_rx = extract_map_value(map, "unpeered_propagation_rx_bytes")
            .and_then(value_as_f64)
            .unwrap_or(0.0);
        let client_received = extract_map_value(map, "clients")
            .and_then(|v| match v {
                Value::Map(map) => extract_map_value(map, "client_propagation_messages_received")
                    .and_then(value_as_i64),
                _ => None,
            })
            .unwrap_or(0);
        let client_served = extract_map_value(map, "clients")
            .and_then(|v| match v {
                Value::Map(map) => extract_map_value(map, "client_propagation_messages_served")
                    .and_then(value_as_i64),
                _ => None,
            })
            .unwrap_or(0);
        output.push_str(&format!(
            "Traffic : {} messages received in total ({})\n",
            unpeered_incoming + client_received,
            prettysize(unpeered_rx)
        ));
        output.push_str(&format!(
            "          {} messages received from unpeered nodes ({})\n",
            unpeered_incoming,
            prettysize(unpeered_rx)
        ));
        output.push_str(&format!(
            "          {} propagation messages received directly from clients\n",
            client_received
        ));
        output.push_str(&format!(
            "          {} propagation messages served to clients\n\n",
            client_served
        ));
    }

    if show_peers {
        if let Some(Value::Map(peers)) = extract_map_value(map, "peers") {
            for (peer_id, value) in peers {
                let id_hex = match peer_id {
                    Value::Binary(bytes) => pretty_hex(bytes),
                    _ => "unknown".to_string(),
                };
                let Value::Map(peer_map) = value else {
                    continue;
                };
                let peer_type = extract_map_value(peer_map, "type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let status = extract_map_value(peer_map, "alive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let status_str = if status { "Available" } else { "Unreachable" };
                let hops = extract_map_value(peer_map, "network_distance")
                    .and_then(value_as_i64)
                    .unwrap_or(-1);
                let hops_str = if hops < 0 {
                    "hops unknown".to_string()
                } else if hops == 1 {
                    "1 hop away".to_string()
                } else {
                    format!("{hops} hops away")
                };
                let last_heard = extract_map_value(peer_map, "last_heard")
                    .and_then(value_as_f64)
                    .unwrap_or(0.0);
                let last_heard_age = (now_s - last_heard).max(0.0);
                let last_heard_str = prettytime(last_heard_age);
                let peering_cost = extract_map_value(peer_map, "peering_cost")
                    .and_then(value_as_i64)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let target_cost = extract_map_value(peer_map, "target_stamp_cost")
                    .and_then(value_as_i64)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let flex = extract_map_value(peer_map, "stamp_cost_flexibility")
                    .and_then(value_as_i64)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let peering_key = extract_map_value(peer_map, "peering_key")
                    .map(|v| match v {
                        Value::Nil => "Not generated".to_string(),
                        Value::Binary(bytes) => format!("Generated, value is {}", pretty_hex(bytes)),
                        _ => "unknown".to_string(),
                    })
                    .unwrap_or_else(|| "Not generated".to_string());
                let transfer_limit = extract_map_value(peer_map, "transfer_limit")
                    .and_then(value_as_f64)
                    .map(|v| prettysize(v * 1000.0))
                    .unwrap_or_else(|| "Unknown".to_string());
                let sync_limit = extract_map_value(peer_map, "sync_limit")
                    .and_then(value_as_f64)
                    .map(|v| prettysize(v * 1000.0))
                    .unwrap_or_else(|| "unknown".to_string());
                let (offered, outgoing, incoming, unhandled) = extract_map_value(peer_map, "messages")
                    .and_then(|v| match v {
                        Value::Map(map) => {
                            let offered = extract_map_value(map, "offered")
                                .and_then(value_as_i64)
                                .unwrap_or(0);
                            let outgoing = extract_map_value(map, "outgoing")
                                .and_then(value_as_i64)
                                .unwrap_or(0);
                            let incoming = extract_map_value(map, "incoming")
                                .and_then(value_as_i64)
                                .unwrap_or(0);
                            let unhandled = extract_map_value(map, "unhandled")
                                .and_then(value_as_i64)
                                .unwrap_or(0);
                            Some((offered, outgoing, incoming, unhandled))
                        }
                        _ => None,
                    })
                    .unwrap_or((0, 0, 0, 0));
                let acceptance_rate = extract_map_value(peer_map, "acceptance_rate")
                    .and_then(value_as_f64)
                    .unwrap_or(0.0)
                    * 100.0;
                let last_sync_attempt = extract_map_value(peer_map, "last_sync_attempt")
                    .and_then(value_as_f64)
                    .unwrap_or(0.0);
                let last_sync_str = if last_sync_attempt <= 0.0 {
                    "never synced".to_string()
                } else {
                    let age = (now_s - last_sync_attempt).max(0.0);
                    format!("last synced {} ago", prettytime(age))
                };

                output.push_str(&format!("  {} peer     {}\n", peer_type, id_hex));
                output.push_str(&format!(
                    "    Status     : {}, {}, last heard {} ago\n",
                    status_str, hops_str, last_heard_str
                ));
                output.push_str(&format!(
                    "    Costs      : Propagation {} (flex {}), peering {}\n",
                    target_cost, flex, peering_cost
                ));
                output.push_str(&format!("    Sync key   : {}\n", peering_key));
                output.push_str(&format!(
                    "    Limits     : {} message limit, {} sync limit\n",
                    transfer_limit, sync_limit
                ));
                output.push_str(&format!(
                    "    Messages   : {} offered, {} outgoing, {} incoming, {:.2}% acceptance rate\n",
                    offered, outgoing, incoming, acceptance_rate
                ));
                output.push_str(&format!("    Sync state : {} unhandled message(s), {}\n\n", unhandled, last_sync_str));
            }
        }
    }

    output
}

async fn remote_control_request(
    identity: PrivateIdentity,
    config: &DaemonConfig,
    remote: &str,
    path: &str,
    data: Value,
    timeout: Duration,
) -> Result<Value, Box<dyn std::error::Error>> {
    let bind_addr = config
        .bind_addr
        .as_deref()
        .unwrap_or(DEFAULT_BIND_ADDR);
    let forward_addr = config
        .forward_addr
        .as_deref()
        .unwrap_or(DEFAULT_FORWARD_ADDR);

    let destination_bytes = parse_destination_bytes(remote)?;

    let mut node_router = RnsNodeRouter::new_udp(
        "lxmd-remote",
        identity,
        bind_addr,
        forward_addr,
    )
    .await
    .map_err(map_lxmf_error)?;

    let mut requests = RnsRequestManager::new();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut req: RnsRequest = requests.register_request(
        destination_bytes,
        Some(path.to_string()),
        data,
        now_ms,
        5_000,
    )?;
    let receipt = node_router
        .send_request_with_receipt_to_hash(destination_bytes, &mut req, timeout)
        .await?;

    let start = Instant::now();
    loop {
        node_router.poll_inbound().await;
        let status = receipt.lock().unwrap().status;
        match status {
            RequestStatus::Ready => {
                let response_bytes = receipt
                    .lock()
                    .unwrap()
                    .response
                    .clone()
                    .ok_or("empty response")?;
                let response = decode_response_bytes(&response_bytes).map_err(map_lxmf_error)?;
                return Ok(response.data);
            }
            RequestStatus::Failed => return Err("remote request failed".into()),
            _ => {}
        }

        if start.elapsed() >= timeout {
            return Err("remote stats timed out".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn remote_stats(
    identity: PrivateIdentity,
    config: &DaemonConfig,
    remote: &str,
    timeout: Duration,
) -> Result<Value, Box<dyn std::error::Error>> {
    remote_control_request(
        identity,
        config,
        remote,
        CONTROL_STATS_PATH,
        Value::Nil,
        timeout,
    )
    .await
}

async fn remote_peer_control(
    identity: PrivateIdentity,
    config: &DaemonConfig,
    remote: &str,
    path: &str,
    peer_hex: &str,
    timeout: Duration,
) -> Result<Value, Box<dyn std::error::Error>> {
    let target = parse_destination_bytes(peer_hex)?;
    remote_control_request(
        identity,
        config,
        remote,
        path,
        Value::Binary(target.to_vec()),
        timeout,
    )
    .await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.exampleconfig {
        print!("{}", include_str!("../lxmd.example.toml"));
        return Ok(());
    }

    let (config_dir, config_path) = resolve_config_location(args.config.clone())?;
    let config = read_config(&config_path)?;
    let paths = resolve_paths(&config_dir, &config_path, &config);
    let log_level = effective_log_level(&config, &args);
    init_logging(log_level);
    if args.rnsconfig.is_some() {
        log::warn!("--rnsconfig is not used by reticulum-rs; using bind/forward from config");
    }

    let bind_addr = config
        .bind_addr
        .as_deref()
        .unwrap_or(DEFAULT_BIND_ADDR);
    let forward_addr = config
        .forward_addr
        .as_deref()
        .unwrap_or(DEFAULT_FORWARD_ADDR);
    let tick_interval_ms = config
        .tick_interval_ms
        .unwrap_or(DEFAULT_TICK_INTERVAL_MS);
    let node_name = config
        .node_name
        .clone()
        .or_else(|| config.propagation.as_ref().and_then(|cfg| cfg.node_name.clone()))
        .unwrap_or_else(|| "lxmd-rs".to_string());

    let identity_path = args.identity.clone().unwrap_or(paths.identity_path.clone());
    let identity = load_or_create_identity(&identity_path)?;

    if args.status || args.peers {
        let timeout = args.timeout.unwrap_or(5.0);
        if let Some(remote) = args.remote.as_deref() {
            let stats = remote_stats(identity, &config, remote, Duration::from_secs_f64(timeout)).await?;
            print!("{}", format_stats_output(&stats, args.status, args.peers));
        } else {
            let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
            let router = build_router(identity, &config, &paths);
            print!("{}", format_status(&router, now_ms, args.status, args.peers));
        }
        return Ok(());
    }

    if let Some(target) = args.sync.as_deref() {
        let timeout = args.timeout.unwrap_or(10.0);
        if let Some(remote) = args.remote.as_deref() {
            let response = remote_peer_control(
                identity,
                &config,
                remote,
                CONTROL_SYNC_PATH,
                target,
                Duration::from_secs_f64(timeout),
            )
            .await?;
            println!("sync response: {response:?}");
        } else {
            let target_bytes = parse_destination_bytes(target)?;
            let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
            let mut router = build_router(identity, &config, &paths);
            if router.request_peer_sync(target_bytes, now_ms) {
                println!("Sync requested for peer {target}");
            } else {
                println!("The requested peer was not found");
            }
        }
        return Ok(());
    }

    if let Some(target) = args.unpeer.as_deref() {
        let timeout = args.timeout.unwrap_or(10.0);
        if let Some(remote) = args.remote.as_deref() {
            let response = remote_peer_control(
                identity,
                &config,
                remote,
                CONTROL_UNPEER_PATH,
                target,
                Duration::from_secs_f64(timeout),
            )
            .await?;
            println!("unpeer response: {response:?}");
        } else {
            let target_bytes = parse_destination_bytes(target)?;
            let mut router = build_router(identity, &config, &paths);
            if router.unpeer(target_bytes) {
                println!("Broke peering with {target}");
            } else {
                println!("The requested peer was not found");
            }
        }
        return Ok(());
    }

    let mut node_router = lxmf_rs::RnsNodeRouter::new_udp(
        node_name,
        identity.clone(),
        &bind_addr,
        &forward_addr,
    )
    .await?;

    let mut router = build_router(identity.clone(), &config, &paths);
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let enable_pn = args.propagation_node
        || config
            .propagation
            .as_ref()
            .and_then(|cfg| cfg.enable_node)
            .unwrap_or(false);
    router.enable_propagation_node(enable_pn, now_ms);

    let ignored_path = paths.config_dir.join("ignored");
    let ignored = read_hash_file(&ignored_path);
    for hash in ignored {
        router.ignore_destination(hash);
    }

    let allowed_path = paths.config_dir.join("allowed");
    if router.auth_required() {
        let allowed = read_hash_file(&allowed_path);
        if !allowed.is_empty() {
            router.set_allowed_identities(allowed);
        }
    }

    let tick_interval = Duration::from_millis(tick_interval_ms.max(1));
    loop {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let tick = router.tick(now_ms);
        let peer_requests = router.dispatch_peer_sync_requests(&tick, now_ms);
        let _ = dispatch_deliveries(&mut node_router, tick.deliveries).await;
        for (mut request, peer_id) in peer_requests {
            let _ = node_router.send_request_to_hash(peer_id, &mut request).await;
        }
        for received in node_router.poll_inbound().await {
            router.receive_inbound(received);
        }
        let inbound_dir = paths.inbound_dir.clone();
        let on_inbound = args
            .on_inbound
            .clone()
            .or_else(|| config.lxmf.as_ref().and_then(|cfg| cfg.on_inbound.clone()));
        for mut msg in router.process_inbound_queue(now_ms, false) {
            let path = write_inbound_message(&inbound_dir, &mut msg);
            if let Some(path) = path {
                println!("inbound lxmf written to {}", path.display());
                if let Some(command) = on_inbound.as_ref() {
                    let _ = Command::new("sh")
                        .arg("-c")
                        .arg(format!("{} \"{}\"", command, path.display()))
                        .output();
                }
            } else {
                println!("inbound lxmf received (no file written)");
            }
        }
        while let Some((peer_id, response)) = router.pop_control_response() {
            if peer_id.iter().all(|byte| *byte == 0) {
                continue;
            }
            let _ = node_router.send_response_to_hash(peer_id, &response).await;
        }
        while let Some((peer_id, response)) = router.pop_peer_sync_response() {
            let _ = node_router.send_response_to_hash(peer_id, &response).await;
        }
        while let Some((peer_id, mut request)) = router.pop_peer_sync_outbound() {
            let _ = node_router.send_request_to_hash(peer_id, &mut request).await;
        }
        tokio::time::sleep(tick_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lxmf_rs::{DeliveryMode, LXMessage};
    use lxmf_rs::RnsRouter;

    #[test]
    fn parses_status_flag() {
        let args = Args::try_parse_from(["lxmd-rs", "--status"]).expect("args");
        assert!(args.status);
    }

    #[test]
    fn parses_peers_flag_with_remote() {
        let args = Args::try_parse_from(["lxmd-rs", "--peers", "--remote", "abcd"]).expect("args");
        assert!(args.peers);
        assert_eq!(args.remote, Some("abcd".to_string()));
    }

    #[test]
    fn format_status_includes_counts() {
        let identity = PrivateIdentity::new_from_name("status");
        let outbound = RnsOutbound::new(identity.clone(), identity.as_identity().clone());
        let router = LxmfRouter::with_config(RnsRouter::new(outbound), RuntimeConfig::default());
        let output = format_status(&router, 1_000, true, false);
        assert!(output.contains("Messagestore"));
        assert!(output.contains("Peers"));
    }

    #[tokio::test]
    async fn dispatch_deliveries_handles_tick_output() {
        let source = PrivateIdentity::new_from_name("dispatch-source");
        let destination = PrivateIdentity::new_from_name("dispatch-dest")
            .as_identity()
            .clone();
        let outbound = RnsOutbound::new(source, destination);
        let mut router = LxmfRouter::with_config(RnsRouter::new(outbound), RuntimeConfig::default());

        let msg = LXMessage::with_strings(
            [1u8; 16],
            [2u8; 16],
            1_700_000_000.0,
            "hi",
            "there",
            Value::Map(vec![]),
        );
        router.enqueue(msg, DeliveryMode::Direct);

        let tick = router.tick(1_000);
        assert_eq!(tick.deliveries.len(), 1);

        let sent = dispatch_deliveries_with(tick.deliveries, |_delivery| async move { Ok::<_, ()>(()) }).await;
        assert_eq!(sent, 1);
    }

    #[test]
    fn resolves_config_location_for_file() {
        let path = PathBuf::from("/tmp/lxmd/config");
        let (dir, cfg) = resolve_config_location(Some(path.clone())).expect("paths");
        assert_eq!(cfg, path);
        assert_eq!(dir, PathBuf::from("/tmp/lxmd"));
    }

    #[test]
    fn resolves_config_location_for_dir() {
        let path = std::env::temp_dir().join(format!("lxmd-rs-dir-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&path);
        let (dir, cfg) = resolve_config_location(Some(path.clone())).expect("paths");
        assert_eq!(dir, path);
        assert_eq!(cfg, dir.join("config"));
    }

    #[test]
    fn resolves_config_location_for_missing_dir() {
        let path = std::env::temp_dir().join(format!("lxmd-rs-missing-{}", std::process::id()));
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }
        let (dir, cfg) = resolve_config_location(Some(path.clone())).expect("paths");
        assert_eq!(dir, path);
        assert_eq!(cfg, dir.join("config"));
    }
}
