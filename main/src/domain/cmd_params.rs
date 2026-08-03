/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * The final parsed parameter structs derived from the original CLI arguments
 * or yml configuration file, to be used in upload, patch, and other tasks.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::time::Duration;

use serde::Deserialize;

#[derive(Clone, Deserialize, Default)]
pub struct UploadCmdConfig {
    pub server_metadata: ServerMetadata,
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
    pub server_metadata: ServerMetadata,

    #[serde(default)]
    pub use_rsync: bool,
    #[serde(default)]
    pub use_sudo: bool,
    #[serde(default)]
    pub silent: bool,

    pub scripts: Vec<String>,

    pub mode: String,
    pub work_path: String,
}

#[derive(Clone, Deserialize, Default)]
pub struct PatchCmdConfig {
    pub server_metadata: ServerMetadata,

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

#[derive(Clone, Deserialize, Default, PartialEq, Eq, Hash)]

pub struct ServerMetadata {
    pub server_key: u64,

    pub host: String,
    pub user: String,
    pub ssh_port: u16,
    pub password: String,
    /// Path to SSH private key (identity file). Preferred over password when set.
    pub identity_file: Option<String>,
    /// Path to OpenSSH certificate (requires `identity_file`).
    pub certificate_file: Option<String>,

    pub connect_timeout: Duration,
    pub max_channels_per_session: usize,
    pub max_sessions_per_server: usize,
    pub session_acquire_timeout: Duration,
    pub max_session_lifetime: Duration,
}
