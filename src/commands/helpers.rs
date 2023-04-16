use std::borrow::Cow;
use std::fmt::Write;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use bytesize::ByteSize;
use comfy_table::{
    presets::ASCII_MARKDOWN, Attribute, Cell, CellAlignment, ContentArrangement, Table,
};
use indicatif::HumanDuration;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use lazy_static::lazy_static;
use log::*;
use rayon::ThreadPoolBuilder;

use crate::backend::{FileType, ReadBackend};
use crate::repofile::Id;
use crate::repository::{parse_command, OpenRepository};

pub fn bytes(b: u64) -> String {
    ByteSize(b).to_string_as(true)
}

lazy_static! {
    pub static ref PROGRESS_INTERVAL: Mutex<Duration> = Mutex::new(Duration::from_millis(100));
    pub static ref NO_PROGRESS: Mutex<bool> = Mutex::new(false);
}

fn progress_intervall() -> Duration {
    *PROGRESS_INTERVAL.lock().unwrap()
}

fn is_no_progress() -> bool {
    *NO_PROGRESS.lock().unwrap()
}

pub fn progress_spinner(prefix: impl Into<Cow<'static, str>>) -> ProgressBar {
    if is_no_progress() {
        return no_progress();
    }
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
    if is_no_progress() {
        return no_progress();
    }
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
    if is_no_progress() {
        return no_progress();
    }
    let p = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
            .with_key("my_eta", |s: &ProgressState, w: &mut dyn Write| 
                 match (s.pos(), s.len()){
                    (pos,Some(len)) if pos != 0 => write!(w,"{:#}", HumanDuration(Duration::from_secs(s.elapsed().as_secs() * (len-pos)/pos))),
                    (_, _) => write!(w,"-"),
                }.unwrap())
            .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {bytes:>10}/{total_bytes:10} {bytes_per_sec:12} (ETA {my_eta})")
            .unwrap()
            );
    p.set_prefix(prefix);
    p.enable_steady_tick(progress_intervall());
    p
}

pub fn warm_up_wait(
    repo: &OpenRepository,
    packs: impl ExactSizeIterator<Item = Id>,
    wait: bool,
) -> Result<()> {
    if let Some(command) = &repo.opts.warm_up_command {
        warm_up_command(packs, command)?;
    } else if repo.opts.warm_up {
        warm_up(&repo.be, packs)?;
    }
    if wait {
        if let Some(wait) = repo.opts.warm_up_wait {
            let p = progress_spinner(format!("waiting {wait}..."));
            std::thread::sleep(*wait);
            p.finish();
        }
    }
    Ok(())
}

pub fn warm_up_command(packs: impl ExactSizeIterator<Item = Id>, command: &str) -> Result<()> {
    let p = progress_counter("warming up packs...");
    p.set_length(packs.len() as u64);
    for pack in packs {
        let actual_command = command.replace("%id", &pack.to_hex());
        debug!("calling {actual_command}...");
        let commands = parse_command::<()>(&actual_command)?.1;
        let status = Command::new(commands[0]).args(&commands[1..]).status()?;
        if !status.success() {
            warn!("warm-up command was not successful for pack {pack:?}. {status}");
        }
    }
    p.finish();
    Ok(())
}

pub fn warm_up(be: &impl ReadBackend, packs: impl ExactSizeIterator<Item = Id>) -> Result<()> {
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
