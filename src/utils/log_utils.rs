use crate::domain::constants::USER_ABORTED_MESSAGE;
use log::{debug, error, info, warn};
use rpassword::prompt_password;
use std::{
    io::{self, Write},
    process::exit,
};

/// Core logging function with lock
pub fn log_with_lock(level: &str, message: &str) {
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
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("INFO", &format!($($arg)*));
    };
}

/// Macro for error log
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("ERROR", &format!($($arg)*));
    };
}

/// Macro for warn log
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("WARN", &format!($($arg)*));
    };
}

/// Macro for debug log
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("DEBUG", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_remote {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let label = format!("{:<6}", "REMOTE");
        println!("[{}] {}", ansi_term::Colour::Purple.paint(&label), msg);
    }};
}

#[macro_export]
macro_rules! log_local {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let label = format!("{:<6}", "LOCAL");
        println!("[{}] {}", ansi_term::Colour::Purple.paint(&label), msg);
    }};
}

#[macro_export]
macro_rules! log_ask {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let label = format!("{:<6}", "ASK");
        print!("[{}] {}", ansi_term::Colour::Cyan.paint(&label), msg);
    }};
}

/// Ask user with a prompt, return true if input is 'y' or 'Y'
/// User must press Enter
pub fn ask_user(prompt: &str, silent: bool) -> Result<(), String> {
    if silent {
        return Ok(());
    }
    log_ask!("{} [y/N]: ", prompt);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    if input.trim().to_lowercase() != "y" {
        return Err(USER_ABORTED_MESSAGE.to_string());
    }
    Ok(())
}

pub fn ask_user_and_abort(prompt: &str, silent: bool) {
    if silent {
        return;
    }

    if let Err(_) = ask_user(prompt, false) {
        exit(1);
    }
}
pub fn prompt_password_or_exit() -> String {
    match prompt_password("Enter SSH password: ") {
        Ok(pwd) if !pwd.is_empty() => pwd,
        _ => {
            log_error!("Password is required.");
            std::process::exit(1);
        }
    }
}
