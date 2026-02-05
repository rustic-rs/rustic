use std::io::Stdout;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use bytesize::ByteSize;
use ratatui::{Terminal, backend::CrosstermBackend};
use rustic_core::{Progress, ProgressBars, ProgressType, RusticProgress};

use super::widgets::{Draw, popup_gauge, popup_text};

#[derive(Debug, Clone)]
pub struct TuiProgressBars {
    pub terminal: Arc<RwLock<Terminal<CrosstermBackend<Stdout>>>>,
}

impl TuiProgressBars {
    fn as_progress(&self, progress_type: ProgressType, prefix: String) -> Progress {
        let progress = TuiProgress {
            terminal: self.terminal.clone(),
            data: Arc::new(RwLock::new(CounterData::new(prefix))),
            progress_type,
        };
        progress.popup();
        Progress::new(progress)
    }
}

impl ProgressBars for TuiProgressBars {
    fn progress(&self, progress_type: ProgressType, prefix: &str) -> Progress {
        self.as_progress(progress_type, prefix.to_string())
    }
}

#[derive(Debug)]
struct CounterData {
    prefix: String,
    begin: SystemTime,
    length: Option<u64>,
    count: u64,
}

impl CounterData {
    fn new(prefix: String) -> Self {
        Self {
            prefix,
            begin: SystemTime::now(),
            length: None,
            count: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TuiProgress {
    terminal: Arc<RwLock<Terminal<CrosstermBackend<Stdout>>>>,
    data: Arc<RwLock<CounterData>>,
    progress_type: ProgressType,
}

fn fmt_duration(d: Duration) -> String {
    let seconds = d.as_secs();
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let (hours, minutes) = (minutes / 60, minutes % 60);
    format!("[{hours:02}:{minutes:02}:{seconds:02}]")
}

impl TuiProgress {
    fn popup(&self) {
        let data = self.data.read().unwrap();
        let elapsed = data.begin.elapsed().unwrap();
        let length = data.length;
        let count = data.count;
        let ratio = match length {
            None | Some(0) => 0.0,
            Some(l) => count as f64 / l as f64,
        };
        let eta = match ratio {
            r if r < 0.01 => " ETA: -".to_string(),
            r if r > 0.999_999 => String::new(),
            r => {
                format!(
                    " ETA: {}",
                    fmt_duration(Duration::from_secs(1) + elapsed.div_f64(r / (1.0 - r)))
                )
            }
        };
        let prefix = &data.prefix;
        let message = match self.progress_type {
            ProgressType::Spinner => {
                format!("{} {prefix}", fmt_duration(elapsed))
            }
            ProgressType::Counter => {
                format!(
                    "{} {prefix} {}{}{eta}",
                    fmt_duration(elapsed),
                    count,
                    length.map_or(String::new(), |l| format!("/{l}"))
                )
            }
            ProgressType::Bytes => {
                format!(
                    "{} {prefix} {}{}{eta}",
                    fmt_duration(elapsed),
                    ByteSize(count).display(),
                    length.map_or(String::new(), |l| format!("/{}", ByteSize(l).display()))
                )
            }
        };
        drop(data);

        let mut terminal = self.terminal.write().unwrap();
        _ = terminal
            .draw(|f| {
                let area = f.area();
                match self.progress_type {
                    ProgressType::Spinner => {
                        let mut popup = popup_text("progress", message.into());
                        popup.draw(area, f);
                    }
                    ProgressType::Counter | ProgressType::Bytes => {
                        let mut popup = popup_gauge("progress", message.into(), ratio);
                        popup.draw(area, f);
                    }
                }
            })
            .unwrap();
    }
}

impl RusticProgress for TuiProgress {
    fn is_hidden(&self) -> bool {
        false
    }
    fn set_length(&self, len: u64) {
        self.data.write().unwrap().length = Some(len);
        self.popup();
    }
    fn set_title(&self, title: &str) {
        self.data.write().unwrap().prefix = title.to_string();
        self.popup();
    }

    fn inc(&self, inc: u64) {
        self.data.write().unwrap().count += inc;
        self.popup();
    }
    fn finish(&self) {}
}
