/*
 * Copyright (C) 2022-2026 rhctl Contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 * **************************************************************************
 * Global constants.
 *
 * Since: 1.0.0
 * Date: October 16, 2025
 */
use std::{collections::HashSet, time::Duration};

pub static REMOTE_TEMP_RHCTL_FOLDER: &str = "/tmp/rhctl";
pub static USER_ABORTED_MESSAGE: &str = "Operation aborted by user";

pub const UPLOAD_TASK_NAME: &str = "UPLOAD";
pub const EXECUTE_TASK_NAME: &str = "EXECUTE";
pub const PATCH_TASK_NAME: &str = "PATCH";
pub const SYSTEM_TASK_NAME: &str = "SYSTEM";

// Log info
pub const LOG_INFO: &str = "INFO";
pub const LOG_ERROR: &str = "ERROR";
pub const LOG_WARN: &str = "WARN";
pub const LOG_DEBUG: &str = "DEBUG";
pub const LOG_REMOTE: &str = "REMOTE";
pub const LOG_LOCAL: &str = "LOCAL";
pub const LOG_ASK: &str = "ASK";
pub const LOG_SHUTDOWN: &str = "SHUTDOWN";

pub const LOG_TASK_NAME_WIDTH: usize = 7;
pub const LOG_LEVEL_WIDTH: usize = 6;

// Default Config
pub const DEFAULT_SSH_PORT: u16 = 22;
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_MAX_CHANNELS_PER_SESSION: usize = 200;
pub const DEFAULT_MAX_SESSIONS_PER_SERVER: usize = 200;
pub const DEFAULT_SESSION_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_MAX_SESSION_LIFETIME: Duration = Duration::from_secs(600);

pub const DEFAULT_EXECUTE_WORK_PATH: &str = "~";
pub const DEFAULT_EXECUTE_MODE: &str = "sync";

// Connection Config
pub const DEFAULT_SSH_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

// Error Handle
pub const SUDO_ERR_MSG: &str = "sudo: a password is required";

pub static DANGEROUS_PATHS: once_cell::sync::Lazy<HashSet<&'static str>> =
    once_cell::sync::Lazy::new(|| {
        HashSet::from([
            "", "/", "/home", "/root", "/etc", "/usr", "/bin", "/sbin", "/var", "/tmp", "/lib",
            "/lib64", "/opt", "/dev", "/proc", "/sys", "~",
        ])
    });
