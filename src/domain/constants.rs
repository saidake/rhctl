
use std::sync::{Mutex};
use log::{debug, error, info, warn};

use once_cell::sync::Lazy;

pub static GLOBAL_LOG_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
