/*
 * Copyright (C) 2022-2026 rsctl Contributors
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
 * Upload a file or directory contents into a remote server directory.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */

use futures::future::join_all;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{ServerMetadata, UploadCmdConfig};
use crate::domain::constants::UPLOAD_TASK_NAME;
use crate::{log_info, log_warn};

pub async fn run(
    config: &UploadCmdConfig,
    mappings: &HashMap<String, String>,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>,
) -> Result<(), String> {
    if !Path::new(&config.properties_file).exists() {
        return Err(format!(
            "Properties file not found: '{}'",
            config.properties_file
        ));
    }
    let mut tasks = Vec::new();

    for (local_item, remote_dir) in mappings {
        // println!("local_item: {}, remote_dir: {}",local_item, remote_dir);
        let local_file_or_dir = Path::new(&local_item).to_path_buf();
        if !local_file_or_dir.exists() {
            log_warn!(
                server_metadata,
                UPLOAD_TASK_NAME,
                "Local item '{}' not found. Skipping.",
                local_item
            );
            continue;
        }

        let remote_dir_resolved = global_server_pool
            .resolve_remote_path(
                server_metadata,
                UPLOAD_TASK_NAME,
                config.use_sudo,
                &remote_dir,
            )
            .await
            .map_err(|e| {
                format!(
                    "Child Upload Task failed: {} --> {} \n\t> {}",
                    local_item, remote_dir, e
                )
            })?;
        if remote_dir_resolved.is_empty() {
            return Err(format!(
                "Failed to resolve remote directory '{}'",
                remote_dir
            ));
        }

        // Check if remote directory is writable
        global_server_pool
            .validate_remote_dir(
                server_metadata,
                UPLOAD_TASK_NAME,
                &remote_dir_resolved,
                config.use_sudo,
            )
            .await
            .map_err(|e| {
                format!(
                    "Child Upload Task failed: {} --> {} \n\t> {}",
                    local_item, remote_dir, e
                )
            })?;

        let local_file_or_dir_clone = local_file_or_dir.clone();
        let remote_dir_clone = remote_dir_resolved.clone();
        let config_clone = config.clone();
        let global_server_pool_clone = global_server_pool.clone();
        let server_metadata_clone = server_metadata.clone(); // clone Arc

        // spawn upload thread
        // spawn async task
        let task = tokio::spawn(async move {
            global_server_pool_clone
                .upload_file_or_dir_contents_into_dir(
                    &server_metadata_clone,
                    UPLOAD_TASK_NAME,
                    &local_file_or_dir_clone,
                    &remote_dir_clone,
                    None,
                    config_clone.use_sudo,
                    config_clone.use_rsync,
                    config_clone.silent,
                    false,
                    true,
                )
                .await
                .map_err(|e| {
                    format!(
                        "Child Upload Task failed: {} --> {} \n\t> {}",
                        local_file_or_dir_clone.display(),
                        remote_dir_clone,
                        e
                    )
                })
        });

        tasks.push(task);
    }

    // collect results
    let results = join_all(tasks).await;
    let mut errors = Vec::new();
    for r in results {
        if let Err(e) = r {
            errors.push(format!("Task join error: {:?}", e));
        } else if let Ok(Err(e)) = r {
            errors.push(e);
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n\n\t"));
    }

    log_info!(server_metadata, UPLOAD_TASK_NAME, "Upload complete.");
    Ok(())
}
