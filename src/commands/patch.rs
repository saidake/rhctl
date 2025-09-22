use std::path::PathBuf;
use std::process::Command;
use log::info;

use crate::common::config::ConfigWrapper;
use crate::common::ssh::SshSession;
use crate::common::utils::{ask_user, resolve_remote_path};

pub fn run(session: &SshSession, config: &ConfigWrapper, local_patch: &str, remote_upload: &str, remote_file: &str, remote_backup: &str, recover: bool) -> Result<(), String> {
    let local_path = PathBuf::from(local_patch);
    if !recover && !local_path.exists() {
        return Err(format!("Local patch file '{}' does not exist", local_patch));
    }

    let resolved_upload = resolve_remote_path(session, config, remote_upload)?;
    let resolved_file = resolve_remote_path(session, config, remote_file)?;
    let resolved_backup = resolve_remote_path(session, config, remote_backup)?;

    if resolved_upload.is_empty() || resolved_file.is_empty() || resolved_backup.is_empty() {
        return Err("Failed to resolve one or more remote paths".to_string());
    }

    if recover {
        if !config.patch.silent && !ask_user(&format!("Restore '{}' from '{}'?", resolved_file, resolved_backup)) {
            return Ok(());
        }
        info!("Restoring '{}' from '{}'", resolved_file, resolved_backup);
        session.execute(&format!("cp {} {} -f", resolved_backup, resolved_file), config.patch.use_sudo)?;
        info!("Recovery completed. Remote file info:");
        let ls_output = session.execute(&format!("ls -al {}", resolved_file), config.patch.use_sudo)?;
        for line in ls_output.lines() {
            info!("{}", line);
        }
        return Ok(());
    }

    info!("==================================== Upload the local file");
    info!("Local file info:");
    let ls_local = Command::new("ls")
        .arg("-al")
        .arg(&local_path)
        .output()
        .map_err(|e| format!("Failed to execute ls -al on '{}': {}", local_path.display(), e))?;
    if !ls_local.status.success() {
        return Err(format!("Failed to get local file info for '{}'", local_path.display()));
    }
    for line in String::from_utf8_lossy(&ls_local.stdout).lines() {
        info!("{}", line);
    }

    if session.file_exists(&resolved_upload)? {
        if !config.patch.silent && !ask_user(&format!("The remote file '{}' already exists. Overwrite with '{}'?", resolved_upload, local_path.display())) {
            return Ok(());
        }
    } else {
        if !config.patch.silent && !ask_user(&format!("Upload '{}' to '{}'?", local_path.display(), resolved_upload)) {
            return Ok(());
        }
    }
    info!("Uploading '{}' to '{}'", local_path.display(), resolved_upload);
    session.scp_upload(&local_path, &resolved_upload, config.patch.use_sudo)?;
    info!("Upload completed. Printing uploaded file info:");
    let ls_upload = session.execute(&format!("ls -al {}", resolved_upload), config.patch.use_sudo)?;
    for line in ls_upload.lines() {
        info!("{}", line);
    }

    info!("==================================== Backup the server file");
    if !session.file_exists(&resolved_file)? {
        return Err(format!("Remote file '{}' does not exist", resolved_file));
    }
    info!("Remote file info before backup:");
    let ls_remote = session.execute(&format!("ls -al {}", resolved_file), config.patch.use_sudo)?;
    for line in ls_remote.lines() {
        info!("{}", line);
    }
    if !config.patch.silent && !ask_user(&format!("Backup '{}' to '{}'?", resolved_file, resolved_backup)) {
        return Ok(());
    }
    info!("Backing up '{}' to '{}'", resolved_file, resolved_backup);
    session.execute(&format!("cp {} {} -f", resolved_file, resolved_backup), config.patch.use_sudo)?;
    info!("Backup completed. Printing backup file info:");
    let ls_backup = session.execute(&format!("ls -al {}", resolved_backup), config.patch.use_sudo)?;
    for line in ls_backup.lines() {
        info!("{}", line);
    }

    info!("==================================== Overwrite the server file");
    info!("Uploaded file info:");
    let ls_upload_before = session.execute(&format!("ls -al {}", resolved_upload), config.patch.use_sudo)?;
    for line in ls_upload_before.lines() {
        info!("{}", line);
    }
    if !config.patch.silent && !ask_user(&format!("Overwrite '{}' with '{}'?", resolved_file, resolved_upload)) {
        return Ok(());
    }
    info!("Applying patch to '{}'", resolved_file);
    session.execute(&format!("cp {} {} -f", resolved_upload, resolved_file), config.patch.use_sudo)?;
    info!("Overwrite completed. Final file info:");
    let ls_final = session.execute(&format!("ls -al {}", resolved_file), config.patch.use_sudo)?;
    for line in ls_final.lines() {
        info!("{}", line);
    }

    info!("Patch complete");
    Ok(())
}