//! Progress Bar Config

use std::{fmt::Write, time::Duration};

use std::io::IsTerminal;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bytesize::ByteSize;
use indicatif::{HumanDuration, ProgressBar, ProgressState, ProgressStyle};

use clap::Parser;
use conflate::Merge;
use jiff::SignedDuration;
use log::info;

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use rustic_core::{Progress, ProgressBars, ProgressType, RusticProgress};

mod constants {
    use std::time::Duration;

    pub(super) const DEFAULT_INTERVAL: Duration = Duration::from_millis(100);
    pub(super) const DEFAULT_LOG_INTERVAL: Duration = Duration::from_secs(10);
}

/// Progress Bar Config
#[serde_as]
#[derive(Default, Debug, Parser, Clone, Copy, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProgressOptions {
    /// Don't show any progress bar
    #[clap(long, global = true, env = "RUSTIC_NO_PROGRESS")]
    #[merge(strategy=conflate::bool::overwrite_false)]
    pub no_progress: bool,

    /// Interval to update progress bars (default: 100ms)
    #[clap(
        long,
        global = true,
        env = "RUSTIC_PROGRESS_INTERVAL",
        value_name = "DURATION",
        conflicts_with = "no_progress"
    )]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub progress_interval: Option<SignedDuration>,
}

impl ProgressOptions {
    /// Get interval for interactive progress bars
    fn interactive_interval(&self) -> Duration {
        self.progress_interval
            .map_or(constants::DEFAULT_INTERVAL, |i| {
                i.try_into().expect("negative durations are not allowed")
            })
    }

    /// Get interval for non-interactive logging
    fn log_interval(&self) -> Duration {
        self.progress_interval
            .map_or(constants::DEFAULT_LOG_INTERVAL, |i| {
                i.try_into().expect("negative durations are not allowed")
            })
    }

    /// Factory Pattern: Create progress indicator based on terminal capabilities
    ///
    /// * `Hidden`: If --no-progress is set.
    /// * `Interactive`: If running in a TTY.
    /// * `NonInteractive`: If running in a pipe/service (logs to stderr).
    fn create_progress(&self, prefix: &str, kind: ProgressType) -> Progress {
        if self.no_progress {
            return Progress::hidden();
        }

        if std::io::stderr().is_terminal() {
            Progress::new(InteractiveProgress::new(
                prefix,
                kind,
                self.interactive_interval(),
            ))
        } else {
            let interval = self.log_interval();
            if interval > Duration::ZERO {
                Progress::new(NonInteractiveProgress::new(prefix, interval, kind))
            } else {
                Progress::hidden()
            }
        }
    }
}

impl ProgressBars for ProgressOptions {
    fn progress(&self, progress_kind: ProgressType, prefix: &str) -> Progress {
        self.create_progress(prefix, progress_kind)
    }
}

// ================ Interactive ================
/// Wrapper around `indicatif::ProgressBar` for interactive terminal usage
#[derive(Debug, Clone)]
pub struct InteractiveProgress {
    bar: ProgressBar,
    kind: ProgressType,
}

impl InteractiveProgress {
    fn new(prefix: &str, kind: ProgressType, tick_interval: Duration) -> Self {
        let style = Self::initial_style(kind);
        let bar = ProgressBar::new(0).with_style(style);
        bar.set_prefix(prefix.to_string());
        bar.enable_steady_tick(tick_interval);
        Self { bar, kind }
    }

    #[allow(clippy::literal_string_with_formatting_args)]
    fn initial_style(kind: ProgressType) -> ProgressStyle {
        let template = match kind {
            ProgressType::Spinner => "[{elapsed_precise}] {prefix:30} {spinner}",
            ProgressType::Counter => "[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {pos:>10}",
            ProgressType::Bytes => {
                "[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {bytes:>10}            {bytes_per_sec:12}"
            }
        };
        ProgressStyle::default_bar().template(template).unwrap()
    }

