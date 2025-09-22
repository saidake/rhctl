use std::path::PathBuf;

use log::{error, info};

use crate::common::config::ConfigWrapper;
use crate::common::ssh::SshSession;
use crate::common::utils::{ask_user, resolve_remote_path};

pub fn run(session: &SshSession, config: &ConfigWrapper, local_patch: &str, remote_upload: &str, remote_file: &str, remote_backup: &str, recover: bool) -> Result<(), String> {
    let local_path = PathBuf::from(local_patch);
    if !local_path.exists() {
        return Err("Local patch file does not exist".to_string());
    }

    let resolved_upload = resolve_remote_path(session, config, remote_upload)?;
    let resolved_file = resolve_remote_path(session, config, remote_file)?;
    let resolved_backup = resolve_remote_path(session, config, remote_backup)?;

    if recover {
        if !config.silent && !ask_user(&format!("Restore {} from {}?", remote_file, remote_backup)) {
            return Ok(());
        }
        session.execute(&format!("cp {} {} -f", resolved_backup, resolved_file), config.use_sudo)?;
        info!("Recovery complete.");
        return Ok(());
    }

    if session.file_exists(&resolved_upload)? {
        if !config.silent && !ask_user(&format!("Overwrite {}?", resolved_upload)) {
            return Ok(());
        }
    }
    session.scp_upload(&local_path, &resolved_upload, config.use_sudo)?;

    if !session.file_exists(&resolved_file)? {
        return Err("Remote file does not exist".to_string());
    }
    if !config.silent && !ask_user(&format!("Backup {} to {}?", remote_file, remote_backup)) {
        return Ok(());
    }
    session.execute(&format!("cp {} {} -f", resolved_file, resolved_backup), config.use_sudo)?;

    if !config.silent && !ask_user(&format!("Overwrite {} with {}?", remote_file, resolved_upload)) {
        return Ok(());
    }
    session.execute(&format!("cp {} {} -f", resolved_upload, resolved_file), config.use_sudo)?;

    info!("Patch complete.");
    Ok(())
}