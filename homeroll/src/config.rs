use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub priory: priory::Config,
}

impl Config {
    pub fn parse(config_file_path: &str) -> Result<Self> {
        let config_content = fs::read_to_string(config_file_path)?;
        let config: Config = toml::from_str(&config_content)?;
        Ok(config)
    }
}
