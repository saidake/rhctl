use std::time::Duration;


pub static REMOTE_TEMP_SBXCTL_FOLDER: &str = "/tmp/sbxctl";
pub static USER_ABORTED_MESSAGE: &str = "Operation aborted by user";


pub const UPLOAD_TASK_NAME: &str = "UPLOAD";
pub const EXECUTE_TASK_NAME: &str = "EXECUTE";
pub const PATCH_TASK_NAME: &str = "PATCH";
pub const SYSTEM_TASK_NAME: &str = "SYSTEM";


// >>> Log info
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

// >>> Default Config
pub const DEFAULT_SSH_PORT: u16 = 22;
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_MAX_CHANNELS_PER_SESSION: usize = 200;
pub const DEFAULT_MAX_SESSIONS_PER_SERVER: usize = 200;
pub const DEFAULT_SESSION_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_MAX_SESSION_LIFETIME: Duration = Duration::from_secs(600);

pub const DEFAULT_EXECUTE_REMOTE_PATH: &str = "~";
