use std::borrow::Cow;
use std::fmt::Write;
use std::process::Command;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{bail, Result};
use bytesize::ByteSize;
use comfy_table::{
    presets::ASCII_MARKDOWN, Attribute, Cell, CellAlignment, ContentArrangement, Table,
};
use indicatif::HumanDuration;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::*;
use rayon::ThreadPoolBuilder;
use rpassword::prompt_password;

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

fn progress_intervall() -> Duration {
    let env_name = "RUSTIC_PROGRESS_INTERVAL";
    std::env::var(env_name)
        .map(|var| {
            humantime::Duration::from_str(&var)
                .expect("{env_name}: please provide a valid duration")
                .into()
        })
        .unwrap_or(Duration::from_millis(100))
}

pub fn progress_spinner(prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
    let p = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {prefix:30} {spinner}")
            .unwrap(),
    );
    p.set_prefix(prefix);
    p.enable_steady_tick(progress_intervall());
    p
}

pub fn progress_counter(prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
    let p = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {pos:>10}/{len:10}")
            .unwrap(),
    );
    p.set_prefix(prefix);
    p.enable_steady_tick(progress_intervall());
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
    p.enable_steady_tick(progress_intervall());
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

pub fn warm_up(
    be: &impl DecryptReadBackend,
    packs: impl ExactSizeIterator<Item = Id>,
) -> Result<()> {
    let mut be = be.clone();
    be.set_option("retry", "false")?;

    let p = progress_counter("warming up packs...");
    p.set_length(packs.len() as u64);

    const MAX_READER: usize = 20;
    let pool = ThreadPoolBuilder::new().num_threads(MAX_READER).build()?;
    let p = &p;
    let be = &be;
    pool.in_place_scope(|s| {
        for pack in packs {
            s.spawn(move |_| {
                // ignore errors as they are expected from the warm-up
                _ = be.read_partial(FileType::Pack, &pack, false, 0, 1);
                p.inc(1);
            });
        }
    });

    p.finish();

    Ok(())
}

pub fn wait(d: Option<humantime::Duration>) {
    if let Some(wait) = d {
        let p = progress_spinner(format!("waiting {}...", wait));
        std::thread::sleep(*wait);
        p.finish();
    }
}

// Helpers for table output

pub fn bold_cell<T: ToString>(s: T) -> Cell {
    Cell::new(s).add_attribute(Attribute::Bold)
}

pub fn table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(ASCII_MARKDOWN)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

pub fn table_with_titles<I: IntoIterator<Item = T>, T: ToString>(titles: I) -> Table {
    let mut table = table();
    table.set_header(titles.into_iter().map(bold_cell));
    table
}

pub fn table_right_from<I: IntoIterator<Item = T>, T: ToString>(start: usize, titles: I) -> Table {
    let mut table = table_with_titles(titles);
    // set alignment of all rows except first start row
    table
        .column_iter_mut()
        .skip(start)
        .for_each(|c| c.set_cell_alignment(CellAlignment::Right));

    table
}
