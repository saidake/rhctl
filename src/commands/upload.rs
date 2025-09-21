use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use log::{error, info, warn};

use crate::common::config::{ConfigWrapper, UploadConfig};
use crate::common::ssh::SshSession;
use crate::common::utils::{ask_user, resolve_remote_path};

pub fn run(session: &SshSession, config: &ConfigWrapper, properties_file: &str, assets_root: &str) -> Result<(), String> {
    let mut mappings = HashMap::new();
    load_properties(properties_file, &mut mappings)?;

    let assets_path = Path::new(assets_root);
    let threads = Arc::new(Mutex::new(Vec::new()));

    for (local_item, remote_dir) in mappings {
        let local_path = assets_path.join(&local_item);
        if !local_path.exists() {
            warn!("Item '{}' not found in assets. Skipping.", local_item);
            continue;
        }

        let remote_dir_resolved = resolve_remote_path(session, config, &remote_dir)?;
        let config_clone = config.clone();
        let session_clone = session.clone();
        let local_path_clone = local_path.clone();
        let remote_dir_clone = remote_dir_resolved.clone();

        let handle = thread::spawn(move || {
            upload_file_or_dir(&session_clone, &config_clone, &local_path_clone, &remote_dir_clone)
                .map_err(|e| format!("Upload failed for {}: {}", local_item, e))
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
    let f = File::open(file).map_err(|e| e.to_string())?;
    for line in io::BufReader::new(f).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            mappings.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
        }
    }
    Ok(())
}

fn upload_file_or_dir(session: &SshSession, config: &ConfigWrapper, local_path: &PathBuf, remote_dir: &str) -> Result<(), String> {
    session.execute(&format!("mkdir -p {}", remote_dir), config.use_sudo)?;

    if local_path.is_dir() {
        for entry in std::fs::read_dir(local_path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let sub_path = entry.path();
            let base_name = sub_path.file_name().unwrap().to_str().unwrap();
            let remote_sub = format!("{}/{}", remote_dir, base_name);

            if session.file_exists(&remote_sub)? {
                if !config.silent && !ask_user(&format!("{} exists. Overwrite? (y/n): ", base_name)) {
                    continue;
                }
            }

            upload_single(session, config, &sub_path, &remote_sub)?;
        }
    } else {
        let base_name = local_path.file_name().unwrap().to_str().unwrap();
        let remote_file = format!("{}/{}", remote_dir, base_name);

        if session.file_exists(&remote_file)? {
            if !config.silent && !ask_user(&format!("{} exists. Overwrite? (y/n): ", base_name)) {
                return Ok(());
            }
        }

        upload_single(session, config, local_path, &remote_file)?;
    }

    Ok(())
}

fn upload_single(session: &SshSession, config: &ConfigWrapper, local_path: &PathBuf, remote_path: &str) -> Result<(), String> {
    if config.use_rsync && command_exists("rsync") {
        let status = std::process::Command::new("rsync")
            .arg("-avz")
            .arg("-e")
            .arg(format!("ssh -p {}", session.port))
            .arg(local_path.to_str().unwrap())
            .arg(format!("{}@{}:{}", session.user, session.host, remote_path))
            .env("RSYNC_PASSWORD", &session.password)
            .status()
            .map_err(|e| e.to_string())?;

        if !status.success() {
            return Err("rsync failed".to_string());
        }
    } else {
        session.scp_upload(local_path, remote_path, config.use_sudo)?;
    }
    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which").arg(cmd).status().map(|s| s.success()).unwrap_or(false)
}