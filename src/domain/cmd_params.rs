use std::time::Duration;

use serde::Deserialize;

#[derive(Clone, Deserialize, Default)]
pub struct UploadCmdConfig {
    pub host: String,
    pub user: String,
    pub ssh_port: u16,
    pub password: String,
    pub connect_timeout: Duration,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,

    pub properties_file: String,
}

#[derive(Clone, Deserialize, Default)]
pub struct PatchCmdConfig {
    pub host: String,
    pub user: String,
    pub ssh_port: u16,
    pub password: String,
    pub connect_timeout: Duration,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,
    #[serde(default)]
    pub recover: bool,

    pub local_path: String,
    pub remote_upload: String,
    pub remote_path: String,
    pub remote_backup: String,
}

#[derive(Clone, Deserialize, Default)]
pub struct ExecuteCmdConfig {
    pub host: String,
    pub user: String,
    pub ssh_port: u16,
    pub password: String,
    pub connect_timeout: Duration,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,

    pub script: String,
    pub remote_path: String,
}
