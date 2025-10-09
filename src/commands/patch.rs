use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common::ssh_pool::ServerPool;
use crate::log_info;
use crate::utils::file_utils::{log_local_file_info, split_unix_path};
use crate::utils::log_utils::ask_user;
use crate::domain::cmd_params::{PatchCmdConfig, ServerMetadata};

pub async fn run(
    config: &PatchCmdConfig,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>
) -> Result<(), String> {
    let local_path = PathBuf::from(&config.local_path);
    if !config.recover && !local_path.exists() {
        return Err(format!(
            "Local patch file '{}' does not exist",
            config.local_path
        ));
    }

    let resolved_upload = global_server_pool
        .resolve_remote_path(server_metadata,config.use_sudo, &config.remote_upload)
        .await?;
    let resolved_file = global_server_pool
        .resolve_remote_path(server_metadata,config.use_sudo, &config.remote_path)
        .await?;
    let resolved_backup = global_server_pool
        .resolve_remote_path(server_metadata,config.use_sudo, &config.remote_backup)
        .await?;

    if resolved_upload.is_empty() || resolved_file.is_empty() || resolved_backup.is_empty() {
        return Err("Failed to resolve one or more remote paths".to_string());
    }

    if config.recover {
        ask_user(
            &format!("Restore '{}' from '{}'?", resolved_file, resolved_backup),
            config.silent,
        )
        .await?;
        log_info!("Restoring '{}' from '{}'", resolved_file, resolved_backup);
        global_server_pool
            .exec(server_metadata,
                &format!("cp {} {} -f", resolved_backup, resolved_file),
                config.use_sudo,
            )
            .await?;
        log_info!("Recovery completed. Remote file info:");
        global_server_pool
            .exec_with_log(server_metadata,&format!("ls -al {}", resolved_file), config.use_sudo)
            .await?;
        return Ok(());
    }

    log_info!("==================================== Upload the local file");
    log_info!("Local file info:");
    log_local_file_info(&local_path)?;
    ask_user(
        &format!(
            "Upload '{}' to '{}'?",
            local_path.display(),
            resolved_upload
        ),
        config.silent,
    )
    .await?;

    log_info!(
        "Uploading '{}' to '{}'",
        local_path.display(),
        resolved_upload
    );
    let (basename, parent) = split_unix_path(&resolved_upload)?;
    // println!("parent: {}",parent);
    global_server_pool
        .validate_remote_dir(server_metadata,&parent, config.use_sudo)
        .await?;
    global_server_pool
        .create_remote_dir(server_metadata,&parent, config.use_sudo)
        .await?;
    global_server_pool
        .upload_file_or_dir_contents_into_dir(server_metadata,
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
    log_info!("Upload completed. Printing uploaded file info:");
    global_server_pool
        .exec_with_log(server_metadata,&format!("ls -al {}", resolved_upload), config.use_sudo)
        .await?;

    log_info!("==================================== Backup the server file");
    if !global_server_pool
        .file_or_dir_exists(server_metadata,&resolved_file, config.use_sudo)
        .await?
    {
        return Err(format!("Remote file '{}' does not exist", resolved_file));
    }
    log_info!("Remote file info before backup:");
    global_server_pool
        .exec_with_log(server_metadata,&format!("ls -al {}", resolved_file), config.use_sudo)
        .await?;
    ask_user(
        &format!("Backup '{}' to '{}'?", resolved_file, resolved_backup),
        config.silent,
    )
    .await?;
    log_info!("Backing up '{}' to '{}'", resolved_file, resolved_backup);
    global_server_pool
        .exec(server_metadata,
            &format!("cp {} {} -f", resolved_file, resolved_backup),
            config.use_sudo,
        )
        .await?;
    log_info!("Backup completed. Printing backup file info:");
    global_server_pool
        .exec_with_log(server_metadata,&format!("ls -al {}", resolved_backup), config.use_sudo)
        .await?;
    log_info!("==================================== Overwrite the server file");
    log_info!("Uploaded file info:");
    let ls_upload_before = global_server_pool
        .exec(server_metadata,&format!("ls -al {}", resolved_upload), config.use_sudo)
        .await?;
    for line in ls_upload_before.lines() {
        log_info!("{}", line);
    }
    ask_user(
        &format!("Overwrite '{}' with '{}'?", resolved_file, resolved_upload),
        config.silent,
    )
    .await?;
    log_info!("Applying patch to '{}'", resolved_file);
    global_server_pool
        .exec(server_metadata,
            &format!("cp {} {} -f", resolved_upload, resolved_file),
            config.use_sudo,
        )
        .await?;
    log_info!("Overwrite completed. Final file info:");
    global_server_pool
        .exec_with_log(server_metadata,&format!("ls -al {}", resolved_file), config.use_sudo)
        .await?;
    log_info!("Patch complete");
    Ok(())
}
