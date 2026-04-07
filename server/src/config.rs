use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub network: NetworkConfig,
    pub station: StationConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub radio: RadioConfig,
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

#[derive(Deserialize, Clone)]
pub struct AudioConfig {
    pub input_device: Option<String>,
    pub sample_rate: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig {
            input_device: None,
            sample_rate: 12000,
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct RadioConfig {
    pub rigctld_host: String,
    pub rigctld_port: u16,
    pub poll_interval_ms: u64,
}

impl Default for RadioConfig {
    fn default() -> Self {
        RadioConfig {
            rigctld_host: "localhost".to_string(),
            rigctld_port: 4532,
            poll_interval_ms: 2000,
        }
    }
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
            audio: AudioConfig::default(),
            radio: RadioConfig::default(),
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
