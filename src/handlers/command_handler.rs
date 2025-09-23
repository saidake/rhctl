use log::error;

use crate::common::utils::prompt_password_or_exit;
use crate::domain::cmd_params::{ExecuteCmdConfig, PatchCmdConfig, UploadCmdConfig};
use crate::domain::yml_config::YmlConfig;
use crate::Commands;

// Helper function to get a required field, exiting if not provided
fn require_field<T: Clone>(
    cli_value: Option<T>,
    yaml_specific: Option<T>,
    yaml_common: Option<T>,
    msg: &str,
) -> T {
    cli_value
        .or(yaml_specific)
        .or(yaml_common)
        .unwrap_or_else(|| {
            error!("{}", msg);
            std::process::exit(1);
        })
}

// Helper function to get an optional field with a default
fn optional_field<T: Clone>(
    cli_value: Option<T>,
    yaml_specific: Option<T>,
    yaml_common: Option<T>,
) -> Option<T> {
    cli_value.or(yaml_specific).or(yaml_common)
}

// Helper function to merge boolean flags with OR logic
fn merge_bool_flag(
    cli_value: bool,
    yaml_specific: Option<bool>,
    yaml_common: Option<bool>,
) -> bool {
    cli_value || yaml_specific.unwrap_or(false) || yaml_common.unwrap_or(false)
}

pub fn merge_upload(subcommand: &Commands, yaml: Option<YmlConfig>) -> UploadCmdConfig {
    match subcommand {
        Commands::Upload {
            host,
            ssh_port,
            user,
            password,
            use_sudo,
            use_rsync,
            silent,
            properties_file,
            assets_root,
            ..
        } => {
            let yaml_upload = yaml.as_ref().and_then(|y| y.upload.as_ref());
            let yaml_common = yaml.as_ref().and_then(|y| y.common.as_ref());

            UploadCmdConfig {
                host: require_field(
                    host.clone(),
                    yaml_upload.and_then(|u| u.host.clone()),
                    yaml_common.and_then(|c| c.host.clone()),
                    "host must be provided via --host <host> or set common.host / upload.host in the config YAML file.",
                ),
                user: require_field(
                    user.clone(),
                    yaml_upload.and_then(|u| u.user.clone()),
                    yaml_common.and_then(|c| c.user.clone()),
                    "user must be provided via --user <user> or set common.user / upload.user in the config YAML file.",
                ),
                ssh_port: optional_field(
                    *ssh_port,
                    yaml_upload.and_then(|u| u.ssh_port),
                    yaml_common.and_then(|c| c.ssh_port),
                )
                .unwrap_or(22),
                password: optional_field(
                    password.clone(),
                    yaml_upload.and_then(|u| u.password.clone()),
                    yaml_common.and_then(|c| c.password.clone()),
                )
                .unwrap_or_else(|| prompt_password_or_exit()),
                use_sudo: merge_bool_flag(
                    *use_sudo,
                    yaml_upload.map(|u| u.use_sudo),
                    yaml_common.map(|c| c.use_sudo),
                ),
                use_rsync: merge_bool_flag(
                    *use_rsync,
                    yaml_upload.map(|u| u.use_rsync),
                    yaml_common.map(|c| c.use_rsync),
                ),
                silent: merge_bool_flag(
                    *silent,
                    yaml_upload.map(|u| u.silent),
                    yaml_common.map(|c| c.silent),
                ),



                properties_file: require_field(
                    properties_file.clone(),
                    yaml_upload.map(|u| u.properties_file.clone()),
                    None,
                    "properties-file must be provided via --properties-file <properties-file> or set upload.properties-file in the config YAML file.",
                ),
                assets_root: require_field(
                    assets_root.clone(),
                    yaml_upload.map(|u| u.assets_root.clone()),
                    None,
                    "assets-root must be provided via --assets-root <assets-root> or set upload.assets-root in the config YAML file.",
                ),
            }
        }
        _ => panic!("Command error"),
    }
}

