use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub network: NetworkConfig,
    pub station: StationConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub radio: RadioConfig,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct NetworkConfig {
    pub host: String,
    pub port: u16,
    pub static_files: String,
    pub operator_password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_password: Option<String>,
    /// Path to TLS certificate PEM file (optional; enables HTTPS/WSS when set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_cert: Option<String>,
    /// Path to TLS private key PEM file (must be set alongside tls_cert).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_key: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct StationConfig {
    pub callsign: String,
    pub grid: String,
    /// ADIF log file path (default: "ft8.adi").
    #[serde(default = "default_log_file")]
    pub log_file: String,
}

fn default_log_file() -> String {
    "ft8.adi".to_string()
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AudioConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_device: Option<String>,
    pub sample_rate: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig {
            input_device: None,
            output_device: None,
            sample_rate: 12000,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
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
                tls_cert: None,
                tls_key: None,
            },
            station: StationConfig {
                callsign: "N0CALL".to_string(),
                grid: "AA00".to_string(),
                log_file: "ft8.adi".to_string(),
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

/// Persist the current in-memory config back to `betterft8.toml`.
pub fn save(config: &Config) -> Result<()> {
    let toml_str = toml::to_string_pretty(config)?;
    std::fs::write("betterft8.toml", toml_str)?;
    Ok(())
}
