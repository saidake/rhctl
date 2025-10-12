use std::sync::Arc;

use crate::{domain::cmd_params::ServerMetadata, log_error, log_remote};

pub fn execution_print(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    line: &str,
    is_stderr: bool,
) -> Result<(), String> {
    if is_stderr {
        log_error!(server_metadata,task_name,"{}", line);
        // std::process::exit(1);
    } else {
        log_remote!(server_metadata,task_name,"{}", line);
    }
    Ok(())
}
