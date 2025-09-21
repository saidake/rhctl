use ssh2::Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

#[derive(Clone)]
pub struct SshSession {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    session: Session,
}

impl SshSession {
    pub fn new(config: &super::config::Config) -> Result<Self, String> {
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port)).map_err(|e| e.to_string())?;
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().map_err(|e| e.to_string())?;
        sess.userauth_password(&config.user, &config.password).map_err(|e| e.to_string())?;

        Ok(Self {
            host: config.host.clone(),
            port: config.port,
            user: config.user.clone(),
            password: config.password.clone(),
            session: sess,
        })
    }

    pub fn execute(&self, cmd: &str, sudo: bool) -> Result<String, String> {
        let mut channel = self.session.channel_session().map_err(|e| e.to_string())?;
        let full_cmd = if sudo {
            format!("echo '{}' | sudo -S -p '' bash -c '{}'", self.password, cmd)
        } else {
            cmd.to_string()
        };
        channel.exec(&full_cmd).map_err(|e| e.to_string())?;
        let mut output = String::new();
        channel.read_to_string(&mut output).map_err(|e| e.to_string())?;
        channel.wait_close().map_err(|e| e.to_string())?;
        if channel.exit_status().map_err(|e| e.to_string())? != 0 {
            return Err(format!("Command failed: {}", output));
        }
        Ok(output)
    }

    pub fn file_exists(&self, path: &str) -> Result<bool, String> {
        let output = self.execute(&format!("test -e '{}'; echo $?", path), false)?;
        Ok(output.trim() == "0")
    }

    pub fn scp_upload(&self, local_path: &Path, remote_path: &str, sudo: bool) -> Result<(), String> {
        if sudo {
            // Upload to temp, then mv with sudo
            let temp_path = super::utils::generate_temp_path("upload");
            self.do_scp_upload(local_path, &temp_path)?;
            self.execute(&format!("mv '{}' '{}'", temp_path, remote_path), true)?;
        } else {
            self.do_scp_upload(local_path, remote_path)?;
        }
        Ok(())
    }

    fn do_scp_upload(&self, local_path: &Path, remote_path: &str) -> Result<(), String> {
        let mut file = std::fs::File::open(local_path).map_err(|e| e.to_string())?;
        let stat = file.metadata().map_err(|e| e.to_string())?;
        let mut channel = self.session.scp_send(Path::new(remote_path), 0o644, stat.len(), None).map_err(|e| e.to_string())?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
        channel.write_all(&buffer).map_err(|e| e.to_string())?;
        channel.send_eof().map_err(|e| e.to_string())?;
        channel.wait_eof().map_err(|e| e.to_string())?;
        channel.close().map_err(|e| e.to_string())?; // Changed from send_close to close
        channel.wait_close().map_err(|e| e.to_string())?;
        Ok(())
    }
}