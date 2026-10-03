use ansi_term::Colour;
use clap::{Parser, Subcommand};
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    execute,
    terminal::{Clear, ClearType},
};
use futures::future::join_all;
use log::{Level, error};
use std::collections::HashMap;
use std::io::Write;
use std::io::{self, stdout};
use std::process::exit;
use std::sync::Arc;
use std::thread;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};
mod commands;
mod common;
mod domain;
mod handlers;
mod utils;

use crate::{common::ssh_pool::{PoolOptions, ServerPool}, utils::log_utils::init_logger};
use crate::handlers::command_handler::{
    parse_execute_config_from_cmd, parse_patch_config_from_cmd, parse_run_steps,
    parse_upload_config_from_cmd,
};
use crate::utils::file_utils::load_properties;
use crate::utils::file_utils::load_yaml_config;
use crate::utils::log_utils::{ask_user, ask_user_and_abort};

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("Invalid duration '{}': {}", s, e))
}

#[derive(Parser)]
#[command(name = "rhctl")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long, help = "Path to YAML configuration file")]
    config: Option<String>,

    #[arg(long, help = "Name of the configuration inside the YAML file to use")]
    config_name: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Upload files based on property mappings")]
    Upload {
        #[arg(long, help = "Remote host IP or hostname")]
        host: String,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

        #[arg(long, help = "Remote username")]
        user: String,

        #[arg(long, help = "Remote password")]
        password: Option<String>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        connect_timeout: Option<Duration>,

        #[arg(long, default_value = "false", help = "Use sudo for operations")]
        use_sudo: bool,

        #[arg(
            long,
            default_value = "false",
            help = "Use rsync if available (falls back to scp)"
        )]
        use_rsync: bool,

        #[arg(
            long,
            default_value = "false",
            help = "Silent mode (no prompts, assume yes)"
        )]
        silent: bool,

        #[arg(long, help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,

        #[arg(
            long = "transfer",
            help = "Inline transfer mapping local=remote-dir (repeatable)"
        )]
        transfer: Vec<String>,

        #[arg(long = "transfer-file", help = "Transfer file with local=remote-dir lines")]
        transfer_file: Option<String>,
    },

    #[command(about = "Execute a local bash script remotely")]
    Execute {
        #[arg(long, help = "Remote host IP or hostname")]
        host: String,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

        #[arg(long, help = "Remote username")]
        user: String,

        #[arg(long, help = "Remote password")]
        password: Option<String>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        connect_timeout: Option<Duration>,

        #[arg(long, default_value = "false", help = "Use sudo for operations")]
        use_sudo: bool,

        #[arg(
            long,
            default_value = "false",
            help = "Use rsync if available (falls back to scp)"
        )]
        use_rsync: bool,

        #[arg(
            long,
            default_value = "false",
            help = "Silent mode (no prompts, assume yes)"
        )]
        silent: bool,

        #[arg(long, help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,

        #[arg(long, help = "Local bash script file")]
        script: String,

        #[arg(long, default_value = "~", help = "Remote working path")]
        remote_path: Option<String>,
    },

    #[command(about = "Patch a remote file with a local patch")]
    Patch {
        #[arg(long, help = "Remote host IP or hostname")]
        host: String,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

        #[arg(long, help = "Remote username")]
        user: String,

        #[arg(long, help = "Remote password")]
        password: Option<String>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        connect_timeout: Option<Duration>,

        #[arg(long, default_value = "false", help = "Use sudo for operations")]
        use_sudo: bool,

        #[arg(
            long,
            default_value = "false",
            help = "Use rsync if available (falls back to scp)"
        )]
        use_rsync: bool,

        #[arg(
            long,
            default_value = "false",
            help = "Silent mode (no prompts, assume yes)"
        )]
        silent: bool,

        #[arg(long, help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,

        #[arg(long, help = "Local source file")]
        local_path: String,

        #[arg(long, help = "Remote upload path for patch")]
        remote_upload: String,

        #[arg(long, help = "Remote target file to patch")]
        remote_path: String,

        #[arg(long, help = "Remote backup file path")]
        remote_backup: String,

        #[arg(long, default_value = "false", help = "Recover from backup")]
        recover: bool,
    },
}

#[tokio::main]
async fn main() {
    let log_level = "info";
    env_logger::Builder::new()
        .format(|buf, record| {
            let level_text = format!("{:<6}", record.level().to_string());
            let level_colored = match record.level() {
                Level::Error => Colour::Red.paint(&level_text),
                Level::Warn => Colour::Yellow.paint(&level_text),
                Level::Info => Colour::Green.paint(&level_text),
                Level::Debug => Colour::Blue.paint(&level_text),
                Level::Trace => Colour::Purple.paint(&level_text),
            };
            writeln!(buf, "[{}] {}", level_colored, record.args())
        })
        .filter_level(match log_level {
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();
    init_logger().await;
    // Simulate concurrent logs
    let t1 = tokio::spawn(async {
        sleep(Duration::from_millis(100)).await;
        log_info!("test1");
    });
    let t2 = tokio::spawn(async {
        let _ = ask_user("Do you want to continue1?", false).await;
    });
    let t22 = tokio::spawn(async {
        let _ = ask_user("Do you want to continue2?", false).await;
    });
    let t3 = tokio::spawn(async {
        sleep(Duration::from_millis(200)).await;
        log_info!("test2");
    });
    let t4 = tokio::spawn(async {
        sleep(Duration::from_millis(400)).await;
        log_info!("test3");
    });

      let t5 = tokio::spawn(async {
        sleep(Duration::from_millis(3000)).await;
        log_info!("test4");
    });

      let t6 = tokio::spawn(async {
        sleep(Duration::from_millis(6000)).await;
        log_info!("test5");
    });

      let t7 = tokio::spawn(async {
        sleep(Duration::from_millis(10000)).await;
        log_info!("test6");
    });

    let _ = tokio::join!(t1, t2, t22, t3, t4, t5, t6, t7);
}
