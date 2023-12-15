//! Progress Bar Config

use std::{borrow::Cow, fmt::Write, time::Duration};

use indicatif::{HumanDuration, ProgressBar, ProgressState, ProgressStyle};

use clap::Parser;
use merge::Merge;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use rustic_core::{Progress, ProgressBars};

/// Progress Bar Config
#[serde_as]
#[derive(Default, Debug, Parser, Clone, Copy, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProgressOptions {
    /// Don't show any progress bar
    #[clap(long, global = true, env = "RUSTIC_NO_PROGRESS")]
    #[merge(strategy=merge::bool::overwrite_false)]
    pub no_progress: bool,

    /// Interval to update progress bars
    #[clap(
        long,
        global = true,
        env = "RUSTIC_PROGRESS_INTERVAL",
        value_name = "DURATION",
        conflicts_with = "no_progress"
    )]
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub progress_interval: Option<humantime::Duration>,
}

impl ProgressOptions {
    /// Get the progress interval
    ///
    /// # Returns
    ///
    /// `Duration::ZERO` if no progress is enabled
    fn progress_interval(&self) -> Duration {
        self.progress_interval.map_or(Duration::ZERO, |i| *i)
    }

    /// Create a hidden progress bar
    pub fn no_progress() -> RusticProgress {
        RusticProgress(ProgressBar::hidden(), ProgressType::Hidden)
    }
}

impl ProgressBars for ProgressOptions {
    type P = RusticProgress;

    fn progress_spinner(&self, prefix: impl Into<Cow<'static, str>>) -> RusticProgress {
        if self.no_progress {
            return Self::no_progress();
        }
        let p = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {prefix:30} {spinner}")
                .unwrap(),
        );
        p.set_prefix(prefix);
        p.enable_steady_tick(self.progress_interval());
        RusticProgress(p, ProgressType::Spinner)
    }

    fn progress_counter(&self, prefix: impl Into<Cow<'static, str>>) -> RusticProgress {
        if self.no_progress {
            return Self::no_progress();
        }
        let p = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {pos:>10}")
                .unwrap(),
        );
        p.set_prefix(prefix);
        p.enable_steady_tick(self.progress_interval());
        RusticProgress(p, ProgressType::Counter)
    }

    fn progress_hidden(&self) -> RusticProgress {
        Self::no_progress()
    }

    fn progress_bytes(&self, prefix: impl Into<Cow<'static, str>>) -> RusticProgress {
        if self.no_progress {
            return Self::no_progress();
        }
        let p = ProgressBar::new(0).with_style(
            ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {bytes:>10}            {bytes_per_sec:12}")
            .unwrap()
            );
        p.set_prefix(prefix);
        p.enable_steady_tick(self.progress_interval());
        RusticProgress(p, ProgressType::Bytes)
    }
}

#[derive(Debug, Clone)]
enum ProgressType {
    Hidden,
    Spinner,
    Counter,
    Bytes,
}

/// A default progress bar
#[derive(Debug, Clone)]
pub struct RusticProgress(ProgressBar, ProgressType);

impl Progress for RusticProgress {
    fn is_hidden(&self) -> bool {
        self.0.is_hidden()
    }

    fn set_length(&self, len: u64) {
        match self.1 {
            ProgressType::Counter => {
                self.0.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {pos:>10}/{len:10}",
                        )
                        .unwrap(),
                );
            }
            ProgressType::Bytes => {
                self.0.set_style(
                    ProgressStyle::default_bar()
                        .with_key("my_eta", |s: &ProgressState, w: &mut dyn Write| 
                            match (s.pos(), s.len()){
                                (pos,Some(len)) if pos != 0 => write!(w,"{:#}", HumanDuration(Duration::from_secs(s.elapsed().as_secs() * (len-pos)/pos))),
                                (_, _) => write!(w,"-"),
                            }.unwrap())
                        .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {bytes:>10}/{total_bytes:10} {bytes_per_sec:12} (ETA {my_eta})")
                        .unwrap()
                );
            }
            _ => {}
        }
        self.0.set_length(len);
    }

    fn set_title(&self, title: &'static str) {
        self.0.set_prefix(title);
    }

    fn inc(&self, inc: u64) {
        self.0.inc(inc);
    }

    fn finish(&self) {
        self.0.finish_with_message("done");
    }
}
