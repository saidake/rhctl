use std::time::Duration;

use serde::Deserialize;

#[derive(Clone, Deserialize, Default)]
pub struct UploadCmdConfig {
    pub server_key: u64,

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
pub struct ExecuteCmdConfig {
    pub server_key: u64,


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


#[derive(Clone, Deserialize, Default)]
pub struct PatchCmdConfig {
    pub server_key: u64,

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


pub trait ServerMetadata {
    fn get_host(&self) -> &str;
    fn get_user(&self) -> &str;
    fn get_ssh_port(&self) -> u16;
    fn get_password(&self) -> &str;
    fn get_server_key(&self) -> u64; 
    fn get_connect_timeout(&self) -> &Duration;
}

impl ServerMetadata for UploadCmdConfig {
    fn get_host(&self) -> &str { &self.host }
    fn get_user(&self) -> &str { &self.user }
    fn get_ssh_port(&self) -> u16 { self.ssh_port }
    fn get_password(&self) -> &str { &self.password }
    fn get_server_key(&self) -> u64 { self.server_key } 
    fn get_connect_timeout(&self) -> &Duration { &self.connect_timeout }
}


impl ServerMetadata for ExecuteCmdConfig {
    fn get_host(&self) -> &str { &self.host }
    fn get_user(&self) -> &str { &self.user }
    fn get_ssh_port(&self) -> u16 { self.ssh_port }
    fn get_password(&self) -> &str { &self.password }
    fn get_server_key(&self) -> u64 { self.server_key } 
    fn get_connect_timeout(&self) -> &Duration { &self.connect_timeout }
}


impl ServerMetadata for PatchCmdConfig {
    fn get_host(&self) -> &str { &self.host }
    fn get_user(&self) -> &str { &self.user }
    fn get_ssh_port(&self) -> u16 { self.ssh_port }
    fn get_password(&self) -> &str { &self.password }
    fn get_server_key(&self) -> u64 { self.server_key } 
    fn get_connect_timeout(&self) -> &Duration { &self.connect_timeout }
}
