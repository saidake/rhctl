use std::fs::File;
use std::io::Read; // Added Read import
use std::path::Path;

use log::{debug, info};

use crate::common::ssh::ServerHandle;
use crate::domain::cmd_params::ExecuteCmdConfig;

pub async fn run(config: &ExecuteCmdConfig, session: &ServerHandle<ExecuteCmdConfig>) -> Result<(), String> {
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
        let temp_remote_dir=session.create_remote_temp_dir("exec",config.use_sudo).await?;
        debug!(
            "Uploading script '{}' to temporary path '{}'",
            config.script, temp_remote_dir
        );
        session.upload_file_or_dir_contents_into_dir(
            script_path,
            &temp_remote_dir,
            None,
            config.use_sudo,
            config.use_rsync,
            config.silent,
            true,
            false
        ).await?;
        let remote_script = format!("{}/{}", temp_remote_dir, script_name);
        info!(
            "Executing script {} in '{}' with sudo",
            script_name, config.remote_path
        );
        session.exec_with_log(
            &format!("cd {} && bash {}", config.remote_path, remote_script),
            config.use_sudo
        ).await?;
        debug!("Cleaning up temporary script '{}'", temp_remote_dir);
        session.exec(&format!("rm -rf {}", temp_remote_dir), config.use_sudo).await?;
    } else {
        let mut content = String::new();
        File::open(script_path)
            .map_err(|e| format!("Failed to open script '{}'. \n\t{}", config.script, e))?
            .read_to_string(&mut content)
            .map_err(|e| format!("Failed to read script '{}'. \n\t{}", config.script, e))?;
        info!("Executing script in '{}': ", config.remote_path);
        session.exec_with_log(
            &format!(
                "cd {} && bash -l -s <<EOF\n{}\nEOF",
                config.remote_path, content
            ),
            false,
        ).await?;
    }

    info!("Execution complete.");
    Ok(())
}
