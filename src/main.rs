// src/main.rs
use clap::{Parser, Subcommand};
use log::{error, info};
use std::env;
use std::process::exit;

mod commands;
mod common;

use common::config::Config;
use common::ssh::SshSession;

#[derive(Parser)]
#[command(name = "remote-tool")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(version = "0.1.0")]
struct Cli {
    #[arg(long, help = "Remote host IP or hostname")]
    host: Option<String>,

    #[arg(long, default_value = "22", help = "Remote SSH port")]
    port: Option<u16>,

    #[arg(long, help = "Remote username")]
    user: Option<String>,

    #[arg(long, help = "Remote password (use environment variable for security)")]
    password: Option<String>,

    #[arg(long, default_value = "false", help = "Use sudo for operations")]
    sudo: bool,

    #[arg(long, default_value = "false", help = "Use rsync if available (falls back to scp)")]
    rsync: bool,

    #[arg(long, default_value = "false", help = "Silent mode (no prompts, assume yes)")]
    silent: bool,

    #[arg(long, default_value = "info", help = "Log level (debug, info, warn, error)")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Upload files based on property mappings")]
    Upload {
        #[arg(long, help = "Path to properties file")]
        properties: String,

        #[arg(long, help = "Assets root directory")]
        assets_root: String,
    },

    #[command(about = "Execute a local bash script remotely")]
    Execute {
        #[arg(help = "Local bash script file")]
        script: String,

        #[arg(long, default_value = "~", help = "Remote working path")]
        remote_path: String,
    },

    #[command(about = "Patch a remote file with a local patch")]
    Patch {
        #[arg(long, help = "Local patch file")]
        local_patch: String,

        #[arg(long, help = "Remote upload path for patch")]
        remote_upload: String,

        #[arg(long, help = "Remote target file to patch")]
        remote_file: String,

        #[arg(long, help = "Remote backup file path")]
        remote_backup: String,

        #[arg(long, default_value = "false", help = "Recover from backup")]
        recover: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    env_logger::builder()
        .filter_level(match cli.log_level.as_str() {
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .init();

    // Load config, prefer env vars over CLI args for security
    let config = Config {
        host: cli.host.unwrap_or_else(|| env::var("REMOTE_HOST").unwrap_or_default()),
        port: cli.port.unwrap_or_else(|| env::var("REMOTE_SSH_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(22)),
        user: cli.user.unwrap_or_else(|| env::var("REMOTE_USER").unwrap_or_default()),
        password: cli.password.unwrap_or_else(|| env::var("REMOTE_PWD").unwrap_or_default()),
        sudo: cli.sudo,
        rsync: cli.rsync,
        silent: cli.silent,
    };

    if config.host.is_empty() || config.user.is_empty() || config.password.is_empty() {
        error!("Missing required config: host, user, or password.");
        exit(1);
    }

    info!("Connecting to {}@{}:{}", config.user, config.host, config.port);

    // Create SSH session
    let session = match SshSession::new(&config) {
        Ok(s) => s,
        Err(e) => {
            error!("SSH connection failed: {}", e);
            exit(1);
        }
    };

    match cli.command {
        Commands::Upload { properties, assets_root } => {
            if let Err(e) = commands::upload::run(&session, &config, &properties, &assets_root) {
                error!("Upload failed: {}", e);
                exit(1);
            }
        }
        Commands::Execute { script, remote_path } => {
            if let Err(e) = commands::execute::run(&session, &config, &script, &remote_path) {
                error!("Execute failed: {}", e);
                exit(1);
            }
        }
        Commands::Patch { local_patch, remote_upload, remote_file, remote_backup, recover } => {
            if let Err(e) = commands::patch::run(&session, &config, &local_patch, &remote_upload, &remote_file, &remote_backup, recover) {
                error!("Patch failed: {}", e);
                exit(1);
            }
        }
    }
}