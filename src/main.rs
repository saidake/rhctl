use clap::{Parser, Subcommand};
use log::{error, info};
use rpassword::prompt_password;
use std::path::Path;
use std::process::exit;

mod commands;
mod common;
mod domain;

use common::config::{load_yaml_config, Config};
use common::ssh::SshSession;

use crate::common::config::ConfigWrapper;

#[derive(Parser)]
#[command(name = "sbxctl")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(version = "0.1.0")]
struct Cli {
    #[arg(long, help = "Remote host IP or hostname")]
    host: Option<String>,

    #[arg(long, default_value = "22", help = "Remote SSH port")]
    port: Option<u16>,

    #[arg(long, help = "Remote username")]
    user: Option<String>,

    #[arg(long, help = "Remote password")]
    password: Option<String>,

    #[arg(long, help = "Path to YAML configuration file")]
    config: Option<String>,

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

    #[arg(
        long,
        default_value = "info",
        help = "Log level (debug, info, warn, error)"
    )]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Upload files based on property mappings")]
    Upload {
        #[arg(long, help = "Path to properties file")]
        properties: Option<String>,

        #[arg(long, help = "Assets root directory")]
        assets_root: Option<String>,
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
        local_patch: Option<String>,

        #[arg(long, help = "Remote upload path for patch")]
        remote_upload: Option<String>,

        #[arg(long, help = "Remote target file to patch")]
        remote_file: Option<String>,

        #[arg(long, help = "Remote backup file path")]
        remote_backup: Option<String>,

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

    // Load config from YAML if provided
    let yaml_config = cli
        .config
        .as_ref()
        .map(|path| load_yaml_config(path))
        .transpose()
        .unwrap_or_default();

    // Merge YAML config with CLI args (CLI args take precedence)
    let config = ConfigWrapper {
        host: cli
            .host
            .or_else(|| yaml_config.as_ref().and_then(|c| c.remote.host.clone()))
            .unwrap_or_default(),
        port: cli
            .port
            .unwrap_or(yaml_config.as_ref().map_or(22, |c| c.remote.ssh_port)),
        user: cli
            .user
            .or_else(|| yaml_config.as_ref().and_then(|c| c.remote.user.clone()))
            .unwrap_or_default(),
        password: cli
            .password
            .or_else(|| yaml_config.as_ref().and_then(|c| c.remote.password.clone())),
        use_sudo: cli.use_sudo,
        use_rsync: cli.use_rsync,
        silent: cli.silent,
        upload: yaml_config
            .as_ref()
            .map(|c| c.upload.clone())
            .unwrap_or_default(),
        execute: yaml_config
            .as_ref()
            .map(|c| c.execute.clone())
            .unwrap_or_default(),
        patch: yaml_config
            .as_ref()
            .map(|c| c.patch.clone())
            .unwrap_or_default(),
    };

    // Validate required fields
    if config.host.is_empty() || config.user.is_empty() {
        error!("Missing required config: host and user must be provided via --host/--user or config file.");
        exit(1);
    }

    // Prompt for password if not provided
    let password = match config.password {
        Some(pwd) => pwd,
        None => match prompt_password("Enter SSH password: ") {
            Ok(pwd) if !pwd.is_empty() => pwd,
            _ => {
                error!("Password is required.");
                exit(1);
            }
        },
    };

    // Create final config with password
    let final_config = ConfigWrapper {
        password: Some(password),
        ..config
    };

    info!(
        "Connecting to {}@{}:{}",
        final_config.user, final_config.host, final_config.port
    );

    // Create SSH session
    let session = match SshSession::new(&final_config) {
        Ok(s) => s,
        Err(e) => {
            error!("SSH connection failed: {}", e);
            exit(1);
        }
    };

    match cli.command {
        Commands::Upload {
            properties,
            assets_root,
        } => {
            let properties =
                properties.unwrap_or_else(|| final_config.upload.properties_file.clone());
            let assets_root =
                assets_root.unwrap_or_else(|| final_config.upload.assets_root.clone());
            if properties.is_empty() || assets_root.is_empty() {
                error!("Missing required arguments: --properties and --assets-root must be provided via CLI or config file.");
                exit(1);
            }
            if !Path::new(&properties).exists() {
                error!("Properties file not found: '{}'", properties);
                exit(1);
            }
            if !Path::new(&assets_root).exists() {
                error!("Assets root directory not found: '{}'", assets_root);
                exit(1);
            }
            if let Err(e) =
                commands::upload::run(&session, &final_config, &properties, &assets_root)
            {
                log_error_with_lock!("Upload failed: {}", e);
                exit(1);
            }
        }
        Commands::Execute {
            script,
            remote_path,
        } => {
            if let Err(e) = commands::execute::run(&session, &final_config, &script, &remote_path) {
                error!("Execute failed: {}", e);
                exit(1);
            }
        }
        Commands::Patch {
            local_patch,
            remote_upload,
            remote_file,
            remote_backup,
            recover,
        } => {
            let local_patch = local_patch.unwrap_or_else(|| final_config.patch.local_patch.clone());
            let remote_upload =
                remote_upload.unwrap_or_else(|| final_config.patch.remote_upload.clone());
            let remote_file = remote_file.unwrap_or_else(|| final_config.patch.remote_file.clone());
            let remote_backup =
                remote_backup.unwrap_or_else(|| final_config.patch.remote_backup.clone());
            if !recover
                && (local_patch.is_empty()
                    || remote_upload.is_empty()
                    || remote_file.is_empty()
                    || remote_backup.is_empty())
            {
                error!("Missing required arguments: --local-patch, --remote-upload, --remote-file, and --remote-backup must be provided via CLI or config file.");
                exit(1);
            }
            if let Err(e) = commands::patch::run(
                &session,
                &final_config,
                &local_patch,
                &remote_upload,
                &remote_file,
                &remote_backup,
                recover,
            ) {
                error!("Patch failed: {}", e);
                exit(1);
            }
        }
    }
}
