//! The statement of an exercise, in a single format (ADR-0016)

use std::{
    fs, io,
    path::{Path, PathBuf},
};

const FILES: [&str; 3] = ["statement.md", "statement.typ", "statement.tex"];

pub fn statement(exercise: &Path) -> io::Result<PathBuf> {
    let mut found = Vec::new();
    for name in FILES {
        let path = exercise.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => found.push(path),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: expected a regular file", path.display()),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    match <[PathBuf; 1]>::try_from(found) {
        Ok([statement]) => Ok(statement),
        Err(found) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}: expected one of {FILES:?}, found {}",
                exercise.display(),
                found.len()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{assert_invalid, temp_dir};

    fn exercise_with(name: &str, files: &[&str]) -> PathBuf {
        let dir = temp_dir(name);
        for file in files {
            fs::write(dir.join(file), "").unwrap();
        }
        dir
    }

    #[test]
    fn each_format_is_found_alone() {
        for file in FILES {
            let exercise = exercise_with(file, &[file]);
            assert_eq!(statement(&exercise).unwrap(), exercise.join(file));
        }
    }

    #[test]
    fn two_formats_are_ambiguous() {
        let exercise = exercise_with("statement-both", &["statement.md", "statement.tex"]);
        assert_invalid(statement(&exercise));
    }

    #[test]
    fn missing_statement_is_invalid() {
        assert_invalid(statement(&exercise_with("statement-none", &[])));
    }

    #[test]
    fn directory_beside_is_refused() {
        let exercise = exercise_with("statement-dir", &["statement.typ"]);
        fs::create_dir(exercise.join("statement.md")).unwrap();
        assert_invalid(statement(&exercise));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_is_not_a_statement() {
        let exercise = exercise_with("statement-link", &[]);
        std::os::unix::fs::symlink("/etc/passwd", exercise.join("statement.md")).unwrap();
        assert_invalid(statement(&exercise));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_beside_is_refused() {
        let exercise = exercise_with("statement-link-beside", &["statement.typ"]);
        std::os::unix::fs::symlink("/etc/passwd", exercise.join("statement.md")).unwrap();
        assert_invalid(statement(&exercise));
    }
}
