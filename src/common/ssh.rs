use ssh2::Session;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::common::utils::{ask_user, connect_ssh, connect_ssh_thread, resolve_remote_path};
use crate::domain::cmd_params::UploadCmdConfig;
use crate::{log_debug_with_lock, log_info_with_lock, log_warn_with_lock};
use log::{debug, info};

#[derive(Clone)]
pub struct SshSession {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    session: Session,
}

impl SshSession {
    pub fn new(
        host: String,
        user: String,
        ssh_port: u16,
        password: String,
    ) -> Result<Self, String> {
        debug!("Connecting to {}:{} as {}", host, ssh_port, user);
        let tcp = TcpStream::connect(format!("{}:{}", host, ssh_port))
            .map_err(|e| format!("Cannot connect to {} on port {}. \n\t{}", host, ssh_port, e))?;
        let mut sess =
            Session::new().map_err(|e| format!("Failed to create SSH session. \n\t{}", e))?;
        sess.set_tcp_stream(tcp);
        sess.handshake()
            .map_err(|e| format!("SSH handshake failed. \n\t{}", e))?;
        sess.userauth_password(&user, &password)
            .map_err(|e| format!("Authentication failed for user '{}'. \n\t{}", user, e))?;
        debug!("SSH session established successfully");
        Ok(Self {
            host: host.clone(),
            port: ssh_port,
            user: user.clone(),
            password: password.clone(),
            session: sess,
        })
    }

    pub fn execute(&self, cmd: &str, use_sudo: bool) -> Result<String, String> {
        debug!("Executing command: {} (sudo: {})", cmd, use_sudo);
        let mut channel = self
            .session
            .channel_session()
            .map_err(|e| format!("Failed to open SSH channel. \n\t{}", e))?;
        let full_cmd = if use_sudo {
            format!("echo '{}' | sudo -S -p '' bash -c '{}'", self.password, cmd)
        } else {
            cmd.to_string()
        };
        channel
            .exec(&full_cmd)
            .map_err(|e| format!("Failed to execute command '{}'. \n\t{}", cmd, e))?;
        let mut output = String::new();
        channel
            .read_to_string(&mut output)
            .map_err(|e| format!("Failed to read command output. \n\t{}", e))?;
        channel
            .wait_close()
            .map_err(|e| format!("Failed to close channel. \n\t{}", e))?;
        let exit_status = channel
            .exit_status()
            .map_err(|e| format!("Failed to get exit status. \n\t{}", e))?;
        debug!("Command exit status: {}", exit_status);
        if exit_status != 0 {
            return Err(format!(
                "Command '{}' failed with exit status {}: {}",
                cmd, exit_status, output
            ));
        }
        Ok(output)
    }

    pub fn execute_stream<F>(
        &self,
        cmd: &str,
        use_sudo: bool,
        mut callback: F,
    ) -> Result<(), String>
    where
        F: FnMut(&str, bool) -> Result<(), String>,
    {
        debug!("Streaming command: {} (sudo: {})", cmd, use_sudo);
        let mut channel = self
            .session
            .channel_session()
            .map_err(|e| format!("Failed to open SSH channel. \n\t{}", e))?;
        let full_cmd = if use_sudo {
            format!("echo '{}' | sudo -S -p '' bash -c '{}'", self.password, cmd)
        } else {
            cmd.to_string()
        };
        channel
            .exec(&full_cmd)
            .map_err(|e| format!("Failed to execute command '{}'. \n\t{}", cmd, e))?;

        // Read stdout
        let stdout = channel.stream(0);
        let stdout_reader = BufReader::new(stdout);
        for line in stdout_reader.lines() {
            let line = line.map_err(|e| format!("Failed to read stdout. \n\t{}", e))?;
            callback(&line, false)?;
        }

        // Read stderr
        let stderr = channel.stderr();
        let stderr_reader = BufReader::new(stderr);
        for line in stderr_reader.lines() {
            let line = line.map_err(|e| format!("Failed to read stderr. \n\t{}", e))?;
            callback(&line, true)?;
        }

        channel
            .wait_close()
            .map_err(|e| format!("Failed to close channel. \n\t{}", e))?;
        let exit_status = channel
            .exit_status()
            .map_err(|e| format!("Failed to get exit status. \n\t{}", e))?;
        debug!("Command exit status: {}", exit_status);
        if exit_status != 0 {
            return Err(format!(
                "Command '{}' failed with exit status {}",
                cmd, exit_status
            ));
        }
        Ok(())
    }

