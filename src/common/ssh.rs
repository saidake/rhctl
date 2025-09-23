use ssh2::Session;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;

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

    pub fn file_exists(&self, path: &str) -> Result<bool, String> {
        debug!("Checking if '{}' exists", path);
        let output = self.execute(&format!("test -e '{}'; echo $?", path), false)?;
        let exists = output.trim() == "0";
        debug!("File '{}' exists: {}", path, exists);
        Ok(exists)
    }

    pub fn check_directory_writable(&self, path: &str, use_sudo: bool) -> Result<(), String> {
        debug!("Ensuring remote directory '{}' exists", path);

        let mkdir_cmd = format!("mkdir -p '{}'", path);
        self.execute(&mkdir_cmd, use_sudo)
            .map_err(|e| format!("Failed to create directory '{}'. \n\t{}", path, e))?;

        debug!("Checking if remote directory '{}' is writable", path);

        let check_cmd = format!("test -w '{}'; echo $?", path);
        let output = self
            .execute(&check_cmd, use_sudo)
            .map_err(|e| format!("Failed to check write permission for '{}'. \n\t{}", path, e))?;

        if output.trim() != "0" {
            return Err(format!("Directory '{}' is not writable", path));
        }

        Ok(())
    }

    pub fn scp_upload(
        &self,
        local_path: &Path,
        remote_path: &str,
        use_sudo: bool,
    ) -> Result<(), String> {
        debug!(
            "Starting SCP upload from '{}' to '{}'",
            local_path.display(),
            remote_path
        );
        if !local_path.exists() {
            return Err(format!(
                "Local file '{}' does not exist",
                local_path.display()
            ));
        }
        if use_sudo {
            let temp_path = super::utils::generate_temp_path("upload");
            debug!("Uploading to temporary path '{}' with sudo", temp_path);
            self.do_scp_upload(local_path, &temp_path)?;
            self.execute(&format!("mv '{}' '{}'", temp_path, remote_path), true)?;
        } else {
            self.do_scp_upload(local_path, remote_path)?;
        }
        debug!("SCP upload to '{}' completed", remote_path);
        Ok(())
    }

    fn do_scp_upload(&self, local_path: &Path, remote_path: &str) -> Result<(), String> {
        let mut file = std::fs::File::open(local_path).map_err(|e| {
            format!(
                "Failed to open local file '{}': {}",
                local_path.display(),
                e
            )
        })?;
        let stat = file.metadata().map_err(|e| {
            format!(
                "Failed to get metadata for '{}': {}",
                local_path.display(),
                e
            )
        })?;
        debug!("File size: {} bytes, permissions: 0644", stat.len());
        let mut channel = self
            .session
            .scp_send(Path::new(remote_path), 0o644, stat.len(), None)
            .map_err(|e| format!("Failed to initiate SCP upload to '{}'. \n\t{}", remote_path, e))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read file '{}'. \n\t{}", local_path.display(), e))?;
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
        channel
            .close()
            .map_err(|e| format!("Failed to close SCP channel for '{}'. \n\t{}", remote_path, e))?;
        channel.wait_close().map_err(|e| {
            format!(
                "Failed to wait for SCP channel close for '{}'. \n\t{}",
                remote_path, e
            )
        })?;
        Ok(())
    }
}
