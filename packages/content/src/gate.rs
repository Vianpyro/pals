//! The single gate every read of an exercise goes through (ADR-0020).

use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use contracts::Id;

use crate::{Schedule, active_release, contest, exercise::exercise_dir};

#[derive(Debug, Clone, Copy)]
pub struct Seat<'a> {
    pub offering: &'a str,
    pub sitting: &'a str,
    pub student: &'a str,
}

pub fn accessible_exercise(
    course_dir: &Path,
    id: &Id,
    schedule: &Schedule,
    now: SystemTime,
) -> io::Result<PathBuf> {
    let dir = exercise_dir(&active_release(course_dir)?, id)?;
    schedule.is_open(now).ok_or_else(|| closed(id))?;
    Ok(dir)
}

pub fn contest_exercise(
    course_dir: &Path,
    contest_id: &Id,
    exercise: &Id,
    seat: Seat,
    schedule: &Schedule,
    now: SystemTime,
) -> io::Result<PathBuf> {
    let drawn = contest(course_dir, contest_id)?
        .draw(seat.offering, contest_id, seat.sitting, seat.student)
        .contains(&exercise);
    // Same error as a closed exercise: a student must not learn what the pools hold
    drawn.ok_or_else(|| closed(exercise))?;
    accessible_exercise(course_dir, exercise, schedule, now)
}

fn closed(id: &Id) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("exercise {:?} is closed", id.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Window,
        fixtures::{CONTEST, course_with_contest, temp_dir},
    };
    use std::{
        assert_matches, fs,
        time::{Duration, UNIX_EPOCH},
    };

    const ALICE: Seat = Seat {
        offering: "log200-a26",
        sitting: "01",
        student: "alice",
    };

    fn course_with_sum(name: &str) -> PathBuf {
        let course = temp_dir(name);
        fs::write(course.join("current"), "abc123").unwrap();
        fs::create_dir_all(course.join("releases/abc123/exercises/sum")).unwrap();
        course
    }

    fn id(value: &str) -> Id {
        value.parse().unwrap()
    }

    fn open() -> Schedule {
        Schedule {
            offering: Some(Window {
                opens_at: UNIX_EPOCH,
                closes_at: Some(UNIX_EPOCH + Duration::from_mins(1)),
            }),
            ..Default::default()
        }
    }

    fn weekly(course: &Path, exercise: &str, schedule: &Schedule) -> io::Result<PathBuf> {
        contest_exercise(
            course,
            &id("weekly"),
            &id(exercise),
            ALICE,
            schedule,
            UNIX_EPOCH,
        )
    }

    #[test]
    fn open_exercise_resolves() {
        let course = course_with_sum("gate-open");
        assert_eq!(
            accessible_exercise(&course, &id("sum"), &open(), UNIX_EPOCH).unwrap(),
            course.join("releases/abc123/exercises/sum")
        );
    }

    #[test]
    fn closed_exercise_is_denied() {
        let course = course_with_sum("gate-closed");
        assert_matches!(
            accessible_exercise(&course, &id("sum"), &Schedule::default(), UNIX_EPOCH),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn drawn_exercise_resolves() {
        let course = course_with_contest("gate-drawn", CONTEST);
        assert_eq!(
            weekly(&course, "a", &open()).unwrap(),
            course.join("releases/abc123/exercises/a")
        );
    }

    #[test]
    fn undrawn_exercise_is_denied() {
        let course = course_with_contest("gate-undrawn", CONTEST);
        assert_matches!(
            weekly(&course, "b", &open()),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn closed_contest_is_denied() {
        let course = course_with_contest("gate-contest-closed", CONTEST);
        assert_matches!(
            weekly(&course, "a", &Schedule::default()),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied
        );
    }
}
