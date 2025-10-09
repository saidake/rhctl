use crate::{ log_error, log_remote};

pub fn execution_print(line: &str, is_stderr: bool) -> Result<(), String> {
    if is_stderr {
        log_error!("{}", line);
        // std::process::exit(1);
    } else {
        log_remote!("{}", line);
    }
    Ok(())
}
