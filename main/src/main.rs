/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * Main entry point of the application.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use clap::{Parser, Subcommand};
use futures::future::join_all;
use std::collections::HashMap;
use std::io::Write;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

mod commands;
mod common;
mod domain;
mod handlers;
mod utils;

use crate::common::ssh_pool::ServerPool;
use crate::domain::constants::{
    DEFAULT_EXECUTE_MODE, DEFAULT_EXECUTE_WORK_PATH, DEFAULT_SSH_PORT, EXECUTE_TASK_NAME,
    PATCH_TASK_NAME, UPLOAD_TASK_NAME,
};
use crate::domain::yml_config::ServerConfig;
use crate::handlers::command_handler::{
    parse_execute_config_from_cmd, parse_patch_config_from_cmd, parse_run_steps,
    parse_upload_config_from_cmd, ParsedRunStep,
};
use crate::utils::file_utils::{load_yaml_config, resolve_upload_mappings};
use crate::utils::log_utils::{ask_user_and_abort_option, flush_logs_and_exit, init_logger};

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| format!("Invalid duration '{}': {}", s, e))
}

#[derive(Parser)]
#[command(name = "rhctl")]
#[command(about = "A high-performance Rust CLI for remote file operations via SSH")]
#[command(override_usage = "rhctl [COMMAND] [OPTIONS]")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(
        long,
        global = true,
        help = "Enable debug logging (default: info)"
    )]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Upload files or directories using transfer mappings")]
    Upload {
        #[arg(long, help = "Remote host IP or hostname")]
        host: String,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

        #[arg(long, help = "Remote username")]
        user: String,

        #[arg(long, help = "Remote password (optional when --identity is set; also used for sudo)")]
        password: Option<String>,

        #[arg(
            long,
            help = "Path to SSH private key (identity file). Preferred over password when set."
        )]
        identity: Option<String>,

        #[arg(
            long,
            help = "Path to OpenSSH certificate (requires --identity)"
        )]
        certificate: Option<String>,

        #[arg(
            long = "transfer",
            help = "Inline transfer mapping local=remote-dir (repeatable). Overrides the same local path from --transfer-file."
        )]
        transfer: Vec<String>,

        #[arg(
            long = "transfer-file",
            help = "Transfer file with local=remote-dir lines (same format as --transfer)"
        )]
        transfer_file: Option<String>,

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
        #[arg(help = "Maximum time allowed to establish a connection to the remote server.")]
        connect_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        #[arg(help = "Maximum number of active SSH sessions allowed per server.")]
        max_sessions_per_server: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        #[arg(help = "Maximum number of concurrent channels allowed per SSH session.")]
        max_channels_per_session: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        #[arg(help = "Maximum time to wait for acquiring a session from the session pool.")]
        session_acquire_timeout: Option<Duration>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        #[arg(help = "Maximum lifetime of an SSH session before it is automatically closed.")]
        max_session_lifetime: Option<Duration>,
    },

    #[command(about = "Execute local bash scripts and/or remote commands via SSH")]
    Execute {
        #[arg(long, help = "Remote host IP or hostname")]
        host: String,

        #[arg(long, help = "Remote SSH port")]
        ssh_port: Option<u16>,

        #[arg(long, help = "Remote username")]
        user: String,

        #[arg(long, help = "Remote password (optional when --identity is set; also used for sudo)")]
        password: Option<String>,

        #[arg(
            long,
            help = "Path to SSH private key (identity file). Preferred over password when set."
        )]
        identity: Option<String>,

        #[arg(
            long,
            help = "Path to OpenSSH certificate (requires --identity)"
        )]
        certificate: Option<String>,

        #[arg(
            long,
            help = "Local bash script to run remotely; optional args after the path (e.g. 'configure.sh --port 5432'). Supports multiple."
        )]
        script: Vec<String>,

        #[arg(
            long,
            help = "Remote shell command to run (no local upload). Supports multiple. Example: --cmd \"systemctl status nats\""
        )]
        cmd: Vec<String>,

        #[arg(
            long,
            requires = "env_name",
            help = "Regex with one capture group; last match on clean lines (no \\r) is saved. Requires --env-name."
        )]
        env_extract_regex: Option<String>,

        #[arg(
            long,
            requires = "env_extract_regex",
            help = "Env key written to remote /etc/rhctl/.env (chmod 600). Requires --env-extract-regex."
        )]
        env_name: Option<String>,

        #[arg(
            long,
            default_value = DEFAULT_EXECUTE_WORK_PATH,
            help = "Remote working directory where the bash script will be executed (defaults to the user's home directory: ~)"
        )]
        work_path: Option<String>,

        #[arg(long, default_value = DEFAULT_EXECUTE_MODE, help = "Execution mode: 'sync' (run sequentially) or 'async' (run concurrently)")]
        mode: Option<String>,

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
        max_sessions_per_server: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_channels_per_session: Option<usize>,

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

        #[arg(long, help = "Remote password (optional when --identity is set; also used for sudo)")]
        password: Option<String>,

        #[arg(
            long,
            help = "Path to SSH private key (identity file). Preferred over password when set."
        )]
        identity: Option<String>,

        #[arg(
            long,
            help = "Path to OpenSSH certificate (requires --identity)"
        )]
        certificate: Option<String>,

        #[arg(long, help = "Local source file")]
        local_path: String,

        #[arg(long, help = "Remote path to upload the local source file")]
        remote_upload: String,

        #[arg(long, help = "Remote target file to apply the patch to")]
        remote_path: String,

        #[arg(long, help = "Backup path for the remote target file before patching")]
        remote_backup: String,

        #[arg(
            long,
            default_value = "false",
            help = "Recover the remote target file from its backup after patching"
        )]
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
        max_sessions_per_server: Option<usize>,

        #[arg(long)]
        #[arg(value_parser = parse_duration)]
        max_channels_per_session: Option<usize>,

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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    // Path placeholders (${NAME}) resolve from the process environment; YAML mode
    // may also supply values via var-map (those override env).
    let env_vars: HashMap<String, String> = HashMap::new();
    let log_level = if cli.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    // Initialize logging
    env_logger::Builder::new()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .filter_level(log_level)
        .filter_module("russh", log::LevelFilter::Info)
        .filter_module("russh_keys", log::LevelFilter::Info)
        .init();

    let log_handle: JoinHandle<()> = init_logger().await;

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
            identity,
            certificate,

            transfer,
            transfer_file,

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
            if transfer.is_empty() && transfer_file.is_none() {
                log_error_with_host_direct!(
                    user.as_str(),
                    host.as_str(),
                    UPLOAD_TASK_NAME,
                    "Provide at least one of --transfer or --transfer-file"
                );
                exit(1);
            }
            let config = parse_upload_config_from_cmd(
                &host,
                &user,
                ssh_port,
                password,
                identity,
                certificate,
                transfer_file,
                transfer,
                use_sudo,
                use_rsync,
                silent,
                connect_timeout,
                max_channels_per_session,
                max_sessions_per_server,
                session_acquire_timeout,
                max_session_lifetime,
                &env_vars,
            );
            let mappings = match resolve_upload_mappings(
                config.transfer_file.as_deref(),
                &config.transfers,
                &env_vars,
            ) {
                Ok(m) => m,
                Err(e) => {
                    log_error_with_host_direct!(
                        user.as_str(),
                        host.as_str(),
                        UPLOAD_TASK_NAME,
                        "{}",
                        e
                    );
                    exit(1);
                }
            };

            log_info_direct!("Starting initial TCP connectivity check for server...");
            let (_, _, result) = ServerPool::check_single_server_by_info(
                host.clone(),
                ssh_port.unwrap_or(DEFAULT_SSH_PORT),
                None,
            )
            .await;
            match result {
                Ok(_) => {}
                Err(e) => {
                    log_error_with_host_direct!(
                        user.as_str(),
                        host.as_str(),
                        UPLOAD_TASK_NAME,
                        "{}",
                        format!("Failed to check server connection: \n\t> {}", e)
                    );
                    exit(1);
                }
            }
            log_info_direct!("Target server is healthy.");

            // println!("test---------");
            let server_metadata = Arc::new(config.server_metadata.clone());
            let global_server_pool_clone = global_server_pool.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = global_server_pool_clone
                    .check_global_remote_temp_dir(
                        &server_metadata,
                        UPLOAD_TASK_NAME,
                        config.use_sudo,
                        config.silent,
                    )
                    .await
                {
                    log_error!(&server_metadata, UPLOAD_TASK_NAME, "{}", e);
                    if let Err(e) = global_server_pool_clone.cleanup_pending_servers().await {
                        log::error!("Cleanup temp forder failed: \n\t> {}", e);
                    }
                    flush_logs_and_exit(log_handle).await;
                }
                let result = commands::upload::run(
                    &config,
                    &mappings,
                    &server_metadata,
                    global_server_pool_clone.clone(),
                )
                .await;
                if let Err(e) = result {
                    log_error!(
                        &server_metadata,
                        UPLOAD_TASK_NAME,
                        "Upload failed: \n\t> {}",
                        e
                    );
                    if let Err(e) = global_server_pool_clone.cleanup_pending_servers().await {
                        log::error!("Cleanup temp forder failed: \n\t> {}", e);
                    }
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
            identity,
            certificate,
            script,
            cmd,
            env_extract_regex,
            env_name,
            work_path,
            mode,

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
                identity,
                certificate,
                script,
                cmd,
                env_extract_regex,
                env_name,
                work_path,
                mode,
                use_sudo,
                use_rsync,
                silent,
                connect_timeout,
                max_channels_per_session,
                max_sessions_per_server,
                session_acquire_timeout,
                max_session_lifetime,
                &env_vars,
            );

            log_info_direct!("Starting initial TCP connectivity check for server...");
            let (_, _, result) = ServerPool::check_single_server_by_info(
                host.clone(),
                ssh_port.unwrap_or(DEFAULT_SSH_PORT),
                None,
            )
            .await;
            match result {
                Ok(_) => {}
                Err(e) => {
                    log_error_with_host_direct!(
                        user.as_str(),
                        host.as_str(),
                        EXECUTE_TASK_NAME,
                        "{}",
                        format!("Failed to check server connection: \n\t> {}", e)
                    );
                    exit(1);
                }
            }
            log_info_direct!("Target server is healthy.");

            let server_metadata = Arc::new(config.server_metadata.clone());
            let global_server_pool_clone = global_server_pool.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = global_server_pool_clone
                    .check_global_remote_temp_dir(
                        &server_metadata,
                        EXECUTE_TASK_NAME,
                        config.use_sudo,
                        config.silent,
                    )
                    .await
                {
                    log_error!(&server_metadata, EXECUTE_TASK_NAME, "{}", e);
                    if let Err(e) = global_server_pool_clone.cleanup_pending_servers().await {
                        log::error!("Cleanup temp forder failed: \n\t> {}", e);
                    }
                    flush_logs_and_exit(log_handle).await;
                }
                let result = commands::execute::run(
                    &config,
                    &server_metadata,
                    global_server_pool_clone.clone(),
                )
                .await;
                if let Err(e) = result {
                    log_error!(
                        &server_metadata,
                        EXECUTE_TASK_NAME,
                        "Execute failed: \n\t> {}",
                        e
                    );
                    if let Err(e) = global_server_pool_clone.cleanup_pending_servers().await {
                        log::error!("Cleanup temp forder failed: \n\t> {}", e);
                    }
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
            identity,
            certificate,

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
                identity,
                certificate,
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
                &env_vars,
            );

            log_info_direct!("Starting initial TCP connectivity check for server...");
            let (_, _, result) = ServerPool::check_single_server_by_info(
                host.clone(),
                ssh_port.unwrap_or(DEFAULT_SSH_PORT),
                None,
            )
            .await;
            match result {
                Ok(_) => {}
                Err(e) => {
                    log_error_with_host_direct!(
                        user.as_str(),
                        host.as_str(),
                        PATCH_TASK_NAME,
                        "{}",
                        format!("Failed to check server connection: \n\t> {}", e)
                    );
                    exit(1);
                }
            }
            log_info_direct!("Target server is healthy.");

            let server_metadata = Arc::new(config.server_metadata.clone());
            let global_server_pool_clone = global_server_pool.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = global_server_pool_clone
                    .check_global_remote_temp_dir(
                        &server_metadata,
                        PATCH_TASK_NAME,
                        config.use_sudo,
                        config.silent,
                    )
                    .await
                {
                    log_error!(&server_metadata, PATCH_TASK_NAME, "{}", e);
                    if let Err(e) = global_server_pool_clone.cleanup_pending_servers().await {
                        log::error!("Cleanup temp forder failed: \n\t> {}", e);
                    }
                    flush_logs_and_exit(log_handle).await;
                }
                let result = commands::patch::run(
                    &config,
                    &server_metadata,
                    global_server_pool_clone.clone(),
                )
                .await;
                if let Err(e) = result {
                    log_error!(
                        &server_metadata,
                        PATCH_TASK_NAME,
                        "Patch failed: \n\t> {}",
                        e
                    );
                    if let Err(e) = global_server_pool_clone.cleanup_pending_servers().await {
                        log::error!("Cleanup temp forder failed: \n\t> {}", e);
                    }
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

            log_info_direct!("Starting initial TCP connectivity check for servers...");
            let failed_servers = global_server_pool
                .check_servers_and_update_known_hosts(yml_config.servers.clone())
                .await;

            if yml_config.servers.len() == failed_servers.len() {
                log_error_root!("All servers failed or no valid servers found.");
                flush_logs_and_exit(log_handle).await;
            } else if !failed_servers.is_empty() {
                ask_user_and_abort_option(
                    None,
                    None,
                    "There are some servers with failed connection. Continue with remaining servers?",
                    false,
                )
                .await;
                // yml_config.servers.retain(|s| !failed_servers.iter().any(|f| f.host == s.host && f.ssh_port == s.ssh_port));
            } else if failed_servers.is_empty() {
                log_info_direct!("All servers are healthy.");
            }
            let server_config_map: std::collections::HashMap<String, ServerConfig> = yml_config
                .servers
                .iter()
                .map(|s| (s.name.clone(), s.clone()))
                .collect();

            let run_steps = parse_run_steps(
                named_config,
                &yml_config,
                &failed_servers,
                &server_config_map,
            );

            let total_steps = run_steps.len();
            for (idx, step) in run_steps.into_iter().enumerate() {
                let step_no = idx + 1;
                let step_kind = match &step {
                    ParsedRunStep::Upload(_) => "upload",
                    ParsedRunStep::Execute(_) => "execute",
                    ParsedRunStep::Patch(_) => "patch",
                };
                log_info_direct!(
                    "Running step {}/{} ({}) ...",
                    step_no,
                    total_steps,
                    step_kind
                );

                let mut step_tasks: Vec<JoinHandle<Result<(), String>>> = Vec::new();

                match step {
                    ParsedRunStep::Upload(configs) => {
                        for (config, vars) in configs {
                            let server_metadata = Arc::new(config.server_metadata.clone());
                            let mappings = match resolve_upload_mappings(
                                config.transfer_file.as_deref(),
                                &config.transfers,
                                &vars,
                            ) {
                                Ok(m) => m,
                                Err(e) => {
                                    log_error_root!("{}", e);
                                    flush_logs_and_exit(log_handle).await;
                                }
                            };
                            let global_server_pool_clone = global_server_pool.clone();
                            let handle = tokio::spawn(async move {
                                if let Err(e) = global_server_pool_clone
                                    .check_global_remote_temp_dir(
                                        &server_metadata,
                                        UPLOAD_TASK_NAME,
                                        config.use_sudo,
                                        config.silent,
                                    )
                                    .await
                                {
                                    log_error!(&server_metadata, UPLOAD_TASK_NAME, "{}", e);
                                    return Err(e);
                                }
                                match commands::upload::run(
                                    &config,
                                    &mappings,
                                    &server_metadata,
                                    global_server_pool_clone.clone(),
                                )
                                .await
                                {
                                    Ok(()) => Ok(()),
                                    Err(e) => {
                                        log_error!(
                                            &server_metadata,
                                            UPLOAD_TASK_NAME,
                                            "Upload failed: \n\t> {}",
                                            e
                                        );
                                        Err(e)
                                    }
                                }
                            });
                            step_tasks.push(handle);
                        }
                    }
                    ParsedRunStep::Execute(configs) => {
                        for (config, _) in configs {
                            let server_metadata = Arc::new(config.server_metadata.clone());
                            let global_server_pool_clone = global_server_pool.clone();
                            let handle = tokio::spawn(async move {
                                if let Err(e) = global_server_pool_clone
                                    .check_global_remote_temp_dir(
                                        &server_metadata,
                                        EXECUTE_TASK_NAME,
                                        config.use_sudo,
                                        config.silent,
                                    )
                                    .await
                                {
                                    log_error!(&server_metadata, EXECUTE_TASK_NAME, "{}", e);
                                    return Err(e);
                                }
                                match commands::execute::run(
                                    &config,
                                    &server_metadata,
                                    global_server_pool_clone.clone(),
                                )
                                .await
                                {
                                    Ok(()) => Ok(()),
                                    Err(e) => {
                                        log_error!(
                                            &server_metadata,
                                            EXECUTE_TASK_NAME,
                                            "Execute failed: \n\t> {}",
                                            e
                                        );
                                        Err(e)
                                    }
                                }
                            });
                            step_tasks.push(handle);
                        }
                    }
                    ParsedRunStep::Patch(configs) => {
                        for (config, _) in configs {
                            let server_metadata = Arc::new(config.server_metadata.clone());
                            let global_server_pool_clone = global_server_pool.clone();
                            let handle = tokio::spawn(async move {
                                if let Err(e) = global_server_pool_clone
                                    .check_global_remote_temp_dir(
                                        &server_metadata,
                                        PATCH_TASK_NAME,
                                        config.use_sudo,
                                        config.silent,
                                    )
                                    .await
                                {
                                    log_error!(&server_metadata, PATCH_TASK_NAME, "{}", e);
                                    return Err(e);
                                }
                                match commands::patch::run(
                                    &config,
                                    &server_metadata,
                                    global_server_pool_clone.clone(),
                                )
                                .await
                                {
                                    Ok(()) => Ok(()),
                                    Err(e) => {
                                        log_error!(
                                            &server_metadata,
                                            PATCH_TASK_NAME,
                                            "Patch failed: \n\t> {}",
                                            e
                                        );
                                        Err(e)
                                    }
                                }
                            });
                            step_tasks.push(handle);
                        }
                    }
                }

                let results = join_all(step_tasks).await;
                let mut step_failed = false;
                for result in results {
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(_)) => step_failed = true,
                        Err(e) => {
                            log_error_root!("Step {} task join error: {}", step_no, e);
                            step_failed = true;
                        }
                    }
                }
                if step_failed {
                    log_error_root!(
                        "Step {}/{} ({}) failed — aborting remaining steps.",
                        step_no,
                        total_steps,
                        step_kind
                    );
                    flush_logs_and_exit(log_handle).await;
                }
                log_info_direct!(
                    "Step {}/{} ({}) completed.",
                    step_no,
                    total_steps,
                    step_kind
                );
            }
        }
    }

    join_all(tasks).await;
    if let Err(e) = global_server_pool.cleanup_pending_servers().await {
        log::error!("Cleanup temp forder failed: \n\t> {}", e);
    }
}
