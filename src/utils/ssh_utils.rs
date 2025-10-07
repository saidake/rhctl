use crate::{common::ssh::ServerHandle, domain::cmd_params::ServerMetadata, log_error, log_remote};
use log::{error, info};

pub fn execution_print(line: &str, is_stderr: bool) -> Result<(), String> {
    if is_stderr {
        log_error!("{}", line);
        // std::process::exit(1);
    } else {
        log_remote!("{}", line);
    }
    Ok(())
}
