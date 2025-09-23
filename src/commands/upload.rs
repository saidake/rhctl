use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::common::ssh::SshSession;
use crate::common::utils::{ask_user, connect_ssh, resolve_remote_path};
use crate::domain::cmd_params::UploadCmdConfig;
use crate::{log_debug_with_lock, log_info_with_lock, log_warn_with_lock};

pub fn run(config: &UploadCmdConfig) -> Result<(), String> {
    let session = connect_ssh(config.host.clone(), config.user.clone(), config.ssh_port, config.password.clone());
    let mut mappings = HashMap::new();
    if let Err(e) = load_properties(config.properties_file.clone(), &mut mappings) {
        return Err(format!(
            "Failed to load properties file '{}': {}",
            config.properties_file, e
        ));
    }

    let assets_path = Path::new(&config.assets_root);
    let threads = Arc::new(Mutex::new(Vec::new()));

    for (local_item, remote_dir) in mappings {
        let local_path = assets_path.join(&local_item);
        if !local_path.exists() {
            log_warn_with_lock!(
                "Local item '{}' not found in assets directory '{}'. Skipping.",
                local_item,
                config.assets_root
            );
            continue;
        }

        let remote_dir_resolved = resolve_remote_path(&session, config.use_sudo, &remote_dir)?;
        if remote_dir_resolved.is_empty() {
            return Err(format!(
                "Failed to resolve remote directory '{}'",
                remote_dir
            ));
        }
        // println!("remote_dir_resolved: '{}'",remote_dir_resolved);
        // Check if remote directory is writable
        if let Err(e) = session.check_directory_writable(&remote_dir_resolved, config.use_sudo) {
            return Err(format!(
                "Remote directory '{}' is not writable: {}",
                remote_dir_resolved, e
            ));
        }

        let local_path_clone = local_path.clone();
        let remote_dir_clone = remote_dir_resolved.clone();
        let session_clone = session.clone();
        let config_clone = config.clone();
        let handle = thread::spawn(move || {
            upload_file_or_dir(
                &session_clone,
                &local_path_clone,
                &remote_dir_clone,
                config_clone.use_sudo,
                config_clone.use_rsync,
                config_clone.silent,
            )
            .map_err(|e| format!("Failed to upload '{}': {}", local_path_clone.display(), e))
        });

        threads.lock().unwrap().push(handle);
    }

    let mut errors = Vec::new();
    for handle in threads.lock().unwrap().drain(..) {
        if let Err(e) = handle.join().unwrap() {
            errors.push(e);
        }
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    log_info_with_lock!("Upload complete.");
    Ok(())
}

fn load_properties(file: String, mappings: &mut HashMap<String, String>) -> Result<(), String> {
    let f = File::open(file).map_err(|e| format!("Error opening file: {}", e))?;
    for (line_num, line) in io::BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("Error reading line {}: {}", line_num + 1, e))?;
        let line = line.trim_end_matches(|c| c == '\n' || c == '\r' || c == ' ' || c == '\t');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid format at line {}: '{}'",
                line_num + 1,
                line
            ));
        }
        let local_path = parts[0].trim();
        let target_path = parts[1].trim();
        // println!("target_path: '{}'",target_path);
        if local_path.is_empty() || target_path.is_empty() {
            return Err(format!(
                "Empty local or target path at line {}: '{}'",
                line_num + 1,
                line
            ));
        }
        mappings.insert(local_path.to_string(), target_path.to_string());
    }
    Ok(())
}

fn upload_file_or_dir(
    session: &SshSession,
    local_path: &PathBuf,
    remote_dir: &str,
    use_sudo: bool,
    use_rsync: bool,
    silent: bool,
) -> Result<(), String> {
    // Create remote directory with appropriate permissions
    let mkdir_cmd = if use_sudo {
        format!("mkdir -p {} && chmod 755 {}", remote_dir, remote_dir)
    } else {
        format!("mkdir -p {}", remote_dir)
    };
    if let Err(e) = session.execute(&mkdir_cmd, use_sudo) {
        return Err(format!(
            "Failed to create remote directory '{}': {}",
            remote_dir, e
        ));
    }

    if local_path.is_dir() {
        for entry in std::fs::read_dir(local_path)
            .map_err(|e| format!("Failed to read directory '{}': {}", local_path.display(), e))?
        {
            let entry = entry.map_err(|e| format!("Error reading directory entry: {}", e))?;
            let sub_path = entry.path();
            let base_name = sub_path
                .file_name()
                .ok_or("Invalid file name")?
                .to_str()
                .ok_or("Invalid file name encoding")?;
            let remote_sub = format!("{}/{}", remote_dir, base_name);

            if session.file_exists(&remote_sub)? {
                if !silent
                    && !ask_user(&format!(
                        "Remote file '{}' already exists. Overwrite?",
                        remote_sub
                    ))
                {
                    continue;
                }
            }

            upload_single(session, use_sudo, use_rsync, &sub_path, &remote_sub)?;
            log_info_with_lock!(
                "Successfully uploaded '{}' to '{}'",
                sub_path.display(),
                remote_sub
            );
        }
    } else {
        let base_name = local_path
            .file_name()
            .ok_or("Invalid file name")?
            .to_str()
            .ok_or("Invalid file name encoding")?;
        let remote_file = format!("{}/{}", remote_dir, base_name);

        if session.file_exists(&remote_file)? {
            if !silent
                && !ask_user(&format!(
                    "Remote file '{}' already exists. Overwrite?",
                    remote_file
                ))
            {
                return Ok(());
            }
        }

        upload_single(session, use_sudo, use_rsync, local_path, &remote_file)?;
        log_info_with_lock!(
            "Successfully uploaded '{}' to '{}'",
            local_path.display(),
            remote_file
        );
    }

    Ok(())
}

fn upload_single(
    session: &SshSession,
    use_sudo: bool,
    use_rsync: bool,
    local_path: &PathBuf,
    remote_path: &str,
) -> Result<(), String> {
    log_debug_with_lock!(
        "Attempting to upload '{}' to '{}'",
        local_path.display(),
        remote_path
    );
    if use_rsync && command_exists("rsync") {
        log_debug_with_lock!("Using rsync for upload");
        let status = std::process::Command::new("rsync")
            .arg("-avz")
            .arg("-e")
            .arg(format!("ssh -p {}", session.port))
            .arg(local_path.to_str().ok_or("Invalid local path encoding")?)
            .arg(format!("{}@{}:{}", session.user, session.host, remote_path))
            .env("RSYNC_PASSWORD", &session.password)
            .status()
            .map_err(|e| format!("Failed to execute rsync: {}", e))?;

        if !status.success() {
            return Err(format!(
                "rsync failed with exit code {}",
                status.code().unwrap_or(-1)
            ));
        }
    } else {
        log_debug_with_lock!("Using SCP for upload");
        session.scp_upload(local_path, remote_path, use_sudo)?;
    }
    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