    pub fn file_or_dir_exists(&self, path: &str) -> Result<bool, String> {
        debug!("Checking if '{}' exists", path);
        let output = self.execute(&format!("test -e '{}'; echo $?", path), false)?;
        let exists = output.trim() == "0";
        debug!("Remote path '{}' exists: {}", path, exists);
        Ok(exists)
    }
    pub fn file_exists(&self, path: &str) -> Result<bool, String> {
        debug!("Checking if file '{}' exists", path);
        let output = self.execute(&format!("test -f '{}'; echo $?", path), false)?;
        let exists = output.trim() == "0";
        debug!("Remote file '{}' exists: {}", path, exists);
        Ok(exists)
    }

    pub fn dir_exists(&self, path: &str) -> Result<bool, String> {
        debug!("Checking if directory '{}' exists", path);
        let output = self.execute(&format!("test -d '{}'; echo $?", path), false)?;
        let exists = output.trim() == "0";
        debug!("Remote directory '{}' exists: {}", path, exists);
        Ok(exists)
    }

    pub fn check_remote_dir_writable(
        &self,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        debug!("Ensuring remote directory '{}' exists", remote_dir);

        let check_file_cmd = format!("[ -f '{}' ] && echo FILE || echo OK", remote_dir);
        let check_output = self
            .execute(&check_file_cmd, use_sudo)
            .map_err(|e| format!("Failed to check path type for '{}'. \n\t{}", remote_dir, e))?;
        if check_output.trim() == "FILE" {
            return Err(format!(
                "Path '{}' exists and is a file, not a directory",
                remote_dir
            ));
        }

        let mkdir_cmd = format!("mkdir -p '{}'", remote_dir);
        self.execute(&mkdir_cmd, use_sudo)
            .map_err(|e| format!("Failed to create directory '{}'. \n\t{}", remote_dir, e))?;

        debug!("Checking if remote directory '{}' is writable", remote_dir);

        let check_cmd = format!("test -w '{}'; echo $?", remote_dir);
        let output = self.execute(&check_cmd, use_sudo).map_err(|e| {
            format!(
                "Failed to check write permission for '{}'. \n\t{}",
                remote_dir, e
            )
        })?;
        if output.trim() != "0" {
            return Err(format!("Directory '{}' is not writable", remote_dir));
        }

        Ok(())
    }

    pub fn do_upload_with_scp(
        &self,
        local_file_or_dir: &Path,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        debug!(
            "Starting SCP upload from '{}' to '{}'",
            local_file_or_dir.display(),
            remote_dir
        );
        if !local_file_or_dir.exists() {
            return Err(format!(
                "Local file '{}' does not exist",
                local_file_or_dir.display()
            ));
        }
        if use_sudo {
            let temp_dir = super::utils::generate_temp_dir("upload");
            debug!("Uploading to temporary path '{}' with sudo", temp_dir);
            self.do_upload_with_scp2(local_file_or_dir, &temp_dir)?;
            self.execute(&format!("mv '{}' '{}'", temp_dir, remote_dir), true)?;
        } else {
            self.do_upload_with_scp2(local_file_or_dir, remote_dir)?;
        }
        debug!("SCP upload to '{}' completed", remote_dir);
        Ok(())
    }