pub fn merge_execute(cli: &Commands, yaml: Option<YmlConfig>) -> ExecuteCmdConfig {
    match cli {
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
            let yaml_execute = yaml.as_ref().and_then(|y| y.execute.as_ref());
            let yaml_common = yaml.as_ref().and_then(|y| y.common.as_ref());

            ExecuteCmdConfig {
                host: require_field(
                    host.clone(),
                    yaml_execute.and_then(|u| u.host.clone()),
                    yaml_common.and_then(|c| c.host.clone()),
                    "host must be provided via --host <host> or set common.host / execute.host in the config YAML file.",
                ),
                user: require_field(
                    user.clone(),
                    yaml_execute.and_then(|u| u.user.clone()),
                    yaml_common.and_then(|c| c.user.clone()),
                    "user must be provided via --user <user> or set common.user / execute.user in the config YAML file.",
                ),
                ssh_port: optional_field(
                    *ssh_port,
                    yaml_execute.and_then(|u| u.ssh_port),
                    yaml_common.and_then(|c| c.ssh_port),
                )
                .unwrap_or(22),
                password: optional_field(
                    password.clone(),
                    yaml_execute.and_then(|u| u.password.clone()),
                    yaml_common.and_then(|c| c.password.clone()),
                )
                .unwrap_or_else(|| prompt_password_or_exit()),
                use_sudo: merge_bool_flag(
                    *use_sudo,
                    yaml_execute.map(|u| u.use_sudo),
                    yaml_common.map(|c| c.use_sudo),
                ),
                use_rsync: merge_bool_flag(
                    *use_rsync,
                    yaml_execute.map(|u| u.use_rsync),
                    yaml_common.map(|c| c.use_rsync),
                ),
                silent: merge_bool_flag(
                    *silent,
                    yaml_execute.map(|u| u.silent),
                    yaml_common.map(|c| c.silent),
                ),



                script: require_field(
                    script.clone(),
                    yaml_execute.map(|u| u.script.clone()),
                    None,
                    "script must be provided via --script <script> or set execute.script in the config YAML file.",
                ),
                remote_path: optional_field(
                    remote_path.clone(),
                    yaml_execute.and_then(|u| u.remote_path.clone()),
                    None,
                )
                .unwrap_or("~".to_string()),
            }
        }
        _ => panic!("Command error"),
    }
}

pub fn merge_patch(cli: &Commands, yaml: Option<YmlConfig>) -> PatchCmdConfig {
    match cli {
        Commands::Patch {
            host,
            ssh_port,
            user,
            password,
            use_sudo,
            use_rsync,
            silent,
            local_patch,
            remote_upload,
            remote_file,
            remote_backup,
            recover,
            ..
        } => {
            let yaml_patch = yaml.as_ref().and_then(|y| y.patch.as_ref());
            let yaml_common = yaml.as_ref().and_then(|y| y.common.as_ref());

            PatchCmdConfig {
                host: require_field(
                    host.clone(),
                    yaml_patch.and_then(|u| u.host.clone()),
                    yaml_common.and_then(|c| c.host.clone()),
                    "host must be provided via --host <host> or set common.host / patch.host in the config YAML file.",
                ),
                user: require_field(
                    user.clone(),
                    yaml_patch.and_then(|u| u.user.clone()),
                    yaml_common.and_then(|c| c.user.clone()),
                    "user must be provided via --user <user> or set common.user / patch.user in the config YAML file.",
                ),
                ssh_port: optional_field(
                    *ssh_port,
                    yaml_patch.and_then(|u| u.ssh_port),
                    yaml_common.and_then(|c| c.ssh_port),
                )
                .unwrap_or(22),
                password: optional_field(
                    password.clone(),
                    yaml_patch.and_then(|u| u.password.clone()),
                    yaml_common.and_then(|c| c.password.clone()),
                )
                .unwrap_or_else(|| prompt_password_or_exit()),
                use_sudo: merge_bool_flag(
                    *use_sudo,
                    yaml_patch.map(|u| u.use_sudo),
                    yaml_common.map(|c| c.use_sudo),
                ),
                use_rsync: merge_bool_flag(
                    *use_rsync,
                    yaml_patch.map(|u| u.use_rsync),
                    yaml_common.map(|c| c.use_rsync),
                ),
                silent: merge_bool_flag(
                    *silent,
                    yaml_patch.map(|u| u.silent),
                    yaml_common.map(|c| c.silent),
                ),
                recover: merge_bool_flag(
                    *recover,
                    yaml_patch.map(|u| u.recover),
                    None
                ),


                local_patch: require_field(
                    local_patch.clone(),
                    yaml_patch.map(|u| u.local_patch.clone()),
                    None,
                    "local-patch must be provided via --local-patch <local-patch> or set patch.local-patch in the config YAML file.",
                ),
                remote_upload: require_field(
                    remote_upload.clone(),
                    yaml_patch.map(|u| u.remote_upload.clone()),
                    None,
                    "remote-upload must be provided via --remote-upload <remote-upload> or set patch.remote-upload in the config YAML file.",
                ),
                remote_file: require_field(
                    remote_file.clone(),
                    yaml_patch.map(|u| u.remote_file.clone()),
                    None,
                    "remote-file must be provided via --remote-file <remote-file> or set patch.remote-file in the config YAML file.",
                ),
                remote_backup: require_field(
                    remote_backup.clone(),
                    yaml_patch.map(|u| u.remote_backup.clone()),
                    None,
                    "remote-backup must be provided via --remote-backup <remote-backup> or set patch.remote-backup in the config YAML file.",
                ),
            }
        }
        _ => panic!("Command error"),
    }
}
