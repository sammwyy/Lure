use std::fs;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Listener
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub bind: String,
    pub max_connections: usize,
}

pub fn default_listener() -> ListenerConfig {
    ListenerConfig {
        bind: "127.0.0.1:25577".to_string(),
        max_connections: 8196,
    }
}

// Proxy
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub compression_threshold: u32,
    pub max_players: i32,
    pub online_mode: bool,
    pub player_limit: i32,
    pub prevent_proxy_connections: bool,
    pub motd: String,
}

pub fn default_proxy() -> ProxyConfig {
    ProxyConfig {
        compression_threshold: 256,
        max_players: 4000,
        online_mode: true,
        player_limit: -1,
        prevent_proxy_connections: false,
        motd: "§dAnother Lure proxy".to_string(),
    }
}

// Servers
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyServer {
    pub name: String,
    pub address: String,
}

pub fn default_servers() -> Vec<ProxyServer> {
    let mut servers: Vec<ProxyServer> = Vec::new();
    servers.push(ProxyServer {
        name: "lobby".to_string(),
        address: "127.0.0.1:25565".to_string(),
    });
    return servers;
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LureConfig {
    #[serde(default = "default_listener")]
    pub listener: ListenerConfig,

    #[serde(default = "default_proxy")]
    pub proxy: ProxyConfig,

    #[serde(default = "default_servers")]
    pub servers: Vec<ProxyServer>,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("{0}")]
    IO(#[from] std::io::Error),
}

pub fn read_config_from_str(string: &str) -> Result<LureConfig, ConfigError> {
    let config: LureConfig = toml::from_str(string).unwrap();
    return Ok(config);
}

pub fn read_config_from_file(file: &str) -> Result<LureConfig, ConfigError> {
    let raw = fs::read_to_string(file)?;
    let config: LureConfig = read_config_from_str(&raw)?;
    return Ok(config);
}
