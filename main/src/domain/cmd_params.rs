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

/// One local script to run remotely: path plus optional CLI args.
/// Parsed from a `--script` / YAML value such as `init.sh --port 5432`.
#[derive(Clone, Debug, Default)]
pub struct ScriptInvocation {
    pub path: String,
    pub args: Vec<String>,
}

/// Parse a script command line into path + args (shell-style quoting).
/// Newlines and extra whitespace between tokens are allowed.
pub fn parse_script_invocation(raw: &str) -> Result<ScriptInvocation, String> {
    let tokens = shlex::split(raw).ok_or_else(|| {
        format!(
            "Invalid script command line (unbalanced quotes): '{}'",
            raw
        )
    })?;
    if tokens.is_empty() {
        return Err("Empty script command line".to_string());
    }
    let mut iter = tokens.into_iter();
    let path = iter.next().unwrap();
    Ok(ScriptInvocation {
        path,
        args: iter.collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_path_only() {
        let inv = parse_script_invocation("scripts/init.sh").unwrap();
        assert_eq!(inv.path, "scripts/init.sh");
        assert!(inv.args.is_empty());
    }

    #[test]
    fn parse_path_with_args() {
        let inv = parse_script_invocation("scripts/postgresql/init.sh --port 5432").unwrap();
        assert_eq!(inv.path, "scripts/postgresql/init.sh");
        assert_eq!(inv.args, vec!["--port", "5432"]);
    }

    #[test]
    fn parse_quoted_path_and_arg_values() {
        let inv = parse_script_invocation(r#"'/path/with spaces/init.sh' --name "my db""#).unwrap();
        assert_eq!(inv.path, "/path/with spaces/init.sh");
        assert_eq!(inv.args, vec!["--name", "my db"]);
    }

    #[test]
    fn parse_multiline_with_ipv6_allow_all() {
        let inv = parse_script_invocation(
            "/tmp/scripts/postgresql/init.sh \
     --port 5432 \
     --auth-method md5 \
     --allowed-ips 0.0.0.0/0,::/0",
        )
        .unwrap();
        assert_eq!(inv.path, "/tmp/scripts/postgresql/init.sh");
        assert_eq!(
            inv.args,
            vec![
                "--port",
                "5432",
                "--auth-method",
                "md5",
                "--allowed-ips",
                "0.0.0.0/0,::/0"
            ]
        );
    }

    #[test]
    fn reject_empty_and_unbalanced() {
        assert!(parse_script_invocation("").is_err());
        assert!(parse_script_invocation("init.sh --name 'unterminated").is_err());
    }
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

    #[serde(skip)]
    pub scripts: Vec<ScriptInvocation>,

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
