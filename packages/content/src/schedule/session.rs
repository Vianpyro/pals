//! End of a session (ADR-0020).

use std::time::SystemTime;

use super::Schedule;

pub fn session_closes_at(students: &[Schedule]) -> Option<SystemTime> {
    let mut latest = None;
    for window in students.iter().filter_map(Schedule::window) {
        let closes_at = window.closes_at?;
        latest = latest.max(Some(closes_at));
    }
    latest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::{
        Accommodation, ExtraTime,
        fixtures::{at, between, since},
    };
    use std::time::Duration;

    fn group_student() -> Schedule {
        Schedule {
            group: Some(between(100, 200)),
            ..Default::default()
        }
    }

    #[test]
    fn closes_at_last_window() {
        let accommodated = Schedule {
            accommodation: Some(Accommodation {
                extra_time: ExtraTime::Percent(50),
                start_shift: Duration::ZERO,
            }),
            mode_applies_accommodation: true,
            ..group_student()
        };
        assert_eq!(
            session_closes_at(&[group_student(), accommodated]),
            Some(at(250))
        );
    }

    #[test]
    fn student_without_window_is_ignored() {
        assert_eq!(
            session_closes_at(&[Schedule::default(), group_student()]),
            Some(at(200))
        );
    }

    #[test]
    fn no_student_window_means_no_closing() {
        assert_eq!(session_closes_at(&[Schedule::default()]), None);
        assert_eq!(session_closes_at(&[]), None);
    }

    #[test]
    fn unbounded_window_keeps_it_open() {
        let practice = Schedule {
            group: Some(since(100)),
            ..Default::default()
        };
        assert_eq!(session_closes_at(&[group_student(), practice]), None);
    }
}
