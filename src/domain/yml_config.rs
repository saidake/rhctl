use serde::Deserialize;
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

    pub scripts: Vec<String>,        // now array of scripts
    pub remote_path: Option<String>, // optional working directory

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

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct GlobalConfig {
    pub max_global_channels: Option<usize>,
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
    
    pub servers: Option<Vec<ServerConfig>>,
    // group name -> server names
    pub group_map: Option<HashMap<String, Vec<String>>>,

    // multiple deployment configs
    pub configs: Option<Vec<NamedConfig>>,
    
    #[serde(default)]
    pub var_map: HashMap<String, String>,
}
