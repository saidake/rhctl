use ansi_term::Colour;
use clap::{Parser, Subcommand};
use futures::future::join_all;
use log::Level;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

mod commands;
mod common;
mod domain;
mod handlers;
mod utils;

use crate::common::ssh_pool::{ServerOptions, ServerPool};
use crate::domain::constants::{EXECUTE_TASK_NAME, PATCH_TASK_NAME, UPLOAD_TASK_NAME};
use crate::handlers::command_handler::{
    parse_execute_config_from_cmd, parse_execute_configs, parse_patch_config_from_cmd,
    parse_patch_configs, parse_upload_config_from_cmd, parse_upload_configs,
};
use crate::utils::file_utils::load_properties;
use crate::utils::file_utils::load_yaml_config;
use crate::utils::log_utils::{ask_user_and_abort, ask_user_and_abort_option, flush_logs_and_exit, init_logger};

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("Invalid duration '{}': {}", s, e))
}

#[derive(Parser)]
#[command(name = "sbxctl")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(author = "Craig Brown")]
#[command(override_usage = "sbxctl [COMMAND] [OPTIONS]")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(
        long,
        global = true,
        help = "Global log level (debug, info, warn, error)"
    )]
    log_level: Option<String>,

    #[arg(
        long,
        global = true, 
        value_parser = parse_var, 
        help = "Global variable in KEY=VALUE format, can be specified multiple times")]
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

        #[arg(long, help = "Path to properties file")]
        properties_file: String,



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

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        connect_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_channels_per_session: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_sessions_per_server: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        session_acquire_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_session_lifetime: Option<Duration>,
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

        #[arg(long, help = "Local bash script file")]
        script: String,

        #[arg(long, default_value = "~", help = "Remote working path")]
        remote_path: Option<String>,


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

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        connect_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_channels_per_session: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_sessions_per_server: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        session_acquire_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_session_lifetime: Option<Duration>,
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

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        connect_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_channels_per_session: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_sessions_per_server: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        session_acquire_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_session_lifetime: Option<Duration>,


    },

    #[command(about = "Run using YAML configuration file")]
    Run {
        #[arg(long, help = "Path to YAML configuration file")]
        config: String,

        #[arg(long, help = "Name of the configuration inside the YAML file to use")]
        config_name: String,
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
        .or_else(|| cli.log_level.as_deref())
        .unwrap_or("info");
    // println!("log_level: {}", log_level);

    // Initialize logging
    env_logger::Builder::new()
        .format(|buf, record| {
            writeln!(buf, "{}", record.args())
        })
        .filter_level(match log_level {
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        })
        .filter_module("russh", log::LevelFilter::Info)
        .filter_module("russh_keys", log::LevelFilter::Info)
        .init();

    let log_handle = init_logger().await;

    // Thread handles for parallel execution
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    let global_server_pool = Arc::new(ServerPool::new());
    global_server_pool
        .clone()
        .start_idle_cleanup(Duration::from_secs(10));

    // CLI mode
    match cli.command {
        Commands::Upload {
            host,
            ssh_port,
            user,
            password,

            properties_file,

            use_sudo,
            use_rsync,
            silent,
            connect_timeout,
            max_channels_per_session,
            max_sessions_per_server,
            session_acquire_timeout,
            max_session_lifetime,
            ..
        } => {
            let config = parse_upload_config_from_cmd(
                &host,
                &user,
                ssh_port,
                password,
                &properties_file,

                use_sudo,
                use_rsync,
                silent,
                connect_timeout,
                max_channels_per_session,
                max_sessions_per_server,
                session_acquire_timeout,
                max_session_lifetime,

                &cli_vars,
            );
            let mut mappings = HashMap::new();
            if let Err(e) =
                load_properties(config.properties_file.as_str(), &mut mappings, &cli_vars)
            {
                log_error_direct!(user.as_str(), host.as_str(), UPLOAD_TASK_NAME,
                    "{}",
                    format!(
                        "Failed to load properties file '{}'. \n\t{}",
                        config.properties_file, e
                    )
                );
                flush_logs_and_exit(log_handle).await;
            }
            // println!("test---------");
            let server_metadata=Arc::new(config.server_metadata.clone());
            let global_server_pool_clone = global_server_pool.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = global_server_pool_clone
                    .check_global_remote_temp_dir(&server_metadata,UPLOAD_TASK_NAME, config.use_sudo, config.silent)
                    .await
                {
                    log_error!(&server_metadata,UPLOAD_TASK_NAME,"{}", e);
                    flush_logs_and_exit(log_handle).await;
                }
                let result = commands::upload::run(&config,  &mappings, &server_metadata, global_server_pool_clone.clone()).await;
                if let Err(e) = result {
                    log_error!(&server_metadata,UPLOAD_TASK_NAME,
                        "Upload failed for {}@{}: \n\t{}",
                        server_metadata.user,
                        server_metadata.host,
                        e
                    );
                    flush_logs_and_exit(log_handle).await;
                }
            });
            tasks.push(handle);
        }
        Commands::Execute {
            host,
            ssh_port,
            user,
            password,
            script,
            remote_path,

            use_sudo,
            use_rsync,
            silent,
            connect_timeout,
            max_channels_per_session,
            max_sessions_per_server,
            session_acquire_timeout,
            max_session_lifetime,
            ..
        } => {
            let config = parse_execute_config_from_cmd(
                &host,
                &user,
                ssh_port,
                password,
                &script,
                remote_path,

                use_sudo,
                use_rsync,
                silent,
                connect_timeout,
                max_channels_per_session,
                max_sessions_per_server,
                session_acquire_timeout,
                max_session_lifetime,
                &cli_vars,
            );
            let server_metadata=Arc::new(config.server_metadata.clone());
            let global_server_pool_clone = global_server_pool.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = global_server_pool_clone
                    .check_global_remote_temp_dir(&server_metadata, EXECUTE_TASK_NAME, config.use_sudo, config.silent)
                    .await
                {
                    log_error!(&server_metadata, EXECUTE_TASK_NAME,"{}", e);
                    flush_logs_and_exit(log_handle).await;
                }
                let result = commands::execute::run(&config, &server_metadata, global_server_pool_clone.clone()).await;
                if let Err(e) = result {
                    log_error!(&server_metadata, EXECUTE_TASK_NAME,
                        "Execute failed for {}@{}: \n\t{}",
                        server_metadata.user,
                        server_metadata.host,
                        e
                    );
                    flush_logs_and_exit(log_handle).await;
                }
            });
            tasks.push(handle);
        }
        Commands::Patch {
            host,
            ssh_port,
            user,
            password,

            local_path,
            remote_upload,
            remote_path,
            remote_backup,
            recover,
            
            use_sudo,
            use_rsync,
            silent,
            connect_timeout,
            max_channels_per_session,
            max_sessions_per_server,
            session_acquire_timeout,
            max_session_lifetime,
            ..
        } => {
            let config = parse_patch_config_from_cmd(
                &host,
                &user,
                ssh_port,
                password,

                recover,
                &local_path,
                &remote_upload,
                &remote_path,
                &remote_backup,
                
                use_sudo,
                use_rsync,
                silent,
                connect_timeout,
                max_channels_per_session,
                max_sessions_per_server,
                session_acquire_timeout,
                max_session_lifetime,

                &cli_vars,
            );
            let server_metadata=Arc::new(config.server_metadata.clone());
            let global_server_pool_clone = global_server_pool.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = 
                global_server_pool_clone.check_global_remote_temp_dir(&server_metadata,PATCH_TASK_NAME, config.use_sudo, config.silent)
                    .await
                {
                    log_error!(&server_metadata,PATCH_TASK_NAME,"{}", e);
                    flush_logs_and_exit(log_handle).await;
                }
                let result = commands::patch::run(&config, &server_metadata, global_server_pool_clone.clone()).await;
                if let Err(e) = result {
                    log_error!(&server_metadata,PATCH_TASK_NAME,
                        "Patch failed for {}@{}: \n\t{}",
                        server_metadata.user,
                        server_metadata.host,
                        e
                    );
                    flush_logs_and_exit(log_handle).await;
                }
            });
            tasks.push(handle);
        }
        Commands::Run {
            config,
            config_name,
            ..
        } => {
            // Load YAML config if provided
            let yml_config = match load_yaml_config(&config) {
                Ok(cfg) => cfg,
                Err(err) => {
                    log_error_root!("{}", err);
                    flush_logs_and_exit(log_handle).await;
                }
            };

            let named_config = match yml_config
                .configs
                .as_ref()
                .and_then(|configs| configs.iter().find(|c| c.name == *config_name))
            {
                Some(c) => c,
                None => {
                    log_error_root!("Config '{}' not found in YAML file", config_name);
                    flush_logs_and_exit(log_handle).await;
                }
            };

            if !cli_vars.is_empty() {
                ask_user_and_abort_option(None, None,
                "CLI --var arguments are ignored when using YAML config. Using vars from YAML instead, Continue?",
                false,
            ).await;
            }
            // Parse all command configs with vars
            let upload_configs = 
            parse_upload_configs(&named_config, &yml_config);
            let execute_configs = 
            parse_execute_configs(&named_config, &yml_config);
            let patch_configs = 
            parse_patch_configs(&named_config, &yml_config);

            // Spawn threads for upload commands
            for (config, vars) in upload_configs {
            let server_metadata=Arc::new(config.server_metadata.clone());
                let mut mappings = HashMap::new();
                if let Err(e) = load_properties(
                    config.properties_file.as_str(),
                    &mut mappings,
                    &yml_config.var_map,
                ) {
                    log_error_root!(
                        "{}",
                        format!(
                            "Failed to load properties file '{}'. \n\t{}",
                            config.properties_file, e
                        )
                    );
                    flush_logs_and_exit(log_handle).await;
                }
                let global_server_pool_clone = global_server_pool.clone();
                let handle = tokio::spawn(async move {
                    if let Err(e) = global_server_pool_clone
                        .check_global_remote_temp_dir(&server_metadata,UPLOAD_TASK_NAME, config.use_sudo, config.silent)
                        .await
                    {
                        log_error!(&server_metadata, UPLOAD_TASK_NAME, "{}", e);
                        return;
                    }
                    let result = commands::upload::run(&config,  &vars, &server_metadata, global_server_pool_clone.clone()).await;
                    if let Err(e) = result {
                        log_error!(
                            &server_metadata,
                            UPLOAD_TASK_NAME,
                            "Upload failed for {}@{}: \n\t{}",
                            server_metadata.user,
                            server_metadata.host,
                            e
                        );
                    }
                });
                tasks.push(handle);
            }

            // Spawn threads for execute commands
            for (config, _) in execute_configs {
                let server_metadata=Arc::new(config.server_metadata.clone());
                let global_server_pool_clone = global_server_pool.clone();
                let handle = tokio::spawn(async move {
                    if let Err(e) = global_server_pool_clone
                        .check_global_remote_temp_dir(&server_metadata, EXECUTE_TASK_NAME, config.use_sudo, config.silent)
                        .await
                    {
                        log_error!(&server_metadata, EXECUTE_TASK_NAME, "{}", e);
                        return;
                    }
                    let result = commands::execute::run(&config, &server_metadata, global_server_pool_clone.clone()).await;
                    if let Err(e) = result {
                        log_error!(&server_metadata, EXECUTE_TASK_NAME,
                            "Execute failed for {}@{}: \n\t{}",
                            server_metadata.user,
                            server_metadata.host,
                            e
                        );
                    }
                });
                tasks.push(handle);
            }

            // Spawn threads for patch commands
            for (config, _) in patch_configs {
                let server_metadata=Arc::new(config.server_metadata.clone());
                let global_server_pool_clone = global_server_pool.clone();
                let handle = tokio::spawn(async move {
                    if let Err(e) = global_server_pool_clone
                        .check_global_remote_temp_dir(&server_metadata, PATCH_TASK_NAME, config.use_sudo, config.silent)
                        .await
                    {
                        log_error!(&server_metadata, PATCH_TASK_NAME,"{}", e);
                        return;
                    }
                    let result = commands::patch::run(&config, &server_metadata, global_server_pool_clone.clone()).await;
                    if let Err(e) = result {
                        log_error!(&server_metadata, PATCH_TASK_NAME,
                            "Patch failed for {}@{}: \n\t{}",
                            server_metadata.user,
                            server_metadata.host,
                            e
                        );
                    }
                });
                tasks.push(handle);
            }
        }
    }

    join_all(tasks).await;
    if let Err(e) = global_server_pool.cleanup_pending_servers().await {
        log::error!("Cleanup temp forder failed: \n\t{}",e);
     }
}
