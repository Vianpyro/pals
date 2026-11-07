//! The standard I/O cases of an exercise: each `<name>.in` with its `<name>.out` (ADR-0013, ADR-0023)
//! In `assessment/visible/` or `assessment/hidden/`

use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    errors::{invalid, located},
    sandbox::MAX_OUTPUT,
};

// An expected output beyond what the sandbox keeps could never match
const MAX_CASE_FILE: u64 = MAX_OUTPUT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Case {
    pub visibility: Visibility,
    pub input: Vec<u8>,
    pub expected: Vec<u8>,
}

// Visible cases first, each kind in name order
pub fn read_cases(assessment: &Path) -> io::Result<Vec<Case>> {
    inputs(assessment)?
        .is_empty()
        .ok_or_else(|| invalid(assessment, "case outside `visible/` and `hidden/`"))?;
    let mut cases = read_kind(&assessment.join("visible"), Visibility::Visible)?;
    cases.extend(read_kind(&assessment.join("hidden"), Visibility::Hidden)?);
    (!cases.is_empty()).ok_or_else(|| invalid(assessment, "no `.in` case"))?;
    Ok(cases)
}

fn read_kind(dir: &Path, visibility: Visibility) -> io::Result<Vec<Case>> {
    inputs(dir)?
        .iter()
        .map(|input| {
            Ok(Case {
                visibility,
                input: read_case_file(input)?,
                expected: read_case_file(&input.with_extension("out"))?,
            })
        })
        .collect()
}

// The `.in` files of `dir` in name order, none if it does not exist
fn inputs(dir: &Path) -> io::Result<Vec<PathBuf>> {
    match fs::symlink_metadata(dir) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        metadata => metadata?
            .is_dir()
            .ok_or_else(|| invalid(dir, "not a directory"))?,
    }

    let mut inputs = fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<PathBuf>>>()?;

    inputs.retain(|path| path.extension() == Some(OsStr::new("in")));
    inputs.sort();
    Ok(inputs)
}

// Untrusted content
fn read_case_file(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| located(path, error))?;
    (metadata.is_file() && metadata.len() <= MAX_CASE_FILE).ok_or_else(|| {
        invalid(
            path,
            &format!("not a regular file of at most {MAX_CASE_FILE} bytes"),
        )
    })?;
    fs::read(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Visibility::{Hidden, Visible};
    use std::assert_matches;

    fn assessment(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("judge-test-cases-{name}"));
        let _ = fs::remove_dir_all(&dir); // left by a previous run
        fs::create_dir_all(&dir).unwrap();
        for (file, contents) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        dir
    }

    fn case(visibility: Visibility, input: &str, expected: &str) -> Case {
        Case {
            visibility,
            input: input.into(),
            expected: expected.into(),
        }
    }

    #[track_caller]
    fn assert_invalid(result: io::Result<Vec<Case>>) {
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidData);
    }

    #[test]
    fn visible_first_then_by_name() {
        let dir = assessment(
            "ordered",
            &[
                ("visible/2.in", "b"),
                ("visible/2.out", "B"),
                ("visible/1.in", "a"),
                ("visible/1.out", "A"),
                ("hidden/1.in", "c"),
                ("hidden/1.out", "C"),
                ("judge.json", "{}"),
            ],
        );
        assert_eq!(
            read_cases(&dir).unwrap(),
            [
                case(Visible, "a", "A"),
                case(Visible, "b", "B"),
                case(Hidden, "c", "C")
            ]
        );
    }

    #[test]
    fn example_has_2_visible_3_hidden() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/content/exercises/sum/assessment");
        let kinds: Vec<Visibility> = read_cases(&dir)
            .unwrap()
            .iter()
            .map(|case| case.visibility)
            .collect();
        assert_eq!(kinds, [Visible, Visible, Hidden, Hidden, Hidden]);
    }

    #[test]
    fn exercise_may_have_only_hidden_cases() {
        let dir = assessment(
            "hidden-only",
            &[("hidden/1.in", "a"), ("hidden/1.out", "A")],
        );
        assert_eq!(read_cases(&dir).unwrap(), [case(Hidden, "a", "A")]);
    }

    #[test]
    fn stray_case_is_invalid() {
        let dir = assessment(
            "outside",
            &[
                ("1.in", "a"),
                ("1.out", "A"),
                ("hidden/2.in", "b"),
                ("hidden/2.out", "B"),
            ],
        );
        assert_invalid(read_cases(&dir));
    }

    #[test]
    fn missing_expected_output_is_named() {
        let dir = assessment("missing", &[("hidden/1.in", "a")]);
        assert_matches!(read_cases(&dir), Err(error) if error.kind() == io::ErrorKind::NotFound && error.to_string().contains("1.out"));
    }

    #[test]
    fn exercise_without_case_is_invalid() {
        assert_invalid(read_cases(&assessment("empty", &[("hidden/1.out", "A")])));
    }

    #[test]
    fn oversized_file_is_invalid() {
        let dir = assessment("oversized", &[("hidden/1.in", "a")]);
        fs::File::create(dir.join("hidden/1.out"))
            .unwrap()
            .set_len(MAX_CASE_FILE + 1)
            .unwrap();
        assert_invalid(read_cases(&dir));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_is_invalid() {
        let dir = assessment("link", &[("hidden/1.out", "A")]);
        std::os::unix::fs::symlink("/etc/passwd", dir.join("hidden/1.in")).unwrap();
        assert_invalid(read_cases(&dir));
    }

    #[cfg(unix)]
    #[test]
    fn linked_folder_is_invalid() {
        let dir = assessment(
            "linked-folder",
            &[("other/1.in", "a"), ("other/1.out", "A")],
        );
        std::os::unix::fs::symlink(dir.join("other"), dir.join("visible")).unwrap();
        assert_invalid(read_cases(&dir));
    }
}
