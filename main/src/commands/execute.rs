/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * Execute a bash file or command on a remote server.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::path::Path;
use std::sync::Arc;

use regex::Regex;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{ExecuteCmdConfig, ScriptInvocation, ServerMetadata};
use crate::domain::constants::EXECUTE_TASK_NAME;
use crate::{log_debug, log_info};
use futures::future::join_all;

const RHCTL_ENV_DIR: &str = "/etc/rhctl";
const RHCTL_ENV_FILE: &str = "/etc/rhctl/.env";

/// Single-quote escape for remote bash (safe inside `sudo bash -c '…'`).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn build_remote_script_command(work_path: &str, remote_script: &str, args: &[String]) -> String {
    let mut cmd = format!("cd {} && bash {}", work_path, shell_quote(remote_script));
    for arg in args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(arg));
    }
    cmd
}

fn build_remote_cmd_command(work_path: &str, cmd: &str) -> String {
    format!("cd {} && {}", work_path, cmd)
}

pub fn validate_env_extract_pairs(
    regexes: &[String],
    names: &[String],
) -> Result<(), String> {
    if regexes.is_empty() && names.is_empty() {
        return Ok(());
    }
    if regexes.is_empty() || names.is_empty() {
        return Err(
            "--env-extract-regex and --env-name must be provided together (same count)".to_string(),
        );
    }
    if regexes.len() != names.len() {
        return Err(format!(
            "--env-extract-regex count ({}) must match --env-name count ({})",
            regexes.len(),
            names.len()
        ));
    }
    Ok(())
}

/// Last capture-group match on lines that do not contain `\\r` (progress output).
pub fn extract_env_value(output: &str, pattern: &str) -> Result<String, String> {
    let re = Regex::new(pattern).map_err(|e| format!("Invalid --env-extract-regex: {}", e))?;
    if re.captures_len() < 2 {
        return Err(
            "--env-extract-regex must contain at least one capture group, e.g. 'URL=(.*)'"
                .to_string(),
        );
    }

    let mut last: Option<String> = None;
    for line in output.split('\n') {
        // PTY often uses CRLF; strip trailing CR before matching.
        let line = line.trim_end_matches('\r');
        // Skip progress/spinner lines that rewrite mid-line with CR.
        if line.contains('\r') {
            continue;
        }
        if let Some(caps) = re.captures(line) {
            if let Some(m) = caps.get(1) {
                last = Some(m.as_str().to_string());
            }
        }
    }

    last.ok_or_else(|| {
        let sample: Vec<&str> = output
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.contains('\r') && !l.trim().is_empty())
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let sample_txt = if sample.is_empty() {
            "(no clean stdout lines collected)".to_string()
        } else {
            sample.join("\n\t")
        };
        let mut msg = format!(
            "No match for --env-extract-regex in remote output (skipped mid-line '\\r' progress)\n\
             \tPattern: {}\n\
             \tRecent clean lines:\n\t{}",
            pattern, sample_txt
        );
        // After shell double-quotes, `\[INFO\]` often becomes `[INFO]` (a character class).
        if pattern.contains('[') && !pattern.contains("\\[") {
            msg.push_str(
                "\n\tHint: use single quotes so brackets stay escaped, e.g. \
                 --env-extract-regex '\\[INFO\\]\\s+DATABASE_URL=(.*)'",
            );
        }
        msg
    })
}

fn validate_env_name(name: &str) -> Result<(), String> {
    let ok = name
        .chars()
        .enumerate()
        .all(|(i, c)| {
            if i == 0 {
                c.is_ascii_alphabetic() || c == '_'
            } else {
                c.is_ascii_alphanumeric() || c == '_'
            }
        })
        && !name.is_empty();
    if ok {
        Ok(())
    } else {
        Err(format!(
            "Invalid --env-name '{}': use [A-Za-z_][A-Za-z0-9_]*",
            name
        ))
    }
}

