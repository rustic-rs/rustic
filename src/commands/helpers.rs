use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{bail, Result};
use rpassword::{prompt_password_stderr, read_password_with_reader};
use vlog::*;

use crate::backend::ReadBackend;
use crate::crypto::Key;
use crate::repo::find_key_in_backend;

const MAX_PASSWORD_RETRIES: usize = 5;

pub fn get_key(be: &impl ReadBackend, password_file: Option<PathBuf>) -> Result<Key> {
    let key = match password_file {
        None => (0..MAX_PASSWORD_RETRIES)
            .map(|_| {
                let pass = prompt_password_stderr("enter repository password: ")?;
                find_key_in_backend(be, &pass, None)
            })
            .find(Result::is_ok)
            .unwrap_or_else(|| bail!("tried too often...aborting!"))?,
        Some(file) => {
            let mut file = BufReader::new(File::open(file)?);
            let pass = read_password_with_reader(Some(&mut file))?;
            find_key_in_backend(be, &pass, None)?
        }
    };
    ve1!("password is correct");
    Ok(key)
}
