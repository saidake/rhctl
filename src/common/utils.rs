
use crate::common::config::ConfigWrapper;
use crate::common::ssh::SshSession;
use crate::domain::constants::GLOBAL_LOG_LOCK;
use std::io::{self, Write, Read};
use std::sync::{Mutex, MutexGuard};
use log::{debug, error, info, warn};


/// Ask user with a prompt, return true if input is 'y' or 'Y'
/// User must press Enter
pub fn ask_user(prompt: &str) -> bool {
    let _lock = GLOBAL_LOG_LOCK.lock().unwrap(); // Ensure ordered input

    print!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    matches!(input.trim().to_lowercase().as_str(), "y")
}

/// Core logging function with lock
pub fn log_with_lock(level: &str, message: &str) {
    let _lock = GLOBAL_LOG_LOCK.lock().unwrap();
    match level {
        "INFO" => info!("{}", message),
        "ERROR" => error!("{}", message),
        "WARN" => warn!("{}", message),
        "DEBUG" => debug!("{}", message),
        _ => println!("[{}] {}", level, message),
    }
}

/// Macro for info log
#[macro_export]
macro_rules! log_info_with_lock {
    ($($arg:tt)*) => {
        $crate::common::utils::log_with_lock("INFO", &format!($($arg)*));
    };
}

/// Macro for error log
#[macro_export]
macro_rules! log_error_with_lock {
    ($($arg:tt)*) => {
        $crate::common::utils::log_with_lock("ERROR", &format!($($arg)*));
    };
}

/// Macro for warn log
#[macro_export]
macro_rules! log_warn_with_lock {
    ($($arg:tt)*) => {
        $crate::common::utils::log_with_lock("WARN", &format!($($arg)*));
    };
}

/// Macro for debug log
#[macro_export]
macro_rules! log_debug_with_lock {
    ($($arg:tt)*) => {
        $crate::common::utils::log_with_lock("DEBUG", &format!($($arg)*));
    };
}


pub fn resolve_remote_path(session: &SshSession, config: &ConfigWrapper, path: &str) -> Result<String, String> {
    session.execute(&format!("echo {}", path), config.use_sudo)
}

pub fn generate_temp_path(prefix: &str) -> String {
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("/tmp/sandbox/{}_{:x}", prefix, timestamp)
}