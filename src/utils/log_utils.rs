use crate::domain::cmd_params::ServerMetadata;
use crate::domain::constants::{
    LOG_ASK, LOG_DEBUG, LOG_ERROR, LOG_INFO, LOG_LEVEL_WIDTH, LOG_LOCAL, LOG_REMOTE, LOG_SHUTDOWN, LOG_TASK_NAME_WIDTH, LOG_WARN, USER_ABORTED_MESSAGE
};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use tokio::task;

use crossterm::{
    cursor, execute,
    terminal::{Clear, ClearType},
};
use log::{debug, error, info, warn};
use once_cell::sync::{Lazy, OnceCell};
use rpassword::prompt_password;
use std::io::stdout;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io::Write, process::exit};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

static ASK_LOCK: Lazy<Arc<Mutex<()>>> = Lazy::new(|| Arc::new(Mutex::new(())));
static ASK_ACTIVE: AtomicBool = AtomicBool::new(false);
static LOG_SENDER: OnceCell<Arc<mpsc::Sender<LogEntry>>> = OnceCell::new();

/// Core logging function with lock
pub fn log_to_bg_handler(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    level: &str,
    message: &str,
) {
    if let Some(tx) = LOG_SENDER.get() {
        let _ = tx.try_send(LogEntry {
            user: Some(server_metadata.user.clone()),
            host: Some(server_metadata.host.clone()),
            task_name: Some(task_name.to_string()),
            level: level.to_string(),
            message: message.to_string(),
        });
    }
}

pub fn log_to_bg_handler_option(
    server_metadata: Option<&Arc<ServerMetadata>>,
    task_name: Option<&str>,
    level: &str,
    message: &str,
) {
    if let Some(tx) = LOG_SENDER.get() {
        match server_metadata {
            Some(meta) => {
                let _ = tx.try_send(LogEntry {
                    user: Some(meta.user.clone()),
                    host: Some(meta.host.clone()),
                    task_name: task_name.map(|s| s.to_string()),
                    level: level.to_string(),
                    message: message.to_string(),
                });
            }
            None => {
                let _ = tx.try_send(LogEntry {
                    user: None,
                    host: None,
                    task_name: task_name.map(|s| s.to_string()),
                    level: level.to_string(),
                    message: message.to_string(),
                });
            }
        }
    }
}

pub fn log_host_to_bg_handler(
    user: Option<&str>,
    host: Option<&str>,
    task_name: Option<&str>,
    level: &str,
    message: &str,
) {
    if let Some(tx) = LOG_SENDER.get() {
        let _ = tx.try_send(LogEntry {
            user: user.map(|s| s.to_string()),
            host: host.map(|s| s.to_string()),
            task_name: task_name.map(|s| s.to_string()),
            level: level.to_string(),
            message: message.to_string(),
        });
    }
}

#[macro_export]
macro_rules! log_info {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler($server_metadata, $task_name, $crate::domain::constants::LOG_INFO, &format!($($arg)*));
    };
}

