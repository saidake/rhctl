use crate::common::ssh::SshSession;
use log::{error, info};

pub fn connect_ssh(host: String, user: String, ssh_port: u16, password: String) -> SshSession {
    info!("Connecting via SSH to {}@{}:{}", user, host, ssh_port);

    match SshSession::new(host.clone(), user.clone(), ssh_port, password.clone()) {
        Ok(session) => {
            info!("SSH session established: {}@{}:{}", user, host, ssh_port);
            session
        }
        Err(e) => {
            error!("SSH connection failed. \n\t{}", e);
            std::process::exit(1);
        }
    }
}

pub fn resolve_remote_path(
    session: &SshSession,
    use_sudo: bool,
    path: &str,
) -> Result<String, String> {
    session
        .exec(&format!("echo {}", path), use_sudo)
        .map(|s| s.trim_end().to_string()) // Trim trailing whitespace and newlines
}

pub fn connect_ssh_thread(
    host: String,
    user: String,
    ssh_port: u16,
    password: String,
) -> SshSession {
    match SshSession::new(
        host.clone(),
        user.clone(),
        ssh_port.clone(),
        password.clone(),
    ) {
        Ok(s) => s,
        Err(e) => {
            error!("SSH connection failed in sub thread. \n\t{}", e);
            std::process::exit(1);
        }
    }
}
