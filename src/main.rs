mod config;
mod connection;
mod keypair;
mod lure;
mod utils;

use anyhow::anyhow;
use std::error::Error;
use std::{env};

use config::LureConfig;
use lure::Lure;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let current_dir = env::current_dir()?;
    let config_file = current_dir.join("settings.toml");
    let config_file_path = config_file.to_str().ok_or(anyhow!("Failed to get config file path"))?;
    let config: LureConfig = LureConfig::load(config_file_path)?;

    println!("{}", config_file_path);
    let _ = config.save(config_file_path);

    let mut lure = Lure::new(config);
    lure.start().await?;
    Ok(())
}
