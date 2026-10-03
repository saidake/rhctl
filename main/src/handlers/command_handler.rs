/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * Parse the original Cli paramters and YAML configuration file.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::collections::{HashMap, HashSet};
use std::process::exit;
use std::time::Duration;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{
    parse_script_invocation, ExecuteCmdConfig, PatchCmdConfig, ServerMetadata, UploadCmdConfig,
};
use crate::domain::constants::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_EXECUTE_MODE, DEFAULT_EXECUTE_WORK_PATH,
    DEFAULT_MAX_CHANNELS_PER_SESSION, DEFAULT_MAX_SESSION_LIFETIME,
    DEFAULT_MAX_SESSIONS_PER_SERVER, DEFAULT_SESSION_ACQUIRE_TIMEOUT, DEFAULT_SSH_PORT,
    EXECUTE_TASK_NAME, PATCH_TASK_NAME, UPLOAD_TASK_NAME,
};
use crate::domain::yml_config::{
    ExecuteStep, NamedConfig, PatchStep, RunStep, ServerConfig, UploadStep, YmlConfig,
};
use crate::utils::file_utils::{expand_vars, substitute_vars, validate_path_chars};
use crate::utils::log_utils::prompt_password_or_exit;
use crate::{log_error_direct, log_error_with_host_direct, log_warn_direct, log_warn_root};

fn resolve_script_invocation(
    raw: &str,
    vars: &HashMap<String, String>,
) -> Result<crate::domain::cmd_params::ScriptInvocation, String> {
    let expanded = expand_vars(raw, vars)?;
    let inv = parse_script_invocation(&expanded)?;
    validate_path_chars(&inv.path)?;
    Ok(inv)
}

/// Resolve SSH credentials: prefer identity/certificate; password is required only when no identity is set.
/// Password remains optional with identity (used for sudo and password fallback).
fn resolve_auth(
    password: Option<String>,
    identity_file: Option<String>,
    certificate_file: Option<String>,
    user: &str,
    host: &str,
    task_name: &str,
) -> (String, Option<String>, Option<String>) {
    if certificate_file.is_some() && identity_file.is_none() {
        log_error_with_host_direct!(
            user,
            host,
            task_name,
            "OpenSSH certificate requires an identity file (--identity / identity-file)."
        );
        exit(1);
    }

    let password = if identity_file.is_some() {
        password.unwrap_or_default()
    } else {
        password.unwrap_or_else(|| prompt_password_or_exit(user, host, task_name))
    };

    (password, identity_file, certificate_file)
}

fn build_server_metadata(
    host: &str,
    user: &str,
    ssh_port: u16,
    password: String,
    identity_file: Option<String>,
    certificate_file: Option<String>,
    connect_timeout: Duration,
    max_channels_per_session: usize,
    max_sessions_per_server: usize,
    session_acquire_timeout: Duration,
    max_session_lifetime: Duration,
) -> ServerMetadata {
    ServerMetadata {
        host: host.to_string(),
        user: user.to_string(),
        ssh_port,
        password,
        identity_file,
        certificate_file,
        server_key: ServerPool::generate_server_key(host, ssh_port, user),
        connect_timeout,
        max_channels_per_session,
        max_sessions_per_server,
        session_acquire_timeout,
        max_session_lifetime,
    }
}

