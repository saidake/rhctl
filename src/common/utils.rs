use std::io::{self, BufRead};

pub fn ask_user(prompt: &str) -> bool {
    println!("{}", prompt);
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_lowercase() == "y"
}

pub fn resolve_remote_path(session: &super::ssh::SshSession, config: &super::config::Config, path: &str) -> Result<String, String> {
    session.execute(&format!("echo {}", path), config.sudo)
}

pub fn generate_temp_path(prefix: &str) -> String {
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("/tmp/sandbox/{}_{:x}", prefix, timestamp)
}