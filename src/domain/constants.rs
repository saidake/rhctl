
use std::sync::{Mutex};

use once_cell::sync::Lazy;

pub static GLOBAL_LOG_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
pub static REMOTE_TEMP_SBXCTL_FOLDER: &str = "/tmp/sbxctl";


