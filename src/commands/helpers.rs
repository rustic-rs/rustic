use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};
use indicatif::HumanDuration;
use indicatif::{ProgressBar, ProgressStyle};
use rpassword::{prompt_password_stderr, read_password_with_reader};
use vlog::*;

use crate::backend::ReadBackend;
use crate::crypto::Key;
use crate::repo::find_key_in_backend;

const MAX_PASSWORD_RETRIES: usize = 5;

pub async fn get_key(be: &impl ReadBackend, password_file: Option<PathBuf>) -> Result<Key> {
    match password_file {
        None => {
            for _i in 0..MAX_PASSWORD_RETRIES {
                let pass = prompt_password_stderr("enter repository password: ")?;
                if let Ok(key) = find_key_in_backend(be, &pass, None).await {
                    ve1!("password is correct");
                    return Ok(key);
                }
            }
            bail!("tried too often...aborting!");
        }
        Some(file) => {
            let mut file = BufReader::new(File::open(file)?);
            let pass = read_password_with_reader(Some(&mut file))?;
            if let Ok(key) = find_key_in_backend(be, &pass, None).await {
                ve1!("password is correct");
                return Ok(key);
            }
        }
    }
    bail!("incorrect password!");
}

pub fn progress_counter() -> ProgressBar {
    if get_verbosity_level() == 1 {
        ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>10}/{len:10}")
                .unwrap(),
        )
    } else {
        ProgressBar::hidden()
    }
}

pub fn progress_bytes() -> ProgressBar {
    if get_verbosity_level() == 1 {
        ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
            .with_key("my_eta", |s| 
                 match (s.pos(), s.len()){
                    (0, _) => "-".to_string(),
                    (pos,Some(len)) => format!("{:#}", HumanDuration(Duration::from_secs(s.elapsed().as_secs() * (len-pos)/pos))),
                    (_, _) => "-".to_string(),
                })
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {bytes:>10}/{total_bytes:10} {bytes_per_sec:12} (ETA {my_eta})")
            .unwrap(),
        )
    } else {
        ProgressBar::hidden()
    }
}
