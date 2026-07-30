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
 * Execute a bash file in remote server.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{ExecuteCmdConfig, ServerMetadata};
use crate::domain::constants::EXECUTE_TASK_NAME;
use crate::{log_debug, log_info};
use futures::future::join_all; // Added for async parallel execution

pub async fn run(
    config: &ExecuteCmdConfig,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>,
) -> Result<(), String> {
    // Early validation: ensure scripts list is not empty
    if config.scripts.is_empty() {
        return Err("No scripts provided for execution".to_string());
    }

    // Helper async closure for single script execution
    let execute_single = |script: String,
                          server_metadata: Arc<ServerMetadata>,
                          global_server_pool: Arc<ServerPool>| async move {
        let script_path = Path::new(&script);
        if !script_path.exists() || !script_path.is_file() {
            return Err(format!(
                "Script file '{}' does not exist or is not a file",
                script
            ));
        }

        let script_name = script_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Failed to get basename for '{}'", &script))?;

        if config.use_sudo {
            // Create temporary directory
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
                script,
                temp_remote_dir
            );

            // Upload script to remote temp dir
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
            log_info!(
                &server_metadata,
                EXECUTE_TASK_NAME,
                "Executing script {} in '{}' with sudo",
                script_name,
                config.work_path
            );

            // Execute remotely
            global_server_pool
                .exec_with_log(
                    &server_metadata,
                    EXECUTE_TASK_NAME,
                    &format!("cd {} && bash {}", config.work_path, remote_script),
                    config.use_sudo,
                )
                .await?;
        } else {
            // Execute without sudo: read and execute inline
            let mut content = String::new();
            File::open(script_path)
                .map_err(|e| format!("Failed to open script '{}'. \n\t> {}", script, e))?
                .read_to_string(&mut content)
                .map_err(|e| format!("Failed to read script '{}'. \n\t> {}", script, e))?;

            log_info!(
                &server_metadata,
                EXECUTE_TASK_NAME,
                "Executing script {} in '{}': ",
                script_path.display(),
                config.work_path
            );

            global_server_pool
                .exec_with_log(
                    &server_metadata,
                    EXECUTE_TASK_NAME,
                    &format!(
                        "cd {} && bash -l -s <<EOF\n{}\nEOF",
                        config.work_path, content
                    ),
                    false,
                )
                .await?;
        }

        Ok(())
    };

    // Execute scripts based on mode
    if config.mode == "async" {
        // Run all scripts concurrently
        let futures = config
            .scripts
            .clone()
            .into_iter()
            .map(|s| execute_single(s, server_metadata.clone(), global_server_pool.clone()));
        let results = join_all(futures).await;

        // Check for any failures
        for result in results {
            if let Err(e) = result {
                // log_error_with_host_direct!(&server_metadata.user, &server_metadata.host, EXECUTE_TASK_NAME, "{}", e);
                return Err(e);
            }
        }
    } else {
        // Run scripts sequentially
        for script in &config.scripts {
            if let Err(e) = execute_single(
                script.clone(),
                server_metadata.clone(),
                global_server_pool.clone(),
            )
            .await
            {
                // log_error_with_host_direct!(&server_metadata.user, &server_metadata.host, EXECUTE_TASK_NAME, "{}", e);
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
