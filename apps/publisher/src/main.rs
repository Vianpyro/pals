//! Validates course content and publishes immutable releases.

use std::{
    env, io,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let (Some(content), Some(course)) = (args.next(), args.next()) else {
        eprintln!("usage: publisher <content> <course>");
        return ExitCode::FAILURE;
    };
    match publish(Path::new(&content), Path::new(&course)) {
        Ok(release) => {
            println!("{}", release.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("publisher: {error}");
            ExitCode::FAILURE
        }
    }
}

fn publish(content: &Path, course: &Path) -> io::Result<PathBuf> {
    let projection = publisher::projection(content)?;
    publisher::publish(course, &projection)
}
