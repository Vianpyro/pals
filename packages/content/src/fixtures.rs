//! Test fixtures shared by the content modules.

use std::{assert_matches, fs, io, path::PathBuf};

pub(crate) const CONTEST: &str = r#"
[[slots]]
pool = ["a", "b"]
[[slots]]
pool = ["c"]
[[slots]]
pool = ["d"]
"#;

pub(crate) fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("content-test-{name}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn course_with_contest(name: &str, contest: &str) -> PathBuf {
    let course = temp_dir(name);
    fs::write(course.join("current"), "abc123").unwrap();
    let release = course.join("releases/abc123");
    for exercise in ["a", "b", "c", "d"] {
        fs::create_dir_all(release.join("exercises").join(exercise)).unwrap();
    }
    fs::create_dir_all(release.join("contests")).unwrap();
    fs::write(release.join("contests/weekly.toml"), contest).unwrap();
    course
}

#[track_caller]
pub(crate) fn assert_invalid(result: io::Result<PathBuf>) {
    assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidData);
}
