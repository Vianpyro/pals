//! Test fixtures shared by the schedule modules.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::Window;

pub(super) fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

pub(super) fn between(opens: u64, closes: u64) -> Window {
    Window {
        opens_at: at(opens),
        closes_at: Some(at(closes)),
    }
}

pub(super) fn since(opens: u64) -> Window {
    Window {
        opens_at: at(opens),
        closes_at: None,
    }
}
