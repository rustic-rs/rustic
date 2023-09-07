use std::borrow::Cow;

use log::info;

/// Trait to report progress information for any rustic action which supports that.
///
/// Implement this trait when you want to display this progress to your users.
pub trait Progress: Send + Sync + Clone {
    /// Check if progress is hidden
    fn is_hidden(&self) -> bool;

    /// Set total length for this progress
    ///
    /// # Arguments
    ///
    /// * `len` - The total length of this progress
    fn set_length(&self, len: u64);

    /// Set title for this progress
    ///
    /// # Arguments
    ///
    /// * `title` - The title of this progress
    fn set_title(&self, title: &'static str);

    /// Advance progress by given increment
    ///
    /// # Arguments
    ///
    /// * `inc` - The increment to advance this progress
    fn inc(&self, inc: u64);

    /// Finish the progress
    fn finish(&self);
}

/// Trait to start progress information report progress information for any rustic action which supports that.
///
/// Implement this trait when you want to display this progress to your users.
pub trait ProgressBars {
    /// The actual type which is able to show the progress
    type P: Progress;

    /// Start a new progress, which is hidden
    fn progress_hidden(&self) -> Self::P;

    /// Start a new progress spinner. Note that this progress doesn't get a length and is not advanced, only finished.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix of the progress
    fn progress_spinner(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;

    /// Start a new progress which counts something
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix of the progress
    fn progress_counter(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;

    /// Start a new progress which counts bytes
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix of the progress
    fn progress_bytes(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
}

/// A dummy struct which shows no progress but only logs titles and end of a progress.
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

/// Don't show progress bars, only log rudimentary progress information.
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
