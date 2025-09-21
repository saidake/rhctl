use serde::Deserialize;
use std::fs::File;
use std::io::Read;

#[derive(Clone, Deserialize, Default)]
pub struct RemoteConfig {
    pub host: Option<String>,
    pub user: Option<String>,
    pub ssh_port: u16,
    pub password: Option<String>,
}

#[derive(Clone, Deserialize, Default)]
pub struct UploadConfig {
    pub use_rsync: bool,
    pub use_sudo: bool,
    pub silent: bool,
    pub assets_root: String,
    pub properties_file: String,
}

#[derive(Clone, Deserialize, Default)]
pub struct PatchConfig {
    pub use_rsync: bool,
    pub use_sudo: bool,
    pub silent: bool,
    pub local_patch: String,
    pub remote_upload: String,
    pub remote_file: String,
    pub remote_backup: String,
}

#[derive(Clone, Deserialize, Default)]
pub struct ExecuteConfig {
    pub use_rsync: bool,
    pub use_sudo: bool,
    pub silent: bool,
}

#[derive(Clone, Deserialize, Default)]
pub struct Config {
    pub remote: RemoteConfig,
    pub upload: UploadConfig,
    pub patch: PatchConfig,
    pub execute: ExecuteConfig,
}

#[derive(Clone)]
pub struct ConfigWrapper {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub use_sudo: bool,
    pub use_rsync: bool,
    pub silent: bool,
    pub upload: UploadConfig,
    pub patch: PatchConfig,
    pub execute: ExecuteConfig,
}

pub fn load_yaml_config(path: &str) -> Result<Config, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open config file {}: {}", path, e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|e| format!("Failed to read config file {}: {}", path, e))?;
    serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse YAML config {}: {}", path, e))
}