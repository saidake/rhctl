/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * Structs mapped from the YAML configuration file.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use serde::Deserialize;
use std::hash::{Hash, Hasher};
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    pub name: String, // server must have a name now
    pub host: String,
    pub user: String,
    pub ssh_port: Option<u16>,
    pub password: Option<String>,
    /// Path to SSH private key (identity file). Preferred over password when set.
    pub identity_file: Option<String>,
    /// Path to OpenSSH certificate (requires `identity-file`).
    pub certificate_file: Option<String>,

    #[serde(default, with = "humantime_serde")]
    pub connect_timeout: Option<Duration>,
    pub max_channels_per_session: Option<usize>,
    pub max_sessions_per_server: Option<usize>,
    #[serde(default, with = "humantime_serde")]
    pub session_acquire_timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde")]
    pub max_session_lifetime: Option<Duration>,
}
// Implement Hash and Eq based on name+host+port (you can adjust the key)
impl PartialEq for ServerConfig {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.host == other.host && self.ssh_port == other.ssh_port
    }
}

impl Eq for ServerConfig {}

impl Hash for ServerConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.host.hash(state);
        self.ssh_port.hash(state);
    }
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct UploadStep {
    pub use_rsync: Option<bool>,
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    /// Path to a transfer file (`local=remote-dir` per line).
    #[serde(default, alias = "properties-file")]
    pub transfer_file: Option<String>,
    /// Inline transfers (`local=remote-dir`).
    #[serde(default)]
    pub transfers: Vec<String>,

    #[serde(default)]
    pub target_servers: Vec<String>,
    #[serde(default)]
    pub target_groups: Vec<String>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PatchStep {
    pub use_rsync: Option<bool>,
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    #[serde(default)]
    pub recover: bool,

    pub local_path: String,
    pub remote_upload: String,
    pub remote_path: String,
    pub remote_backup: String,

    #[serde(default)]
    pub target_servers: Vec<String>,
    #[serde(default)]
    pub target_groups: Vec<String>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ExecuteStep {
    pub use_rsync: Option<bool>,
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    /// Script command lines: path plus optional args, e.g. `configure.sh --port 5432`.
    #[serde(default)]
    pub scripts: Vec<String>,
    /// Remote shell commands (no local upload).
    #[serde(default)]
    pub cmds: Vec<String>,
    /// Regex with one capture group; must be set with `env-name`.
    pub env_extract_regex: Option<String>,
    /// Env key for remote `/etc/rhctl/.env`; must be set with `env-extract-regex`.
    pub env_name: Option<String>,
    /// Working directory on the remote host. `remote-path` is accepted as an alias.
    #[serde(default, alias = "remote-path")]
    pub work_path: Option<String>,
    pub mode: Option<String>,

    #[serde(default)]
    pub target_servers: Vec<String>,
    #[serde(default)]
    pub target_groups: Vec<String>,
}

/// Ordered pipeline step for `rhctl run`.
#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RunStep {
    Upload(UploadStep),
    Execute(ExecuteStep),
    Patch(PatchStep),
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct NamedConfig {
    pub name: String, // config name, e.g. "dev-deploy"

    pub use_rsync: Option<bool>, // common flags (can be overridden inside each task)
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    /// Default targets for all runs (overridable per step when step lists are non-empty).
    #[serde(default)]
    pub target_servers: Vec<String>,
    #[serde(default)]
    pub target_groups: Vec<String>,

    /// Config-scoped `${NAME}` overlays (override global `var-map` / env).
    #[serde(default)]
    pub var_map: HashMap<String, String>,

    /// Ordered upload / execute / patch steps.
    #[serde(default)]
    pub runs: Vec<RunStep>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CommonConfig {
    #[serde(default)]
    pub server: Option<ServerConfigLimits>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfigLimits {
    #[serde(default, with = "humantime_serde")]
    pub connect_timeout: Option<Duration>,
    pub max_channels_per_session: Option<usize>,
    pub max_sessions_per_server: Option<usize>,
    #[serde(default, with = "humantime_serde")]
    pub session_acquire_timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde")]
    pub max_session_lifetime: Option<Duration>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct YmlConfig {
    // list of servers
    pub common: Option<CommonConfig>,

    pub servers: Vec<ServerConfig>,
    // group name -> server names
    pub group_map: Option<HashMap<String, Vec<String>>>,

    // multiple deployment configs
    pub configs: Option<Vec<NamedConfig>>,

    /// Optional `${NAME}` overlays for paths (override process environment).
    #[serde(default)]
    pub var_map: HashMap<String, String>,
}
