use log::error;
use std::collections::HashMap;
use std::process::exit;

use crate::domain::cmd_params::{ExecuteCmdConfig, PatchCmdConfig, UploadCmdConfig};
use crate::domain::yml_config::{
    ExecuteConfig, NamedConfig, PatchConfig, ServerConfig, UploadConfig, YmlConfig,
};
use crate::utils::file_utils::substitute_vars;
use crate::utils::log_utils::{ask_user, ask_user_and_abort, prompt_password_or_exit};

trait TargetConfig {
    fn target_servers(&self) -> &Vec<String>;
    fn target_groups(&self) -> &Vec<String>;
}

impl TargetConfig for UploadConfig {
    fn target_servers(&self) -> &Vec<String> {
        &self.target_servers
    }
    fn target_groups(&self) -> &Vec<String> {
        &self.target_groups
    }
}

impl TargetConfig for PatchConfig {
    fn target_servers(&self) -> &Vec<String> {
        &self.target_servers
    }
    fn target_groups(&self) -> &Vec<String> {
        &self.target_groups
    }
}

impl TargetConfig for ExecuteConfig {
    fn target_servers(&self) -> &Vec<String> {
        &self.target_servers
    }
    fn target_groups(&self) -> &Vec<String> {
        &self.target_groups
    }
}

pub fn parse_upload_configs(
    named_config: &NamedConfig,
    yml_config: &YmlConfig
) -> Vec<(UploadCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    let servers = resolve_servers(named_config, yml_config);
    let vars = &yml_config.vars;

    for upload in &named_config.upload {
        for server in &servers {
            configs.push((
                UploadCmdConfig {
                    host: server.host.clone(),
                    user: server.user.clone(),
                    ssh_port: server.ssh_port,
                    password: server
                        .password
                        .clone()
                        .unwrap_or_else(|| prompt_password_or_exit()),
                    use_sudo: upload.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                    use_rsync: upload.use_rsync.or(named_config.use_rsync).unwrap_or(false),
                    silent: upload.silent.or(named_config.silent).unwrap_or(false),
                    properties_file: substitute_vars(&upload.properties_file, vars).unwrap_or_else(
                        |e| {
                            error!("{}", e);
                            exit(1);
                        },
                    ),
                },
                vars.clone(),
            ));
        }
    }
    configs
}

pub fn parse_execute_configs(
    named_config: &NamedConfig,
    yml_config: &YmlConfig
) -> Vec<(ExecuteCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    let servers = resolve_servers(named_config, yml_config);
    let vars = &yml_config.vars;

    for execute in &named_config.execute {
        for script in &execute.scripts {
            for server in &servers {
                configs.push((
                    ExecuteCmdConfig {
                        host: server.host.clone(),
                        user: server.user.clone(),
                        ssh_port: server.ssh_port,
                        password: server
                            .password
                            .clone()
                            .unwrap_or_else(|| prompt_password_or_exit()),
                        use_sudo: execute.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                        use_rsync: execute
                            .use_rsync
                            .or(named_config.use_rsync)
                            .unwrap_or(false),
                        silent: execute.silent.or(named_config.silent).unwrap_or(false),
                        script: substitute_vars(script, vars).unwrap_or_else(|e| {
                            error!("{}", e);
                            exit(1);
                        }),
                        remote_path: substitute_vars(
                            &execute
                                .remote_path
                                .clone()
                                .unwrap_or_else(|| "~".to_string()),
                            vars,
                        )
                        .unwrap_or_else(|e| {
                            error!("{}", e);
                            exit(1);
                        }),
                    },
                    vars.clone(),
                ));
            }
        }
    }
    configs
}

pub fn parse_patch_configs(
    named_config: &NamedConfig,
    yml_config: &YmlConfig
) -> Vec<(PatchCmdConfig, HashMap<String, String>)> {
    let mut configs = Vec::new();
    let servers = resolve_servers(named_config, yml_config);
    let vars = &yml_config.vars;

    for patch in &named_config.patch {
        for server in &servers {
            configs.push((
                PatchCmdConfig {
                    host: server.host.clone(),
                    user: server.user.clone(),
                    ssh_port: server.ssh_port,
                    password: server
                        .password
                        .clone()
                        .unwrap_or_else(|| prompt_password_or_exit()),

                    use_sudo: patch.use_sudo.or(named_config.use_sudo).unwrap_or(false),
                    use_rsync: patch.use_rsync.or(named_config.use_rsync).unwrap_or(false),
                    silent: patch.silent.or(named_config.silent).unwrap_or(false),

                    recover: patch.recover,
                    local_path: substitute_vars(&patch.local_path, vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                    remote_upload: substitute_vars(&patch.remote_upload, vars).unwrap_or_else(
                        |e| {
                            error!("{}", e);
                            exit(1);
                        },
                    ),
                    remote_path: substitute_vars(&patch.remote_path, vars).unwrap_or_else(|e| {
                        error!("{}", e);
                        exit(1);
                    }),
                    remote_backup: substitute_vars(&patch.remote_backup, vars).unwrap_or_else(
                        |e| {
                            error!("{}", e);
                            exit(1);
                        },
                    ),
                },
                vars.clone(),
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
            error!("Server '{}' not found in servers list", server_name);
            exit(1);
        }
    }

    if let Some(groups) = &yml_config.groups {
        for group_name in config.target_groups() {
            if let Some(group_servers) = groups.get(group_name) {
                for server_name in group_servers {
                    if let Some(server) = server_map.get(server_name) {
                        if !servers
                            .iter()
                            .any(|s: &ServerConfig| s.name == *server_name)
                        {
                            servers.push(server.clone());
                        }
                    } else {
                        error!(
                            "Server '{}' in group '{}' not found in servers list",
                            server_name, group_name
                        );
                        exit(1);
                    }
                }
            } else {
                error!("Group '{}' not found in groups list", group_name);
                exit(1);
            }
        }
    }
}
