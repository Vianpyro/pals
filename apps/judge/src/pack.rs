//! A language as data: one directory with its `pack.toml` (ADR-0013)

use std::{collections::HashMap, ffi::OsStr, fs, io, path::Path};

use contracts::Id;
use serde::Deserialize;

use crate::errors::{invalid, located};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub(crate) image: String,
    pub(crate) source: String,
    pub(crate) run: Vec<String>,
}

pub fn read_packs(dir: &Path) -> io::Result<HashMap<Id, Pack>> {
    let packs = fs::read_dir(dir)
        .map_err(|error| located(dir, error))?
        .map(|entry| read_pack(&entry?.path()))
        .collect::<io::Result<HashMap<_, _>>>()?;
    (!packs.is_empty()).ok_or_else(|| invalid(dir, "no language pack"))?;
    Ok(packs)
}

fn read_pack(dir: &Path) -> io::Result<(Id, Pack)> {
    let id = dir
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| name.parse().ok())
        .ok_or_else(|| invalid(dir, "name is not an identifier"))?;
    let file = dir.join("pack.toml");
    let text = fs::read_to_string(&file).map_err(|error| located(&file, error))?;
    let pack: Pack = toml::from_str(&text).map_err(|error| invalid(&file, &error.to_string()))?;
    (!pack.run.is_empty()).ok_or_else(|| invalid(&file, "`run` is empty"))?;
    (Path::new(&pack.source).file_name() == Some(OsStr::new(&pack.source)))
        .ok_or_else(|| invalid(&file, "`source` is not a plain file name"))?;
    Ok((id, pack))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{assert_matches, path::PathBuf};

    fn languages(name: &str, packs: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("judge-test-packs-{name}"));
        let _ = fs::remove_dir_all(&dir); // left by a previous run
        fs::create_dir_all(&dir).unwrap();
        for (pack, text) in packs {
            fs::create_dir(dir.join(pack)).unwrap();
            fs::write(dir.join(pack).join("pack.toml"), text).unwrap();
        }
        dir
    }

    fn id(value: &str) -> Id {
        value.parse().unwrap()
    }

    const PY: &str = r#"
        image = "python:3.15-alpine"
        source = "main.py"
        run = ["python3", "main.py"]
    "#;
    const OTHER: &str = r#"
        image = "other:1"
        source = "Main"
        run = ["other"]
    "#;

    #[track_caller]
    fn assert_refused(name: &str, directory: &str, text: &str) {
        let result = read_packs(&languages(name, &[(directory, text)]));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidData);
    }

    #[test]
    fn packs_are_named_by_directory() {
        let packs = read_packs(&languages("ok", &[("python", PY), ("other", OTHER)])).unwrap();
        assert_eq!(packs.len(), 2);
        let python = &packs[&id("python")];
        assert_eq!(python.image, "python:3.15-alpine");
        assert_eq!(python.source, "main.py");
        assert_eq!(python.run, ["python3", "main.py"]);
        assert_eq!(packs[&id("other")].image, "other:1");
    }

    #[test]
    fn the_repository_packs_are_valid() {
        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packs/languages"
        ));
        assert!(read_packs(dir).unwrap().contains_key(&id("python")));
    }

    #[test]
    fn a_missing_directory_is_named() {
        let dir = Path::new("/no/such/languages");
        let error = read_packs(dir).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("languages"));
    }

    #[test]
    fn no_pack_is_refused() {
        let result = read_packs(&languages("none", &[]));
        assert_matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidData);
    }

    #[test]
    fn missing_pack_toml_is_named() {
        let dir = languages("missing", &[]);
        fs::create_dir(dir.join("java")).unwrap();
        let error = read_packs(&dir).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("pack.toml"));
    }

    #[test]
    fn invalid_directory_name_is_refused() {
        let error = read_packs(&languages("not-an-id", &[("not an id", PY)])).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("not an identifier"));
    }

    #[test]
    fn an_unknown_field_is_refused() {
        assert_refused("typo", "python", &format!("{PY}\ntimout = 3"));
    }

    #[test]
    fn an_empty_run_is_refused() {
        assert_refused(
            "empty-run",
            "python",
            "image = \"i\"\nsource = \"m\"\nrun = []",
        );
    }

    #[test]
    fn source_escape_is_refused() {
        assert_refused(
            "traversal",
            "python",
            "image = \"i\"\nsource = \"../m\"\nrun = [\"m\"]",
        );
    }
}
