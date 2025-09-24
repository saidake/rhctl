use std::fs::File;
use std::io::Read; // Added Read import
use std::path::Path;

use log::{error, info};

use crate::common::utils::{connect_ssh, generate_temp_dir};
use crate::domain::cmd_params::ExecuteCmdConfig;

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

    if config.use_sudo {
        let temp_remote = generate_temp_dir("exec");
        info!(
            "Uploading script '{}' to temporary path '{}'",
            config.script, temp_remote
        );
        session.do_upload_with_scp(script_path, &temp_remote, false)?;
        info!("Executing script in '{}' with sudo", config.remote_path);
        session.execute_stream(
            &format!("cd {} && bash {}", config.remote_path, temp_remote),
            true,
            |line, is_stderr| {
                if is_stderr {
                    error!("{}", line);
                    std::process::exit(1);
                } else {
                    info!("{}", line);
                }
                Ok(())
            },
        )?;
        info!("Cleaning up temporary script '{}'", temp_remote);
        session.execute(&format!("rm -f {}", temp_remote), true)?;
    } else {
        let mut content = String::new();
        File::open(script_path)
            .map_err(|e| format!("Failed to open script '{}'. \n\t{}", config.script, e))?
            .read_to_string(&mut content)
            .map_err(|e| format!("Failed to read script '{}'. \n\t{}", config.script, e))?;
        info!("Executing script in '{}'", config.remote_path);
        session.execute_stream(
            &format!(
                "cd {} && bash -l -s <<EOF\n{}\nEOF",
                config.remote_path, content
            ),
            false,
            |line, is_stderr| {
                if is_stderr {
                    error!("{}", line);
                    std::process::exit(1);
                } else {
                    info!("{}", line);
                }
                Ok(())
            },
        )?;
    }

    info!("Execution complete.");
    Ok(())
}
