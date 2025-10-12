use std::fs::File;
use std::io::Read; // Added Read import
use std::path::Path;
use std::sync::Arc;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{ExecuteCmdConfig, ServerMetadata};
use crate::domain::constants::EXECUTE_TASK_NAME;
use crate::{log_debug, log_info};
pub async fn run(
    config: &ExecuteCmdConfig,
    server_metadata: &Arc<ServerMetadata>,
    global_server_pool: Arc<ServerPool>,
) -> Result<(), String> {
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
        let temp_remote_dir = global_server_pool
            .create_remote_temp_dir(server_metadata, EXECUTE_TASK_NAME,"exec", config.use_sudo)
            .await?;
        log_debug!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "Uploading script '{}' to temporary path '{}'",
            config.script,
            temp_remote_dir
        );
        global_server_pool
            .upload_file_or_dir_contents_into_dir(
                server_metadata,
                EXECUTE_TASK_NAME,
                script_path,
                &temp_remote_dir,
                None,
                config.use_sudo,
                config.use_rsync,
                config.silent,
                true,
                false,
            )
            .await?;
        let remote_script = format!("{}/{}", temp_remote_dir, script_name);
        log_info!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "Executing script {} in '{}' with sudo",
            script_name,
            config.remote_path
        );
        global_server_pool
            .exec_with_log(
                server_metadata,EXECUTE_TASK_NAME,
                &format!("cd {} && bash {}", config.remote_path, remote_script),
                config.use_sudo,
            )
            .await?;
        log_debug!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "Cleaning up temporary script '{}'",
            temp_remote_dir
        );
        global_server_pool
            .exec(
                server_metadata,EXECUTE_TASK_NAME,
                &format!("rm -rf {}", temp_remote_dir),
                config.use_sudo,
            )
            .await?;
    } else {
        let mut content = String::new();
        File::open(script_path)
            .map_err(|e| format!("Failed to open script '{}'. \n\t> {}", config.script, e))?
            .read_to_string(&mut content)
            .map_err(|e| format!("Failed to read script '{}'. \n\t> {}", config.script, e))?;
        log_info!(
            server_metadata,
            EXECUTE_TASK_NAME,
            "Executing script in '{}': ",
            config.remote_path
        );
        global_server_pool
            .exec_with_log(
                server_metadata,EXECUTE_TASK_NAME,
                &format!(
                    "cd {} && bash -l -s <<EOF\n{}\nEOF",
                    config.remote_path, content
                ),
                false,
            )
            .await?;
    }

    log_info!(server_metadata, EXECUTE_TASK_NAME, "Execution complete.");
    Ok(())
}
