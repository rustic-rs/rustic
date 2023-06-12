use std::borrow::Cow;

pub trait Progress: Send + Sync + Clone {
    fn is_hidden(&self) -> bool;
    fn set_length(&self, len: u64);
    fn set_title(&self, title: &'static str);
    fn inc(&self, inc: u64);
    fn finish(&self);
}

pub trait ProgressBars {
    type P: Progress;
    fn progress_spinner(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
    fn progress_counter(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
    fn progress_hidden(&self) -> Self::P;
    fn progress_bytes(&self, prefix: impl Into<Cow<'static, str>>) -> Self::P;
}
