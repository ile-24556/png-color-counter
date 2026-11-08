use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Result, ensure};
use rayon::prelude::*;

mod png;

fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    ensure!(
        args.len() >= 2,
        "specify at least one target file or directory"
    );

    let target_files = {
        let mut target_files = Vec::new();
        for arg in args {
            let arg = Path::new(&arg);
            if arg.is_file() {
                target_files.push(arg.to_owned());
            } else if arg.is_dir() {
                target_files.extend(list_files(arg)?);
            } else {
                eprintln!("skipping target that does not exist: {arg:?}")
            }
        }
        target_files.sort();
        target_files.dedup();
        target_files
    };

    target_files.par_iter().for_each(process);

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

fn process(file: &PathBuf) {
    if file.extension() != Some(OsStr::new("png")) {
        return;
    }

    match png::count_colors(file) {
        Ok(colors) => println!("{file:?}: {colors} colors"),
        Err(e) => eprintln!("{file:?}: {e}"),
    };
}
