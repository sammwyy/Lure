use std::error::Error;
use std::{env, path::PathBuf};

use config::{read_config_from_file, save_config_to_file, LureConfig};
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
            hosts: config::default_hosts(),
            servers: config::default_servers(),
        });

    println!("{}", config_file.to_str().unwrap());
    if !config_file.exists() {
        save_config_to_file(config.clone(), config_file.to_str().unwrap());
    }

    let lure = Lure::new(config);
    lure.start().await?;
    Ok(())
}
