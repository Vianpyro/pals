//! The single opening rule (ADR-0020).

mod accommodation;
#[cfg(test)]
mod fixtures;
mod session;
mod window;

pub use accommodation::{Accommodation, ExtraTime};
pub use session::session_closes_at;
pub use window::Window;

use std::time::SystemTime;

#[derive(Debug, Clone, Copy, Default)]
pub struct Schedule {
    pub extension: Option<Window>,
    pub group: Option<Window>,
    pub offering: Option<Window>,
    pub accommodation: Option<Accommodation>,
    pub mode_applies_accommodation: bool,
}

impl Schedule {
    pub fn window(&self) -> Option<Window> {
        let window = self.extension.or(self.group).or(self.offering)?;
        Some(match self.accommodation {
            Some(accommodation) if self.mode_applies_accommodation => accommodation.apply(window),
            _ => window,
        })
    }

    pub fn is_open(&self, now: SystemTime) -> bool {
        self.window().is_some_and(|window| window.contains(now))
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{at, between};
    use super::*;
    use std::time::Duration;

    fn window(opens: u64, closes: u64) -> Option<Window> {
        Some(between(opens, closes))
    }

    fn half_more_time() -> Option<Accommodation> {
        Some(Accommodation {
            extra_time: ExtraTime::Percent(50),
            start_shift: Duration::ZERO,
        })
    }

    #[test]
    fn closed_without_any_window() {
        assert!(!Schedule::default().is_open(at(5)));
    }

    #[test]
    fn extension_wins_over_group() {
        let schedule = Schedule {
            extension: window(0, 10),
            group: window(20, 30),
            ..Default::default()
        };
        assert!(schedule.is_open(at(5)));
    }

    #[test]
    fn group_wins_over_offering() {
        let schedule = Schedule {
            group: window(20, 30),
            offering: window(0, 10),
            ..Default::default()
        };
        assert!(!schedule.is_open(at(5)));
        assert!(schedule.is_open(at(25)));
    }

    #[test]
    fn offering_applies_without_group() {
        let schedule = Schedule {
            offering: window(0, 10),
            ..Default::default()
        };
        assert!(schedule.is_open(at(5)));
    }

    #[test]
    fn closed_at_closing_instant() {
        let schedule = Schedule {
            group: window(0, 10),
            ..Default::default()
        };
        assert!(!schedule.is_open(at(10)));
    }

    #[test]
    fn accommodation_follows_group() {
        let schedule = Schedule {
            group: window(100, 200),
            accommodation: half_more_time(),
            mode_applies_accommodation: true,
            ..Default::default()
        };
        assert!(schedule.is_open(at(249)));
        assert!(!schedule.is_open(at(250)));
    }

    #[test]
    fn mode_can_ignore_accommodation() {
        let schedule = Schedule {
            group: window(100, 200),
            accommodation: half_more_time(),
            ..Default::default()
        };
        assert!(!schedule.is_open(at(220)));
    }

    #[test]
    fn accommodation_adds_to_extension() {
        let schedule = Schedule {
            extension: window(300, 400),
            group: window(100, 200),
            accommodation: half_more_time(),
            mode_applies_accommodation: true,
            ..Default::default()
        };
        assert!(schedule.is_open(at(449)));
    }

    #[test]
    fn start_shift_delays_opening() {
        let schedule = Schedule {
            group: window(100, 200),
            accommodation: Some(Accommodation {
                extra_time: ExtraTime::Fixed(Duration::ZERO),
                start_shift: Duration::from_secs(30),
            }),
            mode_applies_accommodation: true,
            ..Default::default()
        };
        assert!(!schedule.is_open(at(110)));
        assert!(schedule.is_open(at(229)));
    }

    #[test]
    fn extra_time_cannot_open_empty_window() {
        let schedule = Schedule {
            group: window(100, 100),
            accommodation: Some(Accommodation {
                extra_time: ExtraTime::Fixed(Duration::from_mins(1)),
                start_shift: Duration::ZERO,
            }),
            mode_applies_accommodation: true,
            ..Default::default()
        };
        assert!(!schedule.is_open(at(110)));
    }
}
