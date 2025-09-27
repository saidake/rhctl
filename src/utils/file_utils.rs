use crate::domain::constants::REMOTE_TEMP_SBXCTL_FOLDER;
use crate::domain::yml_config::YmlConfig;
use crate::local;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

pub fn load_yaml_config(path: &str) -> Result<YmlConfig, String> {
    let mut file =
        File::open(path).map_err(|e| format!("Failed to open config file {}. \n\t{}", path, e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read config file {}. \n\t{}", path, e))?;
    serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse YAML config {}. \n\t{}", path, e))
}

pub fn generate_remote_temp_dir(prefix: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{}/{}_{:x}", REMOTE_TEMP_SBXCTL_FOLDER, prefix, timestamp)
}
pub fn split_unix_path(path: &str) -> Result<(String, String), String> {
    let path: &str = path.trim_end_matches('/');

    match path.rsplit_once('/') {
        Some((parent, basename)) => Ok((basename.to_string(), parent.to_string())),
        None => Err(format!("Invalid unix path: {}", path)),
    }
}

pub fn log_local_file_info(local_path: &Path) -> Result<(), String> {
    let ls_local = Command::new("ls")
        .arg("-al")
        .arg(local_path)
        .output()
        .map_err(|e| {
            format!(
                "Failed to execute ls -al on '{}'. \n\t{}",
                local_path.display(),
                e
            )
        })?;

    if !ls_local.status.success() {
        return Err(format!(
            "Failed to get local file info for '{}'",
            local_path.display()
        ));
    }

    for line in String::from_utf8_lossy(&ls_local.stdout).lines() {
        // Replace `local!` with your actual logging macro
        local!("{}", line);
    }

    Ok(())
}

pub fn get_local_path_base_name(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    let os_str: Option<&OsStr> = if path.is_dir() {
        path.file_name()
            .or_else(|| path.components().last().map(|c| c.as_os_str()))
    } else if path.is_file() {
        path.file_name()
    } else {
        None
    };

    os_str
        .and_then(|s| s.to_str())
        .map(|s| s.trim_end().to_string())
        .ok_or_else(|| format!("Could not get base name: {}", path.display()))
}
