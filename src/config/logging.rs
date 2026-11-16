use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use anyhow::Result;
use clap::{Parser, ValueHint};
use conflate::Merge;
use log::LevelFilter;
use log4rs::{
    Handle,
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Logger, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::config::progress_options::multi_progress;

/// Maximum console log records held while the TUI owns the terminal.
const MAX_CAPTURED_CONSOLE_LOGS: usize = 256;

struct ConsoleCaptureState {
    /// Number of live [`TuiLogCapture`] guards.
    depth: usize,
    records: VecDeque<String>,
    dropped: usize,
}

static CONSOLE_CAPTURE: Mutex<ConsoleCaptureState> = Mutex::new(ConsoleCaptureState {
    depth: 0,
    records: VecDeque::new(),
    dropped: 0,
});

fn console_capture() -> MutexGuard<'static, ConsoleCaptureState> {
    CONSOLE_CAPTURE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Divert console log output away from the terminal while the TUI is active.
///
/// File logging is unaffected. When the last guard is dropped, captured messages
/// are printed to stderr (after the TUI should have restored the terminal).
#[derive(Debug)]
pub struct TuiLogCapture {
    _private: (),
}

impl TuiLogCapture {
    /// Start capturing console logs until this guard is dropped.
    #[must_use = "console logs are only captured while this guard is alive"]
    pub fn start() -> Self {
        let mut state = console_capture();
        if state.depth == 0 {
            state.records.clear();
            state.dropped = 0;
        }
        state.depth = state.depth.saturating_add(1);
        drop(state);
        Self { _private: () }
    }
}

impl Drop for TuiLogCapture {
    fn drop(&mut self) {
        let captured = {
            let mut state = console_capture();
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                Some((
                    std::mem::take(&mut state.records),
                    std::mem::take(&mut state.dropped),
                ))
            } else {
                None
            }
        };

        if let Some((records, dropped)) = captured {
            write_captured_console_logs(std::io::stderr(), records, dropped);
        }
    }
}

fn write_captured_console_logs(mut writer: impl Write, records: VecDeque<String>, dropped: usize) {
    if dropped > 0 {
        _ = writeln!(writer, "[{dropped} older log messages omitted]");
    }
    for record in records {
        _ = writeln!(writer, "{record}");
    }
}

/// Capture `record` when a TUI session owns the terminal.
///
/// Returns `true` if the record was captured and must not be written to the
/// console (which would overwrite the TUI).
fn capture_console_log(record: &log::Record<'_>) -> bool {
    let mut state = console_capture();
    if state.depth == 0 {
        return false;
    }

    if state.records.len() >= MAX_CAPTURED_CONSOLE_LOGS {
        _ = state.records.pop_front();
        state.dropped = state.dropped.saturating_add(1);
    }
    state
        .records
        .push_back(format!("[{}] {}", record.level(), record.args()));
    drop(state);
    true
}

/// Logging Config
#[serde_as]
#[derive(Default, Debug, Parser, Clone, Deserialize, Serialize, Merge)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct LoggingOptions {
    /// Use this log level [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL",
        value_parser(["off", "error", "warn", "info", "debug", "trace"]))]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level: Option<String>,

    /// Use this log level for the log file [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL_LOGFILE",
        value_parser(["off", "error", "warn", "info", "debug", "trace"]))]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level_logfile: Option<String>,

    /// Use this log level in dry-run mode [default: info]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL_DRYRUN",
        value_parser(["off", "error", "warn", "info", "debug", "trace"]))]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level_dryrun: Option<String>,

    /// Use this log level for dependencies [default: warn]
    #[clap(long, global = true, env = "RUSTIC_LOG_LEVEL_DEPENDENCIES",
        value_parser(["off", "error", "warn", "info", "debug", "trace"]))]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_level_dependencies: Option<String>,

    /// Write log messages to the given file (using log-level-logfile)
    #[clap(long, global = true, env = "RUSTIC_LOG_FILE", value_name = "LOGFILE", value_hint = ValueHint::FilePath)]
    #[merge(strategy=conflate::option::overwrite_none)]
    pub log_file: Option<PathBuf>,
}

impl LoggingOptions {
    pub fn config(&self, dry_run: bool) -> Result<Config> {
        let log_level = if dry_run {
            &self.log_level_dryrun
        } else {
            &self.log_level
        };

        let level_filter = log_level
            .as_ref()
            .map_or(LevelFilter::Info, |l| l.parse().unwrap());
        let level_filter_logfile = self
            .log_level_logfile
            .as_ref()
            .map_or(LevelFilter::Info, |l| l.parse().unwrap());
        let level_filter_dependencies = self
            .log_level_dependencies
            .as_ref()
            .map_or(LevelFilter::Warn, |l| l.parse().unwrap());

        let stdout = ConsoleAppender::builder()
            .target(Target::Stderr)
            .encoder(Box::new(PatternEncoder::new("{h([{l}])} {m}{n}")))
            .build();
        let stdout = PbPauseAppender(stdout);

        let mut root_builder = Root::builder().appender("stdout");
        let mut config_builder = Config::builder().appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(level_filter)))
                .build("stdout", Box::new(stdout)),
        );

        if let Some(file) = &self.log_file {
            let file_appender = FileAppender::builder()
                .encoder(Box::new(PatternEncoder::new("{d} [{l}] - {m}{n}")))
                .build(file)?;
            root_builder = root_builder.appender("logfile");
            config_builder = config_builder.appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(level_filter_logfile)))
                    .build("logfile", Box::new(file_appender)),
            );
        }

        let root = root_builder.build(level_filter_dependencies);
        let config = config_builder
            .logger(Logger::builder().build("rustic_rs", LevelFilter::Trace))
            .logger(Logger::builder().build("rustic_core", LevelFilter::Trace))
            .logger(Logger::builder().build("rustic_backend", LevelFilter::Trace))
            .build(root)?;
        Ok(config)
    }

    pub fn start_logger(&self, dry_run: bool) -> Result<()> {
        static HANDLE: OnceLock<Handle> = OnceLock::new();

        let config = self.config(dry_run)?;
        if let Some(handle) = HANDLE.get() {
            handle.set_config(config);
        } else {
            let handle = log4rs::init_config(config)?;
            _ = HANDLE.set(handle);
        }
        Ok(())
    }
}

