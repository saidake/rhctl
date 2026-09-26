/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * Execute a bash file in remote server.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::path::Path;
use std::sync::Arc;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{ExecuteCmdConfig, ScriptInvocation, ServerMetadata};
use crate::domain::constants::EXECUTE_TASK_NAME;
use crate::{log_debug, log_info};
use futures::future::join_all;

/// Single-quote escape for remote bash (safe inside `sudo bash -c '…'`).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn build_remote_command(work_path: &str, remote_script: &str, args: &[String]) -> String {
    let mut cmd = format!("cd {} && bash {}", work_path, shell_quote(remote_script));
    for arg in args {
        cmd.push(' ');
        cmd.push_str(&shell_quote(arg));
    }
    cmd
}

pub async fn run(
    config: &ExecuteCmdConfig,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>,
) -> Result<(), String> {
    if config.scripts.is_empty() {
        return Err("No scripts provided for execution".to_string());
    }

    let execute_single = |script: ScriptInvocation,
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
        let remote_cmd = build_remote_command(&config.work_path, &remote_script, &script.args);

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

        global_server_pool
            .exec_with_log(
                &server_metadata,
                EXECUTE_TASK_NAME,
                &remote_cmd,
                config.use_sudo,
            )
            .await?;

        Ok(())
    };

    if config.mode == "async" {
        let futures = config
            .scripts
            .clone()
            .into_iter()
            .map(|s| execute_single(s, server_metadata.clone(), global_server_pool.clone()));
        let results = join_all(futures).await;

        for result in results {
            if let Err(e) = result {
                return Err(e);
            }
        }
    } else {
        for script in &config.scripts {
            if let Err(e) = execute_single(
                script.clone(),
                server_metadata.clone(),
                global_server_pool.clone(),
            )
            .await
            {
                return Err(e);
            }
        }
    }

    log_info!(
        server_metadata,
        EXECUTE_TASK_NAME,
        "All scripts executed successfully."
    );
    Ok(())
}
