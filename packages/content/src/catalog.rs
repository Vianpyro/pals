//! The exercises of a course's active release, open or not (ADR-0020)

use std::{fs, io, path::Path};

use contracts::Id;

use crate::active_release;

// Lists what the gate could resolve; whether each one is open is the schedule's call
pub fn exercises(course_dir: &Path) -> io::Result<Vec<Id>> {
    let entries = match fs::read_dir(active_release(course_dir)?.join("exercises")) {
        Ok(entries) => entries,
        // A release published from content without exercises has no `exercises/`
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry?;

        if !entry.file_type()?.is_dir() {
            continue;
        }

        let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse().ok())
        else {
            continue;
        };

        ids.push(id);
    }
    ids.sort();

    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::temp_dir;
    use std::{assert_matches, path::PathBuf};

    fn course_with_release(name: &str) -> PathBuf {
        let course = temp_dir(name);
        fs::write(course.join("current"), "abc123").unwrap();
        fs::create_dir_all(course.join("releases/abc123")).unwrap();
        course
    }

    fn add_exercise(course: &Path, id: &str) {
        fs::create_dir_all(course.join("releases/abc123/exercises").join(id)).unwrap();
    }

    fn names(ids: &[Id]) -> Vec<&str> {
        ids.iter().map(Id::as_str).collect()
    }

    #[test]
    fn exercises_are_listed_in_order() {
        let course = course_with_release("catalog-order");
        for id in ["sum", "product", "average"] {
            add_exercise(&course, id);
        }
        assert_eq!(
            names(&exercises(&course).unwrap()),
            ["average", "product", "sum"]
        );
    }

    #[test]
    fn what_the_gate_would_refuse_if_not_listed() {
        let course = course_with_release("catalog-refused");
        add_exercise(&course, "sum");
        add_exercise(&course, "Not_An_Id");
        fs::write(course.join("releases/abc123/exercises/notes"), "").unwrap();
        assert_eq!(names(&exercises(&course).unwrap()), ["sum"]);
    }

    #[test]
    fn release_without_exercises_list_none() {
        let course = course_with_release("catalog-empty");
        assert_eq!(exercises(&course).unwrap(), []);
    }

    #[test]
    fn course_without_release_is_not_found() {
        let course = temp_dir("catalog-unpublished");
        assert_matches!(
            exercises(&course),
            Err(error) if error.kind() == io::ErrorKind::NotFound
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_is_not_listed() {
        let course = course_with_release("catalog-link");
        add_exercise(&course, "sum");
        let link = course.join("releases/abc123/exercises/link");
        let _ = fs::remove_file(&link); // left by a previous run
        std::os::unix::fs::symlink(std::env::temp_dir(), &link).unwrap();
        assert_eq!(names(&exercises(&course).unwrap()), ["sum"]);
    }
}
