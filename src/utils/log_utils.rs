use crate::domain::constants::USER_ABORTED_MESSAGE;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use tokio::task;

use crossterm::{
    cursor, execute,
    terminal::{Clear, ClearType},
};
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use rpassword::prompt_password;
use std::io::stdout;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io::Write, process::exit};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

static ASK_LOCK: Lazy<Arc<Mutex<()>>> = Lazy::new(|| Arc::new(Mutex::new(())));
static ASK_ACTIVE: AtomicBool = AtomicBool::new(false);

static LOG_SENDER: Lazy<Arc<Mutex<Option<mpsc::Sender<LogEntry>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Core logging function with lock
pub fn log_with_lock(level: &str, message: &str) {
    if let Some(tx) = &*LOG_SENDER.try_lock().unwrap() {
        let _ = tx.try_send(LogEntry {
            level: level.to_string(),
            message: message.to_string(),
        });
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
    // ($($arg:tt)*) => {{
    //     let msg = format!($($arg)*);
    //     let label = format!("{:<6}", "REMOTE");
    //     let label_colored = ansi_term::Colour::Purple.paint(&label).to_string();
    //     $crate::utils::log_utils::log_with_lock(&label_colored, &msg);
    //     // println!("[{}] {}", ansi_term::Colour::Purple.paint(&label), msg);
    // }};
    ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("REMOTE", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_local {
     ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("LOCAL", &format!($($arg)*));
    };
}

#[macro_export]
macro_rules! log_ask {
         ($($arg:tt)*) => {
        $crate::utils::log_utils::log_with_lock("ASK", &format!($($arg)*));
    };
}

// /// Ask user with a prompt, return true if input is 'y' or 'Y'
// /// User must press Enter
// pub fn ask_user(prompt: &str, silent: bool) -> Result<(), String> {
//     if silent {
//         return Ok(());
//     }

//     log_ask!("{} [y/N]: ", prompt);
//     std::io::stdout().flush().unwrap();

//     let mut input = String::new();
//     std::io::stdin().read_line(&mut input).unwrap();

//     if input.trim().eq_ignore_ascii_case("y") {
//         Ok(())
//     } else {
//         Err(USER_ABORTED_MESSAGE.to_string())
//     }

//     // // log
//     // log_ask!("{} [y/N]: ", prompt);
//     // io::stdout().flush().unwrap();

//     // let mut input = String::new();
//     // io::stdin().read_line(&mut input).unwrap();

//     // if input.trim().to_lowercase() != "y" {
//     //     return Err(USER_ABORTED_MESSAGE.to_string());
//     // }
//     // Ok(())
// }

/// Ask user with a prompt, return true if input is 'y' or 'Y'
/// User must press Enter
pub async fn ask_user(prompt: &str, silent: bool) -> Result<(), String> {
    if silent {
        return Ok(());
    }

    // lock to ensure only one ask at a time
    let _guard = ASK_LOCK.lock().await;
    ASK_ACTIVE.store(true, Ordering::SeqCst);

    // log the prompt
    log_ask!("{} [y/N]: ", prompt);
    stdout().flush().unwrap();

    // spawn a blocking task to capture single key input
    let res = task::spawn_blocking(move || {
        enable_raw_mode().unwrap(); // enter raw mode
        let mut result = Err(USER_ABORTED_MESSAGE.to_string());

        loop {
            if event::poll(std::time::Duration::from_millis(500)).unwrap() {
                if let Event::Key(KeyEvent { code, .. }) = event::read().unwrap() {
                    match code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            result = Ok(());
                            break;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
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

    ASK_ACTIVE.store(false, Ordering::SeqCst);

    res
}

pub async fn ask_user_and_abort(prompt: &str, silent: bool) {
    if silent {
        return;
    }

    if let Err(_) = ask_user(prompt, false).await {
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

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
}

pub async fn init_logger() -> (tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel::<LogEntry>(100);
    *LOG_SENDER.lock().await = Some(tx.clone());

    let handle =tokio::spawn(async move {
        // let mut last_ask: Option<LogEntry> = None;
        let mut stdout = stdout();
        let mut last_ask: Option<LogEntry> = None;

        while let Some(entry) = rx.recv().await {
            // println!("entry.level.trim()1: --{}--", entry.level.trim());
            if entry.level.trim() != "ASK" && ASK_ACTIVE.load(Ordering::SeqCst) {
                // println!("ASK_ACTIVE --------");
                // execute!(stdout, cursor::MoveUp(1), cursor::MoveToColumn(0), Clear(ClearType::CurrentLine)).unwrap();
                execute!(
                    stdout,
                    cursor::MoveToColumn(0),
                    Clear(ClearType::CurrentLine)
                )
                .unwrap();
            }

            match entry.level.as_str() {
                "INFO" => info!("{}", entry.message),
                "ERROR" => error!("{}", entry.message),
                "WARN" => warn!("{}", entry.message),
                "DEBUG" => debug!("{}", entry.message),
                "REMOTE" => println!(
                    "[{}] {}",
                    ansi_term::Colour::Purple
                        .paint(format!("{:<6}", "REMOTE"))
                        .to_string(),
                    entry.message
                ),
                "LOCAL" => println!(
                    "[{}] {}",
                    ansi_term::Colour::Purple
                        .paint(format!("{:<6}", "LOCAL"))
                        .to_string(),
                    entry.message
                ),
                "ASK" => print!(
                    "[{}] {}",
                    ansi_term::Colour::Cyan
                        .paint(format!("{:<6}", "ASK"))
                        .to_string(),
                    entry.message
                ),
                _ => println!("[{}] {}", entry.level, entry.message),
            }

            // println!("entry.level.trim()2: --{}--", entry.level.trim());
            if entry.level.trim() == "ASK" {
                // println!("ask assignment");
                last_ask = Some(entry);
            } else if entry.level.trim() != "ASK" && ASK_ACTIVE.load(Ordering::SeqCst) {
                // println!("ask 1");
                if let Some(ref ask) = last_ask {
                    // println!("ask: {}", ask.message);
                    execute!(stdout, cursor::MoveToColumn(0)).unwrap();
                    print!(
                        "[{}] {}",
                        ansi_term::Colour::Cyan
                            .paint(format!("{:<6}", "ASK"))
                            .to_string(),
                        ask.message
                    );
                }
            }

            stdout.flush().unwrap();
        }
    });

    handle
}


pub async fn flush_logs_and_exit(logger_handle: tokio::task::JoinHandle<()>) -> ! {
    let tx = {
        let mut guard = LOG_SENDER.lock().await;
        guard.take()
    };
    drop(tx);
    let _ = logger_handle.await;
    std::process::exit(1);
}
