use ansi_term::Colour;
use clap::{Parser, Subcommand};
use futures::future::join_all;
use log::{Level, error};
use std::collections::HashMap;
use std::io::Write;
use std::process::exit;
use std::sync::{Arc};
use std::time::Duration;
use tokio::task::JoinHandle;

mod commands;
mod common;
mod domain;
mod handlers;
mod utils;

use crate::common::ssh::ServerHandle;
use crate::common::ssh_pool::{PoolOptions, ServerPool};
use crate::handlers::command_handler::{
    parse_execute_config_from_cmd, parse_execute_configs, parse_patch_config_from_cmd,
    parse_patch_configs, parse_upload_config_from_cmd, parse_upload_configs,
};
use crate::handlers::validation_handler::validate_cli_args;
use crate::utils::file_utils::{load_properties};
use crate::utils::log_utils::ask_user_and_abort;
use crate::{
    utils::file_utils::load_yaml_config,
};

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("Invalid duration '{}': {}", s, e))
}

#[derive(Parser)]
#[command(name = "sbxctl")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(author = "Craig Brown")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long, help = "Path to YAML configuration file")]
    config: Option<String>,

    #[arg(long, help = "Name of the configuration inside the YAML file to use")]
    config_name: Option<String>,

    #[arg(long, value_parser = parse_var, help = "Global variable in KEY=VALUE format, can be specified multiple times")]
    var: Vec<(String, String)>,
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

        #[arg(long, help = "Path to properties file")]
        properties_file: String,
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

