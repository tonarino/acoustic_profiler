use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get current timestamp as the `Duration` since the UNIX epoch.
pub fn current_timestamp() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get current UNIX time")
}
