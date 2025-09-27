use std::collections::HashMap;
use std::path::{Path};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::domain::cmd_params::UploadCmdConfig;
use crate::utils::ssh_utils::{connect_ssh, connect_ssh_thread, resolve_remote_path};
use crate::{log_info_with_lock, log_warn_with_lock};

pub fn run(config: &UploadCmdConfig) -> Result<(), String> {
    let session = connect_ssh(
        config.host.clone(),
        config.user.clone(),
        config.ssh_port,
        config.password.clone(),
    );
    // Ensure global temp dir exists before starting uploads
    session.check_global_remote_temp_dir(config.use_sudo, config.silent)?;
    let mut mappings = HashMap::new();
    if let Err(e) = session.load_properties(config.properties_file.as_str(), &mut mappings) {
        return Err(format!(
            "Failed to load properties file '{}'. \n\t{}",
            config.properties_file, e
        ));
    }

    let assets_path = Path::new(&config.assets_root);
    let threads = Arc::new(Mutex::new(Vec::new()));

    for (local_item, remote_dir) in mappings {
        let local_file_or_dir = assets_path.join(&local_item);
        if !local_file_or_dir.exists() {
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

        // Check if remote directory is writable
        session.validate_remote_dir(&remote_dir_resolved, config.use_sudo)?;
        session.create_remote_dir(&remote_dir_resolved, config.use_sudo)?;
        
        let local_file_or_dir_clone = local_file_or_dir.clone();
        let remote_dir_clone = remote_dir_resolved.clone();
        let config_clone = config.clone();

        // spawn upload thread
        let handle = thread::spawn(move || {
            let session_thread = connect_ssh_thread(
                config_clone.host.clone(),
                config_clone.user.clone(),
                config_clone.ssh_port,
                config_clone.password.clone(),
            );
            session_thread
                .upload_file_or_dir_contents_into_dir(
                    &local_file_or_dir_clone,
                    &remote_dir_clone,
                    None,
                    config_clone.use_sudo,
                    config_clone.use_rsync,
                    config_clone.silent,
                    false,
                    true
                )
                .map_err(|e| {
                    format!(
                        "Failed to upload '{}' to remote directory '{}' . \n\t{}",
                        local_file_or_dir_clone.display(),
                        remote_dir_clone,
                        e
                    )
                })
        });

        // store thread handle
        threads.lock().unwrap().push(handle);
    }

    // collect thread results
    let mut errors = Vec::new();
    for handle in threads.lock().unwrap().drain(..) {
        if let Err(e) = handle.join().unwrap() {
            errors.push(e);
        }
    }

    // after all uploads done, clean up global temp dir
    session.delete_global_temp_dir(config.use_sudo)?;

    if !errors.is_empty() {
        return Err(errors.join("\n\n\t"));
    }

    log_info_with_lock!("Upload complete.");
    Ok(())
}
