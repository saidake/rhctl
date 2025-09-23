use clap::{Parser, Subcommand};
use log::{debug, error};
use std::{path::Path, process::exit};
use ansi_term::Colour;
use log::Level;
use std::io::Write;
mod commands;
mod common;
mod domain;
mod handlers;

use common::config::load_yaml_config;

use crate::{
    domain::cmd_params::{ExecuteCmdConfig, PatchCmdConfig, UploadCmdConfig},
    handlers::command_handler::{merge_execute, merge_patch, merge_upload},
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
            let level_str = match record.level() {
                Level::Error => Colour::Red.paint("ERROR"),
                Level::Warn  => Colour::Yellow.paint("WARN"),
                Level::Info  => Colour::Green.paint("INFO"),
                Level::Debug => Colour::Blue.paint("DEBUG"),
                Level::Trace => Colour::Purple.paint("TRACE"),
            };
            writeln!(buf, "[{}] {}", level_str, record.args())
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

            if !Path::new(&config.properties_file).exists() {
                error!("Properties file not found: '{}'", config.properties_file);
                exit(1);
            }
            if !Path::new(&config.assets_root).exists() {
                error!("Assets root directory not found: '{}'", config.assets_root);
                exit(1);
            }
            if let Err(e) = commands::upload::run(&config) {
                log_error_with_lock!("Upload failed. \n\t{}", e);
                exit(1);
            }
        }

        Commands::Execute { .. } => {
            let config: ExecuteCmdConfig = merge_execute(&cli.command, yaml_config);
            if let Err(e) = commands::execute::run(&config) {
                error!("Execute failed. \n\t{}", e);
                exit(1);
            }
        }

        Commands::Patch { .. } => {
            let config: PatchCmdConfig = merge_patch(&cli.command, yaml_config);
            if let Err(e) = commands::patch::run(&config) {
                error!("Patch failed. \n\t{}", e);
                exit(1);
            }
        }
    }
}
