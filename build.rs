#[macro_use]
extern crate failure;
extern crate tonic_build;
extern crate walkdir;

use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::process::exit;

use walkdir::{DirEntry, Error as WalkDirError, WalkDir};

#[derive(Debug, Fail)]
enum BuildError {
    #[fail(display = "Could not walk dir, have you run `make init`?: {}", err)]
    WalkDir { err: WalkDirError },
    #[fail(display = "{}", err)]
    Io { err: IoError },
}

impl From<WalkDirError> for BuildError {
    fn from(err: WalkDirError) -> Self {
        BuildError::WalkDir { err }
    }
}

impl From<IoError> for BuildError {
    fn from(err: IoError) -> Self {
        BuildError::Io { err }
    }
}

fn find<P>(root: P, ext: &str) -> Result<Vec<DirEntry>, WalkDirError>
where
    P: AsRef<Path>,
{
    let entries: Result<Vec<DirEntry>, WalkDirError> = WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let actual_ext = e.path().extension().map(|s| s.to_str());
            e.file_type().is_dir() || actual_ext == Some(Some(ext))
        })
        .collect();

    // We have to do a second pass on the walked results and filter out dirs,
    // because if we do it on the first pass, walkdir won't recurse into
    // directories
    let entries: Vec<DirEntry> = entries?
        .into_iter()
        .filter(|e| {
            let actual_ext = e.path().extension().map(|s| s.to_str());
            actual_ext == Some(Some(ext))
        })
        .collect();

    Ok(entries)
}

fn run() -> Result<(), BuildError> {
    let protos: Vec<PathBuf> = find("./proto", "proto")?.into_iter().map(|e| e.into_path()).collect();

    tonic_build::configure()
        .build_server(false)
        .format(false) // disable code formatting since docs.rs will otherwise break
        .compile(&protos.as_slice(), &["./proto".into()])?;

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        exit(1);
    }

    // Tells cargo to only rebuild if the proto directory (or, implicitly,
    // this file) changed
    println!("cargo:rerun-if-changed=./proto");
}