    fn do_upload_with_scp2(
        &self,
        local_file_or_dir: &Path,
        remote_path: &str,
    ) -> Result<(), String> {
        let mut file = std::fs::File::open(local_file_or_dir).map_err(|e| {
            format!(
                "Failed to open local file '{}'. \n\t{}",
                local_file_or_dir.display(),
                e
            )
        })?;
        let stat = file.metadata().map_err(|e| {
            format!(
                "Failed to get metadata for '{}'. \n\t{}",
                local_file_or_dir.display(),
                e
            )
        })?;
        debug!("File size: {} bytes, permissions: 0644", stat.len());
        let mut channel = self
            .session
            .scp_send(Path::new(remote_path), 0o644, stat.len(), None)
            .map_err(|e| {
                format!(
                    "Failed to initiate SCP upload to '{}'. \n\t{}",
                    remote_path, e
                )
            })?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| {
            format!(
                "Failed to read file '{}'. \n\t{}",
                local_file_or_dir.display(),
                e
            )
        })?;
        channel.write_all(&buffer).map_err(|e| {
            format!(
                "Failed to write to SCP channel for '{}'. \n\t{}",
                remote_path, e
            )
        })?;
        channel.send_eof().map_err(|e| {
            format!(
                "Failed to send EOF for SCP upload to '{}'. \n\t{}",
                remote_path, e
            )
        })?;
        channel.wait_eof().map_err(|e| {
            format!(
                "Failed to wait for EOF for SCP upload to '{}'. \n\t{}",
                remote_path, e
            )
        })?;
        channel.close().map_err(|e| {
            format!(
                "Failed to close SCP channel for '{}'. \n\t{}",
                remote_path, e
            )
        })?;
        channel.wait_close().map_err(|e| {
            format!(
                "Failed to wait for SCP channel close for '{}'. \n\t{}",
                remote_path, e
            )
        })?;
        Ok(())
    }

    fn command_exists(&self, cmd: &str) -> bool {
        std::process::Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn do_upload(
        &self,
        use_sudo: bool,
        use_rsync: bool,
        local_file_or_dir: &PathBuf,
        remote_dir: &str,
    ) -> Result<(), String> {
        log_debug_with_lock!(
            "Attempting to upload '{}' to '{}'",
            local_file_or_dir.display(),
            remote_dir
        );

        if use_rsync && self.command_exists("rsync") {
            log_debug_with_lock!("Using rsync for upload");
            if !self.password.is_empty() {
                if self.command_exists("sshpass") {
                    let mut sshpass_cmd = std::process::Command::new("sshpass");
                    sshpass_cmd
                        .arg("-p")
                        .arg(&self.password)
                        .arg("rsync")
                        .arg("-avz")
                        .arg("-e")
                        .arg(format!("ssh -p {}", self.port))
                        .arg(
                            local_file_or_dir
                                .to_str()
                                .ok_or("Invalid local path encoding")?,
                        )
                        .arg(format!("{}@{}:{}", self.user, self.host, remote_dir))
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());

                    let status = sshpass_cmd
                        .status()
                        .map_err(|e| format!("Failed to execute rsync via sshpass. \n\t{}", e))?;

                    if !status.success() {
                        return Err(format!(
                            "rsync (sshpass) failed with exit code {}",
                            status.code().unwrap_or(-1)
                        ));
                    }
                    return Ok(());
                } else {
                    return Err("rsync password required but sshpass not found".to_string());
                }
            }
        } else {
            log_debug_with_lock!("Using SCP for upload");
            self.do_upload_with_scp(local_file_or_dir, remote_dir, use_sudo)?;
        }

        Ok(())
    }

    pub fn load_properties(
        &self,
        file: &str,
        mappings: &mut HashMap<String, String>,
    ) -> Result<(), String> {
        let f = File::open(file).map_err(|e| format!("Error opening file. \n\t{}", e))?;
        for (line_num, line) in BufReader::new(f).lines().enumerate() {
            let line =
                line.map_err(|e| format!("Error reading line {}. \n\t{}", line_num + 1, e))?;
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
            let local_file_or_dir = parts[0].trim();
            let target_path = parts[1].trim();
            // println!("target_path: '{}'",target_path);
            if local_file_or_dir.is_empty() || target_path.is_empty() {
                return Err(format!(
                    "Empty local or target path at line {}: '{}'",
                    line_num + 1,
                    line
                ));
            }
            mappings.insert(local_file_or_dir.to_string(), target_path.to_string());
        }
        Ok(())
    }

    pub fn upload_file_or_dir_into_dir(
        &self,
        local_file_or_dir: &PathBuf,
        remote_dir: &str,
        use_sudo: bool,
        use_rsync: bool,
        silent: bool,
    ) -> Result<(), String> {
        // Create remote directory with appropriate permissions
        if local_file_or_dir.is_dir() {
            for entry in std::fs::read_dir(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to read local directory '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })? {
                let entry =
                    entry.map_err(|e| format!("Error reading directory entry. \n\t{}", e))?;
                let sub_path = entry.path();
                let base_name = sub_path
                    .file_name()
                    .ok_or("Invalid file name")?
                    .to_str()
                    .ok_or("Invalid file name encoding")?;
                let remote_sub = format!("{}/{}", remote_dir, base_name);

                // Check if remote file or directory exists

                if !self.confirm_and_overwrite_remote(&remote_sub, use_sudo, silent)? {
                    continue;
                }

                self.do_upload(use_sudo, use_rsync, &sub_path, &remote_sub)?;
                log_info_with_lock!(
                    "Successfully uploaded '{}' to '{}'",
                    sub_path.display(),
                    remote_sub
                );
            }
        } else {
            let base_name = local_file_or_dir
                .file_name()
                .ok_or("Invalid file name")?
                .to_str()
                .ok_or("Invalid file name encoding")?;
            let remote_file = format!("{}/{}", remote_dir, base_name);

            if self.confirm_and_overwrite_remote(&remote_file, use_sudo, silent)? {
                self.do_upload(use_sudo, use_rsync, local_file_or_dir, &remote_file)?;
                log_info_with_lock!(
                    "Successfully uploaded '{}' to '{}'",
                    local_file_or_dir.display(),
                    remote_file
                );
            }
        }
        Ok(())
    }

    /// Check if a remote path exists (file or directory), ask user for overwrite if needed,
    /// and delete it if confirmed.
    /// Returns Ok(true) if deleted, Ok(false) if skipped, Err(...) on error.
    pub fn confirm_and_overwrite_remote(
        &self,
        remote_path: &str,
        use_sudo: bool,
        silent: bool,
    ) -> Result<bool, String> {
        // Check if remote path is a file or directory
        let is_file = self.file_exists(remote_path)?;
        let is_dir = self.dir_exists(remote_path)?;

        if is_file || is_dir {
            // Build prompt message
            let prompt = if is_file {
                format!(
                    "Remote file '{}' already exists. Overwriting will DELETE it. Continue?",
                    remote_path
                )
            } else {
                format!(
                    "Remote directory '{}' already exists. Overwriting will DELETE it and all its contents. Continue?",
                    remote_path
                )
            };

            // Ask user unless in silent mode
            if !silent && !ask_user(&prompt) {
                return Ok(false); // User chose not to delete
            }

            // Execute deletion
            self.execute(&format!("rm -rf '{}'", remote_path), use_sudo)
                .map_err(|e| {
                    format!(
                        "Failed to remove existing remote {} '{}'. \n\t{}",
                        if is_file { "file" } else { "directory" },
                        remote_path,
                        e
                    )
                })?;

            return Ok(true); // Deleted successfully
        }

        Ok(false) // Path does not exist
    }
}
