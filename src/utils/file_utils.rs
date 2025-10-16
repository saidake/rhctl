/*
 * Copyright 2025 the original author or authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * **************************************************************************
 * File operation utils.
 * 
 * Author: Craig Brown
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use crate::domain::cmd_params::ServerMetadata;
use crate::domain::constants::REMOTE_TEMP_SBXCTL_FOLDER;
use crate::domain::yml_config::YmlConfig;
use crate::log_local;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

pub fn load_yaml_config(path: &str) -> Result<YmlConfig, String> {
    let mut file =
        File::open(path).map_err(|e| format!("Failed to open config file {}. \n\t> {}", path, e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read config file {}. \n\t> {}", path, e))?;
    // println!("contents: {:?}", contents);
    serde_yaml::from_str(&contents)
        .map_err(|e| format!("Failed to parse YAML config {}. \n\t> {}", path, e))
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

pub fn log_local_file_info(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    local_path: &Path,
) -> Result<(), String> {
    let ls_local = Command::new("ls")
        .arg("-al")
        .arg(local_path)
        .output()
        .map_err(|e| {
            format!(
                "Failed to execute ls -al on '{}'. \n\t> {}",
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
        // Replace `log_local` with your actual logging macro
        log_local!(server_metadata, task_name, "{}", line);
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

pub fn load_properties(
    file: &str,
    mappings: &mut HashMap<String, String>,
    vars: &HashMap<String, String>,
) -> Result<(), String> {
    // println!("vars: {:?}", vars);
    let file = substitute_vars(file, vars)?;
    let f = File::open(&file).map_err(|e| format!("Error opening file '{}'. \n\t> {}", file, e))?;
    for (line_num, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("Error reading line {}. \n\t> {}", line_num + 1, e))?;
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
        // println!("parts[0].trim(): {}, parts[1].trim(): {}",parts[0].trim(), parts[1].trim());
        let local_file_or_dir = substitute_vars(parts[0].trim(), vars)?;
        let target_path = substitute_vars(parts[1].trim(), vars)?;
        // println!("local_file_or_dir: {}, target_path: {}",local_file_or_dir, target_path);
        if local_file_or_dir.is_empty() || target_path.is_empty() {
            return Err(format!(
                "Empty local or target path at line {}: '{}'",
                line_num + 1,
                line
            ));
        }
        mappings.insert(local_file_or_dir, target_path);
    }
    Ok(())
}

pub fn substitute_vars(input_path: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    if input_path.trim().is_empty() {
        return Err("Input path is empty".to_string());
    }
    // println!("vars: {:?}", vars);
    let mut result = String::new();
    let mut rest = input_path;

    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];

        if let Some(end) = rest.find('}') {
            let key = &rest[..end];
            match vars.get(key) {
                Some(value) => {
                    if value.is_empty() {
                        return Err(format!(
                            "Variable '{}' is empty in path '{}'",
                            key, input_path
                        ));
                    }
                    result.push_str(value);
                }
                None => {
                    return Err(format!(
                        "Variable '{}' not provided in path '{}'",
                        key, input_path
                    ));
                }
            }
            rest = &rest[end + 1..];
        } else {
            return Err(format!(
                "Unclosed variable placeholder in path '{}'",
                input_path
            ));
        }
    }

    result.push_str(rest);

    // Invalid characters check (macOS / Linux only)
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let forbidden: &[char] = &['<', '>', ':', '"', '|', '?', '*'];
        if result.chars().any(|c| forbidden.contains(&c)) {
            return Err(format!("Path '{}' contains invalid characters.", result));
        }
    }

    Ok(result)
}
