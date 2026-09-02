use bytesize::ByteSize;
use comfy_table::{
    Attribute, Cell, CellAlignment, ContentArrangement, Table, presets::ASCII_MARKDOWN,
};

/// Return whether an error was caused by a consumer closing stdout.
pub(crate) fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
            || cause
                .downcast_ref::<serde_json::Error>()
                .is_some_and(|error| error.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe))
    })
}

/// Helpers for table output
/// Create a new bold cell
pub fn bold_cell<T: ToString>(s: T) -> Cell {
    Cell::new(s).add_attribute(Attribute::Bold)
}

/// Create a new table with default settings
#[must_use]
pub fn table() -> Table {
    let mut table = Table::new();
    _ = table
        .load_style(ASCII_MARKDOWN)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

/// Create a new table with titles
///
/// The first row will be bold
pub fn table_with_titles<I: IntoIterator<Item = T>, T: ToString>(titles: I) -> Table {
    let mut table = table();
    _ = table.set_header(titles.into_iter().map(bold_cell));
    table
}

/// Create a new table with titles and right aligned columns
pub fn table_right_from<I: IntoIterator<Item = T>, T: ToString>(start: usize, titles: I) -> Table {
    let mut table = table_with_titles(titles);
    // set alignment of all rows except first start row
    table
        .column_iter_mut()
        .skip(start)
        .for_each(|c| c.set_cell_alignment(CellAlignment::Right));

    table
}

/// Convert a [`ByteSize`] to a human readable string
#[must_use]
pub fn bytes_size_to_string(b: u64) -> String {
    ByteSize(b).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::is_broken_pipe;
    use std::io::{self, Write};

    #[test]
    fn detects_broken_pipe() {
        let error = anyhow::Error::from(io::Error::from(io::ErrorKind::BrokenPipe));

        assert!(is_broken_pipe(&error));
    }

    struct BrokenPipe;

    impl Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn detects_json_broken_pipe() {
        let error = serde_json::to_writer(BrokenPipe, &()).unwrap_err();

        assert!(is_broken_pipe(&anyhow::Error::from(error)));
    }
}