// Root level
pub fn parse_patch_config_from_cmd(
    host: &str,
    user: &str,
    ssh_port: Option<u16>,
    password: Option<String>,
    identity_file: Option<String>,
    certificate_file: Option<String>,

    recover: bool,
    local_path: &str,
    remote_upload: &str,
    remote_path: &str,
    remote_backup: &str,

    use_sudo: bool,
    use_rsync: bool,
    silent: bool,
    connect_timeout: Option<Duration>,
    max_channels_per_session: Option<usize>,
    max_sessions_per_server: Option<usize>,
    session_acquire_timeout: Option<Duration>,
    max_session_lifetime: Option<Duration>,
    vars: &HashMap<String, String>,
) -> PatchCmdConfig {
    let ssh_port = ssh_port.unwrap_or(DEFAULT_SSH_PORT);
    let (password, identity_file, certificate_file) = resolve_auth(
        password,
        identity_file,
        certificate_file,
        user,
        host,
        PATCH_TASK_NAME,
    );

    PatchCmdConfig {
        server_metadata: build_server_metadata(
            host,
            user,
            ssh_port,
            password,
            identity_file,
            certificate_file,
            connect_timeout.unwrap_or(DEFAULT_CONNECT_TIMEOUT),
            max_channels_per_session.unwrap_or(DEFAULT_MAX_CHANNELS_PER_SESSION),
            max_sessions_per_server.unwrap_or(DEFAULT_MAX_SESSIONS_PER_SERVER),
            session_acquire_timeout.unwrap_or(DEFAULT_SESSION_ACQUIRE_TIMEOUT),
            max_session_lifetime.unwrap_or(DEFAULT_MAX_SESSION_LIFETIME),
        ),
        use_sudo,
        use_rsync,
        silent,
        recover,
        local_path: substitute_vars(&local_path, &vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_upload: substitute_vars(&remote_upload, &vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_path: substitute_vars(&remote_path, &vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_backup: substitute_vars(&remote_backup, &vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
    }
}

// Root level
pub fn parse_execute_config_from_cmd(
    host: &str,
    user: &str,
    ssh_port: Option<u16>,
    password: Option<String>,
    identity_file: Option<String>,
    certificate_file: Option<String>,
    script: Vec<String>,
    work_path: Option<String>,
    mode: Option<String>,

    use_sudo: bool,
    use_rsync: bool,
    silent: bool,
    connect_timeout: Option<Duration>,
    max_channels_per_session: Option<usize>,
    max_sessions_per_server: Option<usize>,
    session_acquire_timeout: Option<Duration>,
    max_session_lifetime: Option<Duration>,
    vars: &HashMap<String, String>,
) -> ExecuteCmdConfig {
    let ssh_port = ssh_port.unwrap_or(DEFAULT_SSH_PORT);
    let (password, identity_file, certificate_file) = resolve_auth(
        password,
        identity_file,
        certificate_file,
        user,
        host,
        EXECUTE_TASK_NAME,
    );
    ExecuteCmdConfig {
        server_metadata: build_server_metadata(
            host,
            user,
            ssh_port,
            password,
            identity_file,
            certificate_file,
            connect_timeout.unwrap_or(DEFAULT_CONNECT_TIMEOUT),
            max_channels_per_session.unwrap_or(DEFAULT_MAX_CHANNELS_PER_SESSION),
            max_sessions_per_server.unwrap_or(DEFAULT_MAX_SESSIONS_PER_SERVER),
            session_acquire_timeout.unwrap_or(DEFAULT_SESSION_ACQUIRE_TIMEOUT),
            max_session_lifetime.unwrap_or(DEFAULT_MAX_SESSION_LIFETIME),
        ),
        use_sudo,
        use_rsync,
        silent,
        scripts: script
            .into_iter()
            .map(|s| {
                resolve_script_invocation(&s, &vars).unwrap_or_else(|e| {
                    log_error_with_host_direct!(user, host, EXECUTE_TASK_NAME, "{}", e);
                    exit(1);
                })
            })
            .collect(),
        mode: mode.unwrap_or(DEFAULT_EXECUTE_MODE.to_string()),
        work_path: substitute_vars(
            &work_path.unwrap_or_else(|| DEFAULT_EXECUTE_WORK_PATH.to_string()),
            &vars,
        )
        .unwrap_or_else(|e| {
            log_error_with_host_direct!(user, host, EXECUTE_TASK_NAME, "{}", e);
            exit(1);
        }),
    }
}

// Root level
pub fn parse_upload_config_from_cmd(
    host: &str,
    user: &str,
    ssh_port: Option<u16>,
    password: Option<String>,
    identity_file: Option<String>,
    certificate_file: Option<String>,
    transfer_file: Option<String>,
    transfers: Vec<String>,

    use_sudo: bool,
    use_rsync: bool,
    silent: bool,
    connect_timeout: Option<Duration>,
    max_channels_per_session: Option<usize>,
    max_sessions_per_server: Option<usize>,
    session_acquire_timeout: Option<Duration>,
    max_session_lifetime: Option<Duration>,
    vars: &HashMap<String, String>,
) -> UploadCmdConfig {
    let ssh_port = ssh_port.unwrap_or(DEFAULT_SSH_PORT);
    let (password, identity_file, certificate_file) = resolve_auth(
        password,
        identity_file,
        certificate_file,
        user,
        host,
        UPLOAD_TASK_NAME,
    );
    let transfer_file = transfer_file.map(|f| {
        substitute_vars(&f, vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(user, host, UPLOAD_TASK_NAME, "{}", e);
            exit(1);
        })
    });
    UploadCmdConfig {
        server_metadata: build_server_metadata(
            host,
            user,
            ssh_port,
            password,
            identity_file,
            certificate_file,
            connect_timeout.unwrap_or(DEFAULT_CONNECT_TIMEOUT),
            max_channels_per_session.unwrap_or(DEFAULT_MAX_CHANNELS_PER_SESSION),
            max_sessions_per_server.unwrap_or(DEFAULT_MAX_SESSIONS_PER_SERVER),
            session_acquire_timeout.unwrap_or(DEFAULT_SESSION_ACQUIRE_TIMEOUT),
            max_session_lifetime.unwrap_or(DEFAULT_MAX_SESSION_LIFETIME),
        ),
        use_sudo,
        use_rsync,
        silent,
        transfer_file,
        transfers,
    }
}

fn server_metadata_from_yml(
    server: &ServerConfig,
    common: &Option<crate::domain::yml_config::CommonConfig>,
    task_name: &str,
) -> ServerMetadata {
    let ssh_port = server.ssh_port.unwrap_or(DEFAULT_SSH_PORT);
    let (password, identity_file, certificate_file) = resolve_auth(
        server.password.clone(),
        server.identity_file.clone(),
        server.certificate_file.clone(),
        &server.user,
        &server.host,
        task_name,
    );
    build_server_metadata(
        &server.host,
        &server.user,
        ssh_port,
        password,
        identity_file,
        certificate_file,
        server
            .connect_timeout
            .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT),
        server
            .max_channels_per_session
            .or_else(|| common.as_ref()?.server.as_ref()?.max_channels_per_session)
            .unwrap_or(DEFAULT_MAX_CHANNELS_PER_SESSION),
        server
            .max_sessions_per_server
            .or_else(|| common.as_ref()?.server.as_ref()?.max_sessions_per_server)
            .unwrap_or(DEFAULT_MAX_SESSIONS_PER_SERVER),
        server
            .session_acquire_timeout
            .or_else(|| common.as_ref()?.server.as_ref()?.session_acquire_timeout)
            .unwrap_or(DEFAULT_SESSION_ACQUIRE_TIMEOUT),
        server
            .max_session_lifetime
            .or_else(|| common.as_ref()?.server.as_ref()?.max_session_lifetime)
            .unwrap_or(DEFAULT_MAX_SESSION_LIFETIME),
    )
}

/// One ordered pipeline step expanded to per-server command configs.
pub enum ParsedRunStep {
    Upload(Vec<(UploadCmdConfig, HashMap<String, String>)>),
    Execute(Vec<(ExecuteCmdConfig, HashMap<String, String>)>),
    Patch(Vec<(PatchCmdConfig, HashMap<String, String>)>),
}

fn merge_var_maps(
    global: &HashMap<String, String>,
    config: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = global.clone();
    for (k, v) in config {
        merged.insert(k.clone(), v.clone());
    }
    merged
}

fn effective_targets<'a>(
    named_config: &'a NamedConfig,
    step_servers: &'a [String],
    step_groups: &'a [String],
) -> (&'a [String], &'a [String]) {
    let servers = if step_servers.is_empty() {
        named_config.target_servers.as_slice()
    } else {
        step_servers
    };
    let groups = if step_groups.is_empty() {
        named_config.target_groups.as_slice()
    } else {
        step_groups
    };
    (servers, groups)
}

/// Parse `runs:` into ordered steps. Servers within a step run in parallel; steps stay sequential.
pub fn parse_run_steps(
    named_config: &NamedConfig,
    yml_config: &YmlConfig,
    failed_servers: &HashSet<String>,
    server_config_map: &HashMap<String, ServerConfig>,
) -> Vec<ParsedRunStep> {
    if named_config.runs.is_empty() {
        log_error_direct!(
            "Config '{}' has no runs. Add ordered upload/execute/patch steps under `runs:`.",
            named_config.name
        );
        exit(1);
    }

    let var_map = merge_var_maps(&yml_config.var_map, &named_config.var_map);
    let common = &yml_config.common;
    let mut steps = Vec::with_capacity(named_config.runs.len());

    for (idx, run) in named_config.runs.iter().enumerate() {
        let step_no = idx + 1;
        match run {
            RunStep::Upload(upload) => {
                let servers = collect_servers_for_step(
                    server_config_map,
                    yml_config,
                    failed_servers,
                    named_config,
                    &upload.target_servers,
                    &upload.target_groups,
                    UPLOAD_TASK_NAME,
                    step_no,
                );
                steps.push(ParsedRunStep::Upload(build_upload_step_configs(
                    named_config,
                    upload,
                    &servers,
                    common,
                    &var_map,
                )));
            }
            RunStep::Execute(execute) => {
                let servers = collect_servers_for_step(
                    server_config_map,
                    yml_config,
                    failed_servers,
                    named_config,
                    &execute.target_servers,
                    &execute.target_groups,
                    EXECUTE_TASK_NAME,
                    step_no,
                );
                steps.push(ParsedRunStep::Execute(build_execute_step_configs(
                    named_config,
                    execute,
                    &servers,
                    common,
                    &var_map,
                )));
            }
            RunStep::Patch(patch) => {
                let servers = collect_servers_for_step(
                    server_config_map,
                    yml_config,
                    failed_servers,
                    named_config,
                    &patch.target_servers,
                    &patch.target_groups,
                    PATCH_TASK_NAME,
                    step_no,
                );
                steps.push(ParsedRunStep::Patch(build_patch_step_configs(
                    named_config,
                    patch,
                    &servers,
                    common,
                    &var_map,
                )));
            }
        }
    }

    steps
}

fn build_upload_step_configs(
    named_config: &NamedConfig,
    upload: &UploadStep,
    servers: &[ServerConfig],
    common: &Option<crate::domain::yml_config::CommonConfig>,
    var_map: &HashMap<String, String>,
) -> Vec<(UploadCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    for server in servers {
        if upload.transfer_file.is_none() && upload.transfers.is_empty() {
            log_error_with_host_direct!(
                &server.user,
                &server.host,
                UPLOAD_TASK_NAME,
                "Upload step requires transfer-file and/or transfers"
            );
            exit(1);
        }
        let transfer_file = upload.transfer_file.as_ref().map(|f| {
            substitute_vars(f, var_map).unwrap_or_else(|e| {
                log_error_with_host_direct!(&server.user, &server.host, UPLOAD_TASK_NAME, "{}", e);
                exit(1);
            })
        });
        configs.push((
            UploadCmdConfig {
                server_metadata: server_metadata_from_yml(server, common, UPLOAD_TASK_NAME),
                use_sudo: upload.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                use_rsync: upload.use_rsync.or(named_config.use_rsync).unwrap_or(false),
                silent: upload.silent.or(named_config.silent).unwrap_or(false),
                transfer_file,
                transfers: upload.transfers.clone(),
            },
            var_map.clone(),
        ));
    }
    configs
}

fn build_execute_step_configs(
    named_config: &NamedConfig,
    execute: &ExecuteStep,
    servers: &[ServerConfig],
    common: &Option<crate::domain::yml_config::CommonConfig>,
    var_map: &HashMap<String, String>,
) -> Vec<(ExecuteCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    for server in servers {
        configs.push((
            ExecuteCmdConfig {
                server_metadata: server_metadata_from_yml(server, common, EXECUTE_TASK_NAME),
                use_sudo: execute.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                use_rsync: execute
                    .use_rsync
                    .or(named_config.use_rsync)
                    .unwrap_or(false),
                silent: execute.silent.or(named_config.silent).unwrap_or(false),
                scripts: execute
                    .scripts
                    .clone()
                    .into_iter()
                    .map(|s| {
                        resolve_script_invocation(&s, var_map).unwrap_or_else(|e| {
                            log_error_with_host_direct!(
                                &server.user,
                                &server.host,
                                EXECUTE_TASK_NAME,
                                "{}",
                                e
                            );
                            exit(1);
                        })
                    })
                    .collect(),
                mode: execute
                    .mode
                    .clone()
                    .unwrap_or(DEFAULT_EXECUTE_MODE.to_string()),
                work_path: substitute_vars(
                    &execute
                        .work_path
                        .clone()
                        .unwrap_or_else(|| DEFAULT_EXECUTE_WORK_PATH.to_string()),
                    var_map,
                )
                .unwrap_or_else(|e| {
                    log_error_with_host_direct!(
                        &server.user,
                        &server.host,
                        EXECUTE_TASK_NAME,
                        "{}",
                        e
                    );
                    exit(1);
                }),
            },
            var_map.clone(),
        ));
    }
    configs
}

fn build_patch_step_configs(
    named_config: &NamedConfig,
    patch: &PatchStep,
    servers: &[ServerConfig],
    common: &Option<crate::domain::yml_config::CommonConfig>,
    var_map: &HashMap<String, String>,
) -> Vec<(PatchCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    for server in servers {
        configs.push((
            PatchCmdConfig {
                server_metadata: server_metadata_from_yml(server, common, PATCH_TASK_NAME),
                use_sudo: patch.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                use_rsync: patch.use_rsync.or(named_config.use_rsync).unwrap_or(false),
                silent: patch.silent.or(named_config.silent).unwrap_or(false),
                recover: patch.recover,
                local_path: substitute_vars(&patch.local_path, var_map).unwrap_or_else(|e| {
                    log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                    exit(1);
                }),
                remote_upload: substitute_vars(&patch.remote_upload, var_map).unwrap_or_else(|e| {
                    log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                    exit(1);
                }),
                remote_path: substitute_vars(&patch.remote_path, var_map).unwrap_or_else(|e| {
                    log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                    exit(1);
                }),
                remote_backup: substitute_vars(&patch.remote_backup, var_map).unwrap_or_else(|e| {
                    log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                    exit(1);
                }),
            },
            var_map.clone(),
        ));
    }
    configs
}

fn collect_servers_for_step(
    server_map: &HashMap<String, ServerConfig>,
    yml_config: &YmlConfig,
    failed_servers: &HashSet<String>,
    named_config: &NamedConfig,
    step_servers: &[String],
    step_groups: &[String],
    task_name: &str,
    step_no: usize,
) -> Vec<ServerConfig> {
    let (target_servers, target_groups) =
        effective_targets(named_config, step_servers, step_groups);

    if target_servers.is_empty() && target_groups.is_empty() {
        log_error_direct!(
            "Config '{}' step {} ({}) has no target-servers or target-groups",
            named_config.name,
            step_no,
            task_name
        );
        exit(1);
    }

    let mut servers: HashSet<ServerConfig> = HashSet::new();

    for server_name in target_servers {
        if failed_servers.contains(server_name) {
            log_warn_root!(
                "Skip failed server '{}' for {} step {} in config '{}'",
                server_name,
                task_name,
                step_no,
                named_config.name
            );
            continue;
        }
        match server_map.get(server_name) {
            Some(server) => {
                servers.insert(server.clone());
            }
            None => {
                log_error_direct!("Server '{}' not found in servers list", server_name);
                exit(1);
            }
        }
    }

    if let Some(group_map) = &yml_config.group_map {
        for group_name in target_groups {
            match group_map.get(group_name) {
                Some(group_servers) => {
                    for server_name in group_servers {
                        if failed_servers.contains(server_name) {
                            log_warn_direct!(
                                "Skip failed server '{}' for {} step {} in config '{}'",
                                server_name,
                                task_name,
                                step_no,
                                named_config.name
                            );
                            continue;
                        }
                        match server_map.get(server_name) {
                            Some(server) => {
                                servers.insert(server.clone());
                            }
                            None => {
                                log_error_direct!(
                                    "Server '{}' in group '{}' not found in servers list",
                                    server_name,
                                    group_name
                                );
                                exit(1);
                            }
                        }
                    }
                }
                None => {
                    log_error_direct!("Group '{}' not found in group_map list", group_name);
                    exit(1);
                }
            }
        }
    }

    if servers.is_empty() {
        log_error_direct!(
            "Config '{}' step {} ({}) resolved to no reachable servers",
            named_config.name,
            step_no,
            task_name
        );
        exit(1);
    }

    servers.into_iter().collect()
}
