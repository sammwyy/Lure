use std::{fs, collections::HashMap};
use std::fs::File;
use std::io::prelude::*;

use serde::{Deserialize, Serialize};

// Listener
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub bind: String,
    pub max_connections: usize,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:25577".to_string(),
            max_connections: 8196,
        }
    }
}

// Proxy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub compression_threshold: u32,
    pub max_players: i32,
    pub online_mode: bool,
    pub player_forward_mode: String,
    pub player_limit: i32,
    pub prevent_proxy_connections: bool,
    pub motd: String,
    pub favicon: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            compression_threshold: 256,
            max_players: 4000,
            online_mode: true,
            player_forward_mode: "none".to_string(),
            player_limit: -1,
            prevent_proxy_connections: false,
            motd: "§dAnother Lure proxy".to_string(),
            favicon: "server-icon.png".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LureConfig {
    #[serde(default)]
    pub listener: ListenerConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default = "LureConfig::default_hosts")]
    pub hosts: HashMap<String, String>,
    #[serde(default = "LureConfig::default_servers")]
    pub servers: HashMap<String, String>,
    #[serde(flatten)]
    pub other_fields: HashMap<String, toml::value::Value>,
}

impl Default for LureConfig {
    fn default() -> Self {
        Self {
            listener: Default::default(),
            proxy: Default::default(),
            hosts: Self::default_hosts(),
            servers: Self::default_servers(),
            other_fields: Default::default()
        }
    }
}

impl LureConfig {
    fn default_hosts() -> HashMap<String, String> {
        let mut hosts = HashMap::new();
        hosts.insert("*".to_string(), "lobby".into());
        hosts
    }

    fn default_servers() -> HashMap<String, String> {
        let mut servers = HashMap::new();
        servers.insert("lobby".to_string(), "127.0.0.1:25565".into());
        servers
    }

    pub fn load (path: &str) -> anyhow::Result<Self, LureConfigLoadError> {
        let raw = fs::read_to_string(path).map_err(|err| LureConfigLoadError::Io(err))?;
        let config: Self = toml::from_str(&raw).map_err(|err| LureConfigLoadError::Parse(err))?;

        for field in &config.other_fields {
            println!("Unknown configuration '{}' with value {:?}", field.0, field.1);
        }

        Ok(config)
    }

    pub fn save (&self, path: &str) -> anyhow::Result<()>{
        let config_str = toml::to_string(&self)?;
        let mut file = File::create(path)?;
        file.write_all(config_str.as_bytes())?;
        Ok(())
    }
}

pub enum LureConfigLoadError {
    Io(std::io::Error),
    Parse(toml::de::Error)
}