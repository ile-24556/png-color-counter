use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Result, ensure};

mod png;

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    ensure!(args.len() == 2, "specify exactly one target directory");
    let dir = Path::new(&args[1]);
    ensure!(dir.is_dir(), "specified directory does not exist");

    for file in list_files(dir)? {
        if file.extension() != Some(OsStr::new("png")) {
            continue;
        }

        match png::count_colors(&file) {
            Ok(colors) => println!("{file:?}: {colors} colors"),
            Err(e) => eprintln!("{file:?}: {e}"),
        };
    }

    Ok(())
}

/// Lists files in the directory recursively.
fn list_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in dir.read_dir()? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(list_files(&path)?);
        } else {
            files.push(path)
        }
    }
    Ok(files)
}
