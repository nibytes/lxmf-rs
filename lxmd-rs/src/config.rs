use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct LxmfConfig {
    pub display_name: Option<String>,
    pub announce_at_start: Option<bool>,
    pub announce_interval: Option<u64>,
    pub delivery_transfer_max_accepted_size: Option<f64>,
    pub on_inbound: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PropagationConfig {
    pub enable_node: Option<bool>,
    pub node_name: Option<String>,
    pub auth_required: Option<bool>,
    pub announce_at_start: Option<bool>,
    pub autopeer: Option<bool>,
    pub autopeer_maxdepth: Option<u32>,
    pub announce_interval: Option<u64>,
    pub message_storage_limit: Option<f64>,
    pub propagation_transfer_max_accepted_size: Option<f64>,
    pub propagation_message_max_accepted_size: Option<f64>,
    pub propagation_sync_max_accepted_size: Option<f64>,
    pub propagation_stamp_cost_target: Option<u32>,
    pub propagation_stamp_cost_flexibility: Option<u32>,
    pub peering_cost: Option<u32>,
    pub remote_peering_cost_max: Option<u32>,
    pub prioritise_destinations: Option<Vec<String>>,
    pub control_allowed: Option<Vec<String>>,
    pub static_peers: Option<Vec<String>>,
    pub max_peers: Option<u32>,
    pub from_static_only: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub loglevel: Option<u8>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DaemonConfig {
    pub storage_path: Option<PathBuf>,
    pub identity_path: Option<PathBuf>,
    pub inbound_dir: Option<PathBuf>,
    pub bind_addr: Option<String>,
    pub forward_addr: Option<String>,
    pub tick_interval_ms: Option<u64>,
    pub node_name: Option<String>,
    pub lxmf: Option<LxmfConfig>,
    pub propagation: Option<PropagationConfig>,
    pub logging: Option<LoggingConfig>,
}

#[cfg(test)]
mod tests {
    use super::DaemonConfig;

    #[test]
    fn parses_minimal_config() {
        let cfg: DaemonConfig = toml::from_str(
            "storage_path = \"/tmp/lxmd\"\n"
                .to_string()
                + "identity_path = \"/tmp/lxmd/identity\"\n"
                + "inbound_dir = \"/tmp/lxmd/messages\"\n\n"
                + "[logging]\nloglevel = 3\n\n"
                + "[lxmf]\n"
                + "display_name = \"Example\"\n"
                + "announce_at_start = true\n",
        )
        .expect("config");

        assert_eq!(cfg.storage_path.unwrap().to_string_lossy(), "/tmp/lxmd");
        assert_eq!(cfg.identity_path.unwrap().to_string_lossy(), "/tmp/lxmd/identity");
        assert_eq!(cfg.inbound_dir.unwrap().to_string_lossy(), "/tmp/lxmd/messages");
        assert_eq!(cfg.logging.unwrap().loglevel, Some(3));
        assert!(cfg.bind_addr.is_none());
    }
}
