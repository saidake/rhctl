/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * SPDX-License-Identifier: Apache-2.0
 * **************************************************************************
 * SSH operation utils.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::sync::Arc;

use crate::{domain::cmd_params::ServerMetadata, log_error, log_remote};

pub fn execution_print(
    server_metadata: &Arc<ServerMetadata>,
    task_name: &str,
    line: &str,
    is_stderr: bool,
) -> Result<(), String> {
    //     let debug_line: String = line.chars().map(|c| match c {
    //     '\r' => "\\r".to_string(),
    //     '\n' => "\\n".to_string(),
    //     other => other.to_string(),
    // }).collect();
    // println!("[DEBUG RAW LINE] {}", debug_line);

    let mut line_clean = line.trim_matches('\r');

    if let Some(pos) = line_clean.rfind('\r') {
        line_clean = &line_clean[pos + 1..];
    }

    if is_stderr {
        log_error!(server_metadata, task_name, "{}", line_clean);
    } else {
        log_remote!(server_metadata, task_name, "{}", line_clean);
    }

    Ok(())
}
