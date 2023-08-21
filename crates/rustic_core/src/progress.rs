use std::borrow::Cow;

use log::info;

/// Trait to report progress information for any rustic action which supports that.
/// Implement this trait when you want to display this progress to your users.
pub trait Progress: Send + Sync + Clone {
    /// Check if progress is hidden
    fn is_hidden(&self) -> bool;
    /// Set total length for this progress
    fn set_length(&self, len: u64);
    /// Set title for this progress
    fn set_title(&self, title: &'static str);
    /// Advance progress by given increment
    fn inc(&self, inc: u64);
    /// Finish the progress
    fn finish(&self);
}

/// Trait to start progress information report progress information for any rustic action which supports that.
/// Implement this trait when you want to display this progress to your users.
pub trait ProgressBars {
    type P: Progress;
    /// Start a new progress, which is hidden
    fn progress_hidden(&self) -> Self::P;
    /// Start a new progress spinner. Note that this progress doesn't get a length and is not advanced, only finished.
    fn progress_spinner(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
    /// Start a new progress which counts something
    fn progress_counter(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
    /// Start a new progress which counts bytes
    fn progress_bytes(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
}

#[derive(Clone, Copy, Debug)]
pub struct NoProgress;
impl Progress for NoProgress {
    fn is_hidden(&self) -> bool {
        true
    }
    fn set_length(&self, _len: u64) {}
    fn set_title(&self, title: &'static str) {
        info!("{title}");
    }
    fn inc(&self, _inc: u64) {}
    fn finish(&self) {
        info!("finished.");
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NoProgressBars;
impl ProgressBars for NoProgressBars {
    type P = NoProgress;
    fn progress_spinner(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P {
        info!("{}", prefix.into());
        NoProgress
    }
    fn progress_counter(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P {
        info!("{}", prefix.into());
        NoProgress
    }
    fn progress_hidden(&self) -> Self::P {
        NoProgress
    }
    fn progress_bytes(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P {
        info!("{}", prefix.into());
        NoProgress
    }
}
