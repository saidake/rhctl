use std::collections::HashMap;
use std::process::exit;
use std::time::Duration;

use crate::common::ssh_pool::ServerPool;
use crate::domain::cmd_params::{
    ExecuteCmdConfig, PatchCmdConfig, ServerMetadata, UploadCmdConfig,
};
use crate::domain::constants::{EXECUTE_TASK_NAME, PATCH_TASK_NAME, UPLOAD_TASK_NAME};
use crate::domain::yml_config::{NamedConfig, ServerConfig, TargetConfig, YmlConfig};
use crate::utils::file_utils::substitute_vars;
use crate::utils::log_utils::prompt_password_or_exit;
use crate::{log_error_with_host_direct, log_error_root};

// Root level
pub fn parse_patch_config_from_cmd(
    host: &str,
    user: &str,
    ssh_port: Option<u16>,
    password: Option<String>,

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
    cli_vars: &HashMap<String, String>,
) -> PatchCmdConfig {
    let server_key = ServerPool::generate_server_key(&host, ssh_port.unwrap_or(22), &user);
    let password =
        password.unwrap_or_else(|| prompt_password_or_exit(&user, &host, PATCH_TASK_NAME));

    PatchCmdConfig {
        server_metadata: ServerMetadata {
            host: host.to_string(),
            user: user.to_string(),
            ssh_port: ssh_port.unwrap_or(22),
            password,
            server_key,

            connect_timeout: connect_timeout.unwrap_or(Duration::from_secs(60)),
            max_channels_per_session: max_channels_per_session.unwrap_or(200),
            max_sessions_per_server: max_sessions_per_server.unwrap_or(200),
            session_acquire_timeout: session_acquire_timeout.unwrap_or(Duration::from_secs(30)),
            max_session_lifetime: max_session_lifetime.unwrap_or(Duration::from_secs(600)),
        },
        use_sudo,
        use_rsync,
        silent,
        recover,
        local_path: substitute_vars(&local_path, &cli_vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_upload: substitute_vars(&remote_upload, &cli_vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_path: substitute_vars(&remote_path, &cli_vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(&user, &host, PATCH_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_backup: substitute_vars(&remote_backup, &cli_vars).unwrap_or_else(|e| {
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
    script: &str,
    remote_path: Option<String>,

    use_sudo: bool,
    use_rsync: bool,
    silent: bool,
    connect_timeout: Option<Duration>,
    max_channels_per_session: Option<usize>,
    max_sessions_per_server: Option<usize>,
    session_acquire_timeout: Option<Duration>,
    max_session_lifetime: Option<Duration>,
    cli_vars: &HashMap<String, String>,
) -> ExecuteCmdConfig {
    let server_key = ServerPool::generate_server_key(&host, ssh_port.unwrap_or(22), &user);
    let password =
        password.unwrap_or_else(|| prompt_password_or_exit(&user, &host, EXECUTE_TASK_NAME));
    ExecuteCmdConfig {
        server_metadata: ServerMetadata {
            host: host.to_string(),
            user: user.to_string(),
            ssh_port: ssh_port.unwrap_or(22),
            password,
            server_key,

            connect_timeout: connect_timeout.unwrap_or(Duration::from_secs(60)),
            max_channels_per_session: max_channels_per_session.unwrap_or(200),
            max_sessions_per_server: max_sessions_per_server.unwrap_or(200),
            session_acquire_timeout: session_acquire_timeout.unwrap_or(Duration::from_secs(30)),
            max_session_lifetime: max_session_lifetime.unwrap_or(Duration::from_secs(600)),
        },
        use_sudo,
        use_rsync,
        silent,
        script: substitute_vars(&script, &cli_vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(user, host, EXECUTE_TASK_NAME, "{}", e);
            exit(1);
        }),
        remote_path: substitute_vars(&remote_path.unwrap_or_else(|| "~".to_string()), &cli_vars)
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
    // connect_timeout: Option<Duration>,
    // use_sudo: bool,
    // use_rsync: bool,
    // silent: bool,
    properties_file: &str,

    use_sudo: bool,
    use_rsync: bool,
    silent: bool,
    connect_timeout: Option<Duration>,
    max_channels_per_session: Option<usize>,
    max_sessions_per_server: Option<usize>,
    session_acquire_timeout: Option<Duration>,
    max_session_lifetime: Option<Duration>,
    cli_vars: &HashMap<String, String>,
) -> UploadCmdConfig {
    let server_key = ServerPool::generate_server_key(&host, ssh_port.unwrap_or(22), &user);
    let password =
        password.unwrap_or_else(|| prompt_password_or_exit(&user, &host, UPLOAD_TASK_NAME));
    UploadCmdConfig {
        server_metadata: ServerMetadata {
            host: host.to_string(),
            user: user.to_string(),
            ssh_port: ssh_port.unwrap_or(22),
            password,
            server_key,

            connect_timeout: connect_timeout.unwrap_or(Duration::from_secs(60)),
            max_channels_per_session: max_channels_per_session.unwrap_or(200),
            max_sessions_per_server: max_sessions_per_server.unwrap_or(200),
            session_acquire_timeout: session_acquire_timeout.unwrap_or(Duration::from_secs(30)),
            max_session_lifetime: max_session_lifetime.unwrap_or(Duration::from_secs(600)),
        },
        use_sudo,
        use_rsync,
        silent,
        properties_file: substitute_vars(&properties_file, &cli_vars).unwrap_or_else(|e| {
            log_error_with_host_direct!(user, host, UPLOAD_TASK_NAME, "{}", e);
            exit(1);
        }),
    }
}

pub fn parse_upload_configs(
    named_config: &NamedConfig,
    yml_config: &YmlConfig,
) -> Vec<(UploadCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    let servers = resolve_servers(named_config, yml_config);
    let var_map = &yml_config.var_map;
    let common = &yml_config.common;

    for upload in &named_config.upload {
        for server in &servers {
            let password = server.password.clone().unwrap_or_else(|| {
                prompt_password_or_exit(&server.user, &server.host, UPLOAD_TASK_NAME)
            });
            configs.push((
                UploadCmdConfig {
                    server_metadata: ServerMetadata {
                        host: server.host.clone(),
                        user: server.user.clone(),
                        ssh_port: server.ssh_port.unwrap_or(22),
                        password,
                        server_key: ServerPool::generate_server_key(
                            &server.host,
                            server.ssh_port.unwrap_or(22),
                            &server.user,
                        ),
                        connect_timeout: server
                            .connect_timeout
                            .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
                            .unwrap_or(Duration::from_secs(60)),
                        max_channels_per_session: server
                            .max_channels_per_session
                            .or_else(|| common.as_ref()?.server.as_ref()?.max_channels_per_session)
                            .unwrap_or(200),
                        max_sessions_per_server: server
                            .max_sessions_per_server
                            .or_else(|| common.as_ref()?.server.as_ref()?.max_sessions_per_server)
                            .unwrap_or(200),
                        session_acquire_timeout: server
                            .session_acquire_timeout
                            .or_else(|| common.as_ref()?.server.as_ref()?.session_acquire_timeout)
                            .unwrap_or(Duration::from_secs(30)),
                        max_session_lifetime: server
                            .max_session_lifetime
                            .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
                            .unwrap_or(Duration::from_secs(600)),
                    },

                    use_sudo: upload.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                    use_rsync: upload.use_rsync.or(named_config.use_rsync).unwrap_or(false),
                    silent: upload.silent.or(named_config.silent).unwrap_or(false),
                    properties_file: substitute_vars(&upload.properties_file, var_map)
                        .unwrap_or_else(|e| {
                            log_error_with_host_direct!(
                                &server.user,
                                &server.host,
                                UPLOAD_TASK_NAME,
                                "{}",
                                e
                            );
                            exit(1);
                        }),
                },
                var_map.clone(),
            ));
        }
    }
    configs
}

pub fn parse_execute_configs(
    named_config: &NamedConfig,
    yml_config: &YmlConfig,
) -> Vec<(ExecuteCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    let servers = resolve_servers(named_config, yml_config);
    let var_map = &yml_config.var_map;
    let common = &yml_config.common;

    for execute in &named_config.execute {
        for script in &execute.scripts {
            for server in &servers {
                let password = server.password.clone().unwrap_or_else(|| {
                    prompt_password_or_exit(&server.user, &server.host, EXECUTE_TASK_NAME)
                });
                configs.push((
                    ExecuteCmdConfig {
                        server_metadata: ServerMetadata {
                            host: server.host.clone(),
                            user: server.user.clone(),
                            ssh_port: server.ssh_port.unwrap_or(22),
                            password,
                            server_key: ServerPool::generate_server_key(
                                &server.host,
                                server.ssh_port.unwrap_or(22),
                                &server.user,
                            ),
                            connect_timeout: server
                                .connect_timeout
                                .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
                                .unwrap_or(Duration::from_secs(60)),
                            max_channels_per_session: server
                                .max_channels_per_session
                                .or_else(|| {
                                    common.as_ref()?.server.as_ref()?.max_channels_per_session
                                })
                                .unwrap_or(200),
                            max_sessions_per_server: server
                                .max_sessions_per_server
                                .or_else(|| {
                                    common.as_ref()?.server.as_ref()?.max_sessions_per_server
                                })
                                .unwrap_or(200),
                            session_acquire_timeout: server
                                .session_acquire_timeout
                                .or_else(|| {
                                    common.as_ref()?.server.as_ref()?.session_acquire_timeout
                                })
                                .unwrap_or(Duration::from_secs(30)),
                            max_session_lifetime: server
                                .max_session_lifetime
                                .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
                                .unwrap_or(Duration::from_secs(600)),
                        },

                        use_sudo: execute.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                        use_rsync: execute
                            .use_rsync
                            .or(named_config.use_rsync)
                            .unwrap_or(false),
                        silent: execute.silent.or(named_config.silent).unwrap_or(false),
                        script: substitute_vars(script, var_map).unwrap_or_else(|e| {
                            log_error_with_host_direct!(
                                &server.user,
                                &server.host,
                                EXECUTE_TASK_NAME,
                                "{}",
                                e
                            );
                            exit(1);
                        }),
                        remote_path: substitute_vars(
                            &execute
                                .remote_path
                                .clone()
                                .unwrap_or_else(|| "~".to_string()),
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
        }
    }
    configs
}

pub fn parse_patch_configs(
    named_config: &NamedConfig,
    yml_config: &YmlConfig,
) -> Vec<(PatchCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    let servers = resolve_servers(named_config, yml_config);
    let var_map = &yml_config.var_map;
    let common = &yml_config.common;

    for patch in &named_config.patch {
        for server in &servers {
            let password = server.password.clone().unwrap_or_else(|| {
                prompt_password_or_exit(&server.user, &server.host, PATCH_TASK_NAME)
            });
            configs.push((
                PatchCmdConfig {
                    server_metadata: ServerMetadata {
                        host: server.host.clone(),
                        user: server.user.clone(),
                        ssh_port: server.ssh_port.unwrap_or(22),
                        password,
                        server_key: ServerPool::generate_server_key(
                            &server.host,
                            server.ssh_port.unwrap_or(22),
                            &server.user,
                        ),
                        connect_timeout: server
                            .connect_timeout
                            .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
                            .unwrap_or(Duration::from_secs(60)),
                        max_channels_per_session: server
                            .max_channels_per_session
                            .or_else(|| common.as_ref()?.server.as_ref()?.max_channels_per_session)
                            .unwrap_or(200),
                        max_sessions_per_server: server
                            .max_sessions_per_server
                            .or_else(|| common.as_ref()?.server.as_ref()?.max_sessions_per_server)
                            .unwrap_or(200),
                        session_acquire_timeout: server
                            .session_acquire_timeout
                            .or_else(|| common.as_ref()?.server.as_ref()?.session_acquire_timeout)
                            .unwrap_or(Duration::from_secs(30)),
                        max_session_lifetime: server
                            .max_session_lifetime
                            .or_else(|| common.as_ref()?.server.as_ref()?.connect_timeout)
                            .unwrap_or(Duration::from_secs(600)),
                    },

                    use_sudo: patch.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                    use_rsync: patch.use_rsync.or(named_config.use_rsync).unwrap_or(false),
                    silent: patch.silent.or(named_config.silent).unwrap_or(false),

                    recover: patch.recover,
                    local_path: substitute_vars(&patch.local_path, var_map).unwrap_or_else(|e| {
                        log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                        exit(1);
                    }),
                    remote_upload: substitute_vars(&patch.remote_upload, var_map).unwrap_or_else(
                        |e| {
                            log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                            exit(1);
                        },
                    ),
                    remote_path: substitute_vars(&patch.remote_path, var_map).unwrap_or_else(|e| {
                        log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                        exit(1);
                    }),
                    remote_backup: substitute_vars(&patch.remote_backup, var_map).unwrap_or_else(
                        |e| {
                            log_error_with_host_direct!(&server.user, &server.host, PATCH_TASK_NAME, "{}", e);
                            exit(1);
                        },
                    ),
                },
                var_map.clone(),
            ));
        }
    }
    configs
}

fn resolve_servers(named_config: &NamedConfig, yml_config: &YmlConfig) -> Vec<ServerConfig> {
    let mut servers = Vec::new();
    let server_map: std::collections::HashMap<String, ServerConfig> = yml_config
        .servers
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|s| (s.name.clone(), s.clone()))
        .collect();

    for upload in &named_config.upload {
        collect_servers(&server_map, &mut servers, upload, yml_config);
    }
    for patch in &named_config.patch {
        collect_servers(&server_map, &mut servers, patch, yml_config);
    }
    for execute in &named_config.execute {
        collect_servers(&server_map, &mut servers, execute, yml_config);
    }

    servers
}

fn collect_servers<T: TargetConfig>(
    server_map: &std::collections::HashMap<String, ServerConfig>,
    servers: &mut Vec<ServerConfig>,
    config: &T,
    yml_config: &YmlConfig,
) {
    for server_name in config.target_servers() {
        if let Some(server) = server_map.get(server_name) {
            if !servers
                .iter()
                .any(|s: &ServerConfig| s.name == *server_name)
            {
                servers.push(server.clone());
            }
        } else {
            log_error_root!("Server '{}' not found in servers list", server_name);
            exit(1);
        }
    }

    if let Some(group_map) = &yml_config.group_map {
        for group_name in config.target_groups() {
            if let Some(group_servers) = group_map.get(group_name) {
                for server_name in group_servers {
                    if let Some(server) = server_map.get(server_name) {
                        if !servers
                            .iter()
                            .any(|s: &ServerConfig| s.name == *server_name)
                        {
                            servers.push(server.clone());
                        }
                    } else {
                        log_error_root!(
                            "Server '{}' in group '{}' not found in servers list",
                            server_name,
                            group_name
                        );
                        exit(1);
                    }
                }
            } else {
                log_error_root!("Group '{}' not found in group_map list", group_name);
                exit(1);
            }
        }
    }
}
