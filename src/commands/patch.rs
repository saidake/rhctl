use log::info;
use std::path::{Path, PathBuf};

use crate::common::ssh::ServerHandle;
use crate::domain::cmd_params::PatchCmdConfig;
use crate::utils::file_utils::{log_local_file_info, split_unix_path};
use crate::utils::log_utils::ask_user;

pub async fn run(config: &PatchCmdConfig, server_handle: &ServerHandle<PatchCmdConfig>) -> Result<(), String> {
    let local_path = PathBuf::from(&config.local_path);
    if !config.recover && !local_path.exists() {
        return Err(format!(
            "Local patch file '{}' does not exist",
            config.local_path
        ));
    }

    let resolved_upload = server_handle.resolve_remote_path( config.use_sudo, &config.remote_upload).await?;
    let resolved_file = server_handle.resolve_remote_path( config.use_sudo, &config.remote_path).await?;
    let resolved_backup = server_handle.resolve_remote_path( config.use_sudo, &config.remote_backup).await?;

    if resolved_upload.is_empty() || resolved_file.is_empty() || resolved_backup.is_empty() {
        return Err("Failed to resolve one or more remote paths".to_string());
    }

    if config.recover {
        ask_user(
            &format!("Restore '{}' from '{}'?", resolved_file, resolved_backup),
            config.silent,
        )?;
        info!("Restoring '{}' from '{}'", resolved_file, resolved_backup);
        server_handle.exec(
            &format!("cp {} {} -f", resolved_backup, resolved_file),
            config.use_sudo,
        ).await?;
        info!("Recovery completed. Remote file info:");
        server_handle.exec_with_log(&format!("ls -al {}", resolved_file), config.use_sudo).await?;
        return Ok(());
    }

    info!("==================================== Upload the local file");
    info!("Local file info:");
    log_local_file_info(&local_path)?;
    ask_user(
        &format!(
            "Upload '{}' to '{}'?",
            local_path.display(),
            resolved_upload
        ),
        config.silent,
    )?;

    info!(
        "Uploading '{}' to '{}'",
        local_path.display(),
        resolved_upload
    );
    let (basename, parent) = split_unix_path(&resolved_upload)?;
    // println!("parent: {}",parent);
    server_handle.validate_remote_dir(&parent, config.use_sudo).await?;
    server_handle.create_remote_dir(&parent, config.use_sudo).await?;
    server_handle.upload_file_or_dir_contents_into_dir(
        Path::new(&local_path),
        &parent,
        Some(&basename),
        config.use_sudo,
        config.use_rsync,
        config.silent,
        false,
        true,
    ).await?;
    info!("Upload completed. Printing uploaded file info:");
    server_handle.exec_with_log(&format!("ls -al {}", resolved_upload), config.use_sudo).await?;

    info!("==================================== Backup the server file");
    if !server_handle.file_or_dir_exists(&resolved_file, config.use_sudo).await? {
        return Err(format!("Remote file '{}' does not exist", resolved_file));
    }
    info!("Remote file info before backup:");
    server_handle.exec_with_log(&format!("ls -al {}", resolved_file), config.use_sudo).await?;
    ask_user(
        &format!("Backup '{}' to '{}'?", resolved_file, resolved_backup),
        config.silent,
    )?;
    info!("Backing up '{}' to '{}'", resolved_file, resolved_backup);
    server_handle.exec(
        &format!("cp {} {} -f", resolved_file, resolved_backup),
        config.use_sudo,
    ).await?;
    info!("Backup completed. Printing backup file info:");
    server_handle.exec_with_log(&format!("ls -al {}", resolved_backup), config.use_sudo).await?;
    info!("==================================== Overwrite the server file");
    info!("Uploaded file info:");
    let ls_upload_before = server_handle.exec(&format!("ls -al {}", resolved_upload), config.use_sudo).await?;
    for line in ls_upload_before.lines() {
        info!("{}", line);
    }
    ask_user(
        &format!("Overwrite '{}' with '{}'?", resolved_file, resolved_upload),
        config.silent,
    )?;
    info!("Applying patch to '{}'", resolved_file);
    server_handle.exec(
        &format!("cp {} {} -f", resolved_upload, resolved_file),
        config.use_sudo,
    ).await?;
    info!("Overwrite completed. Final file info:");
    server_handle.exec_with_log(&format!("ls -al {}", resolved_file), config.use_sudo).await?;
    info!("Patch complete");
    Ok(())
}
