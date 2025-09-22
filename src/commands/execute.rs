use std::fs::File;
use std::io::{ Read};  // Added Read import
use std::path::Path;

use log::{error, info};

use crate::common::config::{ConfigWrapper};
use crate::common::ssh::SshSession;
use crate::common::utils::generate_temp_path;

pub fn run(session: &SshSession, config: &ConfigWrapper, script: &str, remote_path: &str) -> Result<(), String> {
    let script_path = Path::new(script);
    if !script_path.exists() || !script_path.is_file() {
        return Err(format!("Script file '{}' does not exist or is not a file", script));
    }

    if config.execute.use_sudo {
        let temp_remote = generate_temp_path("exec");
        info!("Uploading script '{}' to temporary path '{}'", script, temp_remote);
        session.scp_upload(script_path, &temp_remote, false)?;
        info!("Executing script in '{}' with sudo", remote_path);
        session.execute_stream(&format!("cd {} && bash {}", remote_path, temp_remote), true, |line, is_stderr| {
            if is_stderr {
                error!("{}", line);
            } else {
                info!("{}", line);
            }
            Ok(())
        })?;
        info!("Cleaning up temporary script '{}'", temp_remote);
        session.execute(&format!("rm -f {}", temp_remote), true)?;
    } else {
        let mut content = String::new();
        File::open(script_path)
            .map_err(|e| format!("Failed to open script '{}': {}", script, e))?
            .read_to_string(&mut content)
            .map_err(|e| format!("Failed to read script '{}': {}", script, e))?;
        info!("Executing script in '{}'", remote_path);
        session.execute_stream(&format!("cd {} && bash -l -s <<EOF\n{}\nEOF", remote_path, content), false, |line, is_stderr| {
            if is_stderr {
                error!("{}", line);
            } else {
                info!("{}", line);
            }
            Ok(())
        })?;
    }

    info!("Execution complete.");
    Ok(())
}