//! A submission's code, in the directory its sandboxes mount read-only (ADR-0002, ADR-0013)

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Submission {
    dir: PathBuf,
}

impl Submission {
    /// Saved under `name`, replacing what a crash may have left there
    pub fn write(name: &str, file: &str, source: &str) -> io::Result<Self> {
        let dir = env::temp_dir().join(name);
        match fs::remove_dir_all(&dir) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            result => result?,
        }
        fs::create_dir(&dir)?;

        let submission = Self { dir };
        let path = submission.dir.join(file);
        fs::write(&path, source)?;
        open_to_sandbox(&submission.dir)?;
        open_to_sandbox(&path)?;

        Ok(submission)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Submission {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.dir) {
            eprintln!("judge: {}: {error}", self.dir.display());
        }
    }
}

// The sandbox's user is neither the owner nor in the group; set here rather than left to the umask
fn open_to_sandbox(path: &Path) -> io::Result<()> {
    cfg_select! {
        unix => {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        }
        windows => {
            let _ = path;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, source: &str) -> Submission {
        let name = format!("judge-test-submission-{name}");
        Submission::write(&name, "main.py", source).unwrap()
    }

    #[test]
    fn code_uses_the_packs_file_name() {
        let submission = write("saved", "print(1)");
        assert_eq!(
            fs::read_to_string(submission.dir().join("main.py")).unwrap(),
            "print(1)"
        );
    }

    #[test]
    fn what_a_crash_left_is_replaced() {
        let left = write("crash", "old");
        fs::write(left.dir().join("stale"), "").unwrap();
        let dir = left.dir().to_path_buf();
        std::mem::forget(left); // As a crash would

        let submission = write("crash", "new");
        assert_eq!(submission.dir(), dir);
        assert!(!dir.join("stale").exists());
        assert_eq!(fs::read_to_string(dir.join("main.py")).unwrap(), "new");
    }

    #[test]
    fn dropping_it_removes_the_directory() {
        let submission = write("dropped", "");
        let dir = submission.dir().to_path_buf();
        drop(submission);
        assert!(!dir.exists());
    }

    #[test]
    fn a_failed_write_leaves_nothing() {
        let name = "judge-test-submission-failed";
        let result = Submission::write(name, "no/such/directory", "");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(!env::temp_dir().join(name).exists());
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_can_read_any_umask() {
        use std::os::unix::fs::PermissionsExt;
        let submission = write("mode", "");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(submission.dir()), 0o755);
        assert_eq!(mode(&submission.dir().join("main.py")), 0o755);
    }
}