/// Console appender that coordinates with progress bars and the TUI.
///
/// While a [`TuiLogCapture`] guard is active, records are buffered instead of
/// being written to the terminal. Otherwise the indicatif progress bar is
/// suspended for the duration of the write.
#[derive(Debug)]
struct PbPauseAppender(ConsoleAppender);

impl log4rs::append::Append for PbPauseAppender {
    fn append(&self, record: &log::Record<'_>) -> Result<()> {
        if capture_console_log(record) {
            return Ok(());
        }
        multi_progress().suspend(|| self.0.append(record))
    }

    fn flush(&self) {
        // as of log4rs 1.4.0, <ConsoleAppender as Append>::flush does nothing,
        // so we do not need to pause the progress bar here. In the future,
        // if log4rs changes this behavior, we might need to add a suspend here.
        // But that's not necessary right now, so we just call flush directly
        // to avoid unnecessary suspends.
        self.0.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_tests() -> MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn reset_capture() {
        let mut state = console_capture();
        state.depth = 0;
        state.records.clear();
        state.dropped = 0;
    }

    fn capture_warn(msg: &str) -> bool {
        capture_console_log(
            &log::Record::builder()
                .args(format_args!("{msg}"))
                .level(log::Level::Warn)
                .target("test")
                .build(),
        )
    }

    fn snapshot() -> (usize, Vec<String>, usize) {
        let state = console_capture();
        (
            state.depth,
            state.records.iter().cloned().collect(),
            state.dropped,
        )
    }

    fn drain_capture() {
        let mut state = console_capture();
        state.records.clear();
        state.dropped = 0;
    }

    #[test]
    fn console_logs_pass_through_without_tui_capture() {
        let _lock = lock_tests();
        reset_capture();
        assert!(!capture_warn("will retry Read"));
        assert_eq!(snapshot(), (0, Vec::new(), 0));
    }

    #[test]
    fn tui_capture_holds_console_logs_until_drop() {
        let _lock = lock_tests();
        reset_capture();

        let capture = TuiLogCapture::start();
        assert!(capture_warn("will retry Read (attempt 1)"));
        assert!(capture_warn("still reading index"));
        assert_eq!(
            snapshot(),
            (
                1,
                vec![
                    "[WARN] will retry Read (attempt 1)".to_string(),
                    "[WARN] still reading index".to_string(),
                ],
                0
            )
        );

        drain_capture();
        drop(capture);
        assert_eq!(snapshot(), (0, Vec::new(), 0));
        assert!(!capture_warn("after tui"));
    }

    #[test]
    fn nested_tui_capture_flushes_on_outermost_drop() {
        let _lock = lock_tests();
        reset_capture();

        let outer = TuiLogCapture::start();
        assert!(capture_warn("outer"));
        {
            let inner = TuiLogCapture::start();
            assert!(capture_warn("inner"));
            drop(inner);
            assert_eq!(
                snapshot(),
                (
                    1,
                    vec!["[WARN] outer".to_string(), "[WARN] inner".to_string()],
                    0
                )
            );
        }

        drain_capture();
        drop(outer);
        assert_eq!(snapshot(), (0, Vec::new(), 0));
    }

    #[test]
    fn tui_capture_drops_oldest_records_when_full() {
        let _lock = lock_tests();
        reset_capture();

        let capture = TuiLogCapture::start();
        for i in 0..=MAX_CAPTURED_CONSOLE_LOGS {
            assert!(capture_warn(&format!("msg {i}")));
        }

        let (depth, records, dropped) = snapshot();
        assert_eq!(depth, 1);
        assert_eq!(dropped, 1);
        assert_eq!(records.len(), MAX_CAPTURED_CONSOLE_LOGS);
        assert_eq!(records[0], "[WARN] msg 1");
        assert_eq!(
            records[MAX_CAPTURED_CONSOLE_LOGS - 1],
            format!("[WARN] msg {MAX_CAPTURED_CONSOLE_LOGS}")
        );

        drain_capture();
        drop(capture);
    }

    #[test]
    fn captured_logs_replay_with_omission_notice() {
        let mut out = Vec::new();
        write_captured_console_logs(&mut out, VecDeque::from(["[WARN] retry".to_string()]), 2);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "[2 older log messages omitted]\n[WARN] retry\n"
        );
    }
}
