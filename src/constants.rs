pub mod version;

use std::sync::LazyLock;

pub static POISE_VERSION: &str = "0.6.1";
pub static STARTUP_TIME: LazyLock<std::time::SystemTime> =
    LazyLock::new(std::time::SystemTime::now);
