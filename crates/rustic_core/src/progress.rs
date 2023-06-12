pub trait Progress: Send + Sync + Clone {
    fn is_hidden(&self) -> bool;
    fn set_length(&self, len: u64);
    fn set_title(&self, title: &'static str);
    fn inc(&self, inc: u64);
    fn finish(&self);
}
