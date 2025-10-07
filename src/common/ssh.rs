use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::sync::Arc;
use async_recursion::async_recursion;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::ServerMetadata;
use crate::domain::constants::REMOTE_TEMP_SBXCTL_FOLDER;
use crate::utils::file_utils::{
    generate_remote_temp_dir, get_local_path_base_name, substitute_vars,
};
use crate::utils::log_utils::ask_user;
use crate::utils::ssh_utils::execution_print;
use crate::{log_debug_with_lock, log_info_with_lock, remote};
use log::{debug, error};

#[derive(Clone)]
pub struct ServerHandle<T: ServerMetadata + Send + Sync + 'static> {
    pub server_metadata: Arc<T>,
    pub global_server_pool: Arc<ServerPool>,
}

impl<T: ServerMetadata + Send + Sync + 'static> ServerHandle<T> {
    pub async fn resolve_remote_path(&self, use_sudo: bool, path: &str) -> Result<String, String> {
        self.exec(&format!("echo {}", path), use_sudo).await
    }

    pub async fn exec(&self, cmd: &str, use_sudo: bool) -> Result<String, String> {
        self.exec_with_stream(cmd, use_sudo, false).await
    }

    pub async fn exec_with_log(&self, cmd: &str, use_sudo: bool) -> Result<String, String> {
        self.exec_with_stream(cmd, use_sudo, true).await
    }

    async fn exec_with_stream(
        &self,
        cmd: &str,
        use_sudo: bool,
        print_log: bool,
    ) -> Result<String, String> {
        let server_metadata = self.server_metadata.clone();
        let cmd: String = cmd.to_string();
        let mut channel_guard = self.global_server_pool.get_channel(&server_metadata).await?;

        debug!("Streaming command: {} (sudo: {})", cmd, use_sudo);
        let full_cmd = if use_sudo {
            let escaped = cmd.replace("'", "'\\''");
            format!("sudo -S bash -c '{}'", escaped)
        } else {
            cmd.to_string()
        };

        // println!("full_cmd: {}", full_cmd);
        if use_sudo {
            channel_guard.channel
                .request_pty("xterm", None, None)
                .map_err(|e| format!("Failed to request pty for sudo. \n\t{}", e))?;
        }

        channel_guard.channel
            .exec(&full_cmd)
            .map_err(|e| format!("Failed to execute command '{}'. \n\t{}", cmd, e))?;

        // Input password
        if use_sudo {
            let mut prompt_buf = [0u8; 1024];
            channel_guard.channel
                .read(&mut prompt_buf)
                .map_err(|e| format!("Failed to read sudo prompt: {}", e))?;
            let pw_with_newline = format!("{}\n", server_metadata.get_password());
            channel_guard.channel
                .write_all(pw_with_newline.as_bytes())
                .map_err(|e| format!("Failed to send sudo password. \n\t{}", e))?;
        }

        // Read stdout
        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();

        {
            let stdout = channel_guard.channel.stream(0);
            let stdout_reader = BufReader::new(stdout);
            let mut first_line = true;
            for line in stdout_reader.lines() {
                let line = line.map_err(|e| format!("Failed to read stdout. \n\t{}", e))?;
                if first_line && use_sudo && line.trim().is_empty() {
                    first_line = false;
                    continue;
                }
                // println!("line: ------------{}--------", line);
                stdout_buf.push_str(&line);
                stdout_buf.push('\n');
                if print_log {
                    execution_print(&line, false)?;
                }
            }
        }

        // Read stderr
        {
            let stderr = channel_guard.channel.stderr();
            let stderr_reader = BufReader::new(stderr);
            for line in stderr_reader.lines() {
                let line = line.map_err(|e| format!("Failed to read stderr. \n\t{}", e))?;
                stderr_buf.push_str(&line);
                stderr_buf.push('\n');
                if print_log {
                    execution_print(&line, true)?;
                }
            }
        }

        channel_guard.channel
            .wait_close()
            .map_err(|e| format!("Failed to close channel. \n\t{}", e))?;
        let exit_status = channel_guard.channel
            .exit_status()
            .map_err(|e| format!("Failed to get exit status. \n\t{}", e))?;

        debug!("Command exit status: {}", exit_status);

        if exit_status != 0 {
            let mut msg = format!("Command '{}' failed with exit status {}.", cmd, exit_status);
            if !stdout_buf.trim().is_empty() {
                msg.push_str(&format!("\n\tstdout:\n\t{}", stdout_buf));
            }
            if !stderr_buf.trim().is_empty() {
                msg.push_str(&format!("\n\tstderr:\n\t{}", stderr_buf));
            }
            return Err(msg);
        }
        if stdout_buf.ends_with('\n') {
            stdout_buf.pop();
        }
        // println!("stdout_buf: ------------{}--------", stdout_buf);
        Ok(stdout_buf)
    }

    // This method does not create remote_dir; you should ensure the directory exists and is writable.
    pub async fn upload_file_or_dir_contents_into_dir(
        &self,
        local_file_or_dir: &Path,
        remote_dir: &str,
        new_file_name: Option<&str>,
        use_sudo: bool,
        use_rsync: bool,
        silent: bool,
        direct_write_if_sudo: bool,
        print_log: bool,
    ) -> Result<(), String> {
        // Create a temp directory for the current user
        let mut remote_temp_dir: Option<String> = None;
        if use_sudo && self.server_metadata.get_user() != "root" && !direct_write_if_sudo {
            remote_temp_dir = Some(self.create_remote_temp_dir("upload", use_sudo).await?);
        }

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
                let base_name = get_local_path_base_name(&sub_path)?;
                let remote_sub = format!("{}/{}", remote_dir, base_name);

                // Check if remote file or directory exists
                self.ask_safe_to_transfer(&remote_sub, use_sudo, silent).await?;
                // thread::sleep(Duration::from_secs(500000));
                if use_sudo && self.server_metadata.get_user() != "root" && !direct_write_if_sudo {
                    // println!("sub_path: {}", sub_path.display());

                    let temp_dir = remote_temp_dir.as_ref().unwrap();
                    // println!("do_upload_with_scp_recursive");
                    self.do_upload(use_sudo, use_rsync, &sub_path, &temp_dir, new_file_name).await?;
                } else {
                    self.do_upload(use_sudo, use_rsync, &sub_path, &remote_dir, new_file_name).await?;
                }
            }
            if use_sudo && self.server_metadata.get_user() != "root" && !direct_write_if_sudo {
                let temp_dir = remote_temp_dir.as_ref().unwrap();
                // Move the content in temp_dir to the remote_dir
                // println!("move_and_delete_temp_dir - temp_dir: {}", temp_dir);
                // println!("move_and_delete_temp_dir - remote_dir: {}", remote_dir);
                // thread::sleep(Duration::from_secs(30));
                self.move_and_delete_temp_dir(temp_dir, remote_dir, use_sudo)
                    .await?;
            }
            if print_log {
                log_info_with_lock!(
                    "Successfully uploaded the contents of the folder '{}' into '{}'",
                    local_file_or_dir.display(),
                    remote_dir
                );
            }
        } else {
            let remote_file = format!(
                "{}/{}",
                remote_dir,
                new_file_name.unwrap_or(get_local_path_base_name(&local_file_or_dir)?.as_str())
            );
            self.ask_safe_to_transfer(&remote_file, use_sudo, silent).await?;
            if use_sudo && self.server_metadata.get_user() != "root" && !direct_write_if_sudo {
                let temp_dir = remote_temp_dir.as_ref().unwrap();
                // println!("do_upload_with_scp_recursive");
                self.do_upload(
                    use_sudo,
                    use_rsync,
                    &local_file_or_dir,
                    &temp_dir,
                    new_file_name,
                ).await?;
                // Move the content in temp_dir to the remote_dir
                // println!("move_and_delete_temp_dir - temp_dir: {}", temp_dir);
                // println!("move_and_delete_temp_dir - remote_dir: {}", remote_dir);
                // thread::sleep(Duration::from_secs(30));

                self.move_and_delete_temp_dir(temp_dir, remote_dir, use_sudo)
                    .await?;
            } else {
                self.do_upload(
                    use_sudo,
                    use_rsync,
                    &local_file_or_dir,
                    &remote_dir,
                    new_file_name,
                ).await?;
            }
            if print_log {
                log_info_with_lock!(
                    "Successfully uploaded the file '{}' to '{}'",
                    local_file_or_dir.display(),
                    remote_file
                );
            }
        }

        Ok(())
    }

    async fn move_and_delete_temp_dir(
        &self,
        temp_dir: &str,
        remote_dir: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        // Move hidden files if any exist
        let hidden_exists = self
            .file_or_dir_exists(&format!("{}/.[!.]*", temp_dir), use_sudo)
            .await?;
        // println!("hidden_exists: {}", hidden_exists);
        if hidden_exists {
            self.exec(
                &format!("mv \"{0}\"/.[!.]* \"{1}\"/", temp_dir, remote_dir),
                use_sudo,
            )
            .await?;
        }

        // Move normal files if any exist
        let normal_exists = self
            .file_or_dir_exists(&format!("{0}/*", temp_dir), use_sudo)
            .await?;
        // println!("normal_exists: {}", normal_exists);
        if normal_exists {
            self.exec(
                &format!("mv \"{0}\"/* \"{1}\"/", temp_dir, remote_dir),
                use_sudo,
            )
            .await?;
        }
        // thread::sleep(Duration::from_secs(30));
        // Remove the temporary directory
        self.exec(&format!("rm -rf \"{}\"", temp_dir), use_sudo)
            .await?;

        Ok(())
    }

    /// Check if a remote path REMOTE_TEMP_SBXCTL_FOLDER exists, ask user for overwrite if needed,
    /// and delete it if confirmed.
    /// Returns Ok(true) if deleted, Ok(false) if skipped, Err(...) on error.
    pub async fn check_global_remote_temp_dir(
        &self,
        use_sudo: bool,
        silent: bool,
    ) -> Result<(), String> {
        // Only non-root sudo users need to handle the global temp folder
        if use_sudo && self.server_metadata.get_user() != "root" {
            let exists = self
                .file_or_dir_exists(REMOTE_TEMP_SBXCTL_FOLDER, use_sudo)
                .await?;
            // println!("exists: {}",exists);
            if exists {
                ask_user(format!(
                        "Remote path '{}' already exists. Transfering will DELETE it and use it as a temp folder. Continue?",
                        REMOTE_TEMP_SBXCTL_FOLDER
                    ).as_str(),silent)?;

                self.exec(
                    &format!("rm -rf \"{}\"", REMOTE_TEMP_SBXCTL_FOLDER),
                    use_sudo,
                )
                .await?;
                // .map_err(|e| {
                //     format!(
                //         "Failed to remove existing remote '{}'. \n\t{}",
                //         REMOTE_TEMP_SBXCTL_FOLDER, e
                //     )
                // })?;
            }
        }
        Ok(())
    }

    pub async fn delete_global_temp_dir(&self, use_sudo: bool) -> Result<(), String> {
        if use_sudo && self.server_metadata.get_user() != "root" {
            // println!("delete_global_temp_dir");
            let remote_temp_sbxctl_folder_exists = self
                .file_or_dir_exists(REMOTE_TEMP_SBXCTL_FOLDER, use_sudo)
                .await?;
            // println!(
            //     "remote_temp_sbxctl_folder_exists: {}",
            //     remote_temp_sbxctl_folder_exists
            // );
            if remote_temp_sbxctl_folder_exists {
                self.exec(
                    &format!("rm -rf \"{}\"", REMOTE_TEMP_SBXCTL_FOLDER),
                    use_sudo,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Check if a remote path exists with specific test flag (-e, -f, -d).
    async fn check_path(&self, path: &str, flag: &str, use_sudo: bool) -> Result<bool, String> {
        debug!("Checking if '{}' exists with flag '{}'", path, flag);

        let full_cmd = format!("sh -c 'test {} {}'", flag, path);
        // println!(
        //     "path: {}, full_cmd: {}, use_sudo: {}",
        //     path, full_cmd, use_sudo
        // );

        let result = self.exec(&full_cmd, use_sudo).await;

        match result {
            Ok(_) => {
                debug!("Remote path '{}' exists (flag '{}'): true", path, flag);
                Ok(true)
            }
            Err(e) => {
                // If test returns 1 → path does not exist
                if e.contains("exit status 1") {
                    debug!("Remote path '{}' exists (flag '{}'): false", path, flag);
                    Ok(false)
                } else {
                    // Other errors (such as insufficient sudo permissions) return Err directly
                    Err(format!("Failed to check remote path '{}'. \n\t{}", path, e))
                }
            }
        }
    }

    /// Check if file or directory exists (-e).
    pub async fn file_or_dir_exists(&self, path: &str, use_sudo: bool) -> Result<bool, String> {
        self.check_path(path, "-e", use_sudo).await
    }

    /// Check if file exists (-f).
    pub async fn file_exists(&self, path: &str, use_sudo: bool) -> Result<bool, String> {
        self.check_path(path, "-f", use_sudo).await
    }

    /// Check if directory exists (-d).
    pub async fn dir_exists(&self, path: &str, use_sudo: bool) -> Result<bool, String> {
        self.check_path(path, "-d", use_sudo).await
    }

    // Upload a local file or directory to a remote directory via SCP recursively.
    // An error occurs if the file’s parent directory does not exist.
    #[async_recursion]
    async fn do_upload_with_scp_recursive(
        &self,
        local_file_or_dir: &Path,
        remote_dir: &str,
        use_sudo: bool,
        new_base_name: Option<&str>,
    ) -> Result<(), String> {
        // println!("local_file_or_dir: {}", local_file_or_dir.display());
        // println!("remote_dir: {}", remote_dir);
        // thread::sleep(Duration::from_secs(60));
        if !local_file_or_dir.exists() {
            return Err(format!(
                "Local path '{}' does not exist",
                local_file_or_dir.display()
            ));
        }

        let base_name = new_base_name
            .map(|s| s.to_string())
            .unwrap_or(get_local_path_base_name(&local_file_or_dir)?);
        // Build the remote target path
        let remote_target = format!("{}/{}", remote_dir, base_name);
        // println!(
        //     "local_file_or_dir:{}, remote_target: {}",
        //     local_file_or_dir.display(),
        //     remote_target
        // );

        // Create remote directory if local path is a directory
        if local_file_or_dir.is_dir() {
            // println!("local_file_or_dir.is_dir");
            self.create_remote_dir(remote_target.as_str(), use_sudo).await?;
            // Recursively upload each entry
            for entry in fs::read_dir(local_file_or_dir).map_err(|e| {
                format!(
                    "Failed to read local directory '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })? {
                let entry =
                    entry.map_err(|e| format!("Error reading directory entry. \n\t{}", e))?;
                let sub_path = entry.path();

                // Recursive call
                self.do_upload_with_scp_recursive(&sub_path, &remote_target, use_sudo, None).await?;
            }
        } else {
            // Local path is a file → upload
            let mut file = fs::File::open(local_file_or_dir).map_err(|e| {
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

            debug!(
                "Uploading file '{}' ({} bytes) to '{}'",
                local_file_or_dir.display(),
                stat.len(),
                remote_target
            );

            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).map_err(|e| {
                format!(
                    "Failed to read file '{}'. \n\t{}",
                    local_file_or_dir.display(),
                    e
                )
            })?;

            //                     self.global_server_pool
            // .use_channel(&self.server_metadata, move |channel| {

            // }).await?;
            let mut channel_guard=self.global_server_pool.get_channel(&self.server_metadata).await?;

            channel_guard.channel.write_all(&buffer).map_err(|e| {
                format!(
                    "Failed to write to SCP channel for '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.send_eof().map_err(|e| {
                format!(
                    "Failed to send EOF for SCP upload to '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.wait_eof().map_err(|e| {
                format!(
                    "Failed to wait for EOF for SCP upload to '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.close().map_err(|e| {
                format!(
                    "Failed to close SCP channel for '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
            channel_guard.channel.wait_close().map_err(|e| {
                format!(
                    "Failed to wait for SCP channel close for '{}'. \n\t{}",
                    remote_target, e
                )
            })?;
        }
        // thread::sleep(Duration::from_secs(1500000000));
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

    async fn do_upload(
        &self,
        use_sudo: bool,
        use_rsync: bool,
        local_file_or_dir: &Path,
        remote_dir: &str,
        new_file_name: Option<&str>,
    ) -> Result<(), String> {
        log_debug_with_lock!(
            "Attempting to upload '{}' to '{}'",
            local_file_or_dir.display(),
            remote_dir
        );

        if use_rsync && self.command_exists("rsync") {
            log_debug_with_lock!("Using rsync for upload");
            if !self.server_metadata.get_password().is_empty() {
                if self.command_exists("sshpass") {
                    let remote_target = if let Some(name) = new_file_name {
                        format!("{}/{}", remote_dir, name)
                    } else {
                        remote_dir.to_string()
                    };
                    let mut sshpass_cmd = std::process::Command::new("sshpass");
                    sshpass_cmd
                        .arg("-p")
                        .arg(&self.server_metadata.get_password())
                        .arg("rsync")
                        .arg("-avz")
                        .arg("-e")
                        .arg(format!("ssh -p {}", self.server_metadata.get_ssh_port()))
                        .arg(
                            local_file_or_dir
                                .to_str()
                                .ok_or("Invalid local path encoding")?,
                        )
                        .arg(format!(
                            "{}@{}:{}",
                            self.server_metadata.get_user(),
                            self.server_metadata.get_host(),
                            remote_target
                        ))
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
            debug!(
                "Starting SCP upload from '{}' to '{}'",
                local_file_or_dir.display(),
                remote_dir
            );
            self.do_upload_with_scp_recursive(
                local_file_or_dir,
                remote_dir,
                use_sudo,
                new_file_name,
            ).await?;
            debug!("SCP upload to '{}' completed", remote_dir);
        }

        Ok(())
    }

    /// Check if a remote path exists (file or directory), ask user for overwrite if needed,
    /// and delete it if confirmed.
    /// Returns Ok(true) if deleted, Ok(false) if skipped, Err(...) on error.
    pub async fn ask_safe_to_transfer(
        &self,
        remote_path: &str,
        use_sudo: bool,
        silent: bool,
    ) -> Result<(), String> {
        // Check if remote path is a file or directory
        let is_file = self.file_exists(remote_path, use_sudo).await?;
        let is_dir = self.dir_exists(remote_path, use_sudo).await?;
        // println!(
        //     "is_file: {}, is_dir: {}, remote_path: {}",
        //     is_file, is_dir, remote_path
        // );

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
            ask_user(&prompt, silent)?;
            // Execute deletion
            self.exec(&format!("rm -rf \"{}\"", remote_path), use_sudo)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to remove existing remote {} '{}'. \n\t{}",
                        if is_file { "file" } else { "directory" },
                        remote_path,
                        e
                    )
                })?;
            // println!("delete remote_path: {}", remote_path);
            return Ok(()); // Deleted successfully
        }

        Ok(()) // Path does not exist
    }

    pub async fn validate_remote_dir(&self, remote_dir: &str, use_sudo: bool) -> Result<(), String> {
        debug!("Ensuring remote directory '{}' exists", remote_dir);
        if self.file_exists(remote_dir, use_sudo).await? {
            return Err(format!(
                "Path '{}' exists and is a file, not a directory",
                remote_dir
            ));
        }

        debug!("Checking if remote directory '{}' is writable", remote_dir);
        let check_cmd = format!("test -w \"{}\"; echo $?", remote_dir);
        let output = self.exec(&check_cmd, use_sudo).await.map_err(|e| {
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

    // Creates a remote directory if it doesn't exist.
    // If the directory already exists, no error occurs.
    pub async fn create_remote_dir(&self, remote_dir: &str, use_sudo: bool) -> Result<(), String> {
        if self.dir_exists(remote_dir, use_sudo).await? {
            return Ok(());
        }
        let cmd = if use_sudo {
            format!(
                "mkdir -p \"{}\"; chown {} \"{}\"; chmod 700 \"{}\"",
                remote_dir,
                self.server_metadata.get_user(),
                remote_dir,
                remote_dir
            )
        } else {
            format!("mkdir -p \"{}\"; chmod 700 \"{}\"", remote_dir, remote_dir)
        };

        self.exec(&cmd, use_sudo).await.map_err(|e| {
            format!(
                "Failed to create remote directory '{}'. \n\t{}",
                remote_dir, e
            )
        })?;
        Ok(())
    }

    pub async fn create_remote_temp_dir(&self, prefix: &str, use_sudo: bool) -> Result<String, String> {
        let temp_dir = generate_remote_temp_dir(prefix);
        log_debug_with_lock!("Uploading to temporary path '{}' with sudo", temp_dir);
        self.create_remote_dir(temp_dir.as_str(), use_sudo).await?;
        Ok(temp_dir)
    }
}
