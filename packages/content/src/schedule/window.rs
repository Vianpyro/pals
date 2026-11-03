use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub opens_at: SystemTime,
    pub closes_at: Option<SystemTime>,
}

impl Window {
    pub fn contains(&self, now: SystemTime) -> bool {
        self.opens_at <= now && self.closes_at.is_none_or(|closes_at| now < closes_at)
    }

    pub(super) fn length(&self) -> Option<Duration> {
        let closes_at = self.closes_at?;
        Some(closes_at.duration_since(self.opens_at).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use crate::schedule::fixtures::{at, between, since};
    use std::time::Duration;

    #[test]
    fn contains_its_opening_instant() {
        assert!(between(10, 20).contains(at(10)));
    }

    #[test]
    fn excludes_instants_before_opening() {
        assert!(!between(10, 20).contains(at(9)));
    }

    #[test]
    fn window_without_closing_never_closes() {
        assert!(!since(10).contains(at(9)));
        assert!(since(10).contains(at(u64::from(u32::MAX))));
    }

    #[test]
    fn reversed_window_has_zero_length() {
        assert_eq!(between(20, 10).length(), Some(Duration::ZERO));
    }

    #[test]
    fn window_without_closing_has_no_length() {
        assert_eq!(since(10).length(), None);
    }
}