// Parse KEY=VALUE format for --var
fn parse_var(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!("Invalid --var format: '{}'. Expected KEY=VALUE", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Convert CLI vars to HashMap
    let cli_vars: HashMap<String, String> = cli.var.iter().cloned().collect();

    // Determine log level
    let log_level = cli_vars
        .get("LOG_LEVEL")
        .map(|s| s.as_str())
        .unwrap_or_else(|| match &cli.command {
            Some(Commands::Upload { log_level, .. })
            | Some(Commands::Execute { log_level, .. })
            | Some(Commands::Patch { log_level, .. }) => log_level.as_deref().unwrap_or("info"),
            None => "info",
        });

    // Initialize logging
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

    // Validate CLI arguments
    validate_cli_args(&cli).await;

    // Load YAML config if provided
    let yaml_config = cli
        .config
        .as_ref()
        .map(|path| load_yaml_config(path))
        .transpose()
        .unwrap_or_else(|err| {
            log_error!("{}", err);
            exit(1);
        });

    // Thread handles for parallel execution
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    let options = PoolOptions {
        max_connections: 10,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Some(Duration::from_secs(600)), // 10min default
        max_channel_per_session: 5,
    };
    let global_server_pool = Arc::new(ServerPool::new(options));
    global_server_pool.clone().start_idle_cleanup(Duration::from_secs(10));
    if let Some(config_name) = &cli.config_name {
        // YAML config mode
        let yml_config = yaml_config.as_ref().unwrap_or_else(|| {
            log_error!("YAML config required when --config-name is provided");
            exit(1);
        });

        let named_config = yml_config
            .configs
            .as_ref()
            .and_then(|configs| configs.iter().find(|c| c.name == *config_name))
            .unwrap_or_else(|| {
                log_error!("Config '{}' not found in YAML file", config_name);
                exit(1);
            });

        if !cli_vars.is_empty() {
            ask_user_and_abort(
                "CLI --var arguments are ignored when using YAML config. Using vars from YAML instead, Continue?",
                false,
            ).await;
        }
        // Parse all command configs with vars
        let upload_configs = parse_upload_configs(&named_config, yml_config);
        let execute_configs = parse_execute_configs(&named_config, yml_config);
        let patch_configs = parse_patch_configs(&named_config, yml_config);

        // Spawn threads for upload commands
        for (config, vars) in upload_configs {
            let server_handle = ServerHandle {
                server_metadata: Arc::new(config.clone()),
                global_server_pool: global_server_pool.clone(),
            };
            let mut mappings = HashMap::new();
            if let Err(e) = load_properties(
                config.properties_file.as_str(),
                &mut mappings,
                &yml_config.var_map,
            ) {
                log_error!(
                    "{}",
                    format!(
                        "Failed to load properties file '{}'. \n\t{}",
                        config.properties_file, e
                    )
                );
                exit(1);
            }

            let handle = tokio::spawn(async move {
                if let Err(e) = server_handle
                    .check_global_remote_temp_dir(config.use_sudo, config.silent)
                    .await
                {
                    log_error!("{}", e);
                    exit(1);
                }
                let result = commands::upload::run(&config, &server_handle, &vars).await;
                if let Err(e) = server_handle.delete_global_temp_dir(config.use_sudo).await {
                    log_error!("{}", e);
                }
                if let Err(e) = result {
                    log_error!(
                        "Upload failed for {}@{}: \n\t{}",
                        config.user, config.host, e
                    );
                    exit(1);
                }
            });
            tasks.push(handle);
        }

        // Spawn threads for execute commands
        for (config, _) in execute_configs {
            let server_handle = ServerHandle {
                server_metadata: Arc::new(config.clone()),
                global_server_pool: global_server_pool.clone(),
            };
            let handle = tokio::spawn(async move {
                if let Err(e) = server_handle
                    .check_global_remote_temp_dir(config.use_sudo, config.silent)
                    .await
                {
                    log_error!("{}", e);
                    exit(1);
                }
                let result = commands::execute::run(&config, &server_handle).await;
                if let Err(e) = server_handle.delete_global_temp_dir(config.use_sudo).await {
                    log_error!("{}", e);
                }
                if let Err(e) = result {
                    log_error!(
                        "Execute failed for {}@{}: \n\t{}",
                        config.user, config.host, e
                    );
                    exit(1);
                }
            });
            tasks.push(handle);
        }

        // Spawn threads for patch commands
        for (config, _) in patch_configs {
            let server_handle = ServerHandle {
                server_metadata: Arc::new(config.clone()),
                global_server_pool: global_server_pool.clone(),
            };
            let handle = tokio::spawn(async move {
                if let Err(e) = server_handle
                    .check_global_remote_temp_dir(config.use_sudo, config.silent)
                    .await
                {
                    log_error!("{}", e);
                    exit(1);
                }
                let result = commands::patch::run(&config, &server_handle).await;
                if let Err(e) = server_handle.delete_global_temp_dir(config.use_sudo).await {
                    log_error!("{}", e);
                }
                if let Err(e) = result {
                    log_error!(
                        "Patch failed for {}@{}: \n\t{}",
                        config.user, config.host, e
                    );
                    exit(1);
                }
            });
            tasks.push(handle);
        }
    } else if let Some(command) = cli.command {
        // CLI mode
        match command {
            Commands::Upload {
                host,
                ssh_port,
                user,
                password,
                connect_timeout,
                use_sudo,
                use_rsync,
                silent,
                properties_file,
                ..
            } => {
                let config = parse_upload_config_from_cmd(
                    host,
                    user,
                    ssh_port,
                    password,
                    connect_timeout,
                    use_sudo,
                    use_rsync,
                    silent,
                    properties_file,
                    &cli_vars,
                );
                let mut mappings = HashMap::new();
                if let Err(e) =
                    load_properties(config.properties_file.as_str(), &mut mappings, &cli_vars)
                {
                    log_error!(
                        "{}",
                        format!(
                            "Failed to load properties file '{}'. \n\t{}",
                            config.properties_file, e
                        )
                    );
                    exit(1);
                }
                let server_handle = ServerHandle {
                    server_metadata: Arc::new(config.clone()),
                    global_server_pool: global_server_pool.clone(),
                };
                let handle = tokio::spawn(async move {
                    if let Err(e) = server_handle
                        .check_global_remote_temp_dir(config.use_sudo, config.silent)
                        .await
                    {
                        log_error!("{}", e);
                        exit(1);
                    }
                    let result = commands::upload::run(&config, &server_handle, &mappings).await;
                    if let Err(e) = server_handle.delete_global_temp_dir(config.use_sudo).await {
                        log_error!("{}", e);
                    }
                    if let Err(e) = result {
                        log_error!(
                            "Upload failed for {}@{}: \n\t{}",
                            config.user, config.host, e
                        );
                        exit(1);
                    }
                });
                tasks.push(handle);
            }
            Commands::Execute {
                host,
                ssh_port,
                user,
                password,
                connect_timeout,
                use_sudo,
                use_rsync,
                silent,
                script,
                remote_path,
                ..
            } => {
                let config = parse_execute_config_from_cmd(
                    host,
                    user,
                    ssh_port,
                    password,
                    connect_timeout,
                    use_sudo,
                    use_rsync,
                    silent,
                    script,
                    remote_path,
                    &cli_vars,
                );
                let server_handle = ServerHandle {
                    server_metadata: Arc::new(config.clone()),
                    global_server_pool: global_server_pool.clone(),
                };
                let handle = tokio::spawn(async move {
                    if let Err(e) = server_handle
                        .check_global_remote_temp_dir(config.use_sudo, config.silent)
                        .await
                    {
                        log_error!("{}", e);
                        exit(1);
                    }
                    let result = commands::execute::run(&config, &server_handle).await;
                    if let Err(e) = server_handle.delete_global_temp_dir(config.use_sudo).await {
                        log_error!("{}", e);
                    }
                    if let Err(e) = result {
                        log_error!(
                            "Execute failed for {}@{}: \n\t{}",
                            config.user, config.host, e
                        );
                        exit(1);
                    }
                });
                tasks.push(handle);
            }
            Commands::Patch {
                host,
                ssh_port,
                user,
                password,
                connect_timeout,
                use_sudo,
                use_rsync,
                silent,
                local_path,
                remote_upload,
                remote_path,
                remote_backup,
                recover,
                ..
            } => {
                let config = parse_patch_config_from_cmd(
                    host,
                    user,
                    ssh_port,
                    password,
                    connect_timeout,
                    use_sudo,
                    use_rsync,
                    silent,
                    recover,
                    local_path,
                    remote_upload,
                    remote_path,
                    remote_backup,
                    &cli_vars,
                );
                let server_handle = ServerHandle {
                    server_metadata: Arc::new(config.clone()),
                    global_server_pool: global_server_pool.clone(),
                };
                let handle = tokio::spawn(async move {
                    if let Err(e) = server_handle
                        .check_global_remote_temp_dir(config.use_sudo, config.silent)
                        .await
                    {
                        log_error!("{}", e);
                        exit(1);
                    }
                    let result = commands::patch::run(&config, &server_handle).await;
                    if let Err(e) = server_handle.delete_global_temp_dir(config.use_sudo).await {
                        log_error!("{}", e);
                    }
                    if let Err(e) = result {
                        log_error!(
                            "Patch failed for {}@{}: \n\t{}",
                            config.user, config.host, e
                        );
                        exit(1);
                    }
                });
                tasks.push(handle);
            }
        }
    }

    // Collect results
    let results = join_all(tasks).await;
    let mut errors = Vec::new();
    for result in results {
        if let Err(e) = result {
            errors.push(format!("Task failed: {:?}", e));
        }
    }
    if !errors.is_empty() {
        log_error!("Errors occurred:\n{}", errors.join("\n"));
        exit(1);
    }
}
