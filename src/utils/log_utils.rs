use crate::domain::constants::GLOBAL_LOG_LOCK;
use log::{debug, error, info, warn};
use rpassword::prompt_password;
use std::io::{self, Write};


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

pub fn prompt_password_or_exit() -> String {
    match prompt_password("Enter SSH password: ") {
        Ok(pwd) if !pwd.is_empty() => pwd,
        _ => {
            eprintln!("Password is required.");
            std::process::exit(1);
        }
    }
}

/// Macro for info log
#[macro_export]
macro_rules! log_info_with_lock {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("INFO", &format!($($arg)*));
    };
}

/// Macro for error log
#[macro_export]
macro_rules! log_error_with_lock {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("ERROR", &format!($($arg)*));
    };
}

/// Macro for warn log
#[macro_export]
macro_rules! log_warn_with_lock {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("WARN", &format!($($arg)*));
    };
}

/// Macro for debug log
#[macro_export]
macro_rules! log_debug_with_lock {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("DEBUG", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! remote {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("[{}] {}", ansi_term::Colour::Purple.paint("REMOTE"), msg);
    }};
}

#[macro_export]
macro_rules! local {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("[{}] {}", ansi_term::Colour::Purple.paint("LOCAL"), msg);
    }};
}

