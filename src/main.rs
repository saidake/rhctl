use ansi_term::Colour;
use clap::{Parser, Subcommand};
use log::{Level, error};
use std::collections::HashMap;
use std::io::Write;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;

mod commands;
mod common;
mod domain;
mod handlers;
mod utils;

use crate::handlers::command_handler::{
    parse_execute_configs, parse_patch_configs, parse_upload_configs,
};
use crate::handlers::validation_handler::validate_cli_args;
use crate::utils::file_utils::substitute_vars;
use crate::utils::ssh_utils::connect_ssh;
use crate::{
    domain::cmd_params::{ExecuteCmdConfig, PatchCmdConfig, UploadCmdConfig},
    utils::file_utils::load_yaml_config,
    utils::log_utils::prompt_password_or_exit,
};

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

fn main() {
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
    validate_cli_args(&cli);

    // Load YAML config if provided
    let yaml_config = cli
        .config
        .as_ref()
        .map(|path| load_yaml_config(path))
        .transpose()
        .unwrap_or_else(|err| {
            error!("{}", err);
            exit(1);
        });

    // Thread handles for parallel execution
    let threads = Arc::new(Mutex::new(Vec::new()));

    if let Some(config_name) = &cli.config_name {
        // YAML config mode
        let yml_config = yaml_config.as_ref().unwrap_or_else(|| {
            error!("YAML config required when --config-name is provided");
            exit(1);
        });

        let named_config = yml_config
            .configs
            .as_ref()
            .and_then(|configs| configs.iter().find(|c| c.name == *config_name))
            .unwrap_or_else(|| {
                error!("Config '{}' not found in YAML file", config_name);
                exit(1);
            });

        // Parse all command configs with vars
        let upload_configs = parse_upload_configs(&named_config, yml_config, &cli_vars);
        let execute_configs = parse_execute_configs(&named_config, yml_config, &cli_vars);
        let patch_configs = parse_patch_configs(&named_config, yml_config, &cli_vars);

        // Spawn threads for upload commands
        for (config, vars) in upload_configs {
            let handle = thread::spawn(move || {
                let session = connect_ssh(
                    config.host.clone(),
                    config.user.clone(),
                    config.ssh_port,
                    config.password.clone(),
                );
                if let Err(e) = session.check_global_remote_temp_dir(config.use_sudo, config.silent)
                {
                    error!("{}", e);
                    exit(1);
                }
                let result = commands::upload::run(&config, &session, &vars);
                if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                    error!("{}", e);
                }
                if let Err(e) = result {
                    error!(
                        "Upload failed for {}@{}: \n\t{}",
                        config.user, config.host, e
                    );
                    exit(1);
                }
            });
            threads.lock().unwrap().push(handle);
        }

        // Spawn threads for execute commands
        for (config, vars) in execute_configs {
            let handle = thread::spawn(move || {
                let session = connect_ssh(
                    config.host.clone(),
                    config.user.clone(),
                    config.ssh_port,
                    config.password.clone(),
                );
                if let Err(e) = session.check_global_remote_temp_dir(config.use_sudo, config.silent)
                {
                    error!("{}", e);
                    exit(1);
                }
                let result = commands::execute::run(&config, &session);
                if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                    error!("{}", e);
                }
                if let Err(e) = result {
                    error!(
                        "Execute failed for {}@{}: \n\t{}",
                        config.user, config.host, e
                    );
                    exit(1);
                }
            });
            threads.lock().unwrap().push(handle);
        }

        // Spawn threads for patch commands
        for (config, vars) in patch_configs {
            let handle = thread::spawn(move || {
                let session = connect_ssh(
                    config.host.clone(),
                    config.user.clone(),
                    config.ssh_port,
                    config.password.clone(),
                );
                if let Err(e) = session.check_global_remote_temp_dir(config.use_sudo, config.silent)
                {
                    error!("{}", e);
                    exit(1);
                }
                let result = commands::patch::run(&config, &session);
                if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                    error!("{}", e);
                }
                if let Err(e) = result {
                    error!(
                        "Patch failed for {}@{}: \n\t{}",
                        config.user, config.host, e
                    );
                    exit(1);
                }
            });
            threads.lock().unwrap().push(handle);
        }
    } else if let Some(command) = cli.command {
        // CLI mode
        match command {
            Commands::Upload {
                host,
                ssh_port,
                user,
                password,
                use_sudo,
                use_rsync,
                silent,
                properties_file,
                ..
            } => {
                let config = UploadCmdConfig {
                    host,
                    user,
                    ssh_port: ssh_port.unwrap_or(22),
                    password: password.unwrap_or_else(|| prompt_password_or_exit()),
                    use_sudo,
                    use_rsync,
                    silent,
                    properties_file: substitute_vars(&properties_file, &cli_vars).unwrap_or_else(
                        |e| {
                            error!("{}", e);
                            exit(1);
                        },
                    ),
                };
                let vars = cli_vars.clone();
                let handle = thread::spawn(move || {
                    let session = connect_ssh(
                        config.host.clone(),
                        config.user.clone(),
                        config.ssh_port,
                        config.password.clone(),
                    );
                    if let Err(e) =
                        session.check_global_remote_temp_dir(config.use_sudo, config.silent)
                    {
                        error!("{}", e);
                        exit(1);
                    }
                    let result = commands::upload::run(&config, &session, &vars);
                    if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                        error!("{}", e);
                    }
                    if let Err(e) = result {
                        error!(
                            "Upload failed for {}@{}: \n\t{}",
                            config.user, config.host, e
                        );
                        exit(1);
                    }
                });
                threads.lock().unwrap().push(handle);
            }
            Commands::Execute {
                host,
                ssh_port,
                user,
                password,
                use_sudo,
                use_rsync,
                silent,
                script,
                remote_path,
                ..
            } => {
                let config = ExecuteCmdConfig {
                    host,
                    user,
                    ssh_port: ssh_port.unwrap_or(22),
                    password: password.unwrap_or_else(|| prompt_password_or_exit()),
                    use_sudo,
                    use_rsync,
                    silent,
                    script: substitute_vars(&script, &cli_vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                    remote_path: substitute_vars(
                        &remote_path.unwrap_or_else(|| "~".to_string()),
                        &cli_vars,
                    )
                    .unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                };
                let vars = cli_vars.clone();
                let handle = thread::spawn(move || {
                    let session = connect_ssh(
                        config.host.clone(),
                        config.user.clone(),
                        config.ssh_port,
                        config.password.clone(),
                    );
                    if let Err(e) =
                        session.check_global_remote_temp_dir(config.use_sudo, config.silent)
                    {
                        error!("{}", e);
                        exit(1);
                    }
                    let result = commands::execute::run(&config, &session);
                    if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                        error!("{}", e);
                    }
                    if let Err(e) = result {
                        error!(
                            "Execute failed for {}@{}: \n\t{}",
                            config.user, config.host, e
                        );
                        exit(1);
                    }
                });
                threads.lock().unwrap().push(handle);
            }
            Commands::Patch {
                host,
                ssh_port,
                user,
                password,
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
                let config = PatchCmdConfig {
                    host,
                    user,
                    ssh_port: ssh_port.unwrap_or(22),
                    password: password.unwrap_or_else(|| prompt_password_or_exit()),
                    use_sudo,
                    use_rsync,
                    silent,
                    recover,
                    local_path: substitute_vars(&local_path, &cli_vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                    remote_upload: substitute_vars(&remote_upload, &cli_vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                    remote_path: substitute_vars(&remote_path, &cli_vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                    remote_backup: substitute_vars(&remote_backup, &cli_vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                };
                let vars = cli_vars.clone();
                let handle = thread::spawn(move || {
                    let session = connect_ssh(
                        config.host.clone(),
                        config.user.clone(),
                        config.ssh_port,
                        config.password.clone(),
                    );
                    if let Err(e) =
                        session.check_global_remote_temp_dir(config.use_sudo, config.silent)
                    {
                        error!("{}", e);
                        exit(1);
                    }
                    let result = commands::patch::run(&config, &session);
                    if let Err(e) = session.delete_global_temp_dir(config.use_sudo) {
                        error!("{}", e);
                    }
                    if let Err(e) = result {
                        error!(
                            "Patch failed for {}@{}: \n\t{}",
                            config.user, config.host, e
                        );
                        exit(1);
                    }
                });
                threads.lock().unwrap().push(handle);
            }
        }
    }

    // Collect thread results
    let mut errors = Vec::new();
    for handle in threads.lock().unwrap().drain(..) {
        if let Err(e) = handle.join() {
            errors.push(format!("Thread failed: {:?}", e));
        }
    }
    if !errors.is_empty() {
        error!("Errors occurred during execution:\n{}", errors.join("\n"));
        exit(1);
    }
}
