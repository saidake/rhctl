use std::io::{self, BufRead};

use crate::common::config::ConfigWrapper;
use crate::common::ssh::SshSession;

pub fn ask_user(prompt: &str) -> bool {
    println!("{}", prompt);
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_lowercase() == "y"
}

pub fn resolve_remote_path(session: &SshSession, config: &ConfigWrapper, path: &str) -> Result<String, String> {
    session.execute(&format!("echo {}", path), config.use_sudo)
}

pub fn generate_temp_path(prefix: &str) -> String {
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("/tmp/sandbox/{}_{:x}", prefix, timestamp)
}