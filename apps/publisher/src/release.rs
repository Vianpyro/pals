//! Immuable releases named by the hash of their content, and the `current` pointer (ADR-0005)

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::Projection;

pub fn publish(course: &Path, projection: &Projection) -> io::Result<PathBuf> {
    let revision = revision(projection);
    let release = course.join("releases").join(&revision);
    if !release.try_exists()? {
        write_release(&release, projection)?;
    }
    point_to(course, &revision)?;
    Ok(release)
}

// Length-prefixed so two different projections never feed the hash the same bytes
fn revision(projection: &Projection) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in projection {
        for part in [path.as_os_str().as_encoded_bytes(), bytes.as_slice()] {
            hasher.update((part.len() as u64).to_le_bytes());
            hasher.update(part);
        }
    }
    hex::encode(hasher.finalize())
}

fn write_release(release: &Path, projection: &Projection) -> io::Result<()> {
    let staging = release.with_extension("staging");
    if let Err(error) = fs::remove_dir_all(&staging)
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(error);
    }
    fs::create_dir_all(&staging)?;
    for (path, bytes) in projection {
        let file = staging.join(path);
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(file, bytes)?;
    }
    fs::rename(staging, release)
}

fn point_to(course: &Path, revision: &str) -> io::Result<()> {
    let staging = course.join("current.staging");
    fs::write(&staging, revision)?;
    fs::rename(staging, course.join("current"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::temp_dir;

    fn projection_of(statement: &str) -> Projection {
        Projection::from([(
            PathBuf::from("exercises/sum/statement.md"),
            statement.as_bytes().to_vec(),
        )])
    }

    #[test]
    fn pointer_names_the_published_release() {
        let course = temp_dir("release-pointer");
        let release = publish(&course, &projection_of("# Somme")).unwrap();
        assert_eq!(content::active_release(&course).unwrap(), release);
        assert_eq!(
            fs::read_to_string(release.join("exercises/sum/statement.md")).unwrap(),
            "# Somme"
        );
    }

    #[test]
    fn unchanged_content_creates_no_release() {
        let course = temp_dir("release-unchanged");
        let first = publish(&course, &projection_of("# Somme")).unwrap();
        let second = publish(&course, &projection_of("# Somme")).unwrap();
        assert_eq!(first, second);
        assert_eq!(fs::read_dir(course.join("releases")).unwrap().count(), 1);
    }

    #[test]
    fn changed_content_keeps_the_previous_release() {
        let course = temp_dir("release-changed");
        let old = publish(&course, &projection_of("# Somme")).unwrap();
        let new = publish(&course, &projection_of("# Addition")).unwrap();
        assert_ne!(old, new);
        assert!(old.is_dir());
        assert_eq!(content::active_release(&course).unwrap(), new);
    }

    #[test]
    fn interrupted_run_leaves_nothing_in_the_release() {
        let course = temp_dir("release-interrupted");
        let projection = projection_of("# Somme");
        let release = course.join("releases").join(revision(&projection));
        let staging = release.with_extension("staging");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("stale"), "").unwrap();
        publish(&course, &projection).unwrap();
        assert!(!release.join("stale").exists());
    }
}
