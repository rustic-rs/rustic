use bytesize::ByteSize;
use comfy_table::{
    presets::ASCII_MARKDOWN, Attribute, Cell, CellAlignment, ContentArrangement, Table,
};

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
        .load_preset(ASCII_MARKDOWN)
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
    ByteSize(b).to_string_as(true)
}
