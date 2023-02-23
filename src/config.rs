use std::fs;
use std::fs::File;
use std::io::prelude::*;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use toml::map::Map;

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

// Hosts
pub fn default_hosts() -> Map<std::string::String, toml::Value> {
    let mut hosts = Map::new();
    hosts.insert("*".to_string(), "lobby".into());
    return hosts;
}

// Servers
pub fn default_servers() -> Map<std::string::String, toml::Value> {
    let mut servers = Map::new();
    servers.insert("lobby".to_string(), "127.0.0.1:25565".into());
    return servers;
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LureConfig {
    pub listener: ListenerConfig,
    pub proxy: ProxyConfig,
    pub hosts: Map<std::string::String, toml::Value>,
    pub servers: Map<std::string::String, toml::Value>,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("{0}")]
    IO(#[from] std::io::Error),
}

pub fn config_to_str(config: LureConfig) -> String {
    let raw = toml::to_string(&config).unwrap();
    return raw;
}

pub fn save_config_to_file(config: LureConfig, file_path: &str) -> String {
    let raw = config_to_str(config);
    let mut file = File::create(file_path).unwrap();
    file.write_all(raw.as_bytes()).unwrap();
    return raw;
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
