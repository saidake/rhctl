use serde::Deserialize;

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct CommonConfig {
    pub host: Option<String>,
    pub user: Option<String>,
    pub ssh_port: Option<u16>,
    pub password: Option<String>,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct UploadConfig {
    pub host: Option<String>,
    pub user: Option<String>,
    pub ssh_port: Option<u16>,
    pub password: Option<String>,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,

    pub assets_root: String,
    pub properties_file: String,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct PatchConfig {
    pub host: Option<String>,
    pub user: Option<String>,
    pub ssh_port: Option<u16>,
    pub password: Option<String>,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,
    #[serde(default)]
    pub recover: bool,

    pub local_patch: String,
    pub remote_upload: String,
    pub remote_file: String,
    pub remote_backup: String,
}

#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ExecuteConfig {
    pub host: Option<String>,
    pub user: Option<String>,
    pub ssh_port: Option<u16>,
    pub password: Option<String>,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,

    pub script: String,
    pub remote_path: Option<String>,
}

#[derive(Clone, Deserialize, Default)]
pub struct YmlConfig {
    pub common: Option<CommonConfig>,
    pub upload: Option<UploadConfig>,
    pub patch: Option<PatchConfig>,
    pub execute: Option<ExecuteConfig>,
}
