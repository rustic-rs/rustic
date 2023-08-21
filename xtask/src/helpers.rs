use anyhow::Result as AnyResult;
use dialoguer::{theme::ColorfulTheme, Confirm};
use fs_extra as fsx;
use fsx::dir::CopyOptions;
use glob::glob;
use std::path::{Path, PathBuf};

///
/// Remove a set of files given a glob
///
/// # Errors
/// Fails if listing or removal fails
///
pub fn clean_files(pattern: &str) -> AnyResult<()> {
    let files: Result<Vec<PathBuf>, _> = glob(pattern)?.collect();
    files?.iter().try_for_each(remove_file)
}

///
/// Remove a single file
///
/// # Errors
/// Fails if removal fails
///
pub fn remove_file<P>(path: P) -> AnyResult<()>
where
    P: AsRef<Path>,
{
    fsx::file::remove(path).map_err(anyhow::Error::msg)
}

///
/// Remove a directory with its contents
///
/// # Errors
/// Fails if removal fails
///
pub fn remove_dir<P>(path: P) -> AnyResult<()>
where
    P: AsRef<Path>,
{
    fsx::dir::remove(path).map_err(anyhow::Error::msg)
}

///
/// Check if path exists
///
pub fn exists<P>(path: P) -> bool
where
    P: AsRef<Path>,
{
    std::path::Path::exists(path.as_ref())
}

///
/// Copy entire folder contents
///
/// # Errors
/// Fails if file operations fail
///
pub fn copy_contents<P, Q>(from: P, to: Q, overwrite: bool) -> AnyResult<u64>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let mut opts = CopyOptions::new();
    opts.content_only = true;
    opts.overwrite = overwrite;
    fsx::dir::copy(from, to, &opts).map_err(anyhow::Error::msg)
}

///
/// Prompt the user to confirm an action
///
/// # Panics
/// Panics if input interaction fails
///
pub fn confirm(question: &str) -> bool {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(question)
        .interact()
        .unwrap()
}

/// Gets the cargo root dir
///
pub fn root_dir() -> PathBuf {
    let mut xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    xtask_dir.pop();
    xtask_dir
}
