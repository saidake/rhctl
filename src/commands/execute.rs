use std::fs::File;
use std::io::Read; // Added Read import
use std::path::Path;

use log::{debug, error, info};

use crate::common::utils::{connect_ssh, generate_temp_dir};
use crate::domain::cmd_params::ExecuteCmdConfig;
use crate::remote;

pub fn run(config: &ExecuteCmdConfig) -> Result<(), String> {
    let session = connect_ssh(
        config.host.clone(),
        config.user.clone(),
        config.ssh_port,
        config.password.clone(),
    );
    let script_path = Path::new(&config.script);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!(
            "Script file '{}' does not exist or is not a file",
            config.script
        ));
    }
    let script_name = script_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Failed to get basename for '{}'", &config.script))?;

    if config.use_sudo {
        session.check_global_remote_temp_dir(config.use_sudo, config.silent)?;
        let temp_remote_dir = generate_temp_dir("exec");
        debug!(
            "Uploading script '{}' to temporary path '{}'",
            config.script, temp_remote_dir
        );
        session.upload_file_or_dir_into_temp_dir(
            script_path,
            &temp_remote_dir,
            config.use_sudo,
            config.use_rsync,
            config.silent,
        )?;
        let remote_script = format!("{}/{}", temp_remote_dir, script_name);
        info!(
            "Executing script {} in '{}' with sudo",
            script_name, config.remote_path
        );
        session.execute_stream(
            &format!("cd {} && bash {}", config.remote_path, remote_script),
            config.use_sudo,
        )?;
        debug!("Cleaning up temporary script '{}'", temp_remote_dir);
        session.execute(&format!("rm -rf {}", temp_remote_dir), config.use_sudo)?;
        session.delete_global_temp_dir(config.use_sudo)?;
    } else {
        let mut content = String::new();
        File::open(script_path)
            .map_err(|e| format!("Failed to open script '{}'. \n\t{}", config.script, e))?
            .read_to_string(&mut content)
            .map_err(|e| format!("Failed to read script '{}'. \n\t{}", config.script, e))?;
        info!("Executing script in '{}': ", config.remote_path);
        session.execute_stream(
            &format!(
                "cd {} && bash -l -s <<EOF\n{}\nEOF",
                config.remote_path, content
            ),
            false,
        )?;
    }

    info!("Execution complete.");
    Ok(())
}
