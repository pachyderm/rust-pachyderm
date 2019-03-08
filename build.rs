#[macro_use]
extern crate failure;
extern crate regex;
extern crate tower_grpc_build;
extern crate walkdir;

use std::env;
use std::fs::{create_dir_all, File};
use std::io::{Error as IoError, Read, Write};
use std::path::{Path, PathBuf};
use std::process::exit;

use regex::Regex;
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

fn find<P>(root: P, min_depth: usize, max_depth: usize, ext: &str) -> Result<Vec<DirEntry>, WalkDirError>
where
    P: AsRef<Path>,
{
    let entries: Result<Vec<DirEntry>, WalkDirError> = WalkDir::new(root)
        .min_depth(min_depth)
        .max_depth(max_depth)
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

fn with_file_contents<F, P>(src: P, dest: &Path, f: F) -> Result<(), BuildError>
where
    F: FnOnce(String) -> String,
    P: AsRef<Path>,
{
    let mut contents = String::new();
    File::open(src)?.read_to_string(&mut contents)?;
    contents = f(contents);
    File::create(&dest)?.write_all(contents.as_bytes())?;
    Ok(())
}

fn run() -> Result<(), BuildError> {
    let gogo_matcher = Regex::new("import.*gogo\\.proto.*;|\\[.*gogoproto.*\\]").unwrap();
    let mut compilable_protos = Vec::new();

    for proto in find("./proto/pachyderm/src/client", 1, 4, "proto")? {
        let src = proto.path();
        let dest = Path::new("./proto/client").join(src.strip_prefix("./proto/pachyderm/src/client").unwrap());

        create_dir_all(dest.parent().unwrap())?;

        with_file_contents(src, &dest, |contents| {
            gogo_matcher.replace_all(&contents, "").to_string()
        })?;

        compilable_protos.push(dest);
    }

    tower_grpc_build::Config::new()
        .enable_client(true)
        .build(&compilable_protos, &["./proto".into()])?;

    // TODO: it seems like prost has substitutions for well-known types built
    // in, but tower-grpc-build doesn't use it:
    // https://github.com/danburkert/prost/blob/2f5d570ce4989b87980f989829577a564da37cb2/prost-build/src/extern_paths.rs
    // Figure out why, so we can remove this hack.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR environment variable not set"));
    for rs in find(out_dir, 0, 1, "rs")? {
        let path = rs.into_path();

        with_file_contents(&path, &path, |contents| {
            contents.replace("super::super::google::protobuf::BytesValue", "::std::vec::Vec<u8>")
        })?;
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        exit(1);
    }
}