/// Macro for error log
#[macro_export]
macro_rules! log_error {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler($server_metadata, $task_name, $crate::domain::constants::LOG_ERROR, &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_error_with_host {
    ($user:expr,$host:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_host_to_bg_handler(Some($user), Some($host), Some($task_name), $crate::domain::constants::LOG_ERROR, &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_error_with_host_direct {
    ($user:expr,$host:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_direct_option(Some($user), Some($host), Some($task_name), $crate::domain::constants::LOG_ERROR, &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_error_root {
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_host_to_bg_handler(None, None, None, $crate::domain::constants::LOG_ERROR, &format!($($arg)*));
    };
}

/// Macro for warn log
#[macro_export]
macro_rules! log_warn {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler($server_metadata, $task_name, $crate::domain::constants::LOG_WARN, &format!($($arg)*));
    };
}

/// Macro for debug log
#[macro_export]
macro_rules! log_debug {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler($server_metadata, $task_name, $crate::domain::constants::LOG_DEBUG, &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_remote {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler($server_metadata, $task_name, $crate::domain::constants::LOG_REMOTE, &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_local {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler($server_metadata, $task_name, $crate::domain::constants::LOG_LOCAL, &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_ask {
    ($server_metadata:expr, $task_name:expr, $($arg:tt)*) => {
        $crate::utils::log_utils::log_to_bg_handler_option($server_metadata, $task_name, $crate::domain::constants::LOG_ASK, &format!($($arg)*));
    };
}

/// Ask user with a prompt, return true if input is 'y' or 'Y'
/// User must press Enter
pub async fn ask_user(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    prompt: &str,
    silent: bool,
) -> Result<(), String> {
    ask_user_option(Some(server_metadata), Some(task_name), prompt, silent).await
}

pub async fn ask_user_option(
    server_metadata: Option<&Arc<ServerMetadata>>,
    task_name: Option<&str>,
    prompt: &str,
    silent: bool,
) -> Result<(), String> {
    if silent {
        return Ok(());
    }

    // Lock entire ASK sequence
    let _guard = ASK_LOCK.lock().await;
    log_ask!(server_metadata, task_name, "{} [y/N]: ", prompt);
    stdout().flush().unwrap();

    // Use a scope to keep _guard alive
    let res = {
        // we cannot move `_guard` into blocking thread,
        // but we can keep this scope until after blocking completes.
        let result = task::spawn_blocking(move || {
            enable_raw_mode().unwrap();
            let mut result = Err(USER_ABORTED_MESSAGE.to_string());

            loop {
                if event::poll(std::time::Duration::from_millis(500)).unwrap() {
                    if let Event::Key(KeyEvent { code, .. }) = event::read().unwrap() {
                        match code {
                            KeyCode::Char(c @ 'y') | KeyCode::Char(c @ 'Y') => {
                                print!("{}", c);
                                result = Ok(());
                                break;
                            }
                            KeyCode::Char(c @ 'n') | KeyCode::Char(c @ 'N') => {
                                print!("{}", c);
                                result = Err(USER_ABORTED_MESSAGE.to_string());
                                break;
                            }
                            KeyCode::Char('c')
                                if event::KeyModifiers::CONTROL
                                    .contains(event::KeyModifiers::CONTROL) =>
                            {
                                disable_raw_mode().unwrap();
                                println!(); // move to next line after input
                                std::process::exit(130); // typical exit code for Ctrl+C
                            }
                            _ => {}
                        }
                    }
                }
            }

            disable_raw_mode().unwrap(); // leave raw mode
            println!(); // move to next line after input
            result
        })
        .await
        .unwrap(); // unwrap the JoinHandle

        result
    };

    // guard is dropped here — after ask completes
    ASK_ACTIVE.store(false, Ordering::SeqCst);
    res
}

pub async fn ask_user_and_abort(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    prompt: &str,
    silent: bool,
) {
    if silent {
        return;
    }

    if let Err(_) = ask_user(server_metadata, task_name, prompt, false).await {
        exit(1);
    }
}

pub async fn ask_user_and_abort_option(
    server_metadata: Option<&Arc<ServerMetadata>>,
    task_name: Option<&str>,
    prompt: &str,
    silent: bool,
) {
    if silent {
        return;
    }

    if let Err(_) = ask_user_option(server_metadata, task_name, prompt, false).await {
        exit(1);
    }
}

pub fn prompt_password_or_exit(user: &str, host: &str, task_name: &str) -> String {
    match prompt_password("Enter SSH password: ") {
        Ok(pwd) if !pwd.is_empty() => pwd,
        _ => {
            log_error_with_host!(user, host, task_name, "Password is required.");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub host: Option<String>,
    pub user: Option<String>,
    pub task_name: Option<String>,
    pub level: String,
    pub message: String,
}

pub async fn init_logger() -> tokio::task::JoinHandle<()> {
    let (tx, mut rx) = mpsc::channel::<LogEntry>(100000);
    let tx = Arc::new(tx);
    LOG_SENDER.set(tx.clone()).unwrap();

    let handle = tokio::spawn(async move {
        // let mut last_ask: Option<LogEntry> = None;
        let mut stdout = stdout();
        let mut last_ask: Option<LogEntry> = None;

        while let Some(entry) = rx.recv().await {
            if entry.level == LOG_SHUTDOWN {
                stdout.flush().ok();
                break; 
            }

            match entry.level.as_str() {
                LOG_ASK => {
                    ASK_ACTIVE.store(true, Ordering::SeqCst);
                    print_ask_with_info(
                        entry.host.as_deref(),
                        entry.user.as_deref(),
                        entry.task_name.as_deref(),
                        &entry.message,
                    );
                }
                _ => {
                    // Print common log （ INFO / ERROR / WARN / DEBUG / REMOTE / LOCAL）
                    log_direct_option(
                        entry.host.as_deref(),
                        entry.user.as_deref(),
                        entry.task_name.as_deref(),
                        entry.level.as_str(),
                        &entry.message,
                    );
                }
            }

            // println!("entry.level.trim()2: --{}--", entry.level.trim());
            if entry.level.trim() == LOG_ASK {
                // println!("ask assignment");
                last_ask = Some(entry);
            } else if entry.level.trim() != LOG_ASK && ASK_ACTIVE.load(Ordering::SeqCst) {
                // println!("ask 1");
                // println!("entry.level.trim(): {}", entry.level.trim());
                // println!();
                if let Some(ref ask) = last_ask {
                    // println!("ask.message: {}", ask.message);
                    // println!();
                    print_ask_with_info(
                        ask.host.as_deref(),
                        ask.user.as_deref(),
                        ask.task_name.as_deref(),
                        &ask.message,
                    );
                }
            }

            stdout.flush().unwrap();
        }
    });

    handle
}

pub fn log_direct_option(
    host: Option<&str>,
    user: Option<&str>,
    task_name: Option<&str>,
    level: &str,
    message: &str,
) {
    // Color the log level
    let level_colored = match level {
        LOG_ERROR => ansi_term::Colour::Red
            .paint(format!("{:<width$}", LOG_ERROR, width = LOG_LEVEL_WIDTH))
            .to_string(),
        LOG_WARN => ansi_term::Colour::Yellow
            .paint(format!("{:<width$}", LOG_WARN, width = LOG_LEVEL_WIDTH))
            .to_string(),
        LOG_INFO => ansi_term::Colour::Green
            .paint(format!("{:<width$}", LOG_INFO, width = LOG_LEVEL_WIDTH))
            .to_string(),
        LOG_DEBUG => ansi_term::Colour::Blue
            .paint(format!("{:<width$}", LOG_DEBUG, width = LOG_LEVEL_WIDTH))
            .to_string(),
        LOG_REMOTE | LOG_LOCAL => ansi_term::Colour::Purple
            .paint(format!("{:<width$}", level, width = LOG_LEVEL_WIDTH))
            .to_string(),
        _ => ansi_term::Colour::Green
            .paint(format!("{:<width$}", LOG_INFO, width = LOG_LEVEL_WIDTH))
            .to_string(),
    };

    // Color the user, host, and task if available
    let prefix = match (user, host, task_name) {
        (Some(user), Some(host), Some(task)) => {
            let colored_user = ansi_term::Colour::Fixed(81).paint(user).to_string();
            let colored_host = ansi_term::Colour::Fixed(81).paint(host).to_string();
            let colored_task = ansi_term::Colour::Fixed(216)
                .paint(format!("{:<width$}", task, width = LOG_TASK_NAME_WIDTH))
                .to_string();
            format!(
                "[{}@{}][{}][{}]",
                colored_user, colored_host, colored_task, level_colored
            )
        }
        _ => format!("[{}]", level_colored),
    };

    // Split message by newlines and print each line individually
    // English comment: Each line after the first will have a `>` marker to indicate continuation
    for (i, line) in message.lines().enumerate() {
        let line_to_print = if i == 0 {
            format!("{} {}", prefix, line)
        } else {
            format!("{}", line)
        };
        execute!(
            stdout(),
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine)
        )
        .unwrap();
        match level {
            LOG_ERROR => error!("{}", line_to_print),
            LOG_WARN => warn!("{}", line_to_print),
            LOG_INFO => info!("{}", line_to_print),
            LOG_DEBUG => debug!("{}", line_to_print),
            _ => info!("{}", line_to_print),
        }
    }
}

pub fn print_ask_with_info(
    host: Option<&str>,
    user: Option<&str>,
    task_name: Option<&str>,
    message: &str,
) {
    let colored_ask = &ansi_term::Colour::Cyan
        .paint(format!("{:<width$}", LOG_ASK, width = LOG_LEVEL_WIDTH))
        .to_string();
    match (user, host, task_name) {
        (Some(user), Some(host), Some(task)) => {
            // 207 bright magenta
            let colored_user = ansi_term::Colour::Fixed(81).paint(user).to_string(); // bright magenta
            let colored_host = ansi_term::Colour::Fixed(81).paint(host).to_string(); // cyan-blue
            let colored_task = ansi_term::Colour::Fixed(216)
                .paint(format!("{:<width$}", task, width = LOG_TASK_NAME_WIDTH))
                .to_string(); // orange-yellow
            execute!(
                stdout(),
                cursor::MoveToColumn(0),
                Clear(ClearType::CurrentLine)
            )
            .unwrap();
            // sleep(Duration::from_millis(300));
            print!(
                "[{}@{}][{}][{}] {}",
                colored_user, colored_host, colored_task, colored_ask, message
            );
        }
        _ => {
            execute!(
                stdout(),
                cursor::MoveToColumn(0),
                Clear(ClearType::CurrentLine)
            )
            .unwrap();
            // sleep(Duration::from_millis(300));
            print!("[{}] {}", colored_ask, message);
        }
    }
}

pub async fn flush_logs_and_exit(logger_handle: tokio::task::JoinHandle<()>) -> ! {
    if let Some(tx_arc) = LOG_SENDER.get() {
        let tx = tx_arc.clone();
        let _ = tx.send(LogEntry {
            user: None,
            host: None,
            task_name: None,
            level: LOG_SHUTDOWN.to_string(),
            message: String::new(),
        }).await;
    }
    let _ = logger_handle.await;
    std::process::exit(1);
}