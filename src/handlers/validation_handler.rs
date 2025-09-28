use std::process::exit;

use log::error;

use crate::{utils::log_utils::ask_user, Cli};

// Root level
pub fn validate_cli_args(cli: &Cli) {
    match (&cli.config, &cli.config_name, &cli.command) {
        (Some(config_path), Some(config_name), Some(_)) => {
            // Both YAML file and named config + subcommand provided
            if let Err(e) = ask_user(
                &format!(
                    "Using YAML config '{}' (entry '{}'). Other CLI parameters will be ignored. Continue?",
                    config_path, config_name
                ),
                false,
            ) {
                error!("{}", e);
                exit(1);
            }
        }
        (Some(_), Some(_), None) => {
            // Only YAML file + named config provided: proceed
        }
        (Some(_), None, _) | (None, Some(_), _) => {
            // One of config path or config name missing: invalid
            error!("Both --config and --config-name must be provided together.");
            exit(1);
        }
        (None, None, Some(_)) => {
            // Only subcommand provided: proceed
        }
        (None, None, None) => {
            error!("You must provide either a subcommand or both --config and --config-name.");
            exit(1);
        }
    }
}
