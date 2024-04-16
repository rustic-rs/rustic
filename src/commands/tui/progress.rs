use std::io::Stdout;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use bytesize::ByteSize;
use ratatui::{backend::CrosstermBackend, Terminal};
use rustic_core::{Progress, ProgressBars};

use super::widgets::{popup_text, Draw};

#[derive(Clone)]
pub struct TuiProgressBars {
    pub terminal: Arc<RwLock<Terminal<CrosstermBackend<Stdout>>>>,
}

impl TuiProgressBars {
    fn as_progress(&self, progress_type: TuiProgressType, prefix: String) -> TuiProgress {
        TuiProgress {
            terminal: self.terminal.clone(),
            data: Arc::new(RwLock::new(CounterData::new(prefix))),
            progress_type,
        }
    }
}

impl ProgressBars for TuiProgressBars {
    type P = TuiProgress;
    fn progress_hidden(&self) -> Self::P {
        self.as_progress(TuiProgressType::Hidden, String::new())
    }
    fn progress_spinner(&self, prefix: impl Into<std::borrow::Cow<'static, str>>) -> Self::P {
        let progress = self.as_progress(TuiProgressType::Spinner, String::from(prefix.into()));
        progress.popup();
        progress
    }
    fn progress_counter(&self, prefix: impl Into<std::borrow::Cow<'static, str>>) -> Self::P {
        let progress = self.as_progress(TuiProgressType::Counter, String::from(prefix.into()));
        progress.popup();
        progress
    }
    fn progress_bytes(&self, prefix: impl Into<std::borrow::Cow<'static, str>>) -> Self::P {
        let progress = self.as_progress(TuiProgressType::Bytes, String::from(prefix.into()));
        progress.popup();
        progress
    }
}

struct CounterData {
    prefix: String,
    begin: SystemTime,
    length: u64,
    count: u64,
}

impl CounterData {
    fn new(prefix: String) -> Self {
        Self {
            prefix,
            begin: SystemTime::now(),
            length: 0,
            count: 0,
        }
    }
}

#[derive(Clone)]
enum TuiProgressType {
    Hidden,
    Spinner,
    Counter,
    Bytes,
}

#[derive(Clone)]
pub struct TuiProgress {
    terminal: Arc<RwLock<Terminal<CrosstermBackend<Stdout>>>>,
    data: Arc<RwLock<CounterData>>,
    progress_type: TuiProgressType,
}

impl TuiProgress {
    fn popup(&self) {
        let data = self.data.read().unwrap();
        let seconds = data.begin.elapsed().unwrap().as_secs();
        let (minutes, seconds) = (seconds / 60, seconds % 60);
        let (hours, minutes) = (minutes / 60, minutes % 60);
        let message = match self.progress_type {
            TuiProgressType::Spinner => {
                format!("[{hours:02}:{minutes:02}:{seconds:02}]")
            }
            TuiProgressType::Counter => {
                format!(
                    "[{hours:02}:{minutes:02}:{seconds:02}] {}/{}",
                    data.count, data.length
                )
            }
            TuiProgressType::Bytes => {
                format!(
                    "[{hours:02}:{minutes:02}:{seconds:02}] {}/{}",
                    ByteSize(data.count).to_string_as(true),
                    ByteSize(data.length).to_string_as(true)
                )
            }
            TuiProgressType::Hidden => String::new(),
        }
        .into();

        if !matches!(self.progress_type, TuiProgressType::Hidden) {
            let mut popup = popup_text(data.prefix.clone(), message);
            drop(data);
            let mut terminal = self.terminal.write().unwrap();
            _ = terminal
                .draw(|f| {
                    let area = f.size();
                    popup.draw(area, f);
                })
                .unwrap();
        }
    }
}

impl Progress for TuiProgress {
    fn is_hidden(&self) -> bool {
        matches!(self.progress_type, TuiProgressType::Hidden)
    }
    fn set_length(&self, len: u64) {
        self.data.write().unwrap().length = len;
        self.popup();
    }
    fn set_title(&self, title: &'static str) {
        self.data.write().unwrap().prefix = String::from(title);
        self.popup();
    }

    fn inc(&self, inc: u64) {
        self.data.write().unwrap().count += inc;
        self.popup();
    }
    fn finish(&self) {}
}
