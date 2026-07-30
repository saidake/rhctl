/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 * **************************************************************************
 * Structs mapped from the YAML configuration file.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use serde::Deserialize;
use std::hash::{Hash, Hasher};
use std::{collections::HashMap, time::Duration};

pub trait TargetConfig {
    fn target_servers(&self) -> &Vec<String>;
    fn target_groups(&self) -> &Vec<String>;
}

impl TargetConfig for UploadConfig {
    fn target_servers(&self) -> &Vec<String> {
        &self.target_servers
    }
    fn target_groups(&self) -> &Vec<String> {
        &self.target_groups
    }
}

impl TargetConfig for PatchConfig {
    fn target_servers(&self) -> &Vec<String> {
        &self.target_servers
    }
    fn target_groups(&self) -> &Vec<String> {
        &self.target_groups
    }
}

impl TargetConfig for ExecuteConfig {
    fn target_servers(&self) -> &Vec<String> {
        &self.target_servers
    }
    fn target_groups(&self) -> &Vec<String> {
        &self.target_groups
    }
}

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
pub struct UploadConfig {
    pub use_rsync: Option<bool>,
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    pub properties_file: String,

    #[serde(default)]
    pub target_servers: Vec<String>, // explicitly list server names
    #[serde(default)]
    pub target_groups: Vec<String>, // or use group names
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PatchConfig {
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
pub struct ExecuteConfig {
    pub use_rsync: Option<bool>,
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    pub scripts: Vec<String>,      // now array of scripts
    pub work_path: Option<String>, // optional working directory
    pub mode: Option<String>,

    #[serde(default)]
    pub target_servers: Vec<String>,
    #[serde(default)]
    pub target_groups: Vec<String>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct NamedConfig {
    pub name: String, // config name, e.g. "dev-deploy"

    pub use_rsync: Option<bool>, // common flags (can be overridden inside each task)
    pub use_sudo: Option<bool>,
    pub silent: Option<bool>,

    #[serde(default)]
    pub upload: Vec<UploadConfig>,
    #[serde(default)]
    pub patch: Vec<PatchConfig>,
    #[serde(default)]
    pub execute: Vec<ExecuteConfig>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CommonConfig {
    // #[serde(default)]
    // pub global: Option<GlobalConfig>,
    #[serde(default)]
    pub server: Option<ServerConfigLimits>,
}

// #[derive(Clone, Deserialize, Default)]
// #[serde(rename_all = "kebab-case")]
// pub struct GlobalConfig {
//     pub max_global_channels: Option<usize>,
// }

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

    #[serde(default)]
    pub var_map: HashMap<String, String>,
}
