use std::fs::File;
use std::io::Read;
use std::path::Path;

use log::info;

use crate::common::config::Config;
use crate::common::ssh::SshSession;
use crate::common::utils::generate_temp_path;

pub fn run(session: &SshSession, config: &Config, script: &str, remote_path: &str) -> Result<(), String> {
    let script_path = Path::new(script);
    if !script_path.exists() || !script_path.is_file() {
        return Err("Script file does not exist".to_string());
    }

    if config.sudo {
        // Upload to temp and execute with sudo
        let temp_remote = generate_temp_path("exec");
        session.scp_upload(script_path, &temp_remote, false)?;
        session.execute(&format!("cd {} && bash {}", remote_path, temp_remote), true)?;
        session.execute(&format!("rm -f {}", temp_remote), true)?;
    } else {
        // Execute directly by piping script content
        let mut content = String::new();
        File::open(script_path).map_err(|e| e.to_string())?.read_to_string(&mut content).map_err(|e| e.to_string())?;
        session.execute(&format!("cd {} && bash -l -s <<EOF\n{}\nEOF", remote_path, content), false)?;
    }

    info!("Execution complete.");
    Ok(())
}