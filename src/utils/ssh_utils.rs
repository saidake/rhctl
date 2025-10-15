/*
 * Copyright 2025 the original author or authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * **************************************************************************
 * SSH operation utils.
 * 
 * Author: Craig Brown
 * Date: October 16, 2025
 * Since: 1.0.0
 */
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
