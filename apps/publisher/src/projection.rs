//! The public projection of a content repository: only what a release may show (ADR-0005)

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{exercises, no_import_report, statement};

pub type Projection = BTreeMap<PathBuf, Vec<u8>>;

pub fn projection(content: &Path) -> io::Result<Projection> {
    let mut files = Projection::new();

    for exercise in exercises(content)?.into_values() {
        no_import_report(&exercise)?;
        let file = statement(&exercise)?;
        let published = file.strip_prefix(content).map_err(io::Error::other)?;
        files.insert(published.to_path_buf(), fs::read(&file)?);
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_content() -> &'static Path {
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/content"
        ))
    }

    #[test]
    fn only_statements_are_projected() {
        let files = projection(example_content()).unwrap();
        assert_eq!(
            files.keys().map(PathBuf::as_path).collect::<Vec<_>>(),
            [Path::new("exercises/sum/statement.md")]
        );
    }
}
