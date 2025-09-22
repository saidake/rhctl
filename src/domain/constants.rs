
use std::sync::{Mutex};

use once_cell::sync::Lazy;

pub static GLOBAL_LOG_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
