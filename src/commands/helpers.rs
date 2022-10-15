use std::borrow::Cow;
use std::fmt::Write;
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Result};
use bytesize::ByteSize;
use futures::{stream::FuturesUnordered, TryStreamExt};
use indicatif::HumanDuration;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::*;
use rpassword::prompt_password;
use tokio::spawn;
use tokio::time::sleep;

use crate::backend::{DecryptReadBackend, FileType, ReadBackend};
use crate::crypto::Key;
use crate::repo::{find_key_in_backend, Id};

const MAX_PASSWORD_RETRIES: usize = 5;

pub fn bytes(b: u64) -> String {
    ByteSize(b).to_string_as(true)
}

pub fn get_key(be: &impl ReadBackend, password: Option<String>) -> Result<Key> {
    for _ in 0..MAX_PASSWORD_RETRIES {
        match &password {
            // if password is given, directly return the result of find_key_in_backend and don't retry
            Some(pass) => return find_key_in_backend(be, pass, None),
            None => {
                // TODO: Differentiate between wrong password and other error!
                if let Ok(key) =
                    find_key_in_backend(be, &prompt_password("enter repository password: ")?, None)
                {
                    return Ok(key);
                }
            }
        }
    }
    bail!("incorrect password!");
}

pub fn progress_spinner(prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
    let p = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {prefix:30} {spinner}")
            .unwrap(),
    );
    p.set_prefix(prefix);
    p.enable_steady_tick(Duration::from_millis(100));
    p
}

pub fn progress_counter(prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
    let p = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {pos:>10}/{len:10}")
            .unwrap(),
    );
    p.enable_steady_tick(Duration::from_millis(100));
    p.set_prefix(prefix);
    p
}

pub fn no_progress() -> ProgressBar {
    ProgressBar::hidden()
}

pub fn progress_bytes(prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
    let p = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
            .with_key("my_eta", |s: &ProgressState, w: &mut dyn Write| 
                 match (s.pos(), s.len()){
                    (0, _) => write!(w,"-"),
                    (pos,Some(len)) => write!(w,"{:#}", HumanDuration(Duration::from_secs(s.elapsed().as_secs() * (len-pos)/pos))),
                    (_, _) => write!(w,"-"),
                }.unwrap())
            .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {bytes:>10}/{total_bytes:10} {bytes_per_sec:12} (ETA {my_eta})")
            .unwrap()
            );
    p.set_prefix(prefix);
    p.enable_steady_tick(Duration::from_millis(100));
    p
}

pub fn warm_up_command(packs: impl ExactSizeIterator<Item = Id>, command: &str) -> Result<()> {
    let p = progress_counter("warming up packs...");
    p.set_length(packs.len() as u64);
    for pack in packs {
        let id = pack.to_hex();
        let actual_command = command.replace("%id", &id);
        debug!("calling {actual_command}...");
        let mut commands: Vec<_> = actual_command.split(' ').collect();
        let status = Command::new(commands[0])
            .args(&mut commands[1..])
            .status()?;
        if !status.success() {
            bail!("warm-up command was not successful for pack {id}. {status}");
        }
    }
    p.finish();
    Ok(())
}

pub async fn warm_up(
    be: &impl DecryptReadBackend,
    packs: impl ExactSizeIterator<Item = Id>,
) -> Result<()> {
    let mut be = be.clone();
    be.set_option("retry", "false")?;

    let p = progress_counter("warming up packs...");
    p.set_length(packs.len() as u64);
    let mut stream = FuturesUnordered::new();

    const MAX_READER: usize = 20;
    for pack in packs {
        while stream.len() > MAX_READER {
            stream.try_next().await?;
        }

        let p = p.clone();
        let be = be.clone();
        stream.push(spawn(async move {
            // ignore errors as they are expected from the warm-up
            _ = be.read_partial(FileType::Pack, &pack, false, 0, 1);
            p.inc(1);
        }))
    }

    stream.try_collect().await?;
    p.finish();

    Ok(())
}

pub async fn wait(d: Option<humantime::Duration>) {
    if let Some(wait) = d {
        let p = progress_spinner(format!("waiting {}...", wait));
        sleep(*wait).await;
        p.finish();
    }
}
