use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    pub name: String, // server must have a name now
    pub host: String,
    pub user: String,
    pub ssh_port: u16,
    pub password: Option<String>,
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
pub struct YmlConfig {
    // list of servers
    pub servers: Option<Vec<ServerConfig>>,

    // group name -> server names
    pub groups: Option<HashMap<String, Vec<String>>>,

    // multiple deployment configs
    pub configs: Option<Vec<NamedConfig>>,
    #[serde(default)]
    pub vars: HashMap<String, String>,
}
