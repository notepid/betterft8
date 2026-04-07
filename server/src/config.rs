use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub network: NetworkConfig,
    pub station: StationConfig,
}

#[derive(Deserialize, Clone)]
pub struct NetworkConfig {
    pub host: String,
    pub port: u16,
    pub static_files: String,
    pub operator_password: String,
    pub viewer_password: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct StationConfig {
    pub callsign: String,
    pub grid: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            network: NetworkConfig {
                host: "0.0.0.0".to_string(),
                port: 8073,
                static_files: "../client/dist".to_string(),
                operator_password: "changeme".to_string(),
                viewer_password: None,
            },
            station: StationConfig {
                callsign: "N0CALL".to_string(),
                grid: "AA00".to_string(),
            },
        }
    }
}

pub fn load() -> Result<Config> {
    match std::fs::read_to_string("betterft8.toml") {
        Ok(contents) => Ok(toml::from_str(&contents)?),
        Err(_) => {
            tracing::warn!("betterft8.toml not found, using defaults");
            Ok(Config::default())
        }
    }
}