fn python_str_literal(s: &str) -> String {
    format!("'{s}'", s = s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn build_upsert_env_command(env_name: &str, value: &str) -> String {
    // Avoid indented `if` blocks: Rust `\n\` line-continuations strip leading spaces,
    // which caused IndentationError under `python3 -c`.
    let py = format!(
        "from pathlib import Path\n\
dir_path = Path({dir})\n\
env_file = dir_path / '.env'\n\
dir_path.mkdir(parents=True, exist_ok=True)\n\
dir_path.chmod(0o700)\n\
key = {key}\n\
value = {value}\n\
lines = [ln for ln in (env_file.read_text(encoding='utf-8').splitlines() if env_file.exists() else []) if not ln.startswith(key + '=')]\n\
lines.append(key + '=' + value)\n\
env_file.write_text('\\n'.join(lines) + '\\n', encoding='utf-8')\n\
env_file.chmod(0o600)\n",
        dir = python_str_literal(RHCTL_ENV_DIR),
        key = python_str_literal(env_name),
        value = python_str_literal(value),
    );
    format!("python3 -c {}", shell_quote(&py))
}

async fn persist_env_value(
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: &Arc<ServerPool>,
    use_sudo: bool,
    env_name: &str,
    value: &str,
) -> Result<(), String> {
    validate_env_name(env_name)?;
    let remote_cmd = build_upsert_env_command(env_name, value);
    log_info!(
        server_metadata,
        EXECUTE_TASK_NAME,
        "Writing {} to {}",
        env_name,
        RHCTL_ENV_FILE
    );
    global_server_pool
        .exec_with_log(
            server_metadata,
            EXECUTE_TASK_NAME,
            &remote_cmd,
            use_sudo,
        )
        .await?;
    log_info!(
        server_metadata,
        EXECUTE_TASK_NAME,
        "Saved {}={} in {}",
        env_name,
        value,
        RHCTL_ENV_FILE
    );
    Ok(())
}

pub async fn run(
    config: &ExecuteCmdConfig,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>,
) -> Result<(), String> {
    if config.scripts.is_empty() && config.cmds.is_empty() {
        return Err("Provide at least one --script or --cmd".to_string());
    }

    validate_env_extract_pairs(&config.env_extract_regexes, &config.env_names)?;

    let mut all_output = String::new();

    let execute_script = |script: ScriptInvocation,
                          server_metadata: Arc<ServerMetadata>,
                          global_server_pool: Arc<ServerPool>| async move {
        let script_path = Path::new(&script.path);
        if !script_path.exists() || !script_path.is_file() {
            return Err(format!(
                "Script file '{}' does not exist or is not a file",
                script.path
            ));
        }

        let script_name = script_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Failed to get basename for '{}'", &script.path))?;

        let temp_remote_dir = global_server_pool
            .create_remote_temp_dir(
                &server_metadata.clone(),
                EXECUTE_TASK_NAME,
                "exec",
                config.use_sudo,
            )
            .await?;

        log_debug!(
            &server_metadata,
            EXECUTE_TASK_NAME,
            "Uploading script '{}' to temporary path '{}'",
            script.path,
            temp_remote_dir
        );

        global_server_pool
            .upload_file_or_dir_contents_into_dir(
                &server_metadata,
                EXECUTE_TASK_NAME,
                script_path,
                &temp_remote_dir,
                None,
                config.use_sudo,
                config.use_rsync,
                config.silent,
                true,
                false,
            )
            .await?;

        let remote_script = format!("{}/{}", temp_remote_dir, script_name);
        let remote_cmd =
            build_remote_script_command(&config.work_path, &remote_script, &script.args);

        if script.args.is_empty() {
            log_info!(
                &server_metadata,
                EXECUTE_TASK_NAME,
                "Executing script {} in '{}'",
                script_name,
                config.work_path
            );
        } else {
            log_info!(
                &server_metadata,
                EXECUTE_TASK_NAME,
                "Executing script {} in '{}' with args {:?}",
                script_name,
                config.work_path,
                script.args
            );
        }

        let out = global_server_pool
            .exec_with_log(
                &server_metadata,
                EXECUTE_TASK_NAME,
                &remote_cmd,
                config.use_sudo,
            )
            .await?;

        Ok(out)
    };

    if config.mode == "async" && !config.scripts.is_empty() {
        let futures = config
            .scripts
            .clone()
            .into_iter()
            .map(|s| execute_script(s, server_metadata.clone(), global_server_pool.clone()));
        let results = join_all(futures).await;

        for result in results {
            match result {
                Ok(out) => {
                    if !all_output.is_empty() {
                        all_output.push('\n');
                    }
                    all_output.push_str(&out);
                }
                Err(e) => return Err(e),
            }
        }
    } else {
        for script in &config.scripts {
            let out = execute_script(
                script.clone(),
                server_metadata.clone(),
                global_server_pool.clone(),
            )
            .await?;
            if !all_output.is_empty() {
                all_output.push('\n');
            }
            all_output.push_str(&out);
        }
    }

    for cmd in &config.cmds {
        log_info!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "Executing command in '{}': {}",
            config.work_path,
            cmd
        );
        let remote_cmd = build_remote_cmd_command(&config.work_path, cmd);
        let out = global_server_pool
            .exec_with_log(
                server_metadata,
                EXECUTE_TASK_NAME,
                &remote_cmd,
                config.use_sudo,
            )
            .await?;
        if !all_output.is_empty() {
            all_output.push('\n');
        }
        all_output.push_str(&out);
    }

    for (pattern, env_name) in config
        .env_extract_regexes
        .iter()
        .zip(config.env_names.iter())
    {
        let value = extract_env_value(&all_output, pattern)?;
        persist_env_value(
            server_metadata,
            &global_server_pool,
            config.use_sudo,
            env_name,
            &value,
        )
        .await?;
    }

    if !config.scripts.is_empty() && config.cmds.is_empty() {
        log_info!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "All scripts executed successfully."
        );
    } else if config.scripts.is_empty() && !config.cmds.is_empty() {
        log_info!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "All commands executed successfully."
        );
    } else {
        log_info!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "All scripts and commands executed successfully."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_skips_cr_progress_and_takes_last() {
        let out = "[INFO] DATABASE_URL=first\nprogress 10%\rprogress 50%\r\n[INFO] DATABASE_URL=second\n";
        let v = extract_env_value(out, r"\[INFO\] DATABASE_URL=(.*)").unwrap();
        assert_eq!(v, "second");
    }

    #[test]
    fn extract_accepts_trailing_cr_from_pty() {
        let out = "[INFO]   DATABASE_URL=postgres://u:p@127.0.0.1:5432/db\r\n";
        let v = extract_env_value(out, r"\[INFO\]\s+DATABASE_URL=(.*)").unwrap();
        assert_eq!(v, "postgres://u:p@127.0.0.1:5432/db");
    }

    #[test]
    fn extract_requires_capture_group() {
        assert!(extract_env_value("a=1", r"a=1").is_err());
    }
}
