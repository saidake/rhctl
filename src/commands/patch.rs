/*
 * Copyright 2025 the original author or authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * **************************************************************************
 * Patch a file in remote server.
 * 
 * Author: Craig Brown
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{PatchCmdConfig, ServerMetadata};
use crate::domain::constants::PATCH_TASK_NAME;
use crate::log_info;
use crate::utils::file_utils::{log_local_file_info, split_unix_path};
use crate::utils::log_utils::ask_user;

pub async fn run(
    config: &PatchCmdConfig,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>,
) -> Result<(), String> {
    let local_path = PathBuf::from(&config.local_path);
    if !config.recover && !local_path.exists() {
        return Err(format!(
            "Local patch file '{}' does not exist",
            config.local_path
        ));
    }

    let resolved_upload = global_server_pool
        .resolve_remote_path(
            server_metadata,
            PATCH_TASK_NAME,
            config.use_sudo,
            &config.remote_upload,
        )
        .await?;
    let resolved_file = global_server_pool
        .resolve_remote_path(
            server_metadata,
            PATCH_TASK_NAME,
            config.use_sudo,
            &config.remote_path,
        )
        .await?;
    let resolved_backup = global_server_pool
        .resolve_remote_path(
            server_metadata,
            PATCH_TASK_NAME,
            config.use_sudo,
            &config.remote_backup,
        )
        .await?;

    if resolved_upload.is_empty() || resolved_file.is_empty() || resolved_backup.is_empty() {
        return Err("Failed to resolve one or more remote paths".to_string());
    }

    if config.recover {
        ask_user(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("Restore '{}' from '{}'?", resolved_file, resolved_backup),
            config.silent,
        )
        .await?;
        log_info!(
            server_metadata,
            PATCH_TASK_NAME,
            "Restoring '{}' from '{}'",
            resolved_file,
            resolved_backup
        );
        global_server_pool
            .exec(
                server_metadata,
                PATCH_TASK_NAME,
                &format!("cp {} {} -f", resolved_backup, resolved_file),
                config.use_sudo,
            )
            .await?;
        log_info!(
            server_metadata,
            PATCH_TASK_NAME,
            "Recovery completed. Remote file info:"
        );
        global_server_pool
            .exec_with_log(
                server_metadata,
                PATCH_TASK_NAME,
                &format!("ls -al {}", resolved_file),
                config.use_sudo,
            )
            .await?;
        return Ok(());
    }

    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        ">>> Upload the local file"
    );
    log_info!(server_metadata, PATCH_TASK_NAME, "Local file info:");
    log_local_file_info(server_metadata, PATCH_TASK_NAME, &local_path)?;
    ask_user(
        server_metadata,
        PATCH_TASK_NAME,
        &format!(
            "Upload '{}' to '{}'?",
            local_path.display(),
            resolved_upload
        ),
        config.silent,
    )
    .await?;

    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Uploading '{}' to '{}'",
        local_path.display(),
        resolved_upload
    );
    let (basename, parent) = split_unix_path(&resolved_upload)?;
    // println!("basename: {}, parent: {}",basename, parent);
    global_server_pool
        .validate_remote_dir(server_metadata, PATCH_TASK_NAME, &parent, config.use_sudo)
        .await?;
    global_server_pool
        .upload_file_or_dir_contents_into_dir(
            server_metadata,
            PATCH_TASK_NAME,
            Path::new(&local_path),
            &parent,
            Some(&basename),
            config.use_sudo,
            config.use_rsync,
            config.silent,
            false,
            true,
        )
        .await?;
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Upload completed. Printing uploaded file info:"
    );
    let item_info = global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("ls -al {}", resolved_upload),
            config.use_sudo,
        )
        .await?;
    log_info!(server_metadata, PATCH_TASK_NAME, "{}",item_info);

    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        ">>> Backup the server file"
    );
    if !global_server_pool
        .file_or_dir_exists(
            server_metadata,
            PATCH_TASK_NAME,
            &resolved_file,
            config.use_sudo,
        )
        .await?
    {
        return Err(format!("Remote file '{}' does not exist", resolved_file));
    }
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Remote file info before backup:"
    );

    let item_info = global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("ls -al {}", resolved_file),
            config.use_sudo,
        )
        .await?;
    log_info!(server_metadata, PATCH_TASK_NAME, "{}",item_info);
    ask_user(
        server_metadata,
        PATCH_TASK_NAME,
        &format!("Backup '{}' to '{}'?", resolved_file, resolved_backup),
        config.silent,
    )
    .await?;
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Backing up '{}' to '{}'",
        resolved_file,
        resolved_backup
    );
    global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("cp {} {} -f", resolved_file, resolved_backup),
            config.use_sudo,
        )
        .await?;
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Backup completed. Printing backup file info:"
    );
    let item_info = global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("ls -al {}", resolved_backup),
            config.use_sudo,
        )
        .await?;
    log_info!(server_metadata, PATCH_TASK_NAME, "{}",item_info);
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        ">>> Overwrite the server file"
    );
    log_info!(server_metadata, PATCH_TASK_NAME, "Uploaded file info:");
    let ls_upload_before = global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("ls -al {}", resolved_upload),
            config.use_sudo,
        )
        .await?;
    for line in ls_upload_before.lines() {
        log_info!(server_metadata, PATCH_TASK_NAME, "{}", line);
    }
    ask_user(
        server_metadata,
        PATCH_TASK_NAME,
        &format!("Overwrite '{}' with '{}'?", resolved_file, resolved_upload),
        config.silent,
    )
    .await?;
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Applying patch to '{}'",
        resolved_file
    );
    global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("cp {} {} -f", resolved_upload, resolved_file),
            config.use_sudo,
        )
        .await?;
    log_info!(
        server_metadata,
        PATCH_TASK_NAME,
        "Overwrite completed. Final file info:"
    );
    let item_info = global_server_pool
        .exec(
            server_metadata,
            PATCH_TASK_NAME,
            &format!("ls -al {}", resolved_file),
            config.use_sudo,
        )
        .await?;
    log_info!(server_metadata, PATCH_TASK_NAME, "{}",item_info);
    log_info!(server_metadata, PATCH_TASK_NAME, "Patch complete");
    Ok(())
}
