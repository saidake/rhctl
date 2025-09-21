use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use log::{debug, error, info, warn};

use crate::common::config::{Config, ConfigWrapper};
use crate::common::ssh::SshSession;
use crate::common::utils::{ask_user, resolve_remote_path};

pub fn run(
    session: &SshSession,
    config: &ConfigWrapper,
    properties_file: &str,
    assets_root: &str,
) -> Result<(), String> {
    let mut mappings = HashMap::new();
    if let Err(e) = load_properties(properties_file, &mut mappings) {
        return Err(format!(
            "Failed to load properties file '{}': {}",
            properties_file, e
        ));
    }

    let assets_path = Path::new(assets_root);
    let threads = Arc::new(Mutex::new(Vec::new()));

    for (local_item, remote_dir) in mappings {
        let local_path = assets_path.join(&local_item);
        if !local_path.exists() {
            warn!(
                "Local item '{}' not found in assets directory '{}'. Skipping.",
                local_item, assets_root
            );
            continue;
        }

        let remote_dir_resolved = resolve_remote_path(session, config, &remote_dir)?;
        if remote_dir_resolved.is_empty() {
            return Err(format!(
                "Failed to resolve remote directory '{}'",
                remote_dir
            ));
        }
        let remote_dir_resolved = remote_dir_resolved.trim_end().to_string();
        // println!("remote_dir_resolved: '{}'",remote_dir_resolved);
        // Check if remote directory is writable
        if let Err(e) =
            session.check_directory_writable(&remote_dir_resolved, config.upload.use_sudo)
        {
            return Err(format!(
                "Remote directory '{}' is not writable: {}",
                remote_dir_resolved, e
            ));
        }

        let config_clone = config.clone();
        let session = SshSession::new(&config_clone)
            .map_err(|e| format!("Failed to create SSH session for '{}': {}", local_item, e))?;
        let local_path_clone = local_path.clone();
        let remote_dir_clone = remote_dir_resolved.clone();

        let handle = thread::spawn(move || {
            upload_file_or_dir(
                &session,
                &config_clone,
                &local_path_clone,
                &remote_dir_clone,
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

    info!("Upload complete.");
    Ok(())
}

fn load_properties(file: &str, mappings: &mut HashMap<String, String>) -> Result<(), String> {
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
    config: &ConfigWrapper,
    local_path: &PathBuf,
    remote_dir: &str,
) -> Result<(), String> {
    // Create remote directory with appropriate permissions
    let mkdir_cmd = if config.upload.use_sudo {
        format!("mkdir -p {} && chmod 755 {}", remote_dir, remote_dir)
    } else {
        format!("mkdir -p {}", remote_dir)
    };
    if let Err(e) = session.execute(&mkdir_cmd, config.upload.use_sudo) {
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
                if !config.upload.silent
                    && !ask_user(&format!(
                        "Remote file '{}' already exists. Overwrite? (y/n): ",
                        remote_sub
                    ))
                {
                    continue;
                }
            }

            upload_single(session, config, &sub_path, &remote_sub)?;
            info!(
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
            if !config.upload.silent
                && !ask_user(&format!(
                    "Remote file '{}' already exists. Overwrite? (y/n): ",
                    remote_file
                ))
            {
                return Ok(());
            }
        }

        upload_single(session, config, local_path, &remote_file)?;
        info!(
            "Successfully uploaded '{}' to '{}'",
            local_path.display(),
            remote_file
        );
    }

    Ok(())
}

fn upload_single(
    session: &SshSession,
    config: &ConfigWrapper,
    local_path: &PathBuf,
    remote_path: &str,
) -> Result<(), String> {
    debug!(
        "Attempting to upload '{}' to '{}'",
        local_path.display(),
        remote_path
    );
    if config.upload.use_rsync && command_exists("rsync") {
        debug!("Using rsync for upload");
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
        debug!("Using SCP for upload");
        session.scp_upload(local_path, remote_path, config.upload.use_sudo)?;
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
