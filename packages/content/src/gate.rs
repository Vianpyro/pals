//! The single gate every read of an exercise goes through (ADR-0020).

use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use contracts::Id;

use crate::{Schedule, active_release, exercise::exercise_dir};

pub fn accessible_exercise(
    course_dir: &Path,
    id: &Id,
    schedule: &Schedule,
    now: SystemTime,
) -> io::Result<PathBuf> {
    let dir = exercise_dir(&active_release(course_dir)?, id)?;
    schedule.is_open(now).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("exercise {:?} is closed", id.as_str()),
        )
    })?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Window, fixtures::temp_dir};
    use std::{
        assert_matches, fs,
        time::{Duration, UNIX_EPOCH},
    };

    fn course_with_sum(name: &str) -> PathBuf {
        let course = temp_dir(name);
        fs::write(course.join("current"), "abc123").unwrap();
        fs::create_dir_all(course.join("releases/abc123/exercises/sum")).unwrap();
        course
    }

    fn sum() -> Id {
        "sum".parse().unwrap()
    }

    #[test]
    fn open_exercise_resolves_to_its_directory() {
        let course = course_with_sum("gate-open");
        let schedule = Schedule {
            offering: Some(Window {
                opens_at: UNIX_EPOCH,
                closes_at: Some(UNIX_EPOCH + Duration::from_mins(1)),
            }),
            ..Default::default()
        };
        assert_eq!(
            accessible_exercise(&course, &sum(), &schedule, UNIX_EPOCH).unwrap(),
            course.join("releases/abc123/exercises/sum")
        );
    }

    #[test]
    fn closed_exercise_is_denied() {
        let course = course_with_sum("gate-closed");
        assert_matches!(
            accessible_exercise(&course, &sum(), &Schedule::default(), UNIX_EPOCH),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied
        );
    }
}
