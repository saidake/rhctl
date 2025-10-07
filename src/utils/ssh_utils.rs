use crate::{common::ssh::ServerHandle, domain::cmd_params::ServerMetadata, remote};
use log::{error, info};

pub fn execution_print(line: &str, is_stderr: bool) -> Result<(), String> {
    if is_stderr {
        error!("{}", line);
        // std::process::exit(1);
    } else {
        remote!("{}", line);
    }
    Ok(())
}