    #[allow(clippy::literal_string_with_formatting_args)]
    fn style_with_length(kind: ProgressType) -> ProgressStyle {
        match kind {
            ProgressType::Spinner => Self::initial_style(kind),
            ProgressType::Counter => ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {pos:>10}/{len:10}")
                .unwrap(),
            ProgressType::Bytes => ProgressStyle::default_bar()
                .with_key("my_eta", |s: &ProgressState, w: &mut dyn Write| {
                    let _ = match (s.pos(), s.len()) {
                        (pos, Some(len)) if pos != 0 && len > pos => {
                            let eta_secs = s.elapsed().as_secs() * (len - pos) / pos;
                            write!(w, "{:#}", HumanDuration(Duration::from_secs(eta_secs)))
                        }
                        _ => write!(w, "-"),
                    };
                })
                .template("[{elapsed_precise}] {prefix:30} {bar:40.cyan/blue} {bytes:>10}/{total_bytes:10} {bytes_per_sec:12} (ETA {my_eta})")
                .unwrap(),
        }
    }
}

impl RusticProgress for InteractiveProgress {
    fn is_hidden(&self) -> bool {
        false
    }

    fn set_length(&self, len: u64) {
        if matches!(self.kind, ProgressType::Bytes | ProgressType::Counter) {
            self.bar.set_style(Self::style_with_length(self.kind));
        }
        self.bar.set_length(len);
    }

    fn set_title(&self, title: &str) {
        self.bar.set_prefix(title.to_string());
    }

    fn inc(&self, inc: u64) {
        self.bar.inc(inc);
    }

    fn finish(&self) {
        self.bar.finish_with_message("done");
    }
}

// ================ Non-Interactive ================

/// Store state for non-interactive progress
#[derive(Debug)]
struct NonInteractiveState {
    prefix: String,
    position: u64,
    length: Option<u64>,
    last_log: Instant,
}

/// Periodic logger for non-interactive environments (i.e. systemd)
/// Implemented thread-safe and decouples logging logic from indicatif
#[derive(Clone, Debug)]
pub struct NonInteractiveProgress {
    state: Arc<Mutex<NonInteractiveState>>,
    start: Instant,
    interval: Duration,
    kind: ProgressType,
}

impl NonInteractiveProgress {
    fn new(prefix: &str, interval: Duration, kind: ProgressType) -> Self {
        let now = Instant::now();
        Self {
            state: Arc::new(Mutex::new(NonInteractiveState {
                prefix: prefix.to_string(),
                position: 0,
                length: None,
                last_log: now,
            })),
            start: now,
            interval,
            kind,
        }
    }

    fn format_value(&self, value: u64) -> String {
        match self.kind {
            ProgressType::Bytes => ByteSize(value).to_string(), // delegate bytesize handling
            ProgressType::Counter | ProgressType::Spinner => value.to_string(),
        }
    }

    fn log_progress(&self, state: &NonInteractiveState) {
        let progress = state.length.map_or_else(
            || self.format_value(state.position),
            |len| {
                format!(
                    "{} / {}",
                    self.format_value(state.position),
                    self.format_value(len)
                )
            },
        );
        info!("{}: {}", state.prefix, progress);
    }
}

impl RusticProgress for NonInteractiveProgress {
    fn is_hidden(&self) -> bool {
        false
    }

    fn set_length(&self, len: u64) {
        if let Ok(mut state) = self.state.lock() {
            state.length = Some(len);
        }
    }

    fn set_title(&self, title: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.prefix = title.to_string();
        }
    }

    fn inc(&self, inc: u64) {
        if let Ok(mut state) = self.state.lock() {
            state.position += inc;

            if state.last_log.elapsed() >= self.interval {
                self.log_progress(&state);
                state.last_log = Instant::now();
            }
        }
    }

    fn finish(&self) {
        let Ok(state) = self.state.lock() else {
            return;
        };

        info!(
            "{}: {} done in {:.2?}",
            state.prefix,
            self.format_value(state.position),
            self.start.elapsed()
        );
    }
}
