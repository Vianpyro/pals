//! Resolution of an exercise identifier in a release, the single gate's first step.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use contracts::Id;

pub(super) fn exercise_dir(release: &Path, id: &Id) -> io::Result<PathBuf> {
    let dir = release.join("exercises").join(id.as_str());
    let is_dir = fs::symlink_metadata(&dir)?.is_dir();
    is_dir.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("exercise {:?} is not a directory", id.as_str()),
        )
    })?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{assert_invalid, temp_dir};
    use std::assert_matches;

    fn id(value: &str) -> Id {
        value.parse().unwrap()
    }

    fn release_named(name: &str) -> PathBuf {
        let release = temp_dir(name);
        fs::create_dir_all(release.join("exercises")).unwrap();
        release
    }

    #[test]
    fn known_exercise_resolves() {
        let release = release_named("exercise-known");
        let dir = release.join("exercises").join("sum");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(exercise_dir(&release, &id("sum")).unwrap(), dir);
    }

    #[test]
    fn unknown_exercise_is_not_found() {
        let release = release_named("exercise-unknown");
        assert_matches!(exercise_dir(&release, &id("missing")), Err(error) if error.kind() == io::ErrorKind::NotFound);
    }

    #[test]
    fn file_is_not_an_exercise() {
        let release = release_named("exercise-file");
        fs::write(release.join("exercises").join("notes"), "").unwrap();
        assert_invalid(exercise_dir(&release, &id("notes")));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_is_not_an_exercise() {
        let release = release_named("exercise-link");
        let link = release.join("exercises").join("link");
        let _ = fs::remove_file(&link); // left by a previous run
        // Points to a real directory: following the link would accept it.
        std::os::unix::fs::symlink(std::env::temp_dir(), &link).unwrap();
        assert_invalid(exercise_dir(&release, &id("link")));
    }
}
