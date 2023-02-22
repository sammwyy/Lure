use std::error::Error;
use std::{env, path::PathBuf};

use config::{read_config_from_file, LureConfig};
use lure::Lure;

mod config;
mod connection;
mod lure;

fn get_current_working_dir() -> std::io::Result<PathBuf> {
    env::current_dir()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let root = get_current_working_dir().unwrap();
    let config_file = root.join("settings.toml");
    let config: LureConfig =
        read_config_from_file(config_file.to_str().unwrap()).unwrap_or(LureConfig {
            listener: config::default_listener(),
            proxy: config::default_proxy(),
            servers: config::default_servers(),
        });
    let lure = Lure::new(config);
    lure.start().await?;
    Ok(())
}
