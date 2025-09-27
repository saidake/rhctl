use ansi_term::Colour;
use clap::{Parser, Subcommand};
use log::Level;
use log::{debug, error};
use std::io::Write;
use std::{path::Path, process::exit};
mod commands;
mod common;
mod domain;
mod handlers;
mod utils;

use crate::utils::ssh_utils::connect_ssh;
use crate::{
    domain::cmd_params::{ExecuteCmdConfig, PatchCmdConfig, UploadCmdConfig},
    handlers::command_handler::{merge_execute, merge_patch, merge_upload},
    utils::file_utils::load_yaml_config,
};

#[derive(Parser)]
#[command(name = "sbxctl")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Upload files based on property mappings")]
    Upload {
        #[arg(long, help = "Remote host IP or hostname")]
        host: Option<String>,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

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

        #[arg(long, help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,

        #[arg(long, help = "Path to properties file")]
        properties_file: Option<String>,

        #[arg(long, help = "Assets root directory")]
        assets_root: Option<String>,
    },

    #[command(about = "Execute a local bash script remotely")]
    Execute {
        #[arg(long, help = "Remote host IP or hostname")]
        host: Option<String>,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

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

        #[arg(long, help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,

        #[arg(help = "Local bash script file")]
        script: Option<String>,

        #[arg(long, default_value = "~", help = "Remote working path")]
        remote_path: Option<String>,
    },

    #[command(about = "Patch a remote file with a local patch")]
    Patch {
        #[arg(long, help = "Remote host IP or hostname")]
        host: Option<String>,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

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

        #[arg(long, help = "Log level (debug, info, warn, error)")]
        log_level: Option<String>,

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
    //[CORE] Common data
    let log_level = match &cli.command {
        Commands::Upload { log_level, .. }
        | Commands::Execute { log_level, .. }
        | Commands::Patch { log_level, .. } => log_level.as_deref().unwrap_or("info"),
    };
    let config_path = match &cli.command {
        Commands::Upload { config, .. } => config,
        Commands::Execute { config, .. } => config,
        Commands::Patch { config, .. } => config,
    };
    // Initialize logging
    env_logger::builder()
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

    // Load config from YAML if provided
    let yaml_config = config_path
        .as_ref()
        .map(|path| load_yaml_config(path))
        .transpose()
        .unwrap_or_else(|err| {
            error!("{}", err);
            std::process::exit(1);
        });
    debug!("yaml_config.is_none(): {}", yaml_config.is_none());
    //[CORE] Merge YAML config with CLI args (CLI args take precedence)

    match cli.command {
        Commands::Upload { .. } => {
            let config: UploadCmdConfig = merge_upload(&cli.command, yaml_config);
            let session = connect_ssh(
                config.host.clone(),
                config.user.clone(),
                config.ssh_port,
                config.password.clone(),
            );
            // Ensure global temp dir exists before starting uploads
            if let Err(e) = session.check_global_remote_temp_dir(config.use_sudo, config.silent) {
                error!("{}", e);
                exit(1);
            }
            let result = commands::upload::run(&config, &session);
            if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                error!("{}", e);
            }
            if let Err(e) = result {
                error!("Patch failed. \n\t{}", e);
                exit(1);
            }
        }

        Commands::Execute { .. } => {
            let config: ExecuteCmdConfig = merge_execute(&cli.command, yaml_config);
            let session = connect_ssh(
                config.host.clone(),
                config.user.clone(),
                config.ssh_port,
                config.password.clone(),
            );
            // Ensure global temp dir exists before starting uploads
            if let Err(e) = session.check_global_remote_temp_dir(config.use_sudo, config.silent) {
                error!("{}", e);
                exit(1);
            }
            let result = commands::execute::run(&config, &session);
            if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                error!("{}", e);
            }
            if let Err(e) = result {
                error!("Patch failed. \n\t{}", e);
                exit(1);
            }
        }

        Commands::Patch { .. } => {
            let config: PatchCmdConfig = merge_patch(&cli.command, yaml_config);
            let session = connect_ssh(
                config.host.clone(),
                config.user.clone(),
                config.ssh_port,
                config.password.clone(),
            );
            // Ensure global temp dir exists before starting uploads
            if let Err(e) = session.check_global_remote_temp_dir(config.use_sudo, config.silent) {
                error!("{}", e);
                exit(1);
            }
            let result = commands::patch::run(&config, &session);
            if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                error!("{}", e);
            }
            if let Err(e) = result {
                error!("Patch failed. \n\t{}", e);
                exit(1);
            }
        }
    }
}
