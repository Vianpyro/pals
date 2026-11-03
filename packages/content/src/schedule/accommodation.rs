use std::time::Duration;

use super::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraTime {
    Percent(u32),
    Fixed(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accommodation {
    pub extra_time: ExtraTime,
    pub start_shift: Duration,
}

impl Accommodation {
    pub(super) fn apply(&self, window: Window) -> Window {
        // Modes that apply accomodations are timed: a window without a closing has nothing to extend
        let Some(length) = window.length().filter(|length| !length.is_zero()) else {
            return window;
        };

        let extra = match self.extra_time {
            ExtraTime::Percent(percent) => length * percent / 100,
            ExtraTime::Fixed(extra) => extra,
        };

        Window {
            opens_at: window.opens_at + self.start_shift,
            closes_at: window
                .closes_at
                .map(|closes_at| closes_at + self.start_shift + extra),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::fixtures::{between, since};

    #[test]
    fn fixed_extra_time_only_moves_the_closing() {
        let accommodation = Accommodation {
            extra_time: ExtraTime::Fixed(Duration::from_mins(1)),
            start_shift: Duration::ZERO,
        };
        assert_eq!(accommodation.apply(between(100, 200)), between(100, 260));
    }

    #[test]
    fn percent_uses_the_length_before_the_shift() {
        let accommodation = Accommodation {
            extra_time: ExtraTime::Percent(10),
            start_shift: Duration::from_secs(30),
        };
        assert_eq!(accommodation.apply(between(100, 200)), between(130, 240));
    }

    #[test]
    fn window_without_closing_is_left_as_is() {
        let accommodation = Accommodation {
            extra_time: ExtraTime::Percent(50),
            start_shift: Duration::from_secs(30),
        };
        assert_eq!(accommodation.apply(since(100)), since(100));
    }
}
